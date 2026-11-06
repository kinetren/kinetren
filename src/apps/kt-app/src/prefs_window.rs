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

use crate::controls::{Arrangement, Choice, Chord, Cmd, CodeError, Controls, Group, Style, WheelAct};
use crate::i18n::{k, tr};
use eframe::egui;

/// The key list is listening for a press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capture {
    pub cmd: Cmd,
    /// The key being replaced; `None` — one more key.
    pub slot: Option<usize>,
}

/// A pressed chord that another command holds: asked, never taken silently (rules 16, 20).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Taken {
    pub at: Capture,
    pub chord: Chord,
    pub holder: Cmd,
}

/// The style window's state; it lives in `Start`, so opening a project does not close it.
#[derive(Default)]
pub struct ControlsUi {
    pub open: bool,
    /// Opened by the first entry: the welcome window waits under this answer, as it waits for
    /// the FFmpeg window (it would otherwise cover the tiles, seen in the live scenario).
    pub first_entry: bool,
    /// The Customize part is shown.
    pub customize: bool,
    pub capture: Option<Capture>,
    pub taken: Option<Taken>,
    /// Why the last press cannot be a key here.
    pub refused: Option<crate::i18n::Key>,
    pub filter: String,
    pub code_input: String,
    pub code_error: Option<CodeError>,
}

/// One tile: the picture and two lines.
const TILE: [f32; 2] = [158.0, 124.0];

impl crate::app::KtApp {
    /// The style from the store at the start; the first entry opens the question (L1).
    pub(crate) fn load_controls(&mut self, offer: bool) {
        let choice = self.start.store.start.controls.as_deref().and_then(|c| Choice::from_code(c).ok());
        self.start.controls = Controls::new(choice.unwrap_or_default());
        if offer {
            self.start.store.mark_controls_offered();
            self.start.controls_ui.open = true;
            self.start.controls_ui.first_entry = true;
        }
    }

    /// Everything the window changes comes here: in force at once, on disk at once.
    pub(crate) fn apply_controls(&mut self, choice: Choice) {
        self.start.store.set_controls(choice.style as u8, &choice.to_code());
        self.start.controls = Controls::new(choice);
    }

    /// A tile: the style as the person left it (rule 23), or as it comes.
    pub(crate) fn pick_style(&mut self, style: Style) {
        let kept = self.start.store.controls_of(style as u8).and_then(|c| Choice::from_code(c).ok());
        self.start.controls_ui.capture = None;
        self.start.controls_ui.taken = None;
        self.apply_controls(kept.unwrap_or_else(|| Choice::of(style)));
    }

    /// A typed code: refused with its reason, nothing is applied (rule 16).
    pub(crate) fn enter_code(&mut self) {
        match Choice::from_code(self.start.controls_ui.code_input.trim()) {
            Ok(choice) => {
                self.start.controls_ui.code_error = None;
                self.start.controls_ui.code_input.clear();
                self.apply_controls(choice);
            }
            Err(e) => self.start.controls_ui.code_error = Some(e),
        }
    }

    /// The press the key list waits for. Esc and a second click let go; a key another command
    /// holds asks; a key that cannot be one says why and keeps listening.
    fn controls_listen(&mut self, ctx: &egui::Context) {
        let Some(at) = self.start.controls_ui.capture else { return };
        let press = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, repeat: false, .. } => Some(Chord::of(*key, i.modifiers)),
                egui::Event::Copy => Some(Chord::of(egui::Key::C, egui::Modifiers::CTRL)),
                egui::Event::Cut => Some(Chord::of(egui::Key::X, egui::Modifiers::CTRL)),
                egui::Event::Paste(_) => Some(Chord::of(egui::Key::V, egui::Modifiers::CTRL)),
                _ => None,
            })
        });
        let Some(chord) = press else { return };
        let ui_state = &mut self.start.controls_ui;
        ui_state.refused = None;
        if chord.key == egui::Key::Escape {
            ui_state.capture = None;
            return;
        }
        if crate::controls::reserved(chord.key) || !crate::controls::assignable(chord.key) {
            ui_state.refused = Some(k::controls::REFUSED_KEY);
            return;
        }
        // the transport machine is one key: Ctrl+it is its reverse hold, and a tap has no chord
        if at.cmd == Cmd::PlayPause && (chord.ctrl || chord.shift || chord.alt) {
            ui_state.refused = Some(k::controls::REFUSED_PLAY);
            return;
        }
        ui_state.capture = None;
        match self.start.controls.holder(chord).filter(|h| *h != at.cmd) {
            Some(holder) => self.start.controls_ui.taken = Some(Taken { at, chord, holder }),
            None => {
                let mut choice = self.start.controls.choice.clone();
                choice.bind(at.cmd, at.slot, chord);
                self.apply_controls(choice);
            }
        }
    }

    /// The window `kt_controls`: the tiles, the code, Customize.
    pub(crate) fn draw_controls_window(&mut self, ctx: &egui::Context) {
        // the first entry waits for the FFmpeg window, as the welcome window does
        if !self.start.controls_ui.open || self.ffmpeg_ui.open {
            return;
        }
        self.controls_listen(ctx);
        let mut close = false;
        let win = crate::window_shell::Shell::new(
            egui::Id::new("kt_controls"),
            tr(k::controls::TITLE),
            crate::window_shell::Kind::Tool,
        )
        .size([880.0, 560.0], [560.0, 300.0])
        // centred once, as the welcome window: it is the first thing a person sees
        .default_pos(ctx.content_rect().center() - egui::vec2(440.0, 280.0))
        .probe("controls.window")
        .show(ctx, |ui| {
            crate::widgets::window_body(ui, |ui| {
                ui.add(egui::Label::new(tr(k::controls::INTRO)).wrap());
                ui.add_space(6.0);
                self.controls_tiles(ui);
                ui.add_space(4.0);
                let style = self.start.controls.choice.style;
                ui.add(egui::Label::new(tr(style.missing())).wrap());
                // the marks belong to their owners (L1), under the names they stand for
                ui.add(egui::Label::new(egui::RichText::new(tr(k::controls::TRADEMARKS)).small().weak()).wrap());
                ui.add(egui::Label::new(egui::RichText::new(tr(k::controls::TRADEMARKS_NOTE)).small().weak()).wrap());
                ui.add_space(6.0);
                self.controls_code(ui);
                ui.add_space(6.0);
                let open = self.start.controls_ui.customize;
                let r = ui.add(egui::Button::selectable(open, tr(k::controls::CUSTOMIZE)));
                crate::widgets::field_probe::note("controls.customize", r.rect);
                let r = self.hint(r, tr(k::controls::HINT_CUSTOMIZE));
                if r.clicked() {
                    self.start.controls_ui.customize = !open;
                    self.start.controls_ui.capture = None;
                }
                if self.start.controls_ui.customize {
                    self.controls_customize(ui);
                }
            });
            // A question stands HERE, in the row outside the scroll (R.66): the command's row may
            // lie under the fold, and the person just pressed a key, they did not scroll.
            crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(t) = self.start.controls_ui.taken {
                        self.controls_question(ui, t);
                        return;
                    }
                    let r = ui.button(tr(k::prefs::CLOSE));
                    crate::widgets::field_probe::note("controls.close", r.rect);
                    if r.clicked() {
                        close = true;
                    }
                    let factory = self.start.controls.choice.is_factory();
                    let r = ui.add_enabled(!factory, egui::Button::new(tr(k::controls::RESET)));
                    crate::widgets::field_probe::note("controls.reset", r.rect);
                    if r.clicked() {
                        let style = self.start.controls.choice.style;
                        self.apply_controls(Choice::of(style));
                    }
                    match self.start.controls_ui.refused.filter(|_| self.start.controls_ui.capture.is_some()) {
                        Some(why) => {
                            ui.colored_label(crate::app::HINT_ERROR, tr(why));
                        }
                        // rule 19: the window says where the decision changes later
                        None => {
                            ui.weak(tr(k::controls::CHANGE_LATER));
                        }
                    }
                });
            });
        });
        if close || win.closed() {
            let ui_state = &mut self.start.controls_ui;
            ui_state.open = false;
            ui_state.first_entry = false;
            ui_state.capture = None;
            ui_state.taken = None;
        }
    }

    fn controls_tiles(&mut self, ui: &mut egui::Ui) {
        let mut picked = None;
        let now = self.start.controls.choice.clone();
        ui.horizontal_wrapped(|ui| {
            for style in Style::ALL {
                let like = crate::tf!(k::controls::LIKE, style.product());
                // ours first, with its own name over the caption the owner gave it (L1)
                let (top, under) = if style == Style::Own {
                    (tr(k::controls::OWN).to_string(), like)
                } else {
                    (like, String::new())
                };
                let (rect, r) = ui.allocate_exact_size(egui::vec2(TILE[0], TILE[1]), egui::Sense::click());
                let selected = style == now.style;
                let layout = if selected { now.layout } else { style.layout() };
                let name = format!("{top} {under}");
                r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, name.clone()));
                crate::widgets::field_probe::note(&format!("controls.tile.{}", style as u8), rect);
                let p = ui.painter();
                let fill = if r.hovered() { crate::theme::HOVER } else { crate::theme::WINDOW };
                p.rect_filled(rect, 1.0, fill);
                let edge = if selected { crate::theme::ACCENT } else { crate::theme::OUTLINE };
                p.rect_stroke(rect, 1.0, egui::Stroke::new(if selected { 2.0 } else { 1.0 }, edge), egui::StrokeKind::Inside);
                let pic = egui::Rect::from_min_size(rect.min + egui::vec2(9.0, 9.0), egui::vec2(TILE[0] - 18.0, 70.0));
                layout_picture(p, pic, layout);
                let text = ui.visuals().strong_text_color();
                p.text(egui::pos2(rect.center().x, pic.bottom() + 8.0), egui::Align2::CENTER_TOP, &top, egui::FontId::proportional(13.0), text);
                p.text(
                    egui::pos2(rect.center().x, pic.bottom() + 25.0),
                    egui::Align2::CENTER_TOP,
                    &under,
                    egui::FontId::proportional(11.5),
                    ui.visuals().weak_text_color(),
                );
                if r.clicked() {
                    picked = Some(style);
                }
            }
        });
        if let Some(style) = picked
            && style != now.style
        {
            self.pick_style(style);
        }
    }

    /// The code: what to write down, and where to type one in. Both are here, not behind
    /// Customize: on a reinstall this is the first thing the person needs.
    fn controls_code(&mut self, ui: &mut egui::Ui) {
        let code = self.start.controls.choice.to_code();
        ui.horizontal_wrapped(|ui| {
            ui.label(tr(k::controls::code::LABEL));
            let r = ui.add(egui::Label::new(egui::RichText::new(&code).monospace().strong()).selectable(true));
            crate::widgets::field_probe::note("controls.code", r.rect);
            let r = ui.button(tr(k::controls::code::COPY));
            crate::widgets::field_probe::note("controls.code_copy", r.rect);
            if self.hint(r, tr(k::controls::code::HINT_COPY)).clicked() {
                ui.ctx().copy_text(code.clone());
            }
        });
        ui.horizontal_wrapped(|ui| {
            let edit = egui::TextEdit::singleline(&mut self.start.controls_ui.code_input)
                .hint_text(tr(k::controls::code::ENTER))
                .desired_width(ui.available_width().min(320.0));
            let r = ui.add(edit);
            crate::widgets::field_probe::note("controls.code_input", r.rect);
            if r.changed() {
                self.start.controls_ui.code_error = None;
            }
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let filled = !self.start.controls_ui.code_input.trim().is_empty();
            let b = ui.add_enabled(filled, egui::Button::new(tr(k::controls::code::APPLY)));
            crate::widgets::field_probe::note("controls.code_apply", b.rect);
            if (b.clicked() || enter) && filled {
                self.enter_code();
            }
        });
        if let Some(e) = self.start.controls_ui.code_error {
            ui.colored_label(crate::app::HINT_ERROR, tr(e.text()));
        }
    }

    fn controls_customize(&mut self, ui: &mut egui::Ui) {
        section(ui, tr(k::controls::SECTION_PANELS));
        let mut choice = self.start.controls.choice.clone();
        let r = egui::ComboBox::from_id_salt("controls_layout").selected_text(tr(choice.layout.name())).width(300.0).show_ui(ui, |ui| {
            for a in Arrangement::ALL {
                ui.selectable_value(&mut choice.layout, a, tr(a.name()));
            }
        });
        crate::widgets::field_probe::note("controls.layout", r.response.rect);
        section(ui, tr(k::controls::SECTION_WHEEL));
        ui.horizontal_wrapped(|ui| {
            let word = tr(k::controls::wheel::WORD);
            let wheel = &mut choice.wheel;
            let rows: [(String, &mut WheelAct, &str); 4] = [
                (word.to_string(), &mut wheel.plain, "controls.wheel.plain"),
                (format!("Ctrl+{word}"), &mut wheel.ctrl, "controls.wheel.ctrl"),
                (format!("Shift+{word}"), &mut wheel.shift, "controls.wheel.shift"),
                (format!("Alt+{word}"), &mut wheel.alt, "controls.wheel.alt"),
            ];
            for (name, act, probe) in rows {
                ui.label(name);
                let r = egui::ComboBox::from_id_salt(probe).selected_text(tr(act.name())).show_ui(ui, |ui| {
                    for a in WheelAct::ALL {
                        ui.selectable_value(act, a, tr(a.name()));
                    }
                });
                crate::widgets::field_probe::note(probe, r.response.rect);
            }
        });
        if choice != self.start.controls.choice {
            self.apply_controls(choice);
        }
        section(ui, tr(k::controls::SECTION_KEYS));
        let edit = egui::TextEdit::singleline(&mut self.start.controls_ui.filter)
            .hint_text(tr(k::controls::FILTER))
            .desired_width(ui.available_width().min(320.0));
        let r = ui.add(edit);
        crate::widgets::field_probe::note("controls.filter", r.rect);
        let needle = self.start.controls_ui.filter.trim().to_lowercase();
        for group in Group::ALL {
            let mut titled = false;
            for (cmd, g, name) in crate::controls::COMMANDS {
                if *g != group {
                    continue;
                }
                let name = tr(*name);
                // by the command's name AND by the key (F-UI-002: "what does this key do?")
                let hit = needle.is_empty()
                    || name.to_lowercase().contains(&needle)
                    || self.start.controls.label_all(*cmd).to_lowercase().contains(&needle);
                if !hit {
                    continue;
                }
                if !titled {
                    titled = true;
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(tr(group.title())).strong());
                }
                self.controls_row(ui, *cmd, name);
            }
        }
    }

    /// One command: its name, a cap per key, + for one more, ↺ back to the style.
    fn controls_row(&mut self, ui: &mut egui::Ui, cmd: Cmd, name: &str) {
        let keys = self.start.controls.choice.keys_of(cmd).to_vec();
        let capture = self.start.controls_ui.capture.filter(|c| c.cmd == cmd);
        ui.horizontal(|ui| {
            // half the row, cut by an ellipsis: the caps stand in one column, a long name does not
            // run under them (rule 7), and the whole name is the status line's
            let w = (ui.available_width() * 0.5).clamp(200.0, 420.0);
            let r = ui.add_sized([w, 20.0], egui::Label::new(name).truncate().halign(egui::Align::Min));
            self.hint(r, name);
            for (slot, chord) in keys.iter().enumerate() {
                let listening = capture.is_some_and(|c| c.slot == Some(slot));
                let text = if listening { tr(k::controls::PRESS).to_string() } else { chord.label() };
                let r = ui.add(egui::Button::selectable(listening, text));
                crate::widgets::field_probe::note(&format!("controls.key.{}.{slot}", cmd as u8), r.rect);
                if r.clicked() {
                    self.controls_toggle_capture(Capture { cmd, slot: Some(slot) });
                }
                if listening {
                    let x = ui.button(crate::icons::CLEAR);
                    crate::widgets::field_probe::note(&format!("controls.unbind.{}", cmd as u8), x.rect);
                    if self.hint(x, tr(k::controls::HINT_UNBIND)).clicked() {
                        let mut choice = self.start.controls.choice.clone();
                        choice.unbind(cmd, slot);
                        self.start.controls_ui.capture = None;
                        self.apply_controls(choice);
                    }
                }
            }
            let one_key = cmd == Cmd::PlayPause && !keys.is_empty();
            if keys.len() < crate::controls::MAX_CHORDS && !one_key {
                let listening = capture.is_some_and(|c| c.slot.is_none());
                let text = if listening { tr(k::controls::PRESS) } else { crate::icons::PLUS };
                let r = ui.add(egui::Button::selectable(listening, text));
                crate::widgets::field_probe::note(&format!("controls.add.{}", cmd as u8), r.rect);
                if self.hint(r, tr(k::controls::HINT_ADD)).clicked() {
                    self.controls_toggle_capture(Capture { cmd, slot: None });
                }
            }
            if self.start.controls.choice.is_changed(cmd) {
                let r = ui.button(crate::icons::UNDO);
                crate::widgets::field_probe::note(&format!("controls.reset.{}", cmd as u8), r.rect);
                if self.hint(r, tr(k::controls::HINT_RESET_KEY)).clicked() {
                    let mut choice = self.start.controls.choice.clone();
                    choice.reset_keys(cmd);
                    self.apply_controls(choice);
                }
            }
        });
    }

    /// A key another command holds: who holds it, and the two answers.
    fn controls_question(&mut self, ui: &mut egui::Ui, t: Taken) {
        let holder = crate::controls::cmd_name(t.holder);
        ui.colored_label(crate::app::HINT_ERROR, crate::tf!(k::controls::TAKEN, t.chord.label(), holder));
        let take = ui.button(tr(k::controls::TAKE_IT));
        crate::widgets::field_probe::note("controls.take", take.rect);
        if take.clicked() {
            let mut choice = self.start.controls.choice.clone();
            choice.release(t.holder, t.chord);
            choice.bind(t.at.cmd, t.at.slot, t.chord);
            self.start.controls_ui.taken = None;
            self.apply_controls(choice);
        }
        let keep = ui.button(tr(k::controls::KEEP_IT));
        crate::widgets::field_probe::note("controls.keep", keep.rect);
        if keep.clicked() {
            self.start.controls_ui.taken = None;
        }
    }

    fn controls_toggle_capture(&mut self, at: Capture) {
        let ui_state = &mut self.start.controls_ui;
        ui_state.taken = None;
        ui_state.refused = None;
        ui_state.capture = if ui_state.capture == Some(at) { None } else { Some(at) };
    }

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

                // ---- Layout & Controls style ----
                section(ui, tr(k::controls::TITLE));
                ui.horizontal(|ui| {
                    let style = self.start.controls.choice.style;
                    let like = crate::tf!(k::controls::LIKE, style.product());
                    if self.start.controls.choice.is_factory() {
                        ui.label(like);
                    } else {
                        ui.label(crate::tf!(k::controls::CUSTOMIZED, like));
                    }
                    let r = ui.button(tr(k::controls::CHOOSE));
                    crate::widgets::field_probe::note("prefs.controls", r.rect);
                    if r.clicked() {
                        self.start.controls_ui.open = true;
                    }
                });

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

/// The tile's picture: where the panels stand. Plain blocks: a schema of an arrangement, no
/// product's look and no mark of theirs.
fn layout_picture(p: &egui::Painter, r: egui::Rect, a: Arrangement) {
    let at = |x0: f32, y0: f32, x1: f32, y1: f32| {
        egui::Rect::from_min_max(r.lerp_inside(egui::vec2(x0, y0)), r.lerp_inside(egui::vec2(x1, y1))).shrink(1.0)
    };
    let (bin, preview, meters, timeline) = match a {
        Arrangement::Classic => (at(0.0, 0.0, 0.26, 0.55), at(0.26, 0.0, 0.9, 0.55), at(0.9, 0.0, 1.0, 0.55), at(0.0, 0.55, 1.0, 1.0)),
        Arrangement::TimelineOnTop => (at(0.0, 0.5, 0.26, 1.0), at(0.26, 0.5, 0.9, 1.0), at(0.9, 0.5, 1.0, 1.0), at(0.0, 0.0, 1.0, 0.5)),
        Arrangement::BinBesideTimeline => (at(0.0, 0.5, 0.26, 1.0), at(0.0, 0.0, 1.0, 0.5), at(0.92, 0.5, 1.0, 1.0), at(0.26, 0.5, 0.92, 1.0)),
        Arrangement::MetersBesideTimeline => (at(0.0, 0.0, 0.26, 0.55), at(0.26, 0.0, 1.0, 0.55), at(0.92, 0.55, 1.0, 1.0), at(0.0, 0.55, 0.92, 1.0)),
    };
    p.rect_filled(r, 0.0, crate::theme::PANEL);
    p.rect_filled(bin, 0.0, crate::theme::LANE_CAMERA);
    p.rect_filled(preview, 0.0, crate::theme::EXTREME);
    p.rect_filled(meters, 0.0, crate::theme::AXIS_ZERO);
    p.rect_filled(timeline, 0.0, crate::theme::LANE_VIDEO);
    // two rows of events say "timeline" without a word
    for (row, from, to) in [(0.3, 0.08, 0.55), (0.3, 0.6, 0.9), (0.68, 0.2, 0.75)] {
        let y = timeline.top() + timeline.height() * row;
        let clip = egui::Rect::from_min_max(
            egui::pos2(timeline.lerp_inside(egui::vec2(from, 0.0)).x, y),
            egui::pos2(timeline.lerp_inside(egui::vec2(to, 0.0)).x, y + timeline.height() * 0.22),
        );
        p.rect_filled(clip, 0.0, crate::theme::ACCENT);
    }
    let cross = preview.shrink(preview.height() * 0.3);
    p.rect_stroke(cross, 0.0, egui::Stroke::new(1.0, crate::theme::OUTLINE), egui::StrokeKind::Inside);
}

/// A section title: a gap above, the title in strong text, a thin rule under it.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(title).strong());
    ui.separator();
}
