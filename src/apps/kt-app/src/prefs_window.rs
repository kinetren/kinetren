//! Window “Preferences” (welcome-window-and-settings, capability `ui-reach`): the program's
//! preferences — per machine, outside any project, reachable with no project open.
//!
//! The owner (kkt-26.11.01 item 13): "нафига мне галочка welcome window at start И в самом window
//! и еще продублирована в View?!?! Да и пора бы формировать раздел настроек в file, а то у нас в
//! view уже срач из разного." Each preference is changed HERE and nowhere else (design D9, guard
//! t1294): a second checkbox for the same value — in a menu, in the welcome window — is the
//! defect of that item, whatever its label says. View keeps only what changes what is viewed.
//!
//! Every control acts at once (no Apply): these are switches, not a form to validate.

use crate::i18n::{k, tr};
use eframe::egui;

impl crate::app::KtApp {
    /// The Preferences window (`kt_prefs`).
    pub(crate) fn draw_prefs_window(&mut self, ctx: &egui::Context) {
        if !self.start.prefs_open {
            return;
        }
        let mut close = false;
        let win = crate::window_shell::Shell::new(
            egui::Id::new("kt_prefs"),
            tr(k::prefs::TITLE),
            crate::window_shell::Kind::Tool,
        )
        .width(560.0, [420.0, 200.0])
        .probe("prefs.window")
        .show(ctx, |ui| {
            crate::widgets::window_body(ui, |ui| {
                // ---- Start ----
                section(ui, tr(k::prefs::SECTION_START));
                let mut on = self.start.store.start.show_at_start;
                let r = ui.checkbox(&mut on, tr(k::prefs::WELCOME_AT_START));
                crate::widgets::field_probe::note("prefs.welcome_at_start", r.rect);
                let r = self.hint(r, tr(k::prefs::HINT_WELCOME_AT_START));
                if r.changed() {
                    self.start.store.set_show_at_start(on);
                }

                // ---- Interface ----
                section(ui, tr(k::prefs::SECTION_INTERFACE));
                let choice = self.start.store.lang_choice(None);
                let name = |c: Option<crate::i18n::Lang>| match c {
                    None => tr(k::prefs::LANG_AUTO).to_string(),
                    Some(l) => l.endonym().to_string(),
                };
                let mut picked = None;
                ui.horizontal(|ui| {
                    ui.label(tr(k::prefs::LANGUAGE));
                    // the items are self-names: whoever picks Chinese has nothing to look for in
                    // the Russian list
                    let r = egui::ComboBox::from_id_salt("prefs_lang").selected_text(name(choice)).show_ui(ui, |ui| {
                        let items = std::iter::once(None).chain(crate::i18n::Lang::ALL.into_iter().map(Some));
                        for c in items {
                            if ui.selectable_label(choice == c, name(c)).clicked() {
                                picked = Some(c);
                            }
                        }
                    });
                    // the combo is named by its row (screen readers, the harness): its own text is
                    // the current choice
                    r.response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, tr(k::prefs::LANGUAGE))
                    });
                    crate::widgets::field_probe::note("prefs.language", r.response.rect);
                });
                if let Some(c) = picked {
                    self.start.store.set_lang(c);
                    crate::i18n::set_lang_ctx(ui.ctx(), crate::i18n::resolve(c));
                }
                let mut eco = self.start.store.start.economy_ui;
                let r = ui.checkbox(&mut eco, tr(k::prefs::ECONOMY));
                crate::widgets::field_probe::note("prefs.economy", r.rect);
                let r = self.hint(r, tr(k::hint::ECONOMY));
                if r.changed() {
                    self.start.store.set_economy_ui(eco);
                }

                // ---- Privacy and updates ----
                section(ui, tr(k::prefs::SECTION_PRIVACY));
                // disabled, with the reason in the hint, when this build cannot send (no key) or
                // the environment forbids it
                if let Some(t) = self.telemetry.clone() {
                    let c = t.consent();
                    let mut on = c.on();
                    let r = ui.add_enabled(c.block.is_none(), egui::Checkbox::new(&mut on, tr(k::telemetry::CHECKBOX)));
                    let text = crate::telemetry::hint_text(c.block);
                    // a disabled widget is not `hovered()`: its reason is shown all the same
                    if c.block.is_some() && r.contains_pointer() {
                        self.hover_hint = Some(text.clone());
                    }
                    crate::widgets::field_probe::note("prefs.telemetry", r.rect);
                    let r = self.hint(r, &text);
                    if r.changed() {
                        t.set_choice(on);
                    }
                }
                // the daily new-version check right under it: the same kind of per-machine
                // preference, the same disabled-with-a-reason form
                if let Some(u) = self.update.clone() {
                    let c = u.consent();
                    let mut on = c.on();
                    let r = ui.add_enabled(c.block.is_none(), egui::Checkbox::new(&mut on, tr(k::update::CHECKBOX)));
                    let text = crate::update_check::hint_text(c.block);
                    if c.block.is_some() && r.contains_pointer() {
                        self.hover_hint = Some(text.clone());
                    }
                    crate::widgets::field_probe::note("prefs.update_check", r.rect);
                    let r = self.hint(r, &text);
                    if r.changed() {
                        // off hides the title word in this very frame (the title is rebuilt next)
                        u.set_choice(on);
                        self.title_outdated = self.title_outdated && on;
                    }
                }

                // ---- FFmpeg ----
                section(ui, tr(k::prefs::SECTION_FFMPEG));
                match kt_ffmpeg::backend_info() {
                    Some(i) => {
                        let (a, b, c) = i.avcodec_version;
                        let place = kt_ffmpeg::loaded_place().unwrap_or_default();
                        ui.add(egui::Label::new(crate::tf!(k::ffmpeg::LOADED, i.major, format!("{a}.{b}.{c}"), place)).wrap());
                    }
                    None => {
                        ui.colored_label(crate::app::HINT_ERROR, tr(k::prefs::FFMPEG_MISSING));
                    }
                }
                let r = ui.button(tr(k::prefs::FFMPEG_OPEN));
                crate::widgets::field_probe::note("prefs.ffmpeg", r.rect);
                let r = self.hint(r, tr(k::prefs::HINT_FFMPEG));
                if r.clicked() {
                    self.ffmpeg_ui.open = true;
                }
            });
            crate::widgets::window_actions(ui, |ui| {
                let r = ui.button(tr(k::prefs::CLOSE));
                crate::widgets::field_probe::note("prefs.close", r.rect);
                if r.clicked() {
                    close = true;
                }
            });
        });
        if close || win.closed() {
            self.start.prefs_open = false;
        }
    }
}

/// A section title: a gap above, the title in strong text, a thin rule under it.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(title).strong());
    ui.separator();
}
