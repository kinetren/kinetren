//! Transition window (R.33 C4): the kind, its parameters, the audio curve.
//!
//! Until this round a transition had NOT A SINGLE window: the kind existed in
//! one variant, and the offsets were dragged with the mouse. The user’s claim —
//! “on any splices the transition is always a fade… these must be unified like an fx-chain”,
//! so the window is built like the FX editor: a kind picker by category
//! (the registry `TransitionKind::ALL`), parameters — the same
//! `widgets::params_grid` from the same `ParamSpec` as for effects,
//! cancel rolls the whole session back in one undo step.

use crate::app::KtApp;
use crate::i18n::{Key, k, tr};
use crate::widgets;
use eframe::egui;
use kt_model::{AudioCurve, Command, TransitionEvent, TransitionId, TransitionKind};

pub struct TransitionEditor {
    pub id: TransitionId,
    /// the state of the curves widget (R.34 K4)
    pub view: crate::automation::AutoView,
    /// the state at the moment of opening — “Cancel” restores it
    pub backup: TransitionEvent,
    pub gesture: u64,
    pub open: bool,
}

/// the kind's category for grouping in the picker. The families are the very three
/// agreed with the user (wipes, shifts, stylised).
pub fn kind_category(kind: &TransitionKind) -> Key {
    match kind {
        TransitionKind::Crossfade => k::trn::cat::BASIC,
        TransitionKind::Wipe { .. }
        | TransitionKind::Iris { .. }
        | TransitionKind::Clock { .. }
        | TransitionKind::Barn { .. }
        | TransitionKind::Luma { .. } => k::trn::cat::WIPE,
        TransitionKind::Push { .. }
        | TransitionKind::Slide { .. }
        | TransitionKind::Zoom { .. }
        | TransitionKind::Spin { .. }
        | TransitionKind::Whip { .. }
        | TransitionKind::Split { .. }
        | TransitionKind::Squeeze { .. }
        | TransitionKind::Stretch { .. }
        | TransitionKind::Swap { .. } => k::trn::cat::MOTION,
        // patterns (R.51): mask patterns — their own family
        TransitionKind::Blinds { .. }
        | TransitionKind::Checker { .. }
        | TransitionKind::Blocks { .. }
        | TransitionKind::Spiral { .. } => k::trn::cat::PATTERN,
        // volumetric (R.51): panels and cylinders
        TransitionKind::Flip { .. }
        | TransitionKind::Cube { .. }
        | TransitionKind::Door { .. }
        | TransitionKind::PagePeel { .. }
        | TransitionKind::PageRoll { .. } => k::trn::cat::VOLUME,
        _ => k::trn::cat::STYLE,
    }
}

/// Localised name of the kind. Full paths — test 125 looks up the key by them.
pub fn kind_name(kind: &TransitionKind) -> &'static str {
    match kind {
        TransitionKind::Crossfade => tr(k::trn::kind::CROSSFADE),
        TransitionKind::Wipe { .. } => tr(k::trn::kind::WIPE),
        TransitionKind::Iris { .. } => tr(k::trn::kind::IRIS),
        TransitionKind::Clock { .. } => tr(k::trn::kind::CLOCK),
        TransitionKind::Barn { .. } => tr(k::trn::kind::BARN),
        TransitionKind::Luma { .. } => tr(k::trn::kind::LUMA),
        TransitionKind::Push { .. } => tr(k::trn::kind::PUSH),
        TransitionKind::Slide { .. } => tr(k::trn::kind::SLIDE),
        TransitionKind::Zoom { .. } => tr(k::trn::kind::ZOOM),
        TransitionKind::Spin { .. } => tr(k::trn::kind::SPIN),
        TransitionKind::Whip { .. } => tr(k::trn::kind::WHIP),
        TransitionKind::BlurDissolve { .. } => tr(k::trn::kind::BLUR),
        TransitionKind::Pixelate { .. } => tr(k::trn::kind::PIXELATE),
        TransitionKind::Glitch { .. } => tr(k::trn::kind::GLITCH),
        TransitionKind::Flash { .. } => tr(k::trn::kind::FLASH),
        TransitionKind::Dip { .. } => tr(k::trn::kind::DIP),
        TransitionKind::Blinds { .. } => tr(k::trn::kind::BLINDS),
        TransitionKind::Checker { .. } => tr(k::trn::kind::CHECKER),
        TransitionKind::Blocks { .. } => tr(k::trn::kind::BLOCKS),
        TransitionKind::Spiral { .. } => tr(k::trn::kind::SPIRAL),
        TransitionKind::Split { .. } => tr(k::trn::kind::SPLIT),
        TransitionKind::Squeeze { .. } => tr(k::trn::kind::SQUEEZE),
        TransitionKind::Stretch { .. } => tr(k::trn::kind::STRETCH),
        TransitionKind::Swap { .. } => tr(k::trn::kind::SWAP),
        TransitionKind::Shake { .. } => tr(k::trn::kind::SHAKE),
        TransitionKind::FilmBurn { .. } => tr(k::trn::kind::FILM_BURN),
        TransitionKind::LightLeaks { .. } => tr(k::trn::kind::LIGHT_LEAKS),
        TransitionKind::ZoomBlur { .. } => tr(k::trn::kind::ZOOM_BLUR),
        TransitionKind::Flip { .. } => tr(k::trn::kind::FLIP),
        TransitionKind::Cube { .. } => tr(k::trn::kind::CUBE),
        TransitionKind::Door { .. } => tr(k::trn::kind::DOOR),
        TransitionKind::PagePeel { .. } => tr(k::trn::kind::PAGE_PEEL),
        TransitionKind::PageRoll { .. } => tr(k::trn::kind::PAGE_ROLL),
    }
}

fn curve_name(c: AudioCurve) -> &'static str {
    match c {
        AudioCurve::EqualPower => tr(k::trn::curve::EQUAL_POWER),
        AudioCurve::Linear => tr(k::trn::curve::LINEAR),
        AudioCurve::SCurve => tr(k::trn::curve::S_CURVE),
        AudioCurve::Exponential => tr(k::trn::curve::EXPONENTIAL),
    }
}

/// The direction of the kind, if it has one: with its own control (arrows), not a
/// number — a direction is not a number, and it is not expressed by the parameter spec.
fn dir_of(kind: &mut TransitionKind) -> Option<&mut kt_model::Dir> {
    match kind {
        TransitionKind::Wipe { dir, .. }
        | TransitionKind::Push { dir }
        | TransitionKind::Slide { dir }
        | TransitionKind::Whip { dir, .. }
        | TransitionKind::Squeeze { dir }
        | TransitionKind::Cube { dir, .. }
        | TransitionKind::PagePeel { dir, .. }
        | TransitionKind::PageRoll { dir, .. } => Some(dir),
        _ => None,
    }
}

impl KtApp {
    pub fn open_transition_editor(&mut self, id: TransitionId) {
        if let Some(ed) = self.transition_editors.iter_mut().find(|e| e.id == id) {
            ed.open = true;
            return;
        }
        let Some(t) = self.model.project().transitions.iter().find(|t| t.id == id).cloned() else {
            return;
        };
        self.gesture_counter += 1;
        self.transition_editors.push(TransitionEditor {
            id,
            view: Default::default(),
            backup: t,
            gesture: self.gesture_counter,
            open: true,
        });
    }

    pub fn draw_transition_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.transition_editors);
        for ed in &mut editors {
            self.draw_one_transition_editor(ctx, ed);
        }
        editors.retain(|e| e.open);
        self.transition_editors = editors;
    }

    fn draw_one_transition_editor(&mut self, ctx: &egui::Context, ed: &mut TransitionEditor) {
        // the publish document — one clone per window draw (R.59)
        let pub_doc = self.model.document();
        let Some(cur) = self.model.project().transitions.iter().find(|t| t.id == ed.id).cloned()
        else {
            ed.open = false; // the transition died (clips were pulled apart) — the window goes away
            return;
        };
        let mut hover_out: Option<String> = None;
        let mut edited: Option<TransitionEvent> = None;
        let mut revert = false;
        let mut seek: Option<kt_time::Tick> = None;
        let mut axis_req = crate::automation::AxisReq::default();
        let playhead = self.player.transport.playhead();
        let frame_step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        // transition window on the timeline: the curve domain — the tick from its start.
        // For an overlapping (R.51) window — the overlap zone, the offsets are dead
        // weight, so the length is taken from the window itself.
        let win = cur.window(self.model.project());
        let w0 = win.map(|w| w.start).unwrap_or(kt_time::Tick::ZERO);
        let win_len = win.map(|w| (w.end - w.start).0).unwrap_or(1).max(1);
        let mut open = ed.open;
        let is_fade = cur.is_fade();
        // a transition on an AUDIO track: the picture kind is not needed for it at all —
        // the sound is always crossfaded, it has only a curve
        let audio_only = cur
            .a
            .or(cur.b)
            .and_then(|id| self.model.clip(id).map(|c| c.kind == kt_model::ClipKind::Audio))
            .unwrap_or(false);
        let shown = egui::Window::new(crate::tf!(k::trn::TITLE, ed.id.0))
            .id(egui::Id::new(("transition", ed.id)))
            .open(&mut open)
            .default_size([420.0, 260.0])
            .min_size([340.0, 200.0])
            .resizable(true)
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                let mut t = cur.clone();
                let mut changed = false;
                // ---- kind: picker by families --------------------------
                if !audio_only {
                    ui.horizontal(|ui| {
                        ui.label(tr(k::trn::KIND));
                        egui::ComboBox::from_id_salt(("trkind", ed.id))
                            .selected_text(kind_name(&t.kind))
                            .show_ui(ui, |ui| {
                                let mut cat: Option<Key> = None;
                                for k in TransitionKind::ALL {
                                    let c = kind_category(k);
                                    if cat != Some(c) {
                                        if cat.is_some() {
                                            ui.separator();
                                        }
                                        ui.label(tr(c));
                                        cat = Some(c);
                                    }
                                    let same = k.tag() == t.kind.tag();
                                    if ui.selectable_label(same, kind_name(k)).clicked() && !same {
                                        // changing the kind = its DEFAULTS: the parameters differ
                                        // between kinds, there is nothing to carry over
                                        t.kind = k.clone();
                                        changed = true;
                                    }
                                }
                            });
                    });
                    // ---- kind parameters: the same grid as for effects -------
                    let color =
                        matches!(t.kind, TransitionKind::Flash { .. } | TransitionKind::Dip { .. });
                    // in burn and flare the colour is called tint_* — the same eyedropper
                    let tint = matches!(
                        t.kind,
                        TransitionKind::FilmBurn { .. } | TransitionKind::LightLeaks { .. }
                    );
                    let skip: &[&str] = if color {
                        &["color_r", "color_g", "color_b"]
                    } else if tint {
                        &["tint_r", "tint_g", "tint_b"]
                    } else {
                        &[]
                    };
                    if let Some(dir) = dir_of(&mut t.kind) {
                        ui.horizontal(|ui| {
                            ui.label(tr(k::trn::DIR));
                            for (d, label) in [
                                (kt_model::Dir::Left, tr(k::trn::dir::LEFT)),
                                (kt_model::Dir::Right, tr(k::trn::dir::RIGHT)),
                                (kt_model::Dir::Up, tr(k::trn::dir::UP)),
                                (kt_model::Dir::Down, tr(k::trn::dir::DOWN)),
                            ] {
                                if ui.selectable_label(*dir == d, label).clicked() && *dir != d {
                                    *dir = d;
                                    changed = true;
                                }
                            }
                        });
                    }
                    if let TransitionKind::Iris { shape, .. } = &mut t.kind {
                        ui.horizontal(|ui| {
                            ui.label(tr(k::trn::SHAPE));
                            for (s, label) in [
                                (kt_model::IrisShape::Circle, tr(k::trn::shape::CIRCLE)),
                                (kt_model::IrisShape::Diamond, tr(k::trn::shape::DIAMOND)),
                                (kt_model::IrisShape::Box, tr(k::trn::shape::BOX)),
                            ] {
                                if ui.selectable_label(*shape == s, label).clicked() && *shape != s
                                {
                                    *shape = s;
                                    changed = true;
                                }
                            }
                        });
                    }
                    if let TransitionKind::Clock { reverse, .. } = &mut t.kind
                        && ui.checkbox(reverse, tr(k::trn::REVERSE)).changed()
                    {
                        changed = true;
                    }
                    if let TransitionKind::Barn { vertical, .. } = &mut t.kind
                        && ui.checkbox(vertical, tr(k::trn::VERTICAL)).changed()
                    {
                        changed = true;
                    }
                    // the axis for the R.51 symbiotic shifts — one checkbox for all three
                    if let TransitionKind::Split { vertical }
                    | TransitionKind::Stretch { vertical }
                    | TransitionKind::Swap { vertical, .. }
                    | TransitionKind::Flip { vertical, .. }
                    | TransitionKind::Door { vertical, .. } = &mut t.kind
                        && ui.checkbox(vertical, tr(k::trn::VERTICAL_AXIS)).changed()
                    {
                        changed = true;
                    }
                    if let TransitionKind::Luma { invert, .. } = &mut t.kind
                        && ui.checkbox(invert, tr(k::trn::INVERT)).changed()
                    {
                        changed = true;
                    }
                    // transition inversion (R.51, item 398) — a property of the EVENT:
                    // the gesture is played backwards. For symmetric kinds
                    // this is an identity — the checkbox is not shown (rule 16)
                    if !t.kind.symmetric()
                        && ui.checkbox(&mut t.invert, tr(k::trn::INVERSE)).changed()
                    {
                        changed = true;
                    }
                    if color || tint {
                        ui.horizontal(|ui| {
                            ui.label(tr(k::trn::COLOR));
                            let names = if tint {
                                ["tint_r", "tint_g", "tint_b"]
                            } else {
                                ["color_r", "color_g", "color_b"]
                            };
                            let get = |n: &str| t.kind.get_param(n).unwrap_or(0.0) as f32 / 255.0;
                            let mut c = [get(names[0]), get(names[1]), get(names[2])];
                            if ui.color_edit_button_rgb(&mut c).changed() {
                                for (n, v) in names.iter().zip(c) {
                                    t.kind.set_param(n, (v * 255.0).round() as f64);
                                }
                                changed = true;
                            }
                        });
                    }
                    let pubc = self.publish_ctx(
                        &pub_doc,
                        kt_model::ClipId(0),
                        kt_model::ParamOwner::Transition { id: ed.id },
                    );
                    // the GROUP of kind parameters — the sample is “Transform” (R.66)
                    if let Some(ch) = widgets::group(ui, "trn.params", tr(k::grp::PARAMS), |ui| {
                        widgets::params_grid(ui, ("trp", ed.id), &mut t.kind, skip, Some(&pubc))
                    }) {
                        changed |= ch;
                    }

                    // ---- curves of the kind parameters (R.34 K4) --------------
                    if !t.kind.param_specs().is_empty() {
                        ui.horizontal(|ui| {
                            let specs = t.kind.param_specs();
                            let mut add: Option<String> = None;
                            egui::ComboBox::from_id_salt(("trauto", ed.id))
                                .selected_text(tr(k::fx::AUTO_ADD))
                                .show_ui(ui, |ui| {
                                    for s in specs {
                                        if t.auto.iter().any(|c| c.param == s.name) {
                                            continue;
                                        }
                                        if ui.button(widgets::param_label(&s.name)).clicked() {
                                            add = Some(s.name.to_string());
                                        }
                                    }
                                });
                            if let Some(name) = add {
                                let v = t.kind.get_param(&name).unwrap_or(0.0);
                                t.auto.push(kt_model::ParamCurve {
                                    param: name,
                                    keys: vec![kt_model::AutoKey::at(kt_time::Tick::ZERO, v)],
                                    source: Default::default(),
                                });
                                changed = true;
                            }
                        });
                        let tl_view = self.tl_view();
                        let to_tl = move |x: kt_time::Tick| kt_time::Tick(w0.0 + x.0);
                        let to_cur = move |x: kt_time::Tick| kt_time::Tick(x.0 - w0.0);
                        let actx = crate::automation::AutoCtx {
                            links: None,
                            specs: t.kind.param_specs(),
                            range: (kt_time::Tick::ZERO, kt_time::Tick(win_len)),
                            view: crate::automation::view_in_curve(&to_cur, tl_view),
                            to_timeline: &to_tl,
                            to_curve: &to_cur,
                            frame_step,
                            linked: self.auto_axis_linked,
                            default_shape: Default::default(),
                        };
                        let res = crate::automation::draw(
                            ui,
                            ("trauto", ed.id.0),
                            &actx,
                            &mut t.auto,
                            &mut ed.view,
                            playhead,
                            &mut hover_out,
                        );
                        changed |= res.changed;
                        seek = res.seek;
                        // the axis lock is one per programme (R.40 item 15)
                        axis_req = axis_req.merge(&res);
                    }
                }

                // ---- sound ----------------------------------------------
                if !audio_only {
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    ui.label(tr(k::trn::AUDIO));
                    egui::ComboBox::from_id_salt(("trcurve", ed.id))
                        .selected_text(curve_name(t.audio))
                        .show_ui(ui, |ui| {
                            for c in [
                                AudioCurve::EqualPower,
                                AudioCurve::Linear,
                                AudioCurve::SCurve,
                                AudioCurve::Exponential,
                            ] {
                                if ui.selectable_label(t.audio == c, curve_name(c)).clicked()
                                    && t.audio != c
                                {
                                    t.audio = c;
                                    changed = true;
                                }
                            }
                        });
                });
                // the sound of stylised kinds is crossfaded anyway —
                // saying this plainly is cheaper than letting the user
                // guess why “glitch” does not sound like a glitch
                crate::app::hint_into(
                    &mut hover_out,
                    ui.label(tr(k::trn::AUDIO_NOTE)),
                    tr(k::trn::AUDIO_NOTE),
                );

                ui.horizontal(|ui| {
                    // the window length is a value that is dragged with the mouse (for
                    // an overlapping one — by the clip trim); here it is READ,
                    // so as not to guess from pixels
                    let len = win_len as f64 / 90_000.0;
                    ui.label(crate::tf!(k::trn::LENGTH, format!("{len:.2}")));
                    if is_fade {
                        ui.label(tr(k::trn::IS_FADE));
                    }
                });
                if changed {
                    edited = Some(t);
                }
                });
                crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr(k::fx::CANCEL)).clicked() {
                            revert = true;
                        }
                    });
                });
                });
            });
        // the whole window frame — the README screenshots crop by it
        if let Some(r) = &shown {
            crate::widgets::field_probe::note("win.transition", r.response.rect);
        }
        ed.open = open;
        if let Some(t) = edited {
            self.apply_coalesced(Command::SetTransition(t), ed.gesture);
        }
        if revert {
            self.apply_coalesced(Command::SetTransition(ed.backup.clone()), ed.gesture);
            ed.open = false;
        }
        // the chart's requests about the AXIS are applied in one place (R.40 item 15)
        self.apply_axis_req(axis_req);
        if let Some(t) = seek {
            self.player.seek(t);
        }
        if let Some(h) = hover_out {
            self.hover_hint = Some(h);
        }
    }
}
