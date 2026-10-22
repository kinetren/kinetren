//! Envelope clipboard: the SYSTEM one, with no mirror inside the program (R.61).
//!
//! **The owner’s claim from which everything follows.** “Judging by the fact that
//! the button can be “disabled” — there is some state of the clipboard in the code —
//! that is bad; on a button press you need to take the value from the real
//! clipboard and only then check whether it can be pasted, and offer actions.”
//!
//! He is right twice. Copying wrote BOTH to the system clipboard AND to the mirror
//! `KtApp::env_clipboard`, and the availability of the button was decided BY THE
//! MIRROR — so what was copied in the second window of the program could not be pasted.
//! The second half of the same spot: the button went dark on someone else’s
//! effect kind, which means field matching was not absent but forbidden.
//!
//! **Why the read is IN THE BACKGROUND, not right in the press handler.** On X11
//! the clipboard is OWNED by a window. If the owner is our own window (and that is the most frequent
//! case: the person has just copied an envelope here), a synchronous
//! read sends a selection request to our own event loop — which at this
//! moment is running the drawing and cannot answer. That would hang
//! exactly in the scenario the feature exists for. So the press SCHEDULES the
//! read as a finite, cancellable task (`BgJob`, invariant 5), and the answer
//! arrives on the next frame.

use crate::bg::BgJob;

/// How the clipboard read ended. The error is NAMED: “an impossible
/// action is an error, not silence” (rule 16 of the checklist).
pub enum ClipRead {
    /// A parsed envelope block.
    Envelope(Box<kt_model::ops::EnvelopeBlock>),
    /// The buffer holds text, but not our block.
    Foreign,
    /// The buffer is empty or unavailable (no clipboard manager, no X server).
    Empty,
}

/// Read order: who should receive the result, and the task itself.
pub struct ClipJob {
    /// Where to paste when the answer arrives: the event and the chain slot.
    pub target: (kt_model::ClipId, usize),
    pub job: BgJob<(), ClipRead>,
}

/// Read the system clipboard and parse it.
///
/// It lives in a separate function because TWO entry points
/// call it (the effects window and the morpher window),
/// and because its body is the only place where the program touches the system clipboard for reading at all.
pub fn read_clipboard() -> ClipRead {
    let text = match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
        Ok(t) => t,
        Err(_) => return ClipRead::Empty,
    };
    if text.trim().is_empty() {
        return ClipRead::Empty;
    }
    match serde_json::from_str::<kt_model::ops::EnvelopeBlock>(&text) {
        Ok(b) => ClipRead::Envelope(Box::new(b)),
        Err(_) => ClipRead::Foreign,
    }
}

/// Order a clipboard read in the background.
pub fn spawn_read(target: (kt_model::ClipId, usize)) -> ClipJob {
    ClipJob { target, job: BgJob::spawn("envclip", |_ctl, _send| read_clipboard()) }
}

/// Matching window “curve → target parameter” (R.61).
///
/// It is shown ONLY when the remainder is non-empty: for one’s own effect kind the names match
/// entirely, and an extra step in the most frequent case would be a tax.
pub struct FitDialog {
    pub target: (kt_model::ClipId, usize),
    /// Key shift “entry to entry” — the same as in an ordinary paste.
    pub delta: i64,
    /// Curves whose address the person chooses; `None` means “do not put”.
    pub rows: Vec<(kt_model::ParamCurve, Option<String>)>,
    /// Free parameters of the target.
    pub free: Vec<String>,
    /// What has already been placed silently — applied together with the choice, in one step.
    pub matched: Vec<kt_model::ParamCurve>,
    pub open: bool,
}

impl crate::app::KtApp {
    /// Take the read clipboard and decide its fate (R.61).
    ///
    /// Three outcomes, and not one silent: our block — put it (with a different
    /// effect kind, first asking about the remainder); foreign text or an empty buffer
    /// — a NAMED error in the status line.
    pub(crate) fn poll_env_clipboard(&mut self) {
        let Some(req) = self.env_read.as_mut() else { return };
        if !req.job.poll() {
            return;
        }
        let (target, done) = (req.target, req.job.done.take());
        self.env_read = None;
        let Some(read) = done else { return };
        let block = match read {
            ClipRead::Envelope(b) => *b,
            ClipRead::Foreign => {
                self.warn(crate::i18n::tr(crate::i18n::k::fx::PASTE_ENV_FOREIGN));
                return;
            }
            ClipRead::Empty => {
                self.warn(crate::i18n::tr(crate::i18n::k::fx::PASTE_ENV_EMPTY));
                return;
            }
        };
        let (id, slot) = target;
        let Some(clip) = self.model.clip(id).cloned() else { return };
        let Some(fx) = clip.fx.get(slot).cloned() else { return };
        // keys live in MATERIAL ticks: move them to the entry point of this event,
        // otherwise the curve would drift onto another piece
        let delta = clip.source_in.0 - block.src_in.0;
        let fit = kt_model::ops::fit_envelope(&block.curves, &fx);
        if fit.leftover.is_empty() {
            self.apply_env_fit(target, delta, fit.matched, Vec::new());
            return;
        }
        // the remainder exists — we ask; the window is shown ONLY in this case
        self.env_fit = Some(FitDialog {
            target,
            delta,
            rows: fit.leftover.into_iter().map(|c| (c, None)).collect(),
            free: fit.free,
            matched: fit.matched,
            open: true,
        });
    }

    /// Put the curves into the slot in one history step.
    pub(crate) fn apply_env_fit(
        &mut self,
        target: (kt_model::ClipId, usize),
        delta: i64,
        matched: Vec<kt_model::ParamCurve>,
        remapped: Vec<kt_model::ParamCurve>,
    ) {
        let (id, slot) = target;
        let Some(clip) = self.model.clip(id).cloned() else { return };
        let mut auto = clip.fx_auto.clone();
        if auto.len() <= slot {
            auto.resize(slot + 1, Vec::new());
        }
        for mut c in matched.into_iter().chain(remapped) {
            for kf in &mut c.keys {
                kf.t = kt_time::Tick((kf.t.0 + delta).max(0));
            }
            auto[slot].retain(|old| old.param != c.param);
            auto[slot].push(c);
        }
        self.gesture_counter += 1;
        let g = self.gesture_counter;
        self.set_fx_auto_coalesced(id, auto, g);
    }
}

impl crate::app::KtApp {
    /// Matching window “curve → target parameter”.
    ///
    /// It is shown ONLY when the remainder is non-empty: for one’s own effect kind the names match
    /// entirely, and an extra step in the most frequent case would be a tax.
    /// Declining the choice does NOT lose the curves silently — their number is named.
    pub(crate) fn draw_env_fit(&mut self, ctx: &eframe::egui::Context) {
        use crate::i18n::{k, tr};
        use eframe::egui;
        let Some(mut d) = self.env_fit.take() else { return };
        let mut go = false;
        let mut cancel = false;
        let mut open = d.open;
        egui::Window::new(tr(k::fx::FIT_TITLE))
            .id(egui::Id::new(("env_fit", d.target.0.0, d.target.1)))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                ui.label(egui::RichText::new(tr(k::fx::FIT_HINT)).weak().small());
                egui::Grid::new("env_fit_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    for (i, (c, pick)) in d.rows.iter_mut().enumerate() {
                        ui.label(crate::widgets::param_label(&c.param));
                        let text = pick.clone().unwrap_or_else(|| tr(k::fx::FIT_SKIP).into());
                        let r = egui::ComboBox::from_id_salt(("env_fit", i))
                            .selected_text(text)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(pick.is_none(), tr(k::fx::FIT_SKIP))
                                    .clicked()
                                {
                                    *pick = None;
                                }
                                for f in &d.free {
                                    let on = pick.as_deref() == Some(f.as_str());
                                    if ui
                                        .selectable_label(on, crate::widgets::param_label(f))
                                        .clicked()
                                    {
                                        *pick = Some(f.clone());
                                    }
                                }
                            });
                        crate::widgets::field_probe::note(
                            &format!("envfit.pick.{i}"),
                            r.response.rect,
                        );
                        ui.end_row();
                    }
                });
                ui.separator();
                });
                crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    let b = ui.button(tr(k::fx::FIT_DO));
                    crate::widgets::field_probe::note("envfit.ok", b.rect);
                    if b.clicked() {
                        go = true;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                    });
            });
            });
        if go {
            let mut remapped = Vec::new();
            let mut skipped = 0usize;
            for (c, pick) in d.rows.drain(..) {
                match pick {
                    Some(to) => remapped.push(kt_model::ops::remap_curve(c, &to)),
                    None => skipped += 1,
                }
            }
            let matched = std::mem::take(&mut d.matched);
            self.apply_env_fit(d.target, d.delta, matched, remapped);
            // what was skipped is NAMED: a curve may not vanish silently
            if skipped > 0 {
                self.warn(&crate::tf!(k::fx::FIT_LOST, skipped));
            }
            return;
        }
        if cancel || !open {
            return;
        }
        d.open = open;
        self.env_fit = Some(d);
    }
}
