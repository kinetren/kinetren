//! Окно редактора morpher'а (порт MorpherEditWindow v1, sourceeditwidgets):
//! показывает замороженные цели; для Replace — выбор нового источника из
//! медиа проекта + сдвиг source-in (delta или absolute — фикс D4 v1); для
//! FX-батча — редактирование двух цепочек (видео/аудио), действующих LIVE
//! (фикс D1 v1). Все правки — apply_coalesced(SetMorpher): вся сессия
//! окна = один undo-шаг. Автоматизация в батч-цепочках не редактируется —
//! после bake доступен полный FX-редактор клипа.

use crate::app::KtApp;
use eframe::egui;
use kt_model::*;
use kt_time::{TICK_HZ, Tick};

pub struct MorphEditor {
    pub id: MorphId,
    pub picker_query: String,
    pub gesture: u64,
    pub open: bool,
    /// Выбранные слоты цепочек батча (видео, аудио).
    pub sel: (usize, usize),
}

const MS: i64 = TICK_HZ / 1000; // тиков в мс

impl KtApp {
    pub fn open_morph_editor(&mut self, id: MorphId) {
        if self.morph_editors.iter().any(|e| e.id == id) {
            return;
        }
        self.gesture_counter += 1;
        self.morph_editors.push(MorphEditor {
            id,
            picker_query: String::new(),
            gesture: self.gesture_counter,
            open: true,
            sel: (0, 0),
        });
    }

    pub fn draw_morph_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.morph_editors);
        for ed in &mut editors {
            self.draw_one_morph_editor(ctx, ed);
        }
        editors.retain(|e| e.open);
        self.morph_editors = editors;
    }

    #[allow(clippy::too_many_lines)]
    fn draw_one_morph_editor(&mut self, ctx: &egui::Context, ed: &mut MorphEditor) {
        // событие исчезло (undo/удаление) — окно закрывается само (v1)
        let Some(orig) = self.model.project().morphers.iter().find(|m| m.id == ed.id).cloned()
        else {
            ed.open = false;
            return;
        };
        let is_batch = matches!(orig.kind, MorphKind::EffectBatch { .. });
        let title = if is_batch {
            format!("Morpher FX-батч — событие {}", ed.id.0)
        } else {
            format!("Morpher — событие {}", ed.id.0)
        };
        let mut ev = orig.clone();
        let mut changed = false;
        let mut bake = false;
        let mut delete = false;
        let mut open = ed.open;
        egui::Window::new(title)
            .id(egui::Id::new(("morph", ed.id.0)))
            .open(&mut open)
            .default_size(if is_batch { [560.0, 480.0] } else { [420.0, 260.0] })
            .min_size([380.0, 200.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                // --- замороженные цели (v1: morpherTargetsDescription) -------
                let mut ids: Vec<String> =
                    ev.targets.iter().take(4).map(|t| format!("#{}", t.0)).collect();
                if ev.targets.len() > 4 {
                    ids.push("…".into());
                }
                ui.label(
                    egui::RichText::new(format!(
                        "Заморожено при создании: {} клип(ов). {}",
                        ev.targets.len(),
                        ids.join(", ")
                    ))
                    .weak(),
                );
                ui.separator();

                match &mut ev.kind {
                    MorphKind::Replace { media, source_in_delta, absolute_source_in } => {
                        // --- подмена источника ------------------------------
                        let media_list: Vec<(MediaId, String)> = self
                            .model
                            .project()
                            .media
                            .iter()
                            .map(|m| (m.id, m.path.clone()))
                            .collect();
                        let cur_name = media
                            .and_then(|id| {
                                media_list.iter().find(|(i, _)| *i == id).map(|(_, p)| p.clone())
                            })
                            .unwrap_or_else(|| "— не менять источник —".to_string());
                        ui.horizontal(|ui| {
                            ui.label("Новый источник:");
                            egui::ComboBox::from_id_salt(("morph_src", ed.id.0))
                                .width(240.0)
                                .selected_text(short_name(&cur_name))
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(media.is_none(), "— не менять источник —")
                                        .clicked()
                                    {
                                        *media = None;
                                        changed = true;
                                    }
                                    for (mid, path) in &media_list {
                                        if ui
                                            .selectable_label(
                                                *media == Some(*mid),
                                                short_name(path),
                                            )
                                            .clicked()
                                        {
                                            *media = Some(*mid);
                                            changed = true;
                                        }
                                    }
                                });
                        });
                        ui.label(
                            egui::RichText::new(
                                "Файл не из проекта? Перетащи его в бин — он появится в списке.",
                            )
                            .weak()
                            .small(),
                        );
                        ui.separator();
                        // --- сдвиг source-in (delta / absolute, D4) ---------
                        let mut absolute = absolute_source_in.is_some();
                        if ui
                            .checkbox(&mut absolute, "Абсолютный source-in (вместо сдвига)")
                            .changed()
                        {
                            *absolute_source_in =
                                if absolute { Some(Tick(0)) } else { None };
                            changed = true;
                        }
                        if let Some(abs) = absolute_source_in {
                            let mut ms = abs.0 / MS;
                            ui.horizontal(|ui| {
                                ui.label("Source-in, мс:");
                                if ui
                                    .add(egui::DragValue::new(&mut ms).speed(10).range(0..=i64::MAX))
                                    .changed()
                                {
                                    *abs = Tick(ms * MS);
                                    changed = true;
                                }
                            });
                        } else {
                            let mut ms = source_in_delta.0 / MS;
                            ui.horizontal(|ui| {
                                ui.label("Сдвиг source-in, мс:");
                                if ui.add(egui::DragValue::new(&mut ms).speed(10)).changed() {
                                    *source_in_delta = Tick(ms * MS);
                                    changed = true;
                                }
                            });
                        }
                    }
                    MorphKind::EffectBatch { video_fx, audio_fx } => {
                        ui.label(
                            egui::RichText::new(
                                "Эффекты действуют сразу (live); bake впишет их в клипы.",
                            )
                            .weak()
                            .small(),
                        );
                        changed |= chain_section(
                            ui,
                            ed.id.0,
                            "Видео-FX (допишутся видео-клипам)",
                            video_fx,
                            &mut ed.sel.0,
                            true,
                            &mut ed.picker_query,
                        );
                        ui.separator();
                        changed |= chain_section(
                            ui,
                            ed.id.0,
                            "Аудио-FX (допишутся аудио-клипам)",
                            audio_fx,
                            &mut ed.sel.1,
                            false,
                            &mut ed.picker_query,
                        );
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .button(if is_batch {
                            "Bake FX-батча"
                        } else {
                            "Bake морфинга"
                        })
                        .clicked()
                    {
                        bake = true;
                    }
                    if ui.button("Удалить morpher").clicked() {
                        delete = true;
                    }
                });
            });
        ed.open = open;

        if bake {
            if self.apply(Command::BakeMorpher { id: ed.id }) {
                self.selected_morph = None;
            }
            ed.open = false;
            return;
        }
        if delete {
            if self.apply(Command::RemoveMorpher { id: ed.id }) {
                self.selected_morph = None;
            }
            ed.open = false;
            return;
        }
        if changed && ev != orig {
            self.model_rev += 1;
            if self.model.apply_coalesced(Command::SetMorpher(ev), ed.gesture).is_ok() {
                self.player.set_project(self.model.project().clone());
                self.dirty = true;
            }
        }
    }
}

fn short_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Секция одной цепочки батча: полоса слотов + добавление + параметры
/// выбранного слота. Возвращает true при изменении.
fn chain_section(
    ui: &mut egui::Ui,
    win_id: u64,
    title: &str,
    fx: &mut Vec<FxInstance>,
    sel: &mut usize,
    video: bool,
    query: &mut String,
) -> bool {
    let mut changed = false;
    ui.label(egui::RichText::new(title).strong());
    ui.horizontal_wrapped(|ui| {
        let mut remove: Option<usize> = None;
        for (i, f) in fx.iter().enumerate() {
            if ui
                .selectable_label(*sel == i, crate::fx_editor::fx_display_name(f))
                .clicked()
            {
                *sel = i;
            }
            if ui.small_button("×").clicked() {
                remove = Some(i);
            }
            if i + 1 < fx.len() {
                ui.label(crate::icons::CARET_RIGHT);
            }
        }
        if let Some(i) = remove {
            fx.remove(i);
            *sel = (*sel).min(fx.len().saturating_sub(1));
            changed = true;
        }
        // единый пикер с поиском (п.3 v1.4)
        if let Some(tmpl) =
            crate::fx_registry::fx_picker(ui, ("morph_add", win_id, video), !video, query)
        {
            fx.push(tmpl);
            *sel = fx.len() - 1;
            changed = true;
        }
    });
    if let Some(f) = fx.get_mut(*sel) {
        match f {
            FxInstance::Gain { db } => {
                ui.horizontal(|ui| {
                    ui.label("Gain, dB:");
                    changed |= ui
                        .add(egui::DragValue::new(db).speed(0.1).range(-48.0..=24.0))
                        .changed();
                });
            }
            FxInstance::SlowedReverb { rate, mix, reverb_delay_ms, damp, decay, wet_gain, .. } => {
                egui::Grid::new(("morph_sr", win_id, video)).num_columns(2).show(ui, |ui| {
                    ui.label("Rate:");
                    changed |= ui
                        .add(egui::DragValue::new(rate).speed(0.01).range(0.25..=4.0))
                        .changed();
                    ui.end_row();
                    ui.label("Mix:");
                    changed |=
                        ui.add(egui::DragValue::new(mix).speed(0.01).range(0.0..=1.0)).changed();
                    ui.end_row();
                    ui.label("Delay, мс:");
                    changed |= ui
                        .add(egui::DragValue::new(reverb_delay_ms).speed(1.0).range(1.0..=500.0))
                        .changed();
                    ui.end_row();
                    ui.label("Damp:");
                    changed |=
                        ui.add(egui::DragValue::new(damp).speed(0.01).range(0.0..=1.0)).changed();
                    ui.end_row();
                    ui.label("Decay:");
                    changed |=
                        ui.add(egui::DragValue::new(decay).speed(0.01).range(0.0..=0.98)).changed();
                    ui.end_row();
                    ui.label("Wet gain:");
                    changed |= ui
                        .add(egui::DragValue::new(wet_gain).speed(0.01).range(0.0..=4.0))
                        .changed();
                    ui.end_row();
                });
            }
            other => {
                changed |= crate::fx_editor::draw_video_fx_params(ui, other);
            }
        }
    }
    changed
}
