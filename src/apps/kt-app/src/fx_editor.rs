//! Окно FX-цепочки клипа (порт ClipFxChainWindow + FxChainEditorWidget):
//! полоса карточек «Media → fx → fx», форма параметров, полоса автоматизации
//! (ромбы: клик = выбор, драг = перенос t, double-click = удалить),
//! live-применение через apply_coalesced (вся сессия окна = 1 undo),
//! Отмена = откат к состоянию на момент открытия.

use crate::i18n::{k, tr};
use crate::app::KtApp;
use eframe::egui;
use kt_model::*;

/// Снимок одной цепочки на момент открытия окна: сама цепочка, кривые,
/// выключатели слотов и мастер-выключатель.
pub type ChainSnapshot =
    (ClipId, Vec<FxInstance>, Vec<Vec<kt_model::ParamCurve>>, Vec<bool>, bool);

pub struct FxEditor {
    /// Якорь окна: событие, по которому его открыли (его id — id окна).
    pub clip: ClipId,
    /// Все цепочки, которые показывает окно: якорь и его link-партнёры
    /// (Р.33 п.2 — «для одного и того же события с примагниченными треками
    /// можно и сразу все chain в одном окне давать»).
    pub members: Vec<ClipId>,
    pub picker_query: String,
    /// Состояние КАЖДОЙ цепочки на момент открытия — «Отмена» откатывает
    /// всё окно, а не только якорь.
    pub backup: Vec<ChainSnapshot>,
    pub gesture: u64,
    /// Выбранный слот в каждой цепочке.
    pub selected: std::collections::HashMap<ClipId, usize>,
    /// Состояние виджета автоматизации на каждую (цепочку, слот):
    /// вид, выбранный ключ, незакоммиченный ввод времени.
    pub auto_views: std::collections::HashMap<(ClipId, usize), crate::automation::AutoView>,
    pub open: bool,
}

impl KtApp {
    /// Открыть окно FX. Окно, которое уже показывает эту цепочку (своим
    /// якорем или как партнёра), второй раз не открывается: у A/V-пары
    /// одно окно на обе дорожки.
    pub fn open_fx_editor(&mut self, clip: ClipId) {
        if self.fx_editors.iter().any(|e| e.members.contains(&clip)) {
            return;
        }
        let Some(c) = self.model.clip(clip).cloned() else { return };
        let members: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|m| m.id == clip || (c.link_group.is_some() && m.link_group == c.link_group))
            // якорь первым, дальше по виду и id — порядок стабилен
            .map(|m| (m.id != clip, m.kind == ClipKind::Audio, m.id.0, m.id))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|(_, _, _, id)| id)
            .collect();
        self.gesture_counter += 1;
        let backup = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| (m.id, m.fx.clone(), m.fx_auto.clone(), m.fx_bypass.clone(), m.fx_off))
            .collect();
        let selected = members
            .iter()
            .filter_map(|id| self.model.clip(*id).map(|m| (m.id, m.fx.len().saturating_sub(1))))
            .collect();
        self.fx_editors.push(FxEditor {
            clip,
            members,
            picker_query: String::new(),
            backup,
            gesture: self.gesture_counter,
            selected,
            auto_views: std::collections::HashMap::new(),
            open: true,
        });
    }

    pub(crate) fn set_fx_coalesced(&mut self, clip: ClipId, fx: Vec<FxInstance>, gesture: u64) {
        self.apply_coalesced(Command::SetFx { id: clip, fx }, gesture);
    }

    /// Кривые автоматизации: тот же gesture, что у SetFx — один undo-шаг
    /// на сессию окна.
    pub(crate) fn set_fx_auto_coalesced(
        &mut self,
        clip: ClipId,
        mut auto: Vec<Vec<kt_model::ParamCurve>>,
        gesture: u64,
    ) {
        while auto.last().is_some_and(|v| v.is_empty()) {
            auto.pop(); // хвост без кривых не сериализуется зря
        }
        self.apply_coalesced(Command::SetFxAuto { id: clip, auto }, gesture);
    }

    pub fn draw_fx_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.fx_editors);
        for ed in &mut editors {
            self.draw_one_fx_editor(ctx, ed);
        }
        editors.retain(|e| e.open);
        self.fx_editors = editors;
    }

    #[allow(clippy::too_many_lines)]
    fn draw_one_fx_editor(&mut self, ctx: &egui::Context, ed: &mut FxEditor) {
        // подсказки из ui-замыканий (там `self` занят) — в накопитель
        // кадра, потом в hover_hint: попапов нет (п.14 р.8)
        let mut hover_out: Option<String> = None;
        // окно живёт, пока жива хоть одна его цепочка
        ed.members.retain(|id| self.model.clip(*id).is_some());
        let Some(anchor) = self.model.clip(ed.clip).cloned() else {
            ed.open = false;
            return;
        };
        // камеры для комбо Pan3d (Р.29): id + подпись
        let cameras: Vec<(u64, String)> = self
            .model
            .project()
            .cameras
            .iter()
            .map(|c| {
                (c.id.0, if c.name.is_empty() { format!("C{}", c.id.0) } else { c.name.clone() })
            })
            .collect();
        let title = if ed.members.len() > 1 {
            crate::tf!(k::fx::TITLE_GROUP, ed.clip.0)
        } else {
            crate::tf!(
                k::fx::TITLE,
                if anchor.kind == ClipKind::Audio {
                    tr(k::fx::KIND_AUDIO)
                } else {
                    tr(k::fx::KIND_VIDEO)
                },
                ed.clip.0
            )
        };
        let playhead = self.player.transport.playhead();
        // буферы живут в приложении; в окно отдаём их на время кадра,
        // чтобы не держать &mut self внутри ui-замыкания
        let mut env_clip = self.env_clipboard.clone();
        let mut fx_clip = self.fx_clipboard.clone();
        // что окно наредактировало за кадр: применяем ПОСЛЕ отрисовки
        struct Edited {
            clip: ClipId,
            fx: Option<Vec<FxInstance>>,
            auto: Option<Vec<Vec<kt_model::ParamCurve>>>,
            bypass: Option<Vec<bool>>,
            off: Option<bool>,
        }
        let mut edits: Vec<Edited> = Vec::new();
        let mut revert = false;
        // мини-таймлайн виджета автоматизации двигает ГЛАВНЫЙ курсор
        let mut seek: Option<kt_time::Tick> = None;
        let frame_step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        let mut open = ed.open;
        let members = ed.members.clone();
        egui::Window::new(title)
            .id(egui::Id::new(("fx", ed.clip)))
            .open(&mut open)
            .default_size([580.0, 520.0])
            .min_size([460.0, 320.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                for (n, id) in members.iter().enumerate() {
                    let Some(clip) = self.model.clip(*id).cloned() else { continue };
                    let mut fx = clip.fx.clone();
                    let mut bypass = clip.fx_bypass.clone();
                    bypass.resize(fx.len(), false);
                    let mut auto = clip.fx_auto.clone();
                    auto.resize(fx.len(), Vec::new());
                    let mut chain_on = !clip.fx_off;
                    let mut ch = crate::widgets::ChainChanged::default();
                    let mut params_changed = false;
                    let head = crate::tf!(
                        k::fx::CHAIN_OF,
                        if clip.kind == ClipKind::Audio {
                            tr(k::fx::KIND_AUDIO)
                        } else {
                            tr(k::fx::KIND_VIDEO)
                        },
                        id.0
                    );
                    // свёртка на цепочку: у пары видно обе, лишнюю можно
                    // свернуть («можно дать возможность скрытия-переключения»)
                    egui::CollapsingHeader::new(head)
                        .id_salt(("fxgrp", id.0))
                        .default_open(n == 0 || members.len() <= 2)
                        .show(ui, |ui| {
                            // выбранный слот и строку поиска берём в
                            // локальные переменные: иначе два поля одного
                            // и того же `ed` одолжены одновременно
                            // у цепочки события головы нет: выбор всегда
                            // слот эффекта, поэтому индекс хранится как был
                            let mut sel_slot = crate::widgets::ChainSel::Fx(
                                *ed.selected.get(id).unwrap_or(&0),
                            );
                            let mut query = std::mem::take(&mut ed.picker_query);
                            ch = crate::widgets::chain_strip(
                                ui,
                                ("fxchain", id.0),
                                crate::widgets::ChainEdit {
                                    fx: &mut fx,
                                    bypass: &mut bypass,
                                    auto: Some(&mut auto),
                                    selected: &mut sel_slot,
                                    master_on: &mut chain_on,
                                    head: None,
                                },
                                clip.kind == ClipKind::Audio,
                                &mut query,
                                &mut fx_clip,
                                clip.source_in,
                                &mut hover_out,
                            );
                            ed.picker_query = query;
                            let sel = sel_slot.fx().unwrap_or(0);
                            ed.selected.insert(*id, sel);
                            if let Some(f) = fx.get_mut(sel) {
                                // параметры — ОДНИМ кодом по спекам модели
                                params_changed |= draw_video_fx_params(ui, f, &cameras);
                                let mut auto_ch = false;
                                if let Some(curves) = auto.get_mut(sel) {
                                    let view = ed.auto_views.entry((*id, sel)).or_default();
                                    let (c, s) = draw_fx_auto_section(
                                        ui,
                                        &clip,
                                        sel,
                                        f,
                                        curves,
                                        playhead,
                                        &mut env_clip,
                                        view,
                                        frame_step,
                                        &mut hover_out,
                                    );
                                    auto_ch = c;
                                    if let Some(t) = s {
                                        seek = Some(t);
                                    }
                                }
                                ch.auto |= auto_ch;
                            }
                            // про пустую цепочку говорит сама полоса —
                            // второй раз повторять не надо
                        });
                    if ch.any() || params_changed {
                        edits.push(Edited {
                            clip: *id,
                            fx: (ch.fx || params_changed).then_some(fx),
                            auto: ch.auto.then_some(auto),
                            bypass: ch.bypass.then_some(bypass),
                            off: ch.master.then_some(!chain_on),
                        });
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr(k::fx::DONE)).clicked() {
                        ed.open = false;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        revert = true;
                        ed.open = false;
                    }
                });
            });
        if let Some(h) = hover_out {
            self.hover_hint = Some(h); // подсказка едет в строку состояния
        }
        if let Some(t) = seek {
            // «курсор внутри мини-таймлайна двигает курсор в основном
            // таймлайне, чтобы сразу видеть к чему относится» (Р.33 п.5)
            self.player.seek(kt_time::Tick(t.0.max(0)));
        }
        if !open {
            ed.open = false;
        }
        self.env_clipboard = env_clip;
        self.fx_clipboard = fx_clip;
        if revert {
            // «Отмена» откатывает ВСЁ окно: у пары обе цепочки
            for (id, fx, auto, bypass, off) in ed.backup.clone() {
                self.set_fx_coalesced(id, fx, ed.gesture);
                self.set_fx_auto_coalesced(id, auto, ed.gesture);
                self.apply_coalesced(Command::SetFxBypass { id, bypass }, ed.gesture);
                self.apply_coalesced(Command::SetFxOff { id, off }, ed.gesture);
            }
            return;
        }
        for e in edits {
            if let Some(fx) = e.fx {
                self.set_fx_coalesced(e.clip, fx, ed.gesture);
            }
            if let Some(mut auto) = e.auto {
                // ключи держим отсортированными (модель это валидирует)
                for curves in &mut auto {
                    for c in curves.iter_mut() {
                        c.keys.sort_by_key(|k| k.t.0);
                    }
                }
                self.set_fx_auto_coalesced(e.clip, auto, ed.gesture);
            }
            if let Some(mut b) = e.bypass {
                while b.last() == Some(&false) {
                    b.pop(); // хвостовые false не сериализуются зря
                }
                self.apply_coalesced(
                    Command::SetFxBypass { id: e.clip, bypass: b },
                    ed.gesture,
                );
            }
            if let Some(off) = e.off {
                self.apply_coalesced(Command::SetFxOff { id: e.clip, off }, ed.gesture);
            }
        }
    }
}

/// Секция автоматизации слота: выбор кривых, копипаст огибающих, ключ по
/// курсору — и сам виджет (`automation::draw`: график ↔ таблица).
/// Возвращает (было ли изменение, куда двинуть главный курсор).
/// У слота без автоматизируемых параметров (Mirror) не рисуется вовсе.
#[allow(clippy::too_many_arguments)]
fn draw_fx_auto_section(
    ui: &mut egui::Ui,
    clip: &kt_model::ClipEvent,
    slot: usize,
    f: &FxInstance,
    curves: &mut Vec<kt_model::ParamCurve>,
    playhead: kt_time::Tick,
    env_clip: &mut Option<kt_model::ops::EnvelopeBlock>,
    view: &mut crate::automation::AutoView,
    frame_step: i64,
    hover_out: &mut Option<String>,
) -> (bool, Option<kt_time::Tick>) {
    let (src_in, dur) = (clip.source_in, clip.duration);
    let mat_end = src_in.0 + dur.0;
    // только Smooth-параметры: структурные (длина линий реверба) кривой
    // не подлежат — их смена пересобирает DSP (Р.33)
    let specs: Vec<&kt_model::ParamSpec> = f.auto_params().collect();
    if specs.is_empty() {
        return (false, None);
    }
    let mut changed = false;
    ui.separator();
    ui.horizontal(|ui| {
        // параметры без кривой — кандидаты на добавление
        let free: Vec<&kt_model::ParamSpec> = specs
            .iter()
            .copied()
            .filter(|s| !curves.iter().any(|c| c.param == s.name))
            .collect();
        if !free.is_empty() {
            egui::ComboBox::from_id_salt(("fx_auto_add", clip.id, slot))
                .selected_text(tr(k::fx::AUTO_ADD))
                .show_ui(ui, |ui| {
                    for spec in &free {
                        let n = spec.name;
                        if ui.button(crate::widgets::param_label(n)).clicked() {
                            // стартовая кривая — константа текущего значения
                            let v0 = f.get_param(n).unwrap_or(spec.default);
                            curves.push(kt_model::ParamCurve {
                                param: n.to_string(),
                                ease: true,
                                keys: vec![
                                    kt_model::AutoKey { t: src_in, v: v0 },
                                    kt_model::AutoKey { t: kt_time::Tick(mat_end), v: v0 },
                                ],
                            });
                            changed = true;
                        }
                    }
                });
        }
        // выбор активной кривой (в неё кладёт ключ клик по графику) и её
        // удаление; сглаживание — свойство кривой
        for (ci, c) in curves.iter().enumerate() {
            if ui
                .selectable_label(view.active == ci, crate::widgets::param_label(&c.param))
                .clicked()
            {
                view.active = ci;
            }
        }
        if let Some(c) = curves.get_mut(view.active) {
            changed |= ui.checkbox(&mut c.ease, tr(k::fx::EASE)).changed();
            if ui.button(tr(k::fx::AUTO_REMOVE)).clicked() {
                let i = view.active;
                curves.remove(i);
                view.active = 0;
                view.sel = None;
                changed = true;
            }
        }
    });
    ui.horizontal(|ui| {
        // копипаст огибающих (модель — kt_model::ops, Р.33)
        if !curves.is_empty() && ui.button(tr(k::fx::COPY_ENV)).clicked() {
            let block = kt_model::ops::EnvelopeBlock {
                fx_tag: kt_model::ops::fx_tag(f),
                curves: curves.clone(),
                src_in,
            };
            if let Ok(json) = serde_json::to_string_pretty(&block) {
                ui.ctx().copy_text(json);
            }
            *env_clip = Some(block);
        }
        let same_kind = env_clip.as_ref().is_some_and(|b| b.fx_tag == kt_model::ops::fx_tag(f));
        if crate::app::hint_into(
            hover_out,
            ui.add_enabled(same_kind, egui::Button::new(tr(k::fx::PASTE_ENV))),
            tr(k::fx::PASTE_ENV_HINT),
        )
        .clicked()
            && let Some(b) = env_clip.clone()
        {
            // ключи живут в тиках МАТЕРИАЛА: переносим их на точку входа
            // этого события, иначе кривая уехала бы на чужой кусок
            let d = src_in.0 - b.src_in.0;
            for mut c in b.curves {
                for kf in &mut c.keys {
                    kf.t = kt_time::Tick((kf.t.0 + d).max(0));
                }
                curves.retain(|old| old.param != c.param);
                curves.push(c);
            }
            changed = true;
        }
        // ключ по курсору: позиция считается ТЕМ ЖЕ правилом, что у
        // рендера, и с учётом скорости — до Р.33 сюда клали прогресс
        // таймлайна, и при rate ≠ 1 точка вставала не под курсором
        if !curves.is_empty() && ui.button(tr(k::fx::KF_AT_CURSOR)).clicked() {
            let at = kt_engine::material_at(clip, frame_step, playhead);
            for c in curves.iter_mut() {
                let v = kt_model::curve_value(&c.keys, c.ease, at);
                c.keys.retain(|kf| kf.t != at);
                c.keys.push(kt_model::AutoKey { t: at, v });
                c.keys.sort_by_key(|kf| kf.t.0);
            }
            changed = true;
        }
    });
    // единый виджет: график ↔ таблица (Р.33 С3). Домен слота — материал
    // события; перевод в обе стороны учитывает реверс и скорость (Р.34 К1)
    let to_tl = |t: kt_time::Tick| kt_engine::timeline_at_material(clip, frame_step, t);
    let to_mat = |t: kt_time::Tick| kt_engine::material_at(clip, frame_step, t);
    let ctx = crate::automation::clip_ctx(clip, f, frame_step, &to_tl, &to_mat);
    let res = crate::automation::draw(
        ui,
        ("auto", clip.id.0, slot),
        &ctx,
        curves,
        view,
        playhead,
        hover_out,
    );
    (changed || res.changed, res.seek)
}
pub(crate) fn fx_display_name(f: &FxInstance) -> &'static str {
    match f {
        FxInstance::Gain { .. } => "Gain",
        FxInstance::SlowedReverb { .. } => "Slowed+Reverb",
        FxInstance::Pan { .. } => "Pan",
        FxInstance::CamMotion { .. } => "CamMotion",
        FxInstance::Mirror { .. } => "Mirror",
        FxInstance::Blur { .. } => "Blur",
        FxInstance::Greenscreen { .. } => "Greenscreen",
        FxInstance::Edges { .. } => "Edges",
        FxInstance::Noise { .. } => "Noise",
        FxInstance::Vhs { .. } => "VHS",
        FxInstance::Glow { .. } => "Glow",
        FxInstance::EventGrid { .. } => "EventGrid",
        FxInstance::Pan3d { .. } => "Pan 3D",
    }
}

/// Шаблоны видео-FX с дефолтами v1 (кнопка «+ видео-FX…»).
pub(crate) fn video_fx_templates() -> Vec<(&'static str, FxInstance)> {
    use kt_model::PanMode;
    vec![
        // CamMotion из списка убран (Р.34 К2): его кейфреймы — это Pan
        // с кривыми, и старые проекты конвертируются при чтении.
        ("Pan", FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }),
        ("Mirror", FxInstance::Mirror { horizontal: true, vertical: false }),
        ("Blur", FxInstance::blur(8)),
        (
            "Greenscreen",
            FxInstance::greenscreen([0, 255, 0], 70, 25 ),
        ),
        ("Edges", FxInstance::edges(48)),
        ("Noise", FxInstance::noise(0.08)),
        ("VHS", FxInstance::vhs(4, 0.4)),
        ("Glow", FxInstance::glow(0.65)),
        (
            // Р.29: первый 3D-эффект; дефолты — тождество (слой 1:1)
            "Pan 3D",
            FxInstance::Pan3d {
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
                rot_x: 0.0,
                rot_y: 0.0,
                rot_z: 0.0,
                anchor_x: 0.5,
                anchor_y: 0.5,
                scale: 1.0,
                camera: None,
            },
        ),
        ("EventGrid", FxInstance::EventGrid { cols: 2, rows: 2, center: true }),
    ]
}

/// Параметры эффекта: числовые — ЕДИНОЙ сеткой по спекам модели
/// (`widgets::params_grid`), а рядом — то, что числом не выражается:
/// режим кроп-рамки, флаги зеркала, пипетка ключевого цвета, выбор
/// камеры, кейфреймы CamMotion. `cameras` — (id, подпись) для комбо Pan3d.
///
/// До Р.33 у каждого варианта была своя ручная форма: одни и те же
/// величины рисовались слайдером в одном окне и drag-полем в другом, а
/// диапазоны расходились между окнами и спекой. Теперь диапазон, шаг и
/// единица приходят из одного места.
pub(crate) fn draw_video_fx_params(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    cameras: &[(u64, String)],
) -> bool {
    use kt_model::PanMode;
    let mut ch = false;
    // ключевой цвет удобнее пипеткой, чем тремя числами
    let skip: &[&str] = match f {
        FxInstance::Greenscreen { .. } => &["key_r", "key_g", "key_b"],
        // цвет линий контура и оттенок свечения — тоже пипеткой (Р.33 С5)
        FxInstance::Edges { .. } => &["color_r", "color_g", "color_b"],
        FxInstance::Glow { .. } => &["tint_r", "tint_g", "tint_b"],
        _ => &[],
    };

    // --- нечисловые контролы варианта ------------------------------------
    let mode_row = |ui: &mut egui::Ui, mode: &mut PanMode, ch: &mut bool| {
        ui.horizontal(|ui| {
            ui.label(tr(k::fx::MODE));
            egui::ComboBox::from_id_salt("panmode")
            .selected_text(format!("{mode:?}"))
            .show_ui(ui, |ui| {
                for m in [PanMode::Fit, PanMode::Fill, PanMode::Stretch] {
                    *ch |= ui.selectable_value(mode, m, format!("{m:?}")).changed();
                }
            });
        });
    };
    match f {
        FxInstance::Pan { mode, .. } => mode_row(ui, mode, &mut ch),
        FxInstance::CamMotion { mode, ease, .. } => {
            let mut e = *ease;
            mode_row(ui, mode, &mut ch);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut e, tr(k::fx::EASE)).changed() {
                    ch = true;
                }
            });
            if let FxInstance::CamMotion { ease, .. } = f {
                *ease = e;
            }
        }
        FxInstance::Mirror { horizontal, vertical } => {
            ui.horizontal(|ui| {
                ch |= ui.checkbox(horizontal, tr(k::fx::HORIZONTAL)).changed();
                ch |= ui.checkbox(vertical, tr(k::fx::VERTICAL)).changed();
            });
        }
        FxInstance::Greenscreen { key_r, key_g, key_b, .. } => {
            ui.horizontal(|ui| {
                let mut col = [*key_r as f32 / 255.0, *key_g as f32 / 255.0, *key_b as f32 / 255.0];
                ui.label(tr(k::fx::KEY_COLOR));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *key_r = (col[0] * 255.0) as u8;
                    *key_g = (col[1] * 255.0) as u8;
                    *key_b = (col[2] * 255.0) as u8;
                    ch = true;
                }
            });
        }
        FxInstance::EventGrid { center, .. } => {
            ui.horizontal(|ui| {
                ch |= ui.checkbox(center, tr(k::fx::CENTER)).changed();
            });
        }
        FxInstance::Edges { color_r, color_g, color_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*color_r as f32 / 255.0, *color_g as f32 / 255.0, *color_b as f32 / 255.0];
                ui.label(tr(k::fx::LINE_COLOR));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *color_r = (col[0] * 255.0) as u8;
                    *color_g = (col[1] * 255.0) as u8;
                    *color_b = (col[2] * 255.0) as u8;
                    ch = true;
                }
            });
        }
        FxInstance::Glow { tint_r, tint_g, tint_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*tint_r as f32 / 255.0, *tint_g as f32 / 255.0, *tint_b as f32 / 255.0];
                ui.label(tr(k::fx::TINT));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *tint_r = (col[0] * 255.0) as u8;
                    *tint_g = (col[1] * 255.0) as u8;
                    *tint_b = (col[2] * 255.0) as u8;
                    ch = true;
                }
            });
        }
        FxInstance::Noise { color, .. } => {
            ui.horizontal(|ui| {
                ch |= ui.checkbox(color, tr(k::fx::COLOR_NOISE)).changed();
            });
        }
        _ => {}
    }

    // --- числовые параметры: одна сетка на все варианты -------------------
    ch |= crate::widgets::params_grid(ui, ("fxp", fx_display_name(f)), f, skip);

    // --- хвост: камера Pan3d и кейфреймы CamMotion ------------------------
    match f {
        FxInstance::Pan3d { camera, .. } => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::CAMERA));
                let sel = match camera {
                    None => tr(k::fx::CAMERA_ACTIVE).to_string(),
                    Some(id) => cameras
                        .iter()
                        .find(|(cid, _)| *cid == id.0)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| format!("C{}", id.0)),
                };
                egui::ComboBox::from_id_salt("pan3dcam").selected_text(sel).show_ui(ui, |ui| {
                    ch |= ui.selectable_value(camera, None, tr(k::fx::CAMERA_ACTIVE)).changed();
                    for (cid, name) in cameras {
                        ch |= ui
                            .selectable_value(camera, Some(kt_model::CameraId(*cid)), name.clone())
                            .changed();
                    }
                });
            });
        }
        FxInstance::CamMotion { keyframes, .. } => {
            ui.weak(crate::tf!(k::fx::KEYFRAMES_N, keyframes.len()));
            ui.horizontal(|ui| {
                if ui.button(tr(k::fx::ADD_KF)).clicked() {
                    let t = keyframes.last().map(|k| (k.t + 0.25).min(1.0)).unwrap_or(0.0);
                    keyframes.push(kt_model::CamKeyframe { t, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 });
                    ch = true;
                }
                if !keyframes.is_empty() && ui.button(tr(k::fx::DROP_LAST)).clicked() {
                    keyframes.pop();
                    ch = true;
                }
            });
            for (i, kf) in keyframes.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("#{i}"));
                    ch |= ui
                        .add(egui::DragValue::new(&mut kf.t).range(0.0..=1.0).speed(0.005))
                        .changed();
                    ui.label(tr(k::fxp::PAN_X));
                    ch |= ui
                        .add(egui::DragValue::new(&mut kf.pan_x).range(-4.0..=4.0).speed(0.01))
                        .changed();
                    ui.label(tr(k::fxp::PAN_Y));
                    ch |= ui
                        .add(egui::DragValue::new(&mut kf.pan_y).range(-4.0..=4.0).speed(0.01))
                        .changed();
                    ui.label(tr(k::fxp::ZOOM));
                    ch |= ui
                        .add(egui::DragValue::new(&mut kf.zoom).range(0.05..=32.0).speed(0.01))
                        .changed();
                });
            }
        }
        _ => {}
    }
    ch
}
