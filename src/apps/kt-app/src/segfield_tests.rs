//! segment-shape-fields (kkt-26.11.01 item 10, registry K10): the shape parameters of ONE
//! segment and of a SELECTION of segments, next to “Shape of all segments”.
//!
//! The owner, verbatim: “I want to set the zigzag's parameters and there is nowhere to write
//! them; if I set the zigzag for the whole curve, then there is… and what if I have 100
//! segments with parameters?”. Tests t1115–t1122; the contract — `openspec/specs/automation`
//! (“The shape parameters of one segment and of a selection are edited by fields”) and
//! `ui-reach` (“Selecting segments is reachable by mouse and keyboard and is said”).

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use kt_model::*;
use kt_time::Tick;

const ZIG: fn(f64, f64) -> SegShape =
    |periods, amplitude| SegShape::Zigzag { periods, amplitude, profile: ZigProfile::Triangle };

/// The FX window on event 1 (blur, a radius curve in material 30 000…210 000) with the radius
/// curve made of `keys`.
fn harness_keys(keys: Vec<AutoKey>) -> Harness<'static, KtApp> {
    let mut h = crate::layout_tests::harness_with_fx_window();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[0].fx_auto[0][0].keys = keys;
        p.validate().unwrap();
        app.model = TimelineModel::new(p);
    }
    h.step();
    h.step();
    h
}

/// `n + 1` evenly spaced keys over the curve's material with the values zig-zagging inside
/// the radius passport, every segment of shape `shape(i)`.
fn even_keys(n: usize, shape: impl Fn(usize) -> SegShape) -> Vec<AutoKey> {
    (0..=n)
        .map(|i| {
            let t = 30_000 + (180_000 * i / n) as i64;
            let v = if i % 2 == 0 { 12.0 } else { 36.0 };
            AutoKey::shaped(Tick(t), v, shape(i))
        })
        .collect()
}

fn keys_of(h: &Harness<'_, KtApp>) -> Vec<AutoKey> {
    h.state().model.project().clips[0].fx_auto[0][0].keys.clone()
}

fn view<'a>(h: &'a Harness<'_, KtApp>) -> &'a crate::automation::AutoView {
    h.state().fx_editors[0].auto_views.get(&(ClipId(1), 0)).expect("automation view of slot 0")
}

fn selected(h: &Harness<'_, KtApp>) -> Vec<usize> {
    let keys = keys_of(h);
    let c = ParamCurve { param: "radius".into(), keys, source: Default::default() };
    view(h).seg_sel.indices(0, &c)
}

/// A point inside segment `ki` at the fraction `frac` of its width, at the height of its keys
/// — taken from the drawn key diamonds (the probe of what is drawn, R.49).
fn seg_pt(h: &Harness<'_, KtApp>, ki: usize, frac: f32) -> egui::Pos2 {
    let m = &view(h).key_marks;
    let at = |i: usize| m.iter().find(|(c, k, _)| *c == 0 && *k == i).map(|(_, _, p)| *p).expect("key drawn");
    let (a, b) = (at(ki), at(ki + 1));
    egui::pos2(a.x + (b.x - a.x) * frac, (a.y + b.y) * 0.5)
}

fn press(h: &mut Harness<'_, KtApp>, pos: egui::Pos2, button: egui::PointerButton, mods: egui::Modifiers) {
    h.event(egui::Event::ModifiersChanged(mods));
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos, button, pressed, modifiers: mods });
    }
    h.step();
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

fn click(h: &mut Harness<'_, KtApp>, pos: egui::Pos2, mods: egui::Modifiers) {
    press(h, pos, egui::PointerButton::Primary, mods);
}

fn right_click(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) {
    press(h, pos, egui::PointerButton::Secondary, egui::Modifiers::NONE);
}

/// A drag with the button held, in several frames (egui declares a drag past its threshold).
fn drag(h: &mut Harness<'_, KtApp>, from: egui::Pos2, to: egui::Pos2, mods: egui::Modifiers) {
    h.event(egui::Event::ModifiersChanged(mods));
    h.hover_at(from);
    h.step();
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: mods });
    h.step();
    for f in [0.25, 0.5, 0.75, 1.0] {
        h.hover_at(from + (to - from) * f);
        h.step();
    }
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: mods });
    h.event(egui::Event::PointerGone);
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

/// Click the item `label` of the OPEN menu — the node nearest to where the menu was opened
/// (the curve block may draw the same word in its shape list).
fn menu_pick(h: &mut Harness<'_, KtApp>, label: &str, near: egui::Pos2) {
    let node = h
        .query_all_by_label(label)
        .min_by(|a, b| {
            let (da, db) = ((a.rect().center() - near).length(), (b.rect().center() - near).length());
            da.total_cmp(&db)
        })
        .unwrap_or_else(|| panic!("menu item “{label}” is not there"));
    node.click();
    h.step();
    h.step();
}

/// The rectangle of the painted text `src` lying lowest in the window (the selection row stands
/// under the “all” row, the same words appear in both).
fn lowest_text(h: &Harness<'_, KtApp>, src: &str) -> egui::Rect {
    crate::reach_tests::painted_texts(h)
        .into_iter()
        .filter(|t| t.src == src)
        .map(|t| t.rect)
        .max_by(|a, b| a.center().y.total_cmp(&b.center().y))
        .unwrap_or_else(|| panic!("“{src}” is not drawn"))
}

/// Type `text` into the number field that stands right of the label rect `label`, then Enter.
fn type_into_field(h: &mut Harness<'_, KtApp>, label: egui::Rect, text: &str) {
    let field = egui::pos2(label.right() + 22.0, label.center().y);
    click(h, field, egui::Modifiers::NONE);
    h.event(egui::Event::Text(text.into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
}

fn zig_periods(keys: &[AutoKey]) -> Vec<f64> {
    keys.iter()
        .map(|k| match k.shape {
            SegShape::Zigzag { periods, .. } => periods,
            _ => f64::NAN,
        })
        .collect()
}

/// Test 1115: ONE segment shows its own parameters and an edit lands on it alone.
#[test]
fn t1115_one_segment_shows_and_edits_its_own_parameters() {
    let mut h = harness_keys(vec![
        AutoKey::shaped(Tick(30_000), 8.0, ZIG(4.0, 2.0)),
        AutoKey::shaped(Tick(90_000), 40.0, ZIG(4.0, 2.0)),
        AutoKey::shaped(Tick(150_000), 16.0, SegShape::Linear),
        AutoKey::shaped(Tick(210_000), 44.0, SegShape::Linear),
    ]);
    // right click on the second segment; picking its own shape closes the menu, changes nothing
    let at = seg_pt(&h, 1, 0.3);
    right_click(&mut h, at);
    menu_pick(&mut h, crate::widgets::shape_label(ZIG(1.0, 0.0)), at);
    assert_eq!(selected(&h), vec![1], "the right click did not select the segment");
    assert_eq!(keys_of(&h)[1].shape, ZIG(4.0, 2.0), "picking the same shape reset its parameters");
    // the row names the segment's bounds
    let caption = crate::tf!(k::fx::SHAPE_SEL_ONE, "1.000", "1.667");
    crate::layout_tests::reveal_until(&mut h, &caption);
    let depth = h.state().model.undo_depth();
    let label = lowest_text(&h, tr(k::fx::SHAPE_PERIODS));
    let caption_rect = lowest_text(&h, &caption);
    assert!((label.center().y - caption_rect.center().y).abs() < 4.0, "the periods field is not in the selection row");
    type_into_field(&mut h, label, "9");
    let keys = keys_of(&h);
    assert_eq!(keys[1].shape, ZIG(9.0, 2.0), "the typed periods did not reach the selected segment");
    assert_eq!(keys[0].shape, ZIG(4.0, 2.0), "the edit leaked onto the first segment");
    assert_eq!(keys[2].shape, SegShape::Linear, "the edit leaked onto the third segment");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "the edit is not one history step");
}

/// Test 1116: a SELECTION shows mixed values as mixed, a typed number lands on all of it.
#[test]
fn t1116_selection_shows_mixed_and_a_typed_number_lands_on_all() {
    let periods = [2.0, 4.0, 6.0, 8.0];
    let mut h = harness_keys(even_keys(4, |i| ZIG(periods[i.min(3)], 2.0)));
    let at = seg_pt(&h, 0, 0.3);
    right_click(&mut h, at);
    menu_pick(&mut h, crate::widgets::shape_label(ZIG(1.0, 0.0)), at);
    for ki in [1, 2] {
        let p = seg_pt(&h, ki, 0.3);
        click(&mut h, p, egui::Modifiers::COMMAND);
    }
    assert_eq!(selected(&h), vec![0, 1, 2], "Ctrl+click did not add the segments");
    assert_eq!(keys_of(&h).len(), 5, "Ctrl+click placed a key");
    let caption = crate::tf!(k::fx::SHAPE_SEL_MANY, 3);
    crate::layout_tests::reveal_until(&mut h, &caption);
    // the periods differ → “—”; the amplitudes are equal → the number
    let row_y = lowest_text(&h, &caption).center().y;
    let row: Vec<String> = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .filter(|t| (t.rect.center().y - row_y).abs() < 4.0)
        .map(|t| t.src)
        .collect();
    assert!(row.iter().any(|t| t == crate::widgets::MIXED), "the mixed periods are not shown as mixed: {row:?}");
    assert!(row.iter().any(|t| t.parse::<f64>() == Ok(2.0)), "the common amplitude is not shown: {row:?}");
    let depth = h.state().model.undo_depth();
    let label = lowest_text(&h, tr(k::fx::SHAPE_PERIODS));
    type_into_field(&mut h, label, "5");
    assert_eq!(zig_periods(&keys_of(&h))[..4], [5.0, 5.0, 5.0, 8.0], "the typed number did not land on exactly the selection");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "the edit of three segments is not one history step");
}

/// Every text the last frame painted (a standalone harness has no `KtApp` for
/// `reach_tests::painted_texts`, and a `DragValue` has no node in the accessibility tree).
fn drawn<S>(h: &Harness<'_, S>) -> Vec<String> {
    fn walk(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in &h.output().shapes {
        walk(&cs.shape, &mut out);
    }
    out
}

/// The shape row on its own, over the key indices `sel` of `curve`.
fn row_harness(curve: ParamCurve, sel: Vec<usize>) -> Harness<'static, ParamCurve> {
    Harness::new_ui_state(
        move |ui, c: &mut ParamCurve| {
            ui.horizontal(|ui| {
                let row = crate::widgets::ShapeRow { salt: "t", probe: "t", label: "Row", hint: "" };
                crate::widgets::shape_fields(ui, &row, c, &sel, &sel, &mut None);
            });
        },
        curve,
    )
}

fn curve(shapes: &[SegShape]) -> ParamCurve {
    let keys = (0..=shapes.len())
        .map(|i| AutoKey::shaped(Tick(i as i64 * 90_000), 1.0, shapes.get(i).copied().unwrap_or(SegShape::Linear)))
        .collect();
    ParamCurve { param: "radius".into(), keys, source: Default::default() }
}

fn tension(s: SegShape) -> f64 {
    match s {
        SegShape::Smooth { tension } | SegShape::EaseIn { tension } | SegShape::EaseOut { tension } => tension,
        other => panic!("not a tension shape: {other:?}"),
    }
}

/// Test 1117: a drag on a mixed field shifts every segment by one amount (each kept in its
/// bounds); mixed kinds show no parameter fields; a shape pick keeps same-kind parameters.
#[test]
fn t1117_drag_shifts_by_one_amount_and_a_pick_keeps_same_kind() {
    // --- mixed KINDS: “—” instead of the shape, no parameter fields --------------------
    let mut w = row_harness(
        curve(&[SegShape::Smooth { tension: 1.0 }, SegShape::Smooth { tension: 2.0 }, SegShape::Linear]),
        vec![0, 1, 2],
    );
    w.step();
    let texts = drawn(&w);
    assert!(texts.iter().any(|t| t == crate::widgets::MIXED), "the mixed kinds do not show “—”: {texts:?}");
    assert!(!texts.iter().any(|t| t == tr(k::fx::SHAPE_TENSION)), "a tension field over mixed kinds: {texts:?}");

    // --- a drag on the mixed tension field --------------------------------------------
    let shapes = [SegShape::Smooth { tension: 1.0 }, SegShape::Smooth { tension: 2.0 }, SegShape::Smooth { tension: 3.9 }];
    let mut w = row_harness(curve(&shapes), vec![0, 1, 2]);
    w.step();
    let label = w.get_by_label(tr(k::fx::SHAPE_TENSION)).rect();
    let from = egui::pos2(label.right() + 22.0, label.center().y);
    w.hover_at(from);
    w.step();
    w.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    w.step();
    for dx in [5.0, 10.0, 15.0, 20.0] {
        w.hover_at(from + egui::vec2(dx, 0.0));
        w.step();
    }
    w.event(egui::Event::PointerButton {
        pos: from + egui::vec2(20.0, 0.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    w.step();
    let t: Vec<f64> = w.state().keys[..3].iter().map(|k| tension(k.shape)).collect();
    let d = t[0] - 1.0;
    assert!(d > 0.1, "the drag did not move the tension: {t:?}");
    assert!((t[1] - (2.0 + d)).abs() < 1e-9, "the second segment did not move by the same amount: {t:?}");
    assert_eq!(t[2], crate::widgets::TENSION_BOUNDS.1, "the third did not stop at the bound: {t:?}");

    // --- a shape pick: the zigzag keeps its periods, the other takes the default --------
    let mut w = row_harness(curve(&[ZIG(7.0, 3.0), SegShape::Smooth { tension: 2.0 }]), vec![0, 1]);
    w.step();
    let label = w.get_by_label("Row").rect();
    let combo = egui::pos2(label.right() + 24.0, label.center().y);
    w.hover_at(combo);
    w.step();
    for pressed in [true, false] {
        w.event(egui::Event::PointerButton { pos: combo, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    w.step();
    w.step();
    w.get_by_label(crate::widgets::shape_label(ZIG(1.0, 0.0))).click();
    w.step();
    let k = &w.state().keys;
    assert_eq!(k[0].shape, ZIG(7.0, 3.0), "the zigzag lost its parameters on the pick");
    let default = crate::widgets::all_shapes()[5];
    assert_eq!(k[1].shape, default, "the smooth segment did not take the zigzag's defaults");
}

/// Test 1118: a sweep over empty space, a Shift run, and a plain click.
#[test]
fn t1118_sweep_range_and_plain_click() {
    let mut h = harness_keys(even_keys(6, |_| SegShape::Linear));
    let before = keys_of(&h);
    let (from, to) = (seg_pt(&h, 1, 0.3), seg_pt(&h, 3, 0.7));
    drag(&mut h, from, to, egui::Modifiers::NONE);
    assert_eq!(selected(&h), vec![1, 2, 3], "the sweep did not select the segments it covers");
    assert_eq!(keys_of(&h), before, "the sweep moved or added a key");
    let p = seg_pt(&h, 5, 0.3);
    click(&mut h, p, egui::Modifiers::SHIFT);
    assert_eq!(selected(&h), vec![3, 4, 5], "Shift+click did not select the run from the sweep's end");
    assert_eq!(keys_of(&h), before, "Shift+click placed a key");
    let p = seg_pt(&h, 0, 0.3);
    click(&mut h, p, egui::Modifiers::NONE);
    assert_eq!(keys_of(&h).len(), before.len() + 1, "a plain click must still place a key");
    assert!(selected(&h).is_empty(), "a plain click did not clear the selection");
}

/// Test 1119: the segment menu acts on the selection it belongs to and says so.
#[test]
fn t1119_segment_menu_applies_to_its_selection() {
    let mut h = harness_keys(even_keys(4, |_| SegShape::Linear));
    let (from, to) = (seg_pt(&h, 0, 0.3), seg_pt(&h, 2, 0.7));
    drag(&mut h, from, to, egui::Modifiers::NONE);
    assert_eq!(selected(&h), vec![0, 1, 2]);
    let at = seg_pt(&h, 1, 0.5);
    right_click(&mut h, at);
    let header = crate::tf!(k::fx::SHAPE_SEGMENTS_N, 3);
    assert!(h.query_all_by_label(&header).count() > 0, "the menu header does not say three segments");
    menu_pick(&mut h, crate::widgets::shape_label(SegShape::Hold), at);
    let shapes: Vec<SegShape> = keys_of(&h).iter().map(|k| k.shape).collect();
    assert_eq!(shapes[..4], [SegShape::Hold, SegShape::Hold, SegShape::Hold, SegShape::Linear], "the menu did not act on the selection");
    // a segment outside the selection becomes the selection alone
    let at = seg_pt(&h, 3, 0.5);
    right_click(&mut h, at);
    assert_eq!(selected(&h), vec![3], "a right click outside the selection did not select only it");
    menu_pick(&mut h, crate::widgets::shape_label(SegShape::Linear), at);
}

/// Test 1120: the shape fields write only from a gesture: a legal model value beyond the
/// field's bounds is shown, not clamped into the model by drawing. Red on the former “all”
/// row (its `DragValue` clamped periods 500 to 256 on the first frame).
#[test]
fn t1120_shape_fields_write_only_from_a_gesture() {
    let c = curve(&[ZIG(500.0, 2.0), ZIG(500.0, 2.0), ZIG(500.0, 2.0)]);
    let mut w = Harness::new_ui_state(
        |ui, c: &mut ParamCurve| {
            ui.horizontal(|ui| {
                crate::widgets::shape_all_control(ui, c, &mut None);
            });
            ui.horizontal(|ui| {
                let row = crate::widgets::ShapeRow { salt: "sel", probe: "sel", label: "Sel", hint: "" };
                crate::widgets::shape_fields(ui, &row, c, &[1], &[1], &mut None);
            });
        },
        c.clone(),
    );
    for _ in 0..60 {
        w.step();
    }
    assert_eq!(w.state().keys, c.keys, "drawing the shape fields wrote into the curve");
    let texts = drawn(&w);
    assert!(
        texts.iter().filter(|t| t.parse::<f64>() == Ok(500.0)).count() >= 2,
        "the out-of-range periods are not shown as they are: {texts:?}"
    );
}

/// Test 1121: the keyboard path through the selection row, and its clear button.
#[test]
fn t1121_keyboard_reaches_the_selection_row_and_clear_drops_it() {
    let mut h = harness_keys(even_keys(3, |_| ZIG(4.0, 2.0)));
    let at = seg_pt(&h, 1, 0.3);
    right_click(&mut h, at);
    menu_pick(&mut h, crate::widgets::shape_label(ZIG(1.0, 0.0)), at);
    let caption = crate::tf!(k::fx::SHAPE_SEL_ONE, "1.000", "1.667");
    crate::layout_tests::reveal_until(&mut h, &caption);
    // start in the “all” row's amplitude field, then Tab only
    let amps: Vec<egui::Rect> = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .filter(|t| t.src == tr(k::fx::SHAPE_AMP))
        .map(|t| t.rect)
        .collect();
    let all_amp = amps.iter().min_by(|a, b| a.center().y.total_cmp(&b.center().y)).copied().expect("amplitude label");
    click(&mut h, egui::pos2(all_amp.right() + 22.0, all_amp.center().y), egui::Modifiers::NONE);
    let target = lowest_text(&h, tr(k::fx::SHAPE_PERIODS));
    let target = egui::pos2(target.right() + 22.0, target.center().y);
    let focused_at = |h: &Harness<'_, KtApp>| {
        h.ctx.memory(|m| m.focused()).and_then(|id| h.ctx.read_response(id)).is_some_and(|r| r.rect.contains(target))
    };
    let mut tabs = 0;
    while !focused_at(&h) && tabs < 8 {
        h.key_press(egui::Key::Tab);
        h.step();
        tabs += 1;
    }
    assert!(focused_at(&h), "Tab did not reach the selection row's periods field in {tabs} presses");
    h.event(egui::Event::Text("3".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
    assert_eq!(zig_periods(&keys_of(&h))[..3], [4.0, 3.0, 4.0], "the typed value did not land on the one selected segment");
    // the clear button at the end of the row
    let before = keys_of(&h);
    let row_y = lowest_text(&h, &caption).center().y;
    let x = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .find(|t| t.src == crate::icons::CLEAR && (t.rect.center().y - row_y).abs() < 6.0)
        .map(|t| t.rect.center())
        .expect("the clear button is in the selection row");
    click(&mut h, x, egui::Modifiers::NONE);
    assert!(selected(&h).is_empty(), "the clear button did not drop the selection");
    assert_eq!(keys_of(&h), before, "clearing the selection changed the curve");
    assert!(
        !crate::reach_tests::painted_texts(&h).iter().any(|t| t.src == caption),
        "the selection row is still drawn"
    );
}

/// Test 1122: the gestures are said — the hover hint over a segment, the row caption with the
/// count, the menu header — and the texts exist in every language.
#[test]
fn t1122_segment_selection_is_said() {
    let mut h = harness_keys(even_keys(4, |_| SegShape::Linear));
    let p = seg_pt(&h, 1, 0.3);
    h.hover_at(p);
    h.step();
    h.step();
    assert_eq!(h.state().hover_hint.as_deref(), Some(tr(k::fx::SEG_SELECT_HINT)), "no hint over a segment");
    let at = seg_pt(&h, 0, 0.3);
    right_click(&mut h, at);
    menu_pick(&mut h, crate::widgets::shape_label(SegShape::Linear), at);
    let p = seg_pt(&h, 1, 0.3);
    click(&mut h, p, egui::Modifiers::COMMAND);
    let caption = crate::tf!(k::fx::SHAPE_SEL_MANY, 2);
    crate::layout_tests::reveal_until(&mut h, &caption);
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Ru, crate::i18n::Lang::Es, crate::i18n::Lang::Zh, crate::i18n::Lang::Ar] {
        crate::i18n::set_lang(lang);
        for key in [
            k::fx::SEG_SELECT_HINT,
            k::fx::SHAPE_SEL_ONE,
            k::fx::SHAPE_SEL_MANY,
            k::fx::SHAPE_SEL_HINT,
            k::fx::SHAPE_SEL_CLEAR_HINT,
            k::fx::SHAPE_SEGMENTS_N,
        ] {
            assert!(!tr(key).is_empty(), "{lang:?}: an empty text");
        }
        assert!(tr(k::fx::SEG_SELECT_HINT).contains("Ctrl"), "{lang:?}: the hint does not name Ctrl+click");
    }
    crate::i18n::set_lang(crate::i18n::Lang::En);
}
