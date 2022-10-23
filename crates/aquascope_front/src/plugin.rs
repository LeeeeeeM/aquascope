use std::{
  env,
  path::PathBuf,
  process::{exit, Command},
  time::Instant,
};

use anyhow::Context;
use aquascope::{
  mir::borrowck_facts,
  source_map::{self, FunctionIdentifier, GraphemeIndices, ToSpan},
};
use clap::{Parser, Subcommand};
use eager;
use rustc_hir::BodyId;
use rustc_interface::interface::Result as RustcResult;
use rustc_middle::ty::TyCtxt;
use rustc_plugin::{RustcPlugin, RustcPluginArgs, Utf8Path};
use serde::{self, Deserialize, Serialize};
use ts_rs::TS;

// XXX this value is not actually used anywhere (currently)
// see below comment for an explanaition why I couldn't get
// what I wanted to working.
const INTERFACE_TYPES_DIR: &str = "../../frontend/interface/";
const VERSION: &str = env!("CARGO_PKG_VERSION");

// FIXME ideally, I would like to say:
// `#[ts(export, export_to = interface_file!("Filename.ts"))]`
// in the attribute, but I couldn't get it to work.
// Below is the hollow shell of what I tried.
//
// I "think" a proc_macro_attribute might work, but I wasn't
// convinced enough to create a new library just for that reason.
// If in the future further macros might be of use then I'd be open
// to the idea. Ideally, I could get concat!(...) to expand eagarly
// *before* the `ts` attribute is expanded, but it seems that is only
// possible with "key-value" attributes, e.g. `#[doc = concat!(...)]`.

// trace_macros!(true);
// eager_macro_rules! { $eager_1
//   macro_rules! weird_inner_macro {
//     ($path:literal, $($tt:tt)*) => {
//       #[derive(Debug, Serialize, Deserialize, TS)]
//       #[ts(export, export_to = $path)]
//       $($tt)*
//     };
//   }
// }

// macro_rules! export_interface_file {
//   ($file:literal, $($tt:tt)*) => {
//     eager!{
//       weird_inner_macro!{
//         concat!("../../frontend/interface/", $file),
//         $($tt)*
//       }
//     }
//   };
// }

#[derive(Debug, Parser, Serialize, Deserialize)]
#[clap(version = VERSION)]
pub struct AquascopePluginArgs {
  #[clap(subcommand)]
  command: AquascopeCommand,
}

#[derive(Debug, Subcommand, Serialize, Deserialize)]
enum AquascopeCommand {
  Source {
    file: String,

    #[clap(last = true)]
    flags: Vec<String>,
  },
  Spans {
    file: String,

    #[clap(last = true)]
    flags: Vec<String>,
  },
  VisMethodCalls {
    file: String,

    #[clap(last = true)]
    flags: Vec<String>,
  },
  Preload,
  RustcVersion,
}

// HACK! FIXME
#[derive(Serialize, TS)]
#[ts(export, export_to = "../../frontend/interface/Range.ts")]
pub struct Range {
  char_start: usize,
  char_end: usize,
  byte_start: usize,
  byte_end: usize,
  filename: String,
}

impl From<source_map::Range> for Range {
  fn from(i: source_map::Range) -> Self {
    Range {
      char_start: i.char_start,
      char_end: i.char_end,
      byte_start: i.byte_start,
      byte_end: i.byte_end,
      filename: i.filename,
    }
  }
}

pub struct AquascopePlugin;
impl RustcPlugin for AquascopePlugin {
  type Args = AquascopePluginArgs;

  fn bin_name() -> String {
    "aquascope-driver".into()
  }

  fn args(
    &self,
    target_dir: &Utf8Path,
  ) -> RustcPluginArgs<AquascopePluginArgs> {
    let args = AquascopePluginArgs::parse_from(env::args().skip(1));

    log::debug!("Provided PluginArgs {args:?}");

    let cargo_path =
      env::var("CARGO_PATH").unwrap_or_else(|_| "cargo".to_string());

    use AquascopeCommand::*;
    match &args.command {
      Preload => {
        let mut cmd = Command::new(cargo_path);
        // Note: this command must share certain parameters with rustc_plugin so Cargo will not recompute
        // dependencies when actually running the driver, e.g. RUSTFLAGS.
        cmd
          .args(&["check", "--all", "--all-features", "--target-dir"])
          .arg(target_dir)
          .env("RUSTFLAGS", "-Awarnings");
        let exit_status = cmd.status().expect("could not run cargo");
        exit(exit_status.code().unwrap_or(-1));
      }
      RustcVersion => {
        let commit_hash =
          rustc_interface::util::commit_hash_str().unwrap_or("unknown");
        println!("{commit_hash}");
        exit(0);
      }
      _ => {}
    };

    let (file, flags) = match &args.command {
      Source { file, flags } => (file, flags),
      Spans { file, flags } => (file, flags),
      VisMethodCalls { file, flags } => (file, flags),
      _ => unreachable!(),
    };

    RustcPluginArgs {
      flags: Some(flags.clone()),
      file: Some(PathBuf::from(file)),
      args,
    }
  }

  fn run(
    self,
    compiler_args: Vec<String>,
    plugin_args: AquascopePluginArgs,
  ) -> RustcResult<()> {
    use AquascopeCommand::*;
    match plugin_args.command {
      Source { file, .. } => {
        postprocess(crate::source::source(&compiler_args, file))
      }
      VisMethodCalls { file, .. } => {
        postprocess(crate::method_receivers::method_calls(&compiler_args, file))
      }
      Spans { file, .. } => {
        postprocess(crate::spans::spans(&compiler_args, file))
      }
      _ => unreachable!(),
    }
  }
}

// TODO you could simplify the interface of the Server by converting
// AquascopeResults into a shared form of Result. Then for native commands
// like below you'd go to a RustcResult whereas the server would turn this
// into an axum Result.
fn postprocess<T: Serialize>(result: AquascopeResult<T>) -> RustcResult<()> {
  let result: Result<T, String> = match result {
    Ok(output) => Ok(output),
    Err(e) => match e {
      AquascopeError::BuildError => {
        return Err(
          rustc_errors::ErrorGuaranteed::unchecked_claim_error_was_emitted(),
        );
      }
      AquascopeError::AnalysisError(msg) => Err(msg),
    },
  };

  println!("{}", serde_json::to_string(&result).unwrap());

  Ok(())
}

pub fn run_with_callbacks(
  args: &[String],
  callbacks: &mut (dyn rustc_driver::Callbacks + Send),
) -> AquascopeResult<()> {
  let mut args = args.to_vec();
  args.extend(
    "-Z identify-regions -Z mir-opt-level=0 -A warnings"
      .split(' ')
      .map(|s| s.to_owned()),
  );

  log::debug!("Running command with callbacks: {args:?}");

  let compiler = rustc_driver::RunCompiler::new(&args, callbacks);
  compiler.run().map_err(|_| AquascopeError::BuildError)
}

fn run<A: AquascopeAnalysis, T: ToSpan>(
  analysis: A,
  target: T,
  args: &[String],
) -> AquascopeResult<A::Output> {
  let mut callbacks = AquascopeCallbacks {
    analysis: Some(analysis),
    target,
    output: None,
    rustc_start: Instant::now(),
    // eval_mode: EVAL_MODE.copied(),
  };

  log::info!("Starting rustc analysis...");
  // log::debug!("Eval mode: {:?}", callbacks.eval_mode);

  run_with_callbacks(args, &mut callbacks)?;

  callbacks
    .output
    .unwrap()
    .map_err(|e| AquascopeError::AnalysisError(e.to_string()))
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(tag = "variant")]
#[ts(export, export_to = "../../frontend/interface/AquascopeError.ts")]
pub enum AquascopeError {
  // An error occured before the intended analysis could run.
  BuildError,
  AnalysisError(String),
}

pub type AquascopeResult<T> = Result<T, AquascopeError>;

pub trait AquascopeAnalysis: Sized + Send + Sync {
  type Output: Serialize + Send + Sync;
  fn analyze(
    &mut self,
    tcx: TyCtxt,
    id: BodyId,
  ) -> anyhow::Result<Self::Output>;
}

// Implement AquascopeAnalysis for all functions with a type signature that matches
// AquascopeAnalysis::analyze
impl<F, O> AquascopeAnalysis for F
where
  F: for<'tcx> Fn<(TyCtxt<'tcx>, BodyId), Output = anyhow::Result<O>>
    + Send
    + Sync,
  O: Serialize + Send + Sync,
{
  type Output = O;
  fn analyze(
    &mut self,
    tcx: TyCtxt,
    id: BodyId,
  ) -> anyhow::Result<Self::Output> {
    (self)(tcx, id)
  }
}

struct AquascopeCallbacks<A: AquascopeAnalysis, T: ToSpan> {
  analysis: Option<A>,
  target: T,
  output: Option<anyhow::Result<A::Output>>,
  rustc_start: Instant,
  // eval_mode: Option<EvalMode>,
}

impl<A: AquascopeAnalysis, T: ToSpan> rustc_driver::Callbacks
  for AquascopeCallbacks<A, T>
{
  fn config(&mut self, config: &mut rustc_interface::Config) {
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  fn after_parsing<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    // elapsed("rustc", self.rustc_start);
    // fluid_set!(EVAL_MODE, self.eval_mode.unwrap_or_default());

    let start = Instant::now();
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      // elapsed("global_ctxt", start);
      let mut analysis = self.analysis.take().unwrap();
      self.output = Some((|| {
        let target = self.target.to_span(tcx)?;
        let mut bodies = source_map::find_enclosing_bodies(tcx, target);
        let body = bodies.next().context("Selection did not map to a body")?;
        analysis.analyze(tcx, body)
      })());
    });

    rustc_driver::Compilation::Stop
  }
}
