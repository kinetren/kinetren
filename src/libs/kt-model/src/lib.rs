//! The timeline model: clips, tracks, editing commands, undo, the
//! project format `.ktproj` (see docs/file-format.md).
//!
//! This crate is the single description of a clip in the system: the engine (kt-engine)
//! consumes these types directly, without intermediate retellings
//! (in the old C++ a clip was re-described by 4 structures on the way into the engine).
//!
//! The implementation — stage M1.

pub mod autoplan;
pub mod formula;
pub mod genmedia;
pub mod links;
pub mod model;
pub mod ops;
pub mod project;
pub mod project_file;
pub mod published;
pub mod warp;

pub use autoplan::*;
pub use formula::{Formula, FormulaError, FormulaErrorKind};
pub use genmedia::*;
pub use links::{LinkCtx, LinkPath, formula_preview, read_link, resolve_derived, stop_value};
pub use model::*;
pub use project::*;
pub use published::*;
pub use warp::*;
