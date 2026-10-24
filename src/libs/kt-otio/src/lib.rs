//! The native project format of Kinetren — OpenTimelineIO (.otio, JSON).
//!
//! A pure Rust implementation of a subset of OTIO schemas (the reference — the C++/Python
//! library 0.18.x) with full passthrough of the not-understood: unknown FIELDS
//! of known schemas are kept in their places (`extra`), objects of unknown
//! schemas/versions — as raw JSON (`RawObject`), foreign `metadata` namespaces —
//! verbatim. Our data lives in `metadata["kt"]` (decision R.24).
//!
//! The byte-stability invariant — the IDEMPOTENCE of writing:
//! `write(parse(write(x))) == write(x)`. The first save of a foreign file
//! normalizes the formatting (as the reference library does too).

mod convert;
mod io;
mod legacy;
mod legacy_import;
mod meta;
mod schema;
mod url;

pub use convert::{
    ClipExtra, MediaRefs, Notice, ProjectBundle, Retained, grid_fps, project_to_timeline,
    timeline_to_project,
};
pub use io::{
    AtomicFs, REPLACE_PAUSES_MS, RealFs, Written, atomic_tmp_path, parse_document, replace_file,
    write_atomic, write_atomic_with,
    write_document,
};
pub use legacy::import_legacy;
pub use meta::{
    KT_META_V6, KT_META_V8, KT_META_V9, KT_META_V10, KT_META_V11, KT_META_V12, KT_META_VERSION,
    KT_NS,
    check_kt_version, kt_entry_mut, kt_version,
};
pub use schema::{
    Clip, Composable, Effect, EffectNode, ExternalReference, Gap, LinearTimeWarp, Marker,
    MarkerNode, MediaRefNode, Metadata, MissingReference, RationalTime, RawObject, Stack,
    TimeRange, Timeline, Track, Transition,
};
/// Foreign generator names that have our equivalent (import alias,
/// R.52 S5): they are read first-class, and on re-save they are canonicalized to
/// `kt.<tag>`. The list is short and explicit — the unfamiliar stays passthrough.
pub(crate) fn generator_alias(kind: &str) -> Option<&'static str> {
    match kind {
        "SMPTEBars" => Some("smpte"),
        _ => None,
    }
}

pub use url::{decode_file_url, encode_file_url, media_target_url, relativize, resolve_media_path};

/// The key of the single media reference of our clips (the reference convention).
pub const DEFAULT_MEDIA_KEY: &str = "DEFAULT_MEDIA";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a JSON object")]
    NotAnObject,
    #[error("missing OTIO_SCHEMA tag")]
    MissingSchemaTag,
    #[error("malformed OTIO_SCHEMA tag: {0}")]
    BadSchemaTag(String),
    /// The schema of the structural spine (Timeline/Stack/Track) is newer than the supported one:
    /// “half-understanding” the positions of children is unsafe — an honest refusal.
    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(String),
    #[error("root of an .otio file must be a Timeline, found {0}")]
    UnsupportedRoot(String),
    #[error("not an OpenTimelineIO file")]
    NotOtio,
    /// This is a PROJECT file of its own format (R.70), not an exchange file: the import
    /// door is not needed for it — it is opened by the ordinary “Open”.
    #[error("a Kinetren project file, not an interchange file — use File → Open")]
    ProjectFile,
    /// A project of the former name (`kinotrain` root key): Open refuses it,
    /// the import door repairs it (`kt_model::project_file::from_former_text`).
    #[error("a project of Kinotrain (the former name of Kinetren) — use File → Import older Kinetren project…")]
    FormerProject,
    /// The file was written by a newer Kinetren (metadata.kt.v above ours).
    #[error("project written by a newer Kinetren (kt.v = {0})")]
    KtMetaTooNew(u32),
    /// The metadata.kt section exists but cannot be read. We must not stay silent: with it
    /// the project settings, tracks, morphers and layout would be lost, and the file
    /// would open as a foreign one (R.35 — found by a live scenario).
    #[error("metadata.kt is unreadable: {0}")]
    KtMetaBroken(String),
    /// A file of a PREVIOUS envelope version that the reader does not read (R.67):
    /// it is fixed by the import (`import_older`), not by the reader. The text names the
    /// version and the door — kt-app substitutes the menu item’s name.
    #[error("project of an older Kinetren version (kt.v = {version}) — use File → Import older Kinetren project… ({cause})")]
    NeedsImport { version: u32, cause: String },
    #[error("non-finite number in {0}")]
    NonFinite(&'static str),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// File format by its CONTENT (we do not trust the extension).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// A project file of its own format (R.70): the `kinetren` marker on the root.
    /// It is read by `kt_model::project_file`; this crate only recognizes it.
    Project,
    /// A project file of the former name (`kinotrain` at the root).
    FormerProject,
    Otio,
    /// An old .ktproj: it does not open as a project (decision R.24 item 12) —
    /// only the “Import .ktproj…” item.
    LegacyKtproj,
}

/// Loading an .otio project (a .ktproj file is an honest error: R.24 item 12,
/// “how a project is not opened” — use import_legacy from the import item).
pub fn load_project(
    text: &str,
    base_dir: Option<&std::path::Path>,
) -> Result<ProjectBundle, Error> {
    match detect(text)? {
        FileFormat::Project => Err(Error::ProjectFile),
        FileFormat::FormerProject => Err(Error::FormerProject),
        FileFormat::Otio => {
            let tl = parse_document(text)?;
            match timeline_to_project(&tl, base_dir) {
                // our section of an old version is not read — this is the job of
                // import, not the reader (R.67): we name the version and the door
                Err(Error::KtMetaBroken(cause)) => match meta::kt_version(&tl) {
                    Some(v) if v < meta::KT_META_VERSION => {
                        Err(Error::NeedsImport { version: v, cause })
                    }
                    _ => Err(Error::KtMetaBroken(cause)),
                },
                other => other,
            }
        }
        FileFormat::LegacyKtproj => Err(Error::NotOtio),
    }
}

/// Import of an OLD-version project envelope (R.67): fixes for old forms, then
/// the normal read. See `legacy_import`.
pub use legacy_import::{FIXUPS, Fixup, import_older};

/// IMPORT DOOR (R.70) — one for three inputs: legacy `.ktproj`, our `.otio`
/// of any envelope version, and a foreign `.otio`. The rule for choosing the path lives HERE,
/// and the app and kt-cli call it the same way: a foreign file without our envelope is
/// read by the ordinary reader (it needs no fixes, and `import_older` would
/// reject it), ours of an old version — by the fixes, legacy — by its own reader.
/// A project file does not go through this door: `Error::ProjectFile` names “Open”.
/// Media paths in the result are ABSOLUTE (resolved from `base_dir`), as in
/// `load_project`.
pub fn import_interchange(
    text: &str,
    base_dir: Option<&std::path::Path>,
) -> Result<ProjectBundle, Error> {
    match detect(text)? {
        FileFormat::Project => Err(Error::ProjectFile),
        // a project of the former name: the key is renamed, the rest is read
        // by the normal reader (owner's rule R.67 — repairs live here)
        FileFormat::FormerProject => {
            let f = kt_model::project_file::from_former_text(text)
                .map_err(|e| Error::KtMetaBroken(e.to_string()))?;
            let retained = match f.otio {
                Some(v) => Retained::from_json(&v)?,
                None => Retained::default(),
            };
            let mut project = f.project;
            resolve_project_media(&mut project, base_dir);
            let notices = vec![format!(
                "import fixup: project marker `{}` → `{}` (renamed to Kinetren)",
                kt_model::project_file::FORMER_MARKER,
                kt_model::project_file::MARKER
            )];
            Ok(ProjectBundle { project, retained, notices })
        }
        FileFormat::LegacyKtproj => {
            let (mut project, notices) = import_legacy(text)?;
            resolve_project_media(&mut project, base_dir);
            Ok(ProjectBundle { project, retained: Retained::default(), notices })
        }
        FileFormat::Otio => match load_project(text, base_dir) {
            Err(Error::NeedsImport { .. }) => import_older(text, base_dir),
            other => other,
        },
    }
}

/// Relative media paths → absolute, from the file's directory. Pseudo-paths
/// (`synth:`, `gen:`) and absolute ones are left untouched. Called when the project
/// file is READ: the model in memory holds absolute paths, as after `load_project`.
pub fn resolve_project_media(p: &mut kt_model::Project, base_dir: Option<&std::path::Path>) {
    let Some(base) = base_dir else { return };
    for m in &mut p.media {
        if kt_model::is_pseudo_path(&m.path) || std::path::Path::new(&m.path).is_absolute() {
            continue;
        }
        m.path = resolve_media_path(base, &m.path).to_string_lossy().into_owned();
    }
}

/// Absolute media paths → relative to the directory of the file, where this is
/// expressible (otherwise they stay absolute). Called when the project file is
/// WRITTEN: a project moved together with its material must open (t797).
pub fn relativize_project_media(p: &mut kt_model::Project, base_dir: Option<&std::path::Path>) {
    let Some(base) = base_dir else { return };
    for m in &mut p.media {
        if kt_model::is_pseudo_path(&m.path) || !std::path::Path::new(&m.path).is_absolute() {
            continue;
        }
        if let Some(rel) = relativize(base, std::path::Path::new(&m.path)) {
            m.path = rel;
        }
    }
}

/// Serialization of the project to .otio (idempotent write, test 144).
pub fn save_project(
    p: &kt_model::Project,
    retained: &Retained,
    base_dir: Option<&std::path::Path>,
) -> String {
    write_document(&project_to_timeline(p, retained, base_dir))
}

pub fn detect(text: &str) -> Result<FileFormat, Error> {
    let v: serde_json::Value = serde_json::from_str(text)?;
    let serde_json::Value::Object(map) = v else {
        return Err(Error::NotAnObject);
    };
    if map.contains_key(kt_model::project_file::MARKER) {
        return Ok(FileFormat::Project);
    }
    if map.contains_key(kt_model::project_file::FORMER_MARKER) {
        return Ok(FileFormat::FormerProject);
    }
    if map.get("OTIO_SCHEMA").is_some() {
        return Ok(FileFormat::Otio);
    }
    if map.get("format_version").is_some() {
        return Ok(FileFormat::LegacyKtproj);
    }
    Err(Error::NotOtio)
}
