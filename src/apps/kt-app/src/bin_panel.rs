//! Медиа-бин (порт SourceAreaWidget): список медиа проекта, дроп из ОС,
//! click-to-arm размещение на таймлайн, удаление, офлайн-бейдж.

use crate::app::KtApp;
use eframe::egui;
use kt_model::{Command, MediaId};

impl KtApp {
    pub fn draw_bin(&mut self, ui: &mut egui::Ui) {
        ui.label("Медиа (дроп файлов сюда/на окно; клик — поставить на курсор)");
        ui.add_space(2.0);
        let offline = self.offline.lock().unwrap().clone();
        let media: Vec<(MediaId, String)> =
            self.model.project().media.iter().map(|m| (m.id, m.path.clone())).collect();
        let mut to_place: Option<MediaId> = None;
        let mut to_remove: Option<MediaId> = None;
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            for (id, path) in &media {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                ui.horizontal(|ui| {
                    let is_off = offline.contains(id);
                    let label = if is_off { format!("⚠ {name}") } else { name.clone() };
                    let resp = ui
                        .add(egui::Label::new(label).sense(egui::Sense::click()))
                        .on_hover_text(path.clone());
                    if resp.clicked() {
                        to_place = Some(*id);
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Поставить на курсор").clicked() {
                            to_place = Some(*id);
                            ui.close();
                        }
                        if ui.button("Убрать из проекта").clicked() {
                            to_remove = Some(*id);
                            ui.close();
                        }
                    });
                });
            }
            if media.is_empty() {
                ui.weak("пусто — перетащи медиафайлы на окно");
            }
        });
        if let Some(id) = to_place {
            self.place_media_at_cursor(id);
        }
        if let Some(id) = to_remove
            && !self.apply(Command::RemoveMedia { id })
        {
            // ошибка уже в статусе (MediaInUse)
        }
    }
}
