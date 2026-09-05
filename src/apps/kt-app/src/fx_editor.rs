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
    pub sel_kf: Option<usize>,
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
            sel_kf: None,
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
        let mut open = ed.open;
        egui::Window::new(title)
            .id(egui::Id::new(("fx", ed.clip)))
            .open(&mut open)
            .default_size([580.0, 520.0])
            .min_size([460.0, 320.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                // --- полоса цепочки -------------------------------------------
                ui.horizontal(|ui| {
                    ui.add_enabled(false, egui::Button::new("Media"));
                    for (i, f) in fx.iter().enumerate() {
                        ui.label(crate::icons::CARET_RIGHT);
                        let off = bypass.get(i).copied().unwrap_or(false);
                        let name = if off {
                            crate::tf!(k::fx::OFF_SUFFIX, fx_display_name(f))
                        } else {
                            fx_display_name(f).to_string()
                        };
                        let mut text = egui::RichText::new(name);
                        if off {
                            text = text.weak().strikethrough();
                        }
                        if ui.selectable_label(ed.selected == i, text).clicked() {
                            ed.selected = i;
                            ed.sel_kf = None;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    // единый пикер с поиском (п.3 v1.4), фильтр по виду клипа
                    if let Some(tmpl) = crate::fx_registry::fx_picker(
                        ui,
                        ("fx_add", ed.clip),
                        clip.kind == ClipKind::Audio,
                        &mut ed.picker_query,
                    ) {
                        fx.push(tmpl);
                        auto.push(Vec::new());
                        ed.selected = fx.len() - 1;
                        changed = true;
                    }
                    if !fx.is_empty() {
                        // bypass выбранного слота (п.2)
                        if let Some(b) = bypass.get_mut(ed.selected) {
                            let label = egui::RichText::new(crate::icons::POWER).color(if *b {
                                egui::Color32::from_gray(110)
                            } else {
                                egui::Color32::from_rgb(120, 230, 120)
                            });
                            if crate::app::hint_into(
                                &mut hover_out,
                                ui.button(label),
                                if *b { tr(k::fx::ENABLE) } else { tr(k::fx::DISABLE) },
                            )
                            .clicked()
                            {
                                *b = !*b;
                                bypass_changed = true;
                            }
                        }
                        if ui.button(tr(k::fx::REMOVE_SLOT)).clicked() && ed.selected < fx.len()
                        {
                            fx.remove(ed.selected);
                            auto.remove(ed.selected);
                            auto_changed = true;
                            if ed.selected < bypass.len() {
                                bypass.remove(ed.selected);
                                bypass_changed = true;
                            }
                            ed.selected = ed.selected.saturating_sub(1);
                            ed.sel_kf = None;
                            changed = true;
                        }
                        if crate::app::hint_into(
                            &mut hover_out,
                            ui.button(crate::icons::CARET_LEFT),
                            tr(k::fx::MOVE_LEFT),
                        )
                        .clicked()
                            && ed.selected > 0
                        {
                            fx.swap(ed.selected, ed.selected - 1);
                            auto.swap(ed.selected, ed.selected - 1);
                            auto_changed = true;
                            bypass.swap(ed.selected, ed.selected - 1);
                            bypass_changed = true;
                            ed.selected -= 1;
                            changed = true;
                        }
                        if crate::app::hint_into(
                            &mut hover_out,
                            ui.button(crate::icons::CARET_RIGHT),
                            tr(k::fx::MOVE_RIGHT),
                        )
                        .clicked()
                            && ed.selected + 1 < fx.len()
                        {
                            fx.swap(ed.selected, ed.selected + 1);
                            auto.swap(ed.selected, ed.selected + 1);
                            auto_changed = true;
                            bypass.swap(ed.selected, ed.selected + 1);
                            bypass_changed = true;
                            ed.selected += 1;
                            changed = true;
                        }
                    }
                });
                ui.separator();

                // --- параметры выбранного слота ---------------------------------
                if let Some(f) = fx.get_mut(ed.selected) {
                    changed |= draw_video_fx_params(ui, f, &cameras);
                    #[allow(clippy::single_match)]
                    match f {
                        FxInstance::Gain { db } => {
                            ui.horizontal(|ui| {
                                ui.label("Gain, dB:");
                                changed |= ui
                                    .add(egui::DragValue::new(db).range(-24.0..=24.0).speed(0.1))
                                    .changed();
                            });
                        }
                        FxInstance::SlowedReverb {
                            rate,
                            mix,
                            reverb_delay_ms,
                            damp,
                            decay,
                            wet_gain,
                            automation,
                        } => {
                            egui::Grid::new("srv").num_columns(4).show(ui, |ui| {
                                ui.label("Rate:");
                                changed |= ui
                                    .add(egui::DragValue::new(rate).range(0.25..=4.0).speed(0.005))
                                    .changed();
                                ui.label("Mix:");
                                changed |= ui
                                    .add(egui::DragValue::new(mix).range(0.0..=1.0).speed(0.005))
                                    .changed();
                                ui.end_row();
                                ui.label("Delay, ms:");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(reverb_delay_ms)
                                            .range(10.0..=500.0)
                                            .speed(0.5),
                                    )
                                    .changed();
                                ui.label("Decay:");
                                changed |= ui
                                    .add(egui::DragValue::new(decay).range(0.0..=0.98).speed(0.005))
                                    .changed();
                                ui.end_row();
                                ui.label("Damp:");
                                changed |= ui
                                    .add(egui::DragValue::new(damp).range(0.0..=0.99).speed(0.005))
                                    .changed();
                                ui.label("Wet gain:");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(wet_gain).range(0.0..=2.0).speed(0.01),
                                    )
                                    .changed();
                                ui.end_row();
                            });

                            // --- автоматизация ---------------------------------
                            ui.add_space(4.0);
                            ui.label(
                                tr(k::fx::AUTOMATION),
                            );
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 90.0),
                                egui::Sense::click_and_drag(),
                            );
                            let p = ui.painter_at(rect);
                            p.rect_filled(rect, 3.0, egui::Color32::from_gray(18));
                            // границы клипа t=0/t=1 — зелёный пунктир (порт)
                            for tx in [0.0f32, 1.0] {
                                let x = rect.left() + tx * rect.width();
                                p.line_segment(
                                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                                    egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 160, 90)),
                                );
                            }
                            let rate_to_y = |r: f64| -> f32 {
                                let t = ((r - 0.25) / (2.0 - 0.25)).clamp(0.0, 1.0) as f32;
                                rect.bottom() - 8.0 - t * (rect.height() - 16.0)
                            };
                            // ломаная профиля
                            if automation.len() > 1 {
                                let pts: Vec<egui::Pos2> = automation
                                    .iter()
                                    .map(|k| {
                                        egui::pos2(
                                            rect.left() + k.t as f32 * rect.width(),
                                            rate_to_y(k.rate),
                                        )
                                    })
                                    .collect();
                                p.add(egui::Shape::line(
                                    pts,
                                    egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
                                ));
                            }
                            // ромбы
                            let mut hit: Option<usize> = None;
                            for (i, k) in automation.iter().enumerate() {
                                let x = rect.left() + k.t as f32 * rect.width();
                                let y = rate_to_y(k.rate);
                                let selected = ed.sel_kf == Some(i);
                                let col = if selected {
                                    egui::Color32::from_rgb(120, 180, 255)
                                } else {
                                    egui::Color32::from_gray(220)
                                };
                                p.add(egui::Shape::convex_polygon(
                                    vec![
                                        egui::pos2(x, y - 5.0),
                                        egui::pos2(x + 5.0, y),
                                        egui::pos2(x, y + 5.0),
                                        egui::pos2(x - 5.0, y),
                                    ],
                                    col,
                                    egui::Stroke::NONE,
                                ));
                                if let Some(pos) = resp.interact_pointer_pos()
                                    && (pos - egui::pos2(x, y)).length() < 8.0
                                {
                                    hit = Some(i);
                                }
                            }
                            if resp.double_clicked() {
                                if let Some(i) = hit
                                    && automation.len() > 1
                                {
                                    automation.remove(i);
                                    ed.sel_kf = None;
                                    changed = true;
                                }
                            } else if resp.drag_started() || resp.clicked() {
                                if let Some(i) = hit {
                                    ed.sel_kf = Some(i);
                                }
                            } else if resp.dragged()
                                && let (Some(i), Some(pos)) =
                                    (ed.sel_kf, resp.interact_pointer_pos())
                                && i < automation.len()
                            {
                                let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                                automation[i].t = t as f64;
                                let r = 0.25
                                    + ((rect.bottom() - 8.0 - pos.y) / (rect.height() - 16.0))
                                        .clamp(0.0, 1.0)
                                        as f64
                                        * (2.0 - 0.25);
                                automation[i].rate = r;
                                // сортировка может переставить точки —
                                // выбор следует за СВОЕЙ (по t), а не за
                                // индексом (уезжал на чужой кейфрейм)
                                let moved_t = automation[i].t;
                                automation.sort_by(|a, b| a.t.total_cmp(&b.t));
                                ed.sel_kf =
                                    automation.iter().position(|k| (k.t - moved_t).abs() < 1e-12);
                                changed = true;
                            }
                            ui.horizontal(|ui| {
                                // копипаст огибающей (п.5, порт v1
                                // «Copy/Paste envelopes» + JSON в системный
                                // клипборд для интеропа)
                                if ui.button(tr(k::fx::COPY_ENV)).clicked() {
                                    self.automation_clipboard = Some(automation.clone());
                                    if let Ok(json) = serde_json::to_string_pretty(&automation) {
                                        ui.ctx().copy_text(json);
                                    }
                                }
                                let can_paste = self.automation_clipboard.is_some();
                                if crate::app::hint_into(
                                    &mut hover_out,
                                    ui.add_enabled(
                                        can_paste,
                                        egui::Button::new(tr(k::fx::PASTE_ENV)),
                                    ),
                                    tr(k::fx::PASTE_ENV_HINT),
                                )
                                .clicked()
                                    && let Some(kfs) = self.automation_clipboard.clone()
                                {
                                    *automation = kfs;
                                    ed.sel_kf = None;
                                    changed = true;
                                }
                                if ui.button(tr(k::fx::KF_AT_CURSOR)).clicked() {
                                    let ph = self.player.transport.playhead();
                                    let t = ((ph.0 - clip.start.0) as f64
                                        / clip.duration.0.max(1) as f64)
                                        .clamp(0.0, 1.0);
                                    automation.push(SlowedKeyframe { t, rate: *rate, mix: *mix });
                                    automation.sort_by(|a, b| a.t.total_cmp(&b.t));
                                    changed = true;
                                }
                                if let Some(i) = ed.sel_kf
                                    && i < automation.len()
                                {
                                    ui.label("t:");
                                    changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut automation[i].t)
                                                .range(0.0..=1.0)
                                                .speed(0.002),
                                        )
                                        .changed();
                                    ui.label("rate:");
                                    changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut automation[i].rate)
                                                .range(0.25..=4.0)
                                                .speed(0.005),
                                        )
                                        .changed();
                                    ui.label("mix:");
                                    changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut automation[i].mix)
                                                .range(0.0..=1.0)
                                                .speed(0.005),
                                        )
                                        .changed();
                                }
                            });
                        }
                        // видео-FX рисуются draw_video_fx_params выше
                        _ => {}
                    }
                    // --- автоматизация параметров (Р.30, видео-слоты) ----
                    auto_changed |= draw_fx_auto_section(
                        ui,
                        ed.clip,
                        ed.selected,
                        &fx[ed.selected],
                        &mut auto[ed.selected],
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
                    c.keys.sort_by(|a, b| a.t.total_cmp(&b.t));
                }
            }
            self.set_fx_auto_coalesced(ed.clip, auto, ed.gesture);
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

/// Секция автоматизации параметров слота (Р.30): кривые (t, v) по
/// прогрессу клипа 0..1. Возвращает true при любой правке. У слота без
/// автоматизируемых параметров (Mirror, аудио) не рисуется вовсе.
fn draw_fx_auto_section(
    ui: &mut egui::Ui,
    clip: ClipId,
    slot: usize,
    f: &FxInstance,
    curves: &mut Vec<kt_model::ParamCurve>,
) -> bool {
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
            egui::ComboBox::from_id_salt(("fx_auto_add", clip, slot))
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
                                    kt_model::AutoKey { t: 0.0, v: v0 },
                                    kt_model::AutoKey { t: 1.0, v: v0 },
                                ],
                            });
                            changed = true;
                        }
                    }
                });
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
                let t = c.keys.last().map(|k| (k.t + 0.25).min(1.0)).unwrap_or(0.0);
                let v = c.keys.last().map(|k| k.v).unwrap_or(lo);
                c.keys.push(kt_model::AutoKey { t, v });
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
                ui.label(format!("#{i} t:"));
                changed |= ui
                    .add(egui::DragValue::new(&mut key.t).range(0.0..=1.0).speed(0.005))
                    .changed();
                ui.label("v:");
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

/// Параметры видео-FX (компактная сетка). true = изменилось.
/// `cameras` — (id, подпись) для комбо Pan3d: активная камера / прибитая.
pub(crate) fn draw_video_fx_params(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    cameras: &[(u64, String)],
) -> bool {
    use kt_model::PanMode;
    let mut ch = false;
    let mode_ui = |ui: &mut egui::Ui, mode: &mut PanMode, ch: &mut bool| {
        egui::ComboBox::from_id_salt("panmode").selected_text(format!("{mode:?}")).show_ui(
            ui,
            |ui| {
                for m in [PanMode::Fit, PanMode::Fill, PanMode::Stretch] {
                    *ch |= ui.selectable_value(mode, m, format!("{m:?}")).changed();
                }
            },
        );
    };
    match f {
        FxInstance::Pan { mode, pan_x, pan_y, zoom } => {
            egui::Grid::new("vfx").num_columns(4).show(ui, |ui| {
                ui.label(tr(k::fx::MODE));
                mode_ui(ui, mode, &mut ch);
                ui.label("Zoom:");
                ch |= ui.add(egui::DragValue::new(zoom).range(0.05..=32.0).speed(0.01)).changed();
                ui.end_row();
                ui.label("Pan X:");
                ch |= ui.add(egui::DragValue::new(pan_x).range(-4.0..=4.0).speed(0.01)).changed();
                ui.label("Pan Y:");
                ch |= ui.add(egui::DragValue::new(pan_y).range(-4.0..=4.0).speed(0.01)).changed();
                ui.end_row();
            });
        }
        FxInstance::CamMotion { mode, ease, time_scale, keyframes } => {
            egui::Grid::new("vfx").num_columns(4).show(ui, |ui| {
                ui.label(tr(k::fx::MODE));
                mode_ui(ui, mode, &mut ch);
                ui.label("Ease:");
                ch |= ui.checkbox(ease, "").changed();
                ui.end_row();
                ui.label("Time scale:");
                ch |= ui
                    .add(egui::DragValue::new(time_scale).range(0.05..=8.0).speed(0.01))
                    .changed();
                ui.end_row();
            });
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
            for (i, k) in keyframes.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("#{i} t:"));
                    ch |= ui
                        .add(egui::DragValue::new(&mut k.t).range(0.0..=1.0).speed(0.005))
                        .changed();
                    ui.label("x:");
                    ch |= ui
                        .add(egui::DragValue::new(&mut k.pan_x).range(-4.0..=4.0).speed(0.01))
                        .changed();
                    ui.label("y:");
                    ch |= ui
                        .add(egui::DragValue::new(&mut k.pan_y).range(-4.0..=4.0).speed(0.01))
                        .changed();
                    ui.label("zoom:");
                    ch |= ui
                        .add(egui::DragValue::new(&mut k.zoom).range(0.05..=32.0).speed(0.01))
                        .changed();
                });
            }
        }
        FxInstance::Mirror { horizontal, vertical } => {
            ui.horizontal(|ui| {
                ch |= ui.checkbox(horizontal, tr(k::fx::HORIZONTAL)).changed();
                ch |= ui.checkbox(vertical, tr(k::fx::VERTICAL)).changed();
            });
        }
        FxInstance::Pan3d {
            pos_x,
            pos_y,
            pos_z,
            rot_x,
            rot_y,
            rot_z,
            anchor_x,
            anchor_y,
            scale,
            camera,
        } => {
            // единицы формата (Р.29): позиции в px проекта, углы в градусах
            egui::Grid::new("vfx").num_columns(6).show(ui, |ui| {
                ui.label("X:");
                ch |= ui.add(egui::DragValue::new(pos_x).speed(1.0)).changed();
                ui.label("Y:");
                ch |= ui.add(egui::DragValue::new(pos_y).speed(1.0)).changed();
                ui.label("Z:");
                ch |= ui.add(egui::DragValue::new(pos_z).speed(1.0)).changed();
                ui.end_row();
                ui.label("Rot X:");
                ch |= ui
                    .add(egui::DragValue::new(rot_x).range(-360.0..=360.0).speed(0.5))
                    .changed();
                ui.label("Rot Y:");
                ch |= ui
                    .add(egui::DragValue::new(rot_y).range(-360.0..=360.0).speed(0.5))
                    .changed();
                ui.label("Rot Z:");
                ch |= ui
                    .add(egui::DragValue::new(rot_z).range(-360.0..=360.0).speed(0.5))
                    .changed();
                ui.end_row();
                ui.label("Anchor X:");
                ch |= ui
                    .add(egui::DragValue::new(anchor_x).range(0.0..=1.0).speed(0.005))
                    .changed();
                ui.label("Anchor Y:");
                ch |= ui
                    .add(egui::DragValue::new(anchor_y).range(0.0..=1.0).speed(0.005))
                    .changed();
                ui.label("Scale:");
                ch |= ui
                    .add(egui::DragValue::new(scale).range(0.01..=32.0).speed(0.01))
                    .changed();
                ui.end_row();
            });
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
                    ch |= ui
                        .selectable_value(camera, None, tr(k::fx::CAMERA_ACTIVE))
                        .changed();
                    for (cid, name) in cameras {
                        ch |= ui
                            .selectable_value(
                                camera,
                                Some(kt_model::CameraId(*cid)),
                                name.clone(),
                            )
                            .changed();
                    }
                });
            });
        }
        FxInstance::Blur { radius } => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::RADIUS));
                ch |= ui.add(egui::Slider::new(radius, 1..=48)).changed();
            });
        }
        FxInstance::Greenscreen { key_r, key_g, key_b, tolerance, softness } => {
            ui.horizontal(|ui| {
                let mut col = [*key_r as f32 / 255.0, *key_g as f32 / 255.0, *key_b as f32 / 255.0];
                ui.label(tr(k::fx::KEY_COLOR));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *key_r = (col[0] * 255.0) as u8;
                    *key_g = (col[1] * 255.0) as u8;
                    *key_b = (col[2] * 255.0) as u8;
                    ch = true;
                }
                ui.label(tr(k::fx::TOLERANCE));
                ch |= ui.add(egui::Slider::new(tolerance, 1..=200)).changed();
                ui.label(tr(k::fx::SOFTNESS));
                ch |= ui.add(egui::Slider::new(softness, 0..=100)).changed();
            });
        }
        FxInstance::Edges { threshold } => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::THRESHOLD));
                ch |= ui.add(egui::Slider::new(threshold, 1..=255)).changed();
            });
        }
        FxInstance::Noise { amount } => {
            ui.horizontal(|ui| {
                ui.label("Amount:");
                ch |= ui.add(egui::Slider::new(amount, 0.0..=1.0)).changed();
            });
        }
        FxInstance::Vhs { rgb_shift, bleed } => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::RGB_SHIFT));
                ch |= ui.add(egui::Slider::new(rgb_shift, 0..=12)).changed();
                ui.label("Bleed:");
                ch |= ui.add(egui::Slider::new(bleed, 0.0..=1.0)).changed();
            });
        }
        FxInstance::Glow { strength } => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::STRENGTH));
                ch |= ui.add(egui::Slider::new(strength, 0.0..=2.0)).changed();
            });
        }
        FxInstance::EventGrid { cols, rows, center } => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::COLUMNS));
                ch |= ui.add(egui::DragValue::new(cols).range(1..=512)).changed();
                ui.label(tr(k::fx::ROWS));
                ch |= ui.add(egui::DragValue::new(rows).range(1..=512)).changed();
                ch |= ui.checkbox(center, tr(k::fx::CENTER)).changed();
            });
        }
        _ => {}
    }
    ch
}
