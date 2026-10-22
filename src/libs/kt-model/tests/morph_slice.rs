//! Tests 233, 234, 236 (docs/porting-notes.md): slicing a target with a morpher layer.
//!
//! Since R.35 the morpher acts BY TIME,
//! so a target that is partly covered has to be
//! sliced. Here everything that must remain true in that case is checked: the material of the
//! pieces, the determinism of their ids, degenerate cases and forbidden zones (finding 1 of R.35 —
//! the slice breaks not the target but its neighbours on the track).

use kt_model::*;
use kt_time::{Tick, TickRange};

const LEN: Tick = Tick(90_000);

fn clip(id: u64, kind: ClipKind, lane: u32, start: Tick, dur: Tick) -> ClipEvent {
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
        start,
        duration: dur,
        source_in: Tick(3_000),
        media: MediaId(1),
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
    p.media.push(MediaRef::new(MediaId(1), "synth:sine:440"));
    p.media.push(MediaRef::new(MediaId(2), "synth:ramp"));
    p.clips.push(clip(10, ClipKind::Video, 0, Tick(0), LEN));
    p.ensure_tracks(LaneKind::Morph, 1);
    p
}

/// A morpher layer over the middle of the clip: a source swap + a video effect.
fn layer(start: Tick, dur: Tick) -> MorpherEvent {
    MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start,
        duration: dur,
        targets: MorphTargets::Clips { ids: vec![ClipId(10)] },
        patch: MorphPatch {
            media: Some(MediaId(2)),
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            ..Default::default()
        },
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    }
}

/// Test 233: a clip covered PARTLY splits into three pieces; the material
/// of each piece matches the material of the original at the same points, the sum
/// of the durations equals the original, and the patch went exactly to the middle one.
#[test]
fn t233_partial_cover_slices_into_three() {
    let mut p = base();
    p.morphers.push(layer(Tick(30_000), Tick(30_000)));
    p.validate().unwrap();

    let eff = p.effective_clips();
    assert_eq!(eff.len(), 3, "three pieces");
    assert_eq!(
        eff.iter().map(|c| c.duration.0).sum::<i64>(),
        LEN.0,
        "sum of durations equals the original"
    );
    assert_eq!(eff[0].start, Tick(0));
    assert_eq!(eff[1].start, Tick(30_000));
    assert_eq!(eff[2].start, Tick(60_000));
    // material: the entry point moves by exactly the offset of the piece
    assert_eq!(eff[0].source_in, Tick(3_000));
    assert_eq!(eff[1].source_in, Tick(33_000));
    assert_eq!(eff[2].source_in, Tick(63_000));
    // the patch — only on the middle one
    assert_eq!(eff[0].media, MediaId(1));
    assert_eq!(eff[1].media, MediaId(2), "the covered piece is on the new source");
    assert_eq!(eff[2].media, MediaId(1));
    assert!(eff[0].fx.is_empty() && eff[2].fx.is_empty());
    assert_eq!(eff[1].fx.len(), 1, "the layer’s effect was added to the covered piece");

    // the material matches the original point for point over the whole body
    for t in [Tick(0), Tick(15_000), Tick(30_000), Tick(45_000), Tick(87_000)] {
        let c = eff.iter().find(|c| c.start <= t && t < c.start + c.duration).unwrap();
        let want = clip_material_at(Tick(3_000), Tick(0), LEN, false, 3_000, t);
        let got = clip_material_at(c.source_in, c.start, c.duration, false, 3_000, t);
        assert_eq!(got, want, "material at {t:?}");
    }
}

/// Test 233b: the same on a REVERSED clip — its material is mirrored, and
/// the entry point of the piece moves with its END.
#[test]
fn t233_reversed_slices_keep_material() {
    let mut p = base();
    p.clips[0].reversed = true;
    p.morphers.push(layer(Tick(30_000), Tick(30_000)));
    p.validate().unwrap();

    let eff = p.effective_clips();
    assert_eq!(eff.len(), 3);
    // the points are taken ON THE FRAME GRID — the video is rendered on it, and there the
    // pieces must coincide with the original (the boundaries are placed there by `windows_on`)
    for t in [Tick(0), Tick(21_000), Tick(30_000), Tick(57_000), Tick(87_000)] {
        let c = eff.iter().find(|c| c.start <= t && t < c.start + c.duration).unwrap();
        let want = clip_material_at(Tick(3_000), Tick(0), LEN, true, 3_000, t);
        let got = clip_material_at(c.source_in, c.start, c.duration, true, 3_000, t);
        assert_eq!(got, want, "reverse material at {t:?}");
    }
    // the same rule as for Split: the left half plays the TAIL of the source
    let mut m = TimelineModel::new({
        let mut q = base();
        q.clips[0].reversed = true;
        q
    });
    m.apply(Command::Split {
        id: ClipId(10),
        at: Tick(30_000),
        new_id: ClipId(11),
        new_group: None,
    })
    .unwrap();
    let left = m.project().clips.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(left.source_in, eff[0].source_in, "Split and cut compute source_in the same way");
}

/// Test 234: the ids of the pieces are a pure function of (original, number); piece №0
/// keeps the id of the original, `validate` rejects a real clip in the
/// reserved range, the original is recovered from the id itself.
#[test]
fn t234_slice_ids_are_deterministic_and_reserved() {
    let mut p = base();
    p.morphers.push(layer(Tick(30_000), Tick(30_000)));
    let ids: Vec<ClipId> = p.effective_clips().iter().map(|c| c.id).collect();
    assert_eq!(ids[0], ClipId(10), "piece No.0 holds the original id");
    assert_eq!(ids[1], slice_id(ClipId(10), 1));
    assert_eq!(ids[2], slice_id(ClipId(10), 2));
    assert_eq!(slice_origin(ids[1]), Some(ClipId(10)), "the original is retrieved from the id");
    assert_eq!(slice_origin(ids[0]), None, "a real clip does not pretend to be a piece");
    assert!(same_clip(ids[2], ClipId(10)) && !same_clip(ids[2], ClipId(11)));

    // an edit of a neighbouring clip on another track does not move the ids of the pieces
    let mut q = p.clone();
    q.clips.push(clip(20, ClipKind::Video, 1, Tick(0), LEN));
    let got: Vec<ClipId> =
        q.effective_clips().iter().map(|c| c.id).filter(|&i| same_clip(i, ClipId(10))).collect();
    assert_eq!(got, ids, "a neighbor on another track does not move piece ids");
    // moving a morpher block with the mouse — neither does that
    let mut r = p.clone();
    r.morphers[0].start = Tick(40_000);
    assert_eq!(r.effective_clips().iter().map(|c| c.id).collect::<Vec<_>>(), ids, "block move");

    // a real clip in the reserved range — a model error
    let mut bad = base();
    bad.clips[0].id = ClipId(MAX_REAL_CLIP_ID);
    assert!(matches!(bad.validate(), Err(ProjectError::ReservedClipId(..))));
}

/// Test 236: degenerate cases. A clip covered ENTIRELY is NOT cut and
/// keeps its id (and with it the audio-FX state, invariant №3); a window shorter than the
/// minimum does not act; a stub sticks to the edge of the body; a patch that does nothing
/// to this kind of clip does not cut it at all.
#[test]
fn t236_degenerate_windows() {
    // covered entirely — one clip with its own id
    let mut p = base();
    p.morphers.push(layer(Tick(-10_000), Tick(200_000)));
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "a fully covered one is not cut");
    assert_eq!(eff[0].id, ClipId(10), "and keeps the id");
    assert_eq!(eff[0].media, MediaId(2));

    // a window shorter than the minimum does not act at all
    let mut p = base();
    p.morphers.push(layer(Tick(30_000), Tick(50)));
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1);
    assert_eq!(eff[0].media, MediaId(1), "a microscopic window does nothing");

    // a 50-tick stub at the start of the body sticks to the edge: two pieces, not three
    let mut p = base();
    p.morphers.push(layer(Tick(50), Tick(30_000)));
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 2, "the stub collapsed to the edge of the body");
    assert_eq!(eff[0].start, Tick(0));
    assert_eq!(eff[0].media, MediaId(2), "the first piece is fully covered");
    assert_eq!(eff[0].duration, Tick(30_000), "and the end snapped to the frame grid");

    // A patch with video effects only does not touch the audio beneath it — and with R.55
    // (C2) it does NOT CUT THE VIDEO EITHER: the contribution of the chain can be switched off
    // by a strength curve, so there is no reason to cut for it. Previously there were three pieces here.
    let mut p = base();
    p.clips.push(clip(11, ClipKind::Audio, 0, Tick(0), LEN));
    p.ensure_tracks(LaneKind::Audio, 1);
    let mut m = layer(Tick(30_000), Tick(30_000));
    m.patch.media = None;
    m.targets = MorphTargets::Clips { ids: vec![ClipId(10), ClipId(11)] };
    p.morphers.push(m);
    let eff = p.effective_clips();
    let audio: Vec<&ClipEvent> = eff.iter().filter(|c| c.kind == ClipKind::Audio).collect();
    assert_eq!(audio.len(), 1, "audio is not cut");
    assert_eq!(audio[0].id, ClipId(11));
    let video: Vec<&ClipEvent> = eff.iter().filter(|c| c.kind == ClipKind::Video).collect();
    assert_eq!(video.len(), 1, "video is NOT cut either: the chain is turned off, not cut out");
    assert_eq!(video[0].id, ClipId(10), "and the piece holds the original id (invariant 3)");
    assert!(
        video[0].fx.iter().any(|f| matches!(f, FxInstance::Mirror { .. })),
        "the layer’s chain must be on the body"
    );
    // GATE: the strength is zero outside the block’s window and full inside
    let (i, _) = video[0]
        .fx
        .iter()
        .enumerate()
        .find(|(_, f)| matches!(f, FxInstance::LayerMix { .. }))
        .expect("a strength slot must appear: without it the layer would act on the whole body");
    let cur = &video[0].fx_auto[i][0];
    let at = |t: i64| {
        let ks = &cur.keys;
        let j = ks.partition_point(|k| k.t.0 <= t).saturating_sub(1);
        ks[j].v
    };
    assert!(at(10_000) < 0.01, "before the window the layer must be silent, not active");
    assert!(at(45_000) > 0.99, "inside the window the layer must act at full strength");
    assert!(at(80_000) < 0.01, "after the window the layer must be silent");
}

/// Test 236b: forbidden zones. The boundary of the layer does not stop inside the window
/// of a transition or inside the overlap of a neighbour — it STICKS to their edge (decision 8
/// of R.35): otherwise a cut piece with a larger `start` would stick out above the
/// neighbour (the Vegas rule) and would change the crossfade weights.
#[test]
fn t236_forbidden_zones_snap_the_boundary() {
    let mut p = base();
    p.clips.push(clip(11, ClipKind::Video, 0, LEN, LEN));
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(10)),
        b: Some(ClipId(11)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let zones = kt_model::forbidden_zones(&p, &p.clips[0]);
    assert!(zones.iter().any(|z| z.start == Tick(81_000) && z.end == LEN), "the transition window is a zone");

    // a boundary inside the transition window is pushed to the nearer edge
    p.morphers.push(layer(Tick(30_000), Tick(53_000))); // the end at 83_000 — inside
    let eff = p.effective_clips();
    let first: Vec<&ClipEvent> = eff.iter().filter(|c| same_clip(c.id, ClipId(10))).collect();
    assert_eq!(first.len(), 3, "two cuts: the layer start and the pushed-out end");
    assert_eq!(first[1].start, Tick(30_000));
    assert_eq!(first[2].start, Tick(81_000), "pushing to the near edge of the transition window");
    assert_eq!(first[1].media, MediaId(2), "the layer went to the middle piece");
    assert_eq!(first[2].media, MediaId(1), "the layer did not cover the transition window");

    // an overlap of a neighbour on the track — also a forbidden zone
    let mut p = base();
    p.clips.push(clip(11, ClipKind::Video, 0, Tick(70_000), LEN));
    let zones = kt_model::forbidden_zones(&p, &p.clips[0]);
    assert!(zones.iter().any(|z| z.start == Tick(70_000) && z.end == LEN), "the overlap is a zone");
    p.morphers.push(layer(Tick(30_000), Tick(45_000))); // the end at 75_000 — inside an overlap
    let eff = p.effective_clips();
    let first: Vec<&ClipEvent> = eff.iter().filter(|c| same_clip(c.id, ClipId(10))).collect();
    assert_eq!(first.len(), 3);
    assert_eq!(first[2].start, Tick(70_000), "the boundary sticks to the edge of the overlap, not to the grid");
}

/// Test 236c: `windows_on` — the window in which the morpher really acts.
/// Files before v4 (`timed: false`) patch the whole body regardless of
/// the position of the block — the GOLDEN of migrated projects rests on this.
#[test]
fn t236_untimed_morpher_ignores_block_position() {
    let mut p = base();
    let mut m = layer(Tick(500_000), Tick(1_000)); // the block OUTSIDE the clip
    m.timed = false;
    p.morphers.push(m);
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "an old morpher does not cut");
    assert_eq!(eff[0].media, MediaId(2), "and acts independently of the block time");
    assert_eq!(
        p.morphers[0].windows_on(&p.clips[0], &p, &p, &p.rows()),
        vec![TickRange::new(Tick(0), LEN)]
    );

    // a disabled morpher neither acts nor cuts
    p.morphers[0].timed = true;
    p.morphers[0].start = Tick(30_000);
    p.morphers[0].duration = Tick(30_000);
    p.morphers[0].enabled = false;
    assert_eq!(p.effective_clips(), p.clips, "a disabled morpher is transparent");
}

/// Test 235 (R.35 C1, piece 4): LAYER mode. A morpher without a selection
/// acts on everything BELOW its row; the stacking order is bottom to top
/// (the bottom layer first, like an adjustment layer in AE); reordering the
/// rows changes the order of effects and is rolled back by undo.
#[test]
fn t235_below_layer_and_bottom_up_order() {
    let mut p = base();
    p.ensure_tracks(LaneKind::Video, 2);
    p.ensure_tracks(LaneKind::Audio, 1);
    p.clips.push(clip(20, ClipKind::Video, 1, Tick(0), LEN));
    p.clips.push(clip(30, ClipKind::Audio, 0, Tick(0), LEN));
    let mut m = layer(Tick(0), LEN);
    m.targets = MorphTargets::Below;
    m.patch = MorphPatch {
        video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
        ..Default::default()
    };
    p.morphers.push(m);
    // the morph row by default is the BOTTOMMOST — “below” it is empty
    assert_eq!(p.effective_clips(), p.clips, "the layer at the bottom of the list covers nothing");

    // we raise the morph row to the top — it covers all the video beneath it
    let mut order = p.rows();
    let morph = order.remove(order.iter().position(|&k| k.0 == LaneKind::Morph).unwrap());
    order.insert(0, morph);
    p.set_lane_order(&order);
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 3, "the layer covers the whole thing — no cut");
    for c in eff.iter().filter(|c| c.kind == ClipKind::Video) {
        assert_eq!(c.fx.len(), 1, "video under the layer got the effect");
    }
    assert!(
        eff.iter().find(|c| c.id == ClipId(30)).unwrap().fx.is_empty(),
        "a video layer does not touch audio"
    );

    // the layer sees exactly what is BELOW: between two video tracks
    let mut order = p.rows();
    let morph = order.remove(order.iter().position(|&k| k.0 == LaneKind::Morph).unwrap());
    order.insert(1, morph);
    p.set_lane_order(&order);
    let eff = p.effective_clips();
    let above = eff.iter().find(|c| c.id == ClipId(10)).unwrap();
    let below = eff.iter().find(|c| c.id == ClipId(20)).unwrap();
    assert!(above.fx.is_empty(), "what is ABOVE the layer is not touched");
    assert_eq!(below.fx.len(), 1, "what is below is touched");
}

/// Test 235b: the stacking order bottom to top and its rollback by undo.
#[test]
fn t235_layer_order_is_bottom_up_and_undoable() {
    let mut p = base();
    p.ensure_tracks(LaneKind::Morph, 2);
    for (id, lane, fx) in [
        (1u64, 0u32, FxInstance::Mirror { horizontal: true, vertical: false }),
        (2, 1, FxInstance::Mirror { horizontal: false, vertical: true }),
    ] {
        let mut m = layer(Tick(0), LEN);
        m.id = MorphId(id);
        m.lane = lane;
        m.targets = MorphTargets::Below;
        m.patch = MorphPatch { video_fx: vec![fx], ..Default::default() };
        p.morphers.push(m);
    }
    // both morph rows — above the video
    let mut order = p.rows();
    order.retain(|k| k.0 != LaneKind::Morph);
    order.insert(0, (LaneKind::Morph, 0));
    order.insert(1, (LaneKind::Morph, 1));
    p.set_lane_order(&order);
    let mut m = TimelineModel::new(p);
    let chain = |m: &TimelineModel| m.project().effective_clips()[0].fx.clone();
    let first = chain(&m);
    assert_eq!(first.len(), 2, "both layers act");
    assert!(
        matches!(first[0], FxInstance::Mirror { vertical: true, .. }),
        "the BOTTOM layer (row 1) was applied first"
    );

    // we swap the rows — the order of the effects must flip
    let mut order = m.project().rows();
    order.swap(0, 1);
    m.apply(Command::SetLaneOrder { order }).unwrap();
    let swapped = chain(&m);
    assert!(
        matches!(swapped[0], FxInstance::Mirror { horizontal: true, .. }),
        "reordering the rows reversed the overlay order"
    );
    m.undo().unwrap();
    assert_eq!(chain(&m), first, "undo restored both the row order and the effect order");
}

/// Test 239 (R.35 C1, piece 4): bake CUTS honestly. The render view before the bake and
/// the content of the clips after it coincide, the pieces get real ids,
/// the transition is re-hung on the piece that got the joint, all in ONE undo step.
#[test]
fn t239_bake_materializes_slices() {
    let mut p = base();
    p.clips.push(clip(11, ClipKind::Video, 0, LEN, LEN));
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(10)),
        b: Some(ClipId(11)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.morphers.push(layer(Tick(30_000), Tick(30_000)));
    p.validate().unwrap();
    let before = p.effective_clips();
    assert_eq!(before.len(), 4, "three pieces of the first clip + neighbor");

    let mut m = TimelineModel::new(p);
    m.apply(Command::BakeMorpher { id: MorphId(1) }).unwrap();
    assert!(m.project().morphers.is_empty(), "bake removed the event");
    assert_eq!(m.project().clips.len(), 4, "the pieces became real clips");
    assert!(
        m.project().clips.iter().all(|c| slice_origin(c.id).is_none()),
        "no synthetic ids remain in the project"
    );
    // the geometry and material are the same as were drawn
    for (a, b) in m.project().clips.iter().zip(&before) {
        assert_eq!(
            (a.start, a.duration, a.source_in, a.media),
            (b.start, b.duration, b.source_in, b.media)
        );
        assert_eq!(a.fx, b.fx);
    }
    assert_eq!(m.project().effective_clips().len(), 4, "render did not change");
    // the transition moved to the piece that received the joint
    let t = &m.project().transitions[0];
    let a = m.project().clips.iter().find(|c| Some(c.id) == t.a).unwrap();
    assert_eq!(a.start + a.duration, LEN, "the end of the transition belongs to the piece with the seam");
    assert_ne!(t.a, Some(ClipId(10)), "not to piece No.0");
    m.project().validate().unwrap();

    m.undo().unwrap();
    assert_eq!(m.project().clips.len(), 2, "one undo restored the edit");
    assert_eq!(m.project().morphers.len(), 1, "and the morpher");
    assert_eq!(m.project().transitions[0].a, Some(ClipId(10)));
}

/// Test 243 (R.35 C2): the patch carries BOTH a source swap AND chains — this is
/// one event, not two mutually exclusive kinds (`MorphKind` removed).
/// The switch of the morph slot turns off ONLY the swap; the guard of `RemoveMedia`
/// asks `media_refs`, not a pattern match on the kind (the compiler does not catch such
/// places — this is where the earlier defect lived).
#[test]
fn t243_patch_carries_source_and_chains() {
    let mut p = base();
    let mut m = layer(Tick(0), LEN);
    m.patch.source_in_delta = Tick(4_500);
    m.patch.video_fx = vec![FxInstance::Mirror { horizontal: true, vertical: false }];
    p.morphers.push(m);
    p.validate().unwrap();

    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "covered entirely — not cut");
    assert_eq!(eff[0].media, MediaId(2), "source substitution applies");
    assert_eq!(eff[0].source_in, Tick(7_500), "and the entry point shift too");
    assert_eq!(eff[0].fx.len(), 1, "and the chain — by the same event");

    // the checkbox of the morph slot turns off ONLY the swap, the effects stay
    p.morphers[0].patch.src_off = true;
    let eff = p.effective_clips();
    assert_eq!(eff[0].media, MediaId(1), "a disabled morph slot restores the original source");
    assert_eq!(eff[0].source_in, Tick(3_000), "and the original entry point");
    assert_eq!(eff[0].fx.len(), 1, "layer effects have nothing to do with the morph slot");
    assert!(!p.morphers[0].patch.is_empty(), "substitution settings are intact");

    // media held ONLY by a morpher cannot be dropped from the project
    let mut mm = TimelineModel::new(p);
    assert!(
        mm.apply(Command::RemoveMedia { id: MediaId(2) }).is_err(),
        "the guard must block deletion of media held by a morpher"
    );
    assert!(mm.project().media.iter().any(|m| m.id == MediaId(2)));
}

/// Test 247 (R.35 C4, verbatim: “can the area-selection be made saved
/// as a marked_region in the form of a marked_area, and also referenced… if
/// the user changes his mind he can change/delete/add in the morpher widget
/// the region/area to other ones”). A reference BY NAME: names are unique in one
/// space together with range markers, renaming fixes the references, deletion leaves the
/// reference dead rather than silently dropping it.
#[test]
fn t247_named_areas_and_references() {
    // `unique_name` — one function for the check and for the insertion
    let taken = vec!["final".to_string(), "final 2".to_string()];
    assert_eq!(unique_name(&taken, "final", "area"), "final 3");
    assert_eq!(unique_name(&taken, "start", "area"), "start");
    assert_eq!(unique_name(&taken, "  ", "area"), "area");

    let mut p = base();
    p.ensure_tracks(LaneKind::Video, 2);
    p.clips.push(clip(20, ClipKind::Video, 1, Tick(0), LEN));
    p.areas.push(MarkedArea {
        name: "final".into(),
        start: Tick(30_000),
        duration: Tick(30_000),
        lanes: vec![(LaneKind::Video, 0)],
        color: None,
    });
    let mut m = layer(Tick(0), Tick(1)); // the block is TINY and not there
    m.targets = MorphTargets::Areas { names: vec!["final".into()] };
    p.morphers.push(m);
    p.validate().unwrap();

    // the window is taken from the AREA, not from the block
    let eff = p.effective_clips();
    let first: Vec<&ClipEvent> = eff.iter().filter(|c| same_clip(c.id, ClipId(10))).collect();
    assert_eq!(first.len(), 3, "a clip on a region track is cut along its boundaries");
    assert_eq!(first[1].start, Tick(30_000));
    assert_eq!(first[1].media, MediaId(2), "inside the region the patch applies");
    assert_eq!(first[0].media, MediaId(1), "outside — no");
    // the area named the TRACKS: the neighbouring one is untouched
    let other: Vec<&ClipEvent> = eff.iter().filter(|c| same_clip(c.id, ClipId(20))).collect();
    assert_eq!(other.len(), 1, "a track outside the region is not cut");
    assert_eq!(other[0].media, MediaId(1));

    // one namespace: a range marker with the same name is illegal
    let mut bad = p.clone();
    bad.markers.push(Marker {
        t: Tick(0),
        duration: Tick(1_000),
        name: "final".into(),
        color: None,
    });
    assert!(matches!(bad.validate(), Err(ProjectError::DuplicateName(..))));

    let mut mm = TimelineModel::new(p);
    // the command also rejects an occupied name, and an empty one
    assert!(matches!(
        mm.apply(Command::AddArea(MarkedArea {
            name: "final".into(),
            start: Tick(0),
            duration: Tick(1_000),
            lanes: vec![],
            color: None,
        })),
        Err(CommandError::NameTaken(..))
    ));

    // renaming fixes the REFERENCES and is rolled back in ONE undo
    mm.apply(Command::RenameArea { from: "final".into(), to: "end".into() }).unwrap();
    assert_eq!(mm.project().areas[0].name, "end");
    assert_eq!(
        mm.project().morphers[0].targets,
        MorphTargets::Areas { names: vec!["end".into()] },
        "the morpher reference moved along with the name"
    );
    assert_eq!(mm.project().effective_clips().len(), 4, "and keeps working");
    mm.undo().unwrap();
    assert_eq!(mm.project().areas[0].name, "final");
    assert_eq!(
        mm.project().morphers[0].targets,
        MorphTargets::Areas { names: vec!["final".into()] }
    );

    // Deleting the area leaves the reference DEAD: the user restores the
    // area under the same name, and everything comes back to life (rule 20 — nothing
    // switches itself off)
    mm.apply(Command::RemoveArea { name: "final".into() }).unwrap();
    assert!(mm.project().areas.is_empty());
    assert_eq!(
        mm.project().morphers[0].targets,
        MorphTargets::Areas { names: vec!["final".into()] },
        "the reference stayed in the model"
    );
    assert_eq!(mm.project().effective_clips(), mm.project().clips, "but leads nowhere");
    mm.project().validate().unwrap();
}

/// Test 247b: a reference to a range MARKER gives the same window as the area on
/// all tracks; several overlapping references merge into one.
#[test]
fn t247_marks_give_windows_and_merge() {
    let mut p = base();
    p.markers.push(Marker {
        t: Tick(30_000),
        duration: Tick(20_000),
        name: "a".into(),
        color: None,
    });
    p.markers.push(Marker {
        t: Tick(45_000),
        duration: Tick(15_000),
        name: "b".into(),
        color: None,
    });
    let mut m = layer(Tick(0), Tick(1));
    m.targets =
        MorphTargets::Marks { names: vec!["a".into(), "b".into(), "no such one".into()] };
    p.morphers.push(m);
    p.validate().unwrap();

    let eff = p.effective_clips();
    assert_eq!(eff.len(), 3, "two overlapping references merged into ONE window");
    assert_eq!(eff[1].start, Tick(30_000));
    assert_eq!(eff[1].duration, Tick(30_000), "[30000,60000) — union");
    assert_eq!(eff[1].media, MediaId(2));
    // a point marker does not define a window
    let mut q = base();
    q.markers.push(Marker { t: Tick(30_000), duration: Tick::ZERO, name: "t".into(), color: None });
    let mut m = layer(Tick(0), Tick(1));
    m.targets = MorphTargets::Marks { names: vec!["t".into()] };
    q.morphers.push(m);
    assert_eq!(q.effective_clips(), q.clips, "a point marker is not a range");
}

/// Test 249 (R.35 C5, verbatim: “When I select everything on the timeline and
/// ctrl+c ctrl+v, I want the links to be set for morphers too…
/// and the marked_areas are copied
/// on selection and copying too, and the marked_region together with those inside it. Names are unique,
/// and the user can be asked to enter a prefix”).
#[test]
fn t249_clipboard_carries_morphers_areas_and_marks() {
    use kt_model::ops::*;
    let mut p = base();
    p.clips.push(clip(11, ClipKind::Video, 0, LEN, LEN));
    p.areas.push(MarkedArea {
        name: "final".into(),
        start: Tick(0),
        duration: Tick(45_000),
        lanes: vec![(LaneKind::Video, 0)],
        color: None,
    });
    p.markers.push(Marker {
        t: Tick(10_000),
        duration: Tick(20_000),
        name: "Insert".into(),
        color: None,
    });
    // a morpher on the FIRST clip and a morpher-reference to an area
    p.morphers.push(layer(Tick(0), Tick(45_000)));
    let mut byref = layer(Tick(0), Tick(45_000));
    byref.id = MorphId(2);
    byref.targets = MorphTargets::Areas { names: vec!["final".into(), "foreign".into()] };
    p.morphers.push(byref);
    // a morpher whose targets are OUTSIDE the block — its insertion must skip it
    let mut orphan = layer(Tick(0), Tick(45_000));
    orphan.id = MorphId(3);
    orphan.targets = MorphTargets::Clips { ids: vec![ClipId(11)] };
    p.morphers.push(orphan);
    p.validate().unwrap();

    // we copy the FIRST clip: its morpher, the area and the marker travel with it
    let block = copy_clips(&p, &[ClipId(10)]);
    assert_eq!(block.clips.len(), 1);
    assert_eq!(block.areas.len(), 1, "the region within the block window is copied");
    assert_eq!(block.markers.len(), 1, "a range marker too");
    assert_eq!(block.morphers.len(), 2, "a morpher whose targets are OUTSIDE the block is not copied at all");
    // ...and if the block came from outside (the system clipboard is JSON) and
    // refers to a clip that is not in it, the insertion SKIPS such a
    // morpher and counts it: the batch is atomic, and one bad element would bring
    // down the whole insertion, and it would look like “copy-paste does not work”
    let mut block = block;
    let mut foreign = block.morphers[0].clone();
    foreign.id = MorphId(9);
    foreign.targets = MorphTargets::Clips { ids: vec![ClipId(777)] };
    block.morphers.push(foreign);

    let mut m = TimelineModel::new(p);
    let (mut nc, mut ng, mut nt, mut nm) = (100u64, 100u64, 100u64, 100u64);
    let mut ncam = 100u64;
    let names =
        PasteNames { affix: "copy".into(), before: false, taken: m.project().taken_names() };
    let (cmds, _end, skipped) = paste_block(
        &block,
        Tick(300_000),
        PasteIds {
            clip: &mut nc,
            group: &mut ng,
            transition: &mut nt,
            morph: &mut nm,
            camera: &mut ncam,
        },
        &names,
    );
    assert_eq!(skipped.morphers, 1, "a morpher without surviving targets is skipped");
    m.apply_batch(cmds).expect("the batch must pass, not crash because of a skipped item");

    // names are unique and with a prefix; the default is a SUFFIX (R.38 item 19:
    // “by default one should put a suffix, not a prefix”)
    let a = m.project().areas.iter().find(|a| a.name != "final").expect("copy of the region");
    assert_eq!(a.name, "final copy");
    assert_eq!(a.start, Tick(300_000), "the region is rebased to the insertion point");
    assert!(
        m.project().markers.iter().any(|k| k.name == "Insert copy" && k.t == Tick(310_000)),
        "the marker was also renamed and moved"
    );

    // the targets of a copied morpher point ONLY to the new ids
    let pasted: Vec<&MorpherEvent> =
        m.project().morphers.iter().filter(|x| x.id.0 >= 100).collect();
    assert_eq!(pasted.len(), 2, "two morphers arrived, the third was skipped");
    let frozen = pasted.iter().find(|x| !x.targets.clip_ids().is_empty()).unwrap();
    assert_eq!(frozen.targets.clip_ids(), &[ClipId(100)], "the target is the new clip, not the old one");
    // a reference to the COPIED name moved, to a non-copied one it did not
    let byref = pasted.iter().find(|x| x.targets.clip_ids().is_empty()).unwrap();
    assert_eq!(
        byref.targets,
        MorphTargets::Areas { names: vec!["final copy".into(), "foreign".into()] }
    );
    m.project().validate().unwrap();

    // a SECOND insertion with the same word gives “ 2” — names remain addresses
    let names =
        PasteNames { affix: "copy".into(), before: false, taken: m.project().taken_names() };
    let (cmds, _end, _) = paste_block(
        &block,
        Tick(600_000),
        PasteIds {
            clip: &mut nc,
            group: &mut ng,
            transition: &mut nt,
            morph: &mut nm,
            camera: &mut ncam,
        },
        &names,
    );
    m.apply_batch(cmds).unwrap();
    assert!(
        m.project().areas.iter().any(|a| a.name == "final copy 2"),
        "the second insertion must produce a unique name: {:?}",
        m.project().areas.iter().map(|a| &a.name).collect::<Vec<_>>()
    );
}

/// Test 266 (R.38 item 13, verbatim:
/// “for a morpher I cannot set the automation of effects”).
/// The curves of a layer live in its patch in the domain of the BLOCK (a morpher has no material),
/// and `apply_to` converts them into the material of the target — by the same mapping as everything else.
#[test]
fn t266_morph_layer_curves_rebase_to_target_material() {
    use kt_model::*;
    use kt_time::Tick;

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
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(90_000),
        duration: Tick(90_000 * 4),
        // the entry point is NOT zero: otherwise “material” and “timeline” would coincide
        // and the test could not tell the correct conversion from its absence
        source_in: Tick(90_000 * 10),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.ensure_tracks(LaneKind::Morph, 1);
    p.set_lane_order(&[(LaneKind::Morph, 0), (LaneKind::Video, 0), (LaneKind::Audio, 0)]);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        // the block starts LATER than the clip — the shift must enter the conversion
        start: Tick(90_000 * 2),
        duration: Tick(90_000 * 2),
        targets: MorphTargets::Below,
        patch: MorphPatch {
            video_fx: vec![FxInstance::blur(4)],
            video_auto: vec![vec![ParamCurve {
                param: "radius".into(),

                keys: AutoKey::linear(vec![
                    AutoKey::at(Tick(0), 2.0),
                    AutoKey::at(Tick(90_000 * 2), 9.0),
                ]),
                source: Default::default(),
            }]],
            ..Default::default()
        },
        enabled: true,
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
    });
    p.validate().unwrap();

    let eff = p.effective_clips();
    let patched: Vec<&ClipEvent> = eff.iter().filter(|c| !c.fx.is_empty()).collect();
    assert_eq!(patched.len(), 1, "exactly one piece is covered: {:?}", eff.len());
    let c = patched[0];
    assert_eq!(c.fx_auto.len(), c.fx.len(), "curves are parallel to the chain");
    let keys = &c.fx_auto[0][0].keys;
    assert_eq!(keys.len(), 2);
    // block 2..4 s of the timeline = material 11..13 s (source_in = 10 s,
    // the clip starts at 1 s)
    assert_eq!(keys[0].t, Tick(90_000 * 11), "block start → target material");
    assert_eq!(keys[1].t, Tick(90_000 * 13), "block end → target material");
    assert_eq!(keys[0].v, 2.0);
    assert_eq!(keys[1].v, 9.0);

    // a disabled slot does not carry its curve along: `auto_for` goes in the same
    // order as `chain_for`, otherwise the curve would drift onto a foreign effect
    let mut p2 = p.clone();
    p2.morphers[0].patch.video_bypass = vec![true];
    let eff2 = p2.effective_clips();
    assert!(
        eff2.iter().all(|c| c.fx.is_empty() && c.fx_auto.iter().all(|a| a.is_empty())),
        "a disabled slot brings neither an effect nor a curve"
    );

    // and a patch with only curves is not considered empty: an “empty” one is not written to the file
    let mut only = MorphPatch::default();
    assert!(only.is_empty());
    only.video_auto = vec![vec![ParamCurve {
        param: "radius".into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 1.0)]),
        source: Default::default(),
    }]];
    assert!(!only.is_empty(), "a patch with curves must serialize");
}

// ---------------------------------------------------------------------------
// Tests 270 (R.39): cutting a STRETCHED clip lands on the right material.
// ---------------------------------------------------------------------------

fn warped(rate: f64, keys: Vec<AutoKey>, reversed: bool) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(4 * 90_000),
        source_in: Tick(90_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx: vec![FxInstance::slowed_reverb(rate, 0.0, 85.0)],
        fx_bypass: vec![false],
        fx_auto: if keys.is_empty() {
            vec![vec![]]
        } else {
            vec![vec![ParamCurve {
                param: "rate".into(),
                keys: AutoKey::linear(keys),
                source: Default::default(),
            }]]
        },
        opacity: 1.0,
        fx_off: false,
    }
}

/// Test 270: half speed — half of the material.
#[test]
fn t270_split_half_speed_in_the_middle() {
    let c = warped(0.5, vec![], false);
    let half = TickRange::new(Tick(2 * 90_000), Tick(4 * 90_000));
    let right = clip_slice(&c, half, ClipId(2));
    assert_eq!(
        right.source_in,
        Tick(90_000 + 90_000),
        "over 2 s of output at 0.5× 1 s of material is consumed, not 2 (before R.39 it was 270_000)"
    );
    // the left piece starts where the parent does
    let left = clip_slice(&c, TickRange::new(Tick(0), Tick(2 * 90_000)), ClipId(1));
    assert_eq!(left.source_in, c.source_in, "a cut at the start does not move the material");
}

/// Test 270b: the same, mirrored. “WARP, THEN MIRROR” — the anchor is counted from the
/// material actually consumed, not from the length of the event.
#[test]
fn t270b_split_reversed_half_speed() {
    let c = warped(0.5, vec![], true);
    // in total 2 s of material is consumed
    let first = clip_slice(&c, TickRange::new(Tick(0), Tick(2 * 90_000)), ClipId(1));
    assert_eq!(
        first.source_in,
        Tick(90_000 + 90_000),
        "a mirrored clip starts from the TAIL of what was consumed: 1 s, not 2"
    );
    let last = clip_slice(&c, TickRange::new(Tick(2 * 90_000), Tick(4 * 90_000)), ClipId(2));
    assert_eq!(last.source_in, c.source_in, "the last piece plays the start of the source");
}

/// Test 270c: the speed CURVE is an integral, not a multiplication. And the main point:
/// the keys travel together with the anchor, otherwise the whole curve drifts behind it.
#[test]
fn t270c_split_on_a_rate_curve_moves_keys_with_the_anchor() {
    // a ramp 1.0 → 2.0 over the whole event; keys in ticks of material from source_in
    let c = warped(
        1.0,
        vec![AutoKey::at(Tick(90_000), 1.0), AutoKey::at(Tick(90_000 + 4 * 90_000), 2.0)],
        false,
    );
    let at = Tick(2 * 90_000);
    let right = clip_slice(&c, TickRange::new(at, Tick(4 * 90_000)), ClipId(2));

    // S(0.5) for the ramp 1→2 = 0.5·1 + 0.5²·0.5/… — trapezoid: (1 + 1.5)/2 · 0.5 = 0.625
    let want = Tick(90_000 + (4.0f64 * 90_000.0 * 0.625).round() as i64);
    assert_eq!(right.source_in, want, "the anchor is the integral of the speed, not 2 s");

    // the keys shifted by the SAME delta: the difference (k.t − source_in) is intact
    let d = (want - c.source_in).0 - (at - c.start).0;
    assert_ne!(d, 0, "the scene must be warped, otherwise the test checks nothing");
    for (before, after) in c.fx_auto[0][0].keys.iter().zip(&right.fx_auto[0][0].keys) {
        assert_eq!(
            after.t.0 - right.source_in.0,
            before.t.0 - c.source_in.0 - (at - c.start).0,
            "the automation axis of the piece must stay linear (R.37), \
             and only the material must shift"
        );
    }
}

/// Test 270d: the seam. The end of the first piece and the start of the second are the same
/// material; the sum consumed equals the parent’s.
#[test]
fn t270d_slice_seam_has_no_gap() {
    for (rate, rev) in [(0.5, false), (2.0, false), (0.5, true), (2.0, true)] {
        let c = warped(rate, vec![], rev);
        let at = Tick(90_000 * 3 / 2); // not at a round boundary
        let l = clip_slice(&c, TickRange::new(Tick(0), at), ClipId(1));
        let r = clip_slice(&c, TickRange::new(at, Tick(4 * 90_000)), ClipId(2));
        let used = |x: &ClipEvent| kt_model::clip_warp(x).map(|w| w.used()).unwrap_or(x.duration.0);
        let (near, far) = if rev { (&r, &l) } else { (&l, &r) };
        let seam = near.source_in.0 + used(near);
        assert!(
            (seam - far.source_in.0).abs() <= 1,
            "rate {rate} rev {rev}: the seam diverges by {} ticks",
            seam - far.source_in.0
        );
        assert!(
            (used(&l) + used(&r) - used(&c)).abs() <= 2,
            "rate {rate} rev {rev}: total consumed is not the same as by the parent"
        );
    }
}

/// Test 270e: on a clip WITHOUT warp the previous rule holds byte-for-byte — the GOLDENs
/// and all old tests rest on it.
#[test]
fn t270e_unwarped_slice_is_unchanged() {
    for reversed in [false, true] {
        for fx in [vec![], vec![FxInstance::blur(4)]] {
            let mut c = warped(1.0, vec![], reversed);
            c.fx = fx.clone();
            c.fx_bypass = vec![false; fx.len()];
            c.fx_auto = vec![vec![]; fx.len()];
            for w in [(0, 2), (1, 3), (2, 4)] {
                let win = TickRange::new(Tick(w.0 * 90_000), Tick(w.1 * 90_000));
                let want = if reversed {
                    c.source_in + ((c.start + c.duration) - win.end)
                } else {
                    c.source_in + (win.start - c.start)
                };
                assert_eq!(
                    clip_slice(&c, win, ClipId(9)).source_in,
                    want,
                    "without warp — the previous linear formula, reversed={reversed}"
                );
            }
        }
    }
    // and a disabled warp slot is not a warp either
    let mut off = warped(0.5, vec![], false);
    off.fx_bypass = vec![true];
    assert_eq!(
        clip_slice(&off, TickRange::new(Tick(2 * 90_000), Tick(4 * 90_000)), ClipId(9)).source_in,
        Tick(90_000 + 2 * 90_000),
        "a disabled warp slot does not bend the cut"
    );
    let mut master = warped(0.5, vec![], false);
    master.fx_off = true;
    assert_eq!(
        clip_slice(&master, TickRange::new(Tick(2 * 90_000), Tick(4 * 90_000)), ClipId(9))
            .source_in,
        Tick(90_000 + 2 * 90_000),
        "the master switch of the chain too"
    );
}

/// Test 288 (R.43, item 298): the veto “do not cut a warped clip” does not cancel
/// the TIME of the block.
///
/// The user’s complaint on a real project: “in problems1 … sound from
/// another event came from somewhere”, “in rendereg the automation curves do not work on the very top
/// event”, “the region looping_5 is somehow broken” — and the deciding hint: “there is a slightly earlier
/// version of the project where at least rendereg works correctly”. It had NOT A
/// SINGLE MORPHER.
///
/// Cause: `windows_on` checked the veto FIRST and returned the whole body of the clip,
/// never asking whether the block reached this clip in time.
/// “Entirely” is about the BOUNDARIES of the piece,
/// not about REACHABILITY.
/// In the user’s project a
/// 0.6 s morph block at second 5020 swapped the source of events at second 148 (“Salut” instead of “far”), and a
/// block with slowed&reverb at second 169 handed out a TIME WARP to all events of the
/// project below its row — including the sound, which has no such thing in the file.
#[test]
fn t288_layer_does_not_reach_a_warped_clip_outside_its_window() {
    let warp = FxInstance::SlowedReverb {
        rate: 0.5,
        mix: 0.0,
        reverb_delay_ms: 60.0,
        damp: 0.4,
        decay: 0.7,
        wet_gain: 1.0,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    };

    // the row of the layer must be ABOVE the track of the clip, otherwise `Below` would not
    // cover it and the test would pass for a completely different reason
    let above = |p: &mut Project| {
        p.set_lane_order(&[(LaneKind::Morph, 0), (LaneKind::Video, 0), (LaneKind::Audio, 0)]);
    };

    // --- 1. a warp on the CLIP ITSELF: the block is far away, nothing to touch ---
    let mut p = base();
    above(&mut p);
    p.clips[0].fx = vec![warp.clone()];
    // the target is `Below`, that is, “everything below the layer row”; the block stands
    // FAR from the clip in time
    let mut m = layer(Tick(10 * 90_000), Tick(90_000));
    m.targets = MorphTargets::Below;
    p.morphers.push(m);
    p.validate().unwrap();

    let rows = p.rows();
    assert!(
        p.morphers[0].covers(&p.clips[0], &rows),
        "the layer must cover the clip SPATIALLY — otherwise the check about TIME          would run by itself"
    );
    let wins = p.morphers[0].windows_on(&p.clips[0], &p, &p, &rows);
    assert!(
        wins.is_empty(),
        "a block at the 10th second has nothing to do with the clip 0..1 s, even if that one \
         is stretched: {wins:?}"
    );
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "the clip is not cut");
    assert_eq!(eff[0].media, MediaId(1), "the source must remain its own");
    assert_eq!(eff[0].fx.len(), 1, "no foreign effects appeared on the clip");

    // --- 2. a warp in the PATCH of the layer: the same prohibition -------------
    let mut p = base();
    above(&mut p);
    let mut m = layer(Tick(10 * 90_000), Tick(90_000));
    m.targets = MorphTargets::Below;
    m.patch = MorphPatch { audio_fx: vec![warp.clone()], ..Default::default() };
    p.morphers.push(m);
    p.clips.push(clip(11, ClipKind::Audio, 0, Tick(0), LEN));
    p.ensure_tracks(LaneKind::Audio, 1);
    p.validate().unwrap();

    let eff = p.effective_clips();
    for c in &eff {
        assert!(
            c.fx.is_empty(),
            "a slowed&reverb layer handed warp to event {:?} 10 seconds away from itself: {:?}",
            c.id,
            c.fx
        );
    }

    // --- 3. but INSIDE the window a warped clip is still patched WHOLE ---------
    let mut p = base();
    above(&mut p);
    p.clips[0].fx = vec![warp];
    let mut m = layer(Tick(30_000), Tick(30_000)); // the middle of the clip
    m.targets = MorphTargets::Below;
    p.morphers.push(m);
    p.validate().unwrap();

    let rows = p.rows();
    let wins = p.morphers[0].windows_on(&p.clips[0], &p, &p, &rows);
    assert_eq!(
        wins,
        vec![TickRange::new(Tick(0), LEN)],
        "a covered stretched clip is patched whole and is NOT cut (R.37)"
    );
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "a stretched clip must not be cut");
    assert_eq!(eff[0].media, MediaId(2), "and the patch applies to it");
}

/// Test 291 (R.43, item 300): a source swap by a layer does NOT lose the event’s own animation.
///
/// The complaint verbatim: “in rendereg for some reason pan3d is not applied to the very top
/// event until I disable the source replacement in the morpher, which
/// is strange”.
///
/// The automation key is pinned to the MATERIAL (t212) —
/// and this is correct, as long as
/// the material is the same: trimming hides and reveals the keys, a split divides the curve.
/// But a source swap by a layer changes the coordinate system itself: for the user’s event
/// `source_in` went from 47.0 s to 99.0 s, while the keys stayed
/// at 47.0..59.6 s — that is, ENTIRELY to the left of the new window. The curve was clamped
/// by its last key, and pan3d became constant: rot_y = 0, rot_z = 0.
/// From the outside this is exactly “pan3d is not applied”.
///
/// The rule is the same as for `clip_slice`: the anchor and the keys move by ONE delta.
#[test]
fn t291_source_replacement_carries_the_clips_own_curves() {
    const STEP: i64 = 90_000 / 30;
    let peak = Tick(3_000 + LEN.0 / 2);

    let make = |replace: bool| -> ClipEvent {
        let mut p = base();
        p.clips[0].fx = vec![FxInstance::Pan3d {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            anchor_x: 0.5,
            anchor_y: 0.5,
            scale: 1.0,
            camera: None,
        }];
        p.clips[0].fx_auto = vec![vec![ParamCurve {
            param: "rot_y".into(),

            keys: AutoKey::linear(vec![
                AutoKey::at(Tick(3_000), 0.0),
                AutoKey::at(peak, 30.0),
                AutoKey::at(Tick(3_000 + LEN.0), 0.0),
            ]),
            source: Default::default(),
        }]];
        let mut m = layer(Tick(0), LEN);
        m.patch = MorphPatch {
            media: replace.then_some(MediaId(2)),
            source_in_delta: if replace { Tick(50 * 90_000) } else { Tick::ZERO },
            ..Default::default()
        };
        // a layer that does nothing at all does not reach the target —
        // with `replace == false` the patch is empty, and this is legitimate
        if replace {
            p.morphers.push(m);
        }
        p.validate().unwrap();
        let eff = p.effective_clips();
        assert_eq!(eff.len(), 1, "the event is not cut");
        eff.into_iter().next().unwrap()
    };

    let plain = make(false);
    let patched = make(true);
    assert_eq!(patched.media, MediaId(2), "the source must be substituted");
    assert_ne!(patched.source_in, plain.source_in, "the entry point must move");

    // the value of the curve in the MIDDLE of the event — where the user put
    // a peak — must remain the same: the animation is pinned to the event, not
    // to the frame number of someone else’s file
    let at = |c: &ClipEvent| {
        let mid = Tick(c.start.0 + c.duration.0 / 2);
        let mat =
            kt_model::clip_material_at(c.source_in, c.start, c.duration, c.reversed, STEP, mid);
        kt_model::curve_value(&c.fx_auto[0][0].keys, mat)
    };
    let (a, b) = (at(&plain), at(&patched));
    assert!((a - 30.0).abs() < 1e-6, "without substitution a curve peak stands mid-event, not {a}");
    assert!(
        (b - a).abs() < 1e-6,
        "after substituting the source the curve must give the same: was {a}, now {b} \
         (before R.43 the keyframes stayed in the OLD material domain and the curve was clamped \
          by the last keyframe — “pan3d not applied”)"
    );
}

/// A clip with a fade at BOTH edges: two transitions with one live end.
/// `in/out_offset` are counted from the JOINT (`joint`), so the fade at the entry keeps
/// `out_offset` alive, and the fade at the exit keeps `in_offset`.
fn with_fades_both_ends(p: &mut Project, id: ClipId, len: Tick, fade: Tick) {
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: None,
        b: Some(id),
        in_offset: Tick::ZERO,
        out_offset: fade,
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.transitions.push(TransitionEvent {
        id: TransitionId(2),
        a: Some(id),
        b: None,
        in_offset: fade,
        out_offset: Tick::ZERO,
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    let _ = len;
}

/// Test 555 (R.55, C2): “by time” on an event with fades means exactly
/// what is written.
///
/// **The owner’s complaint verbatim:** “also on events with a transition (like fade)
/// the morpher breaks and applies the edits to the whole event instead of
/// ‘by time’”.
///
/// The cause was not in the morpher but in the SLICING. A piece inside the transition
/// window cannot be cut — this re-layers the neighbours and changes the crossfade weights —
/// so the boundary was pushed from the forbidden zone to the NEAREST edge,
/// including OUTWARD. On the stage below (fades of 20 000 ticks on both sides, the layer
/// window 5 000 … 85 000) the old code gave `s = 0`, `e = 90 000` —
/// exactly “on the whole event”.
///
/// The cure is not to fix the pushing but not to cut at all: the contribution of the chain
/// is switched off by a strength curve, and then the boundary is exact TO THE TICK, while the transitions and
/// neighbours are not touched at all.
#[test]
fn t555_time_window_is_exact_on_an_event_with_fades() {
    let mut p = base();
    with_fades_both_ends(&mut p, ClipId(10), LEN, Tick(20_000));
    p.validate().expect("a scene with two fades is legal");
    // zones into which a CUT boundary does not stand
    let zones = kt_model::forbidden_zones(&p, &p.clips[0]);
    assert!(zones.iter().any(|z| z.start == Tick(0) && z.end == Tick(20_000)), "fade in");
    assert!(zones.iter().any(|z| z.start == Tick(70_000) && z.end == LEN), "fade out");

    // a layer of EFFECTS ONLY: window 5 000 … 85 000, both ends in zones
    let mut m = layer(Tick(5_000), Tick(80_000));
    m.patch.media = None;
    p.morphers.push(m);

    let wins = p.morphers[0].windows_on(&p.clips[0], &p, &p, &p.rows());
    assert_eq!(
        wins,
        vec![TickRange::new(Tick(5_000), Tick(85_000))],
        "the window must be EXACTLY what was asked: no pushing outward, no snapping to the grid"
    );

    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "no cuts appeared: fading is allowed where cutting is not");
    assert_eq!(eff[0].id, ClipId(10), "the original id is intact — and so is the FX state with it (invariant 3)");
    assert_eq!(eff[0].duration, LEN, "the body is untouched");

    // the GATE of the strength: silent outside the window, full strength inside
    let (i, _) = eff[0]
        .fx
        .iter()
        .enumerate()
        .find(|(_, f)| matches!(f, FxInstance::LayerMix { .. }))
        .expect("a strength slot must exist: without it the layer hits the whole body");
    let cur = &eff[0].fx_auto[i][0];
    let at = |t: i64| {
        let ks = &cur.keys;
        let j = ks.partition_point(|k| k.t.0 <= t).saturating_sub(1);
        ks[j].v
    };
    // the domain of the keys is the MATERIAL of the target (`source_in` = 3 000)
    let mat = |tl: i64| tl + 3_000;
    assert!(at(mat(1_000)) < 0.01, "before the window the layer must be silent");
    assert!(at(mat(45_000)) > 0.99, "inside the window — full strength");
    assert!(at(mat(88_000)) < 0.01, "after the window the layer must be silent");
    // and the BOUNDARY IS EXACT: a tick before the window it is still zero, at the very edge it is already the strength
    assert!(at(mat(4_999)) < 0.01, "the boundary moved left");
    assert!(at(mat(5_000)) > 0.99, "the boundary moved right");

    // TRANSITIONS INTACT: their windows did not move, the ends were not lost
    let after = kt_model::forbidden_zones(&p, &p.clips[0]);
    assert_eq!(after, zones, "the layer touched the transition windows — neighbors would have been overlaid");
}

/// Test 556 (R.55, C2): a patch with a SOURCE SWAP still cuts, and the boundary moves ONLY
/// INWARD.
///
/// “Taking a frame from another file” cannot be expressed as a strength curve, so here the
/// cut remains. But the pushing has stopped being “to the nearest edge”:
/// the window now only SHRINKS. On the same stage the old code gave the whole
/// body, the new one — the middle between the fades.
#[test]
fn t556_source_replacing_layer_still_cuts_but_only_inward() {
    let mut p = base();
    with_fades_both_ends(&mut p, ClipId(10), LEN, Tick(20_000));
    p.morphers.push(layer(Tick(5_000), Tick(80_000))); // patch.media = Some(2)
    p.validate().expect("the scene is legal");

    let wins = p.morphers[0].windows_on(&p.clips[0], &p, &p, &p.rows());
    assert_eq!(
        wins,
        vec![TickRange::new(Tick(20_000), Tick(70_000))],
        "boundaries must move INWARD, to the far edges of the zones, not outward"
    );

    let all = p.effective_clips();
    let eff: Vec<&ClipEvent> = all.iter().filter(|c| c.kind == ClipKind::Video).collect();
    assert_eq!(eff.len(), 3, "three pieces: before the fade, covered, after");
    assert_eq!(eff[0].start, Tick(0));
    assert_eq!(eff[1].start, Tick(20_000), "the covered piece starts AFTER the fade window");
    assert_eq!(eff[2].start, Tick(70_000), "and ends BEFORE the second fade window");
    assert_eq!(eff[1].media, MediaId(2), "the substitution went to the middle piece");
    assert_eq!(eff[0].media, MediaId(1), "the layer did not cover the fade in");
    assert_eq!(eff[2].media, MediaId(1), "and the fade out too");
}
