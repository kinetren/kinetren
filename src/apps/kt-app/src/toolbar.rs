//! Плавающий тулбар поверх таймлайна (порт TimelineToolChrome):
//! полупрозрачная градиентная полоса с тенью, кнопки 30×28, подсказки
//! дублируются в статусбар.

use crate::app::KtApp;
use eframe::egui;
use kt_model::ClipKind;
use kt_time::Tick;

pub fn gradient_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    top: egui::Color32,
    bottom: egui::Color32,
) {
    let mut mesh = egui::Mesh::default();
    let i = mesh.vertices.len() as u32;
    for (pos, col) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.right_bottom(), bottom),
        (rect.left_bottom(), bottom),
    ] {
        mesh.vertices.push(egui::epaint::Vertex { pos, uv: egui::epaint::WHITE_UV, color: col });
    }
    mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    painter.add(egui::Shape::mesh(mesh));
}

impl KtApp {
    /// Кнопка тулбара: фиксированный размер, ховер-подсказка + статус-хинт.
    fn tb(&mut self, ui: &mut egui::Ui, label: &str, hint: &str) -> bool {
        let r =
            ui.add_sized([30.0, 26.0], egui::Button::new(label).frame(false)).on_hover_text(hint);
        if r.hovered() {
            self.hover_hint = Some(hint.to_string());
        }
        r.clicked()
    }

    fn tb_toggle(&mut self, ui: &mut egui::Ui, on: bool, label: &str, hint: &str) -> bool {
        let r = ui.add_sized([30.0, 26.0], egui::Button::selectable(on, label)).on_hover_text(hint);
        if r.hovered() {
            self.hover_hint = Some(hint.to_string());
        }
        r.clicked()
    }

    /// Рисуется ПОВЕРХ таймлайна (вызвать после draw_timeline).
    pub fn draw_toolbar_chrome(&mut self, ui: &mut egui::Ui, timeline_rect: egui::Rect) {
        let chrome = egui::Rect::from_min_size(
            egui::pos2(timeline_rect.left() + 58.0, timeline_rect.top() + 40.0),
            egui::vec2((timeline_rect.width() - 76.0).min(760.0), 32.0),
        );
        let painter = ui.painter();
        // тень
        painter.rect_filled(
            chrome.translate(egui::vec2(0.0, 2.5)).expand(1.5),
            8.0,
            egui::Color32::from_black_alpha(70),
        );
        // градиентная плашка
        gradient_rect(
            painter,
            chrome,
            egui::Color32::from_rgba_unmultiplied(38, 41, 46, 235),
            egui::Color32::from_rgba_unmultiplied(24, 26, 30, 235),
        );
        painter.rect(
            chrome,
            8.0,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18)),
            egui::StrokeKind::Inside,
        );

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(chrome.shrink2(egui::vec2(8.0, 2.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let ui = &mut child;
        let playing = self.player.transport.is_playing();
        if self.tb(ui, if playing { "⏸" } else { "▶" }, "Play/Pause (Space)") {
            if playing {
                self.player.pause();
            } else {
                self.player.play();
            }
        }
        if self.tb(ui, "⏮", "В начало") {
            self.player.seek(Tick(0));
        }
        ui.separator();
        if self.tb(ui, "🚩", "Маркер на курсоре (M)") {
            self.marker_toggle_at_cursor();
        }
        if self.tb(ui, "+V", "Добавить видео-дорожку") {
            self.add_track(ClipKind::Video);
        }
        if self.tb(ui, "+A", "Добавить аудио-дорожку") {
            self.add_track(ClipKind::Audio);
        }
        ui.separator();
        if self.tb(ui, "✂", "Разрез / вырез региона (S)") {
            self.cut_action();
        }
        if self.tb(ui, "🗐", "Копировать (Ctrl+C)") {
            self.copy_selection();
        }
        if self.tb(ui, "📋", "Вставить на курсор (Ctrl+V)") {
            self.paste_at_cursor();
        }
        if self.tb(ui, "🗑", "Удалить (Del)") {
            self.delete_action();
        }
        if self.tb(ui, "⇤", "Трим начала к курсору") {
            self.trim_selected_start_to_cursor();
        }
        if self.tb(ui, "⇥", "Трим конца к курсору") {
            self.trim_selected_end_to_cursor();
        }
        if self.tb(ui, "⬆", "На дорожку выше") {
            self.move_selected_lane(-1);
        }
        if self.tb(ui, "⬇", "На дорожку ниже") {
            self.move_selected_lane(1);
        }
        ui.separator();
        if self.tb(ui, "-", "Отдалить (Ctrl+колесо)") {
            self.ticks_per_px = (self.ticks_per_px * 1.25).clamp(30.0, 90_000.0);
        }
        if self.tb(ui, "+", "Приблизить") {
            self.ticks_per_px = (self.ticks_per_px * 0.8).clamp(30.0, 90_000.0);
        }
        ui.separator();
        if self.tb(ui, "⌊", "Регион: начало от курсора (I)") {
            self.set_region_edge_from_cursor(true);
        }
        if self.tb(ui, "⌋", "Регион: конец от курсора (O)") {
            self.set_region_edge_from_cursor(false);
        }
        if self.tb(ui, "✖", "Снять регион") {
            self.clear_region();
        }
        let loop_on = self.model.project().loop_enabled;
        if self.tb_toggle(ui, loop_on, "🔁", "Loop региона (L)") {
            self.toggle_loop();
        }
        ui.separator();
        if self.tb(ui, "↶", "Отменить (Ctrl+Z)") {
            self.undo();
        }
        if self.tb(ui, "↷", "Вернуть (Ctrl+Shift+Z)") {
            self.redo();
        }
        ui.separator();
        if self.tb(ui, "🎬", "Рендер в MP4 (регион или весь проект)") {
            self.start_render();
        }
        if self.tb(ui, if self.dirty { "💾●" } else { "💾" }, "Сохранить (Ctrl+S)") {
            self.save();
        }
    }
}
