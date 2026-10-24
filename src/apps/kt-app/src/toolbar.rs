//! Floating toolbar over the timeline (port of TimelineToolChrome):
//! a semi-transparent gradient strip with a shadow, buttons 30×28, hints
//! duplicated in the status bar.

use crate::app::KtApp;
use crate::i18n::{k, tr};
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

/// Programme logo (item 6 round 7.1): a film frame with the monogram “kt” —
/// drawn by the painter, the same silhouette as in packaging/kinetren.svg.
pub fn draw_logo(painter: &egui::Painter, rect: egui::Rect) {
    let a = crate::theme::ACCENT;
    let body = egui::Rect::from_center_size(rect.center(), rect.size() * 0.92);
    painter.rect_filled(body, 1.0, egui::Color32::from_rgb(18, 20, 24));
    painter.rect_stroke(body, 1.0, egui::Stroke::new(1.0, a), egui::StrokeKind::Middle);
    // film perforation: two “holes” on the left and two on the right
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

/// Borderless title-bar window button, ICONS ARE DRAWN by the painter (item 22 round 6:
/// font glyphs ✕/❐ rendered as “tofu” squares). kind: 0=minimise,
/// 1=maximise/restore, 2=close (red hover).
fn window_button(ui: &mut egui::Ui, kind: u8, maximized: bool) -> egui::Response {
    // 22 px: three buttons fit in a narrow metres column (item 15 round 8).
    // The sense is precisely click: with click_and_drag egui counts the press as the start
    // of a drag, and clicked() does not happen at all (checked t101). A window drag
    // from the button stays with the strip under it — this is the usual title-bar
    // behaviour, and the click itself now gets through (item 9 round 12).
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 20.0), egui::Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        let fill =
            if kind == 2 { egui::Color32::from_rgb(178, 44, 44) } else { crate::theme::HOVER };
        p.rect_filled(rect, 0.0, fill);
    }
    let c = rect.center();
    let s = egui::Stroke::new(1.2, egui::Color32::from_gray(210));
    match kind {
        0 => {
            p.line_segment([c + egui::vec2(-4.5, 3.0), c + egui::vec2(4.5, 3.0)], s);
        }
        1 if maximized => {
            // “restore”: two overlapping squares
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

/// Reel name for the crumb: a long one is truncated, not breaking the row
/// (checklist rule 7; one truncation helper for all places).
fn crumb_label(name: &str) -> String {
    const MAX: usize = 18;
    if name.chars().count() <= MAX {
        return name.to_string();
    }
    let head: String = name.chars().take(MAX - 1).collect();
    format!("{head}…")
}

impl KtApp {
    /// The header of the LEFT column (items 15, 6, 7 round 8): logo + menu + project name with
    /// version. The full-width strip is gone — the centre is given to the
    /// preview from above. The project name is drawn by the painter OUTSIDE the layout: if it does not
    /// fit into the column, it quietly spills to the right over the preview (the user’s decision),
    /// and does not inflate the panel width.
    pub fn draw_column_header(&mut self, ui: &mut egui::Ui) {
        let strip = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), 22.0),
        );
        let painter = ui.painter_at(strip);
        painter.rect_filled(strip, 0.0, crate::theme::WINDOW);
        // window drag from the empty part of the header
        let bg = ui.interact(strip, ui.id().with("header-drag"), egui::Sense::click_and_drag());
        self.window_drag_from(&bg, ui.ctx());
        // logo
        let logo = egui::Rect::from_min_size(
            egui::pos2(strip.left() + 3.0, strip.top() + 3.0),
            egui::vec2(16.0, 16.0),
        );
        draw_logo(&painter, logo);
        // menu: three items if they fit, otherwise a burger (the panel must not
        // inflate to fit the content — regression round 7.1)
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
        // the ACTUAL end of the last menu button: the min_rect of the MenuBar itself
        // is stretched over the full available width, and because of this the project
        // name flew to the right edge of the column (item 5 round 12)
        let menu_end = if wide {
            self.draw_menus(&mut child)
        } else {
            let r = child.menu_button(crate::icons::LIST, |ui| {
                let _ = self.menu_items(ui);
            });
            r.response.rect.right()
        };
        // the menu also sits on the window edge: without this the top quarter of
        // the “File/Edit/View” buttons went into the resize zone (item 9 round 12)
        // ONLY the menu buttons: the logo is not interactive, and if it were grabbed
        // the NW window corner would die (t76 catches it — dragging by the corner stopped).
        self.no_resize_rects.push(egui::Rect::from_min_max(
            egui::pos2(menu_left - 2.0, strip.top() - 4.0),
            egui::pos2(menu_end + 2.0, strip.bottom() + 2.0),
        ));
        ui.advance_cursor_after_rect(strip);
        // project name + version — over everything, starting past the menu
        let fg = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("project-title"),
        ));
        let title_at = fg.text(
            egui::pos2(menu_end + 10.0, strip.center().y),
            egui::Align2::LEFT_CENTER,
            &self.last_title,
            egui::FontId::proportional(11.5),
            egui::Color32::from_gray(125),
        );
        // PATH CRUMBS — on an ALREADY OCCUPIED row, to the right of the project name
        // (checklist rule 13: a separate strip for a label that appears rarely is not
        // made). Each crumb is clickable: this is also the way to reach any
        // level at once.
        let crumbs = self.nest_crumbs();
        if crumbs.is_empty() {
            return;
        }
        // THE PROJECT NAME ITSELF is the first crumb: by clicking it, the person
        // goes to the root in one movement. Without this a path of depth 1 would have no
        // clickable crumbs at all.
        let root_hit = ui.interact(
            title_at.expand2(egui::vec2(2.0, 3.0)),
            ui.id().with("crumb-root"),
            egui::Sense::click(),
        );
        crate::widgets::field_probe::note("nest.root", root_hit.rect);
        let to_root = self.hint(root_hit, tr(k::msg::NEST_CRUMB_HINT)).clicked();
        let mut x = title_at.right() + 8.0;
        let font = egui::FontId::proportional(11.5);
        let mut back_to: Option<usize> = None;
        for (i, name) in crumbs.iter().enumerate() {
            let sep = fg.text(
                egui::pos2(x, strip.center().y),
                egui::Align2::LEFT_CENTER,
                crate::icons::CARET_RIGHT,
                font.clone(),
                egui::Color32::from_gray(90),
            );
            x = sep.right() + 4.0;
            let last = i + 1 == crumbs.len();
            let col = if last { crate::theme::ACCENT } else { egui::Color32::from_gray(150) };
            let r = fg.text(
                egui::pos2(x, strip.center().y),
                egui::Align2::LEFT_CENTER,
                // a long name is truncated, not breaking the row (rule 7):
                // by the same helper as everywhere
                crumb_label(name),
                font.clone(),
                col,
            );
            x = r.right() + 2.0;
            let hit = ui.interact(
                r.expand2(egui::vec2(2.0, 3.0)),
                ui.id().with(("crumb", i)),
                egui::Sense::click(),
            );
            crate::widgets::field_probe::note(
                match i {
                    0 => "nest.crumb0",
                    1 => "nest.crumb1",
                    _ => "nest.crumb",
                },
                hit.rect,
            );
            if self.hint(hit, tr(k::msg::NEST_CRUMB_HINT)).clicked() && !last {
                back_to = Some(i);
            }
        }
        // clicking a crumb is an exit TO IT: as many steps outward as there are
        // levels below it; clicking the project name — straight to the root
        if to_root {
            while !self.nest_path.is_empty() {
                self.leave_nest();
            }
        } else if let Some(i) = back_to {
            for _ in 0..(crumbs.len() - 1 - i) {
                self.leave_nest();
            }
        }
    }

    /// Window buttons — a compact block in the header of the metres panel (item 15 round 8:
    /// no full-width strip, and no overlay over the metres either).
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

    /// The common handling of a window drag from the “empty space” of the strip (item 10 round 8:
    /// from the expanded state the window is first restored, and only
    /// on the NEXT frame does dragging start — X11 does not handle
    /// both commands in one frame).
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

    /// A menu item with a hotkey. The label is assembled HERE, not kept in the
    /// catalogue: the translator must have no way to break the accelerator, and a
    /// Latin label inside an Arabic string would move on reordering.
    fn menu_btn(ui: &mut egui::Ui, label: &str, accel: &str) -> bool {
        crate::widgets::menu_item(ui, label, Some(accel))
    }

    fn menu_btn_probed(ui: &mut egui::Ui, label: &str, accel: &str) -> (bool, egui::Rect) {
        crate::widgets::menu_item_probed(ui, label, Some(accel))
    }

    /// Menu items (three root ones). Separate from the container, so that the
    /// burger shows them too when the column is narrow.
    fn menu_items(&mut self, ui: &mut egui::Ui) -> f32 {
        let file_btn = ui.menu_button(tr(k::menu::file::TITLE), |ui| {
            if ui.button(tr(k::menu::file::NEW)).clicked() {
                self.new_project();
                ui.close();
            }
            if ui.button(tr(k::menu::file::OPEN)).clicked() {
                self.open_project_dialog();
                ui.close();
            }
            // Round 67/70: one import door — legacy .ktproj, the old and
            // foreign .otio; a project of an OLD VERSION of the envelope is repaired by the import, not the reader
            let r = ui.button(tr(k::menu::file::IMPORT_OLDER));
            crate::widgets::field_probe::note("file.import_older", r.rect);
            let r = self.hint(r, tr(k::hint::IMPORT_OLDER));
            if r.clicked() {
                self.import_older_dialog();
                ui.close();
            }
            // THE SAME BODY as in the bin’s empty-space menu (rule 17).
            // Until R.55 a generator was created ONLY from there, and a person could not
            // find this entry — the 8th round claim, verbatim.
            if let Some(kd) = crate::gen_ui::generator_menu(ui) {
                self.create_generator(kd);
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
            // R.70: export to the exchange format is a SEPARATE named action,
            // not a save; the project remains .kt
            let export = ui.menu_button(tr(k::menu::file::EXPORT), |ui| {
                let r = ui.button(tr(k::menu::file::EXPORT_OTIO));
                crate::widgets::field_probe::note("file.export_otio", r.rect);
                let r = self.hint(r, tr(k::hint::EXPORT_OTIO));
                if r.clicked() {
                    self.export_otio_dialog();
                    ui.close();
                }
            });
            crate::widgets::field_probe::note("file.export", export.response.rect);
            ui.separator();
            let r = ui.button(tr(k::menu::file::RENDER));
            crate::widgets::field_probe::note("file.render", r.rect);
            if r.clicked() {
                self.start_render();
                ui.close();
            }
            // R.62: the project settings window — one body with the preview panel combo
            let r = ui.button(tr(k::menu::file::PROJECT_SETTINGS));
            crate::widgets::field_probe::note("file.project_settings", r.rect);
            let r = self.hint(r, tr(k::hint::PROJECT_SETTINGS));
            if r.clicked() {
                self.open_project_settings();
                ui.close();
            }
            ui.separator();
            if ui.button(tr(k::menu::file::QUIT)).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        crate::widgets::field_probe::note("menu.file", file_btn.response.rect);
        let edit_btn = ui.menu_button(tr(k::menu::edit::TITLE), |ui| {
            if Self::menu_btn(ui, tr(k::menu::edit::UNDO), "Ctrl+Z") {
                self.undo();
                ui.close();
            }
            if Self::menu_btn(ui, tr(k::menu::edit::REDO), "Ctrl+Shift+Z") {
                self.redo();
                ui.close();
            }
            ui.separator();
            // rectangles — outward for live scenarios (R.27):
            // the system clipboard in the virtual screen may not exist at all,
            // and the hand goes to the menu, and their path is one (rule 17)
            let (hit, r) = Self::menu_btn_probed(ui, tr(k::menu::edit::COPY), "Ctrl+C");
            crate::widgets::field_probe::note("edit.copy", r);
            if hit {
                self.copy_selection();
                ui.close();
            }
            let (hit, r) = Self::menu_btn_probed(ui, tr(k::menu::edit::PASTE), "Ctrl+V");
            crate::widgets::field_probe::note("edit.paste", r);
            if hit {
                self.paste_at_cursor();
                ui.close();
            }
            // ONE BODY with the event menu item (rule 17)
            if ui.button(tr(k::tl::menu::COLLAPSE)).clicked() {
                self.collapse_selection();
                ui.close();
            }
            if ui.button(tr(k::menu::edit::REMOVE_UNUSED_SEQS)).clicked() {
                self.remove_unused_seqs();
                ui.close();
            }
            if ui.button(tr(k::tl::menu::SYNC_AUDIO)).clicked() {
                // one body for both paths (rule 17): the event menu item
                // and the main menu item do exactly the same thing
                self.sync_selected_by_audio();
                ui.close();
            }
            if Self::menu_btn(ui, tr(k::menu::edit::DELETE), "Del") {
                self.delete_action();
                ui.close();
            }
            // The option “trim stretches the animation together with the event”
            // (R.40). It lives in the EDIT menu, not the view: it changes the
            // result of editing, not how it is shown.
            // Off by default — the key stays on its own frame.
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
        crate::widgets::field_probe::note("menu.edit", edit_btn.response.rect);
        let last = ui.menu_button(tr(k::menu::view::TITLE), |ui| {
            let detach_label = if self.preview_detached {
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
            // The language lives in an ALREADY EXISTING menu (checklist rule 13:
            // no new strip is made for it). The items are self-names:
            // whoever picks Chinese has nothing to look for in the Russian list.
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
            // Telemetry switch next to Language: a per-machine preference lives where the
            // other one is (checklist rule 13). Disabled, with the reason in the hint, when
            // this build cannot send (no key) or the environment forbids it.
            if let Some(t) = self.telemetry.clone() {
                let c = t.consent();
                let mut on = c.on();
                let r = ui.add_enabled(
                    c.block.is_none(),
                    egui::Checkbox::new(&mut on, tr(k::telemetry::CHECKBOX)),
                );
                let text = crate::telemetry::hint_text(c.block);
                // a disabled widget is not `hovered()`: its reason is shown all the same
                if c.block.is_some() && r.contains_pointer() {
                    self.hover_hint = Some(text.clone());
                }
                let r = self.hint(r, &text);
                if r.changed() {
                    t.set_choice(on);
                }
            }
            ui.separator();
            if ui.button(tr(k::menu::view::DIAGNOSTICS)).clicked() {
                self.perf.diag_open = true;
                ui.close();
            }
            if ui.button(tr(k::menu::view::NOTICES)).clicked() {
                self.notices_open = true;
                ui.close();
            }
            // where FFmpeg was found and how to get it (windows-build): a per-machine
            // matter, next to the other per-machine items of this menu
            if ui.button(tr(k::menu::view::FFMPEG)).clicked() {
                self.ffmpeg_ui.open = true;
                ui.close();
            }
        });
        last.response.rect.right()
    }

    /// Draws the three root menus and returns the x of the end of the LAST button.
    fn draw_menus(&mut self, ui: &mut egui::Ui) -> f32 {
        let mut end = ui.min_rect().left();
        egui::MenuBar::new().ui(ui, |ui| {
            end = self.menu_items(ui);
        });
        end
    }

    /// Toolbar button: fixed size, hover hint + status hint.
    fn tb(&mut self, ui: &mut egui::Ui, label: &str, hint: &str) -> bool {
        let r = ui.add_sized([30.0, 26.0], egui::Button::new(label).frame(false));
        self.hint(r, hint).clicked()
    }

    /// The button strip at the BOTTOM of the timeline panel (port: buttons below),
    /// a gradient background over the full width — nothing sticks out.
    pub fn draw_toolbar_strip(&mut self, ui: &mut egui::Ui) {
        let strip = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), 34.0),
        );
        let painter = ui.painter();
        // flat background + 1px seam (theme round 6 — no gradients)
        painter.rect_filled(strip, 0.0, crate::theme::WINDOW);
        painter.line_segment(
            [strip.left_bottom(), strip.right_bottom()],
            egui::Stroke::new(1.0, crate::theme::SEAM),
        );
        // borderless (item 22 round 6): window drag from the empty zones of the row,
        // double-click = maximise; strictly within the frame of drag_started
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
        // Shift here is “drag the region” (like Shift+A/D, item 2 round 16); “to
        // the very start/end” remains on Shift+Home/End
        if self.tb(ui, crate::icons::SKIP_BACK, tr(k::tb::BACK)) {
            let shift = ui.input(|i| i.modifiers.shift);
            self.nav_jump(false, false, shift);
        }
        if self.tb(ui, crate::icons::SKIP_FWD, tr(k::tb::FORWARD)) {
            let shift = ui.input(|i| i.modifiers.shift);
            self.nav_jump(true, false, shift);
        }
        ui.separator();
        // ---- timecode and input mask (item 4 round 10, redone item 9 round 11) ----
        // The input is drawn IN THE SAME RECTANGLE and WITH THE SAME FONT as
        // the timecode itself: the width is taken from the full format, so the
        // toolbar layout does not jump when the mask opens (there was a
        // TextEdit of 96 px against 150 px of text). It works during playback too —
        // the player can seek on the fly.
        {
            let fps = self.model.project().settings.fps;
            // the DISPLAY position: the raw playhead during play jumps in quanta
            // of the audio callback (item 12 of the root CLAUDE.md)
            let live = self.player.video_playhead();
            let font = egui::FontId::monospace(15.0);
            let bright = egui::Color32::from_rgb(235, 235, 240);
            let dim = egui::Color32::from_gray(120);
            let full = crate::util::format_timecode(live, fps);
            let size = ui.painter().layout_no_wrap(full.clone(), font.clone(), bright).size()
                + egui::vec2(6.0, 0.0);
            let (rect, r) = ui.allocate_exact_size(size, egui::Sense::click());
            let editing = self.tc_edit.is_some();
            let r =
                self.hint(r, if editing { tr(k::tb::TIMECODE_EDIT) } else { tr(k::tb::TIMECODE) });
            if r.double_clicked() && !editing {
                self.tc_edit = Some(String::new());
                self.tc_edit_focus = true;
            }
            if let Some(input) = self.tc_edit.clone() {
                if self.tc_edit_focus {
                    r.request_focus();
                    self.tc_edit_focus = false;
                }
                // arrows/Tab must not take the focus out of the mask
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
                // the events are CONSUMED: otherwise the digits and Enter would leak into the hotkeys
                let mut input = input;
                let mut apply = false;
                let mut cancel = false;
                ui.input_mut(|i| {
                    i.events.retain(|e| match e {
                        egui::Event::Text(t) if t.chars().all(|c| c.is_ascii_digit()) => {
                            input.push_str(t);
                            false
                        }
                        // pasting a ready timecode (“1:02:03”) — we parse it
                        // and lay it out into the mask's digit positions
                        egui::Event::Text(t) => match crate::util::parse_timecode(t, fps) {
                            Some(at) => {
                                input = crate::util::tick_to_mask(at);
                                false
                            }
                            None => true,
                        },
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
                // FOCUS MUST RETURN (item 8 round 12): the widget lives each frame with the same id,
                // so “just close it” is not enough —
                // egui went on counting it as focused, and
                // handle_keys is gated by egui_wants_keyboard_input(), and
                // Ctrl+L fired exactly once, until a mouse click.
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
        // the morpher layer (R.35 item 5): the track is created ON TOP of all —
        // otherwise “everything below” would be empty
        if self.tb(ui, "+M", tr(k::tb::ADD_MORPHER)) {
            self.add_morph_layer();
        }
        // camera (R.40 item 5): until this round the camera track could not be
        // created or filled with anything, except via RMB on the 34-pixel
        // header of a FOREIGN row — “what is it for?????”
        if self.tb(ui, "+C", tr(k::tb::ADD_CAMERA)) {
            self.add_camera_layer();
        }
        // a named area from the last selection frame (R.35 item 2)
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
        if self.tb(ui, crate::icons::REGION_IN, tr(k::tb::REGION_IN)) {
            self.set_region_edge_from_cursor(true);
        }
        if self.tb(ui, crate::icons::REGION_OUT, tr(k::tb::REGION_OUT)) {
            self.set_region_edge_from_cursor(false);
        }
        if self.tb(ui, crate::icons::CLEAR, tr(k::tb::REGION_CLEAR)) {
            self.clear_region();
        }
        // the selection tool mode (R.33 item 7, “as in Vegas —
        // toggle button”): in it an ordinary drag over empty space draws a frame
        // instead of a region. Ctrl+drag draws it without the mode too.
        let sel_tool = self.sel_tool;
        let r = ui.add_sized(
            [30.0, 26.0],
            egui::Button::selectable(sel_tool, egui::RichText::new(crate::icons::SELECTION)),
        );
        if self.hint(r, tr(k::tb::SELECT_TOOL)).clicked() {
            self.sel_tool = !sel_tool;
        }
        // three states (item 3 round 11): off / on with a region /
        // ARMED without a region (Esc removed the region, the intent stayed) —
        // a muted icon, the hint explains what is missing
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
        let hint = if armed { tr(k::tb::LOOP_ARMED) } else { tr(k::tb::LOOP) };
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
        if self.tb(ui, crate::icons::RENDER, tr(k::tb::RENDER)) {
            self.start_render();
        }
        if self.tb(ui, crate::icons::SEARCH, tr(k::tb::SEARCH)) {
            self.search_open = true;
            self.search_focus = true;
        }
        // the gauge button publishes its rectangle: the README screenshots
        // (src/scripts/live/readme_shots.py) open the diagnostics through it
        let diag = ui.add_sized([30.0, 26.0], egui::Button::new(crate::icons::GAUGE).frame(false));
        crate::widgets::field_probe::note("tb.diag", diag.rect);
        if self.hint(diag, tr(k::tb::DIAG)).clicked() {
            self.perf.diag_open = true;
        }
        // item 6 round 10: “●” is in none of the Proportional family fonts —
        // a placeholder square was drawn. The asterisk is present everywhere.
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
