//! FILE OF THE PROJECT — its own format (R.70).
//!
//! Decision of the owner (02.10.2026): “we are moving away from the otio format, keeping
//! otio only for export, and making breaking changes so that our format becomes a
//! hundred times better” and “I am against binary blobs — our file must be
//! indexable in git”.
//!
//! Reason. JSON of OpenTimelineIO has no OBJECT IDENTITY: there is no way to say “this is the
//! same reel as above”; only nested content is expressed, so the body of a reel was written
//! again into each of its instances. The owner's three reels gave 15 220 clips and 115 MB, which
//! autosave wrote regularly.
//! Measured against the serialization of the model: 17× on a flat project,
//! 501× and 929× on nested scenes.
//!
//! What is here:
//!
//! * the file is a serialization of the MODEL. “Each entity once” is a property of the model
//!   itself (`Project.sequences` holds the body of a reel, an instance refers by address),
//!   and the format simply does not spoil it;
//! * text, LINE-BASED and deterministic: editing one event changes a few lines,
//!   and one model gives the same bytes. Compact JSON was rejected —
//!   a single line for the whole file has no useful diff; binary and compressed formats are forbidden
//!   by the owner;
//! * the root carries its own marker (`kinetren`; before 2026-10-21 — `kinotrain`, which the import
//!   repairs) and a VERSION; the kind of file is determined by its content, not by its extension.
//!   A version newer than the current one is an honest refusal with its number; a partial read is
//!   forbidden (“it opened, and part of the edit is missing” is the worst of the three outcomes,
//!   F-RES-003). Breaking changes are allowed; there is no compatibility cascade with a foreign schema here;
//! * the foreign passthrough of an imported exchange file travels as a separate field
//!   (`otio`) as an opaque JSON. Its ORDER IS NOT TOUCHED: the order of foreign
//!   keys is part of the promise to the exchange file (export returns them byte for byte,
//!   t154), and determinism is provided by the insertion-ordered `Map` itself
//!   (`kt-otio` enables `serde_json/preserve_order`; cargo features are unified across the
//!   workspace). The only source of non-determinism is the `HashMap` fields of
//!   `Retained`, and they are sorted by key by whoever builds this JSON
//!   (`Retained::to_json`), not by this module.
//!
//! What is NOT here: writing to disk. The only home of atomic writing is `kt-otio`
//! (`write_atomic`, R.49: “there is no second writer of the project file in the tree”),
//! and the revision guard of the application (`write_project_guarded`) calls it with the text
//! from here. This module is responsible for the TEXT.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::project::{Project, ProjectError};

/// Marker key at the root of the file. By it the file is recognised as ours;
/// its value is the format version.
pub const MARKER: &str = "kinetren";

/// The marker of files written before the program was renamed to Kinetren
/// (2026-10-21). The reader does NOT accept it (owner's rule, R.67: old forms
/// are repaired in the import, never taught to the reader); the import door
/// renames the key — `from_former_text`.
pub const FORMER_MARKER: &str = "kinotrain";

/// Version of ITS OWN format. Not to be confused with `project::FORMAT_VERSION` — that is the
/// version of the legacy C++ format (`.ktproj`, key `format_version`), which is read
/// only by the import.
pub const PROJECT_FILE_VERSION: u32 = 1;

/// Extension of the project file. `.ktproj` is NOT reused: two different
/// formats under one suffix is a trap for people and scripts, and the
/// legacy `.ktproj` lives on in the import.
pub const EXTENSION: &str = "kt";

/// Contents of the project file: the model and the opaque remnants of a foreign
/// exchange file, if the project was imported from one.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectFile {
    pub project: Project,
    /// Passthrough of an imported `.otio` (foreign namespaces, markers,
    /// effects) — so that export puts them back in place. `None` for a project that
    /// has never been an exchange file. The contents are opaque to the model.
    pub otio: Option<Value>,
}

/// Why the file did not read. Every outcome is named: the reader must tell
/// the person what this file is and which door to open it through (rule 19).
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ProjectFileError {
    /// This is not a project file. `looks_like` — what it resembles by its contents
    /// (an exchange file, legacy), so that the error names the import door.
    #[error("not a Kinetren project file{}", looks_like_suffix(.looks_like))]
    NotOurs { looks_like: Option<FileKind> },
    /// The file was written by a newer program. A partial read is forbidden.
    #[error(
        "project written by a newer Kinetren (format {version}, this build reads up to {PROJECT_FILE_VERSION})"
    )]
    TooNew { version: u32 },
    #[error("invalid json: {0}")]
    Json(String),
    #[error("project invalid: {0}")]
    Model(#[from] ProjectError),
}

/// What the file resembles if it is not ours. It is distinguished by CONTENTS: the extension
/// cannot be trusted, recovery and tmp files carry foreign suffixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// OpenTimelineIO exchange file (`OTIO_SCHEMA` at the root).
    Otio,
    /// Legacy C++ project (`format_version` at the root).
    LegacyKtproj,
    /// A project of the former name (`kinotrain` at the root): the same
    /// format, refused by the reader, repaired by the import door.
    FormerName,
}

fn looks_like_suffix(kind: &Option<FileKind>) -> String {
    match kind {
        Some(FileKind::Otio) => {
            " — it is an OpenTimelineIO file; use File → Import older Kinetren project…".into()
        }
        Some(FileKind::LegacyKtproj) => {
            " — it is a legacy .ktproj; use File → Import older Kinetren project…".into()
        }
        Some(FileKind::FormerName) => {
            " — it is a project of Kinotrain (the former name of Kinetren); use File → Import older Kinetren project…".into()
        }
        None => String::new(),
    }
}

/// Root envelope of the file. The field order is the line order in the file: the version
/// first, so that an unknown one is rejected without reading the rest.
#[derive(Serialize, Deserialize)]
struct Envelope {
    #[serde(rename = "kinetren")]
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    otio: Option<Value>,
    project: Project,
}

/// Text of the project file: line-based, deterministic, with a trailing
/// newline (so the file plays well with `git diff` and `cat`).
pub fn to_text(project: &Project, otio: Option<&Value>) -> String {
    let env = Envelope {
        version: PROJECT_FILE_VERSION,
        // as is: the order of foreign keys is a promise to the exchange file (t154)
        otio: otio.cloned(),
        project: project.clone(),
    };
    let mut s = serde_json::to_string_pretty(&env)
        .expect("the model always serializes: no non-string map keys, no NaN");
    s.push('\n');
    s
}

/// Read the project file. A foreign file and a file of a future version — a named
/// refusal; the model read is normalized and checked, as with any
/// read from disk.
pub fn from_text(text: &str) -> Result<ProjectFile, ProjectFileError> {
    let peek: Value = serde_json::from_str(text).map_err(|e| ProjectFileError::Json(e.to_string()))?;
    let Some(root) = peek.as_object() else {
        return Err(ProjectFileError::Json("root is not an object".into()));
    };
    let Some(version) = root.get(MARKER) else {
        return Err(ProjectFileError::NotOurs { looks_like: looks_like(root) });
    };
    let version = version
        .as_u64()
        .ok_or_else(|| ProjectFileError::Json(format!("`{MARKER}` is not a version number")))?;
    if version > u64::from(PROJECT_FILE_VERSION) {
        return Err(ProjectFileError::TooNew { version: version as u32 });
    }
    // versions BELOW the current one are read by migration on read, one-way; while
    // there is one version, there is nothing to migrate — the place for future repairs is here
    let env: Envelope =
        serde_json::from_value(peek).map_err(|e| ProjectFileError::Json(e.to_string()))?;
    let mut project = env.project;
    project.normalize();
    project.validate()?;
    Ok(ProjectFile { project, otio: env.otio })
}

/// What a JSON object without our marker resembles.
pub fn looks_like(root: &serde_json::Map<String, Value>) -> Option<FileKind> {
    if root.contains_key(FORMER_MARKER) {
        Some(FileKind::FormerName)
    } else if root.contains_key("OTIO_SCHEMA") {
        Some(FileKind::Otio)
    } else if root.contains_key("format_version") {
        Some(FileKind::LegacyKtproj)
    } else {
        None
    }
}

/// The import door for a project of the former name: the root key
/// the former `kinotrain` is renamed to `kinetren` and the text is read by the normal
/// reader — so the result equals reading the same project written today. A
/// file without the former key is read as is (the door does not refuse a
/// person who picked the wrong menu item).
pub fn from_former_text(text: &str) -> Result<ProjectFile, ProjectFileError> {
    let mut v: Value = serde_json::from_str(text).map_err(|e| ProjectFileError::Json(e.to_string()))?;
    let Some(root) = v.as_object_mut() else {
        return Err(ProjectFileError::Json("root is not an object".into()));
    };
    if !root.contains_key(MARKER)
        && let Some(version) = root.remove(FORMER_MARKER)
    {
        root.insert(MARKER.to_string(), version);
    }
    from_text(&v.to_string())
}

/// Is this our file? Only by the marker at the root — for detecting the file kind
/// before a full read.
pub fn is_project_file(text: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| v.as_object().map(|o| o.contains_key(MARKER)))
        .unwrap_or(false)
}
