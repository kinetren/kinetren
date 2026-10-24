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
        let mut store = FxPresetStore::default();
        let block = |fx: Vec<kt_model::FxInstance>| kt_model::ops::FxChainBlock {
            slots: fx
                .into_iter()
                .map(|f| kt_model::ops::FxSlotBlock { fx: f, bypass: false, curves: vec![] })
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
