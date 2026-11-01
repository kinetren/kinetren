//! t1193 (vhs-tape-simulation, D11; capability `video-fx`): the effects window of the VHS tape
//! simulation shows its deck face — a drum with its heads, the speed switch, the tracking field —
//! and a click on a head plus a condition writes that head's physical quantities into the MODEL.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::layout_tests::harness;
use eframe::egui;
use egui_kittest::Harness;

/// A project with one video event carrying the tape simulation; its effects window is open.
fn harness_with_tape() -> Harness<'static, KtApp> {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
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
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::builtin("vhs_tape")],
        fx_bypass: vec![false],
        fx_auto: vec![vec![]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(kt_model::ClipId(1));
    h.step();
    h.step();
    h
}

/// A press and a release in place, as explicit events (the harness rule: `drop_at` also sends
/// PointerGone, which a click on a painted widget does not survive).
fn click(h: &mut Harness<'static, KtApp>, at: egui::Pos2) {
    h.hover_at(at);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.step();
    h.step();
}

fn tape(h: &Harness<'static, KtApp>) -> kt_model::FxInstance {
    h.state().model.clip(kt_model::ClipId(1)).expect("the event").fx[0].clone()
}

#[test]
fn t1193_the_deck_face_sets_a_head_s_condition() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_tape();
    // the face: two heads on the drum, the speed switch, the tracking field
    let a = h.query_by_label("deck.head.0.0").expect("head A on the drum").rect();
    let b = h.query_by_label("deck.head.0.1").expect("head B on the drum").rect();
    assert!(a.center().x > b.center().x, "head A sits at 0°, head B at 180°");
    assert!(h.query_by_label(tr(k::deck::SP)).is_some() && h.query_by_label(tr(k::deck::LP)).is_some(), "no speed switch");
    assert!(h.query_by_label(tr(k::deck::PICK_HEAD)).is_some(), "the face does not say what to click");
    assert_eq!(crate::deck_face::cond_of(&tape(&h), 0, 1), crate::deck_face::Cond::Clean);

    // click head B, then "Chipped"
    click(&mut h, b.center());
    let chip = h.query_by_label(tr(k::deck::CHIPPED)).expect("the head's condition selector").rect();
    click(&mut h, chip.center());
    let f = tape(&h);
    let [s, g, o] = crate::deck_face::head_rows(0, 1);
    // the rows are stored as f32 (the file form), so compare within its precision
    let near = |row: &str, want: f64| (f.get_param(row).expect(row) - want).abs() < 1e-6;
    assert!(near(o, -6.0), "head B's output is not the chipped one");
    assert!(near(g, 0.6), "head B's gap is not the chipped one");
    assert!(near(s, 0.05), "head B's spacing is not the chipped one");
    assert_eq!(crate::deck_face::cond_of(&f, 0, 1), crate::deck_face::Cond::Chipped);
    // head A is untouched
    assert_eq!(crate::deck_face::cond_of(&f, 0, 0), crate::deck_face::Cond::Clean);

    // the speed switch writes the choice row
    let lp = h.query_by_label(tr(k::deck::LP)).expect("LP").rect();
    click(&mut h, lp.center());
    assert_eq!(tape(&h).get_param("speed"), Some(1.0), "LP did not reach the model");
    // two decks draw a second drum
    let two = h.query_by_label(tr(k::deck::TWO)).expect("two decks").rect();
    click(&mut h, two.center());
    assert_eq!(tape(&h).get_param("decks"), Some(1.0));
    assert!(h.query_by_label("deck.head.1.0").is_some(), "two decks: no play drum");
}
