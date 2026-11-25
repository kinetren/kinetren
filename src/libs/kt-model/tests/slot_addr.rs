//! found-defects 906 (capability `compositions`, `automation`): an address of an effect slot follows the
//! slot — a reel's published parameter, a link's text in any curve, a slot's mask — through a move, an
//! insertion, a removal and the model's own removals, in the edit's undo step.

use kt_model::slot_addr::{map_of, matching};
use kt_model::*;
use kt_time::Tick;

const SEQ: MediaId = MediaId(50);

fn fx(id: &str) -> FxInstance {
    FxInstance::plugin(id).unwrap()
}

fn event(id: u64, kind: ClipKind, media: MediaId, fx: Vec<FxInstance>) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind,
        lane: 0,
        start: Tick(0),
        duration: Tick(900_000),
        source_in: Tick(0),
        media,
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    }
}

fn link(param: &str, path: &str) -> ParamCurve {
    ParamCurve { param: param.into(), keys: vec![], source: CurveSource::Link { path: path.into(), expr: None, stops: vec![] } }
}

fn paths(list: &[ParamCurve]) -> Vec<String> {
    list.iter().map(|c| c.source.link_path().unwrap_or("-").to_string()).collect()
}

/// A reel of two events — 1: blur, glow, vignette; 2: a blur led by event 1's glow — with the glow's
/// radius published, instanced by the root's event 9 with a curve on the handle led by `self`'s vignette.
fn project() -> Project {
    kt_plugins_builtin::install();
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "a.mp4"));
    let mut a = event(1, ClipKind::Video, MediaId(1), vec![fx("blur"), fx("glow"), fx("vignette")]);
    // its own blur is led by its own vignette, and its vignette's mask by the glow
    a.fx_auto = vec![vec![link("radius", "self/fx2/strength")]];
    let mut mask = Mask::new(7, "m", MaskShape::Rect { cx: 0.5, cy: 0.5, w: 0.5, h: 0.5, rot: 0.0, round: 0.0 });
    mask.auto = vec![link("cx", "1/fx1/strength")];
    a.masks.list_mut(Some(2)).push(mask);
    let mut b = event(2, ClipKind::Video, MediaId(1), vec![fx("blur")]);
    b.lane = 1;
    b.fx_auto = vec![vec![link("radius", "1/fx1/strength")]];
    let mut s = Sequence::new(SEQ, "reel");
    s.clips = vec![a, b];
    s.ensure_tracks(LaneKind::Video, 2);
    s.published.push(PublishedParam {
        id: PubId(1),
        name: "glow".into(),
        target: ParamAddr::new(ClipId(1), ParamOwner::FxSlot { slot: 1 }, "strength"),
        ty: ParamType::Num,
        range: Default::default(),
    });
    p.sequences.push(s);
    let mut inst = event(9, ClipKind::Video, SEQ, vec![]);
    inst.overrides = vec![Override { pub_id: PubId(1), value: ParamValue::Num(0.5), curves: vec![link("strength", "self/fx2/strength")] }];
    p.clips.push(inst);
    p
}

fn model() -> TimelineModel {
    let mut p = project();
    p.normalize();
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);
    m.set_editing(Some(SEQ));
    m
}

fn a(m: &TimelineModel) -> ClipEvent {
    m.project().clips.iter().find(|c| c.id == ClipId(1)).unwrap().clone()
}

/// Every address of event 1's slots in the document, as texts.
fn addresses(m: &TimelineModel) -> (Vec<(ParamOwner, String)>, Vec<String>) {
    let d = m.document();
    let s = d.sequence(SEQ).unwrap();
    let published = s.published.iter().map(|p| (p.target.owner.clone(), p.target.param.clone())).collect();
    let one = s.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let two = s.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    let mut links: Vec<String> = one.fx_auto.iter().flat_map(|l| paths(l)).collect();
    links.extend(one.masks.iter().flat_map(|(_, m)| paths(&m.auto)));
    links.extend(two.fx_auto.iter().flat_map(|l| paths(l)));
    links.extend(d.clips.iter().flat_map(|c| c.overrides.iter()).flat_map(|o| paths(&o.curves)));
    (published, links)
}

/// A chain edit as the editor sends it: the slots by `from`, their lists, then the addresses.
fn edit(m: &mut TimelineModel, new: Vec<FxInstance>, from: &[Option<usize>]) {
    let c = a(m);
    let pick = |l: &dyn Fn(usize) -> Vec<ParamCurve>| from.iter().map(|f| f.map(l).unwrap_or_default()).collect::<Vec<_>>();
    let mut masks = c.masks.clone();
    masks.slots = from.iter().map(|f| f.map(|i| c.masks.slot(i).to_vec()).unwrap_or_default()).collect();
    masks.canonicalize();
    let mut auto = pick(&|i| c.fx_auto.get(i).cloned().unwrap_or_default());
    while auto.last().is_some_and(Vec::is_empty) {
        auto.pop();
    }
    m.apply_batch(vec![
        Command::SetFx { id: ClipId(1), fx: new },
        Command::SetFxAuto { id: ClipId(1), auto },
        Command::SetMasks { id: ClipId(1), masks },
        Command::MapFxSlots { id: ClipId(1), to: map_of(from, c.fx.len()) },
    ])
    .unwrap();
}

/// Test t3244 (found-defects 906): after a move, an insertion in the middle and a removal every address of
/// the event's slots names the effect it named before; a removed leader frees its followers and drops its
/// publication (and the instances' values of it); each is one undo step.
#[test]
fn t3244_addresses_follow_the_slots() {
    let mut m = model();
    let start = addresses(&m);
    assert_eq!(start.1, ["self/fx2/strength", "1/fx1/strength", "1/fx1/strength", "self/fx2/strength"], "scene");
    // the glow moved to the front: glow blur vignette
    let c = a(&m);
    edit(&mut m, vec![c.fx[1].clone(), c.fx[0].clone(), c.fx[2].clone()], &[Some(1), Some(0), Some(2)]);
    let (published, links) = addresses(&m);
    assert_eq!(published, [(ParamOwner::FxSlot { slot: 0 }, "strength".to_string())], "the handle lost its glow");
    assert_eq!(links, ["self/fx2/strength", "1/fx0/strength", "1/fx0/strength", "self/fx2/strength"]);
    m.undo().unwrap();
    assert_eq!(addresses(&m), start, "one undo did not give the addresses back");
    // a slot inserted in the middle (an arm before the Output): blur NEW glow vignette
    let c = a(&m);
    edit(&mut m, vec![c.fx[0].clone(), fx("duotone"), c.fx[1].clone(), c.fx[2].clone()], &[Some(0), None, Some(1), Some(2)]);
    let (published, links) = addresses(&m);
    assert_eq!(published[0].0, ParamOwner::FxSlot { slot: 2 });
    assert_eq!(links, ["self/fx3/strength", "1/fx2/strength", "1/fx2/strength", "self/fx3/strength"]);
    m.undo().unwrap();
    // the glow removed: its followers are free, the handle and the instance's value of it are gone, and
    // the vignette's addresses follow it
    let c = a(&m);
    edit(&mut m, vec![c.fx[0].clone(), c.fx[2].clone()], &[Some(0), Some(2)]);
    let (published, links) = addresses(&m);
    assert!(published.is_empty(), "a handle outlived its effect: {published:?}");
    assert_eq!(links, ["self/fx1/strength"], "a follower kept an address of nothing (or of another effect)");
    assert!(m.document().clips.iter().all(|c| c.overrides.is_empty()), "an instance kept the value of a handle that is gone");
    m.undo().unwrap();
    assert_eq!(addresses(&m), start);
    // without the map the same move binds the handle to another effect: what 906 was
    let c = a(&m);
    m.apply(Command::SetFx { id: ClipId(1), fx: vec![c.fx[1].clone(), c.fx[0].clone(), c.fx[2].clone()] }).unwrap();
    assert_ne!(addresses(&m).0.first().map(|p| p.0.clone()), Some(ParamOwner::FxSlot { slot: 0 }), "scene: SetFx alone moves no address");
}

/// Test t3246 (found-defects 906): the MODEL's own removals move the addresses too — a device's half
/// mirrored off its partner (with the slot's masks and the Geometry's place), and the pair's speed taken
/// off with the partner's.
#[test]
fn t3246_a_mirrored_removal_moves_the_addresses() {
    kt_plugins_builtin::install();
    let mut p = project();
    p.sequences.clear();
    p.clips.clear();
    let mut v = event(10, ClipKind::Video, MediaId(1), vec![fx("vhs_tape"), FxInstance::slowed_reverb(0.5, 0.3, 80.0), fx("blur"), fx("glow")]);
    let mut s = event(11, ClipKind::Audio, MediaId(1), vec![fx("vhs_tape_sound"), FxInstance::slowed_reverb(0.5, 0.3, 80.0), fx("gain")]);
    (v.link_group, s.link_group) = (Some(GroupId(1)), Some(GroupId(1)));
    v.geometry_pos = 3;
    v.masks.list_mut(Some(3)).push(Mask::new(1, "m", MaskShape::Rect { cx: 0.5, cy: 0.5, w: 0.5, h: 0.5, rot: 0.0, round: 0.0 }));
    let mut w = event(12, ClipKind::Video, MediaId(1), vec![fx("blur")]);
    w.lane = 1;
    w.fx_auto = vec![vec![link("radius", "10/fx3/strength")]];
    p.clips = vec![v, s, w];
    p.ensure_tracks(LaneKind::Video, 2);
    p.normalize();
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);
    let of = |m: &TimelineModel, id: u64| m.project().clips.iter().find(|c| c.id == ClipId(id)).unwrap().clone();
    // the sound's tape removed: the picture's goes by the mirror
    let s = of(&m, 11);
    m.apply_batch(vec![
        Command::SetFx { id: ClipId(11), fx: s.fx[1..].to_vec() },
        Command::MapFxSlots { id: ClipId(11), to: kt_model::slot_addr::without(3, 0) },
    ])
    .unwrap();
    let v = of(&m, 10);
    assert_eq!(v.fx.iter().map(|f| f.tag()).collect::<Vec<_>>(), ["slowed_reverb", "blur", "glow"], "scene");
    assert_eq!(paths(&of(&m, 12).fx_auto[0]), ["10/fx2/strength"], "the link follows no more its glow");
    assert_eq!((v.geometry_pos, v.masks.slot(2).len()), (2, 1), "the slot's mask or the Geometry's place stayed behind");
    // the sound's speed taken off: the picture's goes by the mirror, its masks and addresses with it
    let s = of(&m, 11);
    m.apply_batch(vec![
        Command::SetFx { id: ClipId(11), fx: s.fx[1..].to_vec() },
        Command::MapFxSlots { id: ClipId(11), to: kt_model::slot_addr::without(2, 0) },
    ])
    .unwrap();
    let v = of(&m, 10);
    assert_eq!(v.fx.iter().map(|f| f.tag()).collect::<Vec<_>>(), ["blur", "glow"], "scene");
    assert_eq!(paths(&of(&m, 12).fx_auto[0]), ["10/fx1/strength"]);
    assert_eq!((v.geometry_pos, v.masks.slot(1).len()), (1, 1), "the mask of the glow stayed behind the speed's slot");
}

/// Test t3247 (found-defects 906): the chain a window's Cancel gives back is matched with the chain there
/// is — the equal instance in its own place, then the equal one elsewhere, then the same kind in order.
#[test]
fn t3247_a_restored_chain_is_matched_by_its_slots() {
    kt_plugins_builtin::install();
    let wide = FxInstance::blur(9);
    let then = vec![FxInstance::blur(2), fx("glow"), FxInstance::blur(2)];
    assert_eq!(matching(&then, &then), [Some(0), Some(1), Some(2)], "equal twins are not swapped");
    let now = vec![fx("glow"), wide.clone(), FxInstance::blur(2)];
    assert_eq!(matching(&then, &now), [Some(1), Some(0), Some(2)]);
    assert_eq!(map_of(&matching(&then, &now), now.len()), [Some(1), Some(0), Some(2)]);
    let grown = vec![fx("duotone"), FxInstance::blur(2), fx("glow"), FxInstance::blur(2)];
    assert_eq!(map_of(&matching(&then, &grown), grown.len()), [None, Some(0), Some(1), Some(2)]);
}
