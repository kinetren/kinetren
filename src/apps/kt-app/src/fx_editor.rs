//! Окно FX-цепочки клипа (порт ClipFxChainWindow + FxChainEditorWidget):
//! полоса карточек «Media → fx → fx», форма параметров, полоса автоматизации
//! (ромбы: клик = выбор, драг = перенос t, double-click = удалить),
//! live-применение через apply_coalesced (вся сессия окна = 1 undo),
//! Отмена = откат к состоянию на момент открытия.

use crate::i18n::{k, tr};
use crate::app::KtApp;
use eframe::egui;
use kt_model::*;

pub struct FxEditor {
    pub clip: ClipId,
    pub picker_query: String,
    pub backup: Vec<FxInstance>,
    /// Кривые автоматизации на момент открытия (Cancel возвращает всё).
    pub backup_auto: Vec<Vec<kt_model::ParamCurve>>,
    pub gesture: u64,
    pub selected: usize,
    pub open: bool,
}

impl KtApp {
    pub fn open_fx_editor(&mut self, clip: ClipId) {
        if self.fx_editors.iter().any(|e| e.clip == clip) {
            return;
        }
        let Some(c) = self.model.clip(clip) else { return };
        self.gesture_counter += 1;
        self.fx_editors.push(FxEditor {
            clip,
            picker_query: String::new(),
            backup: c.fx.clone(),
            backup_auto: c.fx_auto.clone(),
            gesture: self.gesture_counter,
            selected: c.fx.len().saturating_sub(1),
            open: true,
        });
    }

    pub(crate) fn set_fx_coalesced(&mut self, clip: ClipId, fx: Vec<FxInstance>, gesture: u64) {
        self.apply_coalesced(Command::SetFx { id: clip, fx }, gesture);
    }

    /// Кривые автоматизации (Р.30): тот же gesture, что у SetFx — один
    /// undo-шаг на сессию окна.
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
        let Some(clip) = self.model.clip(ed.clip).cloned() else {
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
        let title = crate::tf!(
            k::fx::TITLE,
            if clip.kind == ClipKind::Audio {
                tr(k::fx::KIND_AUDIO)
            } else {
                tr(k::fx::KIND_VIDEO)
            },
            ed.clip.0
        );
        let mut fx = clip.fx.clone();
        let mut bypass = clip.fx_bypass.clone();
        bypass.resize(fx.len(), false);
        // кривые автоматизации (Р.30) двигаются ПАРАЛЛЕЛЬНО слотам
        let mut auto = clip.fx_auto.clone();
        auto.resize(fx.len(), Vec::new());
        let mut changed = false;
        let mut auto_changed = false;
        let mut bypass_changed = false;
        let mut chain_on = !clip.fx_off;
        let mut off_changed = false;
        let playhead = self.player.transport.playhead();
        // буфер огибающих живёт в приложении; в окно отдаём его на время
        // кадра, чтобы не держать &mut self внутри ui-замыкания
        let mut env_clip = self.env_clipboard.clone();
        let mut fx_clip = self.fx_clipboard.clone();
        let mut open = ed.open;
        egui::Window::new(title)
            .id(egui::Id::new(("fx", ed.clip)))
            .open(&mut open)
            .default_size([580.0, 520.0])
            .min_size([460.0, 320.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                // --- полоса цепочки: ОДИН виджет на события и морферы ----
                let ch = crate::widgets::chain_strip(
                    ui,
                    ("fxchain", ed.clip.0),
                    crate::widgets::ChainEdit {
                        fx: &mut fx,
                        bypass: &mut bypass,
                        auto: Some(&mut auto),
                        selected: &mut ed.selected,
                        master_on: &mut chain_on,
                    },
                    clip.kind == ClipKind::Audio,
                    &mut ed.picker_query,
                    &mut fx_clip,
                    clip.source_in,
                    &mut hover_out,
                );
                changed |= ch.fx;
                bypass_changed |= ch.bypass;
                auto_changed |= ch.auto;
                off_changed |= ch.master;
                ui.separator();

                // --- параметры выбранного слота ---------------------------------
                if let Some(f) = fx.get_mut(ed.selected) {
                    // параметры рисуются ОДНИМ кодом по спекам модели
                    // (Р.33): диапазоны, шаг и единицы больше не могут
                    // разойтись между окнами, а подписи идут из каталога
                    changed |= draw_video_fx_params(ui, f, &cameras);
                    // --- автоматизация параметров (Р.30, видео-слоты) ----
                    auto_changed |= draw_fx_auto_section(
                        ui,
                        &clip,
                        ed.selected,
                        &fx[ed.selected],
                        &mut auto[ed.selected],
                        playhead,
                        &mut env_clip,
                        &mut hover_out,
                    );
                } else {
                    ui.weak(tr(k::fx::CHAIN_EMPTY));
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr(k::fx::DONE)).clicked() {
                        ed.open = false;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        fx = ed.backup.clone();
                        auto = ed.backup_auto.clone();
                        auto.resize(fx.len(), Vec::new());
                        changed = true;
                        auto_changed = true;
                        ed.open = false;
                    }
                });
            });
        if let Some(h) = hover_out {
            self.hover_hint = Some(h); // подсказка едет в строку состояния
        }
        if !open {
            ed.open = false;
        }
        if changed {
            self.set_fx_coalesced(ed.clip, fx, ed.gesture);
        }
        if auto_changed {
            // ключи держим отсортированными (модель это валидирует)
            for curves in &mut auto {
                for c in curves.iter_mut() {
                    c.keys.sort_by_key(|k| k.t.0);
                }
            }
            self.set_fx_auto_coalesced(ed.clip, auto, ed.gesture);
        }
        self.env_clipboard = env_clip;
        self.fx_clipboard = fx_clip;
        if off_changed {
            self.apply_coalesced(
                Command::SetFxOff { id: ed.clip, off: !chain_on },
                ed.gesture,
            );
        }
        if bypass_changed {
            // хвостовые false не сериализуются зря
            let mut b = bypass;
            while b.last() == Some(&false) {
                b.pop();
            }
            self.apply_coalesced(Command::SetFxBypass { id: ed.clip, bypass: b }, ed.gesture);
        }
    }
}

/// Секция автоматизации параметров слота: кривые (позиция в МАТЕРИАЛЕ,
/// значение). Возвращает true при любой правке. У слота без
/// автоматизируемых параметров (Mirror) не рисуется вовсе.
///
/// Вид секции здесь временный: единый виджет автоматизации (график ↔
/// таблица, мини-таймлайн) — следующая стадия раунда; сейчас нужно, чтобы
/// правки жили в новом домене.
#[allow(clippy::too_many_arguments)]
fn draw_fx_auto_section(
    ui: &mut egui::Ui,
    clip: &kt_model::ClipEvent,
    slot: usize,
    f: &FxInstance,
    curves: &mut Vec<kt_model::ParamCurve>,
    playhead: kt_time::Tick,
    env_clip: &mut Option<kt_model::ops::EnvelopeBlock>,
    hover_out: &mut Option<String>,
) -> bool {
    // кусок материала, который показывает событие: в нём и живут ключи
    let (src_in, dur) = (clip.source_in, clip.duration);
    let mat_end = src_in.0 + dur.0;
    // только Smooth-параметры: структурные (длина линий реверба) кривой
    // не подлежат — их смена пересобирает DSP (Р.33)
    let specs: Vec<&kt_model::ParamSpec> = f.auto_params().collect();
    if specs.is_empty() {
        return false;
    }
    let mut changed = false;
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(tr(k::fx::AUTO_CLIP));
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
                        if ui.button(n).clicked() {
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
    });
    ui.horizontal(|ui| {
        // копипаст огибающих (Р.33, модель — kt_model::ops): раньше это
        // умел только slowed&reverb своим отдельным виджетом
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
        // рендера (Р.33) — до этого сюда клали прогресс таймлайна, и при
        // скорости ≠ 1 точка вставала не под курсором
        if !curves.is_empty() && ui.button(tr(k::fx::KF_AT_CURSOR)).clicked() {
            let at = kt_model::clip_material_at(
                clip.source_in,
                clip.start,
                clip.duration,
                clip.reversed,
                0,
                playhead,
            );
            for c in curves.iter_mut() {
                let v = kt_model::curve_value(&c.keys, c.ease, at);
                c.keys.retain(|kf| kf.t != at);
                c.keys.push(kt_model::AutoKey { t: at, v });
                c.keys.sort_by_key(|kf| kf.t.0);
            }
            changed = true;
        }
    });
    let mut drop_curve: Option<usize> = None;
    for (ci, c) in curves.iter_mut().enumerate() {
        let (lo, hi) = specs
            .iter()
            .find(|s| s.name == c.param)
            .map_or((0.0, 1.0), |s| (s.min, s.max));
        ui.horizontal(|ui| {
            ui.strong(&c.param);
            ui.label("Ease:");
            changed |= ui.checkbox(&mut c.ease, "").changed();
            if ui.button(tr(k::fx::ADD_KF)).clicked() {
                // четверть куска дальше последнего ключа, но не за конец
                let step = (dur.0 / 4).max(1);
                let t = c.keys.last().map_or(src_in.0, |k| (k.t.0 + step).min(mat_end));
                let v = c.keys.last().map(|k| k.v).unwrap_or(lo);
                c.keys.push(kt_model::AutoKey { t: kt_time::Tick(t), v });
                changed = true;
            }
            if !c.keys.is_empty() && ui.button(tr(k::fx::DROP_LAST)).clicked() {
                c.keys.pop();
                changed = true;
            }
            if ui.button(tr(k::fx::AUTO_REMOVE)).clicked() {
                drop_curve = Some(ci);
            }
        });
        let speed = ((hi - lo) / 500.0).max(1e-4);
        for (i, key) in c.keys.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                // позиция ключа — секунды МАТЕРИАЛА (домен Р.33); доля
                // куска показана рядом, чтобы не гадать «где это в клипе»
                ui.label(format!("#{i}"));
                let mut secs = key.t.0 as f64 / 90_000.0;
                let lo_s = 0.0f64;
                let hi_s = (mat_end as f64 / 90_000.0).max(lo_s);
                if ui
                    .add(
                        egui::DragValue::new(&mut secs)
                            .range(lo_s..=hi_s)
                            .speed(0.01)
                            .suffix(" s"),
                    )
                    .changed()
                {
                    key.t = kt_time::Tick((secs * 90_000.0).round() as i64);
                    changed = true;
                }
                let frac = if dur.0 > 0 {
                    ((key.t.0 - src_in.0) as f64 / dur.0 as f64).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                ui.weak(format!("{:.0}%", frac * 100.0));
                changed |= ui
                    .add(egui::DragValue::new(&mut key.v).range(lo..=hi).speed(speed))
                    .changed();
            });
        }
    }
    if let Some(ci) = drop_curve {
        curves.remove(ci);
        changed = true;
    }
    changed
}

/// Имя эффекта в полосе цепочки.
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
        ("Pan", FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }),
        (
            "CamMotion",
            FxInstance::CamMotion {
                mode: PanMode::Fit,
                ease: true,
                time_scale: 1.0,
                keyframes: vec![],
            },
        ),
        ("Mirror", FxInstance::Mirror { horizontal: true, vertical: false }),
        ("Blur", FxInstance::Blur { radius: 8 }),
        (
            "Greenscreen",
            FxInstance::Greenscreen { key_r: 0, key_g: 255, key_b: 0, tolerance: 70, softness: 25 },
        ),
        ("Edges", FxInstance::Edges { threshold: 48 }),
        ("Noise", FxInstance::Noise { amount: 0.08 }),
        ("VHS", FxInstance::Vhs { rgb_shift: 4, bleed: 0.4 }),
        ("Glow", FxInstance::Glow { strength: 0.65 }),
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
    let skip: &[&str] =
        if matches!(f, FxInstance::Greenscreen { .. }) { &["key_r", "key_g", "key_b"] } else { &[] };

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
