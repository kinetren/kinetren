//! t1337 (pitch-tail-choice, D1; capability `ui-reach`): the pitch's panel shows "Echo of the effects
//! before the pitch" with its two choices and the hint, and a choice writes the slot's row into the
//! MODEL as one undoable edit.

use crate::app::KtApp;
use crate::i18n::{Lang, k, tr};
use crate::layout_tests::harness;
use eframe::egui;
use egui_kittest::Harness;

/// A project with one audio event carrying a pitch; its effects window is open.
fn harness_with_pitch() -> Harness<'static, KtApp> {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:tone"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::plugin_with("delay", &[100.0, 0.0, 0.5, 0.0]).unwrap(), FxInstance::plugin_with("pitch_shift", &[5.0, 0.0]).unwrap()],
        fx_bypass: vec![false, false],
        fx_auto: vec![vec![], vec![]],
        opacity: 1.0,
        fx_off: false,
        masks: Default::default(),
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(kt_model::ClipId(1));
    h.step();
    h.step();
    h
}

fn click(h: &mut Harness<'static, KtApp>, at: egui::Pos2) {
    h.hover_at(at);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.step();
    h.step();
}

fn echo_row(h: &Harness<'static, KtApp>) -> Option<f64> {
    match &h.state().model.clip(kt_model::ClipId(1)).expect("the event").fx[1] {
        kt_model::FxInstance::Plugin(p) => p.get("echo_before"),
        _ => None,
    }
}

#[test]
fn t1337_the_pitch_panel_switches_the_echo() {
    use egui_kittest::kittest::Queryable;
    // every catalog names the line, both choices and the hint
    for lang in Lang::ALL {
        crate::i18n::set_lang(lang);
        for key in [k::fx::ECHO_BEFORE, k::fx::ECHO_CUT, k::fx::ECHO_RING, k::fx::ECHO_HINT_CUT, k::fx::ECHO_HINT_RING] {
            assert!(!tr(key).trim().is_empty(), "{lang:?}: an echo string is empty");
        }
    }
    crate::i18n::set_lang(Lang::En);
    let mut h = harness_with_pitch();
    // the pitch slot selected: the second slot of the strip
    if h.query_by_label(tr(k::fx::ECHO_BEFORE)).is_none() {
        let slot = h.query_by_label("Pitch Shift").expect("the pitch slot on the strip").rect();
        click(&mut h, slot.center());
    }
    use egui::accesskit::Role;
    use egui_kittest::kittest::by;
    let line = h.query_by_label(tr(k::fx::ECHO_BEFORE)).expect("the echo line in the pitch panel").rect();
    // a ComboBox has no label in the accessibility tree: it is found by its shown value
    assert!(h.query(by().role(Role::ComboBox).value(tr(k::fx::ECHO_CUT))).is_some(), "the default \"cut\" is shown");
    assert_eq!(echo_row(&h), Some(0.0));
    // the hint on hover
    h.hover_at(line.center());
    h.step();
    h.step();
    let (text, _) = h.state().status_line_shown.clone();
    assert!(text.starts_with(tr(k::fx::ECHO_HINT_CUT)), "no hint for the echo line: “{text}”");
    // open the choice, pick "rings on"
    h.get(by().role(Role::ComboBox).value(tr(k::fx::ECHO_CUT))).click();
    h.step();
    h.get_by_label(tr(k::fx::ECHO_RING)).click();
    h.step();
    h.step();
    assert_eq!(echo_row(&h), Some(1.0), "the choice did not reach the model");
    assert!(h.query(by().role(Role::ComboBox).value(tr(k::fx::ECHO_RING))).is_some(), "the line does not show the new choice");
    // one undo step returns "cut"
    h.state_mut().undo();
    h.step();
    assert_eq!(echo_row(&h), Some(0.0), "undo did not return \"cut\"");
}
