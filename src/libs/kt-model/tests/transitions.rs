//! Test 135 (item 135 porting-notes, R.24): transitions in the model.
//!
//! A crossfade is an explicit TransitionEvent at the joint of two neighbouring clips (OTIO),
//! not an overlap. Here: commands/undo, validation invariants, the fate
//! of the transition on neighbour edits (reconcile), retargeting on Split.

use kt_model::*;
use kt_time::Tick;

fn base_project() -> Project {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:Ramp"));
    for (id, start) in [(1u64, 0i64), (2, 48_000), (3, 96_000)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(start),
            duration: Tick(48_000),
            source_in: Tick(start.max(0)),
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
    p.validate().unwrap();
    p
}

fn tr(id: u64, a: u64, b: u64, in_off: i64, out_off: i64) -> TransitionEvent {
    TransitionEvent {
        id: TransitionId(id),
        a: Some(ClipId(a)),
        b: Some(ClipId(b)),
        in_offset: Tick(in_off),
        out_offset: Tick(out_off),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    }
}

#[test]
fn t135_commands_undo_and_window() {
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 4_500))).unwrap();
    assert_eq!(m.project().transitions.len(), 1);
    let w = m.project().transitions[0].window(m.project()).unwrap();
    assert_eq!((w.start, w.end), (Tick(39_000), Tick(52_500)), "window around the joint");

    m.apply(Command::SetTransition(tr(1, 1, 2, 1_000, 2_000))).unwrap();
    assert_eq!(m.project().transitions[0].in_offset, Tick(1_000));
    m.apply(Command::RemoveTransition { id: TransitionId(1) }).unwrap();
    assert!(m.project().transitions.is_empty());

    m.undo().unwrap(); // remove rolled back
    assert_eq!(m.project().transitions[0].in_offset, Tick(1_000));
    m.undo().unwrap(); // set rolled back
    assert_eq!(m.project().transitions[0].in_offset, Tick(9_000));
    m.undo().unwrap(); // add rolled back
    assert!(m.project().transitions.is_empty());
}

#[test]
fn t135_validation_rejects_bad_transitions() {
    let mut m = TimelineModel::new(base_project());
    // not neighbours (1 and 3 are not butted together)
    assert!(matches!(
        m.apply(Command::AddTransition(tr(1, 1, 3, 900, 900))),
        Err(CommandError::InvalidTransition(ProjectError::TransitionNotAtJoint(_)))
    ));
    // window wider than the clip body
    assert!(matches!(
        m.apply(Command::AddTransition(tr(1, 1, 2, 48_001, 0))),
        Err(CommandError::InvalidTransition(ProjectError::TransitionTooWide(_)))
    ));
    // already at the minimum
    assert!(matches!(
        m.apply(Command::AddTransition(tr(1, 1, 2, 44, 45))),
        Err(CommandError::InvalidTransition(ProjectError::TransitionTooShort(_)))
    ));
    // negative offset
    assert!(matches!(
        m.apply(Command::AddTransition(tr(1, 1, 2, -10, 900))),
        Err(CommandError::InvalidTransition(ProjectError::TransitionTooWide(_)))
    ));

    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    // a second transition at the same joint
    assert!(matches!(
        m.apply(Command::AddTransition(tr(2, 1, 2, 900, 900))),
        Err(CommandError::InvalidTransition(ProjectError::DuplicateTransitionJoint(_, _)))
    ));
    // the windows of neighbouring transitions on one track overlap:
    // window (1|2) = [39000, 57000), window (2|3) with in=9000 starts at 87000 — ok,
    // but with in=48000/out=0 it starts at 48000 < 57000 — a conflict
    assert!(matches!(
        m.apply(Command::AddTransition(tr(2, 2, 3, 48_000, 0))),
        Err(CommandError::InvalidTransition(ProjectError::TransitionWindowsOverlap(_, _)))
    ));
    m.apply(Command::AddTransition(tr(2, 2, 3, 9_000, 9_000))).unwrap();

    // a busy id and a non-existent id
    assert!(matches!(
        m.apply(Command::AddTransition(tr(2, 1, 2, 900, 900))),
        Err(CommandError::IdInUse(2))
    ));
    assert!(matches!(
        m.apply(Command::SetTransition(tr(99, 1, 2, 900, 900))),
        Err(CommandError::TransitionNotFound(TransitionId(99)))
    ));
    assert!(matches!(
        m.apply(Command::RemoveTransition { id: TransitionId(99) }),
        Err(CommandError::TransitionNotFound(TransitionId(99)))
    ));

    // project validate() catches the same thing directly (for converters)
    let mut p = base_project();
    p.transitions.push(tr(7, 1, 3, 900, 900));
    assert!(matches!(p.validate(), Err(ProjectError::TransitionNotAtJoint(TransitionId(7)))));

    // The invariant of stage 6 (decision 8 of R.24) applies when the layering
    // is OFF (R.28): then overlapping clips on one track is illegal — both in
    // validate(), and through the strict check in apply_batch
    let mut p = base_project();
    p.overlap_layers = false;
    p.clips[1].start = Tick(40_000); // overlap onto clip 1
    assert!(matches!(p.validate(), Err(ProjectError::ClipOverlap(ClipId(1), ClipId(2)))));
    let mut base = base_project();
    base.overlap_layers = false;
    let mut m = TimelineModel::new(base);
    assert!(matches!(
        m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(40_000), new_lane: 0 }),
        Err(CommandError::InvalidLayout(ProjectError::ClipOverlap(_, _)))
    ));
    // and with a foreign placeholder
    let mut p = base_project();
    p.foreign.push(ForeignEvent {
        key: 1,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(46_000),
        duration: Tick(10_000),
    });
    assert!(matches!(p.validate(), Err(ProjectError::ForeignOverlap(_, 1))));
}

#[test]
fn t135_reconcile_on_neighbor_edits() {
    // Moving one neighbour tears the joint — the transition dies (moving it to another
    // track: a time shift would now honestly catch ClipOverlap)
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(48_000), new_lane: 1 }).unwrap();
    assert!(m.project().transitions.is_empty(), "broken joint must kill the transition");
    m.undo().unwrap();
    assert_eq!(m.project().transitions.len(), 1, "undo brings it back");

    // Group move of both neighbours keeps the joint — the transition lives
    // (clip 3 removed: a shift to the right would now honestly catch a collision)
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::RemoveClip { id: ClipId(3) }).unwrap();
    m.apply(Command::LinkClips { ids: vec![ClipId(1), ClipId(2)], group: GroupId(10) }).unwrap();
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(6_000), new_lane: 0 }).unwrap();
    assert_eq!(m.project().transitions.len(), 1, "group move keeps the joint");
    assert_eq!(m.project().transitions[0].joint(m.project()), Some(Tick(54_000)));

    // Trim shortens the body — the offset is clamped; below the minimum — it dies
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 2, 3, 9_000, 0))).unwrap();
    // the end of clip 3 is not touched; trimming ALL of clip 2 from the right is impossible (the joint!),
    // so we clamp through shortening clip 2 from the left: body 2 became 3000
    m.apply(Command::TrimStart { id: ClipId(2), new_start: Tick(93_000) }).unwrap();
    assert_eq!(m.project().transitions.len(), 1);
    assert_eq!(m.project().transitions[0].in_offset, Tick(3_000), "clamped by body");
    // the body cannot become shorter than MIN_CLIP_DURATION, so the clamp hits
    // 90 and the transition stays alive (the “window < minimum” branch in reconcile —
    // a defensive one: unreachable through trims, since the minimums of the clip and of the window are equal)
    m.apply(Command::TrimStart { id: ClipId(2), new_start: Tick(95_910) }).unwrap();
    assert_eq!(m.project().transitions.len(), 1);
    assert_eq!(m.project().transitions[0].in_offset, Tick(90), "clamped to minimal body");

    // Deleting the neighbour — the transition dies
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::RemoveClip { id: ClipId(2) }).unwrap();
    assert!(m.project().transitions.is_empty());
}

#[test]
fn t135_split_retargets_outgoing() {
    // Split of the OUTGOING neighbour: the joint (its end) moves to the right half
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::Split {
        id: ClipId(1),
        at: Tick(24_000),
        new_id: ClipId(50),
        new_group: None,
    })
    .unwrap();
    assert_eq!(m.project().transitions.len(), 1, "transition survives outgoing split");
    let t = &m.project().transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(50)), Some(ClipId(2))));
    assert_eq!(t.joint(m.project()), Some(Tick(48_000)));

    // Split of the INCOMING neighbour: the joint (its start) stays with the left half
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::Split {
        id: ClipId(2),
        at: Tick(72_000),
        new_id: ClipId(51),
        new_group: None,
    })
    .unwrap();
    let t = &m.project().transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(1)), Some(ClipId(2))), "incoming split keeps ids");
    // the body of incoming shortened to 24000 — out_offset got clamped
    assert_eq!(t.out_offset, Tick(9_000).min(Tick(24_000)));
}

// ---- test 155: fades at the track edge (R.25) -----------------------------------

fn fade(id: u64, a: Option<u64>, b: Option<u64>, in_off: i64, out_off: i64) -> TransitionEvent {
    TransitionEvent {
        id: TransitionId(id),
        a: a.map(ClipId),
        b: b.map(ClipId),
        in_offset: Tick(in_off),
        out_offset: Tick(out_off),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    }
}

/// A transition with ONE live end is legal: there is emptiness to its left/right
/// (the track edge or a Gap). The joint is the boundary of a live clip, the window is counted
/// from it; the offset TOWARD the emptiness is not limited by the body.
#[test]
fn t155_fade_is_a_transition_with_one_side() {
    let mut p = base_project();
    // fade-in of the first clip and fade-out of the last
    p.transitions.push(fade(1, None, Some(1), 9_000, 9_000));
    p.transitions.push(fade(2, Some(3), None, 9_000, 9_000));
    p.validate().expect("fades at the track edges are legal");

    let fi = &p.transitions[0];
    assert!(fi.is_fade());
    assert_eq!(fi.joint(&p), Some(Tick(0)), "the fade-in seam is the start of its clip");
    assert_eq!(fi.window(&p).unwrap().start, Tick(-9_000), "part of the window hangs in the void");

    let fo = &p.transitions[1];
    assert_eq!(fo.joint(&p), Some(Tick(144_000)), "the fade-out seam is the end of its clip");

    // the offset TOWARD THE CLIP is still limited by the body
    let mut bad = p.clone();
    bad.transitions[0].out_offset = Tick(48_001);
    assert!(bad.validate().is_err(), "a fade cannot be wider than the body of its clip");
    // and toward the emptiness — no
    let mut wide = p.clone();
    wide.transitions[0].in_offset = Tick(500_000);
    wide.validate().expect("a fade extends freely into the void");

    // a transition with no clips at all — an error
    let mut orphan = base_project();
    orphan.transitions.push(fade(9, None, None, 9_000, 9_000));
    assert!(orphan.validate().is_err());
}

/// A fade lives by the same rules as a crossfade: a trim clamps the window, deleting a
/// clip takes the transition with it, copy-paste carries the fade together with its clip.
#[test]
fn t155_fade_survives_edits_and_clipboard() {
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(fade(1, None, Some(1), 9_000, 9_000))).unwrap();

    // a clip trim shortens the offset on its side
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(4_000) }).unwrap();
    let t = &m.project().transitions[0];
    assert_eq!(t.out_offset, Tick(4_000), "an offset into the body is clamped by trim");
    assert_eq!(t.in_offset, Tick(9_000), "an offset into the void is not touched by trim");

    // deleting the clip takes the fade with it too
    m.apply(Command::RemoveClip { id: ClipId(1) }).unwrap();
    assert!(m.project().transitions.is_empty());

    // copy-paste: the fade travels with its own clip
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(fade(1, None, Some(2), 9_000, 9_000))).unwrap();
    let block = kt_model::ops::copy_group(m.project(), ClipId(2));
    assert_eq!(block.transitions.len(), 1, "fade in a block");
    let (mut next_clip, mut next_group, mut next_tr, mut next_morph) =
        (100u64, 100u64, 100u64, 1u64);
    let mut next_cam = 1u64;
    let (cmds, _end, _) = kt_model::ops::paste_block(
        &block,
        Tick(300_000),
        kt_model::ops::PasteIds {
            clip: &mut next_clip,
            group: &mut next_group,
            transition: &mut next_tr,
            morph: &mut next_morph,
            camera: &mut next_cam,
        },
        &kt_model::ops::PasteNames::default(),
    );
    m.apply_batch(cmds).unwrap();
    let pasted = m.project().transitions.iter().find(|t| t.id == TransitionId(100)).unwrap();
    assert!(pasted.a.is_none() && pasted.b == Some(ClipId(100)), "the inserted fade stayed a fade");
}

// ---- test 159: restoring the “handles” when a crossfade is broken (R.26) ------------------

/// An overlap cuts clips up to the joint; pulling them apart must restore what was cut —
/// as in Vegas, where an overlap simply stops being an overlap.
#[test]
fn t159_restore_handles_after_pulling_apart() {
    // we prepare the state “after the overlap”: clips butted together + transition
    let mut m = TimelineModel::new(base_project());
    let before_trim = m.project().clone();
    // overlap: clip 2 slid 20_000 to the left, the joint is the middle of the overlap
    m.apply_batch(vec![
        Command::MoveClip { id: ClipId(2), new_start: Tick(28_000), new_lane: 0 },
        Command::TrimEnd { id: ClipId(1), new_end: Tick(38_000) },
        Command::TrimStart { id: ClipId(2), new_start: Tick(38_000) },
        Command::AddTransition(tr(1, 1, 2, 10_000, 10_000)),
    ])
    .unwrap();

    // we pull apart: clip 2 moves far to the right
    let cmds = vec![Command::MoveClip { id: ClipId(2), new_start: Tick(300_000), new_lane: 0 }];
    let after = m.preview(&cmds).unwrap();
    assert!(after.transitions.is_empty(), "a broken seam kills the transition");
    let restore = kt_model::ops::restore_handles(m.project(), &after);
    assert_eq!(restore.len(), 2, "per command for each end: {restore:?}");

    let mut all = cmds;
    all.extend(restore);
    m.apply_batch(all).unwrap();

    let p = m.project();
    let a = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let b = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    let a0 = before_trim.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let b0 = before_trim.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(a.duration, a0.duration, "the outgoing one got its tail back");
    assert_eq!(b.duration, b0.duration, "the incoming one got its head back");
    assert_eq!(b.source_in, b0.source_in, "the entry point came back too");
    assert_eq!(b.start, Tick(300_000) - Tick(10_000), "the head grows left from the new position");
}

/// Pulled apart NOT fully — as much returns as fits, and the gap
/// is split between both ends.
#[test]
fn t159_restore_is_clamped_by_free_space() {
    let mut m = TimelineModel::new(base_project());
    // overlap: clip 2 slid 20_000 to the left, the joint is the middle of the overlap
    m.apply_batch(vec![
        Command::MoveClip { id: ClipId(2), new_start: Tick(28_000), new_lane: 0 },
        Command::TrimEnd { id: ClipId(1), new_end: Tick(38_000) },
        Command::TrimStart { id: ClipId(2), new_start: Tick(38_000) },
        Command::AddTransition(tr(1, 1, 2, 10_000, 10_000)),
    ])
    .unwrap();

    // the gap is only 6000 ticks against 20000 “wishes”
    let cmds = vec![Command::MoveClip { id: ClipId(2), new_start: Tick(44_000), new_lane: 0 }];
    let after = m.preview(&cmds).unwrap();
    let mut all = cmds;
    all.extend(kt_model::ops::restore_handles(m.project(), &after));
    m.apply_batch(all).unwrap();

    let p = m.project();
    let a = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let b = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!((a.start + a.duration).0, 41_000, "the outgoing one got half of the gap");
    assert_eq!(b.start.0, 41_000, "the incoming one got the other half, the clips closed up");
    p.validate().expect("no overlap appeared");
}

/// We deleted the neighbour — the surviving clip gets its piece back (the owner’s
/// decision, R.26 item 1).
#[test]
fn t159_restore_after_neighbour_removed() {
    let mut m = TimelineModel::new(base_project());
    // overlap: clip 2 slid 20_000 to the left, the joint is the middle of the overlap
    m.apply_batch(vec![
        Command::MoveClip { id: ClipId(2), new_start: Tick(28_000), new_lane: 0 },
        Command::TrimEnd { id: ClipId(1), new_end: Tick(38_000) },
        Command::TrimStart { id: ClipId(2), new_start: Tick(38_000) },
        Command::AddTransition(tr(1, 1, 2, 10_000, 10_000)),
    ])
    .unwrap();

    let cmds = vec![Command::RemoveClip { id: ClipId(2) }];
    let after = m.preview(&cmds).unwrap();
    let restore = kt_model::ops::restore_handles(m.project(), &after);
    assert_eq!(restore.len(), 1, "only the surviving end is restored");
    let mut all = cmds;
    all.extend(restore);
    m.apply_batch(all).unwrap();
    let a = m.project().clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    assert_eq!((a.start + a.duration), Tick(48_000), "the tail came back whole");
}

/// there is nothing for a fade to return: it is created by an explicit command, cutting nothing
#[test]
fn t159_fade_has_nothing_to_restore() {
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(fade(7, None, Some(2), 9_000, 9_000))).unwrap();
    let cmds = vec![Command::RemoveClip { id: ClipId(2) }];
    let after = m.preview(&cmds).unwrap();
    assert!(kt_model::ops::restore_handles(m.project(), &after).is_empty());
}

/// The return runs into the track’s obstacles — including foreign placeholders
/// (the “no overlaps” invariant counts them too).
#[test]
fn t159_restore_stops_at_obstacles() {
    // we assemble the state “after the overlap” at once: clips butted together + transition,
    // and four thousand ticks past the joint stands a foreign construct
    let mut p = base_project();
    p.clips.retain(|c| c.id != ClipId(3));
    let (a, b) = (Tick(0), Tick(38_000));
    for c in &mut p.clips {
        if c.id == ClipId(1) {
            (c.start, c.duration) = (a, b - a);
        } else {
            (c.start, c.duration) = (b, Tick(38_000));
        }
    }
    p.foreign.push(ForeignEvent {
        key: 1,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(80_000),
        duration: Tick(4_000),
    });
    p.transitions.push(tr(1, 1, 2, 10_000, 10_000));
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);

    // clip 2 moves past the placeholder; clip 1 wants to return 10_000, but
    // it hits it already after 4_000 — still, there is enough room up to the placeholder:
    // we check that the growth stopped exactly at the obstacle
    let cmds = vec![Command::MoveClip { id: ClipId(2), new_start: Tick(90_000), new_lane: 0 }];
    let after = m.preview(&cmds).unwrap();
    let mut all = cmds;
    all.extend(kt_model::ops::restore_handles(m.project(), &after));
    m.apply_batch(all).unwrap();

    let p = m.project();
    p.validate().expect("result is valid");
    let c2 = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(c2.start, Tick(84_000), "the head grew only up to the placeholder");
    let c1 = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    assert_eq!(c1.start + c1.duration, Tick(48_000), "the tail came back whole");
}

/// The return must not break the gesture itself: for a reversed clip TrimEnd
/// is limited by source_in, and for an ordinary TrimStart — by zero material. Without
/// a clamp check_on would return TooShort and roll back the WHOLE batch (the move together with
/// the gift) — so the clamp is done by restore_handles itself.
#[test]
fn t159_restore_never_breaks_the_gesture() {
    for reversed in [false, true] {
        let mut p = base_project();
        p.clips.retain(|c| c.id != ClipId(3));
        for c in &mut p.clips {
            c.reversed = reversed;
            // almost no material beyond the edge: only 2000 ticks can be returned
            c.source_in = Tick(2_000);
        }
        // the state “after the overlap”: butted together + transition
        p.clips[0].duration = Tick(38_000);
        p.clips[1].start = Tick(38_000);
        p.clips[1].duration = Tick(38_000);
        p.transitions.push(tr(1, 1, 2, 10_000, 10_000));
        p.validate().unwrap();
        let mut m = TimelineModel::new(p);

        let cmds = vec![Command::MoveClip { id: ClipId(2), new_start: Tick(300_000), new_lane: 0 }];
        let after = m.preview(&cmds).unwrap();
        let mut all = cmds;
        all.extend(kt_model::ops::restore_handles(m.project(), &after));
        m.apply_batch(all).unwrap_or_else(|e| panic!("reversed={reversed}: gesture failed: {e}"));
        let p = m.project();
        p.validate().unwrap();
        assert_eq!(
            p.clips.iter().find(|c| c.id == ClipId(2)).unwrap().start,
            Tick(300_000 - if reversed { 10_000 } else { 2_000 }),
            "reversed={reversed}: returned as much as there is material"
        );
    }
}

/// R.33 S4: every transition kind has a full parameter spec (the same as
/// for effects), values are set/read/clamped, tags are unique, and
/// constructor defaults match the spec defaults. Without this the transition UI
/// would have to be written by hand — exactly the inconsistency
/// the user complained about.
#[test]
fn t223_transition_kind_specs() {
    let all = TransitionKind::ALL;
    assert_eq!(
        all.len(),
        33,
        "33 kinds: 16 previous + 17 R.51 (the anchor “16” was removed by decision of the owner, docs/user-feedback.md R.51)"
    );
    let mut tags: Vec<&str> = all.iter().map(|k| k.tag()).collect();
    tags.sort_unstable();
    let uniq = tags.len();
    tags.dedup();
    assert_eq!(tags.len(), uniq, "kind tags are unique: {tags:?}");

    let mut with_params = 0;
    for k in TransitionKind::ALL {
        let mut k = k.clone();
        for spec in k.param_specs() {
            with_params += 1;
            let (name, lo, hi) = (spec.name.clone(), spec.min, spec.max);
            assert!(
                (lo..=hi).contains(&spec.default),
                "{}.{name}: default {} outside [{lo}, {hi}]",
                k.tag(),
                spec.default
            );
            assert!(spec.step > 0.0, "{}.{name}: step must be positive", k.tag());
            // kind default == spec default: otherwise “reset” in the UI
            // gives something other than what creating the transition gives
            let fresh = TransitionKind::ALL.iter().find(|x| x.tag() == k.tag()).unwrap().clone();
            let got = fresh.get_param(&name).unwrap();
            assert!(
                (got - spec.default).abs() <= 1e-9,
                "{}.{name}: constructor gives {got}, spec is {}",
                k.tag(),
                spec.default
            );
            let mid = (lo + hi) / 2.0;
            assert!(k.set_param(&name, mid), "{}.{name} did not set", k.tag());
            let got = k.get_param(&name).unwrap();
            assert!((got - mid).abs() <= 0.5, "{}.{name}: {got} vs {mid}", k.tag());
            assert!(k.set_param(&name, hi + 1e6));
            assert!(k.get_param(&name).unwrap() <= hi + 1e-9, "{}.{name} does not clamp", k.tag());
            assert!(k.set_param(&name, lo - 1e6));
            assert!(k.get_param(&name).unwrap() >= lo - 1e-9);
        }
        assert!(!k.set_param("no_such_param", 1.0), "{}: foreign name — rejection", k.tag());
    }
    assert!(with_params >= 20, "parameters of the kinds: {with_params}");
    assert!(
        TransitionKind::Crossfade.param_specs().is_empty(),
        "a crossfade has no numeric parameters — it stays as before"
    );

    // audio curves: monotonic, exact ends, the power sum does not dip
    for c in
        [AudioCurve::EqualPower, AudioCurve::Linear, AudioCurve::SCurve, AudioCurve::Exponential]
    {
        assert!((c.rise(0.0)).abs() < 1e-12, "{c:?}: start from zero");
        assert!((c.rise(1.0) - 1.0).abs() < 1e-12, "{c:?}: end at one");
        let mut prev = -1.0;
        for i in 0..=100 {
            let v = c.rise(i as f64 / 100.0);
            assert!(v >= prev - 1e-12, "{c:?}: not monotonic at {i}");
            prev = v;
        }
    }
    // the character of the curves by the sum of POWERS in the middle of the joint (both halves
    // are computed by one formula, so each one’s contribution is rise(0.5)^2)
    let mid = |c: AudioCurve| 2.0 * c.rise(0.5) * c.rise(0.5);
    assert!((mid(AudioCurve::EqualPower) - 1.0).abs() < 1e-12, "equal-power holds power");
    assert!((mid(AudioCurve::SCurve) - 1.0).abs() < 1e-12, "the S-curve also holds power");
    assert!((mid(AudioCurve::Linear) - 0.5).abs() < 1e-12, "linear gives the classic −3 dB");
    assert!(mid(AudioCurve::Exponential) < 0.5, "exponential — a deliberately deep dip");
    assert_eq!(AudioCurve::default(), AudioCurve::EqualPower, "the default is the previous formula");
}

/// Test 462 (item 399, R.51): a transition on an OVERLAP — a “staircase” pair, window =
/// the overlap zone, mode is not stored. Validation, fate on edits
/// (reconcile), return of handles and copy-paste.
#[test]
fn t462_overlap_transition_model() {
    // clips 1 and 2 overlapping in a “staircase”: [0..60k) and [30k..90k)
    let overlap_project = || {
        let mut p = base_project();
        p.clips.retain(|c| c.id.0 <= 2);
        p.clips[0].duration = Tick(60_000);
        p.clips[1].start = Tick(30_000);
        p.clips[1].duration = Tick(60_000);
        p.validate().unwrap();
        p
    };
    let ov = |id: u64, a: u64, b: u64| TransitionEvent {
        id: TransitionId(id),
        a: Some(ClipId(a)),
        b: Some(ClipId(b)),
        in_offset: Tick::ZERO,
        out_offset: Tick::ZERO,
        kind: TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.1 },
        audio: AudioCurve::Linear,
        auto: vec![],
        invert: false,
    };

    // 1. validation: the staircase is legal, the window is the overlap zone itself
    let mut p = overlap_project();
    p.transitions.push(ov(1, 1, 2));
    p.validate().expect("an overlapping transition is legal");
    let w = p.transitions[0].window(&p).unwrap();
    assert_eq!((w.start, w.end), (Tick(30_000), Tick(60_000)), "window = overlap zone");
    assert!(p.transitions[0].overlap_pair(&p).is_some());

    // 2. reversed role order — rejection (canonicalization is done by reconcile)
    let mut p = overlap_project();
    p.transitions.push(ov(1, 2, 1));
    assert!(p.validate().is_err(), "a reversed pair is not a staircase");

    // 3. full containment — rejection
    let mut p = overlap_project();
    p.clips[1].start = Tick(10_000);
    p.clips[1].duration = Tick(20_000); // clip 2 entirely inside clip 1
    p.transitions.push(ov(1, 1, 2));
    assert!(p.validate().is_err(), "nesting is not a staircase");

    // 4. a third clip in the window — rejection with the culprit’s name
    let mut p = overlap_project();
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(3),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(40_000),
        duration: Tick(10_000),
        source_in: Tick::ZERO,
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.transitions.push(ov(1, 1, 2));
    match p.validate() {
        Err(ProjectError::TransitionOverlapBusy(_, c)) => assert_eq!(c, ClipId(3)),
        other => panic!("TransitionOverlapBusy was expected, got {other:?}"),
    }

    // 5. reconcile: a joint transition survives as an OVERLAP when the pair overlaps
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.project().transitions[0].window(m.project()).unwrap();
    // overlap: clip 2 slid 18k to the left — now a staircase
    m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(30_000), new_lane: 0 }).unwrap();
    let p = m.project();
    assert_eq!(p.transitions.len(), 1, "the transition survived the overlap by overlapping");
    let t = &p.transitions[0];
    assert!(t.overlap_pair(p).is_some(), "now it is an overlapping transition");
    assert_eq!(
        t.kind,
        TransitionKind::Crossfade,
        "kind untouched (see t460 on preserving settings)"
    );
    let w = t.window(p).unwrap();
    assert_eq!((w.start, w.end), (Tick(30_000), Tick(48_000)), "window = new overlap");
    // offsets — dead weight, untouched
    assert_eq!((t.in_offset, t.out_offset), (Tick(9_000), Tick(9_000)));

    // ...and returning to the joint revives the old offsets
    m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(48_000), new_lane: 0 }).unwrap();
    let p = m.project();
    assert_eq!(p.transitions.len(), 1);
    assert!(p.transitions[0].overlap_pair(p).is_none(), "the pair is butt-joined again");
    let w = p.transitions[0].window(p).unwrap();
    assert_eq!((w.start, w.end), (Tick(39_000), Tick(57_000)), "window again from offsets");

    // 6. reconcile: a break (pulled far apart) kills the overlapping transition
    let mut m = TimelineModel::new(overlap_project());
    m.apply(Command::AddTransition(ov(1, 1, 2))).unwrap();
    m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(90_000), new_lane: 0 }).unwrap();
    assert!(m.project().transitions.is_empty(), "overlap disappeared — the transition died");

    // 7. reconcile: canonicalization of roles — clip b moved LEFT of a, the roles
    // mirror, the transition is alive
    let mut m = TimelineModel::new(overlap_project());
    m.apply(Command::AddTransition(ov(1, 1, 2))).unwrap();
    // clip 2 (was the late one) moves to 0, clip 1 starts there too... no:
    // we shift clip 1 to the right so that the former b becomes the early one
    m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(45_000), new_lane: 0 }).unwrap();
    let p = m.project();
    assert_eq!(p.transitions.len(), 1, "the transition is alive after changing the order");
    let t = &p.transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(2)), Some(ClipId(1))), "roles canonicalized: a is early");
    assert!(t.overlap_pair(p).is_some());

    // 8. restore_handles: a break of an overlapping transition does NOT grant trims
    // (nothing was cut during the overlap)
    let before = {
        let mut p = overlap_project();
        p.transitions.push(ov(1, 1, 2));
        p.validate().unwrap();
        p
    };
    let after = {
        let mut p = before.clone();
        p.transitions.clear();
        p.clips[1].start = Tick(90_000); // pulled apart
        p.validate().unwrap();
        p
    };
    assert!(
        ops::restore_handles(&before, &after).is_empty(),
        "an overlapping transition has no cut handles"
    );

    // 9. copy-paste: an overlapping transition survives in the block as a whole
    let p = {
        let mut p = overlap_project();
        p.transitions.push(ov(1, 1, 2));
        p.validate().unwrap();
        p
    };
    let block = ops::copy_clips(&p, &[ClipId(1), ClipId(2)]);
    assert_eq!(block.transitions.len(), 1, "overlapping transition copied");
    let block1 = ops::copy_clips(&p, &[ClipId(1)]);
    assert!(block1.transitions.is_empty(), "without a pair the transition does not move");
}
