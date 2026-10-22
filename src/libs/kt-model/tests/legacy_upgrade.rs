//! Test 143 (item 143 porting-notes, R.24): import of legacy overlaps.
//!
//! An audio pair without FX → a trim to the middle of the overlap plus a transition in the same
//! window (byte-equivalence of the sound is pinned by t136 on the same formula);
//! 3+/FX overlaps → a split across tracks WITHOUT volume compensation (user decision item 9
//! of R.24); a video overlap → demotion of the later clip onto the inserted track below
//! (the composite is byte-identical to the old tie-break).

use kt_model::ops::{UpgradeReport, upgrade_legacy};
use kt_model::*;
use kt_time::Tick;

fn clip(id: u64, kind: ClipKind, lane: u32, start: i64, dur: i64, src: i64) -> ClipEvent {
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
        source_in: Tick(src),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn raw_project(clips: Vec<ClipEvent>) -> Project {
    // NOT validate: legacy overlaps break the invariant before the upgrade
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
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips = clips;
    // the tests t143 pin the conversion of overlaps into transitions/tracks — it lives in the mode
    // WITHOUT interleaving (R.28); with interleaving the import touches nothing, and this is checked
    // by test 164
    p.overlap_layers = false;
    p
}

#[test]
fn t143_audio_pair_becomes_transition() {
    // A[0,90000), B[60000,150000) — the overlap [60000,90000), C = 75000
    let mut p = raw_project(vec![
        clip(1, ClipKind::Audio, 0, 0, 90_000, 0),
        clip(2, ClipKind::Audio, 0, 60_000, 90_000, 7_000),
    ]);
    let rep = upgrade_legacy(&mut p);
    p.validate().expect("after the upgrade the project is valid");
    assert_eq!(rep, UpgradeReport { transitions_created: 1, audio_moved: 0, video_moved: 0 });

    let a = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let b = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(a.start + a.duration, Tick(75_000), "A trimmed to C");
    assert_eq!(b.start, Tick(75_000), "B starts at C");
    assert_eq!(b.source_in, Tick(7_000 + 15_000), "TrimStart shifted the source of B");
    let t = &p.transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(1)), Some(ClipId(2))));
    assert_eq!((t.in_offset, t.out_offset), (Tick(15_000), Tick(15_000)), "window == overlap");
    assert_eq!(t.window(&p).unwrap(), kt_time::TickRange::new(Tick(60_000), Tick(90_000)));

    // idempotence
    assert!(upgrade_legacy(&mut p).is_empty());
    p.validate().unwrap();
}

#[test]
fn t143_reversed_incoming_keeps_source_in() {
    // for a reversed TrimStart the source_in is NOT touched (item 16) — the handle is mirrored
    let mut b = clip(2, ClipKind::Audio, 0, 60_000, 90_000, 5_000);
    b.reversed = true;
    let mut p = raw_project(vec![clip(1, ClipKind::Audio, 0, 0, 90_000, 0), b]);
    upgrade_legacy(&mut p);
    p.validate().unwrap();
    let b = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(b.start, Tick(75_000));
    assert_eq!(b.source_in, Tick(5_000), "reversed: source not shifted");
}

#[test]
fn t143_three_overlap_and_fx_pairs_are_split_to_lanes() {
    // three clips in one zone → a pair is impossible, two move lower
    let mut p = raw_project(vec![
        clip(1, ClipKind::Audio, 0, 0, 90_000, 0),
        clip(2, ClipKind::Audio, 0, 30_000, 90_000, 0),
        clip(3, ClipKind::Audio, 0, 50_000, 90_000, 0),
    ]);
    let rep = upgrade_legacy(&mut p);
    p.validate().expect("the split yields a valid project");
    assert_eq!(rep.transitions_created, 0, "a triple overlap is not a pair");
    assert_eq!(rep.audio_moved, 2);
    // the early one stayed on its own track
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(1)).unwrap().lane, 0);
    // all tracks are distinct in the conflict zone, the material is intact
    assert_eq!(p.clips.len(), 3);

    // an FX clip in a pair → also a split (the trim would shift the automation)
    let mut fxc = clip(2, ClipKind::Audio, 0, 60_000, 90_000, 0);
    fxc.fx = vec![FxInstance::slowed_reverb(0.8, 0.2, 85.0)];
    let mut p = raw_project(vec![clip(1, ClipKind::Audio, 0, 0, 90_000, 0), fxc]);
    let rep = upgrade_legacy(&mut p);
    p.validate().unwrap();
    assert_eq!((rep.transitions_created, rep.audio_moved), (0, 1));
}

#[test]
fn t143_video_overlap_demotes_later_clip_below() {
    // video: the one on top was the one with the smaller start (the old tie-break) —
    // the later one moves to the INSERTED track below, the existing bottom track
    // is shifted further down
    let mut p = raw_project(vec![
        clip(1, ClipKind::Video, 0, 0, 90_000, 0),
        clip(2, ClipKind::Video, 0, 60_000, 90_000, 0),
        clip(3, ClipKind::Video, 1, 0, 30_000, 0), // the background — does not interfere
    ]);
    let rep = upgrade_legacy(&mut p);
    p.validate().unwrap();
    assert_eq!(rep.video_moved, 1);
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(1)).unwrap().lane, 0, "early one on top");
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(2)).unwrap().lane, 1, "late one below");
    assert_eq!(
        p.clips.iter().find(|c| c.id == ClipId(3)).unwrap().lane,
        2,
        "the old background moved down"
    );
    assert!(p.lanes(LaneKind::Video) >= 3);
}

/// Test 164 (item 164 porting-notes, R.28): importing an old project changes NOTHING —
/// “as it was, with overlaps”. The owner’s decision after his real project (1952 clips,
/// 76 overlaps) was unfolded
/// into roughly 40 video tracks.
#[test]
fn t164_legacy_import_keeps_overlaps_and_lanes() {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.ensure_tracks(LaneKind::Video, 2);
    p.ensure_tracks(LaneKind::Audio, 2);
    // a mix typical of a real project: an A/V pair with an overlap, a triple zone
    // and a micro-overlap of one tick
    let mut add = |id: u64, kind: ClipKind, lane: u32, start: i64, dur: i64, group: Option<u64>| {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: group.map(GroupId),
            kind,
            lane,
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
    };
    add(1, ClipKind::Video, 0, 0, 90_000, Some(1));
    add(2, ClipKind::Audio, 0, 0, 90_000, Some(1));
    add(3, ClipKind::Video, 0, 60_000, 90_000, Some(2)); // an overlap of 30 000
    add(4, ClipKind::Audio, 0, 60_000, 90_000, Some(2));
    add(5, ClipKind::Audio, 0, 80_000, 90_000, None); // a triple zone
    add(6, ClipKind::Video, 1, 0, 45_000, None);
    add(7, ClipKind::Video, 1, 44_999, 45_000, None); // a micro-overlap of 1 tick

    let before = p.clips.clone();
    let (lanes_v, lanes_a) = (p.lanes(LaneKind::Video), p.lanes(LaneKind::Audio));
    let report = kt_model::ops::upgrade_legacy(&mut p);

    assert_eq!(p.clips.len(), before.len(), "no clip disappeared");
    for c in &p.clips {
        let was = before.iter().find(|x| x.id == c.id).unwrap();
        assert_eq!(
            (c.start, c.duration, c.source_in, c.lane),
            (was.start, was.duration, was.source_in, was.lane),
            "clip {:?} untouched by the import",
            c.id
        );
    }
    assert_eq!(
        (p.lanes(LaneKind::Video), p.lanes(LaneKind::Audio)),
        (lanes_v, lanes_a),
        "tracks did not multiply"
    );
    assert!(p.transitions.is_empty(), "overlaps stayed overlaps, not transitions");
    assert_eq!(report.transitions_created, 0);
    assert_eq!(report.audio_moved, 0);
    assert_eq!(report.video_moved, 0);
    p.validate().expect("a project with overlaps is valid");
}
