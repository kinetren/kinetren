//! Tests 416-417 (R.49 B9): the sync-by-sound command.

use kt_model::*;
use kt_time::Tick;

fn scene_at(start_a: Tick, source_in_b: Tick, start_b: Tick, reversed_b: bool) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), "synth:sine:440"));
    let clip = |id: u64, media: u64, lane: u32, start: Tick, src: Tick, rev: bool| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start,
        duration: Tick(900_000),
        source_in: src,
        media: MediaId(media),
        gain_db: 0.0,
        reversed: rev,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    p.clips.push(clip(1, 1, 0, start_a, Tick(90_000), false));
    p.clips.push(clip(2, 2, 1, start_b, source_in_b, reversed_b));
    p.ensure_tracks(LaneKind::Audio, 2);
    p.normalize();
    p
}

fn scene(source_in_b: Tick, start_b: Tick, reversed_b: bool) -> Project {
    scene_at(Tick(180_000), source_in_b, start_b, reversed_b)
}

/// Test 416 (R.49 B9): the shift is applied so that the materials line up, and
/// takes into account the trimmed start of each event.
#[test]
fn t416_sync_aligns_the_material_not_the_clip_edges() {
    // B enters its material from another point and stands not there
    let p = scene(Tick(45_000), Tick(600_000), false);
    // “material B lags behind material A by 0.2 s”
    let off = Tick(18_000);
    let ops::SyncPlan::Move(cmds) = ops::sync_by_audio_commands(&p, ClipId(1), ClipId(2), off)
    else {
        panic!("a move was expected");
    };
    assert_eq!(cmds.len(), 1, "one move command was expected: {cmds:?}");
    let mut m = TimelineModel::new(p.clone());
    m.apply_batch(cmds).expect("move");
    let b = m.project().clips.iter().find(|c| c.id == ClipId(2)).unwrap().clone();

    // we check the CONSEQUENCE: the real-world moment at the start of A
    // now stands in the same place for B too
    let step = 90_000 / 30;
    let mat_a =
        clip_material_at(Tick(90_000), Tick(180_000), Tick(900_000), false, step, Tick(180_000));
    let mat_b = Tick(mat_a.0 + off.0);
    let at_b = clip_timeline_at(b.source_in, b.start, b.duration, b.reversed, step, mat_b);
    assert_eq!(at_b.0, 180_000, "materials did not align: moment B landed at {}", at_b.0);

    // a repeat moves nothing: already aligned
    let again = ops::sync_by_audio_commands(m.project(), ClipId(1), ClipId(2), off);
    assert_eq!(
        again,
        ops::SyncPlan::AlreadyAligned,
        "repeated sync moves what is already aligned: {again:?}"
    );
}

/// Test 417 (R.49 B9): sync does NOT trim and does NOT link.
///
/// It answers the question “where”, not “how long” or “together or not”: a magnet
/// trims the neighbour — that is its property, while a link group is a separate decision
/// of the editor. And it must understand reverse: for a reversed event
/// the material runs backward, and its own arithmetic by `source_in` would have gone off here.
#[test]
fn t417_sync_only_moves_and_understands_reverse() {
    let p = scene(Tick(45_000), Tick(600_000), false);
    let before = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap().clone();
    let ops::SyncPlan::Move(cmds) =
        ops::sync_by_audio_commands(&p, ClipId(1), ClipId(2), Tick(18_000))
    else {
        panic!("a move was expected");
    };
    for c in &cmds {
        assert!(
            matches!(c, Command::MoveClip { .. }),
            "sync issued a command that is not a move: {c:?}"
        );
    }
    let mut m = TimelineModel::new(p);
    m.apply_batch(cmds).unwrap();
    let after = m.project().clips.iter().find(|c| c.id == ClipId(2)).unwrap().clone();
    assert_eq!(after.duration, before.duration, "sync trimmed the event");
    assert_eq!(after.source_in, before.source_in, "sync shifted the entry point");
    assert_eq!(after.link_group, before.link_group, "sync linked the events");
    assert_eq!(after.lane, before.lane, "sync reordered the track");

    // REVERSE: the same consequence on a reversed event. The event stands
    // LATER: on a reversed one the start of the material is shown at the end, and
    // alignment drags it back almost the whole length.
    // on a reversed event the start of the material is shown at the END, and
    // alignment drags it back almost the whole length — so the target must
    // not stand at the very start of the project, otherwise there is not enough room (t417b)
    let p = scene_at(Tick(900_000), Tick(45_000), Tick(2_500_000), true);
    let off = Tick(18_000);
    let ops::SyncPlan::Move(cmds) = ops::sync_by_audio_commands(&p, ClipId(1), ClipId(2), off)
    else {
        panic!("a move on reverse was expected");
    };
    let mut m = TimelineModel::new(p);
    m.apply_batch(cmds).unwrap();
    let b = m.project().clips.iter().find(|c| c.id == ClipId(2)).unwrap().clone();
    let step = 90_000 / 30;
    let mat_a =
        clip_material_at(Tick(90_000), Tick(900_000), Tick(900_000), false, step, Tick(900_000));
    let at_b =
        clip_timeline_at(b.source_in, b.start, b.duration, b.reversed, step, Tick(mat_a.0 + off.0));
    assert_eq!(at_b.0, 900_000, "on reverse the materials did not align: {}", at_b.0);
}

/// Alignment that pushes an event past the start of the project is a REJECTION, not a clamp.
/// An event pinned to zero looks successfully synchronized, while
/// the materials stay out of step.
#[test]
fn t417b_sync_refuses_instead_of_clamping_to_zero() {
    let p = scene(Tick(45_000), Tick(600_000), true);
    let r = ops::sync_by_audio_commands(&p, ClipId(1), ClipId(2), Tick(18_000));
    assert_eq!(
        r,
        ops::SyncPlan::OutOfRange,
        "alignment at the start of the project was pinned to zero and reported as success: {r:?}"
    );
}
