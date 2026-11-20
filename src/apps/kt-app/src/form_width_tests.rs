//! fx-form-width: a form uses the width it is given (ui-reach).
//!
//! The owner, kkt-26.11.07 line 13: the camera ISP's sheet ran for screens while half of its width
//! stood empty, "I asked so many times to remember these problems FOR EVER". The mechanism is the
//! shared grid of one column of rows; the guard lays out every kind's form at three widths.

use eframe::egui;
use egui_kittest::Harness;
use kt_model::{FxInstance, GeneratorKind, HasParams, TransitionKind};

/// The widths the guard lays a form out at: narrow, the effects window's default, wide.
const NARROW: f32 = 420.0;
const DEFAULT: f32 = 560.0;
const WIDE: f32 = 1600.0;
/// One screen of form: a form shorter than this at the default width needs no columns.
const SCREEN_H: f32 = 800.0;
/// The most of a wide form's width that may stand unused.
const UNUSED_MAX: f32 = 0.25;

/// Forms that cannot pass, with the reason (≥ 25 characters). Names are `effect <tag>`,
/// `transition <name>`, `generator <tag>`.
pub(crate) const FORM_EXEMPT: &[(&str, &str)] = &[];

/// What a form took when laid out at one width.
#[derive(Debug, Clone, Copy)]
struct Laid {
    /// The form's height.
    h: f32,
    /// The union of what it drew (its labels and fields, as the program notes them).
    used: egui::Rect,
}

/// Lay a form out at `w` and read what it took.
fn lay<P: 'static>(w: f32, carrier: P, draw: impl Fn(&mut egui::Ui, &mut P) + 'static) -> Laid {
    crate::widgets::field_probe::clear_seen();
    let out = std::rc::Rc::new(std::cell::Cell::new(egui::Rect::NOTHING));
    let seen = out.clone();
    let mut h = Harness::builder().with_size(egui::vec2(w, 9000.0)).build_ui_state(
        move |ui, p: &mut P| {
            let r = ui.scope(|ui| draw(ui, p)).response.rect;
            seen.set(r);
        },
        carrier,
    );
    for _ in 0..3 {
        h.step();
    }
    let used = crate::widgets::field_probe::seen_union();
    Laid { h: out.get().height(), used }
}

/// The three checks of D3 over one form; a problem is a line.
fn judge(name: &str, narrow: Laid, default: Laid, wide: Laid, problems: &mut Vec<String>) {
    if FORM_EXEMPT.iter().any(|(n, _)| *n == name) {
        return;
    }
    // 1. a wide form uses its width, unless it already fits one screen at the default width
    if default.h > SCREEN_H && wide.used.is_positive() {
        let unused = 1.0 - (wide.used.right().min(WIDE) - wide.used.left().max(0.0)) / WIDE;
        if unused > UNUSED_MAX {
            problems.push(format!(
                "{name}: at {WIDE} pt the form leaves {:.0} % of its width unused and is {:.0} pt tall",
                unused * 100.0,
                wide.h
            ));
        }
    }
    // 2. widening never makes it taller
    if wide.h > default.h + 1.0 || default.h > narrow.h + 1.0 {
        problems.push(format!(
            "{name}: taller when wider — {:.0} pt at {NARROW}, {:.0} at {DEFAULT}, {:.0} at {WIDE}",
            narrow.h, default.h, wide.h
        ));
    }
    // 3. no row is squeezed below its minimum: nothing stands past the narrow form's edge
    if narrow.used.is_positive() && narrow.used.right() > NARROW + 1.0 {
        problems.push(format!(
            "{name}: at {NARROW} pt a row reaches x={:.0} — a column narrower than its minimum",
            narrow.used.right()
        ));
    }
}

fn effect_form(ui: &mut egui::Ui, f: &mut FxInstance) {
    let mut hover = None;
    let on_video = f.class() != kt_model::FxClass::Audio;
    crate::fx_editor::draw_video_fx_params(ui, f, None, &[], on_video, &mut hover, None);
}

fn grid_form<P: HasParams + ?Sized>(ui: &mut egui::Ui, p: &mut P) {
    crate::widgets::params_grid(ui, "form_width", p, &[], None);
}

/// Every registered kind's form at the three widths: (name, narrow, default, wide).
fn every_form() -> Vec<(String, Laid, Laid, Laid)> {
    crate::plugins::install();
    let mut out = Vec::new();
    fn three<P: Clone + 'static>(p: &P, draw: fn(&mut egui::Ui, &mut P)) -> (Laid, Laid, Laid) {
        (lay(NARROW, p.clone(), draw), lay(DEFAULT, p.clone(), draw), lay(WIDE, p.clone(), draw))
    }
    for f in FxInstance::all() {
        let (n, d, w) = three(f, effect_form);
        out.push((format!("effect {}", f.tag()), n, d, w));
    }
    for t in TransitionKind::all() {
        let (n, d, w) = three(t, |ui, p: &mut TransitionKind| grid_form(ui, p));
        out.push((format!("transition {t:?}"), n, d, w));
    }
    for g in GeneratorKind::all() {
        let (n, d, w) = three(&g, |ui, p: &mut GeneratorKind| grid_form(ui, p));
        out.push((format!("generator {}", g.tag()), n, d, w));
    }
    out
}

/// Test t3591 (fx-form-width): the camera ISP form fills a wide window — several block columns,
/// at most a quarter unused, shorter than at the default width; at 420 nothing is squeezed.
#[test]
fn t3591_the_camera_isp_form_fills_a_wide_window() {
    crate::plugins::install();
    let isp = FxInstance::all().iter().find(|f| f.tag() == "camera_isp").expect("camera_isp is registered").clone();
    let (n, d, w) = (lay(NARROW, isp.clone(), effect_form), lay(DEFAULT, isp.clone(), effect_form), lay(WIDE, isp, effect_form));
    assert!(d.h > SCREEN_H, "the scene: the ISP form at the default width runs past one screen ({:.0} pt)", d.h);
    assert!(w.h < d.h * 0.6, "at {WIDE} pt the ISP form is {:.0} pt tall, at {DEFAULT} {:.0}: its blocks did not stand side by side", w.h, d.h);
    let mut problems = Vec::new();
    judge("effect camera_isp", n, d, w, &mut problems);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Test t3593 (fx-form-width): every registered effect, transition and generator form passes the
/// width guard at a narrow, the default and a wide width, or is listed with its reason.
#[test]
fn t3593_every_form_uses_its_width() {
    let mut problems = Vec::new();
    let forms = every_form();
    assert!(forms.len() > 60, "only {} forms were laid out — the registries were lost", forms.len());
    for (name, n, d, w) in &forms {
        judge(name, *n, *d, *w, &mut problems);
    }
    assert!(problems.is_empty(), "forms that do not use their width:\n  {}", problems.join("\n  "));
}

/// The height a registry window is given before it is widened: short enough that a long form
/// scrolls.
const SHORT_H: f32 = 600.0;

/// Registry windows that cannot reflow, with the reason (≥ 25 characters).
pub(crate) const WINDOW_EXEMPT: &[(&str, &str)] = &[];

/// Every registry window, opened at its default size and then widened to `WIDE` by its corner
/// as a person does: (id, the body's height and its content's at the default, the same at the wide).
fn every_window() -> Vec<(&'static str, f32, f32, f32, f32)> {
    use crate::reach_tests::{OpenWin, window_openers};
    use crate::window_fill_tests::{enlarge, window_rect};
    let mut out = Vec::new();
    for (id, how, _) in window_openers() {
        let OpenWin::By(open) = how else { continue };
        let mut h = crate::layout_tests::harness_sized(1920.0, 1200.0);
        let layers = crate::layout_tests::opened_layers(&mut h, open);
        for _ in 0..4 {
            h.step();
        }
        let Some(&layer) = layers.iter().find(|l| l.order == egui::Order::Middle) else { continue };
        // a sheet a person reads by scrolling: the default width, a short height
        let r0 = window_rect(&h, layer);
        enlarge(&mut h, layer, r0.width(), SHORT_H);
        for _ in 0..3 {
            h.step();
        }
        let (Some(body), Some(at_default)) =
            (crate::widgets::fill_probe::get(&h.ctx, layer), crate::widgets::fill_probe::content(&h.ctx, layer))
        else {
            continue;
        };
        let r1 = window_rect(&h, layer);
        enlarge(&mut h, layer, WIDE, r1.height());
        for _ in 0..3 {
            h.step();
        }
        let at_wide = crate::widgets::fill_probe::content(&h.ctx, layer).unwrap_or(at_default);
        let body_wide = crate::widgets::fill_probe::get(&h.ctx, layer).map_or(body.height(), |b| b.height());
        out.push((id, body.height(), at_default, body_wide, at_wide));
    }
    out
}

/// Test t3593b (fx-form-width): every registry window's form is no taller when widened, and a
/// form that scrolls at the default size gets shorter in a wide window (it reflows into columns).
#[test]
fn t3593b_every_window_reflows_when_widened() {
    let wins = every_window();
    assert!(wins.len() >= 12, "only {} windows were widened — the openers were lost", wins.len());
    let mut problems = Vec::new();
    for (id, body, at_default, body_wide, at_wide) in wins {
        if WINDOW_EXEMPT.iter().any(|(n, _)| *n == id) {
            continue;
        }
        // a body whose content only fills it (a graph canvas) is no form running long
        if at_wide > at_default + 1.0 && at_wide > body_wide + 1.0 {
            problems.push(format!("“{id}”: its form is {at_default:.0} pt tall, {at_wide:.0} when widened"));
        } else if at_default > body + 1.0 && at_wide > at_default - 1.0 {
            problems.push(format!(
                "“{id}”: its form scrolls ({at_default:.0} pt in a body of {body:.0}) and is no shorter at {WIDE} pt — \
                 the width is left unused"
            ));
        }
    }
    assert!(problems.is_empty(), "windows that do not use their width:\n  {}", problems.join("\n  "));
}

/// Test t3594 (fx-form-width): the exempt list names only existing forms, each with a reason.
#[test]
fn t3594_the_exempt_forms_exist_and_say_why() {
    crate::plugins::install();
    let mut names: Vec<String> = FxInstance::all().iter().map(|f| format!("effect {}", f.tag())).collect();
    names.extend(TransitionKind::all().iter().map(|t| format!("transition {t:?}")));
    names.extend(GeneratorKind::all().iter().map(|g| format!("generator {}", g.tag())));
    for (name, why) in FORM_EXEMPT {
        assert!(names.iter().any(|n| n == name), "“{name}” in FORM_EXEMPT is no registered form");
        assert!(why.chars().count() >= 25, "“{name}”: the reason “{why}” is too short");
    }
    let wins: Vec<&str> = crate::reach_tests::window_openers().into_iter().map(|(id, _, _)| id).collect();
    for (id, why) in WINDOW_EXEMPT {
        assert!(wins.contains(id), "“{id}” in WINDOW_EXEMPT is no registry window");
        assert!(why.chars().count() >= 25, "“{id}”: the reason “{why}” is too short");
    }
}

