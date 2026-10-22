//! Tests for the project file — its OWN format (R.70): field-by-field and
//! byte-by-byte round trip, size ∝ model, version, git indexability.
//!
//! The half of t810 that needs `.otio` for comparison (“orders of magnitude smaller
//! than the same project in the exchange format”) lives in `kt-otio/tests/convert.rs`:
//! this crate knows nothing about the exchange format and must not.

use kt_model::project_file::{
    EXTENSION, FileKind, MARKER, PROJECT_FILE_VERSION, ProjectFileError, from_text,
    is_project_file, to_text,
};
use kt_model::*;
use kt_time::Tick;

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

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

/// Rich project: a reel with a crossfade, a morpher and a camera, its two instances,
/// a root transition, a marker, a region, settings — everything that
/// must survive the round trip.
fn rich_project() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "media/a.mp4"));
    p.media.push(MediaRef::new(MediaId(2), "media/b.mp4"));

    let mut s = Sequence::new(MediaId(100), "Intro");
    s.clips.push(clip(11, 1, 0, 45_000));
    s.clips.push(clip(12, 2, 45_000, 45_000));
    s.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(11)),
        b: Some(ClipId(12)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    s.morphers.push(MorpherEvent {
        id: MorphId(3),
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        targets: MorphTargets::Below,
        patch: Default::default(),
        enabled: true,
        timed: true,
        strength: 1.0,
        fade_in: Tick(0),
        fade_out: Tick(0),
    });
    s.ensure_tracks(LaneKind::Morph, 1);
    p.sequences.push(s);

    // two instances of a reel and an ordinary event between them
    p.clips.push(clip(1, 100, 0, 90_000));
    p.clips.push(clip(2, 1, 90_000, 30_000));
    p.clips.push(clip(3, 100, 120_000, 90_000));
    p.transitions.push(TransitionEvent {
        id: TransitionId(2),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick(3_000),
        out_offset: Tick(3_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.markers.push(Marker {
        t: Tick(60_000),
        duration: Tick(0),
        name: "scene".into(),
        color: Some("blue".into()),
    });
    p.region = Some(Region { start: Tick(0), end: Tick(100_000) });
    p.loop_enabled = true;
    // CANONICAL model, as after opening: `ensure_tracks` gives the reel’s
    // tracks a placeholder `TrackId(0)`, and reading hands out the real ids.
    // The round trip has to be compared with what the program actually writes,
    // and it writes the normalized form.
    p.normalize();
    p.validate().expect("the scene is legal");
    p
}

/// Foreign passthrough: deliberately NOT alphabetical, with nesting.
fn foreign_passthrough() -> serde_json::Value {
    serde_json::json!({
        "zeta": {"b": 1, "a": [3, {"y": 1, "x": 2}]},
        "alpha": "foreign namespace",
        "markers": [{"name": "their marker", "color": "RED"}],
    })
}

// ---- t811. field-by-field and byte-by-byte round trip ----------------------------------

#[test]
fn t811_round_trip_is_field_exact_and_byte_stable() {
    let p = rich_project();
    let text = to_text(&p, Some(&foreign_passthrough()));

    let back = from_text(&text).expect("own file reads");
    assert_eq!(back.project, p, "the model after a round trip must match field by field");
    assert!(back.otio.is_some(), "foreign passthrough lost on the round trip");

    let text2 = to_text(&back.project, back.otio.as_ref());
    assert_eq!(text2, text, "a second round trip must be byte-for-byte");

    // the marker and version come FIRST, on the first line: an unknown version
    // is rejected without reading the rest
    let first = text.lines().nth(1).unwrap_or_default();
    assert!(
        first.contains(&format!("\"{MARKER}\": {PROJECT_FILE_VERSION}")),
        "the version must be the first field of the file, but the first one is: {first}"
    );
    assert!(text.ends_with('\n'), "the file must end with a newline");
    assert_eq!(EXTENSION, "kt");
}

// ---- t810 (the half without .otio). size ∝ model, not the number of instances ----

#[test]
fn t810_a_second_instance_adds_a_reference_not_a_body() {
    let mut p = rich_project();
    // remove the second instance: count the gain from its return
    p.clips.retain(|c| c.id != ClipId(3));
    p.validate().unwrap();
    let one = to_text(&p, None);

    p.clips.push(clip(3, 100, 120_000, 90_000));
    p.validate().unwrap();
    let two = to_text(&p, None);

    p.clips.push(clip(4, 100, 210_000, 90_000));
    p.validate().unwrap();
    let three = to_text(&p, None);

    // the body of the reel (two events + a transition + a morpher) is certainly heavier than
    // one reference to it; the gain per instance is the reference
    let body = {
        let mut only_body = Project::new(settings());
        only_body.media = p.media.clone();
        only_body.sequences = p.sequences.clone();
        to_text(&only_body, None).len() - to_text(&Project::new(settings()), None).len()
    };
    let step_2 = two.len() - one.len();
    let step_3 = three.len() - two.len();
    assert!(
        step_2 < body / 2 && step_3 < body / 2,
        "an instance adds a reference to the file, not a body: growth {step_2} and {step_3} bytes \
         with a reel body of {body} bytes"
    );
    assert!(
        step_3.abs_diff(step_2) < 64,
        "each next instance costs the same as the previous one: {step_2} vs {step_3}"
    );
}

// ---- t812. version: rejection forward, foreign file — the import door ----------------

#[test]
fn t812_version_refuses_forward_and_names_the_import_door() {
    let p = rich_project();
    let text = to_text(&p, None);

    // a version GREATER than the current one — rejection with the number, no partial reading
    let newer = text.replacen(
        &format!("\"{MARKER}\": {PROJECT_FILE_VERSION}"),
        &format!("\"{MARKER}\": {}", PROJECT_FILE_VERSION + 7),
        1,
    );
    assert_ne!(newer, text, "scene: version found in the file");
    match from_text(&newer) {
        Err(ProjectFileError::TooNew { version }) => {
            assert_eq!(version, PROJECT_FILE_VERSION + 7)
        }
        other => panic!("a file of a future version must be rejected with its number, not {other:?}"),
    }
    let said = from_text(&newer).unwrap_err().to_string();
    assert!(said.contains(&(PROJECT_FILE_VERSION + 7).to_string()), "the error did not name the version: {said}");

    // an exchange file is not a project — the error names the import door
    let otio = r#"{"OTIO_SCHEMA": "Timeline.1", "name": "x", "tracks": {}}"#;
    match from_text(otio) {
        Err(ProjectFileError::NotOurs { looks_like: Some(FileKind::Otio) }) => {}
        other => panic!("an interchange file must be recognized as OTIO, not {other:?}"),
    }
    let said = from_text(otio).unwrap_err().to_string();
    assert!(said.contains("Import older Kinetren project"), "the error did not name the door: {said}");
    assert!(!is_project_file(otio) && is_project_file(&text));

    // a legacy project is also an import door, named as such
    let legacy = r#"{"format_version": 1, "settings": {}}"#;
    match from_text(legacy) {
        Err(ProjectFileError::NotOurs { looks_like: Some(FileKind::LegacyKtproj) }) => {}
        other => panic!("legacy must be recognized as .ktproj, not {other:?}"),
    }

    // just a foreign JSON — not ours, and this is said without guessing
    match from_text(r#"{"hello": 1}"#) {
        Err(ProjectFileError::NotOurs { looks_like: None }) => {}
        other => panic!("foreign JSON: {other:?}"),
    }

    // a broken model inside OUR envelope — a model error, not “not our file”
    let broken = text.replacen("\"fps\": 30", "\"fps\": 7", 1);
    assert_ne!(broken, text);
    assert!(
        matches!(from_text(&broken), Err(ProjectFileError::Model(_))),
        "a broken model must give a model error"
    );
}

// ---- t822. indexing in git ----------------------------------------------

#[test]
fn t822_the_file_is_indexable_in_git() {
    // about fifty events
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "media/a.mp4"));
    for i in 0..50u64 {
        p.clips.push(clip(1000 + i, 1, i as i64 * 45_000, 45_000));
    }
    p.validate().unwrap();
    let a = to_text(&p, None);

    // line-based: at least as many lines as events (a compact one would give one line)
    let lines_a: Vec<&str> = a.lines().collect();
    assert!(
        lines_a.len() >= 50,
        "the file must be line-based: {} lines for 50 events",
        lines_a.len()
    );

    // editing ONE event changes a few lines, not the file
    let last = p.clips.len() - 1;
    p.clips[last].start = p.clips[last].start + Tick(1_000);
    let b = to_text(&p, None);
    let lines_b: Vec<&str> = b.lines().collect();
    assert_eq!(lines_a.len(), lines_b.len(), "shifting an event does not change the number of lines");
    let changed = lines_a.iter().zip(&lines_b).filter(|(x, y)| x != y).count();
    assert!(changed <= 2, "an edit of one event changed {changed} lines, but must change a few");

    // determinism: the same model — the same bytes, and with the foreign passthrough too
    assert_eq!(to_text(&p, None), to_text(&p.clone(), None));
    let fp = foreign_passthrough();
    let with = to_text(&p, Some(&fp));
    assert_eq!(with, to_text(&p.clone(), Some(&fp.clone())));

    // passthrough is written AS IS: the order of foreign keys is a promise to the exchange file
    // (export returns them byte-for-byte, t154), and sorting them here is not allowed.
    // Determinism — yes, reordering — no.
    let z = with.find("\"zeta\"").expect("passthrough in the file");
    let al = with.find("\"alpha\"").expect("passthrough in the file");
    let m = with.find("\"markers\"").expect("passthrough in the file");
    assert!(z < al && al < m, "the order of foreign keys must be kept: zeta, alpha, markers");
    let y_pos = with.find("\"y\": 1").expect("y");
    let x_pos = with.find("\"x\": 2").expect("x");
    assert!(y_pos < x_pos, "and nested order too: y before x, as in the source");
}
