//! Плавающий тулбар поверх таймлайна (порт TimelineToolChrome):
//! полупрозрачная градиентная полоса с тенью, кнопки 30×28, подсказки
//! дублируются в статусбар.

use crate::i18n::{k, tr};
use crate::app::KtApp;
use eframe::egui;
use kt_model::ClipKind;

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

/// Логотип программы (п.6 р.7.1): кадр киноплёнки с монограммой «kt» —
/// рисуется painter'ом, тот же силуэт, что в packaging/kinotrain.svg.
pub fn draw_logo(painter: &egui::Painter, rect: egui::Rect) {
    let a = crate::theme::ACCENT;
    let body = egui::Rect::from_center_size(rect.center(), rect.size() * 0.92);
    painter.rect_filled(body, 1.0, egui::Color32::from_rgb(18, 20, 24));
    painter.rect_stroke(body, 1.0, egui::Stroke::new(1.0, a), egui::StrokeKind::Middle);
    // перфорация плёнки: по две «дырки» слева и справа
    let hole_w = body.width() * 0.13;
    let hole_h = body.height() * 0.17;
    for i in 0..2 {
        let y = body.top() + body.height() * (0.22 + 0.44 * i as f32);
        for x in [body.left() + hole_w * 0.55, body.right() - hole_w * 0.55] {
            painter.rect_filled(
                egui::Rect::from_center_size(egui::pos2(x, y), egui::vec2(hole_w, hole_h)),
                0.0,
                egui::Color32::from_rgb(70, 78, 92),
            );
        }
    }
    painter.text(
        body.center() + egui::vec2(0.0, 0.5),
        egui::Align2::CENTER_CENTER,
        "kt",
        egui::FontId::proportional(body.height() * 0.62),
        egui::Color32::from_rgb(228, 233, 242),
    );
}

/// Кнопка окна borderless-титлбара, ИКОНКИ РИСУЮТСЯ painter'ом (п.22 р.6:
/// шрифтовые глифы ✕/❐ рендерились «квадратиками»-tofu). kind: 0=свернуть,
/// 1=развернуть/восстановить, 2=закрыть (красный ховер).
fn window_button(ui: &mut egui::Ui, kind: u8, maximized: bool) -> egui::Response {
    // 22 px: три кнопки помещаются в узкую колонку метров (п.15 р.8).
    // Сенс именно click: с click_and_drag egui считает нажатие началом
    // драга и clicked() не наступает вовсе (проверено t101). Драг окна
    // за кнопку остаётся у полосы под ней — это привычное поведение
    // титлбара, а сам клик теперь доходит (п.9 р.12).
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 20.0), egui::Sense::click());
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
    /// Шапка ЛЕВОЙ колонки (пп.15,6,7 р.8): лого + меню + имя проекта с
    /// версией. Полосы на всю ширину больше нет — центр сверху отдан
    /// превью. Имя проекта рисуется painter'ом ВНЕ layout: если в колонку
    /// не влезает, спокойно вытекает вправо поверх превью (решение
    /// пользователя), а ширину панели не раздувает.
    pub fn draw_column_header(&mut self, ui: &mut egui::Ui) {
        let strip = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), 22.0),
        );
        let painter = ui.painter_at(strip);
        painter.rect_filled(strip, 0.0, crate::theme::WINDOW);
        // драг окна за пустую часть шапки
        let bg = ui.interact(strip, ui.id().with("header-drag"), egui::Sense::click_and_drag());
        self.window_drag_from(&bg, ui.ctx());
        // лого
        let logo = egui::Rect::from_min_size(
            egui::pos2(strip.left() + 3.0, strip.top() + 3.0),
            egui::vec2(16.0, 16.0),
        );
        draw_logo(&painter, logo);
        // меню: три пункта, если помещаются, иначе бургер (панель не
        // должна раздуваться под контент — регрессия р.7.1)
        let menu_left = logo.right() + 5.0;
        let wide = strip.right() - menu_left >= 150.0;
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_max(
                    egui::pos2(menu_left, strip.top()),
                    egui::pos2(strip.right(), strip.bottom()),
                ))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        // ФАКТИЧЕСКИЙ конец последней кнопки меню: min_rect самого
        // MenuBar растянут на всю доступную ширину, и имя проекта из-за
        // этого улетало к правому краю колонки (п.5 р.12)
        let menu_end = if wide {
            self.draw_menus(&mut child)
        } else {
            let r = child.menu_button(crate::icons::LIST, |ui| {
                let _ = self.menu_items(ui);
            });
            r.response.rect.right()
        };
        // меню тоже сидит в кромке окна: без этого верхняя четверть
        // кнопок «Файл/Правка/Вид» уходила в ресайз (п.9 р.12)
        // ТОЛЬКО кнопки меню: лого не интерактивно, и если захватить его,
        // умирает угол окна NW (ловит t76 — тянуть за угол перестало)
        self.no_resize_rects.push(egui::Rect::from_min_max(
            egui::pos2(menu_left - 2.0, strip.top() - 4.0),
            egui::pos2(menu_end + 2.0, strip.bottom() + 2.0),
        ));
        ui.advance_cursor_after_rect(strip);
        // имя проекта + версия — поверх всего, начиная за меню
        let fg = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("project-title"),
        ));
        fg.text(
            egui::pos2(menu_end + 10.0, strip.center().y),
            egui::Align2::LEFT_CENTER,
            &self.last_title,
            egui::FontId::proportional(11.5),
            egui::Color32::from_gray(125),
        );
    }

    /// Кнопки окна — компактным блоком в шапке панели метров (п.15 р.8:
    /// полосы на всю ширину нет, оверлея поверх метров тоже).
    pub fn draw_window_buttons(&mut self, ui: &mut egui::Ui) {
        if std::env::var_os("KT_DECORATIONS").is_some() {
            return;
        }
        let strip = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), 22.0),
        );
        ui.painter_at(strip).rect_filled(strip, 0.0, crate::theme::WINDOW);
        let bg = ui.interact(strip, ui.id().with("wbtn-drag"), egui::Sense::click_and_drag());
        self.window_drag_from(&bg, ui.ctx());
        let mut row = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(strip)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        row.spacing_mut().item_spacing.x = 0.0;
        let ctx = row.ctx().clone();
        let close = window_button(&mut row, 2, false);
        self.no_resize_rects.push(close.rect.expand(2.0));
        if self.hint(close, tr(k::win::CLOSE)).clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let h = if max { tr(k::win::RESTORE) } else { tr(k::win::MAXIMIZE) };
        let maxi = window_button(&mut row, 1, max);
        self.no_resize_rects.push(maxi.rect.expand(2.0));
        if self.hint(maxi, h).clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
        }
        let mini = window_button(&mut row, 0, false);
        self.no_resize_rects.push(mini.rect.expand(2.0));
        if self.hint(mini, tr(k::win::MINIMIZE)).clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        ui.advance_cursor_after_rect(strip);
    }

    /// Общая обработка драга окна за «пустое место» полосы (п.10 р.8:
    /// из развёрнутого состояния окно сперва восстанавливается, и только
    /// СЛЕДУЮЩИМ кадром начинается перетаскивание — X11 не переваривает
    /// обе команды в одном кадре).
    pub(crate) fn window_drag_from(&mut self, bg: &egui::Response, ctx: &egui::Context) {
        if bg.drag_started() {
            if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                self.pending_unmax_drag = true;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                KtApp::release_pointer_after_wm_grab(ctx);
            }
        }
        if bg.double_clicked() {
            let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
        }
    }

    /// Пункт меню с горячей клавишей. Ярлык склеивается ЗДЕСЬ, а не лежит в
    /// каталоге: переводчик не должен иметь возможности сломать акселератор, а
    /// латинский ярлык внутри арабской строки уехал бы при переупорядочивании.
    fn menu_btn(ui: &mut egui::Ui, label: &str, accel: &str) -> bool {
        crate::widgets::menu_item(ui, label, Some(accel))
    }

    /// Пункты меню (три корневых). Отдельно от контейнера, чтобы их же
    /// показывал бургер при узкой колонке.
    fn menu_items(&mut self, ui: &mut egui::Ui) -> f32 {
            ui.menu_button(tr(k::menu::file::TITLE), |ui| {
                if ui.button(tr(k::menu::file::NEW)).clicked() {
                    self.new_project();
                    ui.close();
                }
                if ui.button(tr(k::menu::file::OPEN)).clicked() {
                    self.open_project_dialog();
                    ui.close();
                }
                if ui.button(tr(k::menu::file::IMPORT_KTPROJ)).clicked() {
                    // Р.24 п.12: старый формат — только конвертация
                    self.import_ktproj_dialog();
                    ui.close();
                }
                ui.separator();
                if Self::menu_btn(ui, tr(k::menu::file::SAVE), "Ctrl+S") {
                    self.save();
                    ui.close();
                }
                if ui.button(tr(k::menu::file::SAVE_AS)).clicked() {
                    self.save_as();
                    ui.close();
                }
                if ui.button(tr(k::menu::file::RECOVER)).clicked() {
                    self.offer_recovery(false);
                    ui.close();
                }
                ui.separator();
                if ui.button(tr(k::menu::file::RENDER)).clicked() {
                    self.start_render();
                    ui.close();
                }
                ui.separator();
                if ui.button(tr(k::menu::file::QUIT)).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(tr(k::menu::edit::TITLE), |ui| {
                if Self::menu_btn(ui, tr(k::menu::edit::UNDO), "Ctrl+Z") {
                    self.undo();
                    ui.close();
                }
                if Self::menu_btn(ui, tr(k::menu::edit::REDO), "Ctrl+Shift+Z") {
                    self.redo();
                    ui.close();
                }
                ui.separator();
                if Self::menu_btn(ui, tr(k::menu::edit::COPY), "Ctrl+C") {
                    self.copy_selection();
                    ui.close();
                }
                if Self::menu_btn(ui, tr(k::menu::edit::PASTE), "Ctrl+V") {
                    self.paste_at_cursor();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::SYNC_AUDIO)).clicked() {
                    // одно тело на оба пути (правило 17): пункт меню события
                    // и пункт главного меню делают ровно одно и то же
                    self.sync_selected_by_audio();
                    ui.close();
                }
                if Self::menu_btn(ui, tr(k::menu::edit::DELETE), "Del") {
                    self.delete_action();
                    ui.close();
                }
                // Опция «трим растягивает анимацию вместе с событием»
                // (Р.40). Живёт в меню ПРАВКИ, а не вида: она меняет
                // результат редактирования, а не то, как он показан.
                // Выключена по умолчанию — ключ остаётся на своём кадре.
                let mut stretch = self.model.project().trim_stretches_auto;
                if ui.checkbox(&mut stretch, tr(k::menu::edit::TRIM_STRETCHES_AUTO)).changed() {
                    self.apply(kt_model::Command::SetTrimStretchesAuto { on: stretch });
                }
                ui.separator();
                if Self::menu_btn(ui, tr(k::menu::edit::CUT_REGION), "S") {
                    self.cut_action();
                    ui.close();
                }
            });
            let last = ui.menu_button(tr(k::menu::view::TITLE), |ui| {
                let detach_label =
                    if self.preview_detached {
                        tr(k::menu::view::ATTACH_PREVIEW)
                    } else {
                        tr(k::menu::view::DETACH_PREVIEW)
                    };
                if ui.button(detach_label).clicked() {
                    self.preview_detached =
                        crate::app::preview_detached_next(self.preview_detached, true, false);
                    ui.close();
                }
                ui.checkbox(&mut self.ui_economy, tr(k::menu::view::ECONOMY));
                // Язык живёт в УЖЕ существующем меню (правило 13 чек-листа:
                // новой полосы под него не заводим). Пункты — самоназвания:
                // выбравшему китайский нечего искать в русском списке.
                ui.menu_button(tr(k::menu::view::LANGUAGE), |ui| {
                    let items = std::iter::once((None, tr(k::menu::view::LANG_AUTO)))
                        .chain(crate::i18n::Lang::ALL.into_iter().map(|l| (Some(l), l.endonym())));
                    for (choice, label) in items {
                        let active = self.lang_choice == choice;
                        let label = if active {
                            format!("{label} {}", crate::icons::CHECK)
                        } else {
                            label.to_string()
                        };
                        if ui.button(label).clicked() {
                            self.lang_choice = choice;
                            crate::i18n::set_lang_ctx(ui.ctx(), crate::i18n::resolve(choice));
                            ui.close();
                        }
                    }
                });
                ui.separator();
                if ui.button(tr(k::menu::view::DIAGNOSTICS)).clicked() {
                    self.perf.diag_open = true;
                    ui.close();
                }
                if ui.button(tr(k::menu::view::NOTICES)).clicked() {
                    self.notices_open = true;
                    ui.close();
                }
            });
            last.response.rect.right()
    }

    /// Рисует три корневых меню и возвращает x конца ПОСЛЕДНЕЙ кнопки.
    fn draw_menus(&mut self, ui: &mut egui::Ui) -> f32 {
        let mut end = ui.min_rect().left();
        egui::MenuBar::new().ui(ui, |ui| {
            end = self.menu_items(ui);
        });
        end
    }

    /// Кнопка тулбара: фиксированный размер, ховер-подсказка + статус-хинт.
    fn tb(&mut self, ui: &mut egui::Ui, label: &str, hint: &str) -> bool {
        let r = ui.add_sized([30.0, 26.0], egui::Button::new(label).frame(false));
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
            tr(k::tb::PLAY_PAUSE),
        ) {
            self.on_transport_toggle();
        }
        // Shift здесь — «тянуть регион» (как Shift+A/D, п.2 р.16); «в
        // самое начало/конец» осталось за Shift+Home/End
        if self.tb(ui, crate::icons::SKIP_BACK, tr(k::tb::BACK))
        {
            let shift = ui.input(|i| i.modifiers.shift);
            self.nav_jump(false, false, shift);
        }
        if self.tb(ui, crate::icons::SKIP_FWD, tr(k::tb::FORWARD))
        {
            let shift = ui.input(|i| i.modifiers.shift);
            self.nav_jump(true, false, shift);
        }
        ui.separator();
        // ---- тайм-код и маска ввода (п.4 р.10, переделано п.9 р.11) ----
        // Ввод рисуется В ТОМ ЖЕ прямоугольнике и ТЕМ ЖЕ шрифтом, что и
        // сам тайм-код: ширина берётся от полного формата, поэтому
        // раскладка тулбара при открытии маски не дёргается (был
        // TextEdit на 96 px против 150 px текста). Работает и во время
        // воспроизведения — плеер умеет seek на ходу.
        {
            let fps = self.model.project().settings.fps;
            // позиция ПОКАЗА: сырой playhead при игре дёргается квантами
            // аудио-колбэка (п.12 корневого CLAUDE.md)
            let live = self.player.video_playhead();
            let font = egui::FontId::monospace(15.0);
            let bright = egui::Color32::from_rgb(235, 235, 240);
            let dim = egui::Color32::from_gray(120);
            let full = crate::util::format_timecode(live, fps);
            let size = ui.painter().layout_no_wrap(full.clone(), font.clone(), bright).size()
                + egui::vec2(6.0, 0.0);
            let (rect, r) = ui.allocate_exact_size(size, egui::Sense::click());
            let editing = self.tc_edit.is_some();
            let r = self.hint(
                r,
                if editing {
                    tr(k::tb::TIMECODE_EDIT)
                } else {
                    tr(k::tb::TIMECODE)
                },
            );
            if r.double_clicked() && !editing {
                self.tc_edit = Some(String::new());
                self.tc_edit_focus = true;
            }
            if let Some(input) = self.tc_edit.clone() {
                if self.tc_edit_focus {
                    r.request_focus();
                    self.tc_edit_focus = false;
                }
                // стрелки/Tab не должны уводить фокус из маски
                ui.memory_mut(|m| {
                    m.set_focus_lock_filter(
                        r.id,
                        egui::EventFilter {
                            tab: true,
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: false,
                        },
                    )
                });
                // события СЪЕДАЕМ: иначе цифры и Enter утекут в хоткеи
                let mut input = input;
                let mut apply = false;
                let mut cancel = false;
                ui.input_mut(|i| {
                    i.events.retain(|e| match e {
                        egui::Event::Text(t) if t.chars().all(|c| c.is_ascii_digit()) => {
                            input.push_str(t);
                            false
                        }
                        // вставка готового тайм-кода («1:02:03») — разбираем
                        // и раскладываем по разрядам маски
                        egui::Event::Text(t) => {
                            match crate::util::parse_timecode(t, fps) {
                                Some(at) => {
                                    input = crate::util::tick_to_mask(at);
                                    false
                                }
                                None => true,
                            }
                        }
                        egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                            input.pop();
                            false
                        }
                        egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                            apply = true;
                            false
                        }
                        egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                            cancel = true;
                            false
                        }
                        _ => true,
                    })
                });
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 1.0, egui::Color32::from_rgb(22, 24, 28));
                painter.line_segment(
                    [
                        egui::pos2(rect.left() + 2.0, rect.bottom() - 1.0),
                        egui::pos2(rect.right() - 2.0, rect.bottom() - 1.0),
                    ],
                    egui::Stroke::new(1.0, crate::theme::ACCENT),
                );
                let (live_part, typed_part) = crate::util::mask_text(&input);
                let mut x = rect.left() + 3.0;
                for (txt, col) in [(live_part, dim), (typed_part, bright)] {
                    if txt.is_empty() {
                        continue;
                    }
                    let g = painter.layout_no_wrap(txt, font.clone(), col);
                    let w = g.size().x;
                    painter.galley(egui::pos2(x, rect.center().y - g.size().y * 0.5), g, col);
                    x += w;
                }
                if apply {
                    let t = crate::util::mask_to_tick(&input, fps);
                    self.player.seek(t);
                    self.pending_reveal = Some(t.0);
                    self.tc_edit = None;
                } else if cancel || (r.lost_focus() && !r.has_focus()) {
                    self.tc_edit = None;
                } else {
                    self.tc_edit = Some(input);
                }
                // ФОКУС ОБЯЗАН ВЕРНУТЬСЯ (п.8 р.12): виджет живёт каждый
                // кадр с тем же id, поэтому «просто закрыть» мало —
                // egui продолжал считать его сфокусированным, а
                // handle_keys гейтится egui_wants_keyboard_input(), и
                // Ctrl+L срабатывал ровно один раз до клика мышью
                if self.tc_edit.is_none() {
                    ui.memory_mut(|m| m.surrender_focus(r.id));
                }
            } else {
                ui.painter_at(rect).text(
                    egui::pos2(rect.left() + 3.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    full,
                    font,
                    bright,
                );
            }
        }
        self.draw_speed_control(ui);
        ui.separator();
        if self.tb(ui, crate::icons::FLAG, tr(k::tb::MARKER)) {
            self.marker_toggle_at_cursor();
        }
        if self.tb(ui, "+V", tr(k::tb::ADD_VIDEO_LANE)) {
            self.add_track(ClipKind::Video);
        }
        if self.tb(ui, "+A", tr(k::tb::ADD_AUDIO_LANE)) {
            self.add_track(ClipKind::Audio);
        }
        // морфер-слой (Р.35 п.5): дорожка заводится СВЕРХУ над всеми —
        // иначе «всё, что ниже» было бы пусто
        if self.tb(ui, "+M", tr(k::tb::ADD_MORPHER)) {
            self.add_morph_layer();
        }
        // камера (Р.40 п.5): до этого раунда камерную дорожку нельзя было
        // ни завести, ни наполнить ничем, кроме ПКМ по 34-пиксельному
        // заголовку ЧУЖОЙ строки — «для чего он?????»
        if self.tb(ui, "+C", tr(k::tb::ADD_CAMERA)) {
            self.add_camera_layer();
        }
        // именованная область из последней рамки выделения (Р.35 п.2)
        if self.tb(ui, crate::icons::AREA_SAVE, tr(k::tb::SAVE_AREA)) {
            self.save_marked_area();
        }
        ui.separator();
        if self.tb(ui, crate::icons::SCISSORS, tr(k::tb::CUT)) {
            self.cut_action();
        }
        if self.tb(ui, crate::icons::COPY, tr(k::tb::COPY)) {
            self.copy_selection();
        }
        if self.tb(ui, crate::icons::CLIPBOARD, tr(k::tb::PASTE)) {
            self.paste_at_cursor();
        }
        if self.tb(ui, crate::icons::TRASH, tr(k::tb::DELETE)) {
            self.delete_action();
        }
        if self.tb(ui, crate::icons::TRIM_START, tr(k::tb::TRIM_START)) {
            self.trim_selected_start_to_cursor();
        }
        if self.tb(ui, crate::icons::TRIM_END, tr(k::tb::TRIM_END)) {
            self.trim_selected_end_to_cursor();
        }
        if self.tb(ui, crate::icons::LANE_UP, tr(k::tb::LANE_UP)) {
            self.move_selected_lane(-1);
        }
        if self.tb(ui, crate::icons::LANE_DOWN, tr(k::tb::LANE_DOWN)) {
            self.move_selected_lane(1);
        }
        ui.separator();
        if self.tb(ui, crate::icons::MINUS, tr(k::tb::ZOOM_OUT)) {
            self.zoom_at_playhead(1.25);
        }
        if self.tb(ui, crate::icons::PLUS, tr(k::tb::ZOOM_IN)) {
            self.zoom_at_playhead(0.8);
        }
        ui.separator();
        if self.tb(ui, crate::icons::REGION_IN, tr(k::tb::REGION_IN))
        {
            self.set_region_edge_from_cursor(true);
        }
        if self.tb(ui, crate::icons::REGION_OUT, tr(k::tb::REGION_OUT))
        {
            self.set_region_edge_from_cursor(false);
        }
        if self.tb(ui, crate::icons::CLEAR, tr(k::tb::REGION_CLEAR)) {
            self.clear_region();
        }
        // режим-инструмент выделения (Р.33 п.7, «как в vegas —
        // toggle-button»): в нём обычный драг по пустому месту рисует рамку
        // вместо региона. Ctrl+драг рисует её и без режима.
        let sel_tool = self.sel_tool;
        let r = ui.add_sized(
            [30.0, 26.0],
            egui::Button::selectable(sel_tool, egui::RichText::new(crate::icons::SELECTION)),
        );
        if self.hint(r, tr(k::tb::SELECT_TOOL)).clicked() {
            self.sel_tool = !sel_tool;
        }
        // три состояния (п.3 р.11): выключен / включён с регионом /
        // ВЗВЕДЁН без региона (Esc снял регион, намерение осталось) —
        // приглушённый значок, подсказка объясняет, чего не хватает
        let (loop_on, has_region) = {
            let p = self.model.project();
            (p.loop_enabled, p.region.is_some())
        };
        let armed = loop_on && !has_region;
        let label = if armed {
            egui::RichText::new(crate::icons::LOOP).color(egui::Color32::from_gray(120))
        } else {
            egui::RichText::new(crate::icons::LOOP)
        };
        let hint = if armed {
            tr(k::tb::LOOP_ARMED)
        } else {
            tr(k::tb::LOOP)
        };
        let r = ui.add_sized([30.0, 26.0], egui::Button::selectable(loop_on, label));
        if self.hint(r, hint).clicked() {
            self.toggle_loop();
        }
        ui.separator();
        if self.tb(ui, crate::icons::UNDO, tr(k::tb::UNDO)) {
            self.undo();
        }
        if self.tb(ui, crate::icons::REDO, tr(k::tb::REDO)) {
            self.redo();
        }
        ui.separator();
        if self.tb(ui, crate::icons::RENDER, tr(k::tb::RENDER))
        {
            self.start_render();
        }
        if self.tb(ui, crate::icons::SEARCH, tr(k::tb::SEARCH)) {
            self.search_open = true;
            self.search_focus = true;
        }
        if self.tb(ui, crate::icons::GAUGE, tr(k::tb::DIAG)) {
            self.perf.diag_open = true;
        }
        // п.6 р.10: «●» нет ни в одном шрифте семейства Proportional —
        // рисовался квадрат-заглушка. Звёздочка есть везде.
        let save_label = if self.dirty_content {
            format!("{} *", crate::icons::SAVE)
        } else {
            crate::icons::SAVE.to_string()
        };
        if self.tb(ui, &save_label, tr(k::tb::SAVE)) {
            self.save();
        }
    }
}
