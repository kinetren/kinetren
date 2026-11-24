//! Transition window (R.33 C4): the kind, its parameters, the audio curve.
//!
//! Until this round a transition had NOT A SINGLE window: the kind existed in
//! one variant, and the offsets were dragged with the mouse. The user’s claim —
//! “on any splices the transition is always a fade… these must be unified like an fx-chain”,
//! so the window is built like the FX editor: a kind picker by category
//! (the registry `TransitionKind::all()`), parameters — the same
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
    /// When the window opened: the kind cells' misclick guard counts from it (kind-picker-previews).
    pub opened_at: std::time::Instant,
}

/// the kind's category for grouping in the picker. The families are the very three
/// agreed with the user (wipes, shifts, stylised).
pub fn kind_category(kind: &TransitionKind) -> Key {
    match kind {
        TransitionKind::Crossfade | TransitionKind::Missing(_) => k::trn::cat::BASIC,
        // a plugin kind's category is its manifest's (plugin-api-stage1, D12)
        TransitionKind::Plugin(p) => crate::i18n::key_by_name(p.manifest().ui.category_key).unwrap_or(k::trn::cat::BASIC),
    }
}

/// Localised name of the kind. Full paths — test 125 looks up the key by them.
pub fn kind_name(kind: &TransitionKind) -> &'static str {
    match kind {
        TransitionKind::Crossfade | TransitionKind::Missing(_) => tr(k::trn::kind::CROSSFADE),
        // the TRANSLATED name the manifest names (D12), its English name as the fallback
        TransitionKind::Plugin(p) => {
            let ui = &p.manifest().ui;
            crate::i18n::key_by_name(ui.name_key).map_or(ui.name, tr)
        }
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
            opened_at: std::time::Instant::now(),
        });
    }

    pub fn draw_transition_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.transition_editors);
        for ed in &mut editors {
            self.draw_one_transition_editor(ctx, ed);
        }
        // a window closed: its queued kind cells are dropped, and with the last cell window its sources
        if editors.iter().any(|e| !e.open) {
            self.fx_thumbs.bump_generation();
            if editors.iter().all(|e| !e.open) && self.fx_editors.is_empty() && self.morph_editors.is_empty() {
                self.fx_thumbs.forget_scene();
            }
        }
        editors.retain(|e| e.open);
        self.transition_editors = editors;
    }

    fn draw_one_transition_editor(&mut self, ctx: &egui::Context, ed: &mut TransitionEditor) {
        // the publish document — one clone per window draw (R.59)
        let pub_doc = self.model.doc_arc();
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
        // The kind cells (kind-picker-previews, `v26.10` G6): the frame after the click — the cut
        // composed with that kind at the MIDDLE of the window, where both sides show (at an edge every
        // kind is the same frame A or B), on the project the player plays. Nothing is opened before
        // the misclick guard; the cells are Loading until then.
        let scene = cur.a.or(cur.b).map(|side| crate::fx_thumbs::Scene {
            project: self.player.project(),
            rev: self.scene_rev(),
            place: crate::fx_thumbs::Place::Clip(side),
            t: kt_time::Tick(w0.0 + win_len / 2),
            base: self.project_path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf()),
            index_db: self.cache_folder(),
        });
        let decode_ok = crate::fx_thumbs::decode_orders_allowed(ed.opened_at.elapsed());
        let mut pv = scene.map(|sc| crate::fx_thumbs::PickerCtx::for_scene(&self.fx_thumbs, sc, decode_ok));
        let host_w_cell = std::cell::Cell::new(0.0f32);
        let shown = crate::window_shell::Shell::new(
            egui::Id::new(("transition", ed.id)),
            crate::tf!(k::trn::TITLE, ed.id.0),
            crate::window_shell::Kind::Tool,
        )
            .width(440.0, [340.0, 120.0])
            .remember("transition")
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                let mut t = cur.clone();
                let mut changed = false;
                // ---- kind: picker by families --------------------------
                if !audio_only {
                    ui.horizontal(|ui| {
                        ui.label(tr(k::trn::KIND));
                        host_w_cell.set(ui.clip_rect().width());
                        let kind = egui::ComboBox::from_id_salt(("trkind", ed.id))
                            .selected_text(kind_name(&t.kind))
                            .height(crate::widgets::popup_list_height(ui))
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show_ui(ui, |ui| {
                                // a gallery of cells by family, as the effects picker (G6)
                                crate::fx_thumbs::gallery_width(ui, host_w_cell.get());
                                let all = TransitionKind::all();
                                let mut i = 0;
                                while i < all.len() {
                                    let c = kind_category(&all[i]);
                                    let n = all[i..].iter().take_while(|k| kind_category(k) == c).count();
                                    ui.label(egui::RichText::new(tr(c)).small().weak());
                                    ui.horizontal_wrapped(|ui| {
                                        for k in &all[i..i + n] {
                                            let same = k.tag() == t.kind.tag();
                                            let swap = crate::fx_thumbs::Swap::Transition(ed.id, k.clone());
                                            let probe = format!("trkind.{}", k.tag());
                                            let r = crate::fx_thumbs::kind_cell(ui, pv.as_mut(), Some(&swap), kind_name(k), same, &probe);
                                            if r.clicked() {
                                                if !same {
                                                    // changing the kind = its DEFAULTS: the parameters differ
                                                    // between kinds, there is nothing to carry over
                                                    t.kind = k.clone();
                                                    changed = true;
                                                }
                                                ui.close();
                                            }
                                        }
                                    });
                                    i += n;
                                }
                            });
                        crate::widgets::field_probe::note("tr.kind.pick", kind.response.rect);
                        crate::widgets::lesson_mark::control(ui, "tr.kind", &kind.response);
                    });
                    // ---- kind parameters: the same grid as for effects -------
                    let color = matches!(&t.kind, TransitionKind::Plugin(p) if p.get("color_r").is_some());
                    // in burn and flare the colour is called tint_* — the same eyedropper
                    let tint = matches!(&t.kind, TransitionKind::Plugin(p) if p.get("tint_r").is_some());
                    let skip: &[&str] = if color {
                        &["color_r", "color_g", "color_b"]
                    } else if tint {
                        &["tint_r", "tint_g", "tint_b"]
                    } else {
                        &[]
                    };
                    // the kind's direction row (D12): arrows, not a number — the row's index
                    // (left, right, up, down — the file keeps the former `Dir` text)
                    if let TransitionKind::Plugin(p) = &mut t.kind
                        && let Some(cur) = p.get("dir")
                    {
                        ui.horizontal(|ui| {
                            ui.label(tr(k::trn::DIR));
                            for (i, label) in [
                                tr(k::trn::dir::LEFT),
                                tr(k::trn::dir::RIGHT),
                                tr(k::trn::dir::UP),
                                tr(k::trn::dir::DOWN),
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                let i = i as f64;
                                if ui.selectable_label(cur == i, label).clicked() && cur != i {
                                    p.set("dir", i);
                                    changed = true;
                                }
                            }
                        });
                    }
                    // the iris shape row (D12): circle, diamond, box — the row's index
                    if let TransitionKind::Plugin(p) = &mut t.kind
                        && let Some(cur) = p.get("shape")
                    {
                        ui.horizontal(|ui| {
                            ui.label(tr(k::trn::SHAPE));
                            for (i, label) in
                                [tr(k::trn::shape::CIRCLE), tr(k::trn::shape::DIAMOND), tr(k::trn::shape::BOX)]
                                    .into_iter()
                                    .enumerate()
                            {
                                let i = i as f64;
                                if ui.selectable_label(cur == i, label).clicked() && cur != i {
                                    p.set("shape", i);
                                    changed = true;
                                }
                            }
                        });
                    }
                    // a plugin kind's flag rows, captioned as the variants' checkboxes were
                    if let TransitionKind::Plugin(p) = &mut t.kind {
                        let barn = p.tag() == "barn";
                        for (row, label) in [
                            ("reverse", tr(k::trn::REVERSE)),
                            ("vertical", if barn { tr(k::trn::VERTICAL) } else { tr(k::trn::VERTICAL_AXIS) }),
                            ("invert", tr(k::trn::INVERT)),
                        ] {
                            if let Some(cur) = p.get(row) {
                                let mut on = cur > 0.5;
                                if ui.checkbox(&mut on, label).changed() {
                                    p.set(row, if on { 1.0 } else { 0.0 });
                                    changed = true;
                                }
                            }
                        }
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
                            if crate::widgets::rgb_button(ui, &mut c, "trn", None) {
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
                                    for s in specs.iter().filter(|s| s.kind == kt_model::ParamKind::Smooth) {
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
        if shown.closed() {
            open = false;
        }
        // the whole window frame — the README screenshots crop by it
        if let Some(r) = &shown.response {
            crate::widgets::field_probe::note("win.transition", r.rect);
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
