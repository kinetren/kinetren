//! The project file of OUR format against the exchange file (R.70) — the half of the format
//! tests that needs `.otio` for comparison and the atomic writer.
//! The format itself (round trip, version, git) is checked in
//! `kt-model/tests/project_file.rs`: the model knows nothing of the exchange format.
use std::cell::RefCell;
use std::io;
use std::path::Path;

use kt_model::project_file::{from_text, to_text};
use kt_model::*;
use kt_otio::{AtomicFs, Retained, save_project, write_atomic_with};
use kt_time::Tick;

fn clip(id: u64, media: u64, start: i64, dur: i64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// The very scene of the measurement: reel L1 of `inner` clips, L2 of
/// `uses2` instances of L1, the root of `uses1` instances of L2.
fn nested_scene(inner: u64, uses2: u64, uses1: u64) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "media/m1.mp4"));
    let mut next = 1000u64;
    let mut s1 = Sequence::new(MediaId(100), "L1");
    for i in 0..inner {
        next += 1;
        s1.clips.push(clip(next, 1, i as i64 * 45_000, 45_000));
    }
    let l1 = inner as i64 * 45_000;
    let mut s2 = Sequence::new(MediaId(101), "L2");
    for i in 0..uses2 {
        next += 1;
        s2.clips.push(clip(next, 100, i as i64 * l1, l1));
    }
    let l2 = uses2 as i64 * l1;
    for i in 0..uses1 {
        next += 1;
        p.clips.push(clip(next, 101, i as i64 * l2, l2));
    }
    p.sequences.push(s1);
    p.sequences.push(s2);
    p.normalize();
    p.validate().expect("the scene is legal");
    p
}

// ---- t810 (second half). orders of magnitude smaller than the exchange file ---------------

#[test]
fn t810_project_file_is_orders_of_magnitude_smaller_than_the_interchange_file() {
    // measurement of 02.10.2026: .otio 1.97 MB against 3.9 KB of compact JSON; now the file
    // is line-by-line (≈2× compact), and the threshold has a margin
    let p = nested_scene(10, 8, 8);
    let otio = save_project(&p, &Retained::default(), None);
    let ours = to_text(&p, None);
    let ratio = otio.len() / ours.len().max(1);
    assert!(
        ratio >= 100,
        "project file must be orders of magnitude smaller than the exchange file: {} bytes against {} ({}×)",
        ours.len(),
        otio.len(),
        ratio
    );
    // and not at the cost of content: the round trip is intact
    assert_eq!(from_text(&ours).expect("reads").project, p);

    // a flat project without repeats — also smaller, but without orders of magnitude: here the
    // gain comes only from the absence of a foreign wrapper
    let flat = nested_scene(10, 1, 1);
    let r_flat = save_project(&flat, &Retained::default(), None).len() / to_text(&flat, None).len();
    assert!(r_flat >= 4, "and a flat project is smaller than the exchange file: {r_flat}×");
    eprintln!("t810: nested scene {ratio}×, flat {r_flat}×");
}

// ---- t813. the write is honest about power loss ----------------------------

/// A substitute file system: fails at the named step. `write_and_sync` and `rename`
/// are real until told to fail — so the OUTCOME on disk is checked, not the
/// order of calls (that is held by t351).
struct FailAt {
    step: &'static str,
    log: RefCell<Vec<String>>,
}

impl AtomicFs for FailAt {
    fn write_and_sync(&self, tmp: &Path, bytes: &[u8]) -> io::Result<()> {
        self.log.borrow_mut().push("write".into());
        if self.step == "write" {
            // “power lost mid-write”: a stub remained on disk
            std::fs::write(tmp, &bytes[..bytes.len() / 2])?;
            return Err(io::Error::other("power loss during write"));
        }
        std::fs::write(tmp, bytes)
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.log.borrow_mut().push("rename".into());
        if self.step == "rename" {
            return Err(io::Error::other("power loss before rename"));
        }
        std::fs::rename(from, to)
    }
    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        self.log.borrow_mut().push("sync_dir".into());
        Ok(())
    }
}

#[test]
fn t813_project_write_leaves_old_or_new_never_half() {
    let dir = std::env::temp_dir().join(format!("kt-t813-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("film.kt");

    let old = to_text(&nested_scene(3, 1, 1), None);
    let new = to_text(&nested_scene(10, 2, 2), None);
    assert_ne!(old, new);
    std::fs::write(&path, &old).unwrap();

    for step in ["write", "rename"] {
        let fs = FailAt { step, log: RefCell::new(vec![]) };
        let r = write_atomic_with(&fs, &path, &new);
        assert!(r.is_err(), "a break at step “{step}” must be a REFUSAL, not a success");
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, old, "after a break at “{step}” the PREVIOUS project must lie on disk intact");
        assert_eq!(from_text(&on_disk).unwrap().project, nested_scene(3, 1, 1));
    }

    // without a break — the new one wholly, and it is read
    let fs = FailAt { step: "", log: RefCell::new(vec![]) };
    write_atomic_with(&fs, &path, &new).expect("write without a break goes through");
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert_eq!(on_disk, new, "after the write the NEW project must lie on disk intact");
    assert_eq!(from_text(&on_disk).unwrap().project, nested_scene(10, 2, 2));
    assert_eq!(fs.log.borrow().as_slice(), ["write", "rename", "sync_dir"]);

    let _ = std::fs::remove_dir_all(&dir);
}

// ---- t816/t817. foreign passthrough survives the project file -----------------

/// A foreign exchange file with an unknown field on Timeline, a foreign namespace in
/// metadata, a foreign field on a clip and a whole unknown node — exactly what
/// must come back on export.
fn foreign_document() -> String {
    serde_json::json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": "f",
        "vendor_field": {"deep": [1, 2, {"x": true}]},
        "metadata": {"resolve": {"colorVersion": 3}, "fcp_xml": "keep"},
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "children": [{
                "OTIO_SCHEMA": "Track.1",
                "kind": "Video",
                "children": [
                    {
                        "OTIO_SCHEMA": "Clip.2",
                        "name": "known",
                        "custom_clip_field": 42,
                        "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                            "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 90000.0},
                            "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 900.0, "rate": 90000.0}},
                        "media_references": {"DEFAULT_MEDIA": {
                            "OTIO_SCHEMA": "ExternalReference.1", "target_url": "a.mov",
                            "available_range": null}},
                        "active_media_reference_key": "DEFAULT_MEDIA"
                    },
                    {
                        "OTIO_SCHEMA": "FancyClip.7",
                        "zeta": 1,
                        "alpha": 2,
                        "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                            "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 24.0},
                            "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 24.0, "rate": 24.0}}
                    }
                ]
            }]
        }
    })
    .to_string()
}

#[test]
fn t816_retained_survives_json_round_trip() {
    let bundle = kt_otio::load_project(&foreign_document(), None).expect("foreign file is readable");
    let direct = save_project(&bundle.project, &bundle.retained, None);

    let json = bundle.retained.to_json();
    assert_eq!(json, bundle.retained.to_json(), "to_json must be deterministic");
    let back = Retained::from_json(&json).expect("passthrough reads back");
    assert_eq!(back.to_json(), json, "the Retained → JSON → Retained round trip must be an identity");

    // the real measure — EXPORT: with the restored passthrough the file is the same
    let via_json = save_project(&bundle.project, &back, None);
    assert_eq!(via_json, direct, "export through the restored passthrough must be byte for byte");
    for needle in ["vendor_field", "colorVersion", "custom_clip_field", "FancyClip.7", "\"zeta\""] {
        assert!(via_json.contains(needle), "foreign “{needle}” lost on the round trip");
    }
}

#[test]
fn t817_import_project_export_keeps_foreign_fields() {
    // import of a foreign .otio → project file → read → export
    let bundle = kt_otio::load_project(&foreign_document(), None).expect("import");
    let text = to_text(&bundle.project, Some(&bundle.retained.to_json()));
    assert!(text.contains("\"otio\""), "passthrough must lie in the project file as a separate field");

    let file = from_text(&text).expect("project file is readable");
    let retained = Retained::from_json(file.otio.as_ref().expect("passthrough field is in place"))
        .expect("passthrough from the project file is readable");
    let exported = save_project(&file.project, &retained, None);
    assert_eq!(
        exported,
        save_project(&bundle.project, &bundle.retained, None),
        "export after the round trip through the project file must match the direct export"
    );
    // and the foreign key order survived everything: zeta before alpha, as in the source
    let z = exported.find("\"zeta\"").unwrap();
    let a = exported.find("\"alpha\"").unwrap();
    assert!(z < a, "order of foreign keys must be preserved");

    // a project without a passthrough field does not carry it — the file does not grow for nothing
    let plain = to_text(&bundle.project, None);
    assert!(!plain.contains("\"otio\""), "empty passthrough is not written");
}
