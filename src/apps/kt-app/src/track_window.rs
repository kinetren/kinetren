//! Окно ДОРОЖКИ (Р.62, change B): свойства и цепочка в одном месте.
//!
//! Почему одно окно, а не два: у дорожки правится ровно шесть вещей —
//! имя, уровень, режим наложения, три выключателя — и цепочка. Разводить
//! их по двум окнам значило бы заставлять человека открывать второе ради
//! соседнего поля (правило 13 — место дорого, и внимание тоже).
//!
//! Полоса цепочки — ТА ЖЕ `widgets::chain_strip`, что у события и у
//! морфера (правило 17): расходись они, эффект на дорожке вёл бы себя
//! иначе, чем на событии. Виджета автоматизации тут нет намеренно:
//! кривых у дорожки нет (её ключ — тик таймлайна, это следующий change),
//! и попытка поставить ключ отвергается с причиной, а не молчанием.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::{BlendMode, Command, HasParams, LaneKind, TrackId, TrackProps};

pub struct TrackWindow {
    pub id: TrackId,
    /// Снимок на момент открытия: «Отмена» откатывает ВСЮ сессию.
    pub backup: TrackProps,
    pub fx_backup: (Vec<kt_model::FxInstance>, Vec<bool>, bool),
    /// Токен жеста: вся сессия окна — ОДИН шаг истории (как у окна FX).
    pub gesture: u64,
    pub picker_query: String,
    pub selected: crate::widgets::ChainSel,
    pub focus: bool,
    pub open: bool,
}

impl KtApp {
    /// Открыть окно дорожки. Второго окна на ту же дорожку не заводим —
    /// как у окна FX (оно ведёт всю link-группу одной сессией).
    pub fn open_track_window(&mut self, id: TrackId) {
        if self.track_win.as_ref().is_some_and(|w| w.id == id) {
            return;
        }
        let Some(t) = self.model.project().track(id) else { return };
        self.track_win = Some(TrackWindow {
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

    /// Бейдж FX открывает то же окно — правило 17 (парные пути ведут в
    /// одно тело).
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
        let mut open = w.open;
        let mut cancel = false;
        let mut props = track.props();
        let (mut fx, mut bypass) = (track.fx.clone(), track.fx_bypass.clone());
        let mut fx_off = track.fx_off;
        let mut changed_props = false;
        let mut changed_fx = false;
        let mut hover: Option<String> = None;
        let title = crate::tf!(k::track::TITLE, track.label(lane));
        egui::Window::new(title)
            .id(egui::Id::new(("track_window", w.id.0)))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
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

                    // УРОВЕНЬ — по виду и с пределами ИЗ ПАСПОРТА (Р.62):
                    // своих литералов у окна нет, иначе они разъедутся с
                    // моделью (прецедент Gain в Р.33)
                    if let Some(spec) = track.param_spec("gain_db") {
                        ui.label(tr(k::track::LEVEL));
                        let r = ui.add(
                            egui::DragValue::new(&mut props.gain_db)
                                .range(spec.min..=spec.max)
                                .speed(spec.step)
                                .suffix(" dB"),
                        );
                        crate::widgets::field_probe::note("track.level", r.rect);
                        changed_props |= r.changed();
                        ui.end_row();
                    }
                    if let Some(spec) = track.param_spec("opacity") {
                        ui.label(tr(k::track::OPACITY));
                        let r = ui.add(
                            egui::DragValue::new(&mut props.opacity)
                                .range(spec.min..=spec.max)
                                .speed(spec.step),
                        );
                        crate::widgets::field_probe::note("track.level", r.rect);
                        changed_props |= r.changed();
                        ui.end_row();
                    }
                    if track.kind == LaneKind::Video {
                        ui.label(tr(k::track::BLEND));
                        ui.horizontal_wrapped(|ui| {
                            for m in BlendMode::ALL {
                                let r =
                                    ui.selectable_value(&mut props.blend, m, blend_label(m));
                                crate::widgets::field_probe::note(
                                    &format!("track.blend.{}", m.tag()),
                                    r.rect,
                                );
                                changed_props |= r.changed();
                            }
                        });
                        ui.end_row();
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
                    ui.end_row();
                });
                // ЦЕПОЧКА — та же полоса, что у события (правило 17).
                // Служебным строкам её не даём: эффектов у них нет.
                if track.kind.has_switches() {
                    ui.separator();
                    let mut auto: Vec<Vec<kt_model::ParamCurve>> = Vec::new();
                    let mut preset_save = None;
                    let mut preset_action = None;
                    let mut picked = None;
                    let audio = track.kind == LaneKind::Audio;
                    let mut chain_on = !fx_off;
                    // пресеты — только законные для этого вида цепочки
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
                            // полоса адресует цепочку своим id; у дорожки
                            // события нет, и «ноль» здесь значит «не клип»
                            chain: kt_model::ClipId(0),
                            picked: &mut picked,
                        },
                        None,
                    );
                    changed_fx |= ch.any();
                    fx_off = !chain_on;
                }
                ui.label(
                    egui::RichText::new(tr(k::track::NO_CURVES_HINT)).weak().small(),
                );
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
            open = false;
        } else {
            // ВСЯ СЕССИЯ ОКНА — ОДИН шаг истории (общий токен жеста)
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
        if open && w.open {
            self.track_win = Some(w);
        } else {
            // всё, что берёт фокус, обязано его ОТДАВАТЬ (правило 14)
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new(("track_name", w.id.0))));
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
