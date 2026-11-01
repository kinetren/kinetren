//! User store outside the project (R.48, item 343).
//!
//! Until this round there was no such place at all: eframe persistence held
//! the language and the path of the last project; all other “conveniences” lived inside
//! the project file. Four features from the reconnaissance ran into the lack of a common place
//! (a library of FX presets, title templates, export presets,
//! key layouts) — so the module is made COMMON: the catalogue, atomic
//! writing and the broken-file policy are here, while concrete libraries put
//! their own files next to it.
//!
//! Principles common to all future files:
//! - **the project is self-sufficient**: a library holds insertion templates, not
//!   links. A lost library file leaves the projects intact;
//! - **records are stored RAW** (`serde_json::Value`) next to the parsed
//!   copy: an unknown record from a future version is skipped when
//!   shown, but survives saving — an old build does not eat the work of
//!   a new one;
//! - **a file newer than the supported version is read-only**: what we do not
//!   understand must not be overwritten;
//! - **a broken file does not bring the programme down**: it is moved aside to `.broken` at
//!   the first write, and the library starts empty.

use std::path::PathBuf;

/// The version of the presets file format this build can write.
const PRESETS_VERSION: u64 = 1;

/// The user-data directory — the same one where the autosave lives: one
/// place for backup. `KT_USER_STORE_DIR` replaces it in tests and live
/// scenarios (otherwise they would write into the developer’s home directory).
pub fn store_dir() -> PathBuf {
    // the store assigned by the live stand (R.68): each scenario has its own,
    // and the environment variable of a live process cannot be changed from outside
    if let Some(dir) = crate::bench::store_dir() {
        return dir;
    }
    if let Some(dir) = std::env::var_os("KT_USER_STORE_DIR") {
        return PathBuf::from(dir);
    }
    crate::journal::data_dir()
}

/// A parsed preset: a name and a chain ready for insertion.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetRow {
    pub name: String,
    /// The curve keys are rebased to zero: the preset does not remember from which
    /// event it was taken.
    pub block: kt_model::ops::FxChainBlock,
    /// Where it is legitimate to insert it (computed from the effects registry).
    pub audio_ok: bool,
    pub video_ok: bool,
    /// The factory one — is not renamed or deleted.
    pub factory: bool,
}

/// Factory presets live in the binary and are parsed by the SAME code as the
/// user file: so the loader is checked on every launch.
const FACTORY: &str = include_str!("../assets/fx-presets-factory.json");

/// FX preset library.
pub struct FxPresetStore {
    /// Raw records of the user file (forward compatibility).
    raw: Vec<serde_json::Value>,
    /// Parsed ones: user first, then factory.
    rows: Vec<PresetRow>,
    /// The file did not parse — at the first write it will go to `.broken`.
    broken: bool,
    /// The file is newer than we can handle: shown, but not overwritten.
    readonly: bool,
    loaded: bool,
    /// How many records did not parse (an unknown effect kind from a future
    /// version): they remain in the file untouched, but are not shown.
    skipped: usize,
    /// `None` — the store in memory (tests): the home directory is not touched.
    dir: Option<PathBuf>,
}

impl Default for FxPresetStore {
    fn default() -> FxPresetStore {
        FxPresetStore {
            raw: Vec::new(),
            rows: Vec::new(),
            broken: false,
            readonly: false,
            loaded: false,
            skipped: 0,
            // under tests — memory only, unless a directory is set explicitly
            dir: (!cfg!(test) || std::env::var_os("KT_USER_STORE_DIR").is_some()).then(store_dir),
        }
    }
}

impl FxPresetStore {
    /// A store in the given directory (tests with real writing and
    /// live scenarios via `KT_USER_STORE_DIR`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn at(dir: PathBuf) -> FxPresetStore {
        FxPresetStore { dir: Some(dir), ..Default::default() }
    }

    fn path(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("fx-presets.json"))
    }

    /// Parsed presets (lazy loading on first access).
    pub fn rows(&mut self) -> &[PresetRow] {
        self.load();
        &self.rows
    }

    pub fn is_broken(&self) -> bool {
        self.broken
    }

    pub fn is_readonly(&self) -> bool {
        self.readonly
    }

    /// How many file records failed to parse (unknown effects from a
    /// future version). They remain in the file untouched.
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let text = self.path().and_then(|p| std::fs::read_to_string(p).ok());
        if let Some(text) = text {
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) => {
                    let ver = v.get("version").and_then(|x| x.as_u64()).unwrap_or(0);
                    self.readonly = ver > PRESETS_VERSION;
                    self.raw =
                        v.get("presets").and_then(|x| x.as_array()).cloned().unwrap_or_default();
                }
                Err(_) => self.broken = true,
            }
        }
        self.reload_rows();
    }

    /// Add a preset (the name is already unique — the caller picks it).
    /// The curve keys are rebased to zero: the preset does not keep its donor.
    pub fn add(&mut self, name: &str, block: &kt_model::ops::FxChainBlock) -> std::io::Result<()> {
        self.load();
        let mut b = block.clone();
        for slot in &mut b.slots {
            slot.curves = kt_model::ops::rebase_curves(&slot.curves, b.src_in, kt_time::Tick(0));
        }
        b.src_in = kt_time::Tick(0);
        let entry = serde_json::json!({ "name": name, "slots": b.slots });
        self.raw.push(entry);
        self.reload_rows();
        self.save()
    }

    /// Rename a user preset.
    pub fn rename(&mut self, old: &str, new: &str) -> std::io::Result<()> {
        self.load();
        for v in &mut self.raw {
            if v.get("name").and_then(|n| n.as_str()) == Some(old) {
                v["name"] = serde_json::Value::String(new.to_string());
            }
        }
        self.reload_rows();
        self.save()
    }

    /// Delete a user preset. This is an edit of the FILE, not of the model —
    /// the project’s undo does not roll it back (and the hint says so).
    pub fn remove(&mut self, name: &str) -> std::io::Result<()> {
        self.load();
        self.raw.retain(|v| v.get("name").and_then(|n| n.as_str()) != Some(name));
        self.reload_rows();
        self.save()
    }

    /// Taken names (user + factory) — a unique name is built from them
    /// when saving.
    pub fn taken_names(&mut self) -> Vec<String> {
        self.rows().iter().map(|r| r.name.clone()).collect()
    }

    fn reload_rows(&mut self) {
        let mut rows: Vec<PresetRow> = self.raw.iter().filter_map(|v| decode(v, false)).collect();
        self.skipped = self.raw.len() - rows.len();
        if let Ok(f) = serde_json::from_str::<serde_json::Value>(FACTORY) {
            let list = f.get("presets").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let before = rows.len();
            rows.extend(list.iter().filter_map(|v| decode(v, true)));
            debug_assert_eq!(
                rows.len() - before,
                list.len(),
                "the factory preset file must decode completely: it ships in the binary"
            );
        }
        self.rows = rows;
    }

    fn save(&mut self) -> std::io::Result<()> {
        let Some(path) = self.path() else { return Ok(()) };
        if self.readonly {
            return Err(std::io::Error::other("presets file is newer than this build"));
        }
        if self.broken && path.exists() {
            // a broken file is not thrown away silently — it is moved aside next to it
            let _ = kt_otio::replace_file(&path, &path.with_extension("json.broken"));
            self.broken = false;
        }
        let doc = serde_json::json!({ "version": PRESETS_VERSION, "presets": self.raw });
        let text = serde_json::to_string_pretty(&doc)?;
        crate::app::write_project_atomic(&path, &text)
    }
}

/// Parse one record. An unknown effect kind, an empty name or broken
/// curves — the record is skipped (it stays in the file).
fn decode(v: &serde_json::Value, factory: bool) -> Option<PresetRow> {
    // the slots are read BEFORE the picker's registry is asked: without the plugin set a moved
    // kind would read as a missing plugin (plugin-api-stage1)
    crate::plugins::install();
    let name = v.get("name")?.as_str()?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let slots: Vec<kt_model::ops::FxSlotBlock> =
        serde_json::from_value(v.get("slots")?.clone()).ok()?;
    if slots.is_empty() {
        return None;
    }
    // Compatibility is computed BY THE REGISTRY: a preset is visible where
    // ALL its slots are legitimate (Slowed+Reverb is two-sided — it goes into both lists).
    let entries = crate::fx_registry::registry();
    let kind_of = |f: &kt_model::FxInstance| -> Option<(bool, bool)> {
        let name = crate::fx_editor::fx_display_name(f);
        entries.iter().find(|e| e.name == name).map(|e| (e.audio, e.video))
    };
    let mut audio_ok = true;
    let mut video_ok = true;
    for s in &slots {
        let (a, vd) = kind_of(&s.fx)?;
        audio_ok &= a;
        video_ok &= vd;
    }
    Some(PresetRow {
        name,
        block: kt_model::ops::FxChainBlock { slots, src_in: kt_time::Tick(0), fx_off: false },
        audio_ok,
        video_ok,
        factory,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kt-presets-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn chain_with_curve() -> kt_model::ops::FxChainBlock {
        kt_model::ops::FxChainBlock {
            slots: vec![kt_model::ops::FxSlotBlock {
                fx: kt_model::FxInstance::blur(8),
                bypass: false,
                curves: vec![kt_model::ParamCurve {
                    param: "radius".into(),

                    keys: kt_model::AutoKey::linear(vec![
                        kt_model::AutoKey::at(kt_time::Tick(45_000), 1.0),
                        kt_model::AutoKey::at(kt_time::Tick(135_000), 48.0),
                    ]),
                    source: Default::default(),
                }],
                masks: Vec::new(),
            }],
            src_in: kt_time::Tick(45_000),
            fx_off: false,
        }
    }

    /// Test 344 (R.48, item 343): a preset survives a round trip through the file, and its
    /// keys are rebased to zero — the preset does not remember its donor.
    #[test]
    fn t344_preset_survives_the_file_roundtrip() {
        let dir = tmp_dir("roundtrip");
        let mut store = FxPresetStore::at(dir.clone());
        let factory = store.rows().iter().filter(|r| r.factory).count();
        assert!(factory > 0, "factory presets must parse");
        store.add("My chain", &chain_with_curve()).expect("save");

        // a new instance reads the file from disk
        let mut back = FxPresetStore::at(dir.clone());
        let mine: Vec<_> = back.rows().iter().filter(|r| !r.factory).cloned().collect();
        assert_eq!(mine.len(), 1, "a preset must survive writing and reading");
        assert_eq!(mine[0].name, "My chain");
        assert_eq!(
            mine[0].block.slots[0].curves[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
            vec![0, 90_000],
            "keys must be stored from zero (the preset does not remember the donor)"
        );
        assert!(mine[0].video_ok && !mine[0].audio_ok, "Blur is a video preset");

        // rename and delete reach the file
        back.rename("My chain", "Renamed").expect("rename");
        let mut third = FxPresetStore::at(dir.clone());
        assert!(third.rows().iter().any(|r| r.name == "Renamed"));
        third.remove("Renamed").expect("remove");
        let mut fourth = FxPresetStore::at(dir.clone());
        assert!(fourth.rows().iter().all(|r| r.factory), "there must be no deleted preset");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Test 344b: foreign and broken files do not bring the programme down and do not eat
    /// someone else’s work.
    #[test]
    fn t344b_broken_and_future_files_are_survivable() {
        // (1) broken JSON: an empty library, a flag, and on the first write the
        //     file is moved aside to .broken, rather than disappearing
        let dir = tmp_dir("broken");
        std::fs::write(dir.join("fx-presets.json"), "{ not json").unwrap();
        let mut store = FxPresetStore::at(dir.clone());
        assert!(store.rows().iter().all(|r| r.factory), "nothing is read from a broken file");
        assert!(store.is_broken());
        store.add("After crash", &chain_with_curve()).expect("save over broken");
        assert!(dir.join("fx-presets.json.broken").exists(), "the broken file must be kept alongside");
        let mut back = FxPresetStore::at(dir.clone());
        assert!(back.rows().iter().any(|r| r.name == "After crash"));

        // (2) an unknown effect kind: the record is skipped when shown, but it
        //     SURVIVES saving — an old build does not eat the work of a new one
        let dir = tmp_dir("future-kind");
        std::fs::write(
            dir.join("fx-presets.json"),
            r#"{"version":1,"presets":[
                {"name":"From the future","slots":[{"fx":{"kind":"time_machine","x":1}}]}
            ]}"#,
        )
        .unwrap();
        let mut store = FxPresetStore::at(dir.clone());
        assert!(store.rows().iter().all(|r| r.factory), "an unknown kind is not shown");
        assert_eq!(store.skipped(), 1, "but it is computed");
        store.add("Mine", &chain_with_curve()).expect("save");
        let text = std::fs::read_to_string(dir.join("fx-presets.json")).unwrap();
        assert!(text.contains("time_machine"), "a foreign record must survive our write");

        // (3) a file newer than the supported version — read-only
        let dir = tmp_dir("future-version");
        std::fs::write(dir.join("fx-presets.json"), r#"{"version":99,"presets":[]}"#).unwrap();
        let mut store = FxPresetStore::at(dir.clone());
        let _ = store.rows();
        assert!(store.is_readonly(), "the file is newer than us — we read it but do not write");
        assert!(store.add("Mine", &chain_with_curve()).is_err(), "the write must refuse");
    }

    /// Test 347 (R.48, item 346): where a preset may legitimately be inserted is decided by
    /// the REGISTRY, not by guesswork. A mixed chain is not visible in either the audio or
    /// the video window — but it is legitimate for the morpher, and it shows the whole
    /// library (both sides live in the patch).
    #[test]
    fn t347_preset_compatibility_comes_from_the_registry() {
        crate::plugins::install();
        let mut store = FxPresetStore::default();
        let block = |fx: Vec<kt_model::FxInstance>| kt_model::ops::FxChainBlock {
            slots: fx
                .into_iter()
                .map(|f| kt_model::ops::FxSlotBlock { fx: f, bypass: false, curves: vec![], masks: vec![] })
                .collect(),
            src_in: kt_time::Tick(0),
            fx_off: false,
        };
        store.add("audio only", &block(vec![kt_model::FxInstance::gain(-3.0)])).unwrap();
        store.add("video only", &block(vec![kt_model::FxInstance::blur(8)])).unwrap();
        store
            .add("both sides", &block(vec![kt_model::FxInstance::slowed_reverb(0.8, 0.3, 80.0)]))
            .unwrap();
        store
            .add(
                "mixed",
                &block(vec![kt_model::FxInstance::gain(-3.0), kt_model::FxInstance::blur(8)]),
            )
            .unwrap();
        let mut by = |name: &str| store.rows().iter().find(|r| r.name == name).cloned().unwrap();
        let g = by("audio only");
        assert!(g.audio_ok && !g.video_ok, "loudness — audio only");
        let b = by("video only");
        assert!(!b.audio_ok && b.video_ok, "Blur — video only");
        let sr = by("both sides");
        assert!(sr.audio_ok && sr.video_ok, "Slowed+Reverb is legal on both sides");
        let m = by("mixed");
        assert!(
            !m.audio_ok && !m.video_ok,
            "a mixed chain is visible in neither the audio nor the video window (the morpher shows it to everyone)"
        );
    }

    /// Test 344c: the factory file is valid, complete and varied.
    /// It is in the binary, so it is parsed by the same code on every launch.
    #[test]
    fn t344c_factory_presets_are_valid() {
        let mut store = FxPresetStore::default();
        let rows: Vec<_> = store.rows().iter().filter(|r| r.factory).cloned().collect();
        assert!(rows.len() >= 4, "there must be several factory presets");
        let mut names = std::collections::BTreeSet::new();
        let mut any_audio = false;
        let mut any_video = false;
        for r in &rows {
            assert!(names.insert(r.name.clone()), "factory names are unique: {}", r.name);
            assert!(
                r.block.slots.iter().any(|s| !s.curves.is_empty()),
                "{}: the preset must carry an animation — the library exists for its sake",
                r.name
            );
            assert!(r.audio_ok || r.video_ok, "{}: the preset cannot be inserted anywhere", r.name);
            any_audio |= r.audio_ok;
            any_video |= r.video_ok;
        }
        assert!(any_audio && any_video, "factory presets must cover both kinds of events");
    }

    /// Test 756b (R.63 item 8.3 → R.65): the WORKSPACE survives a round trip through the
    /// file, and a file without a section is still read as before.
    ///
    /// The owner: “why the hell must I keep remembering which parameters I entered
    /// last time?”. The settings of the INSTRUMENT live in the common store; sidechain settings
    /// no longer go here — they are the content of the project (the track’s chain)
    /// and go through undo (R.65).
    #[test]
    fn t756b_workspace_settings_survive_the_roundtrip() {
        let d = tmp_dir("ws");
        let path = d.join("workspace.json");
        let mut w = Workspace::default();
        w.meters_w = 123.0;
        w.set_view(
            Some(std::path::Path::new("/tmp/p.kt")),
            ProjectView { view_start: 5.0, ticks_per_px: 700.0, cursor: 4242 },
        );
        save_workspace_at(&path, &w);
        let back = load_workspace_at(&path);
        assert_eq!(back.meters_w, 123.0, "meter width did not survive the round trip through the file");
        assert_eq!(
            back.view_of(Some(std::path::Path::new("/tmp/p.kt")))
                .map(|v| (v.ticks_per_px, v.cursor)),
            Some((700.0, 4242)),
            "the project kind did not survive the round trip"
        );

        // a file WITHOUT sections is still read as before: the old workspace is intact
        let old = d.join("old.json");
        std::fs::write(&old, "{\"version\":1,\"meters_w\":77.0,\"views\":[]}\n").unwrap();
        let back = load_workspace_at(&old);
        assert_eq!(back.meters_w, 77.0);
        assert!(back.view_of(Some(std::path::Path::new("/tmp/p.kt"))).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}

// ---- workspace (R.49, item 360) ------------------------------------------
//
// The meter panel width, zoom, scroll and cursor position rode IN THE PROJECT FILE.
// A consequence, visible on any shared work: you open a colleague’s project —
// you lose your metres width; you save — you impose yours on them. This is the WORKSPACE,
// not the editing.
//
// Track names and heights, bin groups stay in the project: they are about THIS
// edit, and the colleague must receive them together with it.
//
// In the project file itself the former values remain passthrough — what was read is
// written back. Byte stability holds, the GOLDEN files do not move, and an old build,
// opening the file, finds in it exactly what was there.

/// The version of the workspace file.
const WORKSPACE_VERSION: u64 = 1;

/// How many projects we remember. The list is not history, but a convenience; after a hundred
/// edits the oldest records are of use to nobody.
const WORKSPACE_KEEP: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectView {
    pub view_start: f64,
    pub ticks_per_px: f64,
    pub cursor: i64,
}

#[derive(Debug, Clone, Default)]
pub struct Workspace {
    /// 0 — not set: take the value from the project file (the first launch after
    /// the move) or the default.
    pub meters_w: f32,
    /// The view per project, the freshest at the front.
    views: Vec<(String, ProjectView)>,
    /// Display of the geometry frame over the preview frame (R.67). The default is
    /// ON: a useful control is on by default (checklist rule 4), and turning it
    /// off is the person’s decision, and it is remembered.
    /// `None` — the key is not in the file (first launch): the default is taken.
    pub geom_controls: Option<bool>,
    /// The file was read from a future version — read-only.
    readonly: bool,
}

impl Workspace {
    pub fn view_of(&self, project: Option<&std::path::Path>) -> Option<ProjectView> {
        let key = project?.to_string_lossy().into_owned();
        self.views.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    pub fn set_view(&mut self, project: Option<&std::path::Path>, v: ProjectView) {
        let Some(key) = project.map(|p| p.to_string_lossy().into_owned()) else { return };
        self.views.retain(|(k, _)| *k != key);
        self.views.insert(0, (key, v));
        self.views.truncate(WORKSPACE_KEEP);
    }

    pub fn is_readonly(&self) -> bool {
        self.readonly
    }
}

fn workspace_path() -> PathBuf {
    store_dir().join("workspace.json")
}

pub fn load_workspace() -> Workspace {
    load_workspace_at(&workspace_path())
}

pub fn load_workspace_at(path: &std::path::Path) -> Workspace {
    let Ok(text) = std::fs::read_to_string(path) else { return Workspace::default() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        // a broken file does not bring the programme down and does not eat the work: it will be
        // moved aside to `.broken` on the first write
        return Workspace::default();
    };
    let ver = v.get("version").and_then(|x| x.as_u64()).unwrap_or(0);
    let mut w = Workspace { readonly: ver > WORKSPACE_VERSION, ..Default::default() };
    w.meters_w = v.get("meters_w").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    w.geom_controls = v.get("geom_controls").and_then(|x| x.as_bool());
    if let Some(arr) = v.get("views").and_then(|x| x.as_array()) {
        for e in arr.iter().take(WORKSPACE_KEEP) {
            let (Some(path), Some(vs), Some(tp)) = (
                e.get("project").and_then(|x| x.as_str()),
                e.get("view_start").and_then(|x| x.as_f64()),
                e.get("ticks_per_px").and_then(|x| x.as_f64()),
            ) else {
                continue;
            };
            w.views.push((
                path.to_string(),
                ProjectView {
                    view_start: vs,
                    ticks_per_px: tp,
                    cursor: e.get("cursor").and_then(|x| x.as_i64()).unwrap_or(0),
                },
            ));
        }
    }
    w
}

pub fn save_workspace(w: &Workspace) {
    save_workspace_at(&workspace_path(), w);
}

pub fn save_workspace_at(path: &std::path::Path, w: &Workspace) {
    if w.readonly {
        return; // file newer than ours — we do not rewrite what we do not understand
    }
    let views: Vec<serde_json::Value> = w
        .views
        .iter()
        .map(|(k, v)| {
            serde_json::json!({
                "project": k,
                "view_start": v.view_start,
                "ticks_per_px": v.ticks_per_px,
                "cursor": v.cursor,
            })
        })
        .collect();
    let mut doc = serde_json::json!({
        "version": WORKSPACE_VERSION,
        "meters_w": w.meters_w,
        "views": views,
    });
    // the key is written only if the person touched the display: a file without it means
    // “default”, and it stays on
    if let Some(on) = w.geom_controls {
        doc["geom_controls"] = serde_json::json!(on);
    }
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    // by the same code as the project: a break midway leaves no stump
    let _ = kt_otio::write_atomic(path, &format!("{doc:#}\n"));
}

// ---- between-projects records (welcome-window-and-tour) -------------------
//
// The welcome window needs what lives BETWEEN projects: the recent list with its posters,
// the person's project templates, the tour progress and the start choice. Each is its own
// small file next to the workspace, with the store's common rules: a broken file is moved
// aside to `.broken` on the first write and the record starts empty; a file of a future
// version is read but never overwritten. A poster lives under `<store>/posters/`, keyed by
// the project's path — never next to the project or in its sidecar: the sidecar is the
// project's content cache (R.58), a poster is the program's list decoration.

/// The version of the four record files this build writes.
const RECORDS_VERSION: u64 = 1;

/// How many recent projects the list keeps (UI rule 20: nothing grows by itself).
pub const RECENT_KEEP: usize = 12;

/// One row of the recent list.
#[derive(Debug, Clone, PartialEq)]
pub struct RecentEntry {
    pub path: PathBuf,
    pub name: String,
    /// Seconds since the Unix epoch of the last open or save.
    pub opened: u64,
    /// The template the project was made from (a built-in id or `user:<name>`).
    pub template: Option<String>,
}

/// A template the person saved from Project settings.
#[derive(Debug, Clone, PartialEq)]
pub struct UserTemplate {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// Finished steps per lesson, and when the tour was last touched.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TourProgress {
    pub done: std::collections::BTreeMap<u32, std::collections::BTreeSet<u32>>,
    /// Seconds since the epoch of the last step finished or the last tour start; compared with
    /// the newest recent entry: "a project was opened since the tour stopped".
    pub touched: u64,
}

impl TourProgress {
    pub fn started(&self) -> bool {
        self.touched > 0 || self.done.values().any(|s| !s.is_empty())
    }

    pub fn is_done(&self, lesson: u32, step: u32) -> bool {
        self.done.get(&lesson).is_some_and(|s| s.contains(&step))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn finished_steps(&self) -> usize {
        self.done.values().map(|s| s.len()).sum()
    }
}

/// The start record: the switch and the facts the primary tile needs.
#[derive(Debug, Clone, PartialEq)]
pub struct StartChoice {
    /// "Show the welcome window at start" — on by default (D11).
    pub show_at_start: bool,
    /// The template of the last "New from template" click.
    pub last_template: Option<String>,
    /// The former `last_project` key already seeded the recent list.
    pub seeded: bool,
    /// The program has started at least once with this store (the first start is the tour's).
    pub started_before: bool,
}

impl Default for StartChoice {
    fn default() -> StartChoice {
        StartChoice { show_at_start: true, last_template: None, seeded: false, started_before: false }
    }
}

/// Read-state of one record file.
#[derive(Debug, Clone, Copy, Default)]
struct FileState {
    broken: bool,
    readonly: bool,
}

/// The four between-projects records. `dir == None` — memory only (tests).
pub struct StartStore {
    dir: Option<PathBuf>,
    pub recent: Vec<RecentEntry>,
    pub templates: Vec<UserTemplate>,
    pub tour: TourProgress,
    pub start: StartChoice,
    states: [FileState; 4],
}

const F_RECENT: usize = 0;
const F_TEMPLATES: usize = 1;
const F_TOUR: usize = 2;
const F_START: usize = 3;
const FILES: [&str; 4] = ["recent.json", "templates.json", "tour.json", "start.json"];

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The project's path as the key of its list row and poster: canonical when the file exists,
/// absolute otherwise (a deleted project keeps its row and its key). Canonical WITHOUT the
/// Windows verbatim prefix (`kt_dirs::canonical`): the key is also what a welcome card shows,
/// and `\\?\C:\…` is neither what a person typed nor equal to the `absolute` spelling (t986).
pub fn project_key(p: &std::path::Path) -> PathBuf {
    kt_dirs::canonical(p)
        .or_else(|_| std::path::absolute(p))
        .unwrap_or_else(|_| p.to_path_buf())
}

fn read_record(path: Option<&std::path::Path>) -> (Option<serde_json::Value>, FileState) {
    let Some(text) = path.and_then(|p| std::fs::read_to_string(p).ok()) else {
        return (None, FileState::default());
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => {
            let ver = v.get("version").and_then(|x| x.as_u64()).unwrap_or(0);
            (Some(v), FileState { broken: false, readonly: ver > RECORDS_VERSION })
        }
        Err(_) => (None, FileState { broken: true, readonly: false }),
    }
}

impl Default for StartStore {
    fn default() -> StartStore {
        // under tests — memory only, unless a directory is set explicitly (as FxPresetStore)
        let dir = (!cfg!(test) || std::env::var_os("KT_USER_STORE_DIR").is_some()).then(store_dir);
        StartStore::at(dir)
    }
}

impl StartStore {
    /// The store in `dir` (`None` — memory only). Reads all four files.
    pub fn at(dir: Option<PathBuf>) -> StartStore {
        let mut s = StartStore {
            dir,
            recent: Vec::new(),
            templates: Vec::new(),
            tour: TourProgress::default(),
            start: StartChoice::default(),
            states: [FileState::default(); 4],
        };
        let (v, st) = read_record(s.file(F_RECENT).as_deref());
        s.states[F_RECENT] = st;
        if let Some(arr) = v.as_ref().and_then(|v| v.get("recent")).and_then(|x| x.as_array()) {
            for e in arr.iter().take(RECENT_KEEP) {
                let Some(path) = e.get("path").and_then(|x| x.as_str()) else { continue };
                let path = PathBuf::from(path);
                s.recent.push(RecentEntry {
                    name: e
                        .get("name")
                        .and_then(|x| x.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| display_name(&path)),
                    path,
                    opened: e.get("opened").and_then(|x| x.as_u64()).unwrap_or(0),
                    template: e.get("template").and_then(|x| x.as_str()).map(str::to_string),
                });
            }
        }
        let (v, st) = read_record(s.file(F_TEMPLATES).as_deref());
        s.states[F_TEMPLATES] = st;
        if let Some(arr) = v.as_ref().and_then(|v| v.get("templates")).and_then(|x| x.as_array()) {
            for e in arr {
                let (Some(name), Some(w), Some(h), Some(fps)) = (
                    e.get("name").and_then(|x| x.as_str()),
                    e.get("width").and_then(|x| x.as_u64()),
                    e.get("height").and_then(|x| x.as_u64()),
                    e.get("fps").and_then(|x| x.as_u64()),
                ) else {
                    continue;
                };
                if name.trim().is_empty() || w == 0 || h == 0 || fps == 0 {
                    continue;
                }
                s.templates.push(UserTemplate {
                    name: name.to_string(),
                    width: w as u32,
                    height: h as u32,
                    fps: fps as u32,
                });
            }
        }
        let (v, st) = read_record(s.file(F_TOUR).as_deref());
        s.states[F_TOUR] = st;
        if let Some(v) = &v {
            s.tour.touched = v.get("touched").and_then(|x| x.as_u64()).unwrap_or(0);
            if let Some(obj) = v.get("done").and_then(|x| x.as_object()) {
                for (lesson, steps) in obj {
                    let Ok(l) = lesson.parse::<u32>() else { continue };
                    let set: std::collections::BTreeSet<u32> = steps
                        .as_array()
                        .map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect())
                        .unwrap_or_default();
                    s.tour.done.insert(l, set);
                }
            }
        }
        let (v, st) = read_record(s.file(F_START).as_deref());
        s.states[F_START] = st;
        if let Some(v) = &v {
            s.start = StartChoice {
                show_at_start: v.get("welcome_at_start").and_then(|x| x.as_bool()).unwrap_or(true),
                last_template: v.get("last_template").and_then(|x| x.as_str()).map(str::to_string),
                seeded: v.get("seeded").and_then(|x| x.as_bool()).unwrap_or(false),
                started_before: v.get("started_before").and_then(|x| x.as_bool()).unwrap_or(true),
            };
        }
        s
    }

    fn file(&self, f: usize) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(FILES[f]))
    }

    /// The store's directory; `None` — memory only.
    pub fn dir(&self) -> Option<&std::path::Path> {
        self.dir.as_deref()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_broken(&self, file: &str) -> bool {
        FILES.iter().position(|f| *f == file).is_some_and(|i| self.states[i].broken)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_readonly(&self, file: &str) -> bool {
        FILES.iter().position(|f| *f == file).is_some_and(|i| self.states[i].readonly)
    }

    /// Where the poster of a project lives: `<store>/posters/<xxh3 of the key>.png`.
    /// `None` for a memory store.
    pub fn poster_path(&self, project: &std::path::Path) -> Option<PathBuf> {
        let key = project_key(project);
        let h = xxhash_rust::xxh3::xxh3_64(key.to_string_lossy().as_bytes());
        self.dir.as_ref().map(|d| d.join("posters").join(format!("{h:016x}.png")))
    }

    fn write(&mut self, f: usize, body: serde_json::Value) {
        let Some(path) = self.file(f) else { return };
        if self.states[f].readonly {
            return; // newer than us — we read it, we do not rewrite what we do not understand
        }
        if self.states[f].broken && path.exists() {
            let _ = kt_otio::replace_file(&path, &path.with_extension("json.broken"));
            self.states[f].broken = false;
        }
        let mut doc = body;
        doc["version"] = serde_json::json!(RECORDS_VERSION);
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        // tmp + rename, WITHOUT fsync: a break midway leaves no stump, and these records are
        // conveniences — a lost recent list costs nothing, while two fsyncs per open stall the
        // UI thread for seconds on a busy disk (t985 took 90 s with `write_atomic`)
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, format!("{doc:#}\n")).is_ok() {
            let _ = kt_otio::replace_file(&tmp, &path);
        }
    }

    fn save_recent(&mut self) {
        let arr: Vec<serde_json::Value> = self
            .recent
            .iter()
            .map(|e| {
                let mut v = serde_json::json!({
                    "path": e.path.to_string_lossy(),
                    "name": e.name,
                    "opened": e.opened,
                });
                if let Some(t) = &e.template {
                    v["template"] = serde_json::json!(t);
                }
                v
            })
            .collect();
        self.write(F_RECENT, serde_json::json!({ "recent": arr }));
    }

    fn save_templates(&mut self) {
        let arr: Vec<serde_json::Value> = self
            .templates
            .iter()
            .map(|t| serde_json::json!({ "name": t.name, "width": t.width, "height": t.height, "fps": t.fps }))
            .collect();
        self.write(F_TEMPLATES, serde_json::json!({ "templates": arr }));
    }

    fn save_tour(&mut self) {
        let done: serde_json::Map<String, serde_json::Value> = self
            .tour
            .done
            .iter()
            .map(|(l, s)| (l.to_string(), serde_json::json!(s.iter().collect::<Vec<_>>())))
            .collect();
        self.write(F_TOUR, serde_json::json!({ "done": done, "touched": self.tour.touched }));
    }

    fn save_start(&mut self) {
        let mut v = serde_json::json!({
            "welcome_at_start": self.start.show_at_start,
            "seeded": self.start.seeded,
            "started_before": self.start.started_before,
        });
        if let Some(t) = &self.start.last_template {
            v["last_template"] = serde_json::json!(t);
        }
        self.write(F_START, v);
    }

    /// The first start of a build with these records: the former single `last_project`
    /// seeds the list ONCE (design, Migration Plan). Returns whether this is the first start
    /// of the program with this store at all (an empty store and no former key).
    pub fn begin_session(&mut self, last_project: Option<&str>) -> bool {
        let first = !self.start.started_before && self.recent.is_empty() && !self.tour.started();
        let mut changed = !self.start.started_before;
        self.start.started_before = true;
        if !self.start.seeded {
            self.start.seeded = true;
            changed = true;
            if let Some(p) = last_project.filter(|p| !p.is_empty())
                && self.recent.is_empty()
            {
                let path = PathBuf::from(p);
                self.recent.push(RecentEntry {
                    name: display_name(&path),
                    path: project_key(&path),
                    opened: now_secs(),
                    template: None,
                });
                self.save_recent();
            }
        }
        if changed {
            self.save_start();
        }
        first && last_project.is_none_or(|p| p.is_empty())
    }

    /// A named project was opened or saved: its row goes to the top (bounded, newest first).
    /// A row pushed past the bound takes its poster with it.
    pub fn touch_recent(&mut self, path: &std::path::Path, template: Option<&str>) {
        let key = project_key(path);
        let old = self.recent.iter().position(|e| e.path == key).map(|i| self.recent.remove(i));
        let template = template.map(str::to_string).or_else(|| old.and_then(|e| e.template));
        self.recent.insert(
            0,
            RecentEntry { name: display_name(&key), path: key, opened: now_secs(), template },
        );
        while self.recent.len() > RECENT_KEEP {
            if let Some(gone) = self.recent.pop()
                && let Some(p) = self.poster_path(&gone.path)
            {
                let _ = std::fs::remove_file(p);
            }
        }
        self.save_recent();
    }

    /// "Clear the list": the rows go, and so do the posters the list owned — nothing else.
    pub fn clear_recent(&mut self) {
        for e in std::mem::take(&mut self.recent) {
            if let Some(p) = self.poster_path(&e.path) {
                let _ = std::fs::remove_file(p);
            }
        }
        self.save_recent();
    }

    /// Drop one row (its context menu): the poster goes with it.
    pub fn forget_recent(&mut self, path: &std::path::Path) {
        let key = project_key(path);
        self.recent.retain(|e| e.path != key);
        if let Some(p) = self.poster_path(&key) {
            let _ = std::fs::remove_file(p);
        }
        self.save_recent();
    }

    /// Save a template; a template of the same name is replaced.
    pub fn add_template(&mut self, t: UserTemplate) {
        self.templates.retain(|x| x.name != t.name);
        self.templates.push(t);
        self.save_templates();
    }

    pub fn remove_template(&mut self, name: &str) {
        self.templates.retain(|x| x.name != name);
        self.save_templates();
    }

    pub fn set_last_template(&mut self, id: &str) {
        self.start.last_template = Some(id.to_string());
        self.save_start();
    }

    pub fn set_show_at_start(&mut self, on: bool) {
        if self.start.show_at_start != on {
            self.start.show_at_start = on;
            self.save_start();
        }
    }

    /// A lesson step closed.
    pub fn finish_step(&mut self, lesson: u32, step: u32) {
        let fresh = self.tour.done.entry(lesson).or_default().insert(step);
        self.tour.touched = now_secs().max(self.tour.touched);
        if fresh {
            self.save_tour();
        }
    }

    /// The tour was started or continued (the "opened since" clock is reset).
    pub fn touch_tour(&mut self) {
        self.tour.touched = now_secs().max(self.tour.touched);
        self.save_tour();
    }

    /// "Start over": the progress, and nothing else.
    pub fn reset_tour(&mut self) {
        self.tour = TourProgress::default();
        self.save_tour();
    }

    /// A project was opened after the tour was last touched.
    pub fn opened_since_tour(&self) -> bool {
        self.recent.first().is_some_and(|e| e.opened > self.tour.touched)
    }
}

/// The name of a project as the list shows it: the file name without the extension.
pub fn display_name(p: &std::path::Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

#[cfg(test)]
mod records_tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kt-records-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Test 985 (welcome-window-and-tour, durability): the recent list is bounded to 12,
    /// newest first, keeps a missing file, "Clear" removes the posters it owned; the former
    /// `last_project` seeds a list of one; a broken file goes aside to `.broken`, a version-99
    /// file is read and left unwritten.
    #[test]
    fn t985_recent_list_is_bounded_keeps_missing_and_owns_its_posters() {
        let store = tmp("t985-store");
        let projects = tmp("t985-projects");
        let mut s = StartStore::at(Some(store.clone()));
        let mut paths = Vec::new();
        for i in 0..14 {
            let p = projects.join(format!("p{i:02}.kt"));
            std::fs::write(&p, "x").unwrap();
            s.touch_recent(&p, Some("1080p30"));
            // the poster job's product (its own half of t985 runs the real render)
            let poster = s.poster_path(&p).unwrap();
            std::fs::create_dir_all(poster.parent().unwrap()).unwrap();
            std::fs::write(&poster, b"png").unwrap();
            paths.push(p);
        }
        assert_eq!(s.recent.len(), RECENT_KEEP, "the list is bounded");
        assert_eq!(s.recent[0].name, "p13", "newest first");
        assert_eq!(s.recent[11].name, "p02");
        // the two rows pushed past the bound took their posters with them
        for gone in &paths[..2] {
            assert!(!s.poster_path(gone).unwrap().exists(), "a row past the bound left its poster");
        }
        for kept in &paths[2..] {
            assert!(s.poster_path(kept).unwrap().exists());
        }
        // the 3rd kept project's file is deleted: the row stays, with its path
        std::fs::remove_file(&paths[4]).unwrap();
        let back = StartStore::at(Some(store.clone()));
        assert_eq!(back.recent.len(), RECENT_KEEP, "the list survives a reload");
        let row = back.recent.iter().find(|e| e.name == "p04").expect("a missing file stays listed");
        assert!(!row.path.exists(), "and is reported missing by its path");
        assert_eq!(row.template.as_deref(), Some("1080p30"), "the template it was made from");
        // no file appeared in the projects folder but the projects themselves
        let mut names: Vec<String> = std::fs::read_dir(&projects)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert!(names.iter().all(|n| n.ends_with(".kt")), "a foreign file in the project folder: {names:?}");
        // Clear: the list empties, the posters it owned go
        let mut back = back;
        back.clear_recent();
        assert!(back.recent.is_empty());
        let left = std::fs::read_dir(store.join("posters")).unwrap().count();
        assert_eq!(left, 0, "Clear must remove the posters the list owned");
        assert!(StartStore::at(Some(store.clone())).recent.is_empty(), "the clear reached the file");

        // the former last_project seeds a list of one, once
        let fresh = tmp("t985-seed");
        let mut s = StartStore::at(Some(fresh.clone()));
        let first = s.begin_session(Some(paths[13].to_str().unwrap()));
        assert!(!first, "a start with a former project is not the first start");
        assert_eq!(s.recent.len(), 1, "last_project seeds the list");
        let mut again = StartStore::at(Some(fresh.clone()));
        again.clear_recent();
        again.begin_session(Some(paths[13].to_str().unwrap()));
        assert!(again.recent.is_empty(), "the seed happens ONCE");
        // an empty store and no former key: the first start
        let mut empty = StartStore::at(Some(tmp("t985-first")));
        assert!(empty.begin_session(None), "an empty store is the first start");
        assert!(!empty.begin_session(None), "the second start is not");

        // a broken file: aside to .broken, the list starts empty
        let broken = tmp("t985-broken");
        std::fs::write(broken.join("recent.json"), "{ not json").unwrap();
        let mut s = StartStore::at(Some(broken.clone()));
        assert!(s.recent.is_empty() && s.is_broken("recent.json"));
        s.touch_recent(&paths[13], None);
        assert!(broken.join("recent.json.broken").exists(), "the broken file is kept aside");
        assert_eq!(StartStore::at(Some(broken.clone())).recent.len(), 1);

        // a file of version 99: read, never overwritten
        let future = tmp("t985-future");
        let text = r#"{"version":99,"recent":[{"path":"/x/a.kt","name":"a","opened":5}]}"#;
        std::fs::write(future.join("recent.json"), text).unwrap();
        let mut s = StartStore::at(Some(future.clone()));
        assert_eq!(s.recent.len(), 1, "a future file is read");
        assert!(s.is_readonly("recent.json"));
        s.touch_recent(&paths[13], None);
        assert_eq!(std::fs::read_to_string(future.join("recent.json")).unwrap(), text, "and not overwritten");
        for d in [store, projects, fresh, broken, future] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// Test 985, the poster half: a save and a close queue the poster job; it renders by the
    /// offline path into the store's posters folder, and NOTHING appears in the project folder
    /// or next to it (no sidecar written by the poster).
    #[test]
    fn t985_posters_render_into_the_store_never_into_the_project_folder() {
        crate::plugins::install();
        let store = tmp("t985p-store");
        let projects = tmp("t985p-projects");
        let path = projects.join("edit.kt");
        let text = crate::app::project_text_of(&crate::lessons::lesson_project(), &Default::default(), Some(&projects));
        crate::app::write_project_atomic(&path, &text).unwrap();
        let mut app = crate::app::KtApp::open(&path).expect("open");
        app.start.store = StartStore::at(Some(store.clone()));
        let before = crate::welcome::posters_done();
        app.queue_poster(); // what Ctrl+S and the close of a named project do
        assert!(crate::welcome::wait_posters(std::time::Duration::from_secs(60)), "the poster job hung");
        assert!(crate::welcome::posters_done() > before, "the job did not run");
        let poster = app.start.store.poster_path(&path).unwrap();
        assert!(poster.starts_with(store.join("posters")), "the poster lives in the store: {poster:?}");
        let img = image::open(&poster).expect("a PNG poster");
        assert_eq!((img.width(), img.height()), (320, 180));
        // a frame of the project, not a black rectangle (the generator sources of the events
        // were once missing from the poster's plan — every frame came out flat)
        let colours: std::collections::HashSet<[u8; 3]> = img.to_rgb8().pixels().map(|p| p.0).collect();
        assert!(colours.len() > 16, "the poster is flat ({} colours): nothing was composed", colours.len());
        drop(app);
        let mut names: Vec<String> = std::fs::read_dir(&projects)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert!(
            // the project's own sidecar (with its WAL files) is the project's, not the poster's
            names.iter().all(|n| n == "edit.kt" || n.starts_with("edit.ktindex.db")),
            "a poster file appeared next to the project: {names:?}"
        );
        assert!(!names.iter().any(|n| n.ends_with(".png")), "a PNG in the project folder: {names:?}");
        let _ = std::fs::remove_dir_all(&store);
        let _ = std::fs::remove_dir_all(&projects);
    }

    /// Test 987 (welcome-window-and-tour, durability): a user template, the tour progress and
    /// the start choice survive a restart; "Start over" clears the progress and nothing else.
    #[test]
    fn t987_templates_tour_and_start_choice_survive_a_restart() {
        let dir = tmp("t987");
        let mut s = StartStore::at(Some(dir.clone()));
        s.add_template(UserTemplate { name: "Shorts 60".into(), width: 1080, height: 1920, fps: 60 });
        s.finish_step(1, 0);
        s.finish_step(1, 1);
        s.set_show_at_start(false);
        let mut back = StartStore::at(Some(dir.clone()));
        assert_eq!(
            back.templates,
            vec![UserTemplate { name: "Shorts 60".into(), width: 1080, height: 1920, fps: 60 }]
        );
        assert_eq!(back.tour.finished_steps(), 2, "two finished steps survive");
        assert!(back.tour.is_done(1, 0) && back.tour.is_done(1, 1));
        assert!(!back.start.show_at_start, "the start choice is off after the reload");
        back.reset_tour();
        let after = StartStore::at(Some(dir.clone()));
        assert_eq!(after.tour.finished_steps(), 0, "Start over clears the progress");
        assert!(!after.start.show_at_start, "and nothing else: the start choice");
        assert_eq!(after.templates.len(), 1, "and nothing else: the template");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
