//! fx-graph v1b (capability `ui-reach`): the node editor in the windows that edit a chain — a
//! device's halves in one column, a drag that reorders within its lane and is refused across lanes,
//! a cross on every node, the fit without scrolling, a huge device kept small.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::layout_tests::harness;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use kt_model::*;

fn clip(id: u64, kind: ClipKind, fx: Vec<FxInstance>) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: Some(GroupId(1)),
        kind,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
    }
}

/// A linked pair under a morpher layer carrying `patch`; the layer's window is open.
fn morpher_harness(patch: MorphPatch) -> Harness<'static, KtApp> {
    crate::plugins::install();
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(clip(1, ClipKind::Video, vec![]));
    p.clips.push(clip(2, ClipKind::Audio, vec![]));
    p.ensure_tracks(LaneKind::Morph, 1);
    p.validate().unwrap();
    app.model = TimelineModel::new(p);
    assert!(app.apply(Command::AddMorpher(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(1), ClipId(2)] },
        patch,
        timed: true,
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
        enabled: true,
    })));
    app.open_morph_editor(MorphId(1));
    h.step();
    h.step();
    h
}

/// The name label of a node (the speaker marks a device's sound half).
fn node_label(h: &Harness<'_, KtApp>, name: &str) -> egui::Rect {
    h.query_by_label(name).unwrap_or_else(|| panic!("no node “{name}”")).rect()
}

fn sound(name: &str) -> String {
    format!("{} {name}", crate::icons::CAT_VOLUME)
}

/// A drag of the mouse from one place to another, as explicit events.
fn drag(h: &mut Harness<'_, KtApp>, from: egui::Pos2, to: egui::Pos2) {
    h.hover_at(from);
    h.step();
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.step();
    for n in 1..=8 {
        let t = n as f32 / 8.0;
        h.hover_at(from + (to - from) * t);
        h.step();
    }
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.step();
    h.step();
}

fn patch(h: &Harness<'_, KtApp>) -> MorphPatch {
    h.state().model.project().morphers[0].patch.clone()
}

fn tags(fx: &[FxInstance]) -> Vec<&str> {
    fx.iter().map(|f| f.tag()).collect()
}

/// t2994 (fx-graph v1b): a device is one node — in the morpher's node editor VHS Tape's picture half
/// stands over its sound half in one column, the sound half marked with the speaker; the other
/// effects keep their lanes (the blur on the picture, the gain on the sound).
/// - **PATH** double click a morpher block carrying a Blur, VHS Tape and a Gain → the morpher window
///   shows its node editor: the purple source node, Blur and VHS Tape on the picture lane, Gain and
///   VHS Tape's sound on the sound lane, the tape's two halves one above the other in one frame
#[test]
fn t2994_a_device_s_halves_stand_in_one_column() {
    crate::plugins::install();
    let mut h = morpher_harness(MorphPatch {
        video_fx: vec![FxInstance::blur(4), FxInstance::builtin("vhs_tape")],
        audio_fx: vec![FxInstance::plugin("gain").unwrap()],
        ..Default::default()
    });
    h.step();
    let p = patch(&h);
    assert_eq!(tags(&p.audio_fx), ["gain", "vhs_tape_sound"], "the layer does not hold the tape's sound");
    let tape = node_label(&h, "VHS Tape");
    let tape_sound = node_label(&h, &sound("VHS Tape"));
    let blur = node_label(&h, crate::fx_editor::fx_display_name(&FxInstance::blur(4)));
    let gain = node_label(&h, crate::fx_editor::fx_display_name(&FxInstance::plugin("gain").unwrap()));
    assert!((tape.left() - tape_sound.left()).abs() < 1.0, "the halves are not one column: {tape:?} {tape_sound:?}");
    assert!(tape_sound.top() > tape.bottom(), "the sound half is not under the picture half");
    assert!((blur.top() - tape.top()).abs() < 1.0, "Blur left the picture lane");
    assert!((gain.top() - tape_sound.top()).abs() < 1.0, "Gain left the sound lane");
    assert!(blur.left() < tape.left() && gain.left() < tape_sound.left(), "the order of the lanes is not the chain's");
    assert!(crate::widgets::field_probe::rect_of("fx.graph").is_some(), "no node editor in the morpher window");
}

/// t2995 (fx-graph v1b): a drag reorders within its lane and is refused across lanes; every node
/// has its cross, and the device's cross takes both halves.
/// - **PATH** in the morpher's node editor drag Blur onto VHS Tape → Blur stands after the tape →
///   drag Gain onto VHS Tape (another lane) → nothing changes → click the cross of VHS Tape's
///   picture half → both halves go
#[test]
fn t2995_reorder_within_a_lane_refuse_across_remove_a_device() {
    crate::plugins::install();
    let mut h = morpher_harness(MorphPatch {
        video_fx: vec![FxInstance::blur(4), FxInstance::builtin("vhs_tape")],
        audio_fx: vec![FxInstance::plugin("gain").unwrap()],
        ..Default::default()
    });
    let blur_name = crate::fx_editor::fx_display_name(&FxInstance::blur(4));
    let gain_name = crate::fx_editor::fx_display_name(&FxInstance::plugin("gain").unwrap());
    let (from, to) = (node_label(&h, blur_name).center(), node_label(&h, "VHS Tape").center());
    drag(&mut h, from, to);
    assert_eq!(tags(&patch(&h).video_fx), ["vhs_tape", "blur"], "the drag within the lane did not reorder");
    let before = patch(&h);
    let (from, to) = (node_label(&h, gain_name).center(), node_label(&h, "VHS Tape").center());
    drag(&mut h, from, to);
    assert_eq!(patch(&h), before, "a drag across the lanes changed the chain");
    // every node has its cross; the tape's (on its row, right of its name) takes both halves
    let crosses: Vec<egui::Rect> = h.query_all_by_label(crate::icons::CLEAR).map(|n| n.rect()).collect();
    assert!(crosses.len() >= 4, "a node without its cross: {}", crosses.len());
    let tape = node_label(&h, "VHS Tape");
    let x = crosses
        .iter()
        .filter(|r| (r.center().y - tape.center().y).abs() < 8.0 && r.left() > tape.right() - 1.0)
        .min_by(|a, b| a.left().total_cmp(&b.left()))
        .copied()
        .expect("VHS Tape's cross");
    h.hover_at(x.center());
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: x.center(), button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.step();
    h.step();
    let p = patch(&h);
    assert_eq!(tags(&p.video_fx), ["blur"], "the device's cross left a half on the picture");
    assert_eq!(tags(&p.audio_fx), ["gain"], "the device's cross left its sound");
}

/// The event's effects window over one video event carrying `fx`.
fn fx_harness(fx: Vec<FxInstance>) -> Harness<'static, KtApp> {
    crate::plugins::install();
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let mut c = clip(1, ClipKind::Video, fx);
    c.link_group = None;
    c.fx_bypass = vec![false; c.fx.len()];
    p.clips.push(c);
    p.validate().unwrap();
    app.model = TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    h
}

fn fx_window(h: &Harness<'_, KtApp>) -> egui::Rect {
    h.ctx.memory(|m| m.area_rect(egui::Id::new(("fx", ClipId(1))))).expect("the effects window")
}

/// t2999 (fx-graph D4a): a huge device stays a small node — Camera ISP's node shows at most three
/// rows whatever its passport holds, and dragging its exposure slider in the node sets the row in
/// the model in one step.
/// - **PATH** add Camera ISP to an event → its node shows two or three sliders, no more → drag the
///   exposure slider in the node to the right → the preview brightens; the ISP panel stays below
#[test]
fn t2999_a_huge_device_stays_a_small_node() {
    crate::plugins::install();
    let isp = FxInstance::plugin("camera_isp").expect("camera_isp");
    let rows = isp.param_specs().len();
    let mut h = fx_harness(vec![isp]);
    // the ISP's node: the one holding its name (the event's leading Geometry node stands first)
    let name = node_label(&h, "Camera ISP");
    let node = (0..4)
        .filter_map(|i| crate::widgets::field_probe::rect_of(&format!("fx.node.{i}")))
        .find(|r| r.contains_rect(name))
        .expect("the ISP node");
    let sliders: Vec<egui::Rect> =
        h.query_all_by_role(egui::accesskit::Role::Slider).map(|n| n.rect()).filter(|r| node.contains_rect(*r)).collect();
    assert!(!sliders.is_empty() && sliders.len() <= 3, "{} sliders in a node of a {rows}-row device", sliders.len());
    assert!(node.height() <= 22.0 + 3.0 * 22.0 + 4.0 + 0.5, "the node grows with the passport: {node:?}");
    let before = h.state().model.project().clips[0].fx[0].get_param("exposure_ev").unwrap();
    let s = sliders[0];
    drag(&mut h, s.center(), s.center() + egui::vec2(s.width() * 0.4, 0.0));
    let after = h.state().model.project().clips[0].fx[0].get_param("exposure_ev").unwrap();
    assert!(after > before, "the node's exposure slider did not reach the model: {before} → {after}");
}

/// t3001 (fx-graph v1b): a long chain fits without scrolling — 24 effects open as chips inside the
/// window's width; Ctrl+wheel over the graph opens the detailed nodes, "Frame all" returns the fit.
/// - **PATH** open the effects window of an event with 24 effects → every node is in view as a chip
///   → Ctrl+wheel over the graph → the detailed nodes with their sliders → click "Frame all" → all
///   the chips again
#[test]
fn t3001_a_long_chain_fits_without_scrolling() {
    crate::plugins::install();
    let fx: Vec<FxInstance> = (0..24).map(|i| if i % 2 == 0 { FxInstance::blur(2) } else { FxInstance::builtin("vignette") }).collect();
    let mut h = fx_harness(fx);
    let win = fx_window(&h);
    let nodes = |_: &Harness<'_, KtApp>| -> Vec<egui::Rect> {
        (0..24).map(|i| crate::widgets::field_probe::rect_of(&format!("fx.node.{i}")).expect("a node")).collect()
    };
    for r in nodes(&h) {
        assert!(r.left() >= win.left() && r.right() <= win.right(), "a node outside the window's width: {r:?} in {win:?}");
        assert!(r.height() < 30.0, "the long chain did not fold to chips: {r:?}");
    }
    let graph = crate::widgets::field_probe::rect_of("fx.graph").expect("the graph");
    h.hover_at(graph.center());
    h.step();
    h.event(egui::Event::Zoom(1.3));
    h.step();
    h.step();
    assert!(nodes(&h).iter().any(|r| r.height() > 30.0), "Ctrl+wheel did not open the detailed nodes");
    h.get_by_label(tr(k::fx::graph::FIT)).click();
    h.step();
    h.step();
    assert!(nodes(&h).iter().all(|r| r.height() < 30.0), "Frame all did not return the fit");
}

/// t3000 (fx-graph v1b-live, found-defects 811): a single-lane SOUND chain draws its nodes — an
/// audio event's effects window shows its Gain and Pitch Shift as nodes, not In → Out alone.
/// - **PATH** double click an audio event carrying Gain and Pitch Shift → its effects window shows
///   both as nodes between In and Out
#[test]
fn t3000_an_audio_event_s_chain_draws_its_nodes() {
    crate::plugins::install();
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:sine"));
    let mut a = clip(1, ClipKind::Audio, vec![FxInstance::plugin("gain").unwrap(), FxInstance::plugin("pitch_shift").unwrap()]);
    a.link_group = None;
    a.fx_bypass = vec![false, false];
    p.clips.push(a);
    p.validate().unwrap();
    app.model = TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    for i in 0..2 {
        assert!(crate::widgets::field_probe::rect_of(&format!("fx.node.{i}")).is_some(), "the audio event's node {i} is not drawn");
    }
}
