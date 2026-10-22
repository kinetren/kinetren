//! Test 12 (docs/porting-notes.md): round trip of commands, undo, byte-stable serialization,
//! explicit load errors.

use kt_model::*;
use kt_time::Tick;

fn sample_project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1920,
        height: 1080,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "clips/a.mp4"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(10),
        link_group: Some(GroupId(1)),
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(900_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![FxInstance::slowed_reverb(0.82, 0.3, 140.0)],
        opacity: 1.0,
        fx_off: false,
    });
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(11),
        link_group: Some(GroupId(1)),
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(900_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: -3.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p
}

#[test]
fn load_rejects_explicitly() {
    // future version
    let mut p = sample_project();
    p.format_version = 999;
    let s = serde_json::to_string(&p).unwrap();
    assert_eq!(Project::from_json(&s), Err(ProjectError::UnsupportedVersion(999)));

    // unknown media — an error, not a silent dropping of the clip (defect p)
    let mut p = sample_project();
    p.clips[0].media = MediaId(777);
    let s = serde_json::to_string(&p).unwrap();
    assert_eq!(Project::from_json(&s), Err(ProjectError::UnknownMedia(ClipId(10), MediaId(777))));

    // non-positive duration
    let mut p = sample_project();
    p.clips[1].duration = Tick(0);
    let s = serde_json::to_string(&p).unwrap();
    assert_eq!(Project::from_json(&s), Err(ProjectError::NonPositiveDuration(ClipId(11))));
}

#[test]
fn unknown_fields_are_tolerated() {
    // forward-compat: extra fields do not break the load
    let mut v: serde_json::Value = serde_json::to_value(sample_project()).unwrap();
    v["future_field"] = serde_json::json!({"x": 1});
    v["clips"][0]["future_flag"] = serde_json::json!(true);
    let p = Project::from_json(&v.to_string()).unwrap();
    assert_eq!(p, sample_project());
}

#[test]
fn every_command_round_trips_via_undo() {
    let base = sample_project();
    let commands = vec![
        Command::AddMedia { id: MediaId(2), path: "b.wav".into() },
        Command::RemoveClip { id: ClipId(11) },
        Command::MoveClip { id: ClipId(10), new_start: Tick(1000), new_lane: 1 },
        Command::TrimStart { id: ClipId(10), new_start: Tick(9000) },
        Command::TrimEnd { id: ClipId(10), new_end: Tick(450_000) },
        Command::Split {
            id: ClipId(10),
            at: Tick(450_000),
            new_id: ClipId(20),
            new_group: Some(GroupId(2)),
        },
        Command::SetGain { id: ClipId(10), gain_db: 6.0 },
        Command::SetFx { id: ClipId(10), fx: vec![FxInstance::gain(-1.5)] },
        Command::LinkClips { ids: vec![ClipId(10), ClipId(11)], group: GroupId(9) },
        Command::UnlinkClip { id: ClipId(10) },
    ];
    for cmd in commands {
        let mut m = TimelineModel::new(base.clone());
        m.apply(cmd.clone()).unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
        assert_ne!(m.project(), &base, "{cmd:?} must change the model");
        m.undo().unwrap();
        assert_eq!(m.project(), &base, "undo after {cmd:?} must restore the original");
        m.redo().unwrap();
        m.undo().unwrap();
        assert_eq!(m.project(), &base);
    }
}

#[test]
fn invalid_command_leaves_no_undo_entry() {
    // Regression for defect q: pushing undo before validation created no-op records.
    let mut m = TimelineModel::new(sample_project());
    assert!(m.apply(Command::RemoveClip { id: ClipId(404) }).is_err());
    assert!(
        m.apply(Command::Split {
            id: ClipId(10),
            at: Tick(0), // the boundary — nothing to cut
            new_id: ClipId(21),
            new_group: None,
        })
        .is_err()
    );
    assert_eq!(m.undo_depth(), 0, "failed commands must not enter history");
    assert_eq!(m.undo(), Err(CommandError::NothingToUndo));
}

#[test]
fn linked_group_moves_together() {
    let mut m = TimelineModel::new(sample_project());
    m.apply(Command::MoveClip { id: ClipId(10), new_start: Tick(5000), new_lane: 0 }).unwrap();
    let audio = m.clip(ClipId(11)).unwrap();
    assert_eq!(audio.start, Tick(5000), "the A/V partner moves by the same offset");
}

#[test]
fn split_semantics() {
    let mut m = TimelineModel::new(sample_project());
    m.apply(Command::TrimStart { id: ClipId(10), new_start: Tick(9000) }).unwrap();
    let before = m.clip(ClipId(10)).unwrap().clone();
    m.apply(Command::Split {
        id: ClipId(10),
        at: Tick(300_000),
        new_id: ClipId(20),
        new_group: Some(GroupId(7)),
    })
    .unwrap();
    let left = m.clip(ClipId(10)).unwrap();
    let right = m.clip(ClipId(20)).unwrap();
    assert_eq!(left.start, before.start);
    assert_eq!(left.start + left.duration, Tick(300_000));
    assert_eq!(right.start, Tick(300_000));
    assert_eq!(right.start + right.duration, before.start + before.duration);
    // source_in of the right half continues the source WITHOUT A BREAK.
    //
    // The clip of the fixture has rate 0.82 (`slowed_reverb`), therefore “without a break”
    // here is NOT the length of the piece but the material consumed: 291000 · 0.82 = 238620.
    // Before R.39 a linear expectation stood here — that is, the test asserted the defect itself:
    // the cut of a slowed event drifted by 52380 ticks (0.58 s)
    // and gave an audible jump.
    let elapsed = (Tick(300_000) - before.start).0 as f64;
    let want = before.source_in + Tick((elapsed * 0.82).round() as i64);
    assert_eq!(right.source_in, want, "the right half continues the MATERIAL, not the length");
    assert_eq!(right.link_group, Some(GroupId(7)));
}

#[test]
fn trim_start_keeps_source_alignment() {
    let mut m = TimelineModel::new(sample_project());
    m.apply(Command::TrimStart { id: ClipId(10), new_start: Tick(90_000) }).unwrap();
    let c = m.clip(ClipId(10)).unwrap();
    assert_eq!(c.start, Tick(90_000));
    assert_eq!(c.duration, Tick(810_000));
    // The point of the test (“the material under the cursor must not shift”) was written before R.39
    // as a LINEAR number, although the fixture clip has rate 0.82: at
    // moment 90_000 the material under the cursor is 73_800, not 90_000.
    // Now the assertion matches its own name.
    assert_eq!(
        c.source_in,
        Tick((90_000.0f64 * 0.82).round() as i64),
        "material under the cursor must not shift"
    );
}

#[test]
fn undo_depth_is_capped() {
    let mut m = TimelineModel::new(sample_project());
    for i in 0..(MAX_UNDO + 25) {
        m.apply(Command::SetGain { id: ClipId(10), gain_db: i as f32 }).unwrap();
    }
    assert_eq!(m.undo_depth(), MAX_UNDO);
    let mut n = 0;
    while m.undo().is_ok() {
        n += 1;
    }
    assert_eq!(n, MAX_UNDO);
}
