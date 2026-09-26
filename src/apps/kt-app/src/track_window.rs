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
    /// Снимок цепочки дорожки-КЛЮЧА на момент открытия: её компрессор
    /// правится из этого же окна, значит «Отмена» обязана вернуть и его.
    pub key_backup: Option<(TrackId, Vec<kt_model::FxInstance>, Vec<bool>)>,
    pub selected: crate::widgets::ChainSel,
    pub focus: bool,
    pub open: bool,
}

/// Компрессор ли это — вопрос, который в форме дакинга задаётся трижды.
fn is_comp(f: &kt_model::FxInstance) -> bool {
    matches!(f, kt_model::FxInstance::Compressor { .. })
}

/// Копия первого компрессора цепочки с другим ключом — нужна, чтобы
/// СПРОСИТЬ модель про цикл, ничего ещё не меняя.
fn comp_with_key(fx: &[kt_model::FxInstance], key: Option<TrackId>) -> kt_model::FxInstance {
    let mut c = fx.iter().find(|f| is_comp(f)).cloned().unwrap_or_else(kt_model::FxInstance::compressor);
    if let kt_model::FxInstance::Compressor { key: k, .. } = &mut c {
        *k = key;
    }
    c
}

/// Назначить ключ ПЕРВОМУ компрессору цепочки; его нет — завести (человек
/// выбрал ключ, значит прибор ему нужен, и искать его в каталоге он не
/// обязан). Снятие ключа прибор НЕ удаляет: правило 20 — ничего не
/// отрубается само.
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
    /// Открыть окно дорожки. Второго окна на ту же дорожку не заводим —
    /// как у окна FX (оно ведёт всю link-группу одной сессией).
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
        // правки компрессора дорожки-КЛЮЧА собираются здесь и уезжают
        // после отрисовки — той же сессией окна и тем же шагом истории
        let mut key_edit: Option<(TrackId, Vec<kt_model::FxInstance>, Vec<bool>)> = None;
        let title = crate::tf!(k::track::TITLE, track.label(lane));
        egui::Window::new(title)
            .id(egui::Id::new(("track_window", w.id.0)))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                // ТЕЛО — В ПРОКРУТКУ, кнопки НИЖЕ неё (правило 1: попасть
                // мышью с первого раза). С двумя компрессорами дакинга окно
                // переросло экран, и «Закрыть» с «Отменой» уехали за кромку —
                // поймано t775.
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

                    // УРОВЕНЬ — по виду и с пределами ИЗ ПАСПОРТА (Р.62):
                    // своих литералов у окна нет, иначе они разъедутся с
                    // моделью (прецедент Gain в Р.33)
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
                // ДАКИНГ — ЭТО ДВА КОМПРЕССОРА (Р.65). Владелец: «в
                // качестве дакинга можно показать тот же задублированный
                // виджет аудио-компрессора… т.е. компрессор на фоновом
                // объекте и компрессор на переднем объекте». Слева —
                // прибор ЭТОЙ дорожки (фон) с выбором ключа, справа —
                // настоящий компрессор дорожки-КЛЮЧА, приглушённый.
                if track.kind == LaneKind::Audio {
                    ui.separator();
                    ui.label(egui::RichText::new(tr(k::msg::DUCK_TITLE)).strong());
                    // КЛЮЧ: аудиодорожки своей ленты, кроме себя и тех,
                    // что дали бы цикл (модель их всё равно отвергнет —
                    // правило 16: не показывать невозможного молча)
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
                    // ключ выбран — прибор обязан существовать: заводим
                    // его ОДНИМ действием, а не просим человека найти
                    // компрессор в каталоге
                    if changed_fx {
                        set_key(&mut fx, &mut bypass, cur_key);
                    }
                    // ДВА КОМПРЕССОРА РЯДОМ, обоими правит один виджет
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
                        // КОМПРЕССОР РЕЧИ — настоящий, из цепочки ключа
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
                            // приглушённо — но ЖИВОЕ: владелец выбрал
                            // «настоящий компрессор речи», а не показ
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
            if let Some((kid, kfx, kby)) = w.key_backup.clone() {
                self.apply_coalesced(
                    Command::SetTrackFx { id: kid, fx: kfx, bypass: kby },
                    w.gesture,
                );
            }
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
        if let Some((kid, kfx, kby)) = key_edit.clone() {
            // компрессор речи — ТА ЖЕ сессия окна и ТОТ ЖЕ шаг истории
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
