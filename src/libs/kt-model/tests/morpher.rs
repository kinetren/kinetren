//! Test 35 (docs/porting-notes.md): the morpher system in the model.
//! A port of v1 with the correction of the defects of reconnaissance: D1 (FX batch live),
//! D5 (cleanup/inheritance of targets), D6 (undo for everything), D11 (order by id).

use kt_model::*;
use kt_time::Tick;

fn base_project() -> TimelineModel {
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
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(10),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(9_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::gain(-3.0)],
        fx_bypass: Vec::new(),
        fx_auto: vec![],
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
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    TimelineModel::new(p)
}

fn replace_morpher(id: u64, targets: Vec<ClipId>) -> MorpherEvent {
    MorpherEvent {
        id: MorphId(id),
        lane: 0,
        start: Tick(0),
        duration: Tick(90 * 1500),
        targets: MorphTargets::Clips { ids: targets },
        patch: MorphPatch {
            media: Some(MediaId(2)),
            source_in_delta: Tick(4_500),
            absolute_source_in: None,
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    }
}

/// Replace: a source swap + a shift of the entry point only on the targets;
/// the project’s own clips are untouched (flatten is a copy).
#[test]
fn t35_replace_flatten() {
    let mut m = base_project();
    m.apply(Command::AddMorpher(replace_morpher(1, vec![ClipId(10)]))).unwrap();

    let eff = m.project().effective_clips();
    let c10 = eff.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(c10.media, MediaId(2), "the source is substituted");
    assert_eq!(c10.source_in, Tick(13_500), "source_in shifted by delta");
    let c11 = eff.iter().find(|c| c.id == ClipId(11)).unwrap();
    assert_eq!(c11.media, MediaId(1), "a non-target is untouched");
    // the original clips of the project are not changed
    assert_eq!(m.project().clips[0].media, MediaId(1));

    // absolute takes precedence over delta (D4: the field is full-fledged)
    let mut abs = replace_morpher(1, vec![ClipId(10)]);
    abs.patch.absolute_source_in = Some(Tick(1_000));
    m.apply(Command::SetMorpher(abs)).unwrap();
    let eff = m.project().effective_clips();
    assert_eq!(eff.iter().find(|c| c.id == ClipId(10)).unwrap().source_in, Tick(1_000));

    // a negative result is clamped to 0
    let mut neg = replace_morpher(1, vec![ClipId(10)]);
    neg.patch.source_in_delta = Tick(-90_000);
    m.apply(Command::SetMorpher(neg)).unwrap();
    let eff = m.project().effective_clips();
    assert_eq!(eff.iter().find(|c| c.id == ClipId(10)).unwrap().source_in, Tick(0));
}

/// FX batch acts LIVE (fix of D1): effective appends the chain of the required kind
/// (video FX — only to video clips, audio — only to audio clips).
#[test]
fn t35_effect_batch_live_and_typed() {
    let mut m = base_project();
    m.apply(Command::AddMorpher(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(90 * 1500),
        targets: MorphTargets::Clips { ids: vec![ClipId(10), ClipId(11)] },
        patch: MorphPatch {
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            audio_fx: vec![FxInstance::gain(6.0)],
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    }))
    .unwrap();

    let eff = m.project().effective_clips();
    let a = eff.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(a.fx.len(), 2, "audio clip: original Gain + appended one");
    assert!(matches!(a.fx[1], FxInstance::Gain { db, .. } if db == 6.0));
    let v = eff.iter().find(|c| c.id == ClipId(11)).unwrap();
    assert_eq!(v.fx.len(), 1);
    assert!(matches!(v.fx[0], FxInstance::Mirror { .. }));
    // the appended slots are active (fx_bypass is shorter than the chain)
    assert_eq!(a.active_fx().len(), 2);
}

/// Bake == flatten: the render view before the bake and the content of the clips after the bake
/// coincide; the morpher disappears; undo restores everything (D6).
#[test]
fn t35_bake_equals_flatten_and_undo() {
    let mut m = base_project();
    m.apply(Command::AddMorpher(replace_morpher(1, vec![ClipId(10)]))).unwrap();
    let before = m.project().effective_clips();

    m.apply(Command::BakeMorpher { id: MorphId(1) }).unwrap();
    assert!(m.project().morphers.is_empty(), "bake removes the event");
    assert_eq!(m.project().clips, before, "bake writes exactly what was rendered");
    assert_eq!(m.project().effective_clips(), before, "render did not change");

    m.undo().unwrap();
    assert_eq!(m.project().morphers.len(), 1, "undo restores the morpher");
    assert_eq!(m.project().clips[0].media, MediaId(1), "and the original clips");
}

/// The order of application is deterministic by id (D11): two Replace on one clip —
/// the later one wins (cumulatively), regardless of the order in the vector.
#[test]
fn t35_deterministic_order_by_id() {
    let mut m = base_project();
    let mut late = replace_morpher(7, vec![ClipId(10)]);
    late.patch.media = None;
    late.patch.absolute_source_in = Some(Tick(777));
    // we add the LATER id first — the order of the vector is reversed
    m.apply(Command::AddMorpher(late)).unwrap();
    m.apply(Command::AddMorpher(replace_morpher(3, vec![ClipId(10)]))).unwrap();

    let eff = m.project().effective_clips();
    let c = eff.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(c.media, MediaId(2), "morpher id=3 substituted the source");
    assert_eq!(c.source_in, Tick(777), "morpher id=7 applied AFTER id=3");
}

/// D5: deleting a clip cleans it out of targets; split adds the right
/// half; events with emptied targets remain (as in v1), but are harmless.
#[test]
fn t35_targets_follow_clip_edits() {
    let mut m = base_project();
    m.apply(Command::AddMorpher(replace_morpher(1, vec![ClipId(10), ClipId(11)]))).unwrap();

    m.apply(Command::Split {
        id: ClipId(10),
        at: Tick(45_000),
        new_id: ClipId(12),
        new_group: None,
    })
    .unwrap();
    assert!(
        m.project().morphers[0].targets.clip_ids().contains(&ClipId(12)),
        "the right half inherited the targeting"
    );
    let eff = m.project().effective_clips();
    assert_eq!(eff.iter().find(|c| c.id == ClipId(12)).unwrap().media, MediaId(2));

    m.apply(Command::RemoveClip { id: ClipId(10) }).unwrap();
    assert!(!m.project().morphers[0].targets.clip_ids().contains(&ClipId(10)), "the removed one is cleaned up");
    assert!(m.project().validate().is_ok(), "the model is valid after edits");
}

/// Tracks: insertion shifts lane, deletion removes the events of the track; a
/// round trip save→load keeps the morphers byte-for-byte.
#[test]
fn t35_lanes_and_roundtrip() {
    let mut m = base_project();
    let mut ev = replace_morpher(1, vec![ClipId(10)]);
    ev.lane = 0;
    m.apply(Command::AddMorpher(ev)).unwrap();
    assert_eq!(m.project().lanes(LaneKind::Morph), 1);

    m.apply(Command::InsertTrack { kind: LaneKind::Morph, at: 0 }).unwrap();
    assert_eq!(m.project().morphers[0].lane, 1, "insertion shifted the event");

    let json = serde_json::to_string(m.project()).unwrap();
    let loaded = Project::from_json(&json).unwrap();
    assert_eq!(&loaded, m.project(), "round-trip");

    m.apply(Command::RemoveTrack { kind: LaneKind::Morph, at: 1 }).unwrap();
    assert!(m.project().morphers.is_empty(), "track events removed");
    assert_eq!(m.project().lanes(LaneKind::Morph), 1);

    // validation: a morpher cannot refer to a nonexistent clip
    let mut bad = m.project().clone();
    bad.morphers.push(replace_morpher(9, vec![ClipId(999)]));
    assert!(matches!(bad.validate(), Err(ProjectError::MorphUnknownTarget(..))));
}
