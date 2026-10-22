//! TRACK WINDOW (R.62, change B): properties and the chain in one place.
//!
//! Why one window and not two: a track has exactly six things to edit —
//! name, level, blend mode, three switches — and the chain. To split
//! them over two windows would mean making the person open a second one for a neighbouring
//! field (rule 13 — space is dear, and so is attention).
//!
//! The chain strip is THE SAME `widgets::chain_strip` as for an event and
//! for the morpher (rule 17): if they diverged, an effect on a track would behave
//! differently from one on an event. There is deliberately no automation widget here:
//! a track has no curves (its key is the timeline tick — that is the next change),
//! and an attempt to set a key is rejected with a reason, not silently.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::{BlendMode, Command, HasParams, LaneKind, TrackId, TrackProps};

pub struct TrackWindow {
    pub id: TrackId,
    /// a snapshot at opening: “Cancel” rolls back the WHOLE session.
    pub backup: TrackProps,
    pub fx_backup: (Vec<kt_model::FxInstance>, Vec<bool>, bool),
    /// gesture token: the whole session of the window is ONE history step (as for the FX window).
    pub gesture: u64,
    pub picker_query: String,
    /// a snapshot of the KEY track’s chain at the moment of opening: its compressor
    /// is edited from this same window, so “Cancel” must restore it too.
    pub key_backup: Option<(TrackId, Vec<kt_model::FxInstance>, Vec<bool>)>,
    pub selected: crate::widgets::ChainSel,
    pub focus: bool,
    pub open: bool,
}

/// whether it is a compressor is a question asked three times in the ducking form.
fn is_comp(f: &kt_model::FxInstance) -> bool {
    matches!(f, kt_model::FxInstance::Compressor { .. })
}

/// a copy of the chain’s first compressor with another key — needed to
/// ASK the model about a cycle without changing anything yet.
fn comp_with_key(fx: &[kt_model::FxInstance], key: Option<TrackId>) -> kt_model::FxInstance {
    let mut c = fx.iter().find(|f| is_comp(f)).cloned().unwrap_or_else(kt_model::FxInstance::compressor);
    if let kt_model::FxInstance::Compressor { key: k, .. } = &mut c {
        *k = key;
    }
    c
}

/// Assign the key to the FIRST compressor of the chain; if there is none — create one (the person
/// chose a key, so the device is needed for it, and it is not
/// obliged to search for it in the catalogue). Removing the key does NOT delete the device: rule 20 —
/// nothing is switched off by itself.
fn set_key(fx: &mut Vec<kt_model::FxInstance>, bypass: &mut Vec<bool>, key: Option<TrackId>) {
    if let Some(i) = fx.iter().position(is_comp) {
        if let kt_model::FxInstance::Compressor { key: k, .. } = &mut fx[i] {
            *k = key;
        }
        return;
    }
    if key.is_some() {
        let mut c = kt_model::FxInstance::compressor();
        if let kt_model::FxInstance::Compressor { key: k, .. } = &mut c {
            *k = key;
        }
        fx.push(c);
        bypass.push(false);
    }
}

impl KtApp {
    /// Open the track window. We do not make a second window for the same track —
    /// as with the FX window (it drives the whole link group in one session).
    pub fn open_track_window(&mut self, id: TrackId) {
        if self.track_win.as_ref().is_some_and(|w| w.id == id) {
            return;
        }
        let Some(t) = self.model.project().track(id) else { return };
        self.track_win = Some(TrackWindow {
            key_backup: None,
            id,
            backup: t.props(),
            fx_backup: (t.fx.clone(), t.fx_bypass.clone(), t.fx_off),
            gesture: self.gesture_counter,
            picker_query: String::new(),
            selected: crate::widgets::ChainSel::Fx(0),
            focus: true,
            open: true,
        });
    }

    /// The FX badge opens the same window — rule 17 (paired paths lead to
    /// one body).
    pub fn open_track_fx(&mut self, id: TrackId) {
        self.open_track_window(id);
    }

    pub(crate) fn draw_track_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.track_win.take() else { return };
        let Some(track) = self.model.project().track(w.id).cloned() else { return };
        let lane = self
            .model
            .project()
            .rows()
            .into_iter()
            .filter(|(k, _)| *k == track.kind)
            .map(|(_, l)| l)
            .find(|l| {
                self.model.project().track_at(track.kind, *l).map(|t| t.id) == Some(w.id)
            })
            .unwrap_or(0);
        let name_id = egui::Id::new(("track_name", w.id.0));
        let mut open = w.open;
        let mut cancel = false;
        let mut props = track.props();
        let (mut fx, mut bypass) = (track.fx.clone(), track.fx_bypass.clone());
        let mut fx_off = track.fx_off;
        let mut changed_props = false;
        // pasting a chain may bring envelopes; a track has nowhere to put them —
        // we say so ALOUD (rule 16)
        let mut curves_dropped = false;
        let mut changed_fx = false;
        let mut hover: Option<String> = None;
        // edits of the KEY track’s compressor are collected here and sent
        // after drawing — in the same window session and the same history step
        let mut key_edit: Option<(TrackId, Vec<kt_model::FxInstance>, Vec<bool>)> = None;
        let title = crate::tf!(k::track::TITLE, track.label(lane));
        egui::Window::new(title)
            .id(egui::Id::new(("track_window", w.id.0)))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                // THE BODY — IN A SCROLL, the buttons BELOW it (rule 1: to hit
                // with a mouse on the first try). With two ducking compressors the window
                // outgrew the screen, and “Close” with “Cancel” went past the edge —
                // caught by t775.
                let max_h = (ctx.content_rect().height() * 0.62).max(240.0);
                egui::ScrollArea::vertical()
                    .max_height(max_h)
                    .auto_shrink([true, true])
                    .show(ui, |ui| {
                egui::Grid::new("track_props").num_columns(2).show(ui, |ui| {
                    ui.label(tr(k::track::NAME));
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut props.name)
                            .id(egui::Id::new(("track_name", w.id.0)))
                            .desired_width(160.0),
                    );
                    crate::widgets::field_probe::note("track.name", r.rect);
                    if w.focus {
                        r.request_focus();
                        w.focus = false;
                    }
                    changed_props |= r.changed();
                    ui.end_row();

                    // LEVEL — by kind and with limits FROM THE PASSPORT (R.62):
                    // the window has no literals of its own, otherwise they would diverge from the
                    // model (the Gain precedent in R.33)
                    if let Some(spec) = track.param_spec("gain_db") {
                        ui.label(tr(k::track::LEVEL));
                        changed_props |= crate::widgets::plain_row(
                            ui,
                            &crate::widgets::NumRow {
                                label: "",
                                probe: "track.level",
                                unit: spec.unit,
                                min: spec.min,
                                max: spec.max,
                                step: spec.step,
                                slider: None,
                            },
                            &mut props.gain_db,
                        )
                        .changed();
                        ui.end_row();
                    }
                    if let Some(spec) = track.param_spec("opacity") {
                        ui.label(tr(k::track::OPACITY));
                        changed_props |= crate::widgets::plain_row(
                            ui,
                            &crate::widgets::NumRow {
                                label: "",
                                probe: "track.opacity",
                                unit: spec.unit,
                                min: spec.min,
                                max: spec.max,
                                step: spec.step,
                                slider: None,
                            },
                            &mut props.opacity,
                        )
                        .changed();
                        ui.end_row();
                    }
                });
                // BLEND MODE AND SWITCHES — with THEIR OWN rows, not
                // grid cells. The nine blend-mode buttons wrap onto a
                // second row, and `Grid` does not measure the height of the WRAP: in the
                // live window the second row of buttons ran into
                // “Switches” (rule 7, caught by the track.py scenario).
                if track.kind == LaneKind::Video {
                    ui.label(tr(k::track::BLEND));
                    ui.horizontal_wrapped(|ui| {
                        for m in BlendMode::ALL {
                            let r = ui.selectable_value(&mut props.blend, m, blend_label(m));
                            crate::widgets::field_probe::note(
                                &format!("track.blend.{}", m.tag()),
                                r.rect,
                            );
                            changed_props |= r.changed();
                        }
                    });
                }
                ui.label(tr(k::track::SWITCHES));
                ui.horizontal(|ui| {
                    if track.kind.has_switches() {
                        let r = ui.checkbox(&mut props.mute, tr(k::track::MUTE));
                        crate::widgets::field_probe::note("track.mute", r.rect);
                        changed_props |= r.changed();
                        let r = ui.checkbox(&mut props.solo, tr(k::track::SOLO));
                        crate::widgets::field_probe::note("track.solo", r.rect);
                        changed_props |= r.changed();
                    }
                    let r = ui.checkbox(&mut props.lock, tr(k::track::LOCK));
                    crate::widgets::field_probe::note("track.lock", r.rect);
                    changed_props |= r.changed();
                });
                // THE CHAIN — the same strip as for an event (rule 17).
                // Service rows are not given it: they have no effects.
                if track.kind.has_switches() {
                    ui.separator();
                    let mut auto: Vec<Vec<kt_model::ParamCurve>> = Vec::new();
                    let mut preset_save = None;
                    let mut preset_action = None;
                    let mut picked = None;
                    let audio = track.kind == LaneKind::Audio;
                    let mut chain_on = !fx_off;
                    // presets — only those legitimate for this kind of chain
                    let rows: Vec<crate::user_store::PresetRow> = self
                        .presets
                        .rows()
                        .iter()
                        .filter(|r| if audio { r.audio_ok } else { r.video_ok })
                        .cloned()
                        .collect();
                    let ch = crate::widgets::chain_strip(
                        ui,
                        ("track", w.id.0),
                        crate::widgets::ChainEdit {
                            fx: &mut fx,
                            bypass: &mut bypass,
                            auto: &mut auto,
                            selected: &mut w.selected,
                            master_on: &mut chain_on,
                            head: None,
                            presets: crate::widgets::ChainPresets {
                                rows: &rows,
                                save: &mut preset_save,
                                action: &mut preset_action,
                            },
                        },
                        audio,
                        &mut w.picker_query,
                        &mut self.fx_clipboard,
                        kt_time::Tick::ZERO,
                        &mut hover,
                        &mut crate::widgets::ChainCtx {
                            // the strip addresses the chain by its id; a track has no event,
                            // and “zero” here means “not a clip”
                            chain: kt_model::ClipId(0),
                            picked: &mut picked,
                        },
                        None,
                    );
                    changed_fx |= ch.any();
                    fx_off = !chain_on;
                    // A CURVE ON A TRACK — A REFUSAL WITH A REASON, not silence.
                    // The strip is shared with the event (rule 17), and the person can at any moment
                    // insert into it a chain with automation. If we dropped it silently, he would see
                    // “the effect is the same but sounds different” and would look for a defect in the engine.
                    // @@@ C006734 lines=6
                    if auto.iter().any(|c| !c.is_empty()) {
                        curves_dropped = true;
                        auto.clear();
                    }
                }
                ui.label(
                    egui::RichText::new(tr(k::track::NO_CURVES_HINT)).weak().small(),
                );
                // DUCKING IS TWO COMPRESSORS (R.65). The owner: “as ducking one can show the same
                // duplicated audio compressor widget… i.e. a compressor on the background
                // object and a compressor on the foreground object”. On the left —
                // the device of THIS track (background) with the key choice;
                // on the right — the real compressor of the KEY track,
                // muted.
                if track.kind == LaneKind::Audio {
                    ui.separator();
                    ui.label(egui::RichText::new(tr(k::msg::DUCK_TITLE)).strong());
                    // KEY: the audio tracks of its own reel, except itself and those
                    // that would make a cycle (the model would reject them anyway —
                    // rule 16: do not silently show the impossible)
                    let p = self.model.project();
                    let mut cur_key = kt_model::sidechain_keys(&fx).next();
                    let offers: Vec<(TrackId, String)> = p
                        .tracks
                        .iter()
                        .enumerate()
                        .filter(|(_, t)| t.kind == LaneKind::Audio && t.id != w.id)
                        .filter(|(_, t)| {
                            let probe = vec![comp_with_key(&fx, Some(t.id))];
                            !kt_model::sidechain_cycle(&p.tracks, w.id, &probe)
                        })
                        .map(|(i, t)| {
                            let lane = p
                                .tracks
                                .iter()
                                .take(i)
                                .filter(|o| o.kind == LaneKind::Audio)
                                .count() as u32;
                            (t.id, t.label(lane))
                        })
                        .collect();
                    let name_of = |id: TrackId| {
                        offers
                            .iter()
                            .find(|(i, _)| *i == id)
                            .map(|(_, n)| n.clone())
                            .unwrap_or_else(|| tr(k::msg::DUCK_KEY_NONE).to_string())
                    };
                    ui.horizontal(|ui| {
                        ui.label(tr(k::msg::DUCK_KEY));
                        let cb = egui::ComboBox::from_id_salt(("duck_key", w.id.0))
                            .selected_text(match cur_key {
                                Some(id) => name_of(id),
                                None => tr(k::msg::DUCK_KEY_NONE).to_string(),
                            })
                            .show_ui(ui, |ui| {
                                let mut pick = cur_key;
                                if ui
                                    .selectable_value(&mut pick, None, tr(k::msg::DUCK_KEY_NONE))
                                    .clicked()
                                {
                                    cur_key = None;
                                    changed_fx = true;
                                }
                                for (id, name) in &offers {
                                    if ui.selectable_value(&mut pick, Some(*id), name).clicked() {
                                        cur_key = Some(*id);
                                        changed_fx = true;
                                    }
                                }
                            });
                        crate::widgets::field_probe::note("duck.key", cb.response.rect);
                        crate::app::hint_into(
                            &mut hover,
                            cb.response,
                            tr(k::msg::DUCK_KEY_HINT),
                        );
                    });
                    // a key is chosen — the device must exist: it is created with ONE
                    // action, rather than asking the person to find
                    // a compressor in the catalogue
                    if changed_fx {
                        set_key(&mut fx, &mut bypass, cur_key);
                    }
                    // TWO COMPRESSORS SIDE BY SIDE, one widget edits both
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(tr(k::msg::DUCK_BG)).small());
                            match fx.iter_mut().position(|f| is_comp(f)) {
                                Some(i) => {
                                    if crate::widgets::params_grid(
                                        ui,
                                        ("duck_bg", w.id.0),
                                        &mut fx[i],
                                        &[],
                                        None,
                                    ) {
                                        changed_fx = true;
                                    }
                                }
                                None => {
                                    ui.label(
                                        egui::RichText::new(tr(k::msg::DUCK_PICK_KEY))
                                            .weak()
                                            .small(),
                                    );
                                }
                            }
                        });
                        ui.separator();
                        // SPEECH COMPRESSOR — the real one, from the key's chain
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(tr(k::msg::DUCK_KEY_SIDE)).small());
                            let Some(key_id) = cur_key else {
                                ui.label(
                                    egui::RichText::new(tr(k::msg::DUCK_PICK_KEY))
                                        .weak()
                                        .small(),
                                );
                                return;
                            };
                            let Some(kt) = self.model.project().track(key_id).cloned() else {
                                return;
                            };
                            let (mut kfx, mut kby) = (kt.fx.clone(), kt.fx_bypass.clone());
                            // muted — but LIVE: the owner chose
                            // “the real speech compressor”, not a demonstration
                            ui.scope(|ui| {
                                let v = &mut ui.style_mut().visuals;
                                v.override_text_color = Some(v.weak_text_color());
                                match kfx.iter_mut().position(|f| is_comp(f)) {
                                    Some(i) => {
                                        if crate::widgets::params_grid(
                                            ui,
                                            ("duck_key_fx", key_id.0),
                                            &mut kfx[i],
                                            &[],
                                            None,
                                        ) {
                                            key_edit =
                                                Some((key_id, kfx.clone(), kby.clone()));
                                        }
                                    }
                                    None => {
                                        let r = ui.button(tr(k::msg::DUCK_ADD_COMP));
                                        crate::widgets::field_probe::note("duck.add", r.rect);
                                        if r.clicked() {
                                            kfx.push(kt_model::FxInstance::compressor());
                                            kby.push(false);
                                            key_edit =
                                                Some((key_id, kfx.clone(), kby.clone()));
                                        }
                                    }
                                }
                            });
                        });
                    });
                }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    let r = ui.button(tr(k::track::CLOSE));
                    crate::widgets::field_probe::note("track.close", r.rect);
                    if r.clicked() {
                        w.open = false;
                    }
                    let r = ui.button(tr(k::track::CANCEL));
                    crate::widgets::field_probe::note("track.cancel", r.rect);
                    if r.clicked() {
                        cancel = true;
                    }
                });
            });
        // Esc closes the window by CASCADE (`escape_action`), keeping the edits —
        // as with the FX window: they apply on the fly, and “Cancel” can roll them back as a whole.
        // The window needs no handler of its own: with focus in the
        // name field, egui itself releases Esc at the start of the frame, so before the cascade
        // the key reaches it with the same press.
        if curves_dropped {
            self.refuse(tr(k::track::CURVES_DROPPED));
        }
        if let Some(h) = hover {
            self.hover_hint = Some(h);
        }
        if cancel {
            let b = w.backup.clone();
            let (bfx, bby, boff) = w.fx_backup.clone();
            self.apply_coalesced(Command::SetTrackProps { id: w.id, props: b }, w.gesture);
            self.apply_coalesced(
                Command::SetTrackFx { id: w.id, fx: bfx, bypass: bby },
                w.gesture,
            );
            self.apply_coalesced(Command::SetTrackFxOff { id: w.id, off: boff }, w.gesture);
            if let Some((kid, kfx, kby)) = w.key_backup.clone() {
                self.apply_coalesced(
                    Command::SetTrackFx { id: kid, fx: kfx, bypass: kby },
                    w.gesture,
                );
            }
            open = false;
        } else {
            // the WHOLE SESSION OF THE WINDOW — ONE history step (a shared gesture token)
            if changed_props && props != track.props() {
                self.apply_coalesced(Command::SetTrackProps { id: w.id, props }, w.gesture);
            }
            if changed_fx {
                if fx != track.fx || bypass != track.fx_bypass {
                    self.apply_coalesced(
                        Command::SetTrackFx { id: w.id, fx, bypass },
                        w.gesture,
                    );
                }
                if fx_off != track.fx_off {
                    self.apply_coalesced(
                        Command::SetTrackFxOff { id: w.id, off: fx_off },
                        w.gesture,
                    );
                }
            }
        }
        if let Some((kid, kfx, kby)) = key_edit.clone() {
            // the speech compressor — the SAME window session and the SAME history step
            if w.key_backup.is_none()
                && let Some(kt) = self.model.project().track(kid)
            {
                w.key_backup = Some((kid, kt.fx.clone(), kt.fx_bypass.clone()));
            }
            self.apply_coalesced(
                Command::SetTrackFx { id: kid, fx: kfx, bypass: kby },
                w.gesture,
            );
        }
        if open && w.open {
            self.track_win = Some(w);
        } else {
            // anything that takes focus must RELEASE it (rule 14)
            ctx.memory_mut(|m| m.surrender_focus(name_id));
        }
    }
}

pub fn blend_label(m: BlendMode) -> &'static str {
    match m {
        BlendMode::Normal => tr(k::track::BLEND_NORMAL),
        BlendMode::Multiply => tr(k::track::BLEND_MULTIPLY),
        BlendMode::Screen => tr(k::track::BLEND_SCREEN),
        BlendMode::Add => tr(k::track::BLEND_ADD),
        BlendMode::Subtract => tr(k::track::BLEND_SUBTRACT),
        BlendMode::Darken => tr(k::track::BLEND_DARKEN),
        BlendMode::Lighten => tr(k::track::BLEND_LIGHTEN),
        BlendMode::Overlay => tr(k::track::BLEND_OVERLAY),
        BlendMode::Difference => tr(k::track::BLEND_DIFFERENCE),
    }
}
