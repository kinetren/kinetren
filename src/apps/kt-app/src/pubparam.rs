//! Public parameters of a composition: the author’s side and the consumer’s side
//! (R.59, stage 3, item 5 of the plan).
//!
//! The owner’s complaint, verbatim: “someone created a template project for
//! a news broadcast… surely people won’t climb into the composition every time
//! and change the text and logos there”. Hence exactly two sides and
//! two windows:
//!
//! * **AUTHOR** declares the handles. There is one door — right-click on the LABEL of a field
//!   inside the strip (`widgets::PublishCtx`), and the “Public parameters” window
//!   holds what is already declared: name, order, narrowing of limits, removal.
//! * **CONSUMER** fills in what is declared. The “Composition parameters” window
//!   of the instance is the most important window of this round: it is the answer to
//!   “do not climb inside”.
//!
//! Three decisions that are easy to lose:
//!
//! 1. **The handle name is the AUTHOR’S TEXT** (the owner’s decision). It is drawn as is,
//!    bypassing `tr()`, like the name of an event or a strip: it is document content,
//!    not program text (t131/t684).
//! 2. **The template value is not stored** — the window reads it live from the target
//!    (`Project::published_value`) and shows it next to the row. A row
//!    without an override is drawn muted: it is visible that the instance comes from
//!    the strip and so far differs from it in nothing.
//! 3. **The windows edit the CURRENT strip.** Publish commands do not name
//!    the strip (like `AddClip` and the other sixty hands), so the author’s
//!    window opens INSIDE the strip, and entering is part of opening:
//!    the same body as the verb “enter” (rule 17).

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::{
    ClipId, Command, MediaId, Override, ParamAddr, ParamType, ParamValue, PubId, PubRange,
    PublishedParam,
};

/// Dialog for the name of the published handle. The form is that of the strip-name window (t345):
/// live preview of the target’s value, Enter/Esc, focus returns on
/// close (rule 14).
pub struct PublishDialog {
    pub seq: MediaId,
    pub addr: ParamAddr,
    pub ty: ParamType,
    /// The default is the field’s LABEL in the interface language; after confirmation
    /// it is already the author’s text and never returns to the catalog.
    pub name: String,
    pub focus: bool,
}

/// The strip’s “Public parameters” window (the AUTHOR’s side).
pub struct PublishedWin {
    pub seq: MediaId,
    /// The whole session of the window is ONE history step (the same trick as in the FX window).
    pub gesture: u64,
    /// The set at the moment of opening: “Cancel” returns it under the same token.
    pub backup: Vec<PublishedParam>,
    /// Removal confirmation: the handle and how many overrides will die.
    pub confirm: Option<(PubId, usize)>,
    pub open: bool,
}

/// What the strip head asks of the FX window (R.63 item 12): the cursor jump on
/// a click in the graph and the requests about the axis. It applies the commands itself —
/// under the same gesture token as the window.
#[derive(Default)]
pub(crate) struct InstHeadOut {
    pub seek: Option<kt_time::Tick>,
    pub axis: crate::automation::AxisReq,
    /// Edits of the handles: the FX window applies them under ITS OWN gesture token.
    pub ops: Vec<Command>,
}

/// Everything the head needs from the program. A separate structure, because it is drawn
/// INSIDE the FX window, where `self` is already borrowed
/// (the thumbnail picker cache) — and a second mutable reference to the app
/// cannot be had from anywhere.
pub(crate) struct InstHeadCtx<'a> {
    pub doc: &'a kt_model::Project,
    pub inst: &'a kt_model::ClipEvent,
    pub seq: MediaId,
    pub playhead: kt_time::Tick,
    pub tl_view: (kt_time::Tick, kt_time::Tick),
    pub axis_linked: bool,
}

/// Curve name of the handle WITHIN THE WINDOW.
///
/// The automation widget draws curves of ONE carrier and tells them apart by
/// `ParamCurve::param`. For overrides the target name (`radius`, `text`) does not
/// fit: two handles of a strip may aim at identically named parameters
/// of different events, and in the window they would merge into one line. So inside the
/// window the curve is called by the HANDLE NUMBER, and when written to the model the name
/// is returned as the target one — the one that `apply_overrides` expects.
fn pub_key(id: PubId) -> String {
    format!("pub{}", id.0)
}

/// The human-readable value of the handle — one line next to the row.
/// It must not have its own form: it is the SAME text as in the field.
fn value_text(v: &ParamValue, media_name: &dyn Fn(MediaId) -> String) -> String {
    match v {
        ParamValue::Num(x) => {
            if (x - x.round()).abs() < 1e-9 {
                format!("{}", x.round() as i64)
            } else {
                format!("{x:.3}")
            }
        }
        ParamValue::Text(s) => s.clone(),
        ParamValue::Color([r, g, b, a]) => format!("{r:02x}{g:02x}{b:02x}{a:02x}"),
        ParamValue::Flag(b) => (if *b { "1" } else { "0" }).to_string(),
        ParamValue::Choice(s) => s.clone(),
        ParamValue::Media(m) => media_name(*m),
    }
}

impl KtApp {
    /// Declared handles of the strip — from the DOCUMENT, not from the working view:
    /// standing inside the strip, its record in `project.sequences` holds
    /// the contents of the root.
    pub fn published_of(&self, seq: MediaId) -> Vec<PublishedParam> {
        self.model.document().sequence(seq).map(|s| s.published.clone()).unwrap_or_default()
    }

    /// How many overrides of this handle live in the WHOLE document.
    ///
    /// It is counted over the whole tree, not over the visible view: removing the handle
    /// kills them everywhere in one history step (`reconcile_published`), and
    /// it is exactly this number that must be promised.
    pub fn override_uses(&self, seq: MediaId, id: PubId) -> usize {
        let doc = self.model.document();
        let hit =
            |c: &kt_model::ClipEvent| c.media == seq && c.overrides.iter().any(|o| o.pub_id == id);
        doc.clips.iter().filter(|c| hit(c)).count()
            + doc.sequences.iter().flat_map(|s| s.clips.iter()).filter(|c| hit(c)).count()
    }

    /// The OWNER strip of an event in the working view (R.59, stage 3).
    ///
    /// Publishing addresses the target by `ClipId`, and the address must lie in the
    /// strip WHOSE contents it is. An event of an expanded strip physically
    /// stands in the working view next to the root ones, but belongs to the strip
    /// of the strip: if we published it in the current one, `reconcile_published` would remove
    /// the handle in the same batch that created it — silently and without a trace.
    /// `None` — the root: it never has publications (it is nobody’s instance),
    /// and there is no menu item there at all (rule 9).
    pub fn owner_reel_of(&self, doc: &kt_model::Project, id: ClipId) -> Option<MediaId> {
        for b in self.model.bands() {
            if doc
                .sequence(b.seq)
                .is_some_and(|s| s.clips.iter().any(|c| kt_model::same_clip(c.id, id)))
            {
                return Some(b.seq);
            }
        }
        self.model.editing()
    }

    /// A ready publishing context for the property carrier.
    ///
    /// The document comes IN FROM OUTSIDE and is cloned once per drawing of the
    /// window, not per row: `document()` is a clone of the whole project, and calling
    /// it for each parameter would mean allocating on the drawing path
    /// (invariant 12).
    pub fn publish_ctx(
        &self,
        doc: &kt_model::Project,
        clip: ClipId,
        owner: kt_model::ParamOwner,
    ) -> crate::widgets::PublishCtx {
        let seq = self.owner_reel_of(doc, clip);
        let published = seq
            .and_then(|s| doc.sequence(s))
            .map(|s| {
                s.published
                    .iter()
                    .filter(|p| {
                        p.target.owner == owner
                            && (matches!(
                                owner,
                                kt_model::ParamOwner::Transition { .. }
                                    | kt_model::ParamOwner::Camera { .. }
                            ) || kt_model::same_clip(p.target.clip, clip))
                    })
                    .map(|p| (p.target.param.clone(), p.id))
                    .collect()
            })
            .unwrap_or_default();
        crate::widgets::PublishCtx {
            seq,
            clip,
            owner,
            published,
            explain_outside: !doc.sequences.is_empty(),
        }
    }

    /// A free handle number in this strip. Numbers live inside the strip
    /// (`Nested` addresses a handle of ITS OWN strip), so the maximum is taken
    /// over it too.
    fn next_pub_id(&self, seq: MediaId) -> PubId {
        PubId(self.published_of(seq).iter().map(|p| p.id.0).max().unwrap_or(0) + 1)
    }

    /// How many handles the instance overrode and how many there are in all — the text
    /// of the hint under the pin (rule 13: the place is already taken by the label).
    ///
    /// It asks the WORKING VIEW, not the document, and this is not laziness: it is called from
    /// the hover path, that is per frame, while `document()` is a clone of
    /// the whole project (invariant 12, “no allocations in draw paths per
    /// frame”). The answer is correct regardless: standing inside strip X, its record in
    /// `project.sequences` holds the ROOT’s set — but an instance of X cannot be inside
    /// X itself because of cycles, so for a real instance the working view answers the same
    /// as the document does.
    pub fn instance_override_counts(&self, c: &kt_model::ClipEvent) -> (usize, usize) {
        let Some(all) = self.model.project().sequence(c.media).map(|s| &s.published) else {
            return (0, 0);
        };
        let n = c.overrides.iter().filter(|o| all.iter().any(|p| p.id == o.pub_id)).count();
        (n, all.len())
    }

    /// Take the field-label request and execute it (end of frame).
    pub(crate) fn drain_publish_req(&mut self) {
        let Some(req) = crate::widgets::publish_sink::take() else { return };
        match req {
            crate::widgets::PublishReq::Publish { seq, addr, ty, label } => {
                self.publish_dialog =
                    Some(PublishDialog { seq, addr, ty, name: label, focus: true });
            }
            crate::widgets::PublishReq::Unpublish { seq, id } => {
                // Removal FROM THE LABEL — in the same place as publishing: the path is one
                // and the same (rule 17). Asking for confirmation here
                // is also mandatory — the overrides will die in all places.
                let n = self.override_uses(seq, id);
                if n == 0 {
                    self.apply(Command::UnpublishParam { seq, id });
                } else {
                    self.open_published_win(seq);
                    if let Some(w) = self.published_win.as_mut() {
                        w.confirm = Some((id, n));
                    }
                }
            }
        }
    }

    /// Stand INSIDE the strip for the author’s work. The entry body is the same as
    /// the verb “enter” (rule 17): a second way of getting inside must not
    /// be created. `false` — there is nothing to enter through, and this is said.
    fn enter_reel_for_authoring(&mut self, seq: MediaId) -> bool {
        match self.model.project().clips.iter().find(|c| c.media == seq).map(|c| c.id) {
            Some(inst) => self.enter_nest(inst),
            // the strip is not in the edit: there is nothing to enter through, and staying silent
            // about this is not allowed (rule 16)
            None => {
                self.warn(tr(k::msg::NEST_NOT_PLACED));
                return false;
            }
        }
        self.model.editing() == Some(seq)
    }

    /// Open the strip’s “Public parameters” window.
    ///
    /// Entering is PART of opening: publish commands edit the strip
    /// you are standing in, and a window editing the wrong one would be a trap worse than
    /// a missing window. The entry body is the same as that of the verb “enter”
    /// (rule 17).
    pub fn open_published_win(&mut self, seq: MediaId) {
        if self.model.editing() != Some(seq) && !self.enter_reel_for_authoring(seq) {
            return;
        }
        if self.published_win.as_ref().is_some_and(|w| w.seq == seq) {
            return;
        }
        self.gesture_counter += 1;
        self.published_win = Some(PublishedWin {
            seq,
            gesture: self.gesture_counter,
            backup: self.published_of(seq),
            confirm: None,
            open: true,
        });
    }

    /// Dialog for the name of the published handle.
    pub(crate) fn draw_publish_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.publish_dialog.take() else { return };
        let mut go = false;
        let mut cancel = false;
        let mut open = true;
        // LIVE preview: what exactly will go outside under this name
        let doc = self.model.document();
        let media: Vec<(MediaId, String)> =
            doc.media.iter().map(|m| (m.id, m.path.clone())).collect();
        let shown = doc
            .addr_value(Some(d.seq), &d.addr)
            .map(|v| value_text(&v, &|m| media_name(&media, m)))
            .unwrap_or_else(|| tr(k::pubp::INST_TARGET_GONE).to_string());
        egui::Window::new(tr(k::pubp::DIALOG_TITLE))
            .id(egui::Id::new("pub_name"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::pubp::DIALOG_NAME));
                    let resp = ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(220.0));
                    crate::widgets::field_probe::note("pub.dialog.name", resp.rect);
                    if d.focus {
                        resp.request_focus();
                        d.focus = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                });
                ui.label(egui::RichText::new(crate::tf!(k::pubp::TEMPLATE, &shown)).weak().small());
                ui.horizontal(|ui| {
                    let r = ui.button(tr(k::pubp::DIALOG_DO));
                    crate::widgets::field_probe::note("pub.dialog.ok", r.rect);
                    if r.clicked() {
                        go = true;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if go && !d.name.trim().is_empty() {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("pub_name")));
            // The command NAMES the strip (R.63 item 11), so entering it is no longer
            // required for confirmation: “if we publish a parameter without entering the
            // composition fully — why would we automatically enter the composition fully”
            // (the owner).
            // The person stays where they were — at the root with an expanded
            // strip, or in a neighboring strip.
            let param = PublishedParam {
                id: self.next_pub_id(d.seq),
                name: d.name.trim().to_string(),
                target: d.addr.clone(),
                ty: d.ty.clone(),
                range: PubRange::default(),
            };
            self.apply(Command::PublishParam { seq: d.seq, param: Box::new(param) });
            return;
        }
        if cancel || !open {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("pub_name")));
            return;
        }
        self.publish_dialog = Some(d);
    }

    /// The strip’s “Public parameters” window.
    pub(crate) fn draw_published_win(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.published_win.take() else { return };
        // the strip vanished under undo — the window has nothing to talk about
        if self.model.document().sequence(w.seq).is_none() {
            return;
        }
        // the window has ONE strip and it is named — the commands carry its address (R.63 item 11)
        let seq = w.seq;
        let mut list = self.published_of(seq);
        let mut cmds: Vec<Command> = Vec::new();
        let mut cancel = false;
        let mut confirm_now: Option<(PubId, usize)> = None;
        let mut do_unpublish: Option<PubId> = None;
        let mut order: Option<Vec<PubId>> = None;
        let mut done = false;
        let title = format!(
            "{} — {}",
            tr(k::pubp::LIST_TITLE),
            self.model.document().sequence(w.seq).map(|s| s.name.clone()).unwrap_or_default()
        );
        // The target’s passport is the single source of limits (R.60). We take it
        // BEFORE the window: `self` is already borrowed inside the window closure. The row runs
        // IN PARALLEL with `list` — the order of rows changes by a command, that is,
        // from the next frame, and they cannot diverge within a frame.
        let specs: Vec<Option<kt_model::ParamSpec>> = {
            let doc = self.model.document();
            list.iter().map(|p| doc.addr_spec(Some(w.seq), &p.target)).collect()
        };
        let mut open = w.open;
        egui::Window::new(title)
            .id(egui::Id::new(("published_win", w.seq.0)))
            .open(&mut open)
            .resizable(true)
            .default_width(460.0)
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                if list.is_empty() {
                    ui.label(tr(k::pubp::LIST_EMPTY));
                    return;
                }
                let n = list.len();
                for i in 0..n {
                    let id = list[i].id;
                    ui.horizontal(|ui| {
                        // the order of rows is part of the author’s interface
                        // (Resolve 21 introduced it separately, F-TL-012)
                        let up =
                            ui.add_enabled(i > 0, egui::Button::new(crate::icons::LANE_UP).small());
                        crate::widgets::hint(&up, tr(k::pubp::LIST_HINT_ORDER));
                        let down = ui.add_enabled(
                            i + 1 < n,
                            egui::Button::new(crate::icons::LANE_DOWN).small(),
                        );
                        if up.clicked() || down.clicked() {
                            let mut o: Vec<PubId> = list.iter().map(|p| p.id).collect();
                            let j = if up.clicked() { i - 1 } else { i + 1 };
                            o.swap(i, j);
                            order = Some(o);
                        }
                        // NAME — the author’s text: drawn as is, bypassing
                        // the catalog (t684)
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut list[i].name).desired_width(180.0),
                        );
                        crate::widgets::field_probe::note(
                            &format!("pub.row.name.{}", id.0),
                            r.rect,
                        );
                        if r.changed() {
                            cmds.push(Command::SetPublished {
                                seq,
                                param: Box::new(list[i].clone()),
                            });
                        }
                        // narrowing of limits — only for a number: for the other
                        // types “between variants” there is nothing, and a dead
                        // field would be a violation of rule 9.
                        //
                        // The default is the TARGET’S PASSPORT (R.60): an empty narrowing
                        // was drawn as zero (“min and max values always default to 0”),
                        // although the target has its own
                        // limits. There is no copy of the passport in the publication —
                        // it would diverge from the target at the first edit
                        // inside the strip, just as a copy of the template value would have
                        // diverged.
                        if list[i].ty == ParamType::Num
                            && let Some(spec) = specs.get(i).cloned().flatten()
                        {
                            let mut lo = list[i].range.min.unwrap_or(spec.min);
                            let mut hi = list[i].range.max.unwrap_or(spec.max);
                            let mut edited = false;
                            let step = if spec.step > 0.0 { spec.step } else { 0.05 };
                            for (label, v, is_min) in [
                                (tr(k::pubp::RANGE_MIN), &mut lo, true),
                                (tr(k::pubp::RANGE_MAX), &mut hi, false),
                            ] {
                                ui.label(label);
                                // limits and unit — FROM THE TARGET’S PASSPORT, the row is shared
                                let rr = crate::widgets::plain_row(
                                    ui,
                                    &crate::widgets::NumRow {
                                        label: "",
                                        probe: "",
                                        unit: spec.unit,
                                        min: spec.min,
                                        max: spec.max,
                                        step,
                                        slider: None,
                                    },
                                    v,
                                );
                                crate::widgets::hint(&rr, tr(k::pubp::RANGE_HINT));
                                crate::widgets::field_probe::note(
                                    &format!(
                                        "pub.row.{}.{}",
                                        if is_min { "min" } else { "max" },
                                        id.0
                                    ),
                                    rr.rect,
                                );
                                if rr.changed() {
                                    edited = true;
                                }
                            }
                            // “To the target’s limits” — remove the narrowing entirely.
                            // Without this button a narrowed handle could not be
                            // returned to its original state other than by
                            // removing it and declaring it anew (rule 9).
                            let narrowed = !list[i].range.is_empty();
                            let rst = ui
                                .add_enabled(narrowed, egui::Button::new(tr(k::pubp::RANGE_RESET)));
                            crate::widgets::hint(&rst, tr(k::pubp::RANGE_RESET_HINT));
                            crate::widgets::field_probe::note(
                                &format!("pub.row.range_reset.{}", id.0),
                                rst.rect,
                            );
                            if rst.clicked() {
                                list[i].range = PubRange::default();
                                edited = true;
                            } else if edited {
                                list[i].range = PubRange::narrowed(&spec, lo, hi);
                            }
                            if edited {
                                cmds.push(Command::SetPublished {
                                    seq,
                                    param: Box::new(list[i].clone()),
                                });
                            }
                        }
                        let rm = ui.button(crate::icons::CLEAR);
                        crate::widgets::field_probe::note(
                            &format!("pub.row.remove.{}", id.0),
                            rm.rect,
                        );
                        crate::widgets::hint(&rm, tr(k::pubp::UNPUBLISH));
                        if rm.clicked() {
                            confirm_now = Some((id, 0));
                        }
                    });
                }
                });
                crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(tr(k::pubp::DONE)).clicked() {
                        done = true;
                    }
                    // “Cancel” returns the set under the SAME gesture token —
                    // the whole session of the window remains one history step
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
                });
            });

        // --- removal confirmation -----------------------------------------
        if let Some((id, _)) = confirm_now {
            w.confirm = Some((id, self.override_uses(w.seq, id)));
        }
        if let Some((id, n)) = w.confirm {
            let name = list.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
            let mut close = false;
            egui::Window::new(tr(k::pubp::UNPUBLISH_TITLE))
                .id(egui::Id::new("pub_unpublish"))
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    // THE NUMBER IS NAMED ALOUD: removal kills the overrides
                    // in all places of the tree, and learning about it after the fact
                    // would be a loss of work.
                    ui.label(crate::tf!(k::pubp::UNPUBLISH_ASK, &name, n));
                    ui.horizontal(|ui| {
                        let r = ui.button(tr(k::pubp::UNPUBLISH));
                        crate::widgets::field_probe::note("pub.unpublish.ok", r.rect);
                        if r.clicked() {
                            do_unpublish = Some(id);
                            close = true;
                        }
                        if ui.button(tr(k::fx::CANCEL)).clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                w.confirm = None;
            }
        }

        for c in cmds {
            self.apply_coalesced(c, w.gesture);
        }
        if let Some(o) = order {
            self.apply_coalesced(Command::ReorderPublished { seq, order: o }, w.gesture);
        }
        if let Some(id) = do_unpublish {
            self.apply_coalesced(Command::UnpublishParam { seq, id }, w.gesture);
        }
        if cancel {
            // under the same token: the return does not create a second history step.
            // Overrides of removed handles are not revived by this — the confirmation
            // said so plainly, and full restoration is Ctrl+Z.
            let now = self.published_of(w.seq);
            for p in &w.backup {
                if now.iter().any(|x| x.id == p.id) {
                    self.apply_coalesced(
                        Command::SetPublished { seq, param: Box::new(p.clone()) },
                        w.gesture,
                    );
                } else {
                    self.apply_coalesced(
                        Command::PublishParam { seq, param: Box::new(p.clone()) },
                        w.gesture,
                    );
                }
            }
            let o: Vec<PubId> = self
                .published_of(w.seq)
                .iter()
                .map(|p| p.id)
                .filter(|id| w.backup.iter().any(|p| p.id == *id))
                .collect();
            if o.len() == self.published_of(w.seq).len() {
                self.apply_coalesced(Command::ReorderPublished { seq, order: o }, w.gesture);
            }
            return;
        }
        if !open || done {
            return;
        }
        w.open = open;
        self.published_win = Some(w);
    }

    /// The instance’s “Composition parameters” window — the CONSUMER’s side.
    /// The HEAD of the chain strip: the instance’s “Composition parameters” (R.63
    /// item 12).
    ///
    /// The owner: “it is pretty damn stupidly done that composition parameters
    /// open in a separate window, when they could be placed in the same
    /// fx-chain for convenience. I am banning any agents from spawning
    /// any extra windows without an obvious purpose.” The form is the same
    /// down to the line — only its home has moved: instead of its own window it stands
    /// as the head of the strip, next to the generator and the leading node.
    pub(crate) fn draw_inst_head(
        hc: &InstHeadCtx<'_>,
        ui: &mut egui::Ui,
        view: &mut crate::automation::AutoView,
        query: &mut String,
    ) -> InstHeadOut {
        let doc = hc.doc;
        let (clip, seq) = (hc.inst.id, hc.seq);
        let inst = hc.inst.clone();
        let list: Vec<PublishedParam> =
            doc.sequence(seq).map(|s| s.published.clone()).unwrap_or_default();
        if list.is_empty() {
            // EMPTY — but not silently (rule 16): handles are declared inside
            ui.label(egui::RichText::new(tr(k::pubp::INST_NONE)).weak().small());
            return InstHeadOut::default();
        }
        // The domain of the override curve is the INSTANCE’S MATERIAL (the same as
        // the strip time): exactly what the model later converts to the material of
        // the target by the same `clip_material_at` (§469).
        let frame_step = 90_000 / doc.settings.fps.max(1) as i64;
        let at = kt_model::clip_material_at(
            inst.source_in,
            inst.start,
            inst.duration,
            inst.reversed,
            frame_step,
            hc.playhead,
        );
        // The graph axis is the INSTANCE’S MATERIAL, the same domain in which
        // the key sits; for a mirrored instance the ends swap places, so
        // min/max, not “first-last”.
        let mat_range = {
            let a = kt_model::clip_material_at(
                inst.source_in,
                inst.start,
                inst.duration,
                inst.reversed,
                frame_step,
                inst.start,
            );
            let b = kt_model::clip_material_at(
                inst.source_in,
                inst.start,
                inst.duration,
                inst.reversed,
                frame_step,
                inst.start + inst.duration,
            );
            (kt_time::Tick(a.0.min(b.0)), kt_time::Tick(a.0.max(b.0).max(a.0.min(b.0) + 1)))
        };
        let (tl_view, axis_linked, playhead) = (hc.tl_view, hc.axis_linked, hc.playhead);
        let media: Vec<(MediaId, String)> = doc
            .media
            .iter()
            // A STRIP is not offered as a value: a sequence has no `MediaRef`,
            // and substituting a source with a strip is a different mechanism (R.50)
            .filter(|m| doc.sequence(m.id).is_none())
            .map(|m| (m.id, m.path.clone()))
            .collect();
        // Handle passports: the target’s LIMITS, narrowed by the author, and the READY
        // label — the author’s name (R.59: a non-empty `label` of a dynamic
        // owner is drawn verbatim, bypassing the catalog, t684).
        let specs: Vec<kt_model::ParamSpec> = list
            .iter()
            .map(|p| {
                let base = doc.addr_spec(Some(seq), &p.target);
                let mut sp = base.unwrap_or(kt_model::ParamSpec {
                    name: Default::default(),
                    ty: p.ty.clone(),
                    min: 0.0,
                    max: 1.0,
                    default: 0.0,
                    step: 0.01,
                    unit: kt_model::ParamUnit::None,
                    kind: kt_model::ParamKind::Smooth,
                    label: Default::default(),
                });
                sp.min = p.range.min.unwrap_or(sp.min).max(sp.min);
                sp.max = p.range.max.unwrap_or(sp.max).min(sp.max);
                sp.name = std::borrow::Cow::Owned(pub_key(p.id));
                sp.label = std::borrow::Cow::Owned(p.name.clone());
                sp
            })
            .collect();
        // Curves of ALL handles — in one row: the widget draws curves of one
        // carrier, and the carrier here is the instance. In the model they lie by
        // override, so for the window’s duration the name is replaced with
        // the handle number and returned on write.
        let mut auto: Vec<kt_model::ParamCurve> = Vec::new();
        for p in &list {
            if let Some(o) = inst.overrides.iter().find(|o| o.pub_id == p.id)
                && let Some(c) = o.curves.iter().find(|c| c.param == p.target.param)
            {
                let mut c = c.clone();
                c.param = pub_key(p.id);
                auto.push(c);
            }
        }
        let auto_before = auto.clone();
        let total = list.len();
        let over_n = inst.overrides.iter().filter(|o| list.iter().any(|p| p.id == o.pub_id)).count();
        let title = format!(
            "{} — {}",
            tr(k::pubp::INST_TITLE),
            doc.sequence(seq).map(|s| s.name.clone()).unwrap_or_default()
        );
        let mut ops: Vec<Command> = Vec::new();
        // what to edit in the model at the end of the frame: handles with a changed
        // value, handles with a reset override, and the effective
        // values of all rows (a curve needs a base if one is created)
        let mut dirty: Vec<PubId> = Vec::new();
        let mut drop_curve: Vec<PubId> = Vec::new();
        let mut values: Vec<ParamValue> = Vec::new();
        let mut axis_req = crate::automation::AxisReq::default();
        let mut seek: Option<kt_time::Tick> = None;
        let mut hover_out: Option<String> = None;
        let _ = &title;
        {
            let ui = &mut *ui;
                ui.label(
                    egui::RichText::new(crate::tf!(k::pubp::OVERRIDDEN, over_n, total))
                        .weak()
                        .small(),
                );
                egui::Grid::new(("inst_params_grid", clip.0))
                    .num_columns(4)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        for (i, p) in list.iter().enumerate() {
                            let ov = inst.overrides.iter().find(|o| o.pub_id == p.id).cloned();
                            let tmpl = doc.published_value(seq, p.id);
                            let cur = ov
                                .as_ref()
                                .map(|o| o.value.clone())
                                .or_else(|| tmpl.clone())
                                .unwrap_or(ParamValue::Num(0.0));
                            // A ROW WITHOUT AN OVERRIDE — muted:
                            // it is visible that the instance so far differs from the strip in
                            // nothing (rule 13: colour instead of a column)
                            let dim = ov.is_none();
                            // AUTHOR’S NAME — as is, bypassing the catalog (t684)
                            let name = egui::RichText::new(&p.name);
                            let name = if dim { name.weak() } else { name };
                            ui.label(name);
                            let mut next = cur.clone();
                            let mut bind =
                                crate::widgets::CurveBind { curves: &mut auto, sel: view.sel, at };
                            let changed = draw_value_row(
                                ui,
                                p,
                                &specs[i],
                                &mut next,
                                &mut bind,
                                &media,
                                query,
                                doc.addr_type(Some(seq), &p.target),
                            );
                            // the TEMPLATE value beside it — read live,
                            // there is no copy in the file (the owner’s decision)
                            let t = tmpl
                                .as_ref()
                                .map(|v| value_text(v, &|m| media_name(&media, m)))
                                .unwrap_or_else(|| tr(k::pubp::INST_TARGET_GONE).to_string());
                            ui.label(
                                egui::RichText::new(crate::tf!(k::pubp::TEMPLATE, &t))
                                    .weak()
                                    .small(),
                            );
                            let reset =
                                ui.add_enabled(ov.is_some(), egui::Button::new(tr(k::pubp::RESET)));
                            // The ACCESSIBLE NAME is compound, the visible label
                            // is short (the same trick as the picker cells in
                            // R.54): there are as many reset buttons in the window
                            // as there are handles, and “Reset” does not tell them apart
                            // either for a screen reader or for the harness.
                            {
                                let name = format!("{} {}", tr(k::pubp::RESET), p.name);
                                let on = ov.is_some();
                                reset.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        on,
                                        name.clone(),
                                    )
                                });
                            }
                            crate::widgets::field_probe::note(
                                &format!("inst.reset.{}", p.id.0),
                                reset.rect,
                            );
                            crate::widgets::hint(&reset, tr(k::pubp::RESET_HINT));
                            if reset.clicked() {
                                ops.push(Command::ClearOverride { id: clip, pub_id: p.id });
                                drop_curve.push(p.id);
                            } else if changed {
                                dirty.push(p.id);
                            }
                            values.push(next);
                            ui.end_row();
                        }
                    });

                // ---- AUTOMATION ------------------------------------
                //
                // The same widget as the effect slot and the generator
                // (the R.34 decision “one animation mechanism for everything”). Before
                // R.60 the window drew curves BLIND: a nameless glyph
                // created a curve, and every edit of a field put a key at the
                // cursor, and there was nowhere to look at the curve.
                let curvable: Vec<usize> =
                    (0..list.len()).filter(|&i| list[i].ty.curvable()).collect();
                if !curvable.is_empty() {
                    ui.separator();
                    ui.horizontal(|ui| {
                        // the section header says WHOSE curves these are:
                        // their axis is the instance’s time, not the strip’s
                        let ttl = ui.label(tr(k::pubp::AUTO_TITLE));
                        crate::widgets::hint(&ttl, tr(k::pubp::AUTO_HINT));
                        let free: Vec<usize> = curvable
                            .iter()
                            .copied()
                            .filter(|&i| !auto.iter().any(|c| c.param == pub_key(list[i].id)))
                            .collect();
                        if !free.is_empty() {
                            // a MENU, not a glyph: the action must have a
                            // name — both in the interface and for the harness
                            let mb = ui.menu_button(tr(k::fx::AUTO_ADD), |ui| {
                                for &i in &free {
                                    // the item — the AUTHOR’S NAME verbatim (t684)
                                    let item = ui.button(&list[i].name);
                                    crate::widgets::field_probe::note(
                                        &format!("inst.auto_add.{}", list[i].id.0),
                                        item.rect,
                                    );
                                    if item.clicked() {
                                        // the starting curve is a constant
                                        // of the effective value over the whole
                                        // material of the instance, as for the slot
                                        let v0 = values
                                            .get(i)
                                            .and_then(|v| v.as_num())
                                            .unwrap_or(specs[i].default);
                                        auto.push(kt_model::ParamCurve {
                                            param: pub_key(list[i].id),
                                            keys: vec![
                                                kt_model::AutoKey::at(mat_range.0, v0),
                                                kt_model::AutoKey::at(mat_range.1, v0),
                                            ],
                                            source: Default::default(),
                                        });
                                        view.active = auto.len() - 1;
                                        view.sel = None;
                                        dirty.push(list[i].id);
                                        ui.close();
                                    }
                                }
                            });
                            crate::widgets::hint(&mb.response, tr(k::pubp::AUTO_ADD_HINT));
                            crate::widgets::field_probe::note("inst.auto_add", mb.response.rect);
                        }
                    });
                    if !auto.is_empty() {
                        let to_tl = |t: kt_time::Tick| {
                            kt_model::clip_timeline_at(
                                inst.source_in,
                                inst.start,
                                inst.duration,
                                inst.reversed,
                                frame_step,
                                t,
                            )
                        };
                        let to_mat = |t: kt_time::Tick| {
                            kt_model::clip_material_at(
                                inst.source_in,
                                inst.start,
                                inst.duration,
                                inst.reversed,
                                frame_step,
                                t,
                            )
                        };
                        // link targets — events INSIDE the instance’s strip (R.67)
                        let inst_links = || -> Vec<crate::automation::LinkEvent> {
                            doc.sequence(seq)
                                .map(|s| {
                                    let mut root = kt_model::Project::new(doc.settings.clone());
                                    root.clips = s.clips.clone();
                                    crate::automation::link_events(&root, kt_model::ClipId(u64::MAX))
                                })
                                .unwrap_or_default()
                        };
                        let actx = crate::automation::AutoCtx {
                            links: Some(&inst_links),
                            specs: &specs,
                            range: mat_range,
                            view: crate::automation::view_in_curve(&to_mat, tl_view),
                            to_timeline: &to_tl,
                            to_curve: &to_mat,
                            frame_step,
                            linked: axis_linked,
                            default_shape: doc.settings.default_shape,
                        };
                        let top = ui.cursor().min;
                        let res = crate::automation::draw(
                            ui,
                            ("instauto", clip.0),
                            &actx,
                            &mut auto,
                            view,
                            playhead,
                            &mut hover_out,
                        );
                        crate::widgets::field_probe::note(
                            "inst.auto",
                            egui::Rect::from_min_max(top, ui.cursor().min),
                        );
                        if res.changed {
                            // the widget does not report which handle moved —
                            // we compare against the snapshot taken at the start of the frame.
                            for p in &list {
                                let k = pub_key(p.id);
                                let now = auto.iter().find(|c| c.param == k);
                                let was = auto_before.iter().find(|c| c.param == k);
                                if now != was {
                                    dirty.push(p.id);
                                }
                            }
                        }
                        seek = res.seek;
                        axis_req = axis_req.merge(&res);
                    }
                }

        }
        // One command per handle: the row’s value plus its curve, the name
        // of which is returned as the TARGET one — the one `apply_overrides` expects.
        dirty.retain(|id| !drop_curve.contains(id));
        dirty.dedup();
        for (i, p) in list.iter().enumerate() {
            if !dirty.contains(&p.id) {
                continue;
            }
            let key = pub_key(p.id);
            let curves: Vec<kt_model::ParamCurve> = auto
                .iter()
                .filter(|c| c.param == key)
                .map(|c| {
                    let mut c = c.clone();
                    c.param = p.target.param.clone();
                    c
                })
                .collect();
            let value = values.get(i).cloned().unwrap_or(ParamValue::Num(0.0));
            ops.push(Command::SetOverride {
                id: clip,
                ov: Box::new(Override { pub_id: p.id, value, curves }),
            });
        }
        if let Some(h) = hover_out {
            crate::widgets::hint_sink::put(&h);
        }
        // Commands are applied by the FX WINDOW under its own gesture token: the head lives in
        // it, and an edit of a handle has no right to start its own history step
        // next to an edit of the slot.
        InstHeadOut { seek, axis: axis_req, ops }
    }
}

fn media_name(media: &[(MediaId, String)], id: MediaId) -> String {
    media
        .iter()
        .find(|(m, _)| *m == id)
        .map(|(_, p)| {
            std::path::Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.clone())
        })
        .unwrap_or_else(|| format!("#{}", id.0))
}

/// One row of the consumer window — a widget by the handle’s TYPE.
///
/// The field edits exactly what it shows (the same rule as for
/// `param_row_bound`): if there is a curve, its key is edited in the domain of the
/// instance; if not — the value itself.
#[allow(clippy::too_many_arguments)]
fn draw_value_row(
    ui: &mut egui::Ui,
    p: &PublishedParam,
    spec: &kt_model::ParamSpec,
    v: &mut ParamValue,
    bind: &mut crate::widgets::CurveBind<'_>,
    media: &[(MediaId, String)],
    query: &mut String,
    live_ty: Option<ParamType>,
) -> bool {
    let probe = format!("inst.value.{}", p.id.0);
    let key = pub_key(p.id);
    let mut changed = false;
    match v {
        ParamValue::Num(x) => {
            // The ONLY place where “key or constant” is decided:
            // a key is written at the cursor only when the curve is CREATED and
            // DRAWN (rule R.60). Previously a nameless glyph created the curve, and the window
            // turned into a hidden write mode.
            let ci = bind.curves.iter().position(|c| c.param == key);
            let mut shown = match ci {
                Some(i) => {
                    let sel = match bind.sel {
                        Some((c, k)) if c == i && k < bind.curves[i].keys.len() => Some(k),
                        _ => None,
                    };
                    crate::widgets::bound_value(&bind.curves[i], sel, bind.at)
                }
                None => *x,
            };
            // the target’s LIMITS, narrowed by the author — both already in the passport
            let step = if spec.step > 0.0 { spec.step } else { 0.05 };
            let r = crate::widgets::plain_row(
                ui,
                &crate::widgets::NumRow {
                    label: "",
                    probe: &probe,
                    unit: spec.unit,
                    min: spec.min,
                    max: spec.max,
                    step,
                    slider: None,
                },
                &mut shown,
            );
            if r.changed() {
                match ci {
                    // The KEY sits in the domain of the INSTANCE — that is the strip
                    // time; the model will convert it to the target’s material itself.
                    Some(i) => {
                        let sel = match bind.sel {
                            Some((c, k)) if c == i && k < bind.curves[i].keys.len() => Some(k),
                            _ => None,
                        };
                        let at = bind.at;
                        crate::widgets::bound_apply(&mut bind.curves[i], sel, at, shown);
                    }
                    None => *x = shown,
                }
                changed = true;
            }
        }
        ParamValue::Text(s) => {
            let r = ui.add(egui::TextEdit::singleline(s).desired_width(200.0));
            crate::widgets::field_probe::note(&probe, r.rect);
            changed = r.changed();
        }
        ParamValue::Color(rgba) => {
            let mut col = egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
            let r = ui.color_edit_button_srgba(&mut col);
            crate::widgets::field_probe::note(&probe, r.rect);
            if r.changed() {
                *rgba = col.to_srgba_unmultiplied();
                changed = true;
            }
        }
        ParamValue::Flag(b) => {
            let r = ui.checkbox(b, "");
            crate::widgets::field_probe::note(&probe, r.rect);
            changed = r.changed();
        }
        ParamValue::Choice(sel) => {
            // The variants belong to the TARGET and are read from it live: a copy in the
            // file would silently diverge from a new build (§469).
            let variants: Vec<String> = match live_ty {
                Some(ParamType::Choice { variants }) => {
                    variants.iter().map(|s| s.to_string()).collect()
                }
                _ => vec![sel.clone()],
            };
            let r = egui::ComboBox::from_id_salt(("inst-choice", p.id.0))
                .selected_text(sel.clone())
                .show_ui(ui, |ui| {
                    for var in &variants {
                        if ui.selectable_label(var == sel, var).clicked() {
                            *sel = var.clone();
                            changed = true;
                        }
                    }
                });
            crate::widgets::field_probe::note(&probe, r.response.rect);
        }
        ParamValue::Media(m) => {
            ui.label(media_name(media, *m));
            if let Some(Some(pick)) =
                crate::widgets::media_picker(ui, ("inst-media", p.id.0), media, query)
            {
                *m = pick;
                changed = true;
            }
        }
    }
    changed
}
