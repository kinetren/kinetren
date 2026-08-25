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

/// Кнопка окна borderless-титлбара, ИКОНКИ РИСУЮТСЯ painter'ом (п.22 р.6:
/// шрифтовые глифы ✕/❐ рендерились «квадратиками»-tofu). kind: 0=свернуть,
/// 1=развернуть/восстановить, 2=закрыть (красный ховер).
fn window_button(ui: &mut egui::Ui, kind: u8, maximized: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 22.0), egui::Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        let fill = if kind == 2 {
            egui::Color32::from_rgb(178, 44, 44)
        } else {
            crate::theme::HOVER
        };
        p.rect_filled(rect, 0.0, fill);
    }
    let c = rect.center();
    let s = egui::Stroke::new(1.2, egui::Color32::from_gray(210));
    match kind {
        0 => {
            p.line_segment([c + egui::vec2(-4.5, 3.0), c + egui::vec2(4.5, 3.0)], s);
        }
        1 if maximized => {
            // «восстановить»: два наложенных квадрата
            let r0 = egui::Rect::from_center_size(c + egui::vec2(1.5, -1.5), egui::vec2(7.0, 7.0));
            let r1 = egui::Rect::from_center_size(c + egui::vec2(-1.5, 1.5), egui::vec2(7.0, 7.0));
            p.rect_stroke(r0, 0.0, s, egui::StrokeKind::Middle);
            p.rect_filled(r1, 0.0, crate::theme::WINDOW);
            p.rect_stroke(r1, 0.0, s, egui::StrokeKind::Middle);
        }
        1 => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(8.5, 8.5)),
                0.0,
                s,
                egui::StrokeKind::Middle,
            );
        }
        _ => {
            p.line_segment([c + egui::vec2(-4.0, -4.0), c + egui::vec2(4.0, 4.0)], s);
            p.line_segment([c + egui::vec2(-4.0, 4.0), c + egui::vec2(4.0, -4.0)], s);
        }
    }
    resp
}

impl KtApp {
    /// Кнопки окна borderless — ПЛАВАЮЩИЙ оверлей в правом верхнем углу
    /// (п.22 р.6, уточнение: никакой полосы-титлбара на всю ширину —
    /// место не тратим). Драг — за тонкую верхнюю кромку (app.rs) и за
    /// нижний тулбар.
    pub fn draw_window_overlay(&mut self, ctx: &egui::Context) {
        let win = ctx.content_rect();
        egui::Area::new(egui::Id::new("win-buttons"))
            .fixed_pos(egui::pos2(win.right() - 3.0 * 34.0 - 2.0, win.top()))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.horizontal(|ui| {
                    // лёгкая подложка — кнопки читаются на любом контенте
                    let bg = egui::Rect::from_min_size(
                        ui.next_widget_position(),
                        egui::vec2(3.0 * 34.0, 22.0),
                    );
                    ui.painter().rect_filled(bg, 0.0, egui::Color32::from_black_alpha(110));
                    if self.hint(window_button(ui, 0, false), "Свернуть").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    let hint = if max { "Восстановить" } else { "Развернуть" };
                    if self.hint(window_button(ui, 1, max), hint).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                    }
                    if self.hint(window_button(ui, 2, false), "Закрыть").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
    }

    /// Меню-ряд (п.3 р.7): Файл/Правка/Вид в ТОПе левой колонки — жирная
    /// зона драга окна вместо неюзабельной тонкой полоски; полоса на всю
    /// ширину окна не возвращается (п.4: центр сверху — превью).
    pub fn draw_menubar(&mut self, ui: &mut egui::Ui) {
        let strip = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), 22.0),
        );
        // драг окна за пустую часть ряда (кнопки меню перекрывают хит)
        let bg = ui.interact(strip, ui.id().with("menubar-drag"), egui::Sense::click_and_drag());
        if bg.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            KtApp::release_pointer_after_wm_grab(ui.ctx());
        }
        if bg.double_clicked() {
            let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
        }
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Файл", |ui| {
                if ui.button("Новый").clicked() {
                    self.new_project();
                    ui.close();
                }
                if ui.button("Открыть…").clicked() {
                    self.open_project_dialog();
                    ui.close();
                }
                ui.separator();
                if ui.button("Сохранить	Ctrl+S").clicked() {
                    self.save();
                    ui.close();
                }
                if ui.button("Сохранить как…").clicked() {
                    self.save_as();
                    ui.close();
                }
                ui.separator();
                if ui.button("Рендер в MP4…").clicked() {
                    self.start_render();
                    ui.close();
                }
                ui.separator();
                if ui.button("Выход").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Правка", |ui| {
                if ui.button("Отменить	Ctrl+Z").clicked() {
                    self.undo();
                    ui.close();
                }
                if ui.button("Вернуть	Ctrl+Shift+Z").clicked() {
                    self.redo();
                    ui.close();
                }
                ui.separator();
                if ui.button("Копировать	Ctrl+C").clicked() {
                    self.copy_selection();
                    ui.close();
                }
                if ui.button("Вставить на курсор	Ctrl+V").clicked() {
                    self.paste_at_cursor();
                    ui.close();
                }
                if ui.button("Удалить	Del").clicked() {
                    self.delete_action();
                    ui.close();
                }
                if ui.button("Разрез / вырез региона	S").clicked() {
                    self.cut_action();
                    ui.close();
                }
            });
            ui.menu_button("Вид", |ui| {
                let detach_label =
                    if self.preview_detached { "Вернуть превью" } else { "Отделить превью" };
                if ui.button(detach_label).clicked() {
                    self.preview_detached =
                        crate::app::preview_detached_next(self.preview_detached, true, false);
                    ui.close();
                }
                ui.checkbox(&mut self.ui_economy, "Экономный UI (30 fps при игре)");
                ui.separator();
                if ui.button("Диагностика…").clicked() {
                    self.perf.diag_open = true;
                    ui.close();
                }
                if ui.button("Уведомления").clicked() {
                    self.notices_open = true;
                    ui.close();
                }
            });
        });
    }

    /// Кнопка тулбара: фиксированный размер, ховер-подсказка + статус-хинт.
    fn tb(&mut self, ui: &mut egui::Ui, label: &str, hint: &str) -> bool {
        let r = ui.add_sized([30.0, 26.0], egui::Button::new(label).frame(false));
        self.hint(r, hint).clicked()
    }

    fn tb_toggle(&mut self, ui: &mut egui::Ui, on: bool, label: &str, hint: &str) -> bool {
        let r = ui.add_sized([30.0, 26.0], egui::Button::selectable(on, label));
        self.hint(r, hint).clicked()
    }

    /// Полоса кнопок ВНИЗУ панели таймлайна (порт: кнопки снизу),
    /// градиентный фон на всю ширину — ничего не вылезает.
    pub fn draw_toolbar_strip(&mut self, ui: &mut egui::Ui) {
        let strip = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), 34.0),
        );
        let painter = ui.painter();
        // плоский фон + 1px шов (тема р.6 — без градиентов)
        painter.rect_filled(strip, 0.0, crate::theme::WINDOW);
        painter.line_segment(
            [strip.left_bottom(), strip.right_bottom()],
            egui::Stroke::new(1.0, crate::theme::SEAM),
        );
        // borderless (п.22 р.6): драг окна за пустые зоны строки,
        // double-click = maximize; строго в кадре drag_started
        let bg = ui.interact(strip, ui.id().with("titlebar-drag"), egui::Sense::click_and_drag());
        if bg.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            KtApp::release_pointer_after_wm_grab(ui.ctx());
        }
        if bg.double_clicked() {
            let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
        }
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
        // тайм-код + скорость (п.19 р.6: бывшая полоса timestrip)
        ui.label(
            egui::RichText::new(crate::util::format_timecode(
                self.player.transport.playhead(),
                self.model.project().settings.fps,
            ))
            .monospace()
            .size(15.0)
            .color(egui::Color32::from_rgb(235, 235, 240)),
        );
        // п.23 р.7: V-fps (реально показанные НОВЫЕ кадры) рядом с кадром
        ui.label(
            egui::RichText::new(format!("V{:.0}", self.perf.vfps))
                .monospace()
                .size(12.0)
                .color(egui::Color32::from_gray(150)),
        );
        self.draw_speed_control(ui);
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
            self.zoom_at_playhead(1.25);
        }
        if self.tb(ui, crate::icons::PLUS, "Приблизить") {
            self.zoom_at_playhead(0.8);
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
        if self.tb(ui, crate::icons::GAUGE, "Диагностика") {
            self.perf.diag_open = true;
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
