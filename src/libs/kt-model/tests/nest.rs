//! Tests 423-426: nested sequences — the model and its guards.
//!
//! Nesting is a piece of the edit collapsed into ONE event. The reference lives inside
//! the project (owner’s decision R.50), so the document stops being a flat
//! list and becomes a tree. Everything checked here is the price of this
//! transition: the tree must remain a tree (no cycles), finite
//! (the depth is bounded), unambiguous (`ClipId` is unique across the document), and
//! closed under reels (no model reference crosses the reel boundary).

use kt_model::*;
use kt_time::Tick;

const LEN: Tick = Tick(90_000);

fn settings() -> ProjectSettings {
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

fn clip(id: u64, media: u64, start: Tick, dur: Tick) -> ClipEvent {
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
        start,
        duration: dur,
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

fn base() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "synth:sine:440"));
    p
}

/// A reel with one event inside.
fn seq(id: u64, name: &str, inner: Vec<ClipEvent>) -> Sequence {
    let mut s = Sequence::new(MediaId(id), name);
    s.clips = inner;
    s
}

// ---- 423. cycle ---------------------------------------------------------------

#[test]
fn t423_cycle_is_refused_and_names_the_path() {
    // a reel placed inside itself
    let mut p = base();
    p.sequences.push(seq(100, "Intro", vec![clip(10, 100, Tick(0), LEN)]));
    let err = p.validate().expect_err("a reel inside itself must be rejected");
    let text = err.to_string();
    assert!(
        matches!(err, ProjectError::SequenceCycle(..)),
        "self-nesting was not recognized as a cycle: {err}"
    );
    assert!(
        text.matches("Intro").count() >= 2,
        "the error text must name the NAMES and show the path, not the addresses: {text}"
    );

    // a cycle of TWO reels that are not placed in the edit ANYWHERE. The traversal goes from all
    // vertices: from the root such a pair is unreachable, but a file with it would be saved and
    // would not open.
    let mut p = base();
    p.sequences.push(seq(100, "Intro", vec![clip(10, 101, Tick(0), LEN)]));
    p.sequences.push(seq(101, "Titles", vec![clip(11, 100, Tick(0), LEN)]));
    assert!(p.clips.is_empty(), "the pair is not used in the edit — that is the whole point of the case");
    let err = p.validate().expect_err("the cycle A-B must be rejected even outside the edit");
    let text = err.to_string();
    assert!(matches!(err, ProjectError::SequenceCycle(..)), "cycle A-B not recognized: {err}");
    assert!(
        text.contains("Intro") && text.contains("Titles"),
        "both reels must be in the cycle path: {text}"
    );

    // ...but a rhombus (one reel used by two different ones) is not a cycle
    let mut p = base();
    p.sequences.push(seq(100, "Piece", vec![clip(10, 1, Tick(0), LEN)]));
    p.sequences.push(seq(101, "Left", vec![clip(11, 100, Tick(0), LEN)]));
    p.sequences.push(seq(102, "Right", vec![clip(12, 100, Tick(0), LEN)]));
    p.clips.push(clip(1, 101, Tick(0), LEN));
    p.clips.push(clip(2, 102, LEN, LEN));
    p.validate().expect("a diamond is not a cycle: one reel legally stands in two places");
    assert_eq!(
        p.sequence_uses(MediaId(100)),
        2,
        "“how many places change together” is counted wrongly"
    );
}

// ---- 424. depth --------------------------------------------------------------

#[test]
fn t424_depth_is_capped_and_the_cap_names_the_sequence() {
    // a chain of MAX_NEST_DEPTH reels is legitimate
    let chain = |levels: u64| {
        let mut p = base();
        for i in 0..levels {
            let inner = if i == 0 { 1 } else { 100 + i - 1 };
            p.sequences.push(seq(
                100 + i,
                &format!("U{i}"),
                vec![clip(10 + i, inner, Tick(0), LEN)],
            ));
        }
        p.clips.push(clip(1, 100 + levels - 1, Tick(0), LEN));
        p
    };
    chain(MAX_NEST_DEPTH as u64).validate().expect("the depth limit must be legal");

    let err =
        chain(MAX_NEST_DEPTH as u64 + 1).validate().expect_err("one level deeper than the limit — rejection");
    assert!(matches!(err, ProjectError::SequenceTooDeep(..)), "depth not recognized: {err}");
    assert!(
        err.to_string().contains(&format!("U{}", MAX_NEST_DEPTH)),
        "the rejection must name the reel where the tree became too deep: {err}"
    );

    // depth is also counted for a reel that is placed NOWHERE: the complaint must
    // arise where it was assembled, not a week later at the drag
    let mut p = chain(MAX_NEST_DEPTH as u64 + 1);
    p.clips.clear();
    assert!(
        matches!(p.validate(), Err(ProjectError::SequenceTooDeep(..))),
        "a too-deep reel in the bin is skipped — on the very first placement \
         the user would have received a rejection out of nowhere"
    );
}

// ---- 425. one address space, closure of reels -------------------------------

#[test]
fn t425_document_wide_ids_and_closed_reels() {
    // a clip LOOKS INTO a reel — this is legitimate and is not an “unknown media”
    let mut p = base();
    p.sequences.push(seq(100, "Piece", vec![clip(10, 1, Tick(0), LEN)]));
    p.clips.push(clip(1, 100, Tick(0), LEN));
    p.validate().expect("a nested-reel event must be legal");
    assert!(p.is_nest(&p.clips[0]), "nesting not recognized");
    assert!(matches!(p.source(MediaId(100)), Some(Source::Seq(_))), "reel not found at the address");
    assert!(matches!(p.source(MediaId(1)), Some(Source::File(_))), "file not found at the address");
    assert!(p.source(MediaId(7)).is_none(), "a nonexistent address must be None");

    // `ClipId` is unique across the WHOLE document: `Retained` in kt-otio is a global
    // map by id, and matching ids would silently swap material
    let mut p = base();
    p.sequences.push(seq(100, "Piece", vec![clip(10, 1, Tick(0), LEN)]));
    p.clips.push(clip(10, 100, Tick(0), LEN));
    assert!(
        matches!(p.validate(), Err(ProjectError::DuplicateClipId(ClipId(10)))),
        "an identical event id in the root and in a reel is skipped"
    );

    // the address of a reel and the address of a file — ONE space
    let mut p = base();
    p.sequences.push(seq(1, "Collision", vec![clip(10, 1, Tick(0), LEN)]));
    assert!(
        matches!(p.validate(), Err(ProjectError::DuplicateMediaId(MediaId(1)))),
        "a reel with the address of an existing file is skipped"
    );

    // the name of a reel is non-empty: a card without a caption in the bin is indistinguishable from its neighbour
    let mut p = base();
    p.sequences.push(seq(100, "  ", vec![clip(10, 1, Tick(0), LEN)]));
    assert!(
        matches!(p.validate(), Err(ProjectError::SequenceUnnamed(..))),
        "a reel without a name passed"
    );

    // no model reference crosses the reel boundary: a target of a morpher layer
    // in a foreign reel would return None from joint(), and the layer would silently stop
    // acting
    let mut p = base();
    p.ensure_tracks(LaneKind::Morph, 1);
    let mut s = seq(100, "Piece", vec![clip(10, 1, Tick(0), LEN)]);
    s.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: LEN,
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: Default::default(),
        enabled: true,
        timed: true,
        strength: 1.0,
        fade_in: Tick(0),
        fade_out: Tick(0),
    });
    s.ensure_tracks(LaneKind::Morph, 1);
    p.sequences.push(s);
    p.clips.push(clip(1, 100, Tick(0), LEN));
    assert!(
        matches!(p.validate(), Err(ProjectError::MorphUnknownTarget(..))),
        "a morpher layer reached an event of a FOREIGN reel — a transition/layer across the \
         boundary silently stops acting instead of breaking visibly"
    );

    // the high bit of the address is given to reels in the engine: the file there is the shared cache key
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(NEST_TAG | 5), "synth:ramp"));
    assert!(
        matches!(p.validate(), Err(ProjectError::MediaIdTooLarge(..))),
        "a media address with the high bit set is skipped"
    );
}

// ---- 426. a reel does not lag behind the root -------------------------------

/// A risk the compiler does not close by itself: `Sequence` is a partial duplicate of
/// `Project`, and a new content field added to the root has NOTHING
/// to catch it in the reel. Destructuring both structures exhaustively: a new
/// field breaks the test build, and at that moment the decision is made — it is content
/// of the reel (add it to `Sequence`) or a property of the document (add it here to the list of
/// exceptions with a reason).
#[test]
fn t426_sequence_does_not_lag_behind_the_root() {
    let p = Project::new(settings());
    let Project {
        // --- reel content: must be in Sequence too ---
        clips: _,
        transitions: _,
        morphers: _,
        cameras: _,
        // object tracks (R.62, change B) and legacy counters are read-only
        tracks: _,
        legacy_video_lanes: _,
        legacy_audio_lanes: _,
        legacy_morph_lanes: _,
        legacy_camera_lanes: _,
        legacy_lane_order: _,
        overlap_layers: _,
        // public parameters (R.59, stage 3) — the CONTENT of the reel: the root has the field
        // only because entry inside was done by exchange
        published: _,
        // --- properties of the DOCUMENT: the reel has none of its own by the owner’s decision
        // “the nested item inherits the settings of its parent” (R.50) ---
        format_version: _,
        settings: _,
        media: _,
        sequences: _,
        // --- convenience and markup: live only in the root ---
        markers: _,
        areas: _,
        region: _,
        loop_enabled: _,
        timeline_end: _,
        trim_stretches_auto: _,
        ui: _,
        // --- foreign OTIO raw material: does not get inside the reel (A5 places foreign
        // Stacks as real reels, not placeholders) ---
        foreign: _,
    } = p;

    let s = Sequence::new(MediaId(100), "Piece");
    let Sequence {
        id: _,
        name: _,
        clips: _,
        transitions: _,
        morphers: _,
        cameras: _,
        tracks: _,
        legacy_video_lanes: _,
        legacy_audio_lanes: _,
        legacy_morph_lanes: _,
        legacy_camera_lanes: _,
        legacy_lane_order: _,
        overlap_layers: _,
        published: _,
        // its own raster and fps (R.62): a property of the REEL; for the root their role
        // is played by `settings`
        raster: _,
        fps: _,
    } = s;

    // the defaults of the reel coincide with the defaults of the root: an empty reel and an empty
    // project must behave the same, otherwise “collapsing a piece” would change
    // the picture by itself
    let s = Sequence::new(MediaId(100), "Piece");
    let p = Project::new(settings());
    assert_eq!(
        (s.lanes(LaneKind::Video), s.lanes(LaneKind::Audio)),
        (p.lanes(LaneKind::Video), p.lanes(LaneKind::Audio))
    );
    assert_eq!(
        (s.lanes(LaneKind::Morph), s.lanes(LaneKind::Camera)),
        (p.lanes(LaneKind::Morph), p.lanes(LaneKind::Camera))
    );
    assert_eq!(s.overlap_layers, p.overlap_layers);
    assert_eq!(s.end(), Tick(0), "an empty reel must end at zero");
}

// ---- 427-430. commands and gestures (R.50, item 380) -----------------------------

/// Three events in a row on one track, a crossfade between the second and the third,
/// and one “outside” on the left to which the second is joined by a transition.
fn montage() -> Project {
    let mut p = base();
    p.media.push(MediaRef::new(MediaId(2), "synth:ramp"));
    for (i, start) in (0..4).map(|i| (i, Tick(i * LEN.0))) {
        // the second file stands EXACTLY on what will move inside the reel: this checks
        // that the media holds the whole tree, not only the root
        let mut c = clip(10 + i as u64, if (1..=2).contains(&i) { 2 } else { 1 }, start, LEN);
        c.source_in = Tick(1_500);
        p.clips.push(c);
    }
    p
}

/// A snapshot of “how it looks and sounds”: ids are not compared — on unfolding
/// the events legitimately receive fresh addresses, while the PICTURE must remain
/// identical.
fn shape(p: &Project) -> Vec<(ClipKind, u32, i64, i64, i64, u64, bool)> {
    let mut v: Vec<_> = p
        .clips
        .iter()
        .map(|c| (c.kind, c.lane, c.start.0, c.duration.0, c.source_in.0, c.media.0, c.reversed))
        .collect();
    v.sort_by_key(|x| (x.0 as u8, x.1, x.2));
    v
}

/// Transitions as GEOMETRY: the position of the joint and both handles.
fn joints(p: &Project) -> Vec<(i64, i64, i64)> {
    let mut v: Vec<_> = p
        .transitions
        .iter()
        .filter_map(|t| t.joint(p).map(|j| (j.0, t.in_offset.0, t.out_offset.0)))
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn t427_collapse_then_expand_is_identity_in_one_undo_step() {
    let mut p = montage();
    // a crossfade INSIDE the future reel: it must move inside as a whole and
    // return to the same joint — otherwise “collapse, unfold” loses work
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(11)),
        b: Some(ClipId(12)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: Vec::new(),
        invert: false,
    });
    p.validate().expect("the fixture must be legal");
    let before_shape = shape(&p);
    let before_joints = joints(&p);
    let mut m = TimelineModel::new(p);

    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Piece", n.ids());
    m.apply_batch(cmds).expect("collapse must be legal");
    let seq_id = rep.seq.expect("reel not created");

    // the root: two events remained, the third became an instance
    let p = m.project();
    assert_eq!(p.clips.len(), 3, "after collapsing, the root has the wrong number of events");
    let inst = p.clips.iter().find(|c| c.media == seq_id).expect("no instance");
    assert_eq!((inst.start.0, inst.duration.0), (LEN.0, 2 * LEN.0), "instance window is wrong");
    assert_eq!(p.sequence(seq_id).map(|s| s.clips.len()), Some(2), "the reel does not have two events");
    assert_eq!(
        p.sequence(seq_id).map(|s| s.transitions.len()),
        Some(1),
        "the crossfade between collapsed events did not move inside the reel"
    );
    assert!(p.transitions.is_empty(), "the crossfade stayed at the root too — now it is in two places");
    // the ClipIds are PRESERVED: the same events moved, rather than being born anew —
    // transitions, link groups, morpher targets and FX state rest on this
    let inner: Vec<u64> = p.sequence(seq_id).unwrap().clips.iter().map(|c| c.id.0).collect();
    assert_eq!(inner, vec![11, 12], "ids of the collapsed events changed: {inner:?}");

    // ...and ONE history step
    m.undo().expect("collapse must be undoable in one step");
    assert_eq!(shape(m.project()), before_shape, "undo did not restore the edit");
    assert!(m.project().sequences.is_empty(), "undo left the reel");

    // unfolded back — identity
    m.redo().expect("redo");
    let (cmds, rep) = ops::expand_instance(
        m.project(),
        m.project().clips.iter().find(|c| c.media == seq_id).unwrap().id,
        n.ids(),
    );
    assert!(!rep.chain_dropped && !rep.level_dropped, "there was nothing to remove: {rep:?}");
    m.apply_batch(cmds).expect("expanding must be legal");
    assert_eq!(
        shape(m.project()),
        before_shape,
        "collapsed and expanded — the edit must be THE SAME: a nested reel with an empty \
         chain is semantically identical to its contents (settings \
         are inherited, there is no intermediate raster)"
    );
    assert_eq!(joints(m.project()), before_joints, "transitions did not return to their seams");
}

#[test]
fn t428_expand_drops_the_instance_chain_and_says_so() {
    let mut m = TimelineModel::new(montage());
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Piece", n.ids());
    m.apply_batch(cmds).unwrap();
    let seq_id = rep.seq.unwrap();
    let inst = m.project().clips.iter().find(|c| c.media == seq_id).unwrap().id;

    // A CHAIN on a nesting is legitimate since wave B: it is applied to the ASSEMBLED
    // picture — this is why the nesting was introduced
    m.apply(Command::SetFx { id: inst, fx: vec![FxInstance::Gain { db: 3.0, pan: 0.0 }] })
        .expect("a chain on a nested reel is legal");
    m.apply(Command::SetOpacity { id: inst, opacity: 0.5 }).expect("nesting level is legal");
    // ...AND REVERSE WITH STRETCHING TOO (R.55, C4).
    // Until this round they
    // were rejected: the sub-mix was read strictly in order, while both need to be
    // read differently. Now the sub-mix has a producer with a pre-roll, and
    // reverse with a warp act on the SAMPLES of the assembled mix — exactly
    // as on the samples of a file. This was the owner’s complaint: “reverse does not work
    // on a composition”, “slowedandreverb does not fire on precompositions”.
    m.apply(Command::SetReversed { id: inst, reversed: true })
        .expect("reverse of a nested reel must pass");
    let mut fx = m.project().clips.iter().find(|c| c.id == inst).unwrap().fx.clone();
    fx.push(FxInstance::slowed_reverb(0.82, 0.1, 85.0));
    m.apply(Command::SetFx { id: inst, fx })
        .expect("a stretching chain on a nested reel must pass");
    m.project().validate().expect("a document with a reversed and slowed nested reel is legal");

    let (cmds, rep) = ops::expand_instance(m.project(), inst, n.ids());
    assert!(
        rep.chain_dropped && rep.level_dropped,
        "expanding stayed silent about the removed chain and level: {rep:?}. Outside, they \
         applied to the ASSEMBLED picture; inside there is no such place — silently \
         scattering them across pieces means computing something different"
    );
    // REVERSE and SPEED are named SEPARATELY: “reverse of the assembled piece ≠
    // reverse of each piece” — this is the reverse analogue of law G (t438), and dumping it
    // into a general heap with the level means not saying the main thing
    assert!(rep.reverse_dropped, "the removed reverse was not named: {rep:?}");
    assert!(rep.warp_dropped, "the removed speed was not named: {rep:?}");
    m.apply_batch(cmds).unwrap();
    assert!(
        m.project().clips.iter().all(|c| c.fx.is_empty()),
        "the instance chain nevertheless spread across the pieces"
    );
    assert!(
        m.project().clips.iter().all(|c| !c.reversed),
        "the instance reverse spread across the pieces — this is a DIFFERENT edit"
    );
}

#[test]
fn t429_a_used_sequence_refuses_to_leave() {
    let mut m = TimelineModel::new(montage());
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Piece", n.ids());
    m.apply_batch(cmds).unwrap();
    let seq_id = rep.seq.unwrap();

    assert_eq!(
        m.apply(Command::RemoveSequence { id: seq_id }),
        Err(CommandError::SequenceInUse(seq_id, 1)),
        "an occupied reel was silently deleted — events would hang on a nonexistent address"
    );
    assert!(
        ops::remove_unused_sequences(m.project()).is_empty(),
        "an occupied reel was swept up in the cleanup of unused ones"
    );

    // they put it in the SECOND place and detached one of them
    let inst = m.project().clips.iter().find(|c| c.media == seq_id).unwrap().clone();
    let mut twin = inst.clone();
    twin.id = ClipId(n.clip);
    n.clip += 1;
    twin.link_group = None;
    twin.start = Tick(10 * LEN.0);
    m.apply(Command::AddClip(twin.clone())).expect("a second placement of the reel is legal");
    assert_eq!(m.project().sequence_uses(seq_id), 2, "the second place was not counted");

    let cmds = ops::make_independent(m.project(), twin.id, "Piece 2", n.ids());
    m.apply_batch(cmds).expect("unlinking must be legal");
    assert_eq!(m.project().sequences.len(), 2, "its own reel did not appear");
    assert_eq!(m.project().sequence_uses(seq_id), 1, "the first reel was not freed");
    // the content is the same, but the addresses of the events are FRESH (otherwise the kt-otio maps
    // would silently swap material between reels)
    let a: Vec<u64> = m.project().sequences[0].clips.iter().map(|c| c.id.0).collect();
    let b: Vec<u64> = m.project().sequences[1].clips.iter().map(|c| c.id.0).collect();
    assert_eq!(a.len(), b.len(), "the copy of the reel lost events");
    assert!(a.iter().all(|x| !b.contains(x)), "the event addresses of the copy matched the original");

    // freed it — and the cleanup sees it
    m.apply(Command::RemoveClip { id: twin.id }).unwrap();
    let cmds = ops::remove_unused_sequences(m.project());
    assert_eq!(cmds.len(), 1, "the freed reel was not swept into cleanup");
    m.apply_batch(cmds).expect("cleanup must pass");
}

#[test]
fn t430_outer_transition_is_rehung_and_media_is_held_by_the_whole_tree() {
    let mut m = TimelineModel::new(montage());
    // a crossfade between OUTSIDE (10) and the one that will move inside (11)
    m.apply(Command::AddTransition(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(10)),
        b: Some(ClipId(11)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: Vec::new(),
        invert: false,
    }))
    .expect("a crossfade at a seam is legal");

    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Piece", n.ids());
    assert_eq!(rep.rehung, 1, "a transition with one end outside was not re-hung: {rep:?}");
    m.apply_batch(cmds).unwrap();

    let p = m.project();
    let inst = p.clips.iter().find(|c| c.media == rep.seq.unwrap()).unwrap();
    let t = p.transitions.iter().find(|t| t.id == TransitionId(1)).expect(
        "a crossfade with a collapsed piece died together with the vanished end — \
         the user silently lost work",
    );
    assert_eq!((t.a, t.b), (Some(ClipId(10)), Some(inst.id)), "the transition was re-hung to the wrong place");
    assert_eq!(t.joint(p), Some(inst.start), "the seam of the re-hung transition moved");

    // the media holds the WHOLE TREE: a file that remains only inside a reel is not
    // deleted — otherwise the events would hang, and this would surface a week later
    assert!(
        p.clips.iter().all(|c| c.media != MediaId(2)),
        "no events on this file must remain at the root — that is the whole case"
    );
    assert_eq!(
        m.apply(Command::RemoveMedia { id: MediaId(2) }),
        Err(CommandError::MediaInUse(MediaId(2))),
        "a file standing only INSIDE a reel was silently deleted"
    );
}

// ---- 453-455 (R.50, B1): editing context -----------------------------------

/// Project: a root event and a reel with two events inside.
fn with_reel() -> (TimelineModel, MediaId) {
    let mut p = base();
    p.media.push(MediaRef::new(MediaId(2), "synth:ramp"));
    let mut s = Sequence::new(MediaId(100), "Piece");
    s.clips.push(clip(20, 1, Tick(0), LEN));
    s.clips.push(clip(21, 2, LEN, LEN));
    p.sequences.push(s);
    p.clips.push(clip(1, 1, Tick(0), LEN));
    let mut inst = clip(10, 100, LEN, LEN + LEN);
    inst.source_in = Tick(0);
    p.clips.push(inst);
    p.validate().expect("the fixture is legitimate");
    (TimelineModel::new(p), MediaId(100))
}

#[test]
fn t453_commands_edit_the_reel_you_are_standing_in() {
    let (mut m, seq) = with_reel();
    let root_before: Vec<i64> = m.document().clips.iter().map(|c| c.start.0).collect();

    m.set_editing(Some(seq));
    assert_eq!(m.editing(), Some(seq));
    // WORKING VIEW: the reel’s content is at hand, and the whole interface works
    // with it without knowing about nestings at all
    assert_eq!(m.project().clips.len(), 2, "inside the reel ITS events must be present");
    assert_eq!(m.document().clips.len(), 2, "the document must remain a document");

    // an edit INSIDE: the root did not budge, the reel changed
    m.apply(Command::MoveClip { id: ClipId(20), new_start: Tick(LEN.0 / 2), new_lane: 0 })
        .expect("a move inside a reel is legal");
    assert_eq!(
        m.document().clips.iter().map(|c| c.start.0).collect::<Vec<_>>(),
        root_before,
        "an edit INSIDE the reel moved to the root — the person looks at one place, while \
         another one changes"
    );
    assert_eq!(
        m.document().sequence(seq).unwrap().clips[0].start.0,
        LEN.0 / 2,
        "the edit did not reach the reel"
    );

    // the events of the ROOT are neither visible nor addressable from inside
    assert!(
        matches!(
            m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(0), new_lane: 0 }),
            Err(CommandError::ClipNotFound(ClipId(1)))
        ),
        "from inside the reel a ROOT event could be addressed"
    );

    // we exit — and the root is at hand again
    m.set_editing(None);
    m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(LEN.0 * 9), new_lane: 0 })
        .expect("from outside the root is editable again");
    assert_eq!(
        m.document().sequence(seq).unwrap().clips.len(),
        2,
        "the reel was harmed by an edit of the root"
    );
}

#[test]
fn t454_undo_takes_you_back_to_where_the_edit_was() {
    let (mut m, seq) = with_reel();
    // an edit OUTSIDE
    m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(LEN.0 * 9), new_lane: 0 }).unwrap();
    // an edit INSIDE
    m.set_editing(Some(seq));
    m.apply(Command::MoveClip { id: ClipId(20), new_start: Tick(LEN.0 / 2), new_lane: 0 }).unwrap();
    // we exit outward and press Ctrl+Z
    m.set_editing(None);
    m.undo().unwrap();
    assert_eq!(
        m.editing(),
        Some(seq),
        "Ctrl+Z undid the edit INSIDE the reel, but showed the root — for the person this is \
         “pressed, and nothing happened”"
    );
    assert_eq!(
        m.document().sequence(seq).unwrap().clips[0].start.0,
        0,
        "the edit inside was not undone"
    );

    // the next step — an edit outside, and the context must move outward
    m.undo().unwrap();
    assert_eq!(m.editing(), None, "Ctrl+Z before the root edit left us inside the reel");
    assert_eq!(m.document().clips[0].start.0, 0, "the root edit was not undone");

    // redo leads back by the same path
    m.redo().unwrap();
    assert_eq!(m.editing(), None, "redo of the root edit moved us inside");
    m.redo().unwrap();
    assert_eq!(m.editing(), Some(seq), "redo of the edit inside did not return us to the reel");
}

#[test]
fn t455_editing_context_survives_and_yields() {
    let (mut m, seq) = with_reel();
    m.set_editing(Some(seq));

    // the context is a workplace, not work: it lives in the MODEL, not in the
    // project, so there is no way for it to get into the file
    let dump = format!("{:?}", m.project());
    assert!(!dump.contains("editing"), "the editing context leaked into the project");

    // the reel vanished from under one’s hands (undo of a collapse) — we exit to the root and
    // do not remain nowhere
    m.set_editing(None);
    let cmds = ops::remove_unused_sequences(m.project());
    assert!(cmds.is_empty(), "the reel is in use — there is nothing to remove");
    m.apply(Command::RemoveClip { id: ClipId(10) }).unwrap();
    let cmds = ops::remove_unused_sequences(m.project());
    m.apply_batch(cmds).unwrap();
    m.set_editing(Some(seq));
    assert_eq!(m.editing(), None, "entering a vanished reel left us nowhere");
    assert_eq!(
        m.project().clips.len(),
        m.document().clips.len(),
        "a reel without an address replaced the root"
    );
}

// ---- R.55, C3: collapsing loses nothing ---------------------------------

/// Test 557: a morpher layer that moved into the reel covers THE SAME that it covered
/// from outside.
///
/// **The owner’s complaint verbatim:** “When creating a composition, if there was
/// a morpher in the region, for some reason it does not get there, and probably the cameras
/// too and the generators, check it”.
///
/// The layer got inside and DIED there, which is worse than not getting in: the reel
/// had no `lane_order`, and in the default order (video → audio → morphers →
/// cameras) the morph rows come LAST. The mode “everything below” requires
/// targets BELOW the layer — there was nothing beneath it.
///
/// The test asserts the CONSEQUENCE: the set of covered events, not the presence of a field.
#[test]
fn t557_a_layer_inside_covers_what_it_covered_outside() {
    let mut p = base();
    p.clips.push(clip(11, 1, Tick::ZERO, LEN));
    p.clips.push(clip(12, 1, LEN, LEN));
    p.ensure_tracks(LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick::ZERO,
        duration: Tick(2 * LEN.0),
        targets: MorphTargets::Below,
        patch: MorphPatch {
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            ..Default::default()
        },
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    // the morpher row ABOVE — otherwise “everything below” covers nothing
    // both outside and inside, and there would be nothing to compare with
    p.set_lane_order(&[(LaneKind::Morph, 0), (LaneKind::Video, 0)]);
    p.validate().expect("the fixture is legitimate");

    // what the layer covered OUTSIDE: both events got the mirror
    let mirrored = |q: &Project| -> usize {
        q.effective_clips()
            .iter()
            .filter(|c| c.fx.iter().any(|f| matches!(f, FxInstance::Mirror { .. })))
            .count()
    };
    assert_eq!(mirrored(&p), 2, "the scene degenerated: the layer covers nothing, and outside too");

    let mut m = TimelineModel::new(p);
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Piece", n.ids());
    m.apply_batch(cmds).expect("collapse must be legal");
    let seq_id = rep.seq.expect("reel not created");

    assert_eq!(rep.moved_layers, 1, "the layer must move inside and be NAMED");
    let s = m.project().sequence(seq_id).expect("reel");
    assert_eq!(s.morphers.len(), 1, "no layer inside the reel");
    assert!(
        m.project().morphers.is_empty(),
        "the layer stayed at the root TOO: the same thing would be overlaid twice"
    );
    // the ORDER OF ROWS moved along with it
    let rows = s.rows();
    let mine = rows.iter().position(|&k| k == (LaneKind::Morph, 0)).expect("layer row");
    let his = rows.iter().position(|&k| k == (LaneKind::Video, 0)).expect("video row");
    assert!(mine < his, "inside the reel the layer ended up BELOW the target: {rows:?}");

    // CONSEQUENCE: inside the reel the layer covers the same two events
    let inside = m.project().effective_reel(Some(seq_id));
    let n_mirrored = inside
        .iter()
        .filter(|c| c.fx.iter().any(|f| matches!(f, FxInstance::Mirror { .. })))
        .count();
    assert_eq!(n_mirrored, 2, "inside the reel the layer covered nothing — it moved and died");
}

/// Test 558: cameras travel into the reel together with the edit.
///
/// `Sequence` has held `cameras` and `camera_lanes` since R.50, but collapsing did not fill
/// them — the field existed, and there was no one to put anything into it.
/// An event with `Pan3d` that rode on a camera, inside a collapsed reel SILENTLY lost
/// it: `resolve_cam3d` with an empty list returns the default camera.
#[test]
fn t558_cameras_travel_into_the_sequence() {
    let mut p = base();
    p.clips.push(clip(11, 1, Tick::ZERO, LEN));
    p.clips.push(clip(12, 1, LEN, LEN));
    p.ensure_tracks(LaneKind::Camera, 1);
    p.cameras.push(CameraEvent {
        ease: true,
        id: CameraId(7),
        lane: 0,
        start: Tick::ZERO,
        duration: Tick(2 * LEN.0),
        name: "Dolly".into(),
        cam: Default::default(),
        keyframes: Vec::new(),
        auto: Vec::new(),
    });
    p.validate().expect("the fixture is legitimate");

    let mut m = TimelineModel::new(p);
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Piece", n.ids());
    m.apply_batch(cmds).expect("collapse must be legal");
    let seq_id = rep.seq.expect("reel");

    assert_eq!(rep.moved_cameras, 1, "the camera must move inside and be NAMED");
    let s = m.project().sequence(seq_id).expect("reel");
    assert_eq!(s.cameras.len(), 1, "no camera inside the reel");
    assert_eq!(s.cameras[0].id, CameraId(7), "the camera was born anew instead of moving");
    assert_eq!(s.cameras[0].start, Tick::ZERO, "the camera was not rebased to the reel axis");
    assert!(
        s.lanes(LaneKind::Camera) >= 1,
        "the camera row inside the reel was not created — the camera is not visible"
    );
    assert!(
        s.rows().contains(&(LaneKind::Camera, 0)),
        "no camera row in the reel row order: {:?}",
        s.rows()
    );
    assert!(m.project().cameras.is_empty(), "the camera stayed at the root TOO");
    m.project().validate().expect("the document after collapse must be legal");
}

/// Test 559: one can collapse UNDER a layer BY AREA, and the layer stays outside.
///
/// Reference modes inside a reel are forbidden (`MorphRefInsideSequence`):
/// an area and a marker are navigation BY PROJECT, they live in the root. And
/// `block_extras` put such a layer INSIDE — and `validate` rejected the whole
/// batch. Collapsing a selection under a layer “by area” was impossible AT ALL, with
/// an English error in the status bar.
#[test]
fn t559_collapsing_under_an_area_layer_is_possible() {
    let mut p = base();
    p.clips.push(clip(11, 1, Tick::ZERO, LEN));
    p.clips.push(clip(12, 1, LEN, LEN));
    p.ensure_tracks(LaneKind::Morph, 1);
    p.areas.push(MarkedArea {
        name: "Piece".into(),
        start: Tick::ZERO,
        duration: Tick(2 * LEN.0),
        lanes: Vec::new(),
        color: Default::default(),
    });
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick::ZERO,
        duration: Tick(2 * LEN.0),
        targets: MorphTargets::Areas { names: vec!["Piece".into()] },
        patch: MorphPatch {
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            ..Default::default()
        },
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    p.validate().expect("the fixture is legitimate");

    let mut m = TimelineModel::new(p);
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Piece", n.ids());
    m.apply_batch(cmds).expect("collapsing under a reference layer must be possible");
    let seq_id = rep.seq.expect("reel");

    assert_eq!(rep.kept_ref_layers, 1, "the layer left outside must be NAMED");
    assert_eq!(m.project().morphers.len(), 1, "the reference layer must stay at the root");
    assert!(
        m.project().sequence(seq_id).is_some_and(|s| s.morphers.is_empty()),
        "the reference layer moved inside — the document stopped being legal"
    );
    m.project().validate().expect("the document after collapse must be legal");
}

// ---- R.55, C5: unfolding a reel in place --------------------------------------

/// The scene for unfolding: a root with two events and a transition, a reel with two
/// events, its own transition, a morpher layer and a camera, and ITS OWN
/// row order — that is, everything that unfolding must return to its place.
fn expandable() -> Project {
    let mut p = base();
    p.clips.push(clip(1, 1, Tick::ZERO, LEN));
    p.clips.push(clip(2, 1, Tick(3 * LEN.0), LEN));
    p.ensure_tracks(LaneKind::Video, 1);
    p.ensure_tracks(LaneKind::Audio, 1);

    let mut s = Sequence::new(MediaId(100), "Piece");
    s.clips = vec![clip(11, 1, Tick::ZERO, LEN), clip(12, 1, LEN, LEN)];
    s.clips[1].lane = 1;
    s.ensure_tracks(LaneKind::Video, 2);
    s.ensure_tracks(LaneKind::Audio, 1);
    s.ensure_tracks(LaneKind::Morph, 1);
    s.ensure_tracks(LaneKind::Camera, 1);
    s.morphers.push(MorpherEvent {
        id: MorphId(5),
        lane: 0,
        start: Tick(LEN.0 / 2),
        duration: LEN,
        targets: MorphTargets::Below,
        patch: MorphPatch {
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            ..Default::default()
        },
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    s.cameras.push(CameraEvent {
        ease: true,
        id: CameraId(7),
        lane: 0,
        start: Tick::ZERO,
        duration: Tick(2 * LEN.0),
        name: "Dolly".into(),
        cam: Default::default(),
        keyframes: Vec::new(),
        auto: Vec::new(),
    });
    // ITS OWN row order: the morph row on top, otherwise the layer covers nothing
    s.reorder_rows(&[
        (LaneKind::Morph, 0),
        (LaneKind::Video, 0),
        (LaneKind::Video, 1),
        (LaneKind::Audio, 0),
        (LaneKind::Camera, 0),
    ]);
    p.sequences.push(s);

    let mut inst = clip(20, 100, Tick(LEN.0), Tick(2 * LEN.0));
    inst.source_in = Tick::ZERO;
    p.clips.push(inst);
    p.validate().expect("a scene for expansion is legal");
    p
}

/// Test 566 (R.55, C5): UNFOLD AND COLLAPSE — identity.
///
/// The foundation of the whole stage. Unfolding in place is done by the same technique as
/// the editing context in R.50 (item 392): by moving the CONTENT, not by a parameter
/// in sixty places — only with a shift in time and in track number. The whole interface keeps
/// working with the root and knows nothing about the band; the price of this is precisely this identity,
/// and it must be BYTE-FOR-BYTE.
///
/// Red here means the design is wrong, not that the test is picky.
#[test]
fn t566_expanding_and_collapsing_a_band_is_identity() {
    let before = expandable();
    let b = Band::of(&before, ClipId(20)).expect("band for an instance");
    assert_eq!(b.dt, Tick(LEN.0), "band shift computed wrongly");

    let mut p = before.clone();
    p.merge_reel(&b);
    // the events of the reel are PHYSICALLY in the root, with an offset
    assert!(p.sequence(MediaId(100)).is_some_and(|s| s.clips.is_empty()), "the reel body did not become empty");
    let inner = p.clips.iter().find(|c| c.id == ClipId(11)).expect("the reel event is not at the root");
    assert_eq!(inner.start, Tick(LEN.0), "band time not shifted to the root axis");
    assert_eq!(inner.lane, 1, "the band track did not go to the tail of the space");
    assert!(p.clips.iter().any(|c| c.id == ClipId(20)), "the instance must stay at the root");
    // and the rows of the band stand as an UNBROKEN block under the row of the instance
    let rows = p.rows();
    let at = rows.iter().position(|&k| k == (LaneKind::Video, 0)).expect("instance row");
    // the block stands IN ITS OWN ORDER (the morph row on top — as the person set it up inside the reel),
    // only the numbers are shifted to the tail of the space
    let want: Vec<(LaneKind, u32)> = before
        .sequence(MediaId(100))
        .unwrap()
        .rows()
        .into_iter()
        .map(|(k, l)| (k, l + b.base.get(k)))
        .collect();
    for (j, r) in want.iter().enumerate() {
        assert_eq!(rows.get(at + 1 + j), Some(r), "band rows did not form a block: {rows:?}");
    }

    p.unmerge_reel(&b).expect("reassembly must pass");
    assert_eq!(p, before, "expand and reassemble is NOT an identity, and the whole technique is wrong");
}

/// Test 567 (R.55, C5): one cannot drag across the band’s boundary.
///
/// This is the answer to the risk that item 392 names in words “skip one, and the edit
/// silently drifts into the root”. Addressing of edits is not needed: there is nothing
/// to drift into physically, and the assembly catches the attempt — and the batch is rejected WHOLE,
/// rather than “corrected afterwards”.
#[test]
fn t567_an_event_cannot_change_reels_inside_a_band() {
    let p0 = expandable();
    let b = Band::of(&p0, ClipId(20)).expect("band");

    // an event of the BAND was dragged onto a root track
    let mut p = p0.clone();
    p.merge_reel(&b);
    if let Some(c) = p.clips.iter_mut().find(|c| c.id == ClipId(11)) {
        c.lane = 0;
    }
    let mut q = p.clone();
    assert!(
        q.unmerge_reel(&b).is_ok(),
        "an event that moved to the root must simply stay at the root — it is legal there"
    );
    assert!(
        q.sequence(MediaId(100)).is_some_and(|s| s.clips.len() == 1),
        "exactly one event must move out of the reel"
    );

    // but an INSTANCE inside its own reel — never
    let mut p = p0.clone();
    p.merge_reel(&b);
    if let Some(c) = p.clips.iter_mut().find(|c| c.id == ClipId(20)) {
        c.lane = 1;
    }
    assert!(
        matches!(p.unmerge_reel(&b), Err(ProjectError::BandEscape(ClipId(20)))),
        "the instance moved inside its own reel — the reel would contain itself"
    );

    // a track inserted INSIDE the unfolding block (this is what `InsertTrack` does
    // while the band is open; `after_batch` grows `len`) goes to the REEL,
    // while the rows of the root stay as before (R.62: the block is part of the vector of tracks)
    let mut p = p0.clone();
    p.merge_reel(&b);
    let mut grown = b;
    p.insert_track(LaneKind::Video, b.base.video + 1);
    grown.len.video += 1;
    p.unmerge_reel(&grown).expect("insertion inside a block has no right to crash the batch");
    assert_eq!(p.rows(), p0.rows(), "ROOT rows moved from the track inserted into the band");
    assert_eq!(
        p.sequence(MediaId(100)).map(|s| s.lanes(LaneKind::Video)),
        p0.sequence(MediaId(100)).map(|s| s.lanes(LaneKind::Video) + 1),
        "the band track must move into the reel"
    );
}

/// Test 568 (R.55, C5): inserting and deleting a track DO NOT TEAR the band.
///
/// The only place in the construction where correctness rests on a person, not on construction, is
/// `Band::after_batch`. An error there does not fail with any assert itself:
/// the band “slides”, and
/// the assembly starts rejecting batches, and with a symmetric error it silently puts an event in the wrong reel.
///
/// The assertion is one and it is strong: after ANY lane command the identity
/// t566 must remain true.
#[test]
fn t568_inserting_and_removing_lanes_does_not_break_a_band() {
    // a root track BELOW the start of the band — the band moves down as a whole
    for (name, cmd) in [
        ("video track at the root", Command::InsertTrack { kind: LaneKind::Video, at: 0 }),
        ("audio track at the root", Command::InsertTrack { kind: LaneKind::Audio, at: 0 }),
        ("morph row at the root", Command::InsertTrack { kind: LaneKind::Morph, at: 0 }),
        ("camera row at the root", Command::InsertTrack { kind: LaneKind::Camera, at: 0 }),
    ] {
        let mut m = TimelineModel::new(expandable());
        assert!(m.expand_band(ClipId(20)), "the strip did not expand");
        let before_doc = m.document();
        m.apply(cmd).unwrap_or_else(|e| panic!("{name}: command did not pass: {e:?}"));
        // CONSEQUENCE: the content of the reel is in place and has not mixed with the root
        let doc = m.document();
        let s0 = before_doc.sequence(MediaId(100)).unwrap();
        let s1 = doc.sequence(MediaId(100)).unwrap();
        assert_eq!(
            s1.clips.iter().map(|c| c.id).collect::<Vec<_>>(),
            s0.clips.iter().map(|c| c.id).collect::<Vec<_>>(),
            "{name}: reel contents shifted"
        );
        assert_eq!(s1.clips[0].start, s0.clips[0].start, "{name}: time in the reel shifted");
        assert_eq!(s1.clips[0].lane, s0.clips[0].lane, "{name}: track in the reel shifted");
        assert!(doc.clips.iter().any(|c| c.id == ClipId(20)), "{name}: instance disappeared from the root");
        // and it can still be collapsed back
        m.collapse_all_bands();
        assert_eq!(
            m.project().sequence(MediaId(100)).map(|s| s.clips.len()),
            Some(2),
            "{name}: after collapse the reel did not assemble"
        );
    }
}

/// Test 569 (R.55, C5): an edit IN THE BAND reaches the REEL, and an edit in
/// the root reaches the root.
///
/// This is the answer to “how are edits addressed”: not at all. An event of the band physically
/// lies in the root vector of the working view, the command arrives at it
/// by the ordinary path, and the assembly subtracts the offset. What is checked is the CONSEQUENCE — what
/// lies in `document()`, not which functions were called.
#[test]
fn t569_an_edit_inside_a_band_lands_in_the_reel() {
    let mut m = TimelineModel::new(expandable());
    assert!(m.expand_band(ClipId(20)));
    // an event of the BAND is visible in the working view — with the offset
    let inner = m.project().clips.iter().find(|c| c.id == ClipId(11)).expect("no band event");
    let was = inner.start;
    assert_eq!(was, Tick(LEN.0), "band not shifted to the root axis");

    m.apply(Command::MoveClip {
        id: ClipId(11),
        new_start: was + Tick(1_000),
        new_lane: inner.lane,
    })
    .expect("an edit in the band must pass");

    // CONSEQUENCE: an event of the REEL shifted, on its own axis
    let doc = m.document();
    let s = doc.sequence(MediaId(100)).expect("reel");
    let moved = s.clips.iter().find(|c| c.id == ClipId(11)).expect("no event in the reel");
    assert_eq!(moved.start, Tick(1_000), "the edit did not reach the reel, in its axis");
    assert!(
        !doc.clips.iter().any(|c| c.id == ClipId(11)),
        "the reel event stayed at the document ROOT — a different edit would have gone to disk"
    );

    // and an edit of a ROOT event — into the root
    m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(5_000), new_lane: 0 })
        .expect("an edit at the root must pass");
    let doc = m.document();
    assert_eq!(
        doc.clips.iter().find(|c| c.id == ClipId(1)).map(|c| c.start),
        Some(Tick(5_000)),
        "the edit of a root event did not reach the root"
    );

    // Ctrl+Z removes IT in one step and leaves the band unfolded
    m.undo().expect("undo");
    assert_eq!(m.bands().len(), 1, "undo collapsed the expansion — the edit would have disappeared from view");
}

// ---- R.55, C6: inserting a FOREIGN project as a composition ---------------------

/// A foreign project: two events, a transition between them, its own nested reel and
/// ITS OWN areas with markers — the things that must NOT come along.
fn foreign() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "synth:sine:440"));
    // ONE of the paths coincides with what we already have: it must be
    // RECOGNIZED, not arrive as a second instance
    p.media.push(MediaRef::new(MediaId(2), "foreign:only"));
    let mut inner = Sequence::new(MediaId(50), "Nested foreign");
    inner.clips = vec![clip(90, 1, Tick::ZERO, LEN)];
    p.sequences.push(inner);
    p.clips.push(clip(1, 1, Tick::ZERO, LEN));
    p.clips.push(clip(2, 2, LEN, LEN));
    p.clips.push(clip(3, 50, Tick(2 * LEN.0), LEN));
    p.areas.push(MarkedArea {
        name: "Foreign region".into(),
        start: Tick::ZERO,
        duration: LEN,
        lanes: Vec::new(),
        color: Default::default(),
    });
    p.markers.push(Marker {
        t: Tick(LEN.0),
        duration: Tick::ZERO,
        name: "foreign marker".into(),
        color: Default::default(),
    });
    p.validate().expect("a foreign project is legal");
    p
}

/// Test 574 (R.55, C6): a foreign project is inserted AS A COMPOSITION.
///
/// **The owner’s complaint verbatim:** “I drag the project file onto the timeline and
/// a new timeline opens, but I want it to give me a choice — open
/// as a project or insert as a composition”.
///
/// What is checked is the CONSEQUENCE: after insertion the document is legitimate, the addresses
/// did not collide, the nested reels of the foreign project came along with it, and
/// the media that is already ours was RECOGNIZED and not doubled.
#[test]
fn t574_a_foreign_project_is_inserted_as_a_composition() {
    let mut dst = base();
    dst.clips.push(clip(1, 1, Tick::ZERO, LEN));
    dst.validate().expect("an own project is legal");
    let mine_media = dst.media.len();

    let mut m = TimelineModel::new(dst);
    let doc = m.document();
    let mut n = ops::NestCounters::of(&doc);
    let (cmds, rep) =
        ops::import_as_sequences(m.project(), &foreign(), "Foreign", Tick(5 * LEN.0), 0, n.ids());
    assert!(!cmds.is_empty(), "insertion produced no commands");
    m.apply_batch(cmds).expect("inserting a foreign project must be legal");
    m.document().validate().expect("the document must be valid after the paste");

    let seq = rep.seq.expect("root reel of the foreign project");
    assert_eq!(rep.inner_seqs, 1, "nested reel of the foreign project did not arrive: {rep:?}");
    // MEDIA: the matching path is recognized, the foreign one is registered
    assert_eq!(rep.media_reused, 1, "already present media was not recognized and will arrive as a second copy: {rep:?}");
    assert_eq!(rep.media_added, 1, "foreign media was not created: {rep:?}");
    assert_eq!(
        m.document().media.len(),
        mine_media + 1,
        "media was duplicated: one file would have its own worker, its own cache row and its own thumbnails"
    );

    // the INSTANCE landed where it was dropped
    let p = m.document();
    let inst = p.clips.iter().find(|c| c.media == seq).expect("no instance");
    assert_eq!(inst.start, Tick(5 * LEN.0), "the instance did not land at the drop point");
    assert_eq!(inst.source_in, Tick::ZERO);
    // the content is in place
    let s = p.sequence(seq).expect("reel");
    assert_eq!(s.clips.len(), 3, "the contents of the foreign root did not arrive whole");

    // the AREAS AND MARKERS of the foreign project did NOT come along: they are navigation over ITS
    // document, and they must not be substituted for our own
    assert!(p.areas.is_empty(), "a foreign region would have replaced navigation in its own project");
    assert!(p.markers.is_empty(), "a foreign marker would have replaced navigation in its own project");
    assert_eq!(rep.dropped_extras, 2, "what was left outside must be NAMED: {rep:?}");
}

/// Test 575 (R.55, C6): the addresses of a foreign project do not collide with our own.
///
/// `ClipId` is unique across the WHOLE document (t425). If a foreign project arrived with
/// its own numbers, the `Retained` maps in kt-otio would silently swap material,
/// the start timecode and foreign effects between reels (the same class as
/// item 379).
#[test]
fn t575_imported_addresses_never_collide() {
    let mut dst = base();
    // DELIBERATELY the same numbers as the foreign project’s
    dst.clips.push(clip(1, 1, Tick::ZERO, LEN));
    dst.clips.push(clip(2, 1, LEN, LEN));
    dst.clips[1].lane = 1;
    dst.ensure_tracks(LaneKind::Video, 2);
    dst.validate().unwrap();

    let mut m = TimelineModel::new(dst);
    let doc = m.document();
    let mut n = ops::NestCounters::of(&doc);
    let (cmds, _) =
        ops::import_as_sequences(m.project(), &foreign(), "Foreign", Tick(9 * LEN.0), 0, n.ids());
    m.apply_batch(cmds).expect("insertion must be legal");

    // each address occurs EXACTLY ONCE across the whole document
    let p = m.document();
    let mut ids: Vec<u64> = p
        .clips
        .iter()
        .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
        .map(|c| c.id.0)
        .collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total, "event addresses collided after inserting the foreign project");
    let mut mids: Vec<u64> =
        p.media.iter().map(|m| m.id.0).chain(p.sequences.iter().map(|s| s.id.0)).collect();
    let mt = mids.len();
    mids.sort_unstable();
    mids.dedup();
    assert_eq!(mids.len(), mt, "media and reel addresses collided after insertion");
    p.validate().expect("the document after insertion is legal");
}

// ---- R.56, item 449: the order of rows and the unfolded band ---------------------

/// Test 591 (R.56, item 449). The benchmark of unfolding is TAKEN OUT FROM UNDER the invariant
/// “track number = row rank”, and the commands that apply this invariant are rejected
/// as a whole while the band is open.
///
/// The band deliberately occupies the TAIL of the numeric space of tracks, and its visible place is
/// set only by `lane_order` — that is, in the working view the numbers and the rows diverge
/// BY CONSTRUCTION. If we renumbered the benchmark, `unmerge_reel` would lay the events out
/// into the wrong reels, and silently.
#[test]
fn t591_the_bench_is_exempt_and_row_commands_are_refused() {
    let mut m = TimelineModel::new(expandable());
    assert!(m.expand_band(ClipId(20)), "the strip did not expand");

    // the working view since R.62 is canonical BY CONSTRUCTION (the block of the band is part of the
    // vector of tracks), and the benchmark is no longer an exception
    assert!(rows_are_canonical(&m.project().rows()));
    assert!(rows_are_canonical(&m.document().rows()));
    m.document().validate().expect("canon must remain valid with an open band");

    // an ordinary edit inside the band passes
    let inner = m.project().clips.iter().find(|c| c.id == ClipId(11)).unwrap().clone();
    m.apply(Command::MoveClip {
        id: ClipId(11),
        new_start: inner.start + Tick(1_000),
        new_lane: inner.lane,
    })
    .expect("an edit in the band must pass");

    // but the commands of rows do not — and the rejection is NAMED
    let rows = m.project().rows();
    assert!(
        matches!(m.apply(Command::SetLaneOrder { order: rows }), Err(CommandError::BandsOpen)),
        "row order with an open band must be rejected"
    );
    assert!(
        matches!(
            m.apply(Command::RemoveTrack { kind: LaneKind::Video, at: 0 }),
            Err(CommandError::BandsOpen)
        ),
        "deleting a track with an open band must be rejected"
    );
    assert_eq!(m.bands().len(), 1, "the rejection has no right to collapse the expansion itself");
}

/// Test 592 (R.56, item 449). Insertion of a track INTO THE TAIL of the band goes to
/// the BAND: nothing lies behind it by construction, and such an insertion is
/// “the reel needs one more track” (insertion of events in an unfolding).
///
/// Before R.56 the tail track went to no one: `base + len` did not fall into any branch,
/// and an event on it silently drifted into the root.
#[test]
fn t592_a_lane_at_the_band_tail_belongs_to_the_band() {
    let mut m = TimelineModel::new(expandable());
    assert!(m.expand_band(ClipId(20)));
    let base = m.bands()[0].base.video;
    let len = m.bands()[0].len.video;

    m.apply(Command::InsertTrack { kind: LaneKind::Video, at: base + len })
        .expect("insertion at the tail of the band must pass");
    assert_eq!(m.bands()[0].len.video, len + 1, "the tail track did not go to the band");

    // CONSEQUENCE: an event on the new track drifts INTO THE REEL, not into the root
    let mut c = clip(31, 1, Tick(LEN.0 / 2), LEN);
    c.lane = base + len;
    m.apply(Command::AddClip(c)).expect("event on the new track");
    let doc = m.document();
    let s = doc.sequence(MediaId(100)).expect("reel");
    assert!(s.clips.iter().any(|c| c.id == ClipId(31)), "event did not reach the reel");
    assert!(!doc.clips.iter().any(|c| c.id == ClipId(31)), "event stayed at the root");
}

/// Test 598 (R.56, item 452): the reel AS THE ROOT — a document that can be computed
/// offline.
///
/// The preview service of a reel needs a project where the reel stands in place of the root: the code
/// of the composition is one for the whole program (invariant 7), and it has no other
/// entry. What is asserted is the CONSEQUENCE: the content is the same, the document is legitimate,
/// and the reel itself is no longer in the list — otherwise it would refer to itself.
#[test]
fn t598_reel_as_root_is_a_valid_document() {
    let p = expandable();
    let seq = MediaId(100);
    let inner = p.sequence(seq).unwrap().clips.iter().map(|c| c.id).collect::<Vec<_>>();
    let n_morph = p.sequence(seq).unwrap().morphers.len();

    let root = p.reel_as_root(seq).expect("reel exists");
    assert_eq!(root.clips.iter().map(|c| c.id).collect::<Vec<_>>(), inner, "contents wrong");
    assert_eq!(root.morphers.len(), n_morph, "the reel layer did not move with the root");
    assert!(!root.sequences.iter().any(|s| s.id == seq), "the reel remained a reference to itself");
    root.validate().expect("a reel as root must be a legal document");

    // the source project is untouched — the service computes on a COPY
    assert_eq!(p.sequence(seq).unwrap().clips.len(), inner.len());
    assert!(p.reel_as_root(MediaId(999)).is_none(), "nonexistent reel");
}

// ---- R.58: the band INSIDE a full entry ---------------------------------------

/// Test 635 (R.58, item 460): unfolding in place works inside a full entry.
///
/// The owner’s complaint verbatim: “when I open a composition I cannot unfold a
/// nested composition in it”. The model never forbade this: after
/// `swap_reel` the canon is the current reel as the root, `Band::of` counts the bases from
/// IT, not from the document. The guards stood in the application with a wrong
/// comment “the band is counted from the root”. The test nails three things: the bases
/// of the band come from the tracks of the reel in which we stand; unfolding and collapsing —
/// the identity of the working view; the undo snapshot carries both the context and the band.
#[test]
fn t635_a_band_inside_an_entered_reel_counts_from_that_reel() {
    let mut p = base();
    p.ensure_tracks(LaneKind::Video, 1);
    p.ensure_tracks(LaneKind::Audio, 1);
    // Y — the grandchild reel
    let mut y = Sequence::new(MediaId(101), "Grandchild");
    y.clips = vec![clip(31, 1, Tick::ZERO, LEN), clip(32, 1, LEN, LEN)];
    y.ensure_tracks(LaneKind::Video, 1);
    y.ensure_tracks(LaneKind::Audio, 1);
    // X — a reel with an instance of Y on the SECOND track: the bases must come out
    // equal to the number of tracks of X, not of the root
    let mut x = Sequence::new(MediaId(100), "Child");
    x.clips.push(clip(21, 1, Tick::ZERO, LEN));
    let mut yi = clip(22, 101, LEN, Tick(2 * LEN.0));
    yi.lane = 1;
    x.clips.push(yi);
    x.ensure_tracks(LaneKind::Video, 2);
    x.ensure_tracks(LaneKind::Audio, 2);
    p.sequences.push(y);
    p.sequences.push(x);
    p.clips.push(clip(1, 1, Tick::ZERO, LEN));
    p.clips.push(clip(10, 100, LEN, Tick(3 * LEN.0)));
    p.validate().expect("fixture is legal");

    let mut m = TimelineModel::new(p);
    m.set_editing(Some(MediaId(100)));
    let canon_before = m.document();
    let view_before = m.project().clone();

    assert!(m.expand_band(ClipId(22)), "a band inside an entered reel must open");
    let b = &m.bands()[0];
    // R.62: the block of the band stands RIGHT UNDER the row of the instance (V2 of reel X):
    // the video base — two rows of X above it, the audio base — zero (the audio rows of X
    // lie BELOW the block and are shifted by its length during the unfolding)
    assert_eq!(
        (b.base.video, b.base.audio),
        (2, 0),
        "band bases must count the rows of the reel we stand in above the instance, not the root"
    );
    assert_eq!(b.dt, LEN, "band time offset is the instance start minus source_in");
    let inner = m.project().clips.iter().find(|c| c.id == ClipId(31)).expect("band content");
    assert_eq!((inner.lane, inner.start), (2, LEN), "band content sits after X's lanes, shifted");
    assert_eq!(m.document(), canon_before, "expanding changed the DOCUMENT");

    // an edit in the band reaches Y — not X and not the root
    m.apply(Command::MoveClip { id: ClipId(31), new_start: LEN + Tick(1_000), new_lane: 2 })
        .expect("an edit inside the band must pass");
    let doc = m.document();
    assert_eq!(doc.sequence(MediaId(101)).unwrap().clips[0].start, Tick(1_000), "edit missed Y");
    assert!(
        !doc.sequence(MediaId(100)).unwrap().clips.iter().any(|c| c.id == ClipId(31))
            && !doc.clips.iter().any(|c| c.id == ClipId(31)),
        "the event of Y leaked into X or into the root"
    );

    // the undo snapshot carries the CONTEXT and the BAND: exit to the root (the bands are cleared
    // by the model — that is its invariant, restoration is the application’s business), Ctrl+Z
    // — and we are again inside X with Y unfolded
    m.set_editing(None);
    assert!(m.bands().is_empty(), "set_editing must clear bands (model invariant)");
    m.undo().expect("undo");
    assert_eq!(m.editing(), Some(MediaId(100)), "undo did not bring the context back");
    assert_eq!(
        m.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![ClipId(22)],
        "undo did not bring the band back"
    );
    assert_eq!(m.document(), canon_before, "undo did not revert the edit");

    // collapse — the BYTE-FOR-BYTE identity of the working view (as t566 in the root)
    assert!(m.collapse_band(ClipId(22)));
    assert_eq!(m.project(), &view_before, "expand + collapse inside a reel is not identity");
    assert_eq!(m.document(), canon_before);
}

/// Test 723 (R.62, model): the reel’s own raster and fps — a command, a check,
/// undo; the default “as in the project” is not serialized.
#[test]
fn t723_reel_settings_command_and_validation() {
    let mut p = Project::new(settings());
    p.sequences.push(Sequence::new(MediaId(100), "Square"));
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);
    m.apply(Command::SetSequenceSettings {
        id: MediaId(100),
        raster: Some((1080, 1080)),
        fps: Some(12),
    })
    .expect("legal settings");
    let s = m.project().sequence(MediaId(100)).unwrap();
    assert_eq!((s.raster, s.fps), (Some((1080, 1080)), Some(12)));
    // an odd raster and 29.97 are rejected, the model is untouched
    assert!(
        m.apply(Command::SetSequenceSettings {
            id: MediaId(100),
            raster: Some((1081, 1080)),
            fps: None
        })
        .is_err()
    );
    assert!(
        m.apply(Command::SetSequenceSettings { id: MediaId(100), raster: None, fps: Some(7) })
            .is_err()
    );
    assert_eq!(m.project().sequence(MediaId(100)).unwrap().raster, Some((1080, 1080)));
    // an unknown reel — a rejection
    assert!(
        m.apply(Command::SetSequenceSettings { id: MediaId(999), raster: None, fps: None })
            .is_err()
    );
    m.undo().unwrap();
    let s = m.project().sequence(MediaId(100)).unwrap();
    assert_eq!((s.raster, s.fps), (None, None));
    let js = serde_json::to_string(s).unwrap();
    assert!(!js.contains("raster") && !js.contains("\"fps\""), "default is not written: {js}");
    // project validation catches broken settings written bypassing the command
    let mut bad = m.project().clone();
    bad.sequences[0].fps = Some(7);
    assert!(bad.validate().is_err());
}
