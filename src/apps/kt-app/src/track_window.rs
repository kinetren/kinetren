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
    /// Настройки ДАКИНГА (Р.63 п.8): инструмент помнит, что в него ввели
    /// прошлый раз, — они приезжают из `user_store` при открытии окна и
    /// уезжают туда при вызове. «Какого хера я вынужден каждый раз
    /// запоминать какие я параметры ввел в предыдущий раз?» (владелец).
    pub duck: kt_engine::ducking::DuckParams,
    /// Дорожка-ИСТОЧНИК речи (та, ПОД которую приглушаем фон этой).
    /// Она про ЭТОТ проект, поэтому в общем хранилище ей не место.
    pub duck_src: Option<u32>,
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
        let duck = self.workspace.duck.unwrap_or_default();
        self.track_win = Some(TrackWindow {
            duck,
            duck_src: None,
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
        let name_id = egui::Id::new(("track_name", w.id.0));
        let mut open = w.open;
        let mut cancel = false;
        let mut props = track.props();
        let (mut fx, mut bypass) = (track.fx.clone(), track.fx_bypass.clone());
        let mut fx_off = track.fx_off;
        let mut changed_props = false;
        // вставка цепочки может привезти огибающие; у дорожки их некуда
        // положить — говорим об этом ВСЛУХ (правило 16)
        let mut curves_dropped = false;
        let mut changed_fx = false;
        let mut hover: Option<String> = None;
        // «Дакинг» жмут внутри окна, а считать его — дело `&mut self`:
        // намерение собирается здесь и исполняется после отрисовки
        let mut duck_go = false;
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
                });
                // РЕЖИМ НАЛОЖЕНИЯ И ВЫКЛЮЧАТЕЛИ — СВОИМИ строками, а не
                // ячейками сетки. Девять кнопок режима переносятся на
                // вторую строку, а `Grid` высоту ПЕРЕНОСА не меряет: в
                // живом окне вторая строка кнопок наезжала на
                // «Выключатели» (правило 7, поймано сценарием track.py).
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
                    // КРИВАЯ НА ДОРОЖКЕ — ОТКАЗ С ПРИЧИНОЙ, а не молчание.
                    // Полоса общая с событием (правило 17), и вставить в
                    // неё цепочку с автоматизацией человек может в любой
                    // момент. Уроним её молча — он увидит «эффект тот же,
                    // а звучит иначе» и будет искать дефект в движке.
                    if auto.iter().any(|c| !c.is_empty()) {
                        curves_dropped = true;
                        auto.clear();
                    }
                }
                ui.label(
                    egui::RichText::new(tr(k::track::NO_CURVES_HINT)).weak().small(),
                );
                // ДАКИНГ — ЗДЕСЬ, а не своим окном (Р.63 п.8). Владелец:
                // «очень уёбский способ управления. это банально можно
                // удобнее сделать через fx-аудио на весь трек». Фон — та
                // дорожка, чьё окно открыто; речь выбирается рядом.
                if track.kind == LaneKind::Audio {
                    ui.separator();
                    ui.label(egui::RichText::new(tr(k::msg::DUCK_TITLE)).strong());
                    let lanes: Vec<u32> = {
                        let mut v: Vec<u32> = self
                            .model
                            .project()
                            .clips
                            .iter()
                            .filter(|c| c.kind == kt_model::ClipKind::Audio && c.lane != lane)
                            .map(|c| c.lane)
                            .collect();
                        v.sort_unstable();
                        v.dedup();
                        v
                    };
                    let mut src = w.duck_src.or_else(|| lanes.first().copied());
                    ui.horizontal(|ui| {
                        ui.label(tr(k::msg::DUCK_SOURCE_LANE));
                        let cb = egui::ComboBox::from_id_salt(("duck_lane", w.id.0))
                            .selected_text(match src {
                                Some(l) => format!("{}", l + 1),
                                None => "—".to_string(),
                            })
                            .show_ui(ui, |ui| {
                                for l in &lanes {
                                    let mut cur = src;
                                    if ui
                                        .selectable_value(&mut cur, Some(*l), format!("{}", l + 1))
                                        .clicked()
                                    {
                                        src = cur;
                                    }
                                }
                            });
                        crate::widgets::field_probe::note("duck.src", cb.response.rect);
                    });
                    w.duck_src = src;
                    egui::Grid::new(("duck_params", w.id.0)).num_columns(2).show(ui, |ui| {
                        let row = |ui: &mut egui::Ui,
                                       probe: &str,
                                       label: &str,
                                       v: &mut f64,
                                       lo: f64,
                                       hi: f64,
                                       step: f64,
                                       suffix: &str| {
                            ui.label(label);
                            let r = ui.add(
                                egui::DragValue::new(v)
                                    .range(lo..=hi)
                                    .speed(step)
                                    .suffix(suffix),
                            );
                            crate::widgets::field_probe::note(probe, r.rect);
                            ui.end_row();
                        };
                        row(
                            ui,
                            "duck.threshold",
                            tr(k::fxp::THRESHOLD_DB),
                            &mut w.duck.threshold_db,
                            -60.0,
                            0.0,
                            0.5,
                            " dB",
                        );
                        row(
                            ui,
                            "duck.depth",
                            tr(k::msg::DUCK_DEPTH),
                            &mut w.duck.depth_db,
                            0.0,
                            24.0,
                            0.5,
                            " dB",
                        );
                        row(
                            ui,
                            "duck.attack",
                            tr(k::fxp::ATTACK_MS),
                            &mut w.duck.attack_ms,
                            10.0,
                            1000.0,
                            5.0,
                            " ms",
                        );
                        row(
                            ui,
                            "duck.release",
                            tr(k::fxp::RELEASE_MS),
                            &mut w.duck.release_ms,
                            10.0,
                            2000.0,
                            10.0,
                            " ms",
                        );
                    });
                    // правило 19: сказать, что это СНИМОК
                    ui.label(egui::RichText::new(tr(k::msg::DUCK_NOTE)).weak().small());
                    let r = ui.add_enabled(src.is_some(), egui::Button::new(tr(k::msg::DUCK_DO)));
                    crate::widgets::field_probe::note("duck.do", r.rect);
                    crate::widgets::hint(&r, tr(k::msg::DUCK_NOTE));
                    if r.clicked() {
                        duck_go = true;
                    }
                }
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
        // Esc закрывает окно КАСКАДОМ (`escape_action`), правки оставляя,
        // — как у окна FX: они применяются на лету, а вернуть их целиком
        // умеет «Отмена». Своего обработчика окну не нужно: фокус в поле
        // имени Esc снимает сам egui в начале кадра, поэтому до каскада
        // клавиша доходит тем же нажатием.
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
        if duck_go && let Some(src) = w.duck_src {
            // ЦЕЛИ — события ЭТОЙ дорожки: окно про неё и открыто, и
            // «что приглушаем» спрашивать второй раз незачем
            let targets: Vec<kt_model::ClipId> = self
                .model
                .project()
                .clips
                .iter()
                .filter(|c| c.kind == kt_model::ClipKind::Audio && c.lane == lane)
                .map(|c| c.id)
                .collect();
            if targets.is_empty() {
                self.refuse(tr(k::msg::DUCK_NOTHING));
            } else {
                // ИНСТРУМЕНТ ПОМНИТ, что в него ввели (Р.63 п.8)
                self.workspace.duck = Some(w.duck);
                crate::user_store::save_workspace(&self.workspace);
                let d = crate::app::DuckDialog {
                    targets,
                    src_lane: src,
                    params: w.duck,
                };
                self.start_ducking(&d);
            }
        }
        if open && w.open {
            self.track_win = Some(w);
        } else {
            // всё, что берёт фокус, обязано его ОТДАВАТЬ (правило 14)
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
