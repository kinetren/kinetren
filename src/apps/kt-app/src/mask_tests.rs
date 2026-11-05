//! shape-masks (kkt-26.11.01 S1): the masks are drawn and edited where a person acts — on the
//! preview picture, from the preview's menu, the Geometry node, an effect slot and the track
//! window. A file of its own: the change's hunks stay out of the shared test files.

use crate::app::KtApp;
use crate::maskctl::{MaskOwner, MaskTool};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use kt_model::{Command, Mask, MaskShape, Vertex};

fn press(h: &mut Harness<'_, KtApp>, pos: egui::Pos2, pressed: bool, mods: egui::Modifiers) {
    h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: mods });
}

fn click_at(h: &mut Harness<'_, KtApp>, pos: egui::Pos2, mods: egui::Modifiers) {
    h.event(egui::Event::ModifiersChanged(mods));
    h.hover_at(pos);
    h.step();
    press(h, pos, true, mods);
    h.step();
    press(h, pos, false, mods);
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

fn drag(h: &mut Harness<'_, KtApp>, from: egui::Pos2, to: egui::Pos2, mods: egui::Modifiers) {
    h.event(egui::Event::ModifiersChanged(mods));
    h.hover_at(from);
    h.step();
    press(h, from, true, mods);
    h.step();
    h.hover_at(from + (to - from) * 0.25);
    h.step();
    h.hover_at(to);
    h.step();
    press(h, to, false, mods);
    h.event(egui::Event::PointerGone);
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

fn key(h: &mut Harness<'_, KtApp>, k: egui::Key) {
    h.event(egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.step();
    h.event(egui::Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::NONE });
    h.step();
}

/// The screen point of a normalized point of the event's source frame.
fn at(h: &Harness<'_, KtApp>, owner: MaskOwner, p: (f64, f64)) -> egui::Pos2 {
    h.state().mask_point_on_screen(owner, p).expect("the owner is on screen")
}

/// Right click on the picture → Mask → `item`.
fn preview_menu(h: &mut Harness<'_, KtApp>, item: &str) {
    let pos = h.state().preview_img_rect.center();
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    let menu = crate::i18n::tr(crate::i18n::k::masks::MENU);
    h.get_all_by_label_contains(menu).next().expect("the Mask item in the preview menu").click();
    h.step();
    h.step();
    h.get_by_label(item).click();
    h.step();
    h.step();
}

fn picture(h: &Harness<'_, KtApp>, clip: kt_model::ClipId) -> Vec<Mask> {
    h.state().model.clip(clip).expect("event").masks.picture.clone()
}

/// Test 1276: Rectangle from the preview menu, drawn by one drag — one history step.
#[test]
fn t1276_a_rectangle_drawn_from_the_preview_menu_is_one_step() {
    if crate::layout_tests::no_device() {
        return;
    }
    let (mut h, clip) = crate::layout_tests::harness_geom_ctl();
    let owner = MaskOwner::Picture(clip);
    preview_menu(&mut h, crate::i18n::tr(crate::i18n::k::masks::RECT));
    assert_eq!(h.state().mask_ctl.as_ref().map(|e| e.tool), Some(MaskTool::Rect), "the menu did not arm the tool");
    let depth = h.state().model.undo_depth();
    let (a, b) = (at(&h, owner, (0.3, 0.3)), at(&h, owner, (0.6, 0.7)));
    drag(&mut h, a, b, egui::Modifiers::NONE);
    let ms = picture(&h, clip);
    assert_eq!(ms.len(), 1, "the drag drew no mask");
    let MaskShape::Rect { cx, cy, w, h: hh, .. } = ms[0].shape else { panic!("not a rectangle: {:?}", ms[0].shape) };
    for (got, want, what) in [(cx, 0.45, "cx"), (cy, 0.5, "cy"), (w, 0.3, "w"), (hh, 0.4, "h")] {
        assert!((got - want).abs() < 0.03, "{what}: {got} vs {want} — the box is not where it was dragged");
    }
    assert_eq!(h.state().model.undo_depth(), depth + 1, "the drawing is one history step");
    assert_eq!(h.state().mask_ctl.as_ref().map(|e| e.tool), Some(MaskTool::Edit), "after the drag the mask is edited");
    // the mask's handles, not the frame's
    h.state_mut().geom_ctl_drawn = false;
    h.state_mut().mask_ctl_drawn = false;
    h.step();
    assert!(h.state().mask_ctl_drawn && !h.state().geom_ctl_drawn, "two sets of handles over one picture");
    h.state_mut().undo();
    h.step();
    assert!(picture(&h, clip).is_empty(), "undo did not remove the mask in one step");
}

/// Test 1277: the pen draws a Bézier path; Esc drops an unfinished one, a second Esc leaves.
#[test]
fn t1277_the_pen_draws_a_bezier_path() {
    if crate::layout_tests::no_device() {
        return;
    }
    let (mut h, clip) = crate::layout_tests::harness_geom_ctl();
    let owner = MaskOwner::Picture(clip);
    preview_menu(&mut h, crate::i18n::tr(crate::i18n::k::masks::PEN));
    let n = egui::Modifiers::NONE;
    for p in [(0.3, 0.3), (0.7, 0.3), (0.7, 0.7)] {
        let s = at(&h, owner, p);
        click_at(&mut h, s, n);
    }
    let (s, e) = (at(&h, owner, (0.3, 0.7)), at(&h, owner, (0.25, 0.6)));
    drag(&mut h, s, e, n);
    assert_eq!(h.state().mask_ctl.as_ref().map(|e| e.pen.len()), Some(4), "four pen points");
    let first = at(&h, owner, (0.3, 0.3));
    click_at(&mut h, first, n);
    let ms = picture(&h, clip);
    assert_eq!(ms.len(), 1, "clicking the first point did not close the path");
    let MaskShape::Path { verts } = &ms[0].shape else { panic!("not a path") };
    assert_eq!(verts.len(), 4);
    assert!(verts[..3].iter().all(Vertex::is_corner), "clicked points are corners");
    let v = verts[3];
    assert!(v.ox.abs() > 0.01 && (v.ix + v.ox).abs() < 1e-9 && (v.iy + v.oy).abs() < 1e-9, "the dragged point is smooth, its handles mirrored: {v:?}");

    // an unfinished path: Esc drops it, nothing is created; a second Esc leaves the masks
    h.state_mut().arm_mask_tool(owner, MaskTool::Pen);
    for p in [(0.4, 0.4), (0.5, 0.45)] {
        let s = at(&h, owner, p);
        click_at(&mut h, s, n);
    }
    key(&mut h, egui::Key::Escape);
    assert_eq!(h.state().mask_ctl.as_ref().map(|e| e.pen.len()), Some(0), "Esc did not drop the unfinished path");
    assert_eq!(picture(&h, clip).len(), 1, "an unfinished path became a mask");
    key(&mut h, egui::Key::Escape);
    key(&mut h, egui::Key::Escape);
    assert!(h.state().mask_ctl.is_none(), "Esc did not leave mask editing");
}

fn with_path(h: &mut Harness<'_, KtApp>, clip: kt_model::ClipId) -> Mask {
    let m = Mask::new(
        1,
        "Mask 1",
        MaskShape::Path {
            verts: vec![Vertex::corner(0, 0.3, 0.3), Vertex::corner(1, 0.7, 0.3), Vertex::corner(2, 0.5, 0.7)],
        },
    );
    let mut em = h.state().model.clip(clip).unwrap().masks.clone();
    em.picture = vec![m.clone()];
    h.state_mut().model.apply(Command::SetMasks { id: clip, masks: em }).unwrap();
    h.state_mut().edit_mask(MaskOwner::Picture(clip), Some(1));
    h.step();
    m
}

/// Test 1278: dragging a point of an animated path keys the whole path; no key without a gesture.
#[test]
fn t1278_a_point_drag_on_an_animated_path_keys_the_whole_path() {
    if crate::layout_tests::no_device() {
        return;
    }
    let (mut h, clip) = crate::layout_tests::harness_geom_ctl();
    let owner = MaskOwner::Picture(clip);
    with_path(&mut h, clip);
    // idle frames over a static mask write nothing
    let depth = h.state().model.undo_depth();
    for _ in 0..10 {
        h.step();
    }
    assert_eq!(h.state().model.undo_depth(), depth, "drawing the mask wrote into the model");
    // the stopwatch of the row, in the Geometry node's Masks group
    h.state_mut().open_fx_editor(clip);
    if let Some(ed) = h.state_mut().fx_editors.first_mut() {
        ed.selected.insert(clip, crate::widgets::ChainSel::Lead);
    }
    h.step();
    h.step();
    h.get_by_label(crate::icons::TIMER).click();
    h.step();
    h.step();
    let m = picture(&h, clip).remove(0);
    assert!(m.path_animated(), "the stopwatch did not key the path");
    let t0 = m.path_key_ticks();
    assert_eq!(t0.len(), 1);
    // close the window (its session is its own step), move the playhead, drag a point
    h.state_mut().fx_editors.clear();
    let c = h.state().model.clip(clip).cloned().unwrap();
    h.state_mut().player.seek(kt_time::Tick(c.start.0 + c.duration.0 / 2 + 9000));
    h.step();
    h.state_mut().edit_mask(owner, Some(1));
    h.step();
    let depth = h.state().model.undo_depth();
    let (a, b) = (at(&h, owner, (0.3, 0.3)), at(&h, owner, (0.2, 0.25)));
    drag(&mut h, a, b, egui::Modifiers::NONE);
    let m = picture(&h, clip).remove(0);
    let ticks = m.path_key_ticks();
    assert_eq!(ticks.len(), 2, "the drag did not key the path at the new tick: {ticks:?}");
    for r in m.shape_rows() {
        let n = m.auto.iter().find(|c| c.param == r).map_or(0, |c| c.keys.len());
        assert_eq!(n, 2, "{r} is not keyed at both ticks");
    }
    let now = m.at(ticks[1]);
    assert!(now.get("v0.x").unwrap() < 0.25, "the dragged point did not move");
    assert_eq!(now.get("v1.x"), Some(0.7), "an untouched point moved");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "the drag is one history step");
}

/// Test 1282: around a mask the preview still plays and pauses; during playback the handles rest.
#[test]
fn t1282_the_preview_click_and_playback_around_masks() {
    if crate::layout_tests::no_device() {
        return;
    }
    let (mut h, clip) = crate::layout_tests::harness_geom_ctl();
    let owner = MaskOwner::Picture(clip);
    with_path(&mut h, clip);
    assert!(!h.state().player.transport.is_playing());
    let corner = at(&h, owner, (0.97, 0.95));
    click_at(&mut h, corner, egui::Modifiers::NONE);
    // the single click is deferred for the double-click window (wall clock, t74): it ARRIVED
    assert!(
        h.state().preview_click_at.is_some() || h.state().player.transport.is_playing(),
        "a click away from the mask did not reach play/pause"
    );
    h.state_mut().preview_click_at = None;
    h.state_mut().on_transport_toggle();
    h.step();
    h.state_mut().mask_ctl_drawn = false;
    h.step();
    assert!(h.state().mask_ctl_drawn, "during playback the contour is not drawn");
    let before = picture(&h, clip);
    let (a, b) = (at(&h, owner, (0.3, 0.3)), at(&h, owner, (0.1, 0.1)));
    drag(&mut h, a, b, egui::Modifiers::NONE);
    assert_eq!(picture(&h, clip), before, "a handle was dragged during playback");
}

/// The mouse wheel over a window's body, as a person scrolls to what is below its fold.
fn wheel_down(h: &mut Harness<'_, KtApp>, at: egui::Pos2) {
    h.hover_at(at);
    h.step();
    for _ in 0..6 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    h.step();
}

/// Open the collapsed "Masks" group of a form (it is collapsed while there are no masks).
fn open_masks(h: &mut Harness<'_, KtApp>) {
    h.get_by_label(crate::i18n::tr(crate::i18n::k::masks::GROUP)).click();
    // the group opens with an animation: its body is clipped until it settles
    for _ in 0..15 {
        h.step();
    }
}

fn harness_with_slot() -> (Harness<'static, KtApp>, kt_model::ClipId) {
    let (mut h, clip) = crate::layout_tests::harness_geom_ctl();
    let fx = vec![kt_model::FxInstance::plugin("duotone").unwrap()];
    h.state_mut().model.apply(Command::SetFx { id: clip, fx }).unwrap();
    h.state_mut().open_fx_editor(clip);
    if let Some(ed) = h.state_mut().fx_editors.first_mut() {
        ed.selected.insert(clip, crate::widgets::ChainSel::Fx(0));
    }
    h.step();
    h.step();
    (h, clip)
}

/// Test 1279: an effect is limited to a mask from the effects window; Cancel rolls it back.
#[test]
fn t1279_an_effect_is_limited_to_a_mask_from_the_effects_window() {
    if crate::layout_tests::no_device() {
        return;
    }
    let (mut h, clip) = harness_with_slot();
    open_masks(&mut h);
    h.get_by_label(crate::i18n::tr(crate::i18n::k::masks::ELLIPSE)).click();
    h.step();
    let owner = MaskOwner::Slot(clip, 0);
    assert_eq!(h.state().mask_ctl.as_ref().map(|e| (e.owner, e.tool)), Some((owner, MaskTool::Ellipse)));
    let (a, b) = (at(&h, owner, (0.35, 0.35)), at(&h, owner, (0.65, 0.65)));
    drag(&mut h, a, b, egui::Modifiers::NONE);
    let c = h.state().model.clip(clip).cloned().unwrap();
    assert_eq!(c.masks.slot(0).len(), 1, "the slot has no mask");
    assert!(matches!(c.masks.slot(0)[0].shape, MaskShape::Ellipse { .. }));
    assert!(c.masks.picture.is_empty(), "the picture got the slot's mask");
    h.get_by_label(crate::i18n::tr(crate::i18n::k::masks::INVERT)).click();
    h.step();
    h.step();
    assert!(h.state().model.clip(clip).unwrap().masks.slot(0)[0].invert, "Invert did not reach the model");
    h.get_by_label(crate::i18n::tr(crate::i18n::k::fx::CANCEL)).click();
    h.step();
    h.step();
    assert!(h.state().model.clip(clip).unwrap().masks.is_empty(), "Cancel did not roll the masks back");
}

/// Test 1281: the picture masks in the Geometry node — two masks, a mode, the order.
#[test]
fn t1281_the_picture_masks_live_in_the_geometry_node() {
    if crate::layout_tests::no_device() {
        return;
    }
    let (mut h, clip) = crate::layout_tests::harness_geom_ctl();
    let owner = MaskOwner::Picture(clip);
    h.state_mut().open_fx_editor(clip);
    if let Some(ed) = h.state_mut().fx_editors.first_mut() {
        ed.selected.insert(clip, crate::widgets::ChainSel::Lead);
    }
    h.step();
    h.step();
    let open_lead = |h: &mut Harness<'_, KtApp>| {
        h.state_mut().open_fx_editor(clip);
        if let Some(ed) = h.state_mut().fx_editors.first_mut() {
            ed.selected.insert(clip, crate::widgets::ChainSel::Lead);
        }
        h.step();
        h.step();
    };
    open_masks(&mut h);
    for (tool, a, b) in [
        (crate::i18n::k::masks::RECT, (0.2, 0.2), (0.8, 0.8)),
        (crate::i18n::k::masks::ELLIPSE, (0.4, 0.4), (0.6, 0.6)),
    ] {
        h.get_by_label(crate::i18n::tr(tool)).click();
        h.step();
        // the window floats over the picture in the harness: Done, then draw
        h.get_by_label(crate::i18n::tr(crate::i18n::k::fx::DONE)).click();
        h.step();
        let (a, b) = (at(&h, owner, a), at(&h, owner, b));
        drag(&mut h, a, b, egui::Modifiers::NONE);
        open_lead(&mut h);
    }
    assert_eq!(picture(&h, clip).len(), 2);
    // the ellipse's mode → Subtract (the second row's word)
    let words = h.get_all_by_label(crate::i18n::tr(crate::i18n::k::masks::MODE_SUBTRACT)).count();
    assert_eq!(words, 2, "each mask row has its mode words");
    // a person scrolls the window's body down to the masks
    let r = h.get_all_by_label(crate::i18n::tr(crate::i18n::k::masks::MODE_SUBTRACT)).nth(1).expect("the row").rect();
    wheel_down(&mut h, r.center() - egui::vec2(0.0, 120.0));
    h.get_all_by_label(crate::i18n::tr(crate::i18n::k::masks::MODE_SUBTRACT)).nth(1).expect("the second row's mode").click();
    h.step();
    h.step();
    let modes: Vec<_> = picture(&h, clip).iter().map(|m| m.mode).collect();
    assert_eq!(modes[1], kt_model::MaskMode::Subtract, "the mode did not reach the model: {modes:?}");
    // ↑ on the second row: the order is the order of the operations
    let win = h.ctx.memory(|m| m.area_rect(egui::Id::new(("fx", clip)))).expect("the effects window");
    h.get_all_by_label(crate::icons::LANE_UP)
        .filter(|n| win.contains(n.rect().center()))
        .nth(1)
        .expect("the second row's ↑")
        .click();
    h.step();
    h.step();
    let ms = picture(&h, clip);
    assert!(matches!(ms[0].shape, MaskShape::Ellipse { .. }), "the order did not change");
}

/// Test 1280: the track window's Masks section; a track's masks are static and a curve is refused.
#[test]
fn t1280_the_track_window_has_its_masks_and_refuses_curves() {
    if crate::layout_tests::no_device() {
        return;
    }
    let (mut h, clip) = crate::layout_tests::harness_geom_ctl();
    let lane = h.state().model.clip(clip).unwrap().lane;
    let tid = h.state().model.project().track_at(kt_model::LaneKind::Video, lane).unwrap().id;
    h.state_mut().open_track_window(tid);
    h.step();
    h.step();
    assert_eq!(h.query_all_by_label(crate::icons::TIMER).count(), 0, "a track's mask offers a stopwatch");
    open_masks(&mut h);
    // the opened group grew the window's body: a person scrolls to its tools
    let r = h.get_by_label(crate::i18n::tr(crate::i18n::k::masks::RECT)).rect();
    wheel_down(&mut h, r.center() - egui::vec2(0.0, 120.0));
    h.get_by_label(crate::i18n::tr(crate::i18n::k::masks::RECT)).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().mask_ctl.as_ref().map(|e| e.tool),
        Some(MaskTool::Rect),
        "the track window's Rectangle did not arm the tool"
    );
    // the window floats over the picture in the harness: close it, the tool stays armed
    h.state_mut().track_win = None;
    h.step();
    let owner = MaskOwner::Track(tid);
    let (a, b) = (at(&h, owner, (0.1, 0.1)), at(&h, owner, (0.5, 0.9)));
    drag(&mut h, a, b, egui::Modifiers::NONE);
    let t = h.state().model.project().track(tid).unwrap().masks.clone();
    assert_eq!(t.len(), 1, "the track has no mask");
    let mut keyed = t.clone();
    kt_model::set_key(&mut keyed[0].auto, "feather", kt_time::Tick(0), 3.0, kt_model::SegShape::Linear);
    let r = h.state_mut().model.apply(Command::SetTrackMasks { id: tid, masks: keyed });
    assert!(r.is_err(), "a curve on a track mask was accepted");
    assert_eq!(h.state().model.project().track(tid).unwrap().masks, t, "the refusal touched the model");
}

/// Test 1283: the mask texts exist in all five catalogs (their glyphs — t100).
#[test]
fn t1283_mask_texts_are_in_every_catalog() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("i18n");
    let keys_of = |lang: &str| -> Vec<String> {
        let text = std::fs::read_to_string(dir.join(format!("{lang}.toml"))).unwrap();
        let mut on = false;
        let mut out = Vec::new();
        for l in text.lines() {
            if l.starts_with('[') {
                on = l.trim() == "[masks]";
                continue;
            }
            if on && let Some((k, v)) = l.split_once(" = ") {
                assert!(v.trim().len() > 2, "{lang}: masks.{k} is empty");
                out.push(k.trim().to_string());
            }
        }
        out
    };
    let en = keys_of("en");
    assert!(en.len() > 40, "the masks section is missing");
    for lang in ["ru", "es", "zh-Hans", "ar"] {
        assert_eq!(keys_of(lang), en, "{lang}: the masks section differs from English");
    }
}
