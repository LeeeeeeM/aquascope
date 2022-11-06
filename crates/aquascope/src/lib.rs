#![feature(
  rustc_private,
  box_patterns,
  associated_type_defaults,
  min_specialization,
  type_alias_impl_trait,
  generic_associated_types,
  trait_alias,
  let_else,
  let_chains
)]
#![allow(clippy::needless_lifetimes)]

extern crate either;
extern crate polonius_engine;
extern crate rustc_borrowck;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_graphviz;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_index;
extern crate rustc_infer;
extern crate rustc_interface;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_mir_dataflow;
extern crate rustc_mir_transform;
extern crate rustc_serialize;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;
extern crate rustc_trait_selection;
extern crate smallvec;

pub mod analysis;

use serde::Serialize;
use ts_rs::TS;

// re-export Range from Flowistry with TS.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Range {
  pub char_start: usize,
  pub char_end: usize,
  pub byte_start: usize,
  pub byte_end: usize,
  pub filename: String,
}

impl From<flowistry::source_map::Range> for Range {
  fn from(i: flowistry::source_map::Range) -> Self {
    Range {
      char_start: i.char_start,
      char_end: i.char_end,
      byte_start: i.byte_start,
      byte_end: i.byte_end,
      filename: i.filename,
    }
  }
}
