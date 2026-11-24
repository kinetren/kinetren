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
        fx_in: Vec::new(),
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

/// t2996 (fx-graph v1c, the owner's Q3 "Да, коробками"): what applies to an event AFTER its own
/// effects stands in its graph as read-only boxes in the render's order — the morpher layers over
/// it bottom up, then its track's stage with the sidechain key — and a double click on a layer's
/// box opens that layer's window; the track window draws the key as a box wired into its compressor.
/// - **PATH** double click an audio event under two morpher layers on a track whose compressor is
///   keyed by the music track → its effects window shows the event's nodes, then two purple layer
///   boxes, then the track's box with "Key: …" → double click the first layer box → that morpher's
///   window opens → open the track's window → the key box stands under its compressor
#[test]
fn t2996_layers_and_the_track_stand_after_the_event() {
    crate::plugins::install();
    // a double click is two presses closer than egui's 0.3 s: the harness steps 20 ms
    let mut h = crate::layout_tests::harness_lang_dt(crate::i18n::Lang::Ru, Some(0.02));
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:sine"));
    let mut a = clip(1, ClipKind::Audio, vec![FxInstance::plugin("gain").unwrap()]);
    a.link_group = None;
    p.clips.push(a);
    p.ensure_tracks(LaneKind::Audio, 2);
    p.ensure_tracks(LaneKind::Morph, 2);
    let music = p.track_at(LaneKind::Audio, 1).unwrap().id;
    let mut comp = FxInstance::plugin("compressor").unwrap();
    assert!(comp.set_sidechain_key(Some(music)));
    let track = p.track_at(LaneKind::Audio, 0).unwrap().id;
    if let Some(t) = p.track_at_mut(LaneKind::Audio, 0) {
        t.fx = vec![comp];
        t.fx_bypass = vec![false];
    }
    for (id, lane) in [(1u64, 0u32), (2, 1)] {
        p.morphers.push(MorpherEvent {
            id: MorphId(id),
            lane,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(180_000),
            targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
            patch: MorphPatch { audio_fx: vec![FxInstance::plugin("gain").unwrap()], ..Default::default() },
            timed: true,
            strength: 1.0,
            fade_in: kt_time::Tick::ZERO,
            fade_out: kt_time::Tick::ZERO,
            enabled: true,
        });
    }
    p.validate().unwrap();
    let order = p.layers_over(&p.clips[0].clone());
    assert_eq!(order.len(), 2, "both layers reach the event");
    app.model = TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    let boxes: Vec<egui::Rect> = (0..3).map(|n| crate::widgets::field_probe::rect_of(&format!("fx.ctx.{n}")).expect("a context box")).collect();
    // the reading order: a band, then left to right (the chips wrap into bands)
    let read = |r: &egui::Rect| (r.top().round() as i32, r.left().round() as i32);
    assert!(read(&boxes[0]) < read(&boxes[1]) && read(&boxes[1]) < read(&boxes[2]), "the boxes are not in the render's order: {boxes:?}");
    let gain = crate::widgets::field_probe::rect_of("fx.node.0").expect("the event's own node");
    assert!(read(&gain) < read(&boxes[0]), "a layer stands before the event's own effects");
    let first = tr(k::fx::graph::LAYER).replace("{0}", &order[0].0.to_string());
    assert!(h.query_all_by_label_contains(&first).next().is_some(), "the first box is not the lower layer “{first}”");
    let music_name = crate::track_window::track_name(h.state().model.project(), music).unwrap();
    let key = tr(k::fx::graph::KEY).replace("{0}", &music_name);
    assert!(h.query_all_by_label_contains(&key).next().is_some(), "the track's box does not name its key “{key}”");
    // a double click on the first layer's box opens its window
    let c = boxes[0].center();
    h.hover_at(c);
    h.step();
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: c, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
            h.step();
        }
    }
    h.step();
    h.step();
    assert!(h.state().morph_editors.iter().any(|e| e.id == order[0] && e.open), "the layer's window did not open");
    // the track window: the key box under the compressor
    h.state_mut().open_track_window(track);
    h.step();
    h.step();
    assert!(crate::widgets::field_probe::rect_of("fx.key.0").is_some(), "the track window draws no key box");
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

/// A key of the table pressed as a person does: down and up with its modifiers.
fn press(h: &mut Harness<'_, KtApp>, chord: crate::controls::Chord) {
    let modifiers = egui::Modifiers { alt: chord.alt, ctrl: chord.ctrl, shift: chord.shift, command: chord.ctrl, mac_cmd: false };
    for pressed in [true, false] {
        h.event(egui::Event::Key { key: chord.key, physical_key: None, pressed, repeat: false, modifiers });
    }
    h.step();
    h.step();
}

/// t3002 (fx-graph v1c): the key table's own commands act on the graph under the pointer — the
/// mute key switches the selected node, Delete removes it, zoom in opens the detailed nodes and zoom
/// to fit returns the fit (in a style that binds zoom keys; the owner's zooms with the wheel) — and
/// the timeline's event under the window stays untouched.
/// - **PATH** in an event's effects window click the Blur node → with the pointer over the graph press
///   the mute key → Blur is struck through → press Delete → Blur goes, the event stays on the timeline
///   → press zoom in → the detailed nodes → press zoom to fit → the fit again
#[test]
fn t3002_the_table_s_keys_act_on_the_graph_under_the_pointer() {
    use crate::controls::Cmd;
    crate::plugins::install();
    let fx: Vec<FxInstance> = (0..10).map(|i| if i == 0 { FxInstance::blur(2) } else { FxInstance::builtin("vignette") }).collect();
    let mut h = fx_harness(fx);
    let blur = node_label(&h, crate::fx_editor::fx_display_name(&FxInstance::blur(2)));
    h.hover_at(blur.center());
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: blur.center(), button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.step();
    h.step();
    let keys = |h: &Harness<'_, KtApp>, c: Cmd| h.state().start.controls.choice.keys_of(c).first().copied().expect("a bound key");
    let mute = keys(&h, Cmd::TrackMute);
    press(&mut h, mute);
    let c = h.state().model.project().clips[0].clone();
    assert_eq!(c.fx_bypass.first().copied(), Some(true), "the mute key did not switch the selected node");
    let del = keys(&h, Cmd::Delete);
    press(&mut h, del);
    let p = h.state().model.project();
    assert_eq!(p.clips.len(), 1, "Delete over the graph deleted the timeline's event");
    assert!(p.clips[0].fx.iter().all(|f| f.tag() != "blur"), "Delete over the graph did not remove the selected node");
    let nodes = || (1..10).filter_map(|i| crate::widgets::field_probe::rect_of(&format!("fx.node.{i}"))).collect::<Vec<_>>();
    assert!(nodes().iter().all(|r| r.height() < 30.0), "the scene starts from the chips");
    // the owner's own style zooms with the wheel and binds no zoom key: the zoom keys are a style's
    h.state_mut().start.controls = crate::controls::Controls::new(crate::controls::Choice::of(crate::controls::Style::Premiere));
    let zin = keys(&h, Cmd::ZoomIn);
    press(&mut h, zin);
    assert!(nodes().iter().any(|r| r.height() > 30.0), "zoom in over the graph did not open the detailed nodes");
    let fit = keys(&h, Cmd::ZoomFit);
    press(&mut h, fit);
    assert!(nodes().iter().all(|r| r.height() < 30.0), "zoom to fit over the graph did not return the fit");
}

/// t3004 (fx-graph D10): an event and its linked audio are ONE graph — VHS Tape put on the picture
/// stands as one device over both events, its sound half on the sound lane under its picture half;
/// the sound half's switch turns the AUDIO event's slot off and leaves the picture's; the picture
/// half's cross takes the device off both events.
/// - **PATH** double click a video event linked to its audio, carrying VHS Tape → the effects window
///   shows one graph: the picture lane with VHS Tape, the sound lane with its sound half under it →
///   click the sound half's checkbox → it is struck through, the picture keeps the tape → click the
///   picture half's cross → the tape goes from both events
#[test]
fn t3004_an_event_and_its_audio_are_one_graph() {
    crate::plugins::install();
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(clip(1, ClipKind::Video, vec![]));
    p.clips.push(clip(2, ClipKind::Audio, vec![FxInstance::plugin("gain").unwrap()]));
    p.validate().unwrap();
    app.model = TimelineModel::new(p);
    assert!(app.apply(Command::SetFx { id: ClipId(1), fx: vec![FxInstance::builtin("vhs_tape")] }));
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    let tape = node_label(&h, "VHS Tape");
    let tape_sound = node_label(&h, &sound("VHS Tape"));
    assert!((tape.left() - tape_sound.left()).abs() < 1.0, "the halves are not one column: {tape:?} {tape_sound:?}");
    assert!(tape_sound.top() > tape.bottom(), "the sound half is not under the picture half");
    // the sound half's own switch: the audio event's slot
    let boxes: Vec<egui::Rect> = h.query_all_by_role(egui::accesskit::Role::CheckBox).map(|n| n.rect()).collect();
    let cb = boxes
        .iter()
        .filter(|r| (r.center().y - tape_sound.center().y).abs() < 8.0 && r.right() <= tape_sound.left() + 1.0)
        .max_by(|a, b| a.right().total_cmp(&b.right()))
        .copied()
        .expect("the sound half's checkbox");
    click_at(&mut h, cb.center());
    let m = &h.state().model;
    let a = m.clip(ClipId(2)).unwrap();
    let ai = a.fx.iter().position(|f| f.tag() == "vhs_tape_sound").expect("the audio holds the sound half");
    assert_eq!(a.fx_bypass.get(ai).copied(), Some(true), "the sound half's switch did not reach the audio event");
    assert!(!m.clip(ClipId(1)).unwrap().fx_bypass.first().copied().unwrap_or(false), "the picture's tape was switched off with it");
    // the picture half's cross takes the device off both events
    let crosses: Vec<egui::Rect> = h.query_all_by_label(crate::icons::CLEAR).map(|n| n.rect()).collect();
    let tape = node_label(&h, "VHS Tape");
    let x = crosses
        .iter()
        .filter(|r| (r.center().y - tape.center().y).abs() < 8.0 && r.left() > tape.right() - 1.0)
        .min_by(|a, b| a.left().total_cmp(&b.left()))
        .copied()
        .expect("the picture half's cross");
    click_at(&mut h, x.center());
    let m = &h.state().model;
    assert!(m.clip(ClipId(1)).unwrap().fx.iter().all(|f| f.tag() != "vhs_tape"), "the tape stayed on the picture");
    assert!(m.clip(ClipId(2)).unwrap().fx.iter().all(|f| f.tag() != "vhs_tape_sound"), "the device's sound outlived it");
    assert!(m.clip(ClipId(2)).unwrap().fx.iter().any(|f| f.tag() == "gain"), "the audio's own effect went too");
}

fn click_at(h: &mut Harness<'_, KtApp>, at: egui::Pos2) {
    h.hover_at(at);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.step();
    h.step();
}

/// t2997 (fx-graph D11): the Viewer — Ctrl+Shift+click on a node gives the preview the project with
/// that event's chain cut after the node (the one composition path draws it), the right-click menu
/// does the same; Esc gives the whole project back and the window stays; the model never changes.
/// - **PATH** in an event's effects window Ctrl+Shift+click the Blur node → the preview shows the
///   picture after Blur with the badge "Viewer: Blur" → press Esc → the whole picture, the window open
#[test]
fn t2997_the_viewer_shows_the_picture_after_a_node() {
    crate::plugins::install();
    let mut h = fx_harness(vec![FxInstance::blur(3), FxInstance::builtin("vignette")]);
    let model_before = h.state().model.project().clone();
    let blur = node_label(&h, crate::fx_editor::fx_display_name(&FxInstance::blur(3)));
    h.hover_at(blur.center());
    h.step();
    let mods = egui::Modifiers { ctrl: true, shift: true, command: true, ..Default::default() };
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: blur.center(), button: egui::PointerButton::Primary, pressed, modifiers: mods });
    }
    h.step();
    h.step();
    let v = h.state().viewer.clone().expect("Ctrl+Shift+click did not turn the Viewer on");
    assert_eq!(v.clip, ClipId(1));
    let tags_in = |p: &Project| p.clips.iter().find(|c| c.id == ClipId(1)).map(|c| c.fx.iter().map(|f| f.tag().to_string()).collect::<Vec<_>>());
    assert_eq!(tags_in(&h.state().player.project()), Some(vec!["blur".to_string()]), "the preview's project is not cut after Blur");
    assert_eq!(h.state().model.project(), &model_before, "the Viewer changed the model");
    // Esc: the whole picture, the window stays
    h.key_press(egui::Key::Escape);
    h.step();
    h.step();
    assert!(h.state().viewer.is_none(), "Esc did not end the Viewer");
    assert_eq!(tags_in(&h.state().player.project()), Some(vec!["blur".to_string(), "vignette".to_string()]), "the whole chain did not come back");
    assert!(h.state().fx_editors.iter().any(|e| e.open), "Esc closed the window instead of ending the Viewer");
}

/// t3235 (track-device-sound, kkt-26.11.18b line 29): VHS Tape on a whole video
/// track — its window draws the track and its sound track as one graph, the tape's halves one
/// column; the sound half's switch reaches the audio track only; "Device sound: None" takes the
/// half off; Cancel restores both tracks.
/// - **PATH** right click Video 1's header → Track properties → the track window opens → add VHS
///   Tape → the graph shows the picture lane and the sound lane with the tape's two halves in one
///   column, and the row "Device sound: Audio 1" appears → click the sound half's switch → it is
///   off, the picture half stays on → pick "None" in the row → the sound lane goes, the row stays
#[test]
fn t3235_a_track_device_and_its_sound_track_are_one_graph() {
    crate::plugins::install();
    let mut h = harness();
    let app = h.state_mut();
    let v1 = app.model.project().track_at(LaneKind::Video, 0).unwrap().id;
    let a1 = app.model.project().track_at(LaneKind::Audio, 0).unwrap().id;
    let before = (app.model.project().track(v1).cloned(), app.model.project().track(a1).cloned());
    app.open_track_window(v1);
    h.step();
    // the tape, as the picker's click sends it: the model links the sound track of the same number
    {
        let app = h.state_mut();
        let g = app.track_win.as_ref().unwrap().gesture;
        app.apply_coalesced(Command::SetTrackFx { id: v1, fx: vec![FxInstance::builtin("vhs_tape")], bypass: vec![] }, g);
    }
    h.step();
    h.step();
    assert_eq!(h.state().model.project().track(v1).unwrap().sound, Some(a1));
    assert!(crate::widgets::field_probe::rect_of("track.sound").is_some(), "the Device sound row is not drawn");
    let tape = node_label(&h, "VHS Tape");
    let tape_sound = node_label(&h, &sound("VHS Tape"));
    assert!((tape.left() - tape_sound.left()).abs() < 1.0, "the halves are not one column: {tape:?} {tape_sound:?}");
    assert!(tape_sound.top() > tape.bottom(), "the sound half is not under the picture half");
    let boxes: Vec<egui::Rect> = h.query_all_by_role(egui::accesskit::Role::CheckBox).map(|n| n.rect()).collect();
    let cb = boxes
        .iter()
        .filter(|r| (r.center().y - tape_sound.center().y).abs() < 8.0 && r.right() <= tape_sound.left() + 1.0)
        .max_by(|a, b| a.right().total_cmp(&b.right()))
        .copied()
        .expect("the sound half's checkbox");
    click_at(&mut h, cb.center());
    let p = h.state().model.project();
    let a = p.track(a1).unwrap();
    let ai = a.fx.iter().position(|f| f.tag() == "vhs_tape_sound").expect("the audio track holds the sound half");
    assert_eq!(a.fx_bypass.get(ai).copied(), Some(true), "the sound half's switch did not reach the audio track");
    assert!(!p.track(v1).unwrap().fx_bypass.first().copied().unwrap_or(false), "the picture half went off with it");
    // "Device sound: None"
    click_at(&mut h, crate::widgets::field_probe::rect_of("track.sound").unwrap().center());
    let none = crate::widgets::field_probe::rect_of("track.sound.none").expect("the None item");
    click_at(&mut h, none.center());
    let p = h.state().model.project();
    assert_eq!(p.track(v1).unwrap().sound, None);
    assert!(p.track(a1).unwrap().fx.is_empty(), "clearing the link left the half");
    assert!(h.query_by_label(&sound("VHS Tape")).is_none(), "the sound lane stayed");
    assert!(crate::widgets::field_probe::rect_of("track.sound").is_some(), "the row went with the link");
    // Cancel restores both tracks
    click_at(&mut h, crate::widgets::field_probe::rect_of("track.cancel").unwrap().center());
    let p = h.state().model.project();
    assert_eq!((p.track(v1).cloned(), p.track(a1).cloned()), before, "Cancel left a track changed");
}

/// t3237 (found-defects 820): Datamosh picked in a video TRACK's window is refused aloud — a stream
/// stage decodes an event's own stream and the engine skips the slot on a track — and the chain
/// stays as it was; another effect from the same picker lands.
/// - **PATH** right click Video 1's header → Track properties → open the graph's picker → type
///   "mosh" → click Datamosh → the status line says Datamosh breaks an event's own stream and a
///   track has none, the graph holds no Datamosh node → pick Blur → it stands on the track
#[test]
fn t3237_datamosh_on_a_track_is_refused_with_its_reason() {
    crate::plugins::install();
    let mut h = harness();
    let v1 = h.state().model.project().track_at(LaneKind::Video, 0).unwrap().id;
    h.state_mut().open_track_window(v1);
    h.step();
    h.step();
    let pick = |h: &mut Harness<'_, KtApp>, query: &str, tag: &str| {
        click_at(h, crate::widgets::field_probe::rect_of("fxreg.add").expect("the graph's picker").center());
        h.event(egui::Event::Text(query.into()));
        h.step();
        h.step();
        let cell = crate::widgets::field_probe::rect_of(&format!("fxreg.tpl.{tag}")).unwrap_or_else(|| panic!("no {tag} in the picker"));
        click_at(h, cell.center());
    };
    pick(&mut h, "mosh", "datamosh");
    let app = h.state();
    assert!(app.model.project().track(v1).unwrap().fx.is_empty(), "Datamosh landed on the track");
    let said = app.error_status.as_ref().map(|(t, _)| t.clone()).unwrap_or_default();
    assert_eq!(said, tr(k::track::NO_STREAM), "the refusal did not name its reason");
    pick(&mut h, "blur", "blur");
    assert_eq!(h.state().model.project().track(v1).unwrap().fx.len(), 1, "an ordinary effect did not land");
}

// ---- fx-graph-dag v2d: branches and merges ------------------------------------------------------

fn at(name: &str) -> egui::Pos2 {
    crate::widgets::field_probe::rect_of(name).unwrap_or_else(|| panic!("nothing published “{name}”")).center()
}

/// The node (its strip index) whose name label is `name`.
fn node_of(h: &Harness<'_, KtApp>, name: &str) -> usize {
    let label = node_label(h, name);
    (0..16)
        .find(|i| crate::widgets::field_probe::rect_of(&format!("fx.node.{i}")).is_some_and(|r| r.expand(1.0).contains_rect(label)))
        .unwrap_or_else(|| panic!("“{name}” stands in no node"))
}

/// A wire let go on the empty space under the graph, then a row of the picker that opens there.
fn branch(h: &mut Harness<'_, KtApp>, from: &str, search: &str, probe: &str) {
    let g = crate::widgets::field_probe::rect_of("fx.graph").expect("the graph");
    let to = egui::pos2(g.center().x, g.bottom() + 12.0);
    drag(h, at(from), to);
    pick_row(h, search, probe);
}

fn pick_row(h: &mut Harness<'_, KtApp>, search: &str, probe: &str) {
    h.step();
    h.event(egui::Event::Text(search.to_string()));
    h.step();
    h.step();
    let c = at(probe);
    h.hover_at(c);
    h.step();
    click_at(h, c);
    h.step();
}

fn event(h: &Harness<'_, KtApp>) -> ClipEvent {
    h.state().model.project().clips.iter().find(|c| c.id == ClipId(1)).unwrap().clone()
}

/// The wiring by names: slot → what its main, second and matte inputs read.
fn wired(h: &Harness<'_, KtApp>) -> Vec<String> {
    use kt_model::fx_graph::{Node, Socket, reads_of};
    let c = event(h);
    let name = |n: Node| match n {
        Node::Source => "source".to_string(),
        Node::Slot(j) => c.fx[j].tag().to_string(),
    };
    (0..c.fx.len())
        .map(|i| {
            let ins: Vec<String> = reads_of(&c.fx_in, i)
                .map(|(n, s)| match s {
                    Socket::Main => name(n),
                    Socket::Second => format!("fg:{}", name(n)),
                    Socket::Matte => format!("matte:{}", name(n)),
                })
                .collect();
            format!("{}<{}", c.fx[i].tag(), ins.join(","))
        })
        .collect()
}

fn said(h: &Harness<'_, KtApp>) -> String {
    h.state().error_status.as_ref().map(|(t, _)| t.clone()).unwrap_or_default()
}

/// t3218 (fx-graph-dag D8): a wire from a node's output let go on empty space opens the picker
/// there; the picked effect is a new arm (a row under the trunk, dim until it reaches Out); Merge is
/// the picker's first row and takes the Output as its Background; a wire onto a Merge's Matte socket
/// connects it; Slowed picked on the selected arm stands in the arm and is marked as a second read.
/// Each gesture is one undo step, and "Cancel" of the window gives the line back.
/// - **PATH** double click an event carrying a Blur → its node editor shows In → Geometry → Blur →
///   Out → drag a wire from Geometry's output to the empty space below → the picker opens there →
///   pick Vignette → a second row appears under the trunk with Vignette wired from Geometry → drag
///   from Blur's output to the empty space → pick Merge → Merge stands before Out with Blur in
///   Background → drag a wire from Vignette's output onto Merge's Matte socket → it connects →
///   click Vignette and pick Slowed+Reverb → it stands after Vignette in the arm, marked "Reads the
///   source again"
#[test]
fn t3218_a_branch_a_merge_and_a_matte_by_gestures() {
    crate::plugins::install();
    let mut h = fx_harness(vec![FxInstance::blur(3)]);
    let line = event(&h);
    let undo0 = h.state().model.undo_depth();
    let lead = node_of(&h, crate::fx_editor::fx_display_name(&FxInstance::geometry()));
    branch(&mut h, &format!("fx.out.{lead}"), "vignette", "fxreg.tpl.vignette");
    assert_eq!(wired(&h), ["vignette<source", "blur<source"], "the arm did not start at the Geometry's output");
    assert_eq!(h.state().model.undo_depth(), undo0 + 1, "the branch is not one undo step");
    let (v, b) = (node_of(&h, "Vignette"), node_of(&h, "Blur"));
    let r = |i: usize| crate::widgets::field_probe::rect_of(&format!("fx.node.{i}")).unwrap();
    assert!(r(v).top() > r(b).bottom() - 1.0, "the arm is not a row under the trunk: {:?} {:?}", r(v), r(b));

    branch(&mut h, &format!("fx.out.{b}"), "merge", "fxreg.merge");
    assert_eq!(wired(&h), ["vignette<source", "blur<source", "merge<blur"], "Merge does not stand before Out on the blur");
    assert_eq!(h.state().model.undo_depth(), undo0 + 2);

    let (v, m) = (node_of(&h, "Vignette"), node_of(&h, "Merge"));
    drag(&mut h, at(&format!("fx.out.{v}")), at(&format!("fx.matte.{m}")));
    assert_eq!(wired(&h)[2], "merge<blur,matte:vignette", "the wire did not reach the Matte socket");
    assert_eq!(h.state().model.undo_depth(), undo0 + 3);

    // the picker's click adds after the selected node of a chain that branches
    let label = node_label(&h, "Vignette").center();
    h.hover_at(label);
    h.step();
    click_at(&mut h, label);
    let add = at("fxreg.add");
    h.hover_at(add);
    h.step();
    click_at(&mut h, add);
    pick_row(&mut h, "slowed", "fxreg.tpl.slowed_reverb");
    assert_eq!(
        wired(&h),
        ["vignette<source", "blur<source", "slowed_reverb<vignette", "merge<blur,matte:slowed_reverb"],
        "Slowed did not land in the arm"
    );
    assert_eq!(h.state().model.undo_depth(), undo0 + 4);
    let s = node_of(&h, "Slowed+Reverb");
    assert!(crate::widgets::field_probe::rect_of(&format!("fx.read.{s}")).is_some(), "the second read is not marked");
    let c = event(&h);
    assert_eq!(kt_model::fx_graph::read_count(&c.fx, &c.fx_in), 2);

    // four steps back: the line the window opened with, and forward again
    for _ in 0..4 {
        h.state_mut().undo();
    }
    h.step();
    assert_eq!(event(&h), line, "four undos did not give the line back");
    for _ in 0..4 {
        h.state_mut().redo();
    }
    h.step();
    h.step();
    // the window's Cancel takes its whole session back, wires with slots
    let cancel = at("fx.cancel");
    h.hover_at(cancel);
    h.step();
    click_at(&mut h, cancel);
    h.step();
    assert_eq!(event(&h), line, "Cancel left the branches");
}

/// t3219 (fx-graph-dag D8): a wire dragged off a Merge's Foreground disconnects it and the arm, kept,
/// is drawn dim; a node dropped on a wire of an arm stands on that wire; the cross on a slot of a
/// chain that branches keeps the wiring (found-defects 903); the key of the table does the same.
/// - **PATH** an event with Blur on the trunk, Vignette in an arm and a Merge → drag the wire off
///   Merge's Foreground socket to empty space → Vignette is dim, the picture is the trunk's → drag
///   from Vignette's output back onto the Foreground socket → drag Glow from the trunk onto the
///   arm's wire → Glow stands in the arm → click the cross of Merge → Blur is the Output, the arm stays
#[test]
fn t3219_disconnect_insert_on_a_wire_and_remove() {
    use kt_model::fx_graph::{FxIn, SlotInputs};
    crate::plugins::install();
    let mut h = fx_harness(vec![FxInstance::blur(3)]);
    let merge = FxInstance::Merge { mode: MergeMode::Blend(BlendMode::Screen), mix: 1.0 };
    // vignette(0, source) glow(1, source) blur(2) merge(3: blur, fg vignette)
    let w = vec![
        SlotInputs::default(),
        SlotInputs { main: FxIn::Source, ..Default::default() },
        SlotInputs::default(),
        SlotInputs { main: FxIn::Prev, second: Some(FxIn::Slot(0)), matte: None },
    ];
    let fx = vec![FxInstance::builtin("vignette"), FxInstance::builtin("glow"), FxInstance::blur(3), merge];
    assert!(h.state_mut().apply_batch(vec![
        Command::SetFx { id: ClipId(1), fx },
        Command::SetFxIn { id: ClipId(1), fx_in: w },
    ]));
    h.step();
    h.step();
    assert_eq!(wired(&h), ["vignette<source", "glow<source", "blur<glow", "merge<blur,fg:vignette"]);
    let g = crate::widgets::field_probe::rect_of("fx.graph").unwrap();
    let undo0 = h.state().model.undo_depth();
    let m = node_of(&h, "Merge");
    drag(&mut h, at(&format!("fx.fg.{m}")), egui::pos2(g.center().x, g.bottom() + 12.0));
    assert_eq!(wired(&h)[3], "merge<blur", "the wire did not come off");
    assert_eq!(event(&h).fx.len(), 4, "the arm went with its wire");
    assert_eq!(h.state().model.undo_depth(), undo0 + 1);
    let v = node_of(&h, "Vignette");
    drag(&mut h, at(&format!("fx.out.{v}")), at(&format!("fx.fg.{m}")));
    assert_eq!(wired(&h)[3], "merge<blur,fg:vignette");
    // Glow onto the wire from the arm into the Merge
    let (glow, wire) = (node_label(&h, "Glow").center(), at(&format!("fx.wire.{m}.fg")));
    drag(&mut h, glow, wire);
    assert_eq!(wired(&h), ["vignette<source", "glow<vignette", "blur<source", "merge<blur,fg:glow"], "Glow is not on the arm's wire");
    // Delete over the graph: the selected node of a chain that branches goes with its wires kept
    let label = node_label(&h, "Glow").center();
    h.hover_at(label);
    h.step();
    click_at(&mut h, label);
    h.key_press(egui::Key::Delete);
    h.step();
    h.step();
    assert_eq!(wired(&h), ["vignette<source", "blur<source", "merge<blur,fg:vignette"], "Delete broke the wiring");
    // the Merge's cross: its Background is the Output again, the arm stays and dangles
    let m = node_of(&h, "Merge");
    let node = crate::widgets::field_probe::rect_of(&format!("fx.node.{m}")).unwrap();
    let x = h.query_all_by_label(crate::icons::CLEAR).map(|n| n.rect()).find(|r| node.contains_rect(*r)).expect("Merge's cross");
    h.hover_at(x.center());
    h.step();
    click_at(&mut h, x.center());
    h.step();
    assert_eq!(wired(&h), ["vignette<source", "blur<source"]);
    let c = event(&h);
    assert_eq!(kt_model::fx_graph::reaches_output(&c.fx_in, 2), [false, true], "the blur is not the Output");
}

/// t3220 (fx-graph-dag D8, rule 16): what the graph cannot do is refused in the status line and
/// changes nothing — the leading Geometry into an arm, a wire that closes a loop, a wire across
/// lanes, a fifth read, a picture's Merge mode never offered on the sound lane.
/// - **PATH** in a graph with an arm drag the Geometry node onto the arm's wire → the status line
///   says it stays before the first branch → drag a wire from Merge onto Blur's input → "would close
///   a loop" → in a morpher drag a wire from a picture node onto a sound node → refused → add a Merge
///   on the sound lane → its mode list is Sum and Crossfade
#[test]
fn t3220_refusals_name_their_reason_and_change_nothing() {
    use kt_model::fx_graph::{FxIn, SlotInputs};
    crate::plugins::install();
    let mut h = fx_harness(vec![FxInstance::blur(3)]);
    let mosh = FxInstance::plugin("datamosh").unwrap();
    let merge = || FxInstance::Merge { mode: MergeMode::default(), mix: 1.0 };
    let src = || SlotInputs { main: FxIn::Source, ..Default::default() };
    let onto = |main: u32| SlotInputs { main: FxIn::Slot(main), second: Some(FxIn::Prev), matte: None };
    // three Datamosh arms merged one by one over a blur: four reads
    let fx = vec![FxInstance::blur(3), mosh.clone(), merge(), mosh.clone(), merge(), mosh, merge()];
    let w = vec![SlotInputs::default(), src(), onto(0), src(), onto(2), src(), onto(4)];
    assert!(h.state_mut().apply_batch(vec![Command::SetFx { id: ClipId(1), fx }, Command::SetFxIn { id: ClipId(1), fx_in: w }]));
    h.step();
    h.step();
    let c = event(&h);
    assert_eq!(kt_model::fx_graph::read_count(&c.fx, &c.fx_in), kt_model::fx_graph::MAX_READS, "scene");
    let lead = node_of(&h, crate::fx_editor::fx_display_name(&FxInstance::geometry()));
    // a fifth read
    branch(&mut h, &format!("fx.out.{lead}"), "slowed", "fxreg.tpl.slowed_reverb");
    // dangling it opens nothing: the read is refused where it would reach Out
    let dangling = event(&h);
    let s = node_of(&h, "Slowed+Reverb");
    let last = (0..16).filter(|i| crate::widgets::field_probe::rect_of(&format!("fx.matte.{i}")).is_some()).max().unwrap();
    drag(&mut h, at(&format!("fx.out.{s}")), at(&format!("fx.matte.{last}")));
    assert_eq!(event(&h), dangling, "a fifth read was made");
    assert_eq!(said(&h), crate::tf!(k::fx::graph::WHY_READS, kt_model::fx_graph::MAX_READS));
    // a loop: the Output into the first blur
    let b = node_of(&h, "Blur");
    drag(&mut h, at(&format!("fx.out.{last}")), at(&format!("fx.in.{b}")));
    assert_eq!(event(&h), dangling, "a loop was made");
    assert_eq!(said(&h), tr(k::fx::graph::WHY_LOOP));
    // the lead into an arm: onto Slowed's wire
    let geo = crate::widgets::field_probe::rect_of(&format!("fx.slot.{lead}")).unwrap_or_else(|| node_label(&h, crate::fx_editor::fx_display_name(&FxInstance::geometry())));
    let to = node_label(&h, "Slowed+Reverb").center();
    drag(&mut h, geo.center(), to);
    assert_eq!(event(&h), dangling, "the lead moved into an arm");
    assert_eq!(said(&h), tr(k::fx::graph::WHY_LEAD));

    // across lanes, and the sound lane's Merge
    let mut h = morpher_harness(MorphPatch {
        video_fx: vec![FxInstance::blur(4)],
        audio_fx: vec![FxInstance::plugin("gain").unwrap()],
        ..Default::default()
    });
    let before = patch(&h);
    let (b, g) = (node_of(&h, "Blur"), node_of(&h, crate::fx_editor::fx_display_name(&FxInstance::plugin("gain").unwrap())));
    drag(&mut h, at(&format!("fx.out.{b}")), at(&format!("fx.in.{g}")));
    assert_eq!(patch(&h), before, "a wire crossed the lanes");
    assert_eq!(said(&h), tr(k::fx::graph::LANE_REFUSED));
    let gr = crate::widgets::field_probe::rect_of("fx.graph").unwrap();
    drag(&mut h, at(&format!("fx.out.{g}")), egui::pos2(gr.center().x, gr.bottom() + 12.0));
    pick_row(&mut h, "merge", "fxreg.merge");
    let p = patch(&h);
    assert_eq!(tags(&p.audio_fx), ["gain", "merge"], "the Merge is not on the sound lane");
    assert!(matches!(p.audio_fx[1], FxInstance::Merge { mode: MergeMode::Sum, .. }), "{:?}", p.audio_fx[1]);
    assert_eq!(tags(&p.video_fx), ["blur"]);
}

/// t3207 (fx-graph-dag D3): the Viewer on a node of an arm gives the player the node and what it is
/// made from — not the slots that stand before it in the list; Esc gives the whole graph back and
/// the model never changes.
/// - **PATH** an event with Glow on the trunk and Vignette in an arm, merged → Ctrl+Shift+click
///   Vignette → the preview shows the vignette alone with the badge "Viewer: Vignette" → press Esc →
///   the full picture
#[test]
fn t3207_the_viewer_shows_a_node_of_an_arm_by_its_ancestors() {
    use kt_model::fx_graph::{FxIn, SlotInputs};
    crate::plugins::install();
    let mut h = fx_harness(vec![FxInstance::blur(3)]);
    let fx = vec![FxInstance::builtin("glow"), FxInstance::builtin("vignette"), FxInstance::Merge { mode: MergeMode::default(), mix: 1.0 }];
    let w = vec![
        SlotInputs::default(),
        SlotInputs { main: FxIn::Source, ..Default::default() },
        SlotInputs { main: FxIn::Slot(0), second: Some(FxIn::Prev), matte: None },
    ];
    assert!(h.state_mut().apply_batch(vec![Command::SetFx { id: ClipId(1), fx }, Command::SetFxIn { id: ClipId(1), fx_in: w }]));
    h.step();
    h.step();
    let before = h.state().model.project().clone();
    let label = node_label(&h, "Vignette");
    h.hover_at(label.center());
    h.step();
    let mods = egui::Modifiers { ctrl: true, shift: true, command: true, ..Default::default() };
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: label.center(), button: egui::PointerButton::Primary, pressed, modifiers: mods });
    }
    h.step();
    h.step();
    assert_eq!(h.state().viewer.as_ref().map(|v| v.name.clone()), Some("Vignette".to_string()));
    let played = |h: &Harness<'_, KtApp>| {
        let p = h.state().player.project();
        let c = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap().clone();
        (c.fx.iter().map(|f| f.tag().to_string()).collect::<Vec<_>>(), c.fx_in.clone())
    };
    assert_eq!(played(&h), (vec!["vignette".to_string()], vec![]), "the player did not get the arm alone");
    assert_eq!(h.state().model.project(), &before, "the Viewer changed the model");
    h.key_press(egui::Key::Escape);
    h.step();
    h.step();
    assert_eq!(played(&h).0.len(), 3, "Esc did not give the graph back");
}

/// t3299 (fx-graph-dag D2, D3): the coverage strip and the picker's cells know the graph — an event
/// with Slowed in an arm has a second row over its own line whose ticks are what that read has warm,
/// and a candidate cell stands after the selected node of an arm, a branch's on the arm alone.
/// - **PATH** put Slowed in an arm of an event → a second thin row stands over the event's coverage
///   line on the timeline → open Diagnostics → the performance section says "Reads ×1: #1
///   Slowed+Reverb"
#[test]
fn t3299_a_read_has_its_row_and_a_cell_its_place() {
    use kt_model::fx_graph::{FxIn, Node, SlotInputs};
    crate::plugins::install();
    let mut h = fx_harness(vec![FxInstance::blur(3)]);
    let fx = vec![FxInstance::slowed_reverb(0.5, 0.35, 100.0), FxInstance::blur(3), FxInstance::Merge { mode: MergeMode::default(), mix: 1.0 }];
    let w = vec![
        SlotInputs::default(),
        SlotInputs { main: FxIn::Source, ..Default::default() },
        SlotInputs { main: FxIn::Prev, second: Some(FxIn::Slot(0)), matte: None },
    ];
    assert!(h.state_mut().apply_batch(vec![Command::SetFx { id: ClipId(1), fx }, Command::SetFxIn { id: ClipId(1), fx_in: w }]));
    let wait = kt_time::test_deadline();
    while h.state().player.video_read_rows().is_empty() {
        assert!(std::time::Instant::now() < wait, "the engine never listed the read");
        h.step();
    }
    let row = h.state().player.video_read_rows().remove(0);
    assert_eq!((row.clip, row.opens.tag()), (ClipId(1), "slowed_reverb"));
    // half the speed walks half the material over the same window
    assert_eq!((row.end - row.start).0, 180_000);
    assert_eq!(row.material.1 - row.material.0, 90_000);
    while crate::widgets::field_probe::rect_of("clip.1.read.0").is_none() {
        assert!(std::time::Instant::now() < wait, "no row of the read over the event's line");
        h.step();
    }

    // the cells: after the selected node of the arm; a branch — the arm alone
    let p = h.state().model.project().clone();
    let cand = [FxInstance::builtin("glow")];
    let chain = |place| {
        let q = crate::fx_thumbs::with_candidate(&p, place, &cand, None).expect("a scene");
        let c = q.clips.iter().find(|c| c.id == ClipId(1)).unwrap().clone();
        q.validate().expect("the cell's project is valid");
        (c.fx.iter().map(|f| f.tag().to_string()).collect::<Vec<_>>(), c.fx_in)
    };
    use crate::fx_thumbs::Place;
    // the strip: Geometry 0, Slowed 1, Blur 2, Merge 3
    let (tags, w) = chain(Place::After(ClipId(1), 1));
    assert_eq!(tags, ["slowed_reverb", "blur", "glow", "merge"]);
    assert_eq!((w[2].main, w[3].main, w[3].second), (FxIn::Slot(0), FxIn::Slot(1), Some(FxIn::Prev)), "the candidate is not on the arm into the Merge");
    assert_eq!(chain(Place::Arm(ClipId(1), Node::Slot(2))), (vec!["blur".into(), "glow".into()], vec![]));
    assert_eq!(chain(Place::Clip(ClipId(1))).0, ["slowed_reverb", "blur", "merge", "glow"], "the tail is not the tail");
}

/// t3240 (fx-graph-dag D6, D8): branches where a chain is not one event's line — the sound lane of a
/// pair's one editor (the video event's wiring untouched), a device's halves leaving a pair whose
/// sound branches, a track's chain with the window's Cancel, and a copied graph pasted after itself.
/// - **PATH** double click a linked event carrying Blur, VHS Tape and a Gain on its sound → drag a
///   wire from Gain's output to empty space → pick Compressor → Merge it → only the sound lane has a
///   second row → click the cross of VHS Tape → both halves go, the sound's branch stays → Copy the
///   chain, Paste → the branches come twice → open the video track's window, branch from In → Cancel
///   → the track is as it was
#[test]
fn t3240_branches_on_a_pair_a_device_a_track_and_a_paste() {
    crate::plugins::install();
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(clip(1, ClipKind::Video, vec![FxInstance::blur(3)]));
    p.clips.push(clip(2, ClipKind::Audio, vec![FxInstance::plugin("gain").unwrap()]));
    p.validate().unwrap();
    app.model = TimelineModel::new(p);
    assert!(app.apply(Command::SetFx { id: ClipId(1), fx: vec![FxInstance::blur(3), FxInstance::builtin("vhs_tape")] }));
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    let of = |h: &Harness<'_, KtApp>, id: u64| h.state().model.clip(ClipId(id)).unwrap().clone();
    assert_eq!(tags(&of(&h, 2).fx), ["gain", "vhs_tape_sound"], "scene");
    let gain = node_of(&h, crate::fx_editor::fx_display_name(&FxInstance::plugin("gain").unwrap()));
    branch(&mut h, &format!("fx.out.{gain}"), "compressor", "fxreg.tpl.compressor");
    let comp = node_of(&h, crate::fx_editor::fx_display_name(&FxInstance::plugin("compressor").unwrap()));
    branch(&mut h, &format!("fx.out.{comp}"), "merge", "fxreg.merge");
    let a = of(&h, 2);
    assert_eq!(tags(&a.fx), ["gain", "compressor", "vhs_tape_sound", "merge"], "{:?}", a.fx_in);
    assert!(!a.fx_in.is_empty() && of(&h, 1).fx_in.is_empty(), "the wiring is not the sound lane's alone");
    assert_eq!(kt_model::fx_graph::reaches_output(&a.fx_in, 4), [true; 4]);
    // the device leaves both lanes by one cross: the sound's branch stays and the project is valid
    let tape = node_label(&h, "VHS Tape");
    let x = h
        .query_all_by_label(crate::icons::CLEAR)
        .map(|n| n.rect())
        .filter(|r| (r.center().y - tape.center().y).abs() < 8.0 && r.left() > tape.right() - 1.0)
        .min_by(|a, b| a.left().total_cmp(&b.left()))
        .expect("VHS Tape's cross");
    h.hover_at(x.center());
    h.step();
    click_at(&mut h, x.center());
    h.step();
    let a = of(&h, 2);
    assert_eq!((tags(&of(&h, 1).fx), tags(&a.fx)), (vec!["blur"], vec!["gain", "compressor", "merge"]));
    assert_eq!(kt_model::fx_graph::reaches_output(&a.fx_in, 3), [true; 3], "{:?}", a.fx_in);
    assert!(said(&h).is_empty(), "an edit was refused: {}", said(&h));
    h.state().model.project().validate().unwrap();

    // a copied graph pasted after itself keeps its branches
    let mut h = fx_harness(vec![FxInstance::blur(3)]);
    let lead = node_of(&h, crate::fx_editor::fx_display_name(&FxInstance::geometry()));
    branch(&mut h, &format!("fx.out.{lead}"), "vignette", "fxreg.tpl.vignette");
    let v = node_of(&h, "Vignette");
    branch(&mut h, &format!("fx.out.{v}"), "merge", "fxreg.merge");
    assert_eq!(wired(&h), ["vignette<source", "blur<source", "merge<blur,fg:vignette"]);
    let c = event(&h);
    let block = kt_model::ops::FxChainBlock {
        slots: c.fx.iter().map(|f| kt_model::ops::FxSlotBlock { fx: f.clone(), bypass: false, curves: vec![], masks: vec![] }).collect(),
        src_in: kt_time::Tick(0),
        fx_off: false,
        fx_in: c.fx_in.clone(),
    };
    h.state_mut().fx_clipboard = Some(block);
    // nothing selected in the lane's middle: the paste goes after the Output
    let m = node_label(&h, "Merge").center();
    h.hover_at(m);
    h.step();
    click_at(&mut h, m);
    h.step();
    let paste = h.get_by_label(tr(k::fx::PASTE_CHAIN)).rect().center();
    h.hover_at(paste);
    h.step();
    click_at(&mut h, paste);
    h.step();
    assert_eq!(
        wired(&h)[3..],
        ["vignette<merge", "blur<merge", "merge<blur,fg:vignette"],
        "the pasted graph is not its branches over the Output"
    );

    // a video track's chain: In has no Geometry after it, a branch starts right there
    let mut h = harness();
    let v1 = h.state().model.project().track_at(LaneKind::Video, 0).unwrap().id;
    assert!(h.state_mut().apply(Command::SetTrackFx { id: v1, fx: vec![FxInstance::blur(3)], bypass: vec![] }));
    let before = h.state().model.project().track(v1).cloned();
    h.state_mut().open_track_window(v1);
    h.step();
    h.step();
    branch(&mut h, "fx.src.0", "vignette", "fxreg.tpl.vignette");
    let t = h.state().model.project().track(v1).unwrap().clone();
    assert_eq!((tags(&t.fx), t.fx_in.len()), (vec!["vignette", "blur"], 2), "the track has no branch");
    click_at(&mut h, crate::widgets::field_probe::rect_of("track.cancel").unwrap().center());
    h.step();
    assert_eq!(h.state().model.project().track(v1).cloned(), before, "Cancel left the track's branch");
}

/// t3245 (found-defects 906): in the effects window a node dragged to another place takes its addresses
/// along — another event's link follows the effect it was led by, the curve view stays with its slot —
/// a branch started in front of it moves them on, a cross frees the follower, and Cancel gives all of it
/// back; undo does too.
/// - **PATH** an event with Blur, Glow, Vignette and another event whose blur is led by the Glow's
///   strength → in the first event's effects window drag Glow onto Blur → the other event still follows
///   the Glow → start a branch from Geometry → still the Glow → Cancel → the chain and the link as they were
#[test]
fn t3245_addresses_follow_a_node_and_cancel_gives_them_back() {
    crate::plugins::install();
    let mut h = fx_harness(vec![FxInstance::blur(3), FxInstance::builtin("glow"), FxInstance::builtin("vignette")]);
    let mut p = h.state().model.project().clone();
    let mut b = clip(2, ClipKind::Video, vec![FxInstance::blur(4)]);
    (b.link_group, b.lane) = (None, 1);
    let led = |path: &str| vec![vec![ParamCurve { param: "radius".into(), keys: vec![], source: CurveSource::Link { path: path.into(), expr: None, stops: vec![] } }]];
    b.fx_auto = led("1/fx1/strength");
    p.clips.push(b);
    p.ensure_tracks(LaneKind::Video, 2);
    p.validate().unwrap();
    h.state_mut().model = TimelineModel::new(p);
    h.state_mut().fx_editors.clear();
    h.state_mut().open_fx_editor(ClipId(1));
    h.step();
    h.step();
    let path = |h: &Harness<'_, KtApp>| {
        let c = h.state().model.clip(ClipId(2)).unwrap();
        c.fx_auto.first().and_then(|l| l.first()).and_then(|c| c.source.link_path()).map(str::to_string)
    };
    let names = |h: &Harness<'_, KtApp>| event(h).fx.iter().map(|f| f.tag().to_string()).collect::<Vec<_>>();
    let start = (event(&h), path(&h));
    let (from, to) = (node_label(&h, "Glow").center(), node_label(&h, "Blur").center());
    drag(&mut h, from, to);
    assert_eq!(names(&h), ["glow", "blur", "vignette"], "scene");
    assert_eq!(path(&h).as_deref(), Some("1/fx0/strength"), "the follower lost its Glow");
    // a branch in front: the arm's slot stands before them
    let lead = node_of(&h, crate::fx_editor::fx_display_name(&FxInstance::geometry()));
    branch(&mut h, &format!("fx.out.{lead}"), "posterize", "fxreg.tpl.posterize");
    let moved = names(&h).iter().position(|n| n == "glow").unwrap();
    assert_eq!(path(&h), Some(format!("1/fx{moved}/strength")), "{:?}", names(&h));
    h.state_mut().undo();
    h.step();
    h.step();
    assert_eq!(path(&h).as_deref(), Some("1/fx0/strength"), "undo left the address");
    // Cancel: the window's whole session back
    let cancel = at("fx.cancel");
    h.hover_at(cancel);
    h.step();
    click_at(&mut h, cancel);
    h.step();
    assert_eq!((event(&h), path(&h)), start, "Cancel left the slots or their addresses");
    // a cross: the leader is gone, its follower is free
    h.state_mut().open_fx_editor(ClipId(1));
    h.step();
    h.step();
    let glow = node_of(&h, "Glow");
    let node = crate::widgets::field_probe::rect_of(&format!("fx.node.{glow}")).unwrap();
    let x = h.query_all_by_label(crate::icons::CLEAR).map(|n| n.rect()).find(|r| node.contains(r.center())).expect("Glow's cross");
    h.hover_at(x.center());
    h.step();
    click_at(&mut h, x.center());
    h.step();
    assert_eq!(names(&h), ["blur", "vignette"], "scene");
    assert_eq!(path(&h), None, "the follower follows the Vignette that took the Glow's number");
}

