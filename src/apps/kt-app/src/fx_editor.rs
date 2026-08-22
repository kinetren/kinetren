//! Окно FX-цепочки клипа (порт ClipFxChainWindow + FxChainEditorWidget):
//! полоса карточек «Media → fx → fx», форма параметров, полоса автоматизации
//! (ромбы: клик = выбор, драг = перенос t, double-click = удалить),
//! live-применение через apply_coalesced (вся сессия окна = 1 undo),
//! Отмена = откат к состоянию на момент открытия.

use crate::app::KtApp;
use eframe::egui;
use kt_model::*;

pub struct FxEditor {
    pub clip: ClipId,
    pub backup: Vec<FxInstance>,
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
            backup: c.fx.clone(),
            gesture: self.gesture_counter,
            selected: c.fx.len().saturating_sub(1),
            sel_kf: None,
            open: true,
        });
    }

    fn set_fx_coalesced(&mut self, clip: ClipId, fx: Vec<FxInstance>, gesture: u64) {
        if self.model.apply_coalesced(Command::SetFx { id: clip, fx }, gesture).is_ok() {
            self.player.set_project(self.model.project().clone());
            self.dirty = true;
        }
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
        let Some(clip) = self.model.clip(ed.clip).cloned() else {
            ed.open = false;
            return;
        };
        let title = format!(
            "{} эффекты — клип {}",
            if clip.kind == ClipKind::Audio { "Аудио" } else { "Видео" },
            ed.clip.0
        );
        let mut fx = clip.fx.clone();
        let mut changed = false;
        let mut open = ed.open;
        egui::Window::new(title)
            .id(egui::Id::new(("fx", ed.clip)))
            .open(&mut open)
            .default_width(560.0)
            .show(ctx, |ui| {
                // --- полоса цепочки -------------------------------------------
                ui.horizontal(|ui| {
                    ui.add_enabled(false, egui::Button::new("Media"));
                    for (i, f) in fx.iter().enumerate() {
                        ui.label("→");
                        let name = match f {
                            FxInstance::Gain { .. } => "Gain",
                            FxInstance::SlowedReverb { .. } => "Slowed+Reverb",
                        };
                        if ui.selectable_label(ed.selected == i, name).clicked() {
                            ed.selected = i;
                            ed.sel_kf = None;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("+ Gain").clicked() {
                        fx.push(FxInstance::Gain { db: 0.0 });
                        ed.selected = fx.len() - 1;
                        changed = true;
                    }
                    if ui.button("+ Slowed+Reverb").clicked() {
                        fx.push(FxInstance::slowed_reverb(0.82, 0.1, 85.0));
                        ed.selected = fx.len() - 1;
                        changed = true;
                    }
                    if !fx.is_empty() {
                        if ui.button("Удалить слот").clicked() && ed.selected < fx.len()
                        {
                            fx.remove(ed.selected);
                            ed.selected = ed.selected.saturating_sub(1);
                            ed.sel_kf = None;
                            changed = true;
                        }
                        if ui.button("←").on_hover_text("Сдвинуть слот влево").clicked()
                            && ed.selected > 0
                        {
                            fx.swap(ed.selected, ed.selected - 1);
                            ed.selected -= 1;
                            changed = true;
                        }
                        if ui.button("→").on_hover_text("Сдвинуть слот вправо").clicked()
                            && ed.selected + 1 < fx.len()
                        {
                            fx.swap(ed.selected, ed.selected + 1);
                            ed.selected += 1;
                            changed = true;
                        }
                    }
                });
                ui.separator();

                // --- параметры выбранного слота ---------------------------------
                if let Some(f) = fx.get_mut(ed.selected) {
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
                            ui.label("Автоматизация (t = прогресс клипа 0..1):");
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
                                automation.sort_by(|a, b| a.t.total_cmp(&b.t));
                                changed = true;
                            }
                            ui.horizontal(|ui| {
                                if ui.button("+ keyframe на курсоре").clicked() {
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
                    }
                } else {
                    ui.weak("Цепочка пуста — добавь эффект");
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Готово").clicked() {
                        ed.open = false;
                    }
                    if ui.button("Отмена (вернуть как было)").clicked() {
                        fx = ed.backup.clone();
                        changed = true;
                        ed.open = false;
                    }
                });
            });
        if !open {
            ed.open = false;
        }
        if changed {
            self.set_fx_coalesced(ed.clip, fx, ed.gesture);
        }
    }
}
