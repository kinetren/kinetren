//! Окно редактора morpher'а (порт MorpherEditWindow v1, sourceeditwidgets):
//! показывает замороженные цели; для Replace — выбор нового источника из
//! медиа проекта + сдвиг source-in (delta или absolute — фикс D4 v1); для
//! FX-батча — редактирование двух цепочек (видео/аудио), действующих LIVE
//! (фикс D1 v1). Все правки — apply_coalesced(SetMorpher): вся сессия
//! окна = один undo-шаг. Автоматизация в батч-цепочках не редактируется —
//! после bake доступен полный FX-редактор клипа.

use crate::i18n::{k, tr};
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
        // камеры для комбо Pan3d в FX-батче (Р.29)
        let cameras: Vec<(u64, String)> = self
            .model
            .project()
            .cameras
            .iter()
            .map(|c| {
                (c.id.0, if c.name.is_empty() { format!("C{}", c.id.0) } else { c.name.clone() })
            })
            .collect();
        let title = if is_batch {
            crate::tf!(k::mrph::TITLE_BATCH, ed.id.0)
        } else {
            crate::tf!(k::mrph::TITLE, ed.id.0)
        };
        let mut ev = orig.clone();
        let mut changed = false;
        let mut bake = false;
        let mut delete = false;
        let mut open = ed.open;
        // выключатель события (Р.33) — та же мастер-галка, что у цепочки
        // события; буфер FX общий с окном событий
        let mut enabled = ev.enabled;
        let mut fx_clip = self.fx_clipboard.clone();
        // подсказки — в статусбар, как везде (попапов в программе нет)
        let mut hover_out: Option<String> = None;
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
                    egui::RichText::new(crate::tf!(
                        k::mrph::FROZEN,
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
                            .unwrap_or_else(|| tr(k::mrph::KEEP_SOURCE).to_string());
                        ui.horizontal(|ui| {
                            ui.label(tr(k::mrph::NEW_SOURCE));
                            egui::ComboBox::from_id_salt(("morph_src", ed.id.0))
                                .width(240.0)
                                .selected_text(short_name(&cur_name))
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(media.is_none(), tr(k::mrph::KEEP_SOURCE))
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
                                tr(k::mrph::NOT_IN_BIN),
                            )
                            .weak()
                            .small(),
                        );
                        ui.separator();
                        // --- сдвиг source-in (delta / absolute, D4) ---------
                        let mut absolute = absolute_source_in.is_some();
                        if ui
                            .checkbox(&mut absolute, tr(k::mrph::ABSOLUTE_IN))
                            .changed()
                        {
                            *absolute_source_in =
                                if absolute { Some(Tick(0)) } else { None };
                            changed = true;
                        }
                        if let Some(abs) = absolute_source_in {
                            let mut ms = abs.0 / MS;
                            ui.horizontal(|ui| {
                                ui.label(tr(k::mrph::SOURCE_IN_MS));
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
                                ui.label(tr(k::mrph::SOURCE_IN_DELTA_MS));
                                if ui.add(egui::DragValue::new(&mut ms).speed(10)).changed() {
                                    *source_in_delta = Tick(ms * MS);
                                    changed = true;
                                }
                            });
                        }
                    }
                    MorphKind::EffectBatch {
                        video_fx,
                        audio_fx,
                        video_bypass,
                        audio_bypass,
                    } => {
                        ui.label(
                            egui::RichText::new(
                                tr(k::mrph::LIVE_NOTE),
                            )
                            .weak()
                            .small(),
                        );
                        // ОДИН виджет цепочки на события и морферы (Р.33):
                        // галки, крестики, перестановка мышью, мастер-галка
                        // и копипаст — одинаково в обоих местах
                        changed |= chain_section(
                            ui,
                            (ed.id.0, true),
                            tr(k::mrph::VIDEO_FX),
                            video_fx,
                            video_bypass,
                            &mut ed.sel.0,
                            true,
                            &mut ed.picker_query,
                            &cameras,
                            &mut enabled,
                            &mut fx_clip,
                            &mut hover_out,
                        );
                        ui.separator();
                        changed |= chain_section(
                            ui,
                            (ed.id.0, false),
                            tr(k::mrph::AUDIO_FX),
                            audio_fx,
                            audio_bypass,
                            &mut ed.sel.1,
                            false,
                            &mut ed.picker_query,
                            &cameras,
                            &mut enabled,
                            &mut fx_clip,
                            &mut hover_out,
                        );
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .button(if is_batch {
                            tr(k::mrph::BAKE_BATCH)
                        } else {
                            tr(k::mrph::BAKE_MORPH)
                        })
                        .clicked()
                    {
                        bake = true;
                    }
                    if ui.button(tr(k::mrph::REMOVE)).clicked() {
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
        self.fx_clipboard = fx_clip;
        if let Some(h) = hover_out {
            self.hover_hint = Some(h);
        }
        ev.enabled = enabled;
        changed |= ev.enabled != orig.enabled;
        if changed && ev != orig {
            self.apply_coalesced(Command::SetMorpher(ev), ed.gesture);
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
#[allow(clippy::too_many_arguments)]
fn chain_section(
    ui: &mut egui::Ui,
    win_id: (u64, bool),
    title: &str,
    fx: &mut Vec<FxInstance>,
    bypass: &mut Vec<bool>,
    sel: &mut usize,
    video: bool,
    query: &mut String,
    cameras: &[(u64, String)],
    enabled: &mut bool,
    clipboard: &mut Option<kt_model::ops::FxChainBlock>,
    hover: &mut Option<String>,
) -> bool {
    let mut changed = false;
    ui.label(egui::RichText::new(title).strong());
    // морфер не хранит кривых: у батча автоматизации нет (она живёт на
    // событии), поэтому auto = None
    let ch = crate::widgets::chain_strip(
        ui,
        win_id,
        crate::widgets::ChainEdit {
            fx,
            bypass,
            auto: None,
            selected: sel,
            master_on: enabled,
        },
        !video,
        query,
        clipboard,
        kt_time::Tick(0),
        hover,
    );
    changed |= ch.any();
    if let Some(f) = fx.get_mut(*sel) {
        // те же параметры тем же кодом, что в окне событий (Р.33):
        // раньше здесь была вторая форма со СВОИМИ диапазонами
        changed |= crate::fx_editor::draw_video_fx_params(ui, f, cameras);
    }
    changed
}
