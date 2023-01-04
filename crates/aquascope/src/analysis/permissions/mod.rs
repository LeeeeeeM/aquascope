mod context;
mod graphviz;
mod output;
mod places_conflict;
pub mod utils;

use std::ops::{Deref, DerefMut};

pub use context::PermissionsCtxt;
pub use output::{compute, Output};
use polonius_engine::FactTypes;
use rustc_borrowck::consumers::RustcFacts;
use rustc_data_structures::fx::FxHashMap;
use rustc_middle::{
  mir::{Mutability, Place},
  ty::Ty,
};
use serde::Serialize;
use ts_rs::TS;

use crate::{
  analysis::{KeyShifter, LoanKey, MoveKey},
  Range,
};

#[derive(Copy, Clone, Debug)]
pub struct AquascopeFacts;

impl polonius_engine::FactTypes for AquascopeFacts {
  type Origin = <RustcFacts as FactTypes>::Origin;
  type Loan = <RustcFacts as FactTypes>::Loan;
  type Point = <RustcFacts as FactTypes>::Point;
  type Variable = <RustcFacts as FactTypes>::Variable;
  type Path = PathIndex;
}

rustc_index::newtype_index! {
  pub struct PathIndex {
    DEBUG_FORMAT = "path{}"
  }
}

impl polonius_engine::Atom for PathIndex {
  fn index(self) -> usize {
    rustc_index::vec::Idx::index(self)
  }
}

// ------------------------------------------------
// General Information

pub type Path = <AquascopeFacts as FactTypes>::Path;
pub type Point = <AquascopeFacts as FactTypes>::Point;
pub type Loan = <AquascopeFacts as FactTypes>::Loan;
pub type Variable = <AquascopeFacts as FactTypes>::Variable;

// ------------------------------------------------
// Permission Boundaries

#[derive(Clone, Copy, Hash, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Permissions {
  pub read: bool,
  pub write: bool,
  pub drop: bool,
}

// In contrast to Permissions, the PermissionsData stores all relevant
// information about what factors into the permissions. Things like
// declared type information, loan refinements, move refinements, etc.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PermissionsData {
  // Type declaration information
  pub type_droppable: bool,
  pub type_writeable: bool,
  pub type_copyable: bool,

  // Liveness information
  pub is_live: bool,

  // Initialization information
  // TODO: this should be an Option<MoveKey> once moves are tracked.
  pub path_moved: bool,

  // Refinement information
  #[serde(skip_serializing_if = "Option::is_none")]
  pub loan_read_refined: Option<LoanKey>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub loan_write_refined: Option<LoanKey>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub loan_drop_refined: Option<LoanKey>,

  // Permissions can be directly derived from the above
  // information but we don't want that logic duplicated anywhere.
  pub permissions: Permissions,
}

#[derive(Debug, Clone, Serialize, PartialEq, TS)]
#[ts(export)]
pub enum Refiner {
  Loan(LoanKey),
  Move(MoveKey),
}

#[derive(Debug, Clone, Serialize, PartialEq, TS)]
#[ts(export)]
pub struct RefinementRegion {
  pub refiner_point: Refiner,
  pub refined_ranges: Vec<Range>,
}

#[derive(Clone, PartialEq, Eq, Default, Debug)]
/// A representation of the permissions *forall* places in the body under analysis.
pub struct PermissionsDomain<'tcx>(FxHashMap<Place<'tcx>, PermissionsData>);

// ------------------------------------------------

impl Permissions {
  // No "Top" value exists for permissions as this is on a per-place basis.
  // That is, the top value depends on a places type declaration.
  pub fn bottom() -> Permissions {
    Permissions {
      read: false,
      write: false,
      drop: false,
    }
  }
}

impl std::fmt::Debug for Permissions {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if !self.read && !self.write && !self.drop {
      write!(f, "∅")
    } else {
      if self.read {
        write!(f, "R")?;
      }
      if self.write {
        write!(f, "W")?;
      }
      if self.drop {
        write!(f, "D")?;
      }
      Ok(())
    }
  }
}

// XXX: this is only valid when the Ty is an *expected* type.
// This is because expected types do not rely on the mutability of
// the binding, e.g. `let mut x = ...` and all of the expected information
// is really just in the type.
impl<'tcx> From<Ty<'tcx>> for Permissions {
  fn from(ty: Ty<'tcx>) -> Self {
    let read = true;
    let (write, drop) = match ty.ref_mutability() {
      None => (false, true),
      Some(Mutability::Not) => (false, false),
      Some(Mutability::Mut) => (true, false),
    };
    Self { read, write, drop }
  }
}

impl<'tcx> From<FxHashMap<Place<'tcx>, PermissionsData>>
  for PermissionsDomain<'tcx>
{
  fn from(m: FxHashMap<Place<'tcx>, PermissionsData>) -> Self {
    PermissionsDomain(m)
  }
}

impl<'tcx> Deref for PermissionsDomain<'tcx> {
  type Target = FxHashMap<Place<'tcx>, PermissionsData>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for PermissionsDomain<'_> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl KeyShifter for PermissionsData {
  fn shift_keys(self, loan_shift: LoanKey) -> Self {
    PermissionsData {
      loan_read_refined: self.loan_read_refined.map(|l| l + loan_shift),
      loan_write_refined: self.loan_write_refined.map(|l| l + loan_shift),
      loan_drop_refined: self.loan_drop_refined.map(|l| l + loan_shift),
      ..self
    }
  }
}
