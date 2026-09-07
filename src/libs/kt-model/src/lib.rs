//! Модель таймлайна: клипы, дорожки, команды редактирования, undo,
//! формат проекта `.ktproj` (см. docs/file-format.md).
//!
//! Этот крейт — единственное описание клипа в системе: движок (kt-engine)
//! потребляет эти типы напрямую, без промежуточных пересказов
//! (в старом C++ клип переописывался 4 структурами по пути в движок).
//!
//! Реализация — этап M1 (см. docs/porting-notes.md).

pub mod model;
pub mod ops;
pub mod project;
pub mod warp;

pub use model::*;
pub use project::*;
pub use warp::*;
