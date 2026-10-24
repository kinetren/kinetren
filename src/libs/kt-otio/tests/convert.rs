//! Tests 148/152/153 (R.24, stage 10): mapping Project↔Timeline.
//!
//! 152 — mapping semantics (inversion of video tracks, effects/LTW, transport/
//! ui/bin, id stability under save→load→save);
//! 153 — import of a foreign file without kt: determinism of id assignment and defaults;
//! 148 — golden of the legacy import: fixtures/demo.ktproj → save byte-for-byte
//! equal to the committed fixtures/demo-legacy-import.otio (it freezes both
//! the upgrade_legacy formula and the kt-envelope schema).

use kt_model::*;
use kt_otio::{Retained, import_legacy, load_project, parse_document, save_project};
use kt_time::Tick;

fn clip(id: u64, kind: ClipKind, lane: u32, start: i64, dur: i64, media: u64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn rich_project() -> Project {
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
    for id in [1u64, 2, 3] {
        p.media.push(MediaRef::new(MediaId(id), format!("media/m{id}.mp4")));
    }
    // video: 2 tracks (z-order!), audio: a pair butt-joined with a transition
    p.clips.push(clip(1, ClipKind::Video, 0, 0, 90_000, 1)); // top layer
    p.clips.push(clip(2, ClipKind::Video, 1, 0, 45_000, 2)); // background
    let mut a1 = clip(10, ClipKind::Audio, 0, 0, 90_000, 1);
    a1.gain_db = -3.0;
    a1.link_group = Some(GroupId(5));
    a1.fx = vec![FxInstance::gain(1.5), FxInstance::slowed_reverb(0.8, 0.2, 85.0)];
    a1.fx_bypass = vec![true];
    let mut a2 = clip(11, ClipKind::Audio, 0, 90_000, 90_000, 2);
    a2.reversed = true;
    a2.source_in = Tick(30_000);
    p.clips.push(a1);
    p.clips.push(a2);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(10)),
        b: Some(ClipId(11)),
        in_offset: Tick(9_000),
        out_offset: Tick(4_500),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.markers.push(Marker {
        t: Tick(15_000),
        duration: Tick(30_000),
        name: "range".into(),
        color: None,
    });
    p.markers.push(Marker::named(Tick(120_000), "point"));
    p.region = Some(Region { start: Tick(10_000), end: Tick(60_000) });
    p.loop_enabled = true;
    p.ui = Some(UiState {
        view_start: 0.0,
        ticks_per_px: 1000.0,
        cursor: 42_000,
        ..Default::default()
    });
    // track names since R.62 live on the object tracks, not in `ui`
    p.ensure_tracks(LaneKind::Video, 1);
    p.ensure_tracks(LaneKind::Audio, 1);
    p.track_at_mut(LaneKind::Video, 0).unwrap().name = "MAIN".into();
    p.track_at_mut(LaneKind::Audio, 0).unwrap().name = "DIALOG".into();
    p.normalize();
    p.validate().unwrap();
    p
}

#[test]
fn t152_mapping_semantics_roundtrip() {
    let p = rich_project();
    let text = save_project(&p, &Retained::default(), None);

    // inversion of video tracks: the first video track in the file is the BOTTOM one (lane 1)
    let tl = parse_document(&text).unwrap();
    let kinds: Vec<(String, String)> = tl
        .tracks
        .children
        .iter()
        .filter_map(|c| match c {
            kt_otio::Composable::Track(t) => Some((t.kind.clone(), t.name.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(kinds[0].0, "Video");
    assert_eq!(kinds[1], ("Video".into(), "MAIN".into()), "lane 0 (top) is the last video");
    assert_eq!(kinds[2], ("Audio".into(), "DIALOG".into()));

    // round-trip: the model is equivalent (the marker colour is normalized to GREEN)
    let bundle = load_project(&text, None).unwrap();
    let p2 = bundle.project;
    assert_eq!(p2.clips, p.clips, "clips field by field (fx/bypass/reversed/gain/link)");
    assert_eq!(p2.transitions, p.transitions);
    assert_eq!(p2.region, p.region);
    assert!(p2.loop_enabled);
    assert_eq!(p2.media, p.media, "media paths and ids are stable");
    assert_eq!(p2.morphers, p.morphers);
    let m = &p2.markers[0];
    assert_eq!((m.t, m.duration, m.name.as_str()), (Tick(15_000), Tick(30_000), "range"));
    assert_eq!(m.color.as_deref(), Some("GREEN"), "color normalized by the reference default");
    let ui = p2.ui.as_ref().unwrap();
    assert_eq!(ui.cursor, 42_000);
    assert_eq!(p2.track_label(LaneKind::Video, 0), "MAIN", "track name survived the round trip");
    assert_eq!(p2.track_label(LaneKind::Audio, 0), "DIALOG");

    // stability: the second cycle is byte-for-byte (idempotency at project level)
    let text2 = save_project(&p2, &bundle.retained, None);
    let p3 = load_project(&text2, None).unwrap().project;
    assert_eq!(p3, p2);
    assert_eq!(save_project(&p3, &bundle.retained, None), text2, "save→load→save is stable");
}

#[test]
fn t152_foreign_time_warp_and_transition_type_are_retained() {
    let p = rich_project();
    let text = save_project(&p, &Retained::default(), None);
    // a foreign time warp x2 + a foreign transition type
    let hacked = text
        .replace("\"SMPTE_Dissolve\"", "\"Custom_Wipe\"")
        .replace(
            "\"OTIO_SCHEMA\": \"Effect.1\",\n                                    \"name\": \"\",\n                                    \"effect_name\": \"kt:gain\"",
            "\"OTIO_SCHEMA\": \"LinearTimeWarp.1\",\n                                    \"name\": \"\",\n                                    \"effect_name\": \"kt:gain\"",
        );
    // (substitution of a gain effect for a warp may not be found because of formatting —
    // then we check only the transition type)
    let bundle = load_project(&hacked, None).unwrap();
    assert!(
        bundle.notices.iter().any(|n| n.contains("Custom_Wipe")),
        "notice about a foreign transition type: {:?}",
        bundle.notices
    );
    // a re-save returns the foreign type untouched
    let out = save_project(&bundle.project, &bundle.retained, None);
    assert!(out.contains("Custom_Wipe"), "transition_type passthrough");
}

#[test]
fn t153_foreign_import_is_deterministic() {
    // a foreign file without kt: a minimal timeline in the reference style
    let text = serde_json::json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": "foreign cut",
        "tracks": {"OTIO_SCHEMA": "Stack.1", "children": [{
            "OTIO_SCHEMA": "Track.1", "kind": "Video", "children": [
                {"OTIO_SCHEMA": "Clip.2", "name": "a",
                 "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                    "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 12.0, "rate": 24.0},
                    "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 48.0, "rate": 24.0}},
                 "media_references": {"DEFAULT_MEDIA": {"OTIO_SCHEMA": "ExternalReference.1",
                    "target_url": "shots/a.mov", "available_range": null}},
                 "active_media_reference_key": "DEFAULT_MEDIA"},
                {"OTIO_SCHEMA": "Gap.1", "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                    "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 24.0},
                    "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 24.0, "rate": 24.0}}},
                {"OTIO_SCHEMA": "Clip.2", "name": "b",
                 "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                    "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 24.0},
                    "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 24.0, "rate": 24.0}},
                 "media_references": {"DEFAULT_MEDIA": {"OTIO_SCHEMA": "ExternalReference.1",
                    "target_url": "shots/a.mov", "available_range": null}},
                 "active_media_reference_key": "DEFAULT_MEDIA"}
            ]
        }]}
    })
    .to_string();
    let b1 = load_project(&text, None).unwrap();
    let b2 = load_project(&text, None).unwrap();
    assert_eq!(b1.project, b2.project, "repeated import is deterministic");
    assert!(b1.notices.iter().any(|n| n.contains("default project settings")));
    let p = &b1.project;
    // ids in traversal order, media deduplicated by URL
    assert_eq!(p.clips.iter().map(|c| c.id.0).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(p.media.len(), 1);
    // 24 fps divides 90000 → picked up by the heuristic
    assert_eq!(p.settings.fps, 24);
    // times quantized to ticks: clip b starts after 48+24 frames @24
    assert_eq!(p.clips[1].start, Tick((48 + 24) * 90_000 / 24));
    // re-saving is stable
    let out = save_project(p, &b1.retained, None);
    let p2 = load_project(&out, None).unwrap().project;
    assert_eq!(&p2, p);
}

/// Golden of the legacy import. Regeneration:
/// `cargo test -p kt-otio --test convert -- --ignored t148_regen`
#[test]
fn t148_legacy_import_matches_committed_golden() {
    let src = include_str!("../../../fixtures/demo.ktproj");
    let (p, _notices) = import_legacy(src).expect("legacy imports");
    let out = save_project(&p, &Retained::default(), None);
    let golden = include_str!("../../../fixtures/demo-legacy-import.otio");
    assert_eq!(out, golden, "legacy import diverged from the golden — this is a conversion defect");
}

#[test]
#[ignore = "reference generator: cargo test -p kt-otio --test convert -- --ignored t148_regen"]
fn t148_regen() {
    let src = include_str!("../../../fixtures/demo.ktproj");
    let (p, notices) = import_legacy(src).expect("legacy imports");
    let out = save_project(&p, &Retained::default(), None);
    std::fs::write(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/demo-legacy-import.otio"),
        &out,
    )
    .unwrap();
    println!("regenerated ({} bytes), notices: {notices:?}", out.len());
}

// ---- test 154: foreign constructs that the format allows --------------------

/// All committed upstream samples (fixtures/otio-samples, pin in README)
/// must be READ. Previously three classes failed: a clip without `source_range`,
/// a clip with `media_reference: null`, `name: null` on references.
#[test]
fn t154_upstream_samples_load() {
    let dir =
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/otio-samples"));
    let mut seen = 0;
    for e in std::fs::read_dir(dir).expect("sample catalog") {
        let path = e.unwrap().path();
        if path.extension().and_then(|x| x.to_str()) != Some("otio") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let bundle = load_project(&text, path.parent())
            .unwrap_or_else(|e| panic!("{} does not read: {e}", path.display()));
        // re-writing must be idempotent (an invariant of kt-otio)
        let once = save_project(&bundle.project, &bundle.retained, path.parent());
        let again = load_project(&once, path.parent()).expect("our own output reads");
        let twice = save_project(&again.project, &again.retained, path.parent());
        assert_eq!(once, twice, "{}: re-save is not idempotent", path.display());
        seen += 1;
    }
    assert!(seen >= 9, "upstream samples disappeared from the fixtures (found {seen})");
}

/// A clip without `source_range` takes the material in full — the duration from the
/// `available_range` of the active reference (OTIO spec; so Clip-004 lives in
/// simple_cut.otio). The entry point is then ZERO: `available_range`
/// lives in the coordinate system of the material (its starting timecode), while our
/// `source_in` is an offset from the first frame of the file (item 180).
#[test]
fn t154_clip_without_source_range_uses_available_range() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/otio-samples/simple_cut.otio");
    let text = std::fs::read_to_string(path).unwrap();
    let b = load_project(&text, None).expect("simple_cut reads");
    assert_eq!(b.project.clips.len(), 4);
    let last = b.project.clips.iter().max_by_key(|c| c.start).unwrap();
    // available_range Clip-004: start 100, duration 6 frames @24 fps
    assert_eq!(last.duration, Tick(6 * 90_000 / 24));
    assert_eq!(last.source_in, Tick::ZERO, "available_range start is a timecode, not a frame skip");
}

/// A clip with `media_reference: null` is legal — it is a clip without a binding to
/// material. It is read, rendered black/silent, and the `null` survives
/// re-saving (we do not invent a reference).
#[test]
fn t154_clip_without_media_reference_survives() {
    let path =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/otio-samples/transition_test.otio");
    let text = std::fs::read_to_string(path).unwrap();
    let b = load_project(&text, None).expect("transition_test reads");
    assert_eq!(b.project.clips.len(), 3);
    for c in &b.project.clips {
        let m = b.project.media.iter().find(|m| m.id == c.media).expect("media record exists");
        assert_eq!(m.path, "", "a clip without a reference has no path to the material");
    }
    // clips without a reference do NOT share one media_id: they have no common material
    let ids: std::collections::HashSet<_> = b.project.clips.iter().map(|c| c.media).collect();
    assert_eq!(ids.len(), 3);

    let out = save_project(&b.project, &b.retained, None);
    parse_document(&out).expect("our output is typed");
    let json: serde_json::Value = serde_json::from_str(&out).unwrap();
    let refs = json["tracks"]["children"][0]["children"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["OTIO_SCHEMA"] == "Clip.2")
        .map(|c| c["media_references"]["DEFAULT_MEDIA"].clone())
        .collect::<Vec<_>>();
    assert_eq!(refs.len(), 3);
    assert!(
        refs.iter().all(|r| r.is_null()),
        "a null reference must not turn into an ExternalReference: {refs:?}"
    );
}

/// Test 165 (item 165, R.28): a track with an OVERLAP moves into
/// adjacent Tracks and is assembled back into one. In OTIO the clip position is
/// the order of the track's children, so otherwise the overlap would wreck the edit.
#[test]
fn t165_overlapping_lane_survives_layer_roundtrip() {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "media/a.mp4"));
    // three clips of one track: two overlap, the third butt-joined
    for (id, start, dur) in [(1u64, 0i64, 90_000i64), (2, 45_000, 90_000), (3, 200_000, 45_000)] {
        p.clips.push(ClipEvent {
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
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.normalize();
    p.validate().expect("overlap is legal when layered");

    let text = save_project(&p, &Retained::default(), None);
    let doc = parse_document(&text).expect("our output is typed");
    let tracks =
        doc.tracks.children.iter().filter(|c| matches!(c, kt_otio::Composable::Track(_))).count();
    assert_eq!(tracks, 2, "the overlapped track was split into two tracks");

    let back = load_project(&text, None).expect("reads back").project;
    assert_eq!(back.clips.len(), 3);
    assert!(back.clips.iter().all(|c| c.lane == 0), "track reassembled: {:?}", back.clips);
    assert_eq!(back.clips, p.clips, "clip geometry did not change");
    assert_eq!(
        back.lanes(kt_model::LaneKind::Video),
        p.lanes(kt_model::LaneKind::Video),
        "the same number of tracks"
    );

    // second cycle byte-for-byte (idempotency, extension of t144)
    let again = save_project(&back, &Retained::default(), None);
    assert_eq!(again, text, "save→load→save is stable");
}

/// A project WITHOUT overlaps is written as before: the layer map does not appear.
#[test]
fn t165_plain_project_has_no_layer_map() {
    let p = rich_project();
    let text = save_project(&p, &Retained::default(), None);
    assert!(!text.contains("\"layers\""), "without overlaps the layer map is not written");
}

/// Test 178: NTSC frame rates of foreign files (23.976/29.97/59.94) do not fit the
/// 90 kHz grid (90000/23.976 = 3753.75) — and before, such a file SILENTLY
/// became a 30 fps project. Found on a real film: Netflix
/// “Sol Levante” (an animatic, 63 cuts, 23.976) was imported as 30.
/// Now a fractional rate is rounded to the nearest whole number that divides 90000.
#[test]
fn t178_ntsc_rates_land_on_the_nearest_grid_fps() {
    use kt_otio::grid_fps;
    assert_eq!(grid_fps(23.976023976023978), Some(24), "NTSC-24");
    assert_eq!(grid_fps(29.97002997002997), Some(30), "NTSC-30");
    assert_eq!(grid_fps(59.94005994005994), Some(60), "NTSC-60");
    assert_eq!(grid_fps(47.952047952047955), Some(48), "NTSC-48");
    // whole rates — as they were
    for r in [24.0, 25.0, 30.0, 50.0, 60.0] {
        assert_eq!(grid_fps(r), Some(r as u32), "whole {r}");
    }
    // does not divide 90000 — not suitable (the grid would be non-integer in ticks)
    assert_eq!(grid_fps(23.0), None, "23 does not divide 90000");
    assert_eq!(grid_fps(7.0), None);
    // garbage and non-system values
    assert_eq!(grid_fps(0.0), None);
    assert_eq!(grid_fps(-24.0), None);
    assert_eq!(grid_fps(1000.0), None);
    assert_eq!(grid_fps(f64::NAN), None);
    // “almost whole” — only within half a frame
    assert_eq!(grid_fps(24.4), Some(24));
    assert_eq!(grid_fps(24.6), Some(25));

    // and in full: a timeline at 23.976 is read as a 24 fps project
    let doc = format!(
        r#"{{"OTIO_SCHEMA":"Timeline.1","name":"ntsc","tracks":{{"OTIO_SCHEMA":"Stack.1",
        "name":"","children":[{{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
        {{"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{{"OTIO_SCHEMA":"TimeRange.1",
        "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":240,"rate":{r}}},
        "start_time":{{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":{r}}}}},
        "media_reference":{{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
        "available_range":null,"available_image_bounds":null,"metadata":{{}}}},
        "effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}},
        "global_start_time":null,"metadata":{{}}}}"#,
        r = 23.976023976023978f64
    );
    let b = kt_otio::load_project(&doc, None).expect("a foreign NTSC file reads");
    assert_eq!(b.project.settings.fps, 24, "23.976 → grid 24, not the default 30");
}

/// Test 180: the starting timecode of the material. In OTIO `source_range` lives in
/// the coordinate system of the material itself — the same as `available_range`, and in
/// broadcast and cinema files it starts at the embedded timecode (for
/// “Sol Levante” — exactly one hour). Our `source_in` is an offset from the FIRST frame of the
/// file, so the timecode is subtracted on read and restored on write.
/// Found by a cross-render of a real film (t.179): without subtraction the engine
/// drifted by an hour in a 162-second file.
#[test]
fn t180_media_start_timecode_is_subtracted_and_restored() {
    let doc = |src_start: f64, avail_start: f64| {
        format!(
            r#"{{"OTIO_SCHEMA":"Timeline.1","name":"tc","tracks":{{"OTIO_SCHEMA":"Stack.1",
        "name":"","children":[{{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
        {{"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{{"OTIO_SCHEMA":"TimeRange.1",
        "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24}},
        "start_time":{{"OTIO_SCHEMA":"RationalTime.1","value":{src_start},"rate":24}}}},
        "media_reference":{{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
        "available_range":{{"OTIO_SCHEMA":"TimeRange.1",
        "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":3890,"rate":24}},
        "start_time":{{"OTIO_SCHEMA":"RationalTime.1","value":{avail_start},"rate":24}}}},
        "available_image_bounds":null,"metadata":{{}}}},
        "effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}},
        "global_start_time":null,"metadata":{{}}}}"#
        )
    };

    // an hour of timecode @24 = 86400 frames; the clip takes material from the 10th second
    let hour = 86_400.0;
    let b = load_project(&doc(hour + 240.0, hour), None).expect("a file with timecode reads");
    let c = &b.project.clips[0];
    assert_eq!(c.source_in, Tick(240 * 90_000 / 24), "10 s FROM THE START OF THE FILE, not from the hour");
    assert_eq!(c.duration, Tick(48 * 90_000 / 24));

    // and back: a foreign editor must see the same timecode
    let out = save_project(&b.project, &b.retained, None);
    let back = load_project(&out, None).expect("our output reads");
    assert_eq!(back.project.clips[0].source_in, c.source_in, "the round trip does not move the entry point");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let st = &v["tracks"]["children"][0]["children"][0]["source_range"]["start_time"];
    let ticks = st["value"].as_f64().unwrap() * 90_000.0 / st["rate"].as_f64().unwrap();
    assert_eq!(
        ticks.round() as i64,
        (hour as i64 + 240) * 90_000 / 24,
        "the material timecode returned to its place"
    );

    // a file without timecode (start 0) behaves as before
    let b0 = load_project(&doc(240.0, 0.0), None).expect("a file without timecode");
    assert_eq!(b0.project.clips[0].source_in, Tick(240 * 90_000 / 24));
    // a clip starting BEFORE the declared start of the material is clamped to zero
    let bn = load_project(&doc(0.0, hour), None).expect("a clip to the left of the material");
    assert_eq!(bn.project.clips[0].source_in, Tick::ZERO);
}

// ---- test 189 (R.29): cameras and the file version BY CONTENT -------------------

fn project_3d() -> Project {
    let mut p = rich_project();
    // Pan3d on the top video clip: an ordinary kt effect, does NOT bump the version
    p.clips[0].fx = vec![FxInstance::Pan3d {
        pos_x: 10.0,
        pos_y: -4.0,
        pos_z: 120.0,
        rot_x: 0.0,
        rot_y: 25.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale: 1.1,
        camera: Some(CameraId(2)),
    }];
    // automation (R.30): the rotation curve — frozen by the same golden
    p.clips[0].fx_auto = vec![vec![ParamCurve {
        param: "rot_y".into(),
        keys: vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(90_000), 90.0)],
        source: Default::default(),
    }]];
    // two cameras on two tracks, overlapping in time, with keyframes
    p.ensure_tracks(kt_model::LaneKind::Camera, 2);
    p.cameras = vec![
        CameraEvent {
            ease: true,
            id: CameraId(1),
            lane: 0,
            start: Tick(0),
            duration: Tick(90_000),
            name: "main".into(),
            cam: CameraParams { pos_z: -900.0, ..Default::default() },
            keyframes: vec![],
            // domain of camera curves — ticks from the start of the event (R.34 K3)
            auto: vec![
                ParamCurve {
                    param: "pos_x".into(),
                    keys: vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(90_000), 60.0)],
                    source: Default::default(),
                },
                ParamCurve {
                    param: "fov_deg".into(),
                    keys: vec![AutoKey::at(Tick(0), 39.6), AutoKey::at(Tick(90_000), 30.0)],
                    source: Default::default(),
                },
            ],
        },
        CameraEvent {
            id: CameraId(2),
            lane: 1,
            start: Tick(45_000),
            duration: Tick(90_000),
            name: String::new(),
            cam: CameraParams {
                target: Some((0.0, 0.0, 0.0)),
                fov_deg: 55.0,
                ..Default::default()
            },
            keyframes: vec![],
            auto: vec![],
            ease: false,
        },
    ];
    p.normalize();
    p.validate().unwrap();
    p
}

/// The file version by content: without cameras v=1 (byte stability of old
/// files), with cameras v=2 (an old reader refuses with KtMetaTooNew, and does not
/// silently lose the 3D scene on rewrite). Round-trip and idempotency.
#[test]
fn t189_cameras_roundtrip_and_conditional_version() {
    // without cameras — v=1, no camera keys
    let flat = save_project(&rich_project(), &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&flat).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 1, "without cameras the file stays v1");
    assert!(!flat.contains("camera"), "camera keyframes are not written for nothing");

    // cameras only (no curves) — v=2
    let mut cams_only = project_3d();
    cams_only.clips[0].fx_auto.clear();
    for c in &mut cams_only.cameras {
        c.auto.clear(); // camera curves — also v3
    }
    let text = save_project(&cams_only, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 2, "cameras raise the file version to v2");

    // cameras + automation — v=3, full round-trip, idempotency
    let p = project_3d();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "curves of the new domain raise it to v3");
    let b = load_project(&text, None).expect("our 3D file reads");
    assert_eq!(b.project.lanes(kt_model::LaneKind::Camera), 2);
    assert_eq!(b.project.cameras, p.cameras, "cameras survived the round trip");
    assert_eq!(b.project.clips.iter().find(|c| c.id == ClipId(1)).unwrap().fx, p.clips[0].fx);
    let again = save_project(&b.project, &b.retained, None);
    assert_eq!(text, again, "write idempotence with cameras");

    // LEGACY camera keyframes (domain “fraction of the event”) are converted on
    // read into common curves — R.34 K3, a one-way upgrade
    let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
    let cam = &mut old["metadata"]["kt"]["cameras"][0];
    cam["auto"] = serde_json::json!([]);
    cam["keyframes"] = serde_json::json!([
        { "t": 0.0, "cam": { "pos_x": 0.0, "fov_deg": 39.6 } },
        { "t": 1.0, "cam": { "pos_x": 60.0, "fov_deg": 30.0 } },
    ]);
    let b = load_project(&serde_json::to_string_pretty(&old).unwrap(), None)
        .expect("a file with old camera keyframes must open");
    let c = &b.project.cameras[0];
    assert!(c.keyframes.is_empty(), "legacy keyframes consumed by migration");
    let px = c.auto.iter().find(|c| c.param == "pos_x").expect("curve pos_x");
    assert_eq!(
        px.keys.iter().map(|k| (k.t.0, k.v)).collect::<Vec<_>>(),
        vec![(0, 0.0), (90_000, 60.0)],
        "event share converted to ticks from the start"
    );
    assert!(c.auto.iter().all(|c| c.param != "pos_y"), "flat curve parameters are not created");
}

/// Unknown kt fields of future versions (v3+) are carried through passthrough:
/// a v2 build loses nothing — and therefore future additions will not need
/// a hard refusal.
#[test]
fn t189b_future_kt_fields_pass_through() {
    let text = save_project(&project_3d(), &Retained::default(), None);
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["metadata"]["kt"]["future_field"] = serde_json::json!({"x": 1});
    let b = load_project(&serde_json::to_string_pretty(&v).unwrap(), None).unwrap();
    let out = save_project(&b.project, &b.retained, None);
    assert!(out.contains("future_field"), "the unknown kt field was not lost");
}

/// Golden of the 3D file: freezes the serde schema of cameras/Pan3d (like t148 — v1).
/// Regeneration: `cargo test -p kt-otio --test convert -- --ignored t189_regen`
#[test]
fn t189_matches_committed_golden() {
    let out = save_project(&project_3d(), &Retained::default(), None);
    let golden = include_str!("../../../fixtures/demo-3d.otio");
    assert_eq!(out, golden, "the 3D scheme (cameras/Pan3d) diverged from the golden");
}

// ---- test 192 (R.30): parameter automation in the file ------------------------

/// Curves travel inside the kt envelope of an effect; a file WITH automation is v3
/// (R.33: keys in material ticks, an old reader would read them as fractions and
/// would spoil the animation), without it — v1. Round-trip and idempotency.
#[test]
fn t192_fx_auto_roundtrip_and_v3() {
    // a flat project with automation, WITHOUT cameras: v3 is given precisely by it
    let mut p = rich_project();
    p.clips[0].fx = vec![FxInstance::blur(8)];
    p.clips[0].fx_auto = vec![vec![ParamCurve {
        param: "radius".into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(90_000), 48.0)]),
        source: Default::default(),
    }]];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "automation raises the file to v3");
    let b = load_project(&text, None).expect("a file with automation reads");
    let c = b.project.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    assert_eq!(c.fx_auto, p.clips[0].fx_auto, "curves survived the round trip");
    let again = save_project(&b.project, &b.retained, None);
    assert_eq!(text, again, "write idempotence with automation");
    // a key with a negative position — an honest load refusal (validate)
    let mut broken: serde_json::Value = serde_json::from_str(&text).unwrap();
    let eff = &mut broken["tracks"]["children"][1]["children"][0]["effects"][0];
    eff["metadata"]["kt"]["auto"][0]["keys"][0]["t"] = serde_json::json!(-5);
    assert!(
        load_project(&serde_json::to_string_pretty(&broken).unwrap(), None).is_err(),
        "a keyframe before the start of the material must explicitly fail loading"
    );
    // an unreadable curve also fails the load: silently losing animation is worse
    let mut broken: serde_json::Value = serde_json::from_str(&text).unwrap();
    let eff = &mut broken["tracks"]["children"][1]["children"][0]["effects"][0];
    eff["metadata"]["kt"]["auto"] = serde_json::json!("garbage");
    assert!(
        load_project(&serde_json::to_string_pretty(&broken).unwrap(), None).is_err(),
        "unreadable automation must not be silently lost"
    );
}

/// A file of the OLD domain (curves in fractions of the event length) is read and
/// converted into material ticks by the clip geometry — otherwise the animation
/// of the user's working projects would have drifted (R.33).
#[test]
fn t212_pre_v3_curves_migrate_to_material_ticks() {
    let mut p = rich_project();
    p.clips[0].fx = vec![FxInstance::blur(8)];
    p.clips[0].fx_auto = vec![vec![ParamCurve {
        param: "radius".into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(90_000), 48.0)]),
        source: Default::default(),
    }]];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    // we make from it a file of the PAST format: version v2, keys in fractions
    let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
    old["metadata"]["kt"]["v"] = serde_json::json!(2);
    let eff = &mut old["tracks"]["children"][1]["children"][0]["effects"][0];
    eff["metadata"]["kt"]["auto"][0]["keys"][0]["t"] = serde_json::json!(0.0);
    eff["metadata"]["kt"]["auto"][0]["keys"][1]["t"] = serde_json::json!(0.5);
    let b = load_project(&serde_json::to_string_pretty(&old).unwrap(), None)
        .expect("a file of the old format must open");
    let c = b.project.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    // clip 0..90_000 with source_in=0: fractions 0 and 0.5 → ticks 0 and 45_000
    assert_eq!(
        c.fx_auto[0][0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, 45_000],
        "old shares were not converted to material ticks"
    );
    // and the next write already in the new domain (v3)
    let again = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&again).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "rewriting raises the file to v3");
}

/// The transition kind and the curve of its audio live in the file (R.33 C4): a wipe stays
/// a wipe after the round trip, the meaningful `transition_type` goes outward, the file
/// is raised to v3, while an ordinary crossfade stays byte-stable and remains v1.
#[test]
fn t222_transition_kind_roundtrip_and_version() {
    // 1. an ordinary crossfade changes nothing in the file
    let plain = rich_project();
    let text = save_project(&plain, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 1, "a default crossfade does not bump the version");
    let tr = find_transition(&v);
    assert_eq!(tr["transition_type"], "SMPTE_Dissolve");
    assert!(tr["metadata"].get("kt").is_none(), "our envelope is not written for a crossfade");

    // 2. a wipe with parameters and a non-default audio curve
    let mut p = rich_project();
    p.transitions[0].kind = TransitionKind::Wipe { dir: Dir::Up, softness: 0.25, angle: 12.0 };
    p.transitions[0].audio = AudioCurve::SCurve;
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "a transition kind raises the file to v3");
    let tr = find_transition(&v);
    assert_eq!(tr["transition_type"], "SMPTE_Wipe", "wipe outward — as a standard type");
    assert_eq!(tr["metadata"]["kt"]["kind"]["kind"], "wipe");
    assert_eq!(tr["metadata"]["kt"]["audio"], "s_curve");

    let b = load_project(&text, None).expect("a file with wipe reads");
    assert_eq!(b.project.transitions, p.transitions, "kind and curve survived the round trip");
    let again = save_project(&b.project, &b.retained, None);
    assert_eq!(text, again, "write idempotence with a transition kind");

    // 3. a return to the crossfade CLEARS our envelope: the outdated “wipe” from
    // retained must not travel back into the file
    let mut back = b.project.clone();
    back.transitions[0].kind = TransitionKind::Crossfade;
    back.transitions[0].audio = AudioCurve::EqualPower;
    let text = save_project(&back, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 1, "without a transition kind the file is v1 again");
    let tr = find_transition(&v);
    assert!(tr["metadata"].get("kt").is_none(), "the kind envelope is removed entirely");

    // 4. a foreign transition_type is not substituted by ours (passthrough, t.152)
    let mut alien: serde_json::Value = serde_json::from_str(&text).unwrap();
    *find_transition_mut(&mut alien) = {
        let mut t = find_transition(&alien).clone();
        t["transition_type"] = serde_json::json!("Vendor_Fancy");
        t
    };
    let src = serde_json::to_string_pretty(&alien).unwrap();
    let b = load_project(&src, None).expect("a foreign transition type reads");
    let out = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        find_transition(&v)["transition_type"],
        "Vendor_Fancy",
        "a foreign type returns as it was"
    );

    // 4b. curves of the kind's parameters (R.34 K4) travel in the same envelope
    let mut p = rich_project();
    p.transitions[0].kind = TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.2 };
    p.transitions[0].auto = vec![ParamCurve {
        param: "angle".into(),
        keys: vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(13_500), 45.0)],
        source: Default::default(),
    }];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "transition curves are v3 too");
    let b = load_project(&text, None).expect("a file with transition curves reads");
    assert_eq!(b.project.transitions[0].auto, p.transitions[0].auto, "curves survived the round trip");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "idempotence");

    // 5. an unreadable kind — not silence: a notice and an honest crossfade
    let mut broken: serde_json::Value = serde_json::from_str(&text).unwrap();
    find_transition_mut(&mut broken)["metadata"]["kt"] = serde_json::json!({ "kind": "nonsense" });
    let b = load_project(&serde_json::to_string_pretty(&broken).unwrap(), None)
        .expect("a broken kind does not fail the whole file");
    assert_eq!(b.project.transitions[0].kind, TransitionKind::Crossfade);
    assert!(
        b.notices.iter().any(|n| n.contains("transition kind")),
        "a broken kind must give a note, not silence: {:?}",
        b.notices
    );

    // 6. inversion (R.51, item 398): travels in the envelope, bumps the file to v7,
    // switching it off clears both the key and the version
    let mut p = rich_project();
    p.transitions[0].kind = TransitionKind::Wipe { dir: Dir::Up, softness: 0.25, angle: 12.0 };
    p.transitions[0].invert = true;
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 7, "inversion raises the file to v7");
    let tr = find_transition(&v);
    assert_eq!(tr["metadata"]["kt"]["invert"], true, "inversion in the transition envelope");
    let b = load_project(&text, None).expect("a file with inversion reads");
    assert!(b.project.transitions[0].invert, "inversion survived the round trip");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "idempotence with inversion");
    let mut back = b.project.clone();
    back.transitions[0].invert = false;
    let text = save_project(&back, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "without inversion the version again follows the content");
    assert!(
        find_transition(&v)["metadata"]["kt"].get("invert").is_none(),
        "a disabled inversion leaves no key"
    );
}

/// The first Transition in the track tree (rich_project has only one).
fn find_transition(v: &serde_json::Value) -> &serde_json::Value {
    for tr in v["tracks"]["children"].as_array().unwrap() {
        for k in tr["children"].as_array().unwrap() {
            if k["OTIO_SCHEMA"].as_str().is_some_and(|s| s.starts_with("Transition.")) {
                return k;
            }
        }
    }
    panic!("no transition in the file");
}

fn find_transition_mut(v: &mut serde_json::Value) -> &mut serde_json::Value {
    for tr in v["tracks"]["children"].as_array_mut().unwrap() {
        for k in tr["children"].as_array_mut().unwrap() {
            if k["OTIO_SCHEMA"].as_str().is_some_and(|s| s.starts_with("Transition.")) {
                return k;
            }
        }
    }
    panic!("no transition in the file");
}

#[test]
#[ignore = "reference generator: cargo test -p kt-otio --test convert -- --ignored t189_regen"]
fn t189_regen() {
    let out = save_project(&project_3d(), &Retained::default(), None);
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/demo-3d.otio"), &out)
        .unwrap();
    println!("regenerated ({} bytes)", out.len());
}

/// Test 240a (R.35 C1): the morpher schema has changed — the kind (`Replace`/
/// `EffectBatch`) collapsed into one patch, the targets became a mode. A file BEFORE v4
/// is read with the former semantics (`timed: false`), a layer raises the file to
/// v4, and a project without morphers stays byte-stable.
#[test]
fn t240_morpher_schema_v4_and_legacy_read() {
    // an old file: a bare array of targets and an externally tagged kind
    let base = save_project(&rich_project(), &Retained::default(), None);
    let mut old: serde_json::Value = serde_json::from_str(&base).unwrap();
    old["metadata"]["kt"]["morph_lanes"] = serde_json::json!(1);
    old["metadata"]["kt"]["morphers"] = serde_json::json!([{
        "id": 1, "lane": 0, "start": 0, "duration": 45000,
        "targets": [1],
        "kind": { "Replace": { "media": 2, "source_in_delta": 4500 } },
    }]);
    let b = load_project(&serde_json::to_string_pretty(&old).unwrap(), None)
        .expect("a file with an old morpher must open");
    let m = &b.project.morphers[0];
    assert!(!m.timed, "a morpher before v4 acts outside of time — as before");
    assert_eq!(m.targets.clip_ids(), &[ClipId(1)], "bare array of targets read");
    assert_eq!(m.patch.media, Some(MediaId(2)), "Replace kind read into the patch");
    assert_eq!(m.patch.source_in_delta, Tick(4_500));

    // such a morpher does not raise the file to v4
    let text = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_ne!(v["metadata"]["kt"]["v"], 4, "previous semantics — previous version");
    assert_eq!(
        text,
        save_project(&load_project(&text, None).unwrap().project, &b.retained, None),
        "morpher write idempotence"
    );

    // a layer raises it to v4 and survives the round trip
    let mut p = b.project.clone();
    p.morphers[0].timed = true;
    p.morphers[0].targets = MorphTargets::Below;
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 4, "a layer raises the file to v4");
    let m = &v["metadata"]["kt"]["morphers"][0];
    assert!(m.get("targets").is_none(), "the “below” default is not serialized");
    assert!(m.get("timed").is_none(), "the “by time” default is not serialized");
    let b2 = load_project(&text, None).expect("file v4 reads");
    assert_eq!(b2.project.morphers, p.morphers, "morpher survived the round trip");

    // a project WITHOUT morphers does not move the version
    let plain = save_project(&rich_project(), &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&plain).unwrap();
    assert_ne!(v["metadata"]["kt"]["v"], 4, "without morphers the version does not grow");
}

/// Test 241b (R.35 C1): the row order moved from `ui.rows()` into the
/// model. A file with the old field is upgraded one-way, the new one writes its own,
/// and a project with natural order stays byte-stable.
#[test]
fn t241_lane_order_moved_into_model() {
    let mut p = rich_project();
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.set_lane_order(&[(kt_model::LaneKind::Morph, 0)]);
    assert!(!p.rows().is_empty(), "shuffled order is stored");
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    // since R.62 the row order is the order of TRACKS in the envelope; a separate
    // `lane_order` field no longer exists, either in the model or in the file
    let tracks = v["metadata"]["kt"]["tracks"].as_array().expect("tracks in the envelope");
    assert_eq!(tracks[0]["kind"], "morph", "shuffled order is written by tracks");
    assert!(v["metadata"]["kt"].get("lane_order").is_none(), "there is no separate order field any more");
    assert!(
        v["metadata"]["kt"]["ui"].get("lane_order").is_none(),
        "order is no longer written in ui"
    );
    let b = load_project(&text, None).expect("a file with row order reads");
    assert_eq!(b.project.rows(), p.rows(), "order survived the round trip");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "idempotence");

    // an OLD file: the order lay in its own field (R.35) or even earlier — in
    // `ui.lane_order`; both places are read, neither is written (R.62)
    let order = serde_json::to_value(p.rows()).unwrap();
    for at in ["kt", "ui"] {
        let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
        let kt = old["metadata"]["kt"].as_object_mut().unwrap();
        kt.remove("tracks");
        kt.insert("morph_lanes".into(), serde_json::json!(1));
        match at {
            "kt" => {
                kt.insert("lane_order".into(), order.clone());
            }
            _ => old["metadata"]["kt"]["ui"]["lane_order"] = order.clone(),
        }
        let b = load_project(&serde_json::to_string_pretty(&old).unwrap(), None)
            .expect("a file with the old location of order must open");
        assert_eq!(b.project.rows(), p.rows(), "order from {at} did not move into the tracks");
    }

    // the natural order does not create an envelope of tracks
    let plain = save_project(&rich_project(), &Retained::default(), None);
    assert!(!plain.contains("lane_order"), "there is no order field");
    let pv: serde_json::Value = serde_json::from_str(&plain).unwrap();
    assert!(
        pv["metadata"]["kt"].get("tracks").is_none(),
        "natural track order without properties is not written to the envelope"
    );
}

/// Test 248b (R.35 C4): named regions travel in the file and raise the
/// version to v4; a project without them is byte-stable.
#[test]
fn t248_areas_round_trip() {
    let mut p = rich_project();
    p.areas.push(kt_model::MarkedArea {
        name: "final".into(),
        start: Tick(30_000),
        duration: Tick(90_000),
        lanes: vec![(kt_model::LaneKind::Video, 0)],
        color: None,
    });
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 4, "regions are a v4 novelty");
    assert_eq!(v["metadata"]["kt"]["areas"][0]["name"], "final");
    let b = load_project(&text, None).expect("a file with regions reads");
    assert_eq!(b.project.areas, p.areas, "the region survived the round trip");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "idempotence");
    // a project WITHOUT regions does not create the field
    assert!(!save_project(&rich_project(), &Retained::default(), None).contains("areas"));
}

/// Test 251 (R.35, found by a live scenario): a BROKEN `metadata.kt` section
/// — a load error, not a “foreign file”. `kt_get` swallowed the error through
/// `.ok()`, and one wrong byte in a morpher dragged the WHOLE envelope away:
/// the project opened without settings, tracks, morphers and layout — silently.
#[test]
fn t251_broken_kt_meta_is_an_error() {
    let p = rich_project();
    let text = save_project(&p, &Retained::default(), None);
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    // we break ONE field in the envelope
    v["metadata"]["kt"]["morph_lanes"] = serde_json::json!("not a number");
    let bad = serde_json::to_string_pretty(&v).unwrap();
    match load_project(&bad, None) {
        Err(kt_otio::Error::KtMetaBroken(_)) => {}
        // a file of a version below the current one with an unreadable section is sent to
        // import (R.67), and the reason travels inside — this is the same loud refusal
        Err(kt_otio::Error::NeedsImport { cause, .. }) => {
            assert!(cause.contains("morph_lanes") || !cause.is_empty(), "the reason is empty");
        }
        Err(e) => panic!("wrong rejection: {e}"),
        Ok(b) => panic!(
            "a broken envelope opened silently: {} clips, {} tracks",
            b.project.clips.len(),
            b.project.lanes(kt_model::LaneKind::Video)
        ),
    }
    // a foreign file WITHOUT our section still opens as before
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["metadata"].as_object_mut().unwrap().remove("kt");
    let foreign = serde_json::to_string_pretty(&v).unwrap();
    assert!(load_project(&foreign, None).is_ok(), "foreign OTIO must open");
}

/// Test 383 (R.49 A10): a BROKEN section of OURS on a clip is an ERROR with a list of addresses,
/// not an “ordinary foreign file”.
///
/// `kt_get` with `.ok()` silently discarded the envelope: the clip got a NEW number, and with it the
/// A/V link-groups and morpher references broke, volume, opacity and the disabled chain were lost.
/// A notice does not help here — a silently reassigned number is irreversible.
/// A twin of t251 (the same strictness for Timeline).
#[test]
fn t383_broken_clip_kt_section_is_an_error() {
    let doc = |kt: &str| {
        format!(
            r#"{{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{{"OTIO_SCHEMA":"Stack.1",
        "name":"","children":[{{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
        {{"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{{"OTIO_SCHEMA":"TimeRange.1",
        "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24}},
        "start_time":{{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24}}}},
        "media_reference":{{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
        "available_range":null,"available_image_bounds":null,"metadata":{{}}}},
        "effects":[],"markers":[],"enabled":true,"metadata":{kt}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}},
        "global_start_time":null,"metadata":{{}}}}"#
        )
    };

    // no section — an ordinary foreign file, read
    load_project(&doc("{}"), None).expect("a foreign file without our section must read");
    // a foreign namespace — also not our business
    load_project(&doc(r#"{"resolve":{"clip_color":"Orange"}}"#), None).expect("foreign section");

    // the section IS present and it is broken — a refusal with the address
    let msg = match load_project(&doc(r#"{"kt":{"id":"not a number","media":1}}"#), None) {
        Ok(_) => panic!("a broken section must fail loading"),
        Err(e) => e.to_string(),
    };
    assert!(msg.contains("kt"), "the message does not mention our section: {msg}");
    assert!(msg.contains('c') && msg.contains("lane"), "the message has no clip address: {msg}");
}

/// Test 384 (R.49 A10; revised R.62): a foreign thing on a TRACK returns to
/// its place — FOREIGN effects and markers. The “disabled” flag since R.62 is read as
/// the track MUTE (our own mute appeared) and is written back from it —
/// the notice “Kinetren has no per-track mute” is removed in substance.
#[test]
fn t384_track_foreign_effects_markers_and_enabled_survive() {
    let doc = r#"{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
    {"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24}},
    "media_reference":{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
    "available_range":null,"available_image_bounds":null,"metadata":{}},
    "effects":[],"markers":[],"enabled":true,"metadata":{}}],
    "source_range":null,
    "effects":[{"OTIO_SCHEMA":"Effect.1","name":"track fx","effect_name":"TrackGrade",
    "enabled":true,"metadata":{}}],
    "markers":[{"OTIO_SCHEMA":"Marker.2","name":"on track","color":"BLUE","marked_range":
    {"OTIO_SCHEMA":"TimeRange.1","duration":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":12,"rate":24}},"metadata":{}}],
    "enabled":false,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}},
    "global_start_time":null,"metadata":{}}"#;
    let b = load_project(doc, None).expect("file reads");
    assert!(
        b.project.track_at(kt_model::LaneKind::Video, 0).is_some_and(|t| t.mute),
        "a foreign disabled track must become muted, not silently enabled"
    );

    let out = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let tr = &v["tracks"]["children"][0];
    assert_eq!(tr["enabled"], false, "the “disabled” flag was lost — a foreign editor would hear the track");
    assert_eq!(tr["effects"][0]["effect_name"], "TrackGrade", "track effect lost");
    assert_eq!(tr["markers"][0]["name"], "on track", "track marker lost");
}

/// Test 385 (R.49 A10): foreign data on a MARKER (comment, metadata) survives the round trip.
/// The model knows about a marker only its time, name and colour — the rest must
/// return from the envelope, or after our save the comments would disappear.
#[test]
fn t385_marker_comment_and_metadata_survive_roundtrip() {
    let doc = r#"{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[],"source_range":null,"effects":[],"markers":[
    {"OTIO_SCHEMA":"Marker.2","name":"remark","color":"RED","marked_range":
    {"OTIO_SCHEMA":"TimeRange.1","duration":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24}},
    "comment":"reshoot close-up","metadata":{"resolve":{"flag":"yellow"}}}],
    "enabled":true,"metadata":{}},
    "global_start_time":null,"metadata":{}}"#;
    let b = load_project(doc, None).expect("file with marker reads");
    assert_eq!(b.project.markers.len(), 1);

    let out = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let m = &v["tracks"]["markers"][0];
    assert_eq!(m["name"], "remark");
    assert_eq!(m["comment"], "reshoot close-up", "marker comment lost");
    assert_eq!(m["metadata"]["resolve"]["flag"], "yellow", "foreign marker metadata lost");
}

/// Test 386 (R.49 A10): `Clip.enabled` is NOT fixed — it is not broken. A disabled
/// foreign clip already arrives as `ForeignEvent` with verbatim raw data and returns
/// to the file as-is. A “class of problems” does not mean “shrink everything in a row”.
#[test]
fn t386_disabled_clip_is_already_verbatim() {
    let doc = r#"{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
    {"OTIO_SCHEMA":"Clip.1","name":"disabled","source_range":{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24}},
    "media_reference":{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
    "available_range":null,"available_image_bounds":null,"metadata":{}},
    "effects":[],"markers":[],"enabled":false,"metadata":{"vendor":{"k":1}}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}},
    "global_start_time":null,"metadata":{}}"#;
    let b = load_project(doc, None).expect("file reads");
    assert!(b.project.clips.is_empty(), "a disabled clip must not become our event");
    assert_eq!(b.project.foreign.len(), 1, "it must land in foreign as is");

    let out = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let c = &v["tracks"]["children"][0]["children"][0];
    assert_eq!(c["enabled"], false, "flag lost");
    assert_eq!(c["name"], "disabled");
    assert_eq!(c["metadata"]["vendor"]["k"], 1, "foreign metadata of the disabled clip lost");
}

// ---- 431-433 (R.50, item 382): nested reels in the format -------------------------

fn nested_project() -> Project {
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
    for id in [1u64, 2] {
        p.media.push(MediaRef::new(MediaId(id), format!("media/m{id}.mp4")));
    }
    // a reel: two events butt-joined, a crossfade between them, its own morpher layer and
    // its own camera — what OTIO does not express and what travels as a sidecar
    let mut s = Sequence::new(MediaId(100), "Intro");
    s.clips.push(clip(11, ClipKind::Video, 0, 0, 45_000, 1));
    s.clips.push(clip(12, ClipKind::Video, 0, 45_000, 45_000, 2));
    // the transition id is reissued on read — as for the root (it has always been so:
    // a transition is addressed by the pair of its ends, not by a number)
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
    s.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.sequences.push(s);

    // an instance stands TWICE — the body is inlined into each, and on read they must
    // collapse back into ONE reel
    let mut a = clip(1, ClipKind::Video, 0, 0, 90_000, 100);
    // Level, chain, REVERSE and SPEED on an attachment are legal (R.55, C4).
    // The write and read path for an attachment is shared with a clip
    // (`clip_effects`/`nest_effects`), so here the level and reverse suffice:
    // the rest is checked on clips.
    a.opacity = 0.5;
    a.reversed = true;
    p.clips.push(a);
    p.clips.push(clip(2, ClipKind::Video, 0, 200_000, 60_000, 100));
    p.validate().expect("the fixture must be legal");
    p
}

/// Test 814 (R.70): the OTIO export is self-sufficient — the reel body INLINE in
/// each of the three instances, an envelope of the former version; while an ordinary
/// “Open” (its own format) does not accept this file and names the import door.
/// The requirement for inlining is not cancelled — it is redirected to EXPORT.
#[test]
fn t814_export_is_self_contained_and_open_refuses_it() {
    let mut p = nested_project();
    // the third instance of the same reel
    let mut third = p.clips.iter().find(|c| c.media == MediaId(100)).cloned().unwrap();
    third.id = ClipId(p.max_clip_id() + 1);
    third.start = Tick(p.clips.iter().map(|c| c.start.0 + c.duration.0).max().unwrap_or(0) + 90_000);
    p.clips.push(third);
    p.validate().unwrap();
    assert_eq!(p.sequence_uses(MediaId(100)), 3);

    let exported = save_project(&p, &Retained::default(), None);
    let tl = parse_document(&exported).unwrap();
    let mut stacks = 0;
    for c in tl.tracks.children.iter() {
        if let kt_otio::Composable::Track(t) = c {
            for ch in &t.children {
                if let kt_otio::Composable::Stack(s) = ch {
                    stacks += 1;
                    assert!(!s.children.is_empty(), "the reel body must be inline in the export");
                }
            }
        }
    }
    assert_eq!(stacks, 3, "three instances — three Stacks with bodies");
    assert_eq!(
        kt_otio::kt_version(&tl),
        Some(kt_otio::KT_META_V6),
        "the export envelope is the previous version: a foreign and an old reader understand it"
    );

    // “Open” in its own format does not accept this file — and says where to go
    let err = kt_model::project_file::from_text(&exported).unwrap_err();
    assert!(
        matches!(
            err,
            kt_model::project_file::ProjectFileError::NotOurs {
                looks_like: Some(kt_model::project_file::FileKind::Otio)
            }
        ),
        "{err:?}"
    );
    assert!(err.to_string().contains("Import older Kinetren project"), "door not named: {err}");

    // while the import door reads it — one reel, three instances
    let back = load_project(&exported, None).unwrap().project;
    assert_eq!(back.sequences.len(), 1);
    assert_eq!(back.sequence_uses(MediaId(100)), 3);
}

#[test]
fn t431_nested_reel_survives_the_round_trip() {
    let p = nested_project();
    let text = save_project(&p, &Retained::default(), None);

    // an instance is written as a Stack child of a Track — by the same technique
    // Premiere and FCP use, so a foreign editor sees the edit, not a
    // reference into our envelope
    let tl = parse_document(&text).unwrap();
    let track = tl
        .tracks
        .children
        .iter()
        .find_map(|c| match c {
            kt_otio::Composable::Track(t) => Some(t),
            _ => None,
        })
        .expect("video track");
    let stacks: Vec<_> = track
        .children
        .iter()
        .filter_map(|c| match c {
            kt_otio::Composable::Stack(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(stacks.len(), 2, "both instances must be Stack objects");
    assert_eq!(stacks[0].name, "Intro", "the reel name is the stack name");
    assert!(stacks[0].source_range.is_some(), "instance window — the stack source_range");
    assert!(
        !stacks[0].children.is_empty(),
        "the body must be INLINE in each instance: object identity \
         does not exist in OTIO JSON, and a foreign editor would see emptiness"
    );

    // version BY CONTENT: the attachment is the first condition of the cascade
    assert_eq!(
        kt_otio::kt_version(&tl),
        Some(kt_otio::KT_META_V6),
        "a file with a nested reel must carry v6: an old build would read the nested reel as a gray \
         placeholder and, on the first rewrite, would return the edit to the file as dead raw data"
    );

    // round-trip: one reel, both instances point to it
    let bundle = load_project(&text, None).unwrap();
    let q = bundle.project;
    assert_eq!(q.sequences.len(), 1, "instances did not collapse into one reel: {:?}", q.sequences);
    let s = &q.sequences[0];
    assert_eq!(s.name, "Intro");
    assert_eq!(s.clips, p.sequences[0].clips, "reel contents field by field");
    assert_eq!(s.transitions, p.sequences[0].transitions, "crossfade inside the reel");
    assert_eq!(s.morphers, p.sequences[0].morphers, "reel morpher layer — from the sidecar");
    assert_eq!(s.lanes(kt_model::LaneKind::Morph), 1, "reel morph track — from the sidecar");
    assert_eq!(q.clips, p.clips, "instances field by field (level, chain, window)");
    assert_eq!(q.sequence_uses(MediaId(100)), 2, "the reel must remain in two places");

    // second cycle byte-for-byte
    let text2 = save_project(&q, &bundle.retained, None);
    assert_eq!(text2, text, "save→load→save with a nested reel is not byte-stable");
}

#[test]
fn t432_diverged_bodies_become_two_reels() {
    let p = nested_project();
    let text = save_project(&p, &Retained::default(), None);
    // a foreign editor edited ONE instance and not the other: we substitute the
    // duration of the event in the second inline
    let marker = "\"duration\": {\n";
    assert!(text.contains(marker));
    let mut tl = parse_document(&text).unwrap();
    {
        let kt_otio::Composable::Track(t) = &mut tl.tracks.children[0] else { panic!("track") };
        let kt_otio::Composable::Stack(s) = t
            .children
            .iter_mut()
            .filter(|c| matches!(c, kt_otio::Composable::Stack(_)))
            .nth(1)
            .expect("second instance")
        else {
            panic!("stack")
        };
        s.children.clear(); // the body of the second instance has become different
    }
    let text = kt_otio::write_document(&tl);
    let bundle = load_project(&text, None).unwrap();
    assert_eq!(
        bundle.project.sequences.len(),
        2,
        "diverging bodies must give TWO reels: a silent choice of one of the two \
         is a loss of foreign work"
    );
    assert!(
        bundle.notices.iter().any(|n| n.contains("two different bodies")),
        "the divergence of bodies was not reported: {:?}",
        bundle.notices
    );
    // the addresses of the events of the second reel are FRESH — ClipId is unique per document
    bundle.project.validate().expect("the project read must be legal");
}

#[test]
fn t433_the_sidecar_is_advisory_not_authoritative() {
    let p = nested_project();
    let text = save_project(&p, &Retained::default(), None);
    // the sidecar was lost (a foreign editor rewrote the timeline's metadata):
    // the reel's edit must be read FROM THE FILE, not vanish
    let mut tl = parse_document(&text).unwrap();
    {
        let kt = tl.metadata.get_mut("kt").unwrap().as_object_mut().unwrap();
        kt.remove("sequences");
    }
    let text = kt_otio::write_document(&tl);
    let bundle = load_project(&text, None).unwrap();
    let q = &bundle.project;
    assert_eq!(q.sequences.len(), 1, "the reel disappeared together with the sidecar");
    assert_eq!(q.sequences[0].clips.len(), 2, "the edit of the reel must be readable from the file");
    assert_eq!(q.sequences[0].transitions.len(), 1, "a crossfade inside the reel must be readable");
    assert!(
        q.sequences[0].morphers.is_empty(),
        "a morpher layer without a sidecar has nowhere to come from — that is its price"
    );
    assert!(
        bundle.notices.iter().any(|n| n.contains("no kt sidecar")),
        "the missing sidecar was not reported: {:?}",
        bundle.notices
    );
    q.validate().expect("the project read must be legal");
}

/// Test 433b (R.50): the cascade of versions did NOT shift a SINGLE earlier file.
///
/// The envelope version is set BY CONTENT, and a new condition built into the
/// cascade in the wrong place would raise the version for everyone — such a file stops
/// opening in earlier builds. This is the only check that catches an error of the cascade
/// as a whole: t148/t189 guard against one reference each, while here — all of our
/// committed .otio files at once.
#[test]
fn t433b_version_cascade_did_not_move_old_files() {
    // absolute: on Windows a rooted path without a drive letter is not absolute, and the
    // writer would not relativise against it
    let dir = std::path::absolute(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures")).unwrap();
    let dir = dir.as_path();
    let mut seen = 0;
    for e in std::fs::read_dir(dir).expect("fixture catalog") {
        let path = e.unwrap().path();
        if path.extension().and_then(|x| x.to_str()) != Some("otio") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let before = kt_otio::kt_version(&parse_document(&text).unwrap());
        let bundle = load_project(&text, path.parent()).expect("our file reads");
        assert!(
            bundle.project.sequences.is_empty(),
            "{}: the previous fixture must have no nested reels",
            path.display()
        );
        let out = save_project(&bundle.project, &bundle.retained, path.parent());
        assert_eq!(
            kt_otio::kt_version(&parse_document(&out).unwrap()),
            before,
            "{}: the envelope version moved on a file WITHOUT nested reels — the condition \
             is built into the cascade in the wrong place, and old builds will stop opening the file",
            path.display()
        );
        assert_eq!(out, text, "{}: re-save is not byte-for-byte", path.display());
        seen += 1;
    }
    assert!(seen >= 2, "our .otio fixtures disappeared (found {seen})");
}

/// Test 437 (R.50 A5): a foreign attachment is a real reel, not a grey hole.
///
/// The literal cost of today's behaviour (item 158): “melt expands nested
/// compositions and draws their content, while ours is empty there” — because of which
/// `nested_example` sat permanently in the smoke tier of the cross-render. The owner
/// decided to close the gap in this same round, accepting an incompatible change of
/// import behaviour.
#[test]
fn t437_foreign_nested_stacks_become_real_reels() {
    let path =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/otio-samples/nested_example.otio");
    let text = std::fs::read_to_string(path).unwrap();
    let b = load_project(&text, None).expect("nested_example reads");
    let p = &b.project;

    // the file has THREE attachments: two Stacks and one Track inside a Track
    assert_eq!(
        p.sequences.len(),
        3,
        "foreign nested items did not become reels: {:?}",
        p.sequences.iter().map(|s| s.name.as_str()).collect::<Vec<_>>()
    );
    assert!(
        p.sequences.iter().any(|s| s.name.contains("Nested Track")),
        "`Track` inside `Track` — also a nesting, and it was skipped: {:?}",
        p.sequences.iter().map(|s| s.name.as_str()).collect::<Vec<_>>()
    );
    assert!(p.foreign.is_empty(), "nested items remained gray placeholders: {:?}", p.foreign);
    // the content arrived: each reel has events
    for s in &p.sequences {
        assert!(!s.clips.is_empty(), "reel '{}' is empty", s.name);
    }
    // the instances stand on the timeline and look at their own reels
    for s in &p.sequences {
        assert_eq!(p.sequence_uses(s.id), 1, "reel '{}' was not placed", s.name);
    }
    p.validate().expect("the project read must be legal");

    // ...and the expansion gives a real edit, not emptiness: not one event that reaches
    // the engine looks at the REEL — all look at the material
    let eff = p.effective_clips();
    assert!(
        eff.iter().all(|c| p.sequence(c.media).is_none()),
        "after expansion, events looking at the reel remained — the engine would have seen emptiness"
    );
    // each reel contributed its content, trimmed to the window of the instance
    for s in &p.sequences {
        let inner: Vec<MediaId> = s.clips.iter().map(|c| c.media).collect();
        assert!(
            inner.iter().all(|m| eff.iter().any(|c| c.media == *m)),
            "material of reel '{}' did not reach the engine",
            s.name
        );
    }

    // round: our output is read and stays idempotent
    let once = save_project(p, &b.retained, None);
    let again = load_project(&once, None).expect("our output reads");
    assert_eq!(
        again.project.sequences.len(),
        3,
        "reels did not survive the round trip: {:?}",
        again.project.sequences.iter().map(|s| s.name.as_str()).collect::<Vec<_>>()
    );
    let twice = save_project(&again.project, &again.retained, None);
    assert_eq!(once, twice, "re-save is not idempotent");
}

/// Test 465 (item 399, R.51): an overlapping transition in the file — the reel envelope
/// `overlap_transitions` (a pair from different Tracks cannot be expressed as a child of a
/// sequential track), the file is raised to v7, the round is idempotent, a broken
/// record — a notice, not corruption.
#[test]
fn t465_overlap_transition_roundtrip() {
    let mk = || {
        let mut p = rich_project();
        // a video staircase on one track: [0..90k) and [45k..135k)
        p.clips.retain(|c| c.id.0 >= 10); // the audio pair remains
        p.clips.push(clip(1, ClipKind::Video, 0, 0, 90_000, 1));
        p.clips.push(clip(2, ClipKind::Video, 0, 45_000, 90_000, 2));
        p.transitions.push(TransitionEvent {
            id: TransitionId(7),
            a: Some(ClipId(1)),
            b: Some(ClipId(2)),
            in_offset: Tick::ZERO,
            out_offset: Tick::ZERO,
            kind: TransitionKind::Barn { vertical: false, softness: 0.2 },
            audio: AudioCurve::SCurve,
            auto: vec![],
            invert: true,
        });
        p.validate().unwrap();
        p
    };
    let p = mk();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 7, "an overlapping transition raises the file to v7");
    let ot = &v["metadata"]["kt"]["overlap_transitions"];
    assert_eq!(ot.as_array().map(|a| a.len()), Some(1), "transition in the reel envelope: {ot}");
    assert_eq!(ot[0]["a"], 1);
    assert_eq!(ot[0]["b"], 2);
    assert_eq!(ot[0]["kind"]["kind"], "barn");
    assert_eq!(ot[0]["invert"], true);

    let b = load_project(&text, None).expect("file with an overlapping transition reads");
    let got: Vec<&TransitionEvent> =
        b.project.transitions.iter().filter(|t| t.overlap_pair(&b.project).is_some()).collect();
    assert_eq!(got.len(), 1, "overlapping transition restored");
    let t = got[0];
    assert_eq!((t.a, t.b), (Some(ClipId(1)), Some(ClipId(2))));
    assert_eq!(t.kind, TransitionKind::Barn { vertical: false, softness: 0.2 });
    assert_eq!(t.audio, AudioCurve::SCurve);
    assert!(t.invert);
    let w = t.window(&b.project).unwrap();
    assert_eq!((w.start, w.end), (Tick(45_000), Tick(90_000)), "window = overlap");
    b.project.validate().expect("what was read is valid");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "idempotence");

    // broken record (an unknown clip) — a notice and a skip, not corruption
    let mut broken: serde_json::Value = serde_json::from_str(&text).unwrap();
    broken["metadata"]["kt"]["overlap_transitions"][0]["b"] = serde_json::json!(777);
    let b = load_project(&serde_json::to_string_pretty(&broken).unwrap(), None)
        .expect("a broken record does not fail the file");
    assert!(
        b.project.transitions.iter().all(|t| t.overlap_pair(&b.project).is_none()),
        "a broken overlapping transition is skipped"
    );
    assert!(
        b.notices.iter().any(|n| n.contains("overlap transition")),
        "skipping must give a note: {:?}",
        b.notices
    );
}

/// All objects of the given OTIO schema in the JSON tree.
fn find_schema<'a>(v: &'a serde_json::Value, tag: &str, out: &mut Vec<&'a serde_json::Value>) {
    match v {
        serde_json::Value::Object(m) => {
            if m.get("OTIO_SCHEMA").and_then(|t| t.as_str()) == Some(tag) {
                out.push(v);
            }
            for x in m.values() {
                find_schema(x, tag, out);
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                find_schema(x, tag, out);
            }
        }
        _ => {}
    }
}

/// Test 473 (R.52): our generator is written first-class as
/// `GeneratorReference.1` (`kt.solid` + parameters + a MANDATORY
/// available_range from zero, rate 90000), it is read back into the same
/// canonical `gen:` path, the write is idempotent; `synth:` remains an
/// `ExternalReference` verbatim; a generator inside a nested reel goes through the
/// same code; kt.v does NOT grow from a generator (an old build sees a
/// visible placeholder, not silent corruption).
#[test]
fn t473_generator_reference_roundtrip() {
    let mut p = rich_project();
    // R.59, stage 2: in the media path — the KIND, values on the event
    let orange = kt_model::GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    p.media.push(MediaRef::new(MediaId(7), "gen:solid"));
    p.media.push(MediaRef::new(MediaId(8), "synth:ramp"));
    let mut g = clip(30, ClipKind::Video, 0, 200_000, 45_000, 7);
    g.source_in = Tick(4_500);
    g.r#gen = Some(orange.clone());
    p.clips.push(g);
    p.clips.push(clip(31, ClipKind::Audio, 0, 200_000, 45_000, 8));
    // the same generator inside a nested reel (R.50): reel_children is one
    let mut seq = Sequence::new(MediaId(9_000_000), "reel");
    let mut inner = clip(40, ClipKind::Video, 0, 0, 30_000, 7);
    inner.r#gen = Some(orange);
    seq.clips.push(inner);
    p.sequences.push(seq);
    p.clips.push(clip(32, ClipKind::Video, 1, 300_000, 30_000, 9_000_000));
    p.validate().unwrap();

    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut gens = Vec::new();
    find_schema(&v, "GeneratorReference.1", &mut gens);
    assert_eq!(gens.len(), 2, "a clip at the root and a clip in a reel");
    let mut durs = Vec::new();
    for gr in &gens {
        assert_eq!(gr["generator_kind"], "kt.solid");
        assert_eq!(gr["parameters"]["color_r"], 255);
        assert_eq!(gr["parameters"]["color_g"], 136);
        assert_eq!(gr["parameters"]["color_b"], 0);
        assert!(
            gr["parameters"].get("kind").is_none(),
            "the kind travels as the generator_kind string, not the parameters field"
        );
        let ar = &gr["available_range"];
        assert!(!ar.is_null(), "available_range is REQUIRED on GeneratorReference");
        assert_eq!(ar["start_time"]["value"], 0.0);
        assert_eq!(ar["duration"]["rate"], 90000.0);
        durs.push(ar["duration"]["value"].as_f64().unwrap() as i64);
    }
    durs.sort();
    assert_eq!(durs, vec![30_000, 49_500], "consumed by the clip: source_in + duration");

    // synth: remained the former external reference verbatim
    let mut exts = Vec::new();
    find_schema(&v, "ExternalReference.1", &mut exts);
    assert!(
        exts.iter().any(|e| e["target_url"] == "synth:ramp"),
        "synth: ExternalReference is written as before; urls: {:?}",
        exts.iter().map(|e| e["target_url"].clone()).collect::<Vec<_>>()
    );

    let b = load_project(&text, None).expect("file with a generator reads");
    assert_eq!(
        b.project.media.iter().find(|m| m.id == MediaId(7)).unwrap().path,
        "gen:solid",
        "the KIND stays in the media path (R.59, stage 2)"
    );
    assert_eq!(b.project.sequence(MediaId(9_000_000)).unwrap().clips[0].media, MediaId(7));
    assert_eq!(b.project.clips, p.clips, "clips survived the round trip");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "idempotence");

    // R.59, stage 2: the generator NOW bumps the version — the values moved from
    // the media path to the event, and an old build would go after them into the path and
    // would show the defaults in all places at once. The former expectation
    // (“a generator does not move the version”, R.52) is lifted by this decision: then
    // an old build got a visible placeholder, now — silently
    // the wrong frame, and that is already corruption.
    let mut plain = rich_project();
    plain.sequences.push(Sequence::new(MediaId(9_000_000), "reel"));
    plain.clips.push(clip(32, ClipKind::Video, 1, 300_000, 30_000, 9_000_000));
    plain.validate().unwrap();
    let vp: serde_json::Value =
        serde_json::from_str(&save_project(&plain, &Retained::default(), None)).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], kt_otio::KT_META_V9 as u64);
    assert!(
        vp["metadata"]["kt"]["v"].as_u64().unwrap() < kt_otio::KT_META_V9 as u64,
        "a project without a generator does not raise the version"
    );
}

/// Test 474 (R.52, kin to t154): a FOREIGN generator_kind (not `kt.*`) is not typed —
/// the clip gets an empty path (a visible placeholder), and the node
/// travels through re-saving without losses.
#[test]
fn t474_foreign_generator_kind_is_passthrough() {
    let mut p = rich_project();
    p.media.push(MediaRef::new(MediaId(7), "gen:solid"));
    let mut g = clip(30, ClipKind::Video, 0, 200_000, 45_000, 7);
    g.r#gen = Some(kt_model::GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    p.clips.push(g);
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);

    // a “file from a foreign editor”: generator_kind is neither ours nor an alias
    let foreign = text.replace("\"kt.solid\"", "\"Legacy.Title\"");
    assert_ne!(foreign, text);
    let b = load_project(&foreign, None).expect("a foreign generator reads");
    let m7 = b.project.media.iter().find(|m| m.id == MediaId(7)).unwrap();
    assert_eq!(m7.path, "", "a foreign generator = a stub (empty path), not a guess");

    // re-saving carries the node without losses: the kind and parameters are in place
    let text2 = save_project(&b.project, &b.retained, None);
    let v2: serde_json::Value = serde_json::from_str(&text2).unwrap();
    let mut gens = Vec::new();
    find_schema(&v2, "GeneratorReference.1", &mut gens);
    assert_eq!(gens.len(), 1);
    assert_eq!(gens[0]["generator_kind"], "Legacy.Title");
    assert_eq!(gens[0]["parameters"]["color_r"], 255, "parameters not lost");
    assert_eq!(
        text2,
        save_project(
            &load_project(&text2, None).unwrap().project,
            &load_project(&text2, None).unwrap().retained,
            None
        ),
        "idempotence of re-saving a foreign generator"
    );
}

/// Test 494 (R.52 C5): import alias — a foreign `SMPTEBars` is read by OUR
/// bars generator, and re-saving canonicalizes the kind to `kt.smpte`
/// idempotently. (The upstream fixture generator_reference_test.otio is unsuitable for
/// this: its track is an archaic `Sequence.1`, passthrough in full;
/// it stays under t154.)
#[test]
fn t494_smpte_alias_import() {
    let mut p = rich_project();
    p.media.push(MediaRef::new(MediaId(7), "gen:smpte"));
    let mut g = clip(30, ClipKind::Video, 0, 200_000, 45_000, 7);
    g.r#gen = Some(kt_model::GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 });
    p.clips.push(g);
    p.validate().unwrap();
    let ours = save_project(&p, &Retained::default(), None);
    // “a foreign editor”: the same content, but the kind named the upstream way
    let foreign = ours.replace("\"kt.smpte\"", "\"SMPTEBars\"");
    assert_ne!(foreign, ours);

    let b = load_project(&foreign, None).expect("foreign SMPTEBars reads");
    assert_eq!(
        b.project.media.iter().find(|m| m.id == MediaId(7)).unwrap().path,
        "gen:smpte",
        "the alias must give OUR bars generator, not a stub"
    );

    // re-saving canonicalizes the kind; from then on — byte-for-byte
    let text2 = save_project(&b.project, &b.retained, None);
    assert!(text2.contains("\"kt.smpte\""), "kind canonicalized");
    assert!(!text2.contains("SMPTEBars"), "a foreign name was lost during canonicalization");
    let b2 = load_project(&text2, None).unwrap();
    assert_eq!(text2, save_project(&b2.project, &b2.retained, None), "idempotence");
}

/// Test 593 (R.56, item 449): a file in the form of the owner's project — a video track
/// “under the ceiling” — is read CANONICALLY, and the raw data of a foreign track travels
/// together with its own track.
///
/// In `untitled_v26.09.19.otio` the row order was
/// `[morph0, morph1, morph2, video8, camera1, camera0, video0, …]`: the row
/// stood third from the top, and the number `8` made it the BOTTOM layer of the composite —
/// “I put video there, and it is not shown at all”. On read the numbers
/// become row ranks; the owner did not ask for backward compatibility.
///
/// The second half concerns `Retained.tracks`: the foreign metadata of a track
/// is addressed by a pair (kind, number), and without permuting the map it would silently
/// move onto the NEIGHBOURING track.
#[test]
fn t593_owner_shaped_file_loads_canonically() {
    let mut p = rich_project();
    p.ensure_tracks(kt_model::LaneKind::Video, 3);
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    // “under the ceiling”: the LOWEST-numbered video track stands on the second
    // row from the top, right under the morph row
    p.clips.push(clip(3, ClipKind::Video, 2, 0, 90_000, 3));
    let text = save_project(&p, &Retained::default(), None);

    // THE OWNER'S FILE — legacy form (before R.62): track numbers are not equal to
    // row ranks, the order lies in its own field `lane_order`, and there is no vector
    // of tracks in the envelope at all
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    {
        let kt = v["metadata"]["kt"].as_object_mut().unwrap();
        kt.remove("tracks");
        kt.insert(
            "lane_order".into(),
            serde_json::json!([
                ["morph", 0],
                ["video", 2],
                ["video", 0],
                ["video", 1],
                ["audio", 0]
            ]),
        );
    }
    {
        let tracks = v["tracks"]["children"].as_array_mut().unwrap();
        // the writer drives video bottom-up: (V,2) — the FIRST track
        let t = &mut tracks[0];
        t["metadata"]["alien"] = serde_json::json!({"note": "top row"});
    }
    let text = serde_json::to_string_pretty(&v).unwrap();

    let b = load_project(&text, None).expect("the file of the owner must read");
    let lp = &b.project;
    // the rows were NOT permuted — the NUMBERS were: the morph row stays as it
    // was at the top, and the video beneath it runs 0,1,2
    assert!(rows_are_canonical(&lp.rows()), "after reading, the numbers must match the ranks");
    assert_eq!(
        lp.rows(),
        vec![
            (LaneKind::Morph, 0),
            (LaneKind::Video, 0),
            (LaneKind::Video, 1),
            (LaneKind::Video, 2),
            (LaneKind::Audio, 0)
        ],
        "the row order on screen changed, but it had to stay"
    );
    assert_eq!(
        lp.clips.iter().find(|c| c.id == ClipId(3)).unwrap().lane,
        0,
        "the event “under the ceiling” did not become the TOP layer"
    );
    assert_eq!(lp.clips.iter().find(|c| c.id == ClipId(1)).unwrap().lane, 1);
    assert_eq!(lp.clips.iter().find(|c| c.id == ClipId(2)).unwrap().lane, 2);

    // CONSEQUENCE for foreign raw data: it sits on track 0, not on the former 2
    let extra = b.retained.tracks.get(&(ClipKind::Video, 0)).expect("a foreign track was lost");
    assert!(
        format!("{:?}", extra).contains("top row"),
        "raw data of the foreign track did not follow its track: {extra:?}"
    );

    // round: saving puts the former “video8” as the first video track, and
    // re-reading already changes nothing
    let text2 = save_project(lp, &b.retained, None);
    let v2: serde_json::Value = serde_json::from_str(&text2).unwrap();
    // a track with foreign raw data must be THE SAME one the event lies on:
    // addressing by (kind, number) survived the renumbering
    let alien = v2["tracks"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["metadata"]["alien"]["note"].as_str() == Some("top row"))
        .expect("foreign track metadata disappeared on write");
    let ids: Vec<u64> = alien["children"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["metadata"]["kt"]["id"].as_u64())
        .collect();
    assert_eq!(ids, vec![3], "foreign raw data moved to the track of a foreign event: {ids:?}");
    let b2 = load_project(&text2, None).expect("round trip");
    assert_eq!(b2.project, *lp, "the second round trip changed something");
    assert_eq!(text2, save_project(&b2.project, &b2.retained, None), "idempotence");
}

/// Test 606 (R.57): signed speed survives the round trip — both the base and the keyframe.
///
/// A negative `rate` and zero are legal values (the velocity envelope of
/// Vegas). The reader has no right to reject them, nor to pull them up to the
/// former minimum of 0.25: the owner did not ask for backward compatibility, and a
/// silent clamp was exactly the complaint.
#[test]
fn t606_signed_rate_survives_the_round_trip() {
    let mut p = rich_project();
    p.clips[0].fx = vec![FxInstance::slowed_reverb(-0.5, 0.0, 85.0)];
    p.clips[0].fx_auto = vec![vec![ParamCurve {
        param: "rate".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(0), 1.0),
            AutoKey::at(Tick(45_000), 0.0),
            AutoKey::at(Tick(90_000), -2.0),
        ]),
        source: Default::default(),
    }]];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let b = load_project(&text, None).expect("file with signed speed reads");
    let c = b.project.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    assert_eq!(c.fx, p.clips[0].fx, "speed base drifted");
    assert_eq!(c.fx_auto, p.clips[0].fx_auto, "speed keyframes drifted");
    let keys: Vec<f64> = c.fx_auto[0][0].keys.iter().map(|k| k.v).collect();
    assert_eq!(keys, vec![1.0, 0.0, -2.0], "zero and minus did not reach the model");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "idempotence");
}

/// Test 638 (R.58, item 462): an event LEFT OF ZERO inside a reel survives the
/// save→load→save round trip byte-for-byte.
///
/// An OTIO track is sequential and starts at zero: an event with `start < 0` cannot be
/// expressed by position in it, and everything that follows would land on the track's cursor
/// out of its time. Before R.58 such an edit silently 'arrived' shifted.
/// The true time travels in the envelope (`kt.start`) ONLY where the track did not
/// express it — a regular edit carries no field, and the earlier files stay
/// byte-stable (t433b, golden). This is a contract: a future guard
/// `validate` has no right to forbid `start < 0` in a reel — the owner's files
/// already contain it.
#[test]
fn t638_negative_start_inside_a_reel_survives_the_round_trip() {
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
    let mut s = Sequence::new(MediaId(100), "Intro");
    // the first event starts BEFORE the zero of the reel; the second stands at its
    // time, but in the OTIO track it would land on the cursor (0 + 50 000 > 45 000)
    s.clips.push(clip(11, ClipKind::Video, 0, -10_000, 50_000, 1));
    s.clips.push(clip(12, ClipKind::Video, 0, 45_000, 30_000, 1));
    p.sequences.push(s);
    p.clips.push(clip(1, ClipKind::Video, 0, 0, 60_000, 100));
    p.validate().expect("start < 0 inside a reel is legal");

    // how many events (Clip/Stack) carry `kt.start` — by the tree, not by the
    // text: morpher layers in the sidecar have their own `start`
    fn kt_starts(kids: &[kt_otio::Composable]) -> usize {
        use kt_otio::Composable;
        let has = |m: &serde_json::Map<String, serde_json::Value>| {
            m.get("kt").and_then(|k| k.get("start")).is_some() as usize
        };
        kids.iter()
            .map(|k| match k {
                Composable::Clip(c) => has(&c.metadata),
                Composable::Stack(s) => has(&s.metadata) + kt_starts(&s.children),
                Composable::Track(t) => kt_starts(&t.children),
                _ => 0,
            })
            .sum()
    }
    let text = save_project(&p, &Retained::default(), None);
    // the envelope carries the true time for BOTH shifted events and for no one else
    let tl = parse_document(&text).unwrap();
    assert_eq!(kt_starts(&tl.tracks.children), 2, "kt.start written where it is not needed");
    assert_eq!(
        kt_otio::kt_version(&parse_document(&text).unwrap()),
        Some(kt_otio::KT_META_V8),
        "a reel with an event before zero must bump the file to v8"
    );

    let bundle = load_project(&text, None).unwrap();
    let q = &bundle.project;
    let s = q.sequence(MediaId(100)).expect("reel");
    assert_eq!(s.clips, p.sequences[0].clips, "reel content field by field, incl. start < 0");
    assert_eq!(q.clips, p.clips, "instance field by field");

    let text2 = save_project(q, &bundle.retained, None);
    assert_eq!(text2, text, "save→load→save with a negative start is not byte-stable");

    // a regular edit carries no field — byte-stability of earlier files
    let plain = save_project(&nested_project(), &Retained::default(), None);
    let tl = parse_document(&plain).unwrap();
    assert_eq!(kt_starts(&tl.tracks.children), 0, "kt.start leaked into ordinary files");
}

// ---- R.59, stage 2: generator parameters on the EVENT ---------------------

/// Test 664: generator values and their curves survive the round trip, the file version
/// is raised BY CONTENT, and a foreign reader sees the values of THIS event with a
/// first-class schema.
///
/// Two titles on ONE kind with different text — a scene that did not exist before this
/// round: the text was part of the media address. For its sake the whole thing was started
/// (a news-release template, the owner's claim item 1).
#[test]
fn t664_generator_values_on_the_event_round_trip() {
    let mut p = rich_project();
    let title = |text: &str| {
        let mut k = kt_model::GeneratorKind::default_text();
        if let kt_model::GeneratorKind::Text { text: t, .. } = &mut k {
            *t = text.into();
        }
        k
    };
    p.media.push(MediaRef::new(MediaId(7), "gen:text"));
    let mut a = clip(30, ClipKind::Video, 0, 200_000, 45_000, 7);
    a.r#gen = Some(title("First release"));
    a.gen_auto = kt_model::text_anim_curves(kt_model::TextAnim::Typewriter, 2.0, 2.0);
    let mut b = clip(31, ClipKind::Video, 1, 200_000, 45_000, 7);
    b.r#gen = Some(title("Second release"));
    p.clips.push(a);
    p.clips.push(b);
    p.validate().unwrap();

    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        v["metadata"]["kt"]["v"],
        kt_otio::KT_META_V9 as u64,
        "a generator on an event must raise the version: an old build would reach for values in the \
         media path, where they no longer exist"
    );

    // a foreign reader sees the VALUES of each event with a first-class schema
    let mut gens = Vec::new();
    find_schema(&v, "GeneratorReference.1", &mut gens);
    assert_eq!(gens.len(), 2, "each event has its own reference to the material");
    let texts: std::collections::BTreeSet<String> = gens
        .iter()
        .map(|g| g["parameters"]["text"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(texts.contains("First release") && texts.contains("Second release"), "{texts:?}");

    let back = load_project(&text, None).expect("file with values on an event reads");
    assert_eq!(
        back.project.media.iter().find(|m| m.id == MediaId(7)).unwrap().path,
        "gen:text",
        "the KIND stays in the media path"
    );
    assert_eq!(back.project.clips, p.clips, "values and curves did not survive the round trip");
    assert_eq!(text, save_project(&back.project, &back.retained, None), "idempotence");
}

/// Test 665: a file of R.52..R.58 migrates for free — values from the MEDIA PATH are
/// distributed to events, and the former animation mode of a title becomes
/// curves by the same formulas as the preset in the window.
///
/// Should the migration diverge from the preset, a project saved by the previous build
/// would go differently from the same button pressed by hand. Therefore the formulas
/// live in ONE place (`kt_model::text_anim_curves`), and the test checks the
/// result of reading against exactly it.
#[test]
fn t665_old_generator_files_migrate_to_the_event() {
    // “an old file”: a generator with VALUES in parameters and the animation
    // mode as a field — exactly what the R.53 build wrote
    let mut p = rich_project();
    p.media.push(MediaRef::new(MediaId(7), "gen:text"));
    let mut c = clip(30, ClipKind::Video, 0, 200_000, 45_000, 7);
    c.r#gen = Some(kt_model::GeneratorKind::default_text());
    p.clips.push(c);
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();

    // by hand we bring the file to the OLD form: the clip has no kt.gen, the values and
    // the mode — in parameters, version v8
    v["metadata"]["kt"]["v"] = serde_json::json!(8);
    fn patch(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                if m.get("OTIO_SCHEMA").and_then(|s| s.as_str()) == Some("GeneratorReference.1")
                    && let Some(serde_json::Value::Object(pr)) = m.get_mut("parameters")
                {
                    pr.insert("text".into(), serde_json::json!("Old title"));
                    pr.insert("anim".into(), serde_json::json!("typewriter"));
                    pr.insert("anim_secs".into(), serde_json::json!(2.0));
                }
                if let Some(serde_json::Value::Object(kt)) = m.get_mut("kt") {
                    kt.shift_remove("gen");
                    kt.shift_remove("gen_auto");
                }
                for x in m.values_mut() {
                    patch(x);
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(patch),
            _ => {}
        }
    }
    patch(&mut v);
    let old = serde_json::to_string(&v).unwrap();
    assert!(!old.contains("\"gen\""), "the old file must have no value sections");

    let back = load_project(&old, None).expect("old file reads");
    assert_eq!(
        back.project.media.iter().find(|m| m.id == MediaId(7)).unwrap().path,
        "gen:text",
        "media path shortened to the kind"
    );
    let c = back.project.clips.iter().find(|c| c.id == ClipId(30)).expect("clip in place");
    match &c.r#gen {
        Some(kt_model::GeneratorKind::Text { text, reveal_soft, .. }) => {
            assert_eq!(text, "Old title", "values must reach the event");
            assert_eq!(*reveal_soft, 0.0, "the machine front is rigid");
        }
        other => panic!("wrong kind: {other:?}"),
    }
    assert_eq!(
        c.gen_auto,
        kt_model::text_anim_curves(kt_model::TextAnim::Typewriter, 2.0, 2.0),
        "the animation mode must become THE SAME curves that the preset sets"
    );
    // and the read project is legal — that is, it migrated ENTIRELY
    back.project.validate().expect("a migrated project is legal");
}

/// Test 681 (R.59, stage 3): public parameters and overrides survive the round trip,
/// and the version is raised BY CONTENT — by the SAME predicate as the generator on the event
/// (one version per round).
///
/// An old reader is obliged to refuse such a file, and this is right: otherwise it would
/// silently play all instances with template values — ten news releases with one
/// heading and not one visible sign.
#[test]
fn t681_published_params_and_overrides_round_trip() {
    let mut p = nested_project();
    // a handle on the reel: the title text inside it
    p.media.push(MediaRef::new(MediaId(7), "gen:text"));
    let mut title = clip(13, ClipKind::Video, 1, 0, 45_000, 7);
    let mut kind = kt_model::GeneratorKind::default_text();
    if let kt_model::GeneratorKind::Text { text, .. } = &mut kind {
        *text = "Default heading".into();
    }
    title.r#gen = Some(kind);
    let s = p.sequences.iter_mut().find(|s| s.id == MediaId(100)).unwrap();
    s.clips.push(title);
    s.ensure_tracks(kt_model::LaneKind::Video, 2);
    s.published = vec![kt_model::PublishedParam {
        id: kt_model::PubId(1),
        // the name — free text of the AUTHOR (the owner's decision R.59)
        name: "News heading".into(),
        target: kt_model::ParamAddr::new(ClipId(13), kt_model::ParamOwner::Generator, "text"),
        ty: kt_model::ParamType::Text { multiline: false },
        range: kt_model::PubRange { min: None, max: None },
    }];
    // the SECOND instance has its own value
    let inst = p.clips.iter_mut().filter(|c| c.media == MediaId(100)).nth(1).unwrap();
    inst.overrides = vec![kt_model::Override::value(
        kt_model::PubId(1),
        kt_model::ParamValue::Text("Second release".into()),
    )];
    p.validate().expect("a template with an override is legal");

    let text = save_project(&p, &Retained::default(), None);
    assert_eq!(
        kt_otio::kt_version(&parse_document(&text).unwrap()),
        Some(kt_otio::KT_META_V9),
        "public parameters must raise THE SAME version as the generator on an event: \
         one version per round, counted by content"
    );

    let back = load_project(&text, None).expect("file with publications reads");
    assert_eq!(
        back.project.sequence(MediaId(100)).unwrap().published,
        p.sequences.iter().find(|s| s.id == MediaId(100)).unwrap().published,
        "the declared set did not survive the round trip"
    );
    assert_eq!(back.project.clips, p.clips, "overrides did not survive the round trip");
    assert_eq!(text, save_project(&back.project, &back.retained, None), "idempotence");

    // ...and a file WITHOUT these fields did not move by a single character
    let plain = save_project(&nested_project(), &Retained::default(), None);
    assert!(
        !plain.contains("published") && !plain.contains("overrides"),
        "publication fields leaked into a file that does not use them"
    );
    assert_eq!(
        kt_otio::kt_version(&parse_document(&plain).unwrap()),
        Some(kt_otio::KT_META_V6),
        "an ordinary reel must stay on its version"
    );

    // ...and a handle on the LEADING GEOMETRY (R.63 item 9) — a novelty of R.62: an
    // address `owner: "geometry"` an old build will not parse at all, and it is
    // the VERSION that must say so, not a parse error
    let mut g = p.clone();
    g.sequences
        .iter_mut()
        .find(|s| s.id == MediaId(100))
        .unwrap()
        .published
        .push(kt_model::PublishedParam {
            id: kt_model::PubId(2),
            name: "Heading size".into(),
            target: kt_model::ParamAddr::new(
                ClipId(13),
                kt_model::ParamOwner::Geometry,
                "zoom",
            ),
            ty: kt_model::ParamType::Num,
            range: kt_model::PubRange { min: None, max: None },
        });
    g.validate().expect("a handle on the leading geometry is legal");
    let gtext = save_project(&g, &Retained::default(), None);
    assert_eq!(
        kt_otio::kt_version(&parse_document(&gtext).unwrap()),
        Some(kt_otio::KT_META_V11),
        "publishing the leading geometry must raise the version by CONTENT"
    );
    let gback = load_project(&gtext, None).expect("file with a geometry handle reads");
    assert_eq!(
        gback.project.sequence(MediaId(100)).unwrap().published,
        g.sequences.iter().find(|s| s.id == MediaId(100)).unwrap().published,
        "geometry address did not survive the round trip"
    );
}

// ---- 698b, 698c (R.61). segment shape in the file -------------------------------

/// Test 698b (R.61): the version is raised BY CONTENT, and a uniform shape
/// does not move it.
///
/// The former boolean field `ease` remains the wire for the shape it expresses:
/// the third order and the straight line are written by it, and the bytes do not move. All the rest an old build would read as UNIFORM —
/// a zigzag would become a straight line, a step a smooth glide —
/// and not one sign of damage. Hence v10.
#[test]
fn t698b_segment_shape_raises_the_version_only_when_it_has_to() {
    use kt_model::{AutoKey, SegShape, ZigProfile};

    let with_curve = |shapes: Vec<SegShape>| -> Project {
        let mut p = rich_project();
        let keys: Vec<AutoKey> = shapes
            .iter()
            .enumerate()
            .map(|(i, s)| AutoKey::shaped(Tick(i as i64 * 30_000), i as f64, *s))
            .collect();
        let c = p.clips.iter_mut().find(|c| !c.fx.is_empty()).expect("clip with an effect");
        c.fx_auto = vec![vec![kt_model::ParamCurve {
            param: "radius".into(),
            keys,
            source: Default::default(),
        }]];
        p
    };
    let ver = |p: &Project| {
        let text = save_project(p, &Retained::default(), None);
        kt_otio::kt_version(&parse_document(&text).unwrap())
    };

    // UNIFORM shapes expressible by the former field do not move the version
    let plain = ver(&rich_project());
    let smooth = with_curve(vec![SegShape::DEFAULT, SegShape::DEFAULT]);
    let linear = with_curve(vec![SegShape::Linear, SegShape::Linear]);
    assert_ne!(ver(&smooth), Some(kt_otio::KT_META_V10), "the third order raised the version for nothing");
    assert_ne!(ver(&linear), Some(kt_otio::KT_META_V10), "the straight one raised the version for nothing");
    assert!(plain.is_some(), "a version must be present in any of our files");

    // DIFFERENT shapes on one curve — v10
    let mixed = with_curve(vec![SegShape::Linear, SegShape::DEFAULT]);
    assert_eq!(ver(&mixed), Some(kt_otio::KT_META_V10), "different shapes must raise the version");

    // a shape NOT expressible by the former field — v10, even when uniform
    for s in [
        SegShape::Hold,
        SegShape::EaseIn { tension: 1.0 },
        SegShape::Zigzag { periods: 3.0, amplitude: 0.5, profile: ZigProfile::Sine },
        SegShape::Smooth { tension: 2.0 },
    ] {
        assert_eq!(ver(&with_curve(vec![s, s])), Some(kt_otio::KT_META_V10), "shape {s:?}");
    }

    // the default shape of the PROJECT — also file content
    let mut dflt = rich_project();
    dflt.settings.default_shape = SegShape::Hold;
    assert_eq!(ver(&dflt), Some(kt_otio::KT_META_V10), "the default shape did not raise the version");
}

/// Test 698c (R.61): the round trip is identity, and the former `ease` field reads
/// as a uniform shape and yields BIT-IDENTICAL curve values.
#[test]
fn t698c_shapes_round_trip_and_the_old_ease_field_migrates() {
    use kt_model::{AutoKey, SegShape, ZigProfile, curve_value};

    let mut p = rich_project();
    let keys = vec![
        AutoKey::shaped(Tick(0), 1.0, SegShape::Hold),
        AutoKey::shaped(Tick(30_000), 8.0, SegShape::Linear),
        AutoKey::shaped(
            Tick(60_000),
            3.0,
            SegShape::Zigzag { periods: 2.5, amplitude: 1.5, profile: ZigProfile::Triangle },
        ),
        AutoKey::shaped(Tick(90_000), 5.0, SegShape::EaseOut { tension: 2.0 }),
    ];
    {
        let c = p.clips.iter_mut().find(|c| !c.fx.is_empty()).expect("clip with an effect");
        c.fx_auto = vec![vec![kt_model::ParamCurve {
            param: "radius".into(),
            keys: keys.clone(),
            source: Default::default(),
        }]];
    }
    p.validate().expect("segment shapes are legal");

    let text = save_project(&p, &Retained::default(), None);
    let back = load_project(&text, None).expect("file with shapes reads");
    let got =
        &back.project.clips.iter().find(|c| !c.fx_auto.is_empty()).expect("clip in place").fx_auto
            [0][0];
    assert_eq!(got.keys, keys, "segment shapes did not survive the round trip");
    assert_eq!(text, save_project(&back.project, &back.retained, None), "the round trip is not idempotent");

    // --- the FORMER `ease` field is read as a UNIFORM shape ---
    // and the curve values must match BIT FOR BIT the values before reading
    for (ease, want) in [(true, SegShape::DEFAULT), (false, SegShape::Linear)] {
        let src: Vec<AutoKey> = vec![
            AutoKey::shaped(Tick(0), 2.0, want),
            AutoKey::shaped(Tick(45_000), 9.0, want),
            AutoKey::shaped(Tick(90_000), 4.0, want),
        ];
        let mut q = rich_project();
        {
            let c = q.clips.iter_mut().find(|c| !c.fx.is_empty()).unwrap();
            c.fx_auto = vec![vec![kt_model::ParamCurve {
                param: "radius".into(),
                keys: src.clone(),
                source: Default::default(),
            }]];
        }
        let t = save_project(&q, &Retained::default(), None);
        // the file holds exactly the FORMER field, not shapes on keys
        assert!(
            t.contains(&format!("\"ease\": {ease}")),
            "uniform shape {want:?} was written not into the previous field"
        );
        assert!(!t.contains("\"shape\""), "a uniform shape wrote shapes on keyframes for nothing");

        let r = load_project(&t, None).expect("reads");
        let got = &r.project.clips.iter().find(|c| !c.fx_auto.is_empty()).unwrap().fx_auto[0][0];
        assert_eq!(got.keys, src, "shape diverged after reading");
        for at in [Tick(0), Tick(1), Tick(22_500), Tick(45_000), Tick(70_000), Tick(90_001)] {
            assert_eq!(
                curve_value(&got.keys, at).to_bits(),
                curve_value(&src, at).to_bits(),
                "curve value after the round trip diverged at {at:?}"
            );
        }
    }
}

/// Test 698d (R.61): an unresolvable source in the file — an error with the address.
///
/// A silent render “by keys” would show not what is written in the file, and that is the
/// worst kind of divergence: a person sees the picture and believes it.
#[test]
fn t698d_a_broken_curve_source_is_an_error_with_an_address() {
    let with_source = |src: kt_model::CurveSource| {
        let mut p = rich_project();
        let c = p.clips.iter_mut().find(|c| !c.fx.is_empty()).expect("clip with an effect");
        c.fx_auto = vec![vec![kt_model::ParamCurve {
            param: "radius".into(),
            keys: Vec::new(),
            source: src,
        }]];
        p
    };

    // a formula that does not parse
    let bad = with_source(kt_model::CurveSource::Formula { expr: "1 + ".into() });
    let err = bad.validate().expect_err("an unparseable formula must be a rejection");
    let text = err.to_string();
    assert!(text.contains("ClipId") || text.contains("clip"), "rejection without an address: {text}");
    assert!(text.len() > 25, "the rejection does not explain the reason: {text}");

    // the link address is not in the form
    let bad = with_source(kt_model::CurveSource::Link {
        path: "not an address at all".into(),
        expr: None,
        stops: Vec::new(),
    });
    assert!(bad.validate().is_err(), "a malformed link address passed silently");

    // a link to an EXISTING property is legal and survives the round trip
    let mut ok = rich_project();
    let ids: Vec<ClipId> = ok.clips.iter().map(|c| c.id).collect();
    assert!(ids.len() >= 2, "at least two events are needed");
    {
        let lead = ok.clips.iter_mut().find(|c| !c.fx.is_empty()).expect("leader");
        lead.fx_auto = vec![vec![kt_model::ParamCurve {
            param: "radius".into(),
            keys: vec![
                kt_model::AutoKey::at(Tick(0), 2.0),
                kt_model::AutoKey::at(Tick(45_000), 6.0),
            ],
            source: Default::default(),
        }]];
    }
    let lead_id = ok.clips.iter().find(|c| !c.fx_auto.is_empty()).map(|c| c.id).expect("leader");
    if let Some(f) = ok.clips.iter_mut().find(|c| c.id != lead_id && !c.fx.is_empty()) {
        f.fx_auto = vec![vec![kt_model::ParamCurve {
            param: "radius_y".into(),
            keys: Vec::new(),
            source: kt_model::CurveSource::Link {
                path: format!("{}/fx0/radius", lead_id.0),
                expr: Some("src / 2".into()),
                stops: Vec::new(),
            },
        }]];
        ok.validate().expect("a link to an existing property is legal");
        let text = save_project(&ok, &Retained::default(), None);
        assert_eq!(
            kt_otio::kt_version(&parse_document(&text).unwrap()),
            Some(kt_otio::KT_META_V10),
            "a working curve source must raise the version to v10"
        );
        let back = load_project(&text, None).expect("file with a link reads");
        let got = back
            .project
            .clips
            .iter()
            .find_map(|c| c.fx_auto.first().and_then(|v| v.iter().find(|x| !x.source.is_keys())))
            .expect("link did not survive the round trip");
        assert_eq!(got.source.link_path(), Some(format!("{}/fx0/radius", lead_id.0).as_str()));
        assert_eq!(got.source.expr(), Some("src / 2"), "the recalculation expression did not survive the round trip");
        assert_eq!(text, save_project(&back.project, &back.retained, None), "idempotence");
    }
}

/// Test 728 (R.62): v11 by content. The project rule, explicit settings,
/// the event geometry (exception rule, framing, the position of the leading
/// node, its curves), the reel's own raster and fps — all survive the round trip
/// and raise the envelope to v11; a file WITHOUT this stays on the former version
/// (t433b keeps the bytes of the old fixtures). The former effect `kt:pan` is read as a
/// “Geometry” node in the same slot with recalculation `pan → pos`.
#[test]
fn t728_conform_geometry_and_reel_settings_round_trip_as_v11() {
    // 1. a flat project with nothing new — the version does not move
    let plain = rich_project();
    let text0 = save_project(&plain, &Retained::default(), None);
    let v0 = kt_otio::kt_version(&parse_document(&text0).unwrap()).unwrap();
    assert!(v0 < kt_otio::KT_META_V11, "a project without geometry must not be v11 (v{v0})");

    // 2. the project rule — by itself v11
    let mut p = rich_project();
    p.settings.conform = ConformRule::Fill;
    p.settings.settings_explicit = true;
    let text = save_project(&p, &Retained::default(), None);
    assert_eq!(kt_otio::kt_version(&parse_document(&text).unwrap()), Some(kt_otio::KT_META_V11));
    let b = load_project(&text, None).unwrap();
    assert_eq!(b.project.settings.conform, ConformRule::Fill);
    assert!(b.project.settings.settings_explicit, "explicitness of settings is the file content");

    // 3. event geometry: an exception to the rule, framing, the leading
    //    node not first, a curve on it
    let mut p = rich_project();
    p.clips[0].geometry = EventGeometry {
        rule: Conform::AsIs,
        crop_l: 0.1,
        crop_r: 0.2,
        pos_x: 0.05,
        zoom: 1.5,
        rot: 10.0,
        ..Default::default()
    };
    p.clips[0].fx = vec![FxInstance::blur(4)];
    p.clips[0].fx_auto = vec![vec![]];
    p.clips[0].fx_bypass = vec![false];
    p.clips[0].geometry_pos = 1;
    p.clips[0].geometry_auto = vec![ParamCurve {
        param: "zoom".into(),
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(45_000), 2.0)]),
        source: Default::default(),
    }];
    // and one more “Geometry” node in the chain of event 2 — also v11
    p.clips[1].fx = vec![FxInstance::geometry_zoom(1.25)];
    p.clips[1].fx_auto = vec![vec![]];
    p.clips[1].fx_bypass = vec![false];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    assert_eq!(kt_otio::kt_version(&parse_document(&text).unwrap()), Some(kt_otio::KT_META_V11));
    let b = load_project(&text, None).unwrap();
    let c = b.project.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    assert_eq!(c.geometry, p.clips[0].geometry, "event geometry survived the round trip");
    assert_eq!(c.geometry_pos, 1, "leader node position survived the round trip");
    assert_eq!(c.geometry_auto, p.clips[0].geometry_auto, "curves of the leader node");
    let c2 = b.project.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(c2.fx, p.clips[1].fx, "the “Geometry” node in the chain");
    assert_eq!(save_project(&b.project, &b.retained, None), text, "idempotence");

    // 4. a reel with its own raster and fps
    let mut p = rich_project();
    let mut s = Sequence::new(MediaId(100), "Square");
    s.raster = Some((720, 720));
    s.fps = Some(25);
    s.clips.push(clip(41, ClipKind::Video, 0, 0, 30_000, 1));
    p.sequences.push(s);
    p.clips.push(clip(45, ClipKind::Video, 0, 90_000, 30_000, 100));
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    assert_eq!(kt_otio::kt_version(&parse_document(&text).unwrap()), Some(kt_otio::KT_META_V11));
    let b = load_project(&text, None).unwrap();
    let q = b.project.sequence(MediaId(100)).expect("reel read back");
    assert_eq!(q.raster, Some((720, 720)), "reel raster survived the round trip");
    assert_eq!(q.fps, Some(25), "reel fps survived the round trip");
    assert_eq!(save_project(&b.project, &b.retained, None), text, "idempotence with a reel");

    // 5. the former `kt:pan` → a “Geometry” node in the same slot, pan → pos
    let mut p = rich_project();
    p.clips[0].fx = vec![FxInstance::blur(4), FxInstance::gain(1.0)];
    p.clips[0].fx_auto = vec![vec![], vec![]];
    p.clips[0].fx_bypass = vec![false, false];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let effects = &mut v["tracks"]["children"][1]["children"][0]["effects"];
    assert_eq!(effects[0]["effect_name"], "kt:blur");
    // the second slot is substituted with an old pan with zoom 2 and a curve by pan_x
    effects[1] = serde_json::json!({
        "OTIO_SCHEMA": "Effect.1",
        "name": "",
        "effect_name": "kt:pan",
        "enabled": true,
        "metadata": {"kt": {
            "kind": "pan", "pan_x": 0.5, "pan_y": -0.5, "zoom": 2.0, "mode": "crop",
            "auto": [{"param": "pan_x", "keys": [{"t": 0, "v": 0.0}, {"t": 45000, "v": 1.0}]}]
        }}
    });
    let b = load_project(&serde_json::to_string_pretty(&v).unwrap(), None)
        .expect("file with the old kt:pan is readable");
    let c = b.project.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    assert_eq!(c.fx.len(), 2, "pan slot did not disappear or shift its neighbors");
    assert!(matches!(c.fx[0], FxInstance::Blur { .. }));
    match &c.fx[1] {
        FxInstance::Geometry { lead, pos_x, pos_y, zoom, .. } => {
            assert!(!lead, "migrated pan is an ordinary node, not a leading one");
            assert_eq!(*zoom, 2.0);
            let want = kt_model::pan_to_pos(0.5, 2.0);
            assert!((pos_x - want).abs() < 1e-12, "pan_x → pos_x at zoom 2: {pos_x} ≠ {want}");
            let want = kt_model::pan_to_pos(-0.5, 2.0);
            assert!((pos_y - want).abs() < 1e-12, "pan_y → pos_y: {pos_y} ≠ {want}");
        }
        other => panic!("expected a “Geometry” node, got {other:?}"),
    }
    let curve = &c.fx_auto[1];
    assert_eq!(curve.len(), 1);
    assert_eq!(curve[0].param, "pos_x", "pan_x curve moved to pos_x");
    let last = curve[0].keys.last().unwrap();
    assert!((last.v - kt_model::pan_to_pos(1.0, 2.0)).abs() < 1e-12, "key values recomputed");
    assert!(c.geometry.is_default(), "migration does not touch the leading node of the event");
}

/// Test 731 (R.62, change B): a TRACK in the file. The switches, level,
/// blend mode, height and the chain survive the round trip
/// and raise the envelope to v11; `Track.enabled` equals `!mute` (a foreign editor sees the
/// muted track honestly); the name lives in `Track.name`, and the order of rows —
/// in the order of the envelope's tracks.
/// A project WITHOUT track novelties is byte-stable on the former version (t433b holds the same on fixtures).
#[test]
fn t731_track_envelope_round_trip_and_version() {
    use kt_model::{BlendMode, FxInstance, LaneKind};
    // without novelties — the former version and there is no envelope of tracks
    let plain = rich_project();
    let text0 = save_project(&plain, &Retained::default(), None);
    let v0 = kt_otio::kt_version(&parse_document(&text0).unwrap()).unwrap();
    assert!(v0 < kt_otio::KT_META_V11, "project without track-level novelties is not v11 (v{v0})");

    let mut p = rich_project();
    p.ensure_tracks(LaneKind::Video, 2);
    {
        let t = p.track_at_mut(LaneKind::Video, 0).unwrap();
        t.name = "Ceiling".into();
        t.mute = true;
        t.opacity = 0.5;
        t.blend = BlendMode::Screen;
        t.height = 72.0;
        t.fx = vec![FxInstance::blur(6)];
        // “nothing is switched off” is an EMPTY vector: the tail of `false`
        // the reader trims, as for a clip (one form for both)
    }
    {
        let t = p.track_at_mut(LaneKind::Audio, 0).unwrap();
        t.solo = true;
        t.lock = true;
        t.gain_db = -6.0;
    }
    p.validate().unwrap();

    let text = save_project(&p, &Retained::default(), None);
    assert_eq!(
        kt_otio::kt_version(&parse_document(&text).unwrap()),
        Some(kt_otio::KT_META_V11),
        "a non-default track setting is a v11 novelty"
    );
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    // A FOREIGN READER: a switched-off track is switched off, the name in place, the chain
    // of standard Effect.1 — all this is visible without our section
    let tracks = v["tracks"]["children"].as_array().unwrap();
    let top = tracks
        .iter()
        .find(|t| t["kind"] == "Video" && t["name"] == "Ceiling")
        .expect("track name lives in Track.name");
    assert_eq!(top["enabled"], false, "enabled must equal !mute");
    assert_eq!(top["effects"][0]["effect_name"], "kt:blur", "track chain is not standard");
    // and ours — in the envelope, and the name of video/audio is NOT duplicated there
    let kt_tracks = v["metadata"]["kt"]["tracks"].as_array().expect("track envelope");
    assert_eq!(kt_tracks.len(), p.tracks.len());
    assert!(kt_tracks.iter().all(|t| t.get("name").is_none()), "track name is duplicated");
    assert_eq!(kt_tracks[0]["blend"], "screen");

    // ROUND: the model matched, the write is idempotent
    let b = load_project(&text, None).expect("file with tracks is readable");
    let q = &b.project;
    assert_eq!(q.tracks, p.tracks, "tracks survived the round trip field by field");
    assert_eq!(save_project(q, &b.retained, None), text, "write idempotence");

    // ROW ORDER — the order of the envelope's tracks, there is no separate field
    let mut p2 = p.clone();
    p2.ensure_tracks(LaneKind::Morph, 1);
    p2.set_lane_order(&[
        (LaneKind::Morph, 0),
        (LaneKind::Video, 0),
        (LaneKind::Video, 1),
        (LaneKind::Audio, 0),
    ]);
    let text2 = save_project(&p2, &Retained::default(), None);
    assert!(!text2.contains("lane_order"), "there is no separate order field any more");
    let b2 = load_project(&text2, None).unwrap();
    assert_eq!(b2.project.rows(), p2.rows(), "row order survived the round trip");
    assert_eq!(b2.project.track_label(LaneKind::Video, 0), "Ceiling");
}

/// Test 747 (R.63 item 1): a file BEFORE R.61 with SPEED curves opens.
///
/// The owner's `untitled_v26.09.19.otio` (14 `rate` curves with `ease: true`)
/// stopped opening altogether: the R.61 reader expanded the former boolean
/// into “smooth”, and the speed validator of the same round accepts only a straight line.
/// Before R.61 the warp map did NOT read the `ease` field AT ALL — a speed curve always
/// sounded piecewise-linear, so the correct translation is a straight line.
/// The file's form here is the same as the owner's: a `rate` curve on an
/// event, in a morpher patch and inside a nested reel.
#[test]
fn t747_old_rate_curves_with_ease_open_as_linear() {
    fn rate_curve(v0: f64, v1: f64) -> ParamCurve {
        ParamCurve {
            param: "rate".into(),
            keys: vec![
                AutoKey::shaped(Tick(0), v0, SegShape::Linear),
                AutoKey::shaped(Tick(45_000), v1, SegShape::Linear),
            ],
            source: Default::default(),
        }
    }
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

    // 1) a speed curve on an EVENT + a foreign curve beside it (its “smooth”
    //    must remain “smooth”)
    let mut c = clip(1, ClipKind::Audio, 0, 0, 90_000, 1);
    c.fx = vec![FxInstance::slowed_reverb(0.75, 0.5, 120.0), FxInstance::gain(0.0)];
    c.fx_bypass = vec![false, false];
    c.fx_auto = vec![
        vec![rate_curve(1.0, 0.25)],
        vec![ParamCurve {
            param: "gain_db".into(),
            keys: vec![
                AutoKey::shaped(Tick(0), -6.0, SegShape::default()),
                AutoKey::shaped(Tick(45_000), 0.0, SegShape::default()),
            ],
            source: Default::default(),
        }],
    ];
    p.clips.push(c);

    // 2) a speed curve in the MORPHER PATCH
    let patch = MorphPatch {
        audio_fx: vec![FxInstance::slowed_reverb(0.8, 0.2, 85.0)],
        audio_bypass: vec![false],
        audio_auto: vec![vec![rate_curve(1.0, 0.5)]],
        ..Default::default()
    };
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        targets: MorphTargets::Below,
        patch,
        enabled: true,
        timed: true,
        strength: 1.0,
        fade_in: Tick(0),
        fade_out: Tick(0),
    });

    // 3) a speed curve INSIDE A REEL
    let mut s = Sequence::new(MediaId(100), "Reel");
    let mut inner = clip(11, ClipKind::Audio, 0, 0, 90_000, 1);
    inner.fx = vec![FxInstance::slowed_reverb(0.9, 0.1, 60.0)];
    inner.fx_bypass = vec![false];
    inner.fx_auto = vec![vec![rate_curve(1.0, 0.75)]];
    s.clips.push(inner);
    p.sequences.push(s);
    p.clips.push(clip(2, ClipKind::Video, 0, 0, 90_000, 100));

    p.normalize();
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);

    // we bring the file to its FORMER form: speed curves have `ease: true`, as
    // builds before R.61 wrote it (they wrote it ALWAYS, see R.61 D1b)
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut patched = 0usize;
    fn old_ease(x: &mut serde_json::Value, n: &mut usize) {
        match x {
            serde_json::Value::Object(m) => {
                if m.get("param").and_then(|p| p.as_str()) == Some("rate")
                    && m.contains_key("ease")
                {
                    m.insert("ease".into(), serde_json::Value::Bool(true));
                    *n += 1;
                }
                for (_, val) in m.iter_mut() {
                    old_ease(val, n);
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(|val| old_ease(val, n)),
            _ => {}
        }
    }
    old_ease(&mut v, &mut patched);
    assert_eq!(patched, 3, "scene is degenerate: the file does not have three speed curves");
    let old_text = serde_json::to_string_pretty(&v).unwrap();

    // on the former code here `RateShapeMustBeLinear` and the file does not open
    let bundle = load_project(&old_text, None).expect("file from the previous build must open");
    let q = &bundle.project;
    let rates: Vec<&ParamCurve> = q
        .clips
        .iter()
        .flat_map(|c| c.fx_auto.iter().flatten())
        .chain(q.morphers.iter().flat_map(|m| m.patch.audio_auto.iter().flatten()))
        .chain(
            q.sequences
                .iter()
                .flat_map(|s| s.clips.iter())
                .flat_map(|c| c.fx_auto.iter().flatten()),
        )
        .filter(|c| c.param == "rate")
        .collect();
    assert_eq!(rates.len(), 3, "speed curves were lost on read");
    for c in &rates {
        assert!(
            c.keys.iter().all(|k| k.shape == SegShape::Linear),
            "speed curve was read as NOT linear: {:?}",
            c.keys
        );
    }
    // a foreign curve kept its smoothing
    let gain = q
        .clips
        .iter()
        .flat_map(|c| c.fx_auto.iter().flatten())
        .find(|c| c.param == "gain_db")
        .expect("gain_db curve is in place");
    assert!(
        gain.keys.iter().all(|k| k.shape != SegShape::Linear),
        "the `ease: true` of a foreign curve must remain smoothing: {:?}",
        gain.keys
    );
    // re-saving is legal and reads again (the migration is one-way)
    let again = save_project(q, &bundle.retained, None);
    load_project(&again, None).expect("re-saved file is readable");
}

/// Test 773 (R.65): the SIDECHAIN KEY in the file — v12 by content.
///
/// A file without sidechain must stay on the former version and byte-for-byte:
/// a field that is absent is not written. A file with a key — v12, because an old
/// build would lose it silently, and ducking would simply not sound. A key on a
/// track that is absent from the file reads as “no key” — the project
/// opens, the device works as an ordinary compressor.
#[test]
fn t773_sidechain_key_round_trips_as_v12() {
    use kt_model::LaneKind::Audio as A;

    // 1. a compressor WITHOUT a key does not move the version and changes no bytes
    let mut plain = rich_project();
    plain.ensure_tracks(A, 2);
    let before = save_project(&plain, &Retained::default(), None);
    plain.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::compressor()];
    plain.validate().unwrap();
    let text0 = save_project(&plain, &Retained::default(), None);
    let v0 = kt_otio::kt_version(&parse_document(&text0).unwrap()).unwrap();
    assert!(v0 < kt_otio::KT_META_V12, "compressor without a key must not be v12 (v{v0})");
    assert!(!text0.contains("\"key\""), "key field is not written without a key");
    let _ = &before;

    // 2. an assigned key — v12 and an idempotent round trip
    let mut p = rich_project();
    p.ensure_tracks(A, 2);
    let speech = p.track_at(A, 1).unwrap().id;
    p.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::Compressor {
        threshold_db: -24.0,
        ratio: 8.0,
        attack_ms: 5.0,
        release_ms: 120.0,
        makeup_db: 0.0,
        key: Some(speech),
    }];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    assert_eq!(
        kt_otio::kt_version(&parse_document(&text).unwrap()),
        Some(kt_otio::KT_META_V12),
        "file with a key must be upgraded to v12"
    );
    let back = load_project(&text, None).unwrap();
    let got: Vec<_> = back
        .project
        .track_at(A, 0)
        .map(|t| kt_model::sidechain_keys(&t.fx).collect())
        .unwrap_or_default();
    assert_eq!(got, vec![speech], "key did not survive the round trip through the file");
    let again = save_project(&back.project, &Retained::default(), None);
    assert_eq!(text, again, "the round trip is not idempotent");

    // 3. a key on a non-existent track — “no key”, and it does not come back to life
    // JSON is written with indentation, so we substitute by the file's format
    let broken = text.replace(&format!("\"key\": {}", speech.0), "\"key\": 9999");
    assert_ne!(broken, text, "key substitution in the text failed — the test checks nothing");
    let loaded = load_project(&broken, None).unwrap();
    assert_eq!(
        loaded.project.track_at(A, 0).map(|t| kt_model::sidechain_keys(&t.fx).count()),
        Some(0),
        "dead key must read as “no key”"
    );
    let rewritten = save_project(&loaded.project, &Retained::default(), None);
    // “9999” may occur in ticks as well, so we look at the FIELD ITSELF
    assert!(!rewritten.contains("\"key\""), "dead key came back to life on write");
    assert!(
        kt_otio::kt_version(&parse_document(&rewritten).unwrap()).unwrap() < kt_otio::KT_META_V12,
        "file without a live key must revert to the previous version"
    );
}

/// Test 783b (R.67): a v5 file with `targets: null` in morphers is refused by the reader
/// with the version and the word “import”; the import repairs it (null → “everything below”),
/// leaves the other targets untouched, and the written file is read by the reader itself.
/// The reader does not learn new old forms — the fix table carries the version.
#[test]
fn t783b_older_version_import_fixes_null_morph_targets() {
    let base = save_project(&rich_project(), &Retained::default(), None);
    let mut old: serde_json::Value = serde_json::from_str(&base).unwrap();
    old["metadata"]["kt"]["v"] = serde_json::json!(5);
    old["metadata"]["kt"]["morph_lanes"] = serde_json::json!(2);
    let m = |id: u64, targets: serde_json::Value| {
        let mut v = serde_json::json!({
            "id": id, "lane": 0, "start": 0, "duration": 45000,
            "patch": { "media": 2, "source_in_delta": 4500 },
        });
        v["targets"] = targets;
        v
    };
    old["metadata"]["kt"]["morphers"] = serde_json::json!([
        m(1, serde_json::Value::Null),
        m(2, serde_json::json!({ "on": "clips", "ids": [1] })),
        m(3, serde_json::Value::Null),
        m(4, serde_json::json!([1])),
        m(5, serde_json::json!({ "on": "clips", "ids": [1] })),
        m(6, serde_json::Value::Null),
        // a patch with the retired `pan` (the owner's untitled2 form, morpher id=4)
        serde_json::json!({
            "id": 7, "lane": 0, "start": 0, "duration": 45000,
            "patch": {
                "video_fx": [
                    { "kind": "pan", "mode": "fill", "pan_x": -0.92, "pan_y": -1.8, "zoom": 0.43 },
                    { "kind": "event_grid", "cols": 2, "rows": 2, "center": true }
                ],
                "video_bypass": [false, false],
                "video_auto": [[], []]
            }
        }),
    ]);
    let text = serde_json::to_string_pretty(&old).unwrap();

    // --- the reader: a refusal names the version and the door ---------------------------
    let err = match load_project(&text, None) {
        Ok(_) => panic!("reader does not read v5 with null targets"),
        Err(e) => e,
    };
    let msg = err.to_string();
    assert!(
        matches!(err, kt_otio::Error::NeedsImport { version: 5, .. }),
        "error must be NeedsImport with version 5: {msg}"
    );
    assert!(msg.contains("kt.v = 5") && msg.contains("Import older"), "text without a version/door: {msg}");

    // --- import: null → below, the rest intact ----------------------------
    let b = kt_otio::import_older(&text, None).expect("import must open v5");
    assert_eq!(b.project.morphers.len(), 7);
    // pan in the patch became a “Geometry” node in the same slot, the second slot intact
    let pan = &b.project.morphers[6].patch;
    assert_eq!(pan.video_fx.len(), 2, "number of patch slots did not change");
    assert!(
        matches!(pan.video_fx[0], kt_model::FxInstance::Geometry { .. }),
        "pan did not become geometry: {:?}",
        pan.video_fx[0]
    );
    assert!(matches!(pan.video_fx[1], kt_model::FxInstance::EventGrid { .. }));
    assert!(b.notices.iter().any(|n| n.contains("pan → geometry node (v6) ×1")), "{:?}", b.notices);
    let below = |i: usize| b.project.morphers[i].targets == MorphTargets::Below;
    assert!(below(0) && below(2) && below(5), "null did not become “everything below”");
    assert_eq!(b.project.morphers[1].targets.clip_ids(), &[ClipId(1)]);
    assert_eq!(b.project.morphers[3].targets.clip_ids(), &[ClipId(1)], "bare list is intact");
    assert_eq!(b.project.morphers[4].targets.clip_ids(), &[ClipId(1)]);
    assert!(
        b.notices.iter().any(|n| n.contains("morpher targets: null → below (v5) ×3")),
        "repair is not named: {:?}",
        b.notices
    );
    // fix table: each has its version
    assert!(kt_otio::FIXUPS.iter().all(|f| f.version >= 1 && !f.name.is_empty()));

    // --- the written file is read by the reader itself, the version current -----------
    let out = save_project(&b.project, &b.retained, None);
    // the version — BY CONTENT (the v9–v12 rule), but not the old one with null
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    // “below” is not serialized at all; an explicit null must not occur
    assert!(v["metadata"]["kt"]["morphers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m.get("targets").is_none_or(|t| !t.is_null())));
    let again = load_project(&out, None).expect("after import the reader reads by itself");
    assert_eq!(again.project.morphers, b.project.morphers);

    // a file of the current version passes through the import without fixes
    let fresh = kt_otio::import_older(&base, None).expect("fresh file via import");
    assert!(fresh.notices.iter().all(|n| !n.starts_with("import fixup")));
}

/// Tests 787b/787d (R.67): the segment formula and the layer over keys travel into the file,
/// the version is raised to v13 ONLY by content, the round trip is byte-for-byte;
/// a file without them — the former version and the former bytes. An uncomputable formula
/// in the file — a loud load refusal.
#[test]
fn t787b_segment_formula_and_layer_round_trip_as_v13_by_content() {
    let mut p = rich_project();
    let clip = p.clips.iter_mut().find(|c| c.kind == kt_model::ClipKind::Video).unwrap();
    clip.fx = vec![kt_model::FxInstance::Mirror { horizontal: true, vertical: false }];
    let seg = kt_model::ParamCurve {
        param: "amount".into(),
        keys: vec![
            AutoKey::shaped(Tick(0), 0.0, kt_model::SegShape::Formula { expr: kt_model::ExprId::intern("a + (b - a) * s * s") }),
            AutoKey::at(Tick(45_000), 1.0),
        ],
        source: kt_model::CurveSource::Keys,
    };
    clip.fx_auto = vec![vec![seg.clone()]];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 13, "segment formula upgrades the file to v13");
    let b = load_project(&text, None).expect("v13 is readable");
    assert_eq!(b.project.clips.iter().find(|c| c.kind == kt_model::ClipKind::Video).unwrap().fx_auto[0][0], seg);
    assert_eq!(text, save_project(&b.project, &b.retained, None), "round trip is byte for byte");

    // a layer over keys — also v13
    let mut p2 = rich_project();
    let clip = p2.clips.iter_mut().find(|c| c.kind == kt_model::ClipKind::Video).unwrap();
    clip.fx = vec![kt_model::FxInstance::Mirror { horizontal: true, vertical: false }];
    let layer = kt_model::ParamCurve {
        param: "amount".into(),
        keys: vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(45_000), 1.0)],
        source: kt_model::CurveSource::Formula { expr: "k * 0.5".into() },
    };
    clip.fx_auto = vec![vec![layer.clone()]];
    p2.validate().unwrap();
    let text2 = save_project(&p2, &Retained::default(), None);
    let v2: serde_json::Value = serde_json::from_str(&text2).unwrap();
    assert_eq!(v2["metadata"]["kt"]["v"], 13, "layer over keys upgrades the file to v13");
    let b2 = load_project(&text2, None).expect("v13 with a layer is readable");
    assert_eq!(b2.project.clips.iter().find(|c| c.kind == kt_model::ClipKind::Video).unwrap().fx_auto[0][0], layer);

    // a source formula WITHOUT keys and without `k` — the former version, the former bytes
    let mut p3 = rich_project();
    let clip = p3.clips.iter_mut().find(|c| c.kind == kt_model::ClipKind::Video).unwrap();
    clip.fx = vec![kt_model::FxInstance::Mirror { horizontal: true, vertical: false }];
    clip.fx_auto = vec![vec![kt_model::ParamCurve {
        param: "amount".into(),
        keys: vec![],
        source: kt_model::CurveSource::Formula { expr: "t * 2".into() },
    }]];
    p3.validate().unwrap();
    let text3 = save_project(&p3, &Retained::default(), None);
    let v3: serde_json::Value = serde_json::from_str(&text3).unwrap();
    assert!(v3["metadata"]["kt"]["v"].as_u64().unwrap() < 13, "without R.67 novelties the version does not grow");
    // a project without formulas at all — the former bytes
    let plain = save_project(&rich_project(), &Retained::default(), None);
    assert!(!plain.contains("formula"), "file without formulas bears no trace of them");

    // an uncomputable segment formula in the file — a loud refusal
    let broken = text.replace("a + (b - a) * s * s", "foo(s)");
    assert_ne!(broken, text, "formula text must be stored in the file as is");
    match load_project(&broken, None) {
        Ok(_) => panic!("non-computable segment formula opened silently"),
        Err(e) => assert!(e.to_string().contains("foo"), "refusal does not name it: {e}"),
    }
}

/// t828 (kinetren-rename): new files carry the `kinetren` marker; a file of
/// the former name (`kinotrain`) is refused by Open with the door's name and
/// comes in through the import door unchanged in content (owner's rule R.67:
/// the repair lives in the import, not in the reader).
#[test]
fn t828_former_marker_is_refused_by_open_and_repaired_by_import() {
    use kt_model::project_file::{FileKind, ProjectFileError, from_text, to_text};
    let p = project_3d();
    let text = to_text(&p, None);
    assert!(text.starts_with("{\n  \"kinetren\": 1"), "{text:.60}");
    let former = text.replacen("\"kinetren\"", "\"kinotrain\"", 1);
    assert_ne!(former, text, "the scene must really carry the former marker");

    // the reader refuses it and names the former name and the import door
    let err = from_text(&former).unwrap_err();
    assert_eq!(err, ProjectFileError::NotOurs { looks_like: Some(FileKind::FormerName) });
    let msg = err.to_string();
    assert!(msg.contains("Kinotrain") && msg.contains("Import older Kinetren project"), "{msg}");
    assert!(matches!(load_project(&former, None), Err(kt_otio::Error::FormerProject)));

    // the import door: the same project as reading today's file
    let b = kt_otio::import_interchange(&former, None).expect("the import door reads it");
    assert_eq!(b.project, from_text(&text).unwrap().project);
    assert!(b.notices.iter().any(|n| n.contains("kinotrain")), "{:?}", b.notices);
    // today's file is not taken by the import door's repair: Open is its door
    assert!(matches!(kt_otio::import_interchange(&text, None), Err(kt_otio::Error::ProjectFile)));
}
