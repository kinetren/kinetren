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

    /// Полоса кнопок ВНИЗУ панели таймлайна (порт: кнопки снизу),
    /// градиентный фон на всю ширину — ничего не вылезает.
    pub fn draw_toolbar_strip(&mut self, ui: &mut egui::Ui) {
        let strip = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), 34.0),
        );
        let painter = ui.painter();
        gradient_rect(
            painter,
            strip,
            egui::Color32::from_rgb(40, 43, 48),
            egui::Color32::from_rgb(26, 28, 32),
        );
        painter.line_segment(
            [strip.left_top(), strip.right_top()],
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(strip.shrink2(egui::vec2(8.0, 2.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        ui.advance_cursor_after_rect(strip);
        let ui = &mut child;
        let playing = self.player.transport.is_playing();
        if self.tb(
            ui,
            if playing { crate::icons::PAUSE } else { crate::icons::PLAY },
            "Play/Pause (Space)",
        ) {
            self.on_transport_toggle();
        }
        if self.tb(ui, crate::icons::SKIP_BACK, "В начало") {
            self.player.seek(Tick(0));
        }
        ui.separator();
        if self.tb(ui, crate::icons::FLAG, "Маркер на курсоре (M)") {
            self.marker_toggle_at_cursor();
        }
        if self.tb(ui, "+V", "Добавить видео-дорожку") {
            self.add_track(ClipKind::Video);
        }
        if self.tb(ui, "+A", "Добавить аудио-дорожку") {
            self.add_track(ClipKind::Audio);
        }
        ui.separator();
        if self.tb(ui, crate::icons::SCISSORS, "Разрез / вырез региона (S)") {
            self.cut_action();
        }
        if self.tb(ui, crate::icons::COPY, "Копировать (Ctrl+C)") {
            self.copy_selection();
        }
        if self.tb(ui, crate::icons::CLIPBOARD, "Вставить на курсор (Ctrl+V)") {
            self.paste_at_cursor();
        }
        if self.tb(ui, crate::icons::TRASH, "Удалить (Del)") {
            self.delete_action();
        }
        if self.tb(ui, crate::icons::TRIM_START, "Трим начала к курсору") {
            self.trim_selected_start_to_cursor();
        }
        if self.tb(ui, crate::icons::TRIM_END, "Трим конца к курсору") {
            self.trim_selected_end_to_cursor();
        }
        if self.tb(ui, crate::icons::LANE_UP, "На дорожку выше") {
            self.move_selected_lane(-1);
        }
        if self.tb(ui, crate::icons::LANE_DOWN, "На дорожку ниже") {
            self.move_selected_lane(1);
        }
        ui.separator();
        if self.tb(ui, crate::icons::MINUS, "Отдалить (Ctrl+колесо)") {
            self.follow_playhead = false; // ручной зум (п.14 р.6)
            self.ticks_per_px = (self.ticks_per_px * 1.25).clamp(30.0, 90_000.0);
        }
        if self.tb(ui, crate::icons::PLUS, "Приблизить") {
            self.follow_playhead = false;
            self.ticks_per_px = (self.ticks_per_px * 0.8).clamp(30.0, 90_000.0);
        }
        ui.separator();
        if self.tb(ui, crate::icons::REGION_IN, "Регион: начало от курсора (I)")
        {
            self.set_region_edge_from_cursor(true);
        }
        if self.tb(ui, crate::icons::REGION_OUT, "Регион: конец от курсора (O)")
        {
            self.set_region_edge_from_cursor(false);
        }
        if self.tb(ui, crate::icons::CLEAR, "Снять регион") {
            self.clear_region();
        }
        let loop_on = self.model.project().loop_enabled;
        if self.tb_toggle(ui, loop_on, crate::icons::LOOP, "Loop региона (L)") {
            self.toggle_loop();
        }
        ui.separator();
        if self.tb(ui, crate::icons::UNDO, "Отменить (Ctrl+Z)") {
            self.undo();
        }
        if self.tb(ui, crate::icons::REDO, "Вернуть (Ctrl+Shift+Z)") {
            self.redo();
        }
        ui.separator();
        if self.tb(ui, crate::icons::RENDER, "Рендер в MP4 (регион или весь проект)")
        {
            self.start_render();
        }
        let save_label = if self.dirty {
            format!("{}●", crate::icons::SAVE)
        } else {
            crate::icons::SAVE.to_string()
        };
        if self.tb(ui, &save_label, "Сохранить (Ctrl+S)") {
            self.save();
        }
    }
}
