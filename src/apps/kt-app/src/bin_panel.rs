//! Медиа-бин (порт SourceAreaWidget + п.11 р.6): карточки с превью,
//! отсмотр «возюканьем» мыши по превью (hover x → момент файла → тумба из
//! общего кэша/sidecar-БД), имя + длительность; клик — поставить на курсор.
//!
//! Уроки CLAUDE.md: repaint при pending-тумбе — request_repaint_after,
//! не безусловный; текстуры — общие thumb_textures/LRU приложения;
//! «last good» на медиа — превью не мигает при недогруженных бакетах.

use crate::app::KtApp;
use crate::thumbs::{THUMB_STEP_LADDER, ThumbKey, ThumbState, thumb_key_t, thumb_source_step};
use eframe::egui;
use kt_model::{Command, MediaId};

const PREVIEW_W: f32 = 128.0;
const PREVIEW_H: f32 = 72.0;
/// Логических позиций скраба на файл (п.14 р.7: было 8 — «слишком мало
/// превьюшек»); тумбы филмстрипа подхватываются near-поиском в БД.
const BIN_SCRUB_SLOTS: f32 = 48.0;

impl KtApp {
    pub fn draw_bin(&mut self, ui: &mut egui::Ui) {
        ui.label("Медиа (дроп файлов сюда/на окно; клик — поставить на курсор)");
        ui.add_space(2.0);
        let offline = self.offline.lock().unwrap().clone();
        // offline→online: негативные кэши тумб устарели (п.1 р.7)
        for id in self.bin_last_offline.difference(&offline) {
            self.thumbs.invalidate_media(*id);
        }
        self.bin_last_offline = offline.clone();
        let media: Vec<(MediaId, String)> =
            self.model.project().media.iter().map(|m| (m.id, m.path.clone())).collect();
        let mut to_place: Option<MediaId> = None;
        let mut to_remove: Option<MediaId> = None;
        let mut pending = false;
        // п.12 р.7: бин занимает ВСЮ ширину панели (auto_shrink по
        // умолчанию ужимал по контенту — справа была пустота);
        // п.13: карточки в grid (horizontal_wrapped, колонки от ширины)
        egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, true]).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for (id, path) in &media {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                let is_synth = path.starts_with("synth:");
                let is_off = offline.contains(id);
                ui.vertical(|ui| {
                    ui.set_width(PREVIEW_W);
                    // --- превью со скрабом -------------------------------
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(PREVIEW_W, PREVIEW_H),
                        egui::Sense::click(),
                    );
                    let p = ui.painter_at(rect);
                    p.rect_filled(rect, 2.0, egui::Color32::from_rgb(10, 11, 13));
                    if !is_synth && !is_off {
                        let dur = self.thumbs.duration_or_probe(*id);
                        // момент: hover-x → доля файла; без hover — постер
                        let (t, tol) = match (resp.hover_pos(), dur) {
                            (Some(pos), Some(d)) if d.0 > 0 => {
                                let frac = ((pos.x - rect.left()) / rect.width())
                                    .clamp(0.0, 0.999);
                                let step = thumb_source_step(
                                    d,
                                    BIN_SCRUB_SLOTS * PREVIEW_W,
                                    PREVIEW_W,
                                );
                                (
                                    thumb_key_t(0, (frac as f64 * d.0 as f64) as i64, step),
                                    step / 2,
                                )
                            }
                            _ => (0, THUMB_STEP_LADDER[0] / 2),
                        };
                        let key = ThumbKey { media: *id, t };
                        let tex = self.thumb_texture(ui, key);
                        let shown = match &tex {
                            Some(t) => Some(t.clone()),
                            None => {
                                // Failed НЕ перезаказываем и НЕ держим
                                // repaint-цикл (п.1 р.7)
                                if self.thumbs.state(&key) != ThumbState::Failed {
                                    self.thumbs.request_near(key, tol);
                                    pending = true;
                                }
                                // не мигаем: последний удачный кадр медиа
                                let last = self.bin_last_good.get(id).copied();
                                last.and_then(|k| self.thumb_texture(ui, k))
                            }
                        };
                        if tex.is_some() {
                            self.bin_last_good.insert(*id, key);
                        }
                        if let Some(t) = shown {
                            p.image(
                                t.id(),
                                rect.shrink(1.0),
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        }
                        // полоса-скраб позиции под курсором
                        if let (Some(pos), Some(d)) = (resp.hover_pos(), dur)
                            && d.0 > 0
                        {
                            p.line_segment(
                                [
                                    egui::pos2(pos.x, rect.top() + 1.0),
                                    egui::pos2(pos.x, rect.bottom() - 1.0),
                                ],
                                egui::Stroke::new(
                                    1.0,
                                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 170),
                                ),
                            );
                        }
                    } else {
                        p.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            if is_off { crate::icons::WARN } else { "♪" },
                            egui::FontId::proportional(18.0),
                            egui::Color32::from_gray(120),
                        );
                    }
                    if resp.clicked() {
                        to_place = Some(*id);
                    }
                    resp.clone().context_menu(|ui| {
                        if ui.button("Поставить на курсор").clicked() {
                            to_place = Some(*id);
                            ui.close();
                        }
                        if ui.button("Убрать из проекта").clicked() {
                            to_remove = Some(*id);
                            ui.close();
                        }
                    });
                    // --- имя + длительность (под превью) -----------------
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if let Some(d) = self.thumbs.duration(*id) {
                            let s = d.0 / 90_000;
                            ui.monospace(
                                egui::RichText::new(format!(
                                    "{}:{:02}:{:02}",
                                    s / 3600,
                                    s % 3600 / 60,
                                    s % 60
                                ))
                                .size(11.0)
                                .weak(),
                            );
                        }
                        let label = if is_off {
                            format!("{} {name}", crate::icons::WARN)
                        } else {
                            name.clone()
                        };
                        let r = ui.add(
                            egui::Label::new(egui::RichText::new(label).size(12.0)).truncate(),
                        );
                        self.hint(r, path);
                    });
                });
            }
            if media.is_empty() {
                ui.weak("пусто — перетащи медиафайлы на окно");
            }
            });
        });
        if pending {
            // тумба грузится — мягкий ретрай, не 60 fps
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
        if let Some(id) = to_place {
            self.place_media_at_cursor(id);
        }
        if let Some(id) = to_remove
            && !self.apply(Command::RemoveMedia { id })
        {
            // ошибка уже в статусе (MediaInUse)
        }
    }

    /// Тумба как egui-текстура из ОБЩЕГО кэша приложения (тот же путь,
    /// что у филмстрипа таймлайна): get → load_texture → LRU.
    fn thumb_texture(
        &mut self,
        ui: &egui::Ui,
        key: ThumbKey,
    ) -> Option<egui::TextureHandle> {
        if let Some(t) = self.thumb_textures.get(&key) {
            return Some(t.clone());
        }
        let img = self.thumbs.get(&key)?;
        let tex = ui.ctx().load_texture(
            format!("thumb-{}-{}", key.media.0, key.t),
            egui::ImageData::from((*img).clone()),
            egui::TextureOptions::LINEAR,
        );
        self.thumb_textures.insert(key, tex.clone());
        self.thumb_lru.retain(|k| k != &key);
        self.thumb_lru.push_back(key);
        while self.thumb_lru.len() > crate::thumbs::MAX_READY {
            if let Some(old) = self.thumb_lru.pop_front() {
                self.thumb_textures.remove(&old);
            }
        }
        Some(tex)
    }
}
