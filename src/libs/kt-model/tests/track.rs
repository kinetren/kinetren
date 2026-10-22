//! R.62, change B: the track is a model object with a stable id.

use kt_model::HasParams;
use kt_model::LaneKind::{Audio as A, Morph as M, Video as V};
use kt_model::*;
use kt_time::Tick;

fn settings30() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn vclip(id: u64, lane: u32, start: i64) -> ClipEvent {
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
        lane,
        start: Tick(start),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn project() -> Project {
    let mut p = Project::new(settings30());
    p.media.push(MediaRef::new(MediaId(1), "synth:sine:440"));
    p.ensure_tracks(V, 2);
    p.clips.push(vclip(1, 0, 0));
    p.clips.push(vclip(2, 1, 100_000));
    p.validate().unwrap();
    p
}

/// Test 729 (R.62): the track id is stable across insertion above, reordering
/// and deletion of a neighbour; the name and height are track content and go through
/// undo; properties travel with the object, not with the number.
#[test]
fn t729_track_id_is_stable_and_name_height_are_undoable() {
    let mut m = TimelineModel::new(project());
    let bottom = m.project().track_at(V, 1).unwrap().id;
    assert_ne!(bottom.0, 0, "id assigned");

    // name and height — ONE property command, and it is rolled back
    let mut props = m.project().track(bottom).unwrap().props();
    props.name = "Titles".into();
    props.height = 48.0;
    m.apply(Command::SetTrackProps { id: bottom, props }).unwrap();
    assert_eq!(m.project().track_label(V, 1), "Titles");
    assert_eq!(m.project().track(bottom).unwrap().height, 48.0);
    m.undo().unwrap();
    assert_eq!(m.project().track_label(V, 1), "V2", "name did not roll back");
    assert_eq!(m.project().track(bottom).unwrap().height, 0.0, "height did not roll back");
    m.redo().unwrap();

    // inserting a track ABOVE: the number grew, the id is the same, properties with it
    m.apply(Command::InsertTrack { kind: V, at: 0 }).unwrap();
    let p = m.project();
    assert_eq!(p.lanes(V), 3);
    assert_eq!(p.track_at(V, 2).unwrap().id, bottom, "id moved when inserting a neighbor");
    assert_eq!(p.track_label(V, 2), "Titles");
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(2)).unwrap().lane, 2);
    let inserted = p.track_at(V, 0).unwrap().id;
    assert!(inserted != bottom && inserted.0 != 0, "the new track got its own id");

    // reordering: the bottom one moved to the top — the id and properties travel with the object
    m.apply(Command::SetLaneOrder { order: vec![(V, 2), (V, 0), (V, 1), (A, 0)] }).unwrap();
    let p = m.project();
    assert_eq!(p.track_at(V, 0).unwrap().id, bottom);
    assert_eq!(p.track_label(V, 0), "Titles");
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(2)).unwrap().lane, 0);

    // deleting a neighbour: the id stays, the neighbour’s content goes. After
    // the reordering the rows read “Titles” (the former bottom one), the empty inserted one,
    // event track 1 — so the deletion of RANK 2 takes the content away
    m.apply(Command::RemoveTrack { kind: V, at: 2 }).unwrap();
    let p = m.project();
    assert_eq!(p.lanes(V), 2);
    assert_eq!(p.track_at(V, 0).unwrap().id, bottom);
    assert!(p.clips.iter().all(|c| c.id != ClipId(1)), "events of the deleted track remained");
    m.undo().unwrap();
    assert_eq!(m.project().lanes(V), 3, "undo did not restore the track");
    assert!(m.project().clips.iter().any(|c| c.id == ClipId(1)), "undo did not restore the events");

    // the last track of a view cannot be removed; service ones — all of them can
    assert!(matches!(
        m.apply(Command::RemoveTrack { kind: A, at: 0 }),
        Err(CommandError::LastTrack(A))
    ));
    m.apply(Command::InsertTrack { kind: M, at: 0 }).unwrap();
    m.apply(Command::RemoveTrack { kind: M, at: 0 }).unwrap();
    assert_eq!(m.project().lanes(M), 0);

    // ids are unique per document, including reels
    let mut p = m.project().clone();
    p.sequences.push(Sequence::new(MediaId(100), "Piece"));
    p.assign_track_ids();
    let ids: Vec<TrackId> =
        p.tracks.iter().chain(p.sequences[0].tracks.iter()).map(|t| t.id).collect();
    let uniq: std::collections::HashSet<TrackId> = ids.iter().copied().collect();
    assert_eq!(ids.len(), uniq.len());
}

/// Test 730 (R.62): the track property spec sheet. Limits and types come from there, and
/// not from interface literals; the level is declared PER VIEW (dB for audio,
/// opacity for video); for service rows the only toggle is
/// the lock. A track property has no curve and cannot have one: its key is a timeline
/// tick, and that is a second automation domain (the next change),
/// so the rows are structural and `auto_params()` is empty.
#[test]
fn t730_track_property_passport_and_no_curves() {
    let p = project();
    let audio = p.track_at(A, 0).unwrap();
    let video = p.track_at(V, 0).unwrap();

    // level — by view; the track has no foreign row (dead controls do not exist)
    let spec = audio.param_spec("gain_db").expect("an audio track has level in dB");
    assert_eq!((spec.min, spec.max, spec.default), (TRACK_GAIN_MIN_DB, TRACK_GAIN_MAX_DB, 0.0));
    assert_eq!(spec.unit, ParamUnit::Db);
    assert!(audio.param_spec("opacity").is_none(), "audio has no opacity");
    let spec = video.param_spec("opacity").expect("a video track has opacity");
    assert_eq!((spec.min, spec.max, spec.default), (0.0, 1.0, 1.0));
    assert!(video.param_spec("gain_db").is_none(), "video has no level in dB");

    // the clamp goes THROUGH the spec sheet
    let mut t = audio.clone();
    assert!(t.set_param("gain_db", 999.0));
    assert_eq!(t.gain_db, TRACK_GAIN_MAX_DB, "level is not clamped by the passport limit");
    let mut t = video.clone();
    assert!(t.set_param("opacity", -5.0));
    assert_eq!(t.opacity, 0.0);

    // toggles: clip rows have three, service rows only the lock
    let names = |t: &Track| t.flag_specs().iter().map(|f| f.name).collect::<Vec<_>>();
    assert_eq!(names(audio), vec!["mute", "solo", "lock"]);
    let mut q = project();
    q.ensure_tracks(M, 1);
    let morph = q.track_at(M, 0).unwrap();
    assert_eq!(names(morph), vec!["lock"], "a morph row has no mute and solo");
    assert_eq!(morph.get_flag("mute"), None);

    // blend mode — a typed choice FOR VIDEO, keyed by the variant
    assert_eq!(video.get_value("blend"), Some(ParamValue::Choice("normal".into())));
    let mut t = video.clone();
    assert!(t.set_value("blend", &ParamValue::Choice("screen".into())));
    assert_eq!(t.blend, BlendMode::Screen);
    assert!(!t.set_value("blend", &ParamValue::Choice("no such".into())), "a foreign key not silently");
    assert_eq!(audio.value_type("blend"), None, "an audio track has no overlay mode");

    // TRACKS HAVE NO CURVES: not a single animatable row
    for t in [audio, video] {
        assert!(
            t.param_specs().iter().all(|s| s.kind != ParamKind::Smooth),
            "the track time domain is the next task: there must be no animatable series"
        );
    }
    // and the model rejects them loudly if they nonetheless appeared in the file
    let mut bad = project();
    bad.track_at_mut(A, 0).unwrap().fx_auto = vec![vec![ParamCurve {
        param: "gain_db".into(),
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(90_000), 1.0)]),
        source: Default::default(),
    }]];
    assert!(
        matches!(bad.validate(), Err(ProjectError::TrackCurvesUnsupported(_))),
        "a curve on a track must fail loading with a reason: {:?}",
        bad.validate()
    );
}

/// Test 738 (R.62): the LOCK is one gate for the whole command registry. Each
/// command either names the rows it edits (`target_rows`) or is declared “does not touch row content”;
/// the match itself is exhaustive, so a new command will not compile until it is
/// assigned to one half.
/// Here the CONSEQUENCE is checked: a gesture on a locked track is rejected with
/// a reason, while the track’s own properties can be edited.
#[test]
fn t738_locked_track_refuses_edits_but_stays_editable_itself() {
    let mut m = TimelineModel::new(project());
    let locked = m.project().track_at(V, 0).unwrap().id;
    let mut props = m.project().track(locked).unwrap().props();
    props.lock = true;
    m.apply(Command::SetTrackProps { id: locked, props }).unwrap();

    // gestures on EVENTS of a locked track — rejection with a reason
    let moved = Command::MoveClip { id: ClipId(1), new_start: Tick(5_000), new_lane: 0 };
    for cmd in [
        moved.clone(),
        Command::RemoveClip { id: ClipId(1) },
        Command::TrimStart { id: ClipId(1), new_start: Tick(1_000) },
        Command::Split { id: ClipId(1), at: Tick(45_000), new_id: ClipId(99), new_group: None },
        Command::SetOpacity { id: ClipId(1), opacity: 0.5 },
        Command::SetFx { id: ClipId(1), fx: vec![] },
    ] {
        assert!(
            matches!(m.apply(cmd.clone()), Err(CommandError::TrackLocked(t)) if t == locked),
            "a gesture on a locked track must be a rejection: {cmd:?}"
        );
    }
    // the model is untouched
    assert_eq!(m.project().clips.iter().find(|c| c.id == ClipId(1)).unwrap().start, Tick(0));

    // a MOVE ONTO a locked track is also a rejection (the lock applies to the row,
    // not the event): an event from a neighbouring track does not slide in
    assert!(matches!(
        m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(0), new_lane: 0 }),
        Err(CommandError::TrackLocked(_))
    ));

    // deleting a locked track takes its events with it — also a rejection
    assert!(matches!(
        m.apply(Command::RemoveTrack { kind: V, at: 0 }),
        Err(CommandError::TrackLocked(_))
    ));

    // but the track’s OWN properties are edited — otherwise there would be no way to unlock it
    let mut props = m.project().track(locked).unwrap().props();
    props.mute = true;
    m.apply(Command::SetTrackProps { id: locked, props }).expect("mute of a locked track");
    assert!(m.project().track(locked).unwrap().mute);
    let mut props = m.project().track(locked).unwrap().props();
    props.lock = false;
    m.apply(Command::SetTrackProps { id: locked, props }).expect("the lock is released");
    m.apply(moved).expect("after releasing the lock the gesture passes");

    // the neighbouring UNlocked track keeps working all this time
    let free = m.project().track_at(V, 1).map(|t| t.id);
    assert!(free.is_some());
    m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(7_000), new_lane: 1 })
        .expect("the neighboring track is not locked");
}
