//! Media bin (a port of SourceAreaWidget + item 11 of round 6): cards with previews,
//! scrubbing by moving the mouse over the preview (hover x → file moment → thumbnail from the
//! shared cache/sidecar DB), name + duration; click — place at the cursor.
//!
//! Lessons from CLAUDE.md: repaint on a pending thumbnail — request_repaint_after,
//! not unconditional; textures — the application's shared thumb_textures/LRU;
//! “last good” for media — the preview does not flicker while buckets are not fully loaded.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::thumbs::{THUMB_STEP_LADDER, ThumbKey, ThumbState, thumb_key_t, thumb_source_step};
use eframe::egui;
use kt_model::{Command, MediaId};

const PREVIEW_W: f32 = 128.0;
const PREVIEW_H: f32 = 72.0;
/// Logical scrub positions per file (item 14 of round 7: there were 8 — “too few
/// preview images”); filmstrip thumbnails are picked up by near-search in the DB.
const BIN_SCRUB_SLOTS: f32 = 48.0;

/// The width of the autoscroll band at the top/bottom edge of the visible part of the
/// column (item 1 of round 16). Inside it a drag does not “carry out” the card, it scrolls
/// the list — as in the timeline.
const BIN_EDGE_MARGIN: f32 = 28.0;

/// How far the cursor must LEAVE the visible band for it to count as a
/// carry-out to the timeline (item 1 of round 17). A tap at the edge is “scroll the list”,
/// not “carry out”: the user complained that on a quick tap the card flies back
/// while the view stays scrolled. Vertically the margin is larger than the width of the
/// autoscroll band, horizontally — the minimal one
/// (a sideways exit unambiguously means carry-out).
const BIN_ESCAPE_SLACK: egui::Vec2 = egui::vec2(6.0, 36.0);

/// The bin's vertical autoscroll speed: the same curve as the timeline's
/// “jump” zones (heat^4.15) — fast right at the edge, barely noticeable just above.
/// It returns the CONTENT OFFSET in points per frame:
/// down (cursor at the bottom edge) — negative, up — positive.
pub fn bin_edge_scroll(view: egui::Rect, y: f32) -> f32 {
    let margin = BIN_EDGE_MARGIN.min(view.height() * 0.25).max(1.0);
    let heat = if y < view.top() + margin {
        -((view.top() + margin - y) / margin).clamp(0.0, 1.0)
    } else if y > view.bottom() - margin {
        ((y - (view.bottom() - margin)) / margin).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if heat == 0.0 {
        return 0.0;
    }
    // sign: heat>0 (bottom) → the content moves UP, that is, the scroll delta
    // is negative (see Ui::scroll_with_delta)
    -heat.signum() * (heat.abs().powf(4.15) * 0.18 * 240.0).clamp(1.0, 42.0)
}

/// The raw vertical wheel per frame in points (item 1 of round 17). We take
/// `Event::MouseWheel` BEFORE egui processes it: `smooth_scroll_delta` is already zeroed
/// during an active drag by the ScrollArea gate.
fn raw_wheel_y(ui: &egui::Ui) -> f32 {
    ui.input(|i| {
        i.raw
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                    egui::MouseWheelUnit::Point => delta.y,
                    egui::MouseWheelUnit::Line => delta.y * 20.0,
                    egui::MouseWheelUnit::Page => delta.y * 200.0,
                }),
                _ => None,
            })
            .sum()
    })
}

/// The payload of dragging a card out of the bin (item 5 of round 8).
#[derive(Clone, Copy, Debug)]
pub struct BinDrag(pub MediaId);

/// The state of dragging a card inside the bin (item 8 of round 9).
#[derive(Clone, Debug)]
pub struct BinDragState {
    pub id: MediaId,
    /// The cursor offset inside the card.
    pub grab: egui::Vec2,
    /// The original position — we roll back to it on carry-out to the timeline.
    pub start: egui::Pos2,
    /// The cursor has already left the bin: from now on it is a carry-out, the layout does not change.
    pub escaped: bool,
    /// Who travels together (group or selection) and from which positions: the delta
    /// is applied to all at once (item 13 of round 11).
    pub mates: Vec<(MediaId, egui::Pos2)>,
}

/// Reel verbs in the bin (R.55, C6). A separate type, not a string: the menu
/// is built in a closure, and the action must happen OUTSIDE it — otherwise `self`
/// is taken by a borrow.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReelVerb {
    Expand,
    Open,
    Rename,
    /// The “Public parameters” window of the reel (R.59, stage 3).
    Published,
    Remove,
}

impl KtApp {
    /// Index of the group the card belongs to.
    pub fn bin_group_of(&self, id: MediaId) -> Option<usize> {
        self.bin_groups.iter().position(|g| g.members.contains(&id))
    }

    /// Whom to pull together with the card: its whole group, and if there is no group —
    /// the whole selection (when the card is part of it), otherwise the card alone.
    pub fn bin_drag_set(&self, id: MediaId) -> Vec<MediaId> {
        if let Some(gi) = self.bin_group_of(id) {
            return self.bin_groups[gi].members.clone();
        }
        if self.bin_sel.contains(&id) {
            return self.bin_sel.iter().copied().collect();
        }
        vec![id]
    }

    /// Whether the card is locked (the group's lock).
    pub fn bin_locked(&self, id: MediaId) -> bool {
        self.bin_group_of(id).is_some_and(|gi| self.bin_groups[gi].locked)
    }

    /// Gather the selected cards into a group (a single one is also a group:
    /// so that “lock on a card” does not introduce a second mechanism).
    pub fn bin_group_selection(&mut self, name: String) -> Option<usize> {
        let members: Vec<MediaId> = self.bin_sel.iter().copied().collect();
        if members.is_empty() {
            return None;
        }
        // a card belongs to at most one group
        for m in &members {
            if let Some(gi) = self.bin_group_of(*m) {
                self.bin_groups[gi].members.retain(|x| x != m);
            }
        }
        self.bin_groups.retain(|g| !g.members.is_empty());
        self.bin_groups.push(kt_model::BinGroup { name, members, locked: false });
        self.dirty_ui = true;
        Some(self.bin_groups.len() - 1)
    }

    pub fn bin_ungroup(&mut self, gi: usize) {
        if gi < self.bin_groups.len() {
            self.bin_groups.remove(gi);
            self.dirty_ui = true;
        }
    }

    pub fn draw_bin(&mut self, ui: &mut egui::Ui) {
        // the card rectangles live for ONE frame: a card could have been moved by
        // scrolling, collapsed into a group or disappeared with the media. In the
        // free mode the mode itself computes them (group frames are drawn
        // BEFORE the cards), in the grid — `draw_bin_card` fills them in.
        if !self.bin_free {
            self.bin_card_rects.clear();
        }
        // the background goes FIRST: in egui an interaction registered later lies
        // higher, so icons and cards added below take their own clicks and
        // hover, and the background gets only what is outside them
        self.bin_background_menu(ui);
        // The mode switch is an ICON, not a checkbox (item 8 of round 8: no more checkboxes
        // “out of place”)
        ui.horizontal(|ui| {
            let free = self.bin_free;
            let r = ui
                .add_sized([22.0, 20.0], egui::Button::selectable(free, crate::icons::ARROWS_OUT));
            if self.hint(r, tr(k::bin::HINT_FREE)).clicked() {
                self.bin_free = true;
            }
            let r = ui.add_sized(
                [22.0, 20.0],
                egui::Button::selectable(!free, crate::icons::SQUARES_FOUR),
            );
            if self.hint(r, tr(k::bin::HINT_GRID)).clicked() {
                self.bin_free = false;
            }
        });
        let offline = self.offline.lock().unwrap().clone();
        // offline→online: the negative thumbnail caches are stale (item 1 of round 7)
        for id in self.bin_last_offline.difference(&offline) {
            self.thumbs.invalidate_media(*id);
        }
        self.bin_last_offline = offline.clone();
        // Bin cards — files, generators AND REELS (R.55, C6).
        //
        // Until this round there were no reels in the bin at all: a composition could go into a
        // second place only by copy-pasting an already placed instance, and an empty one could not be created at all —
        // a specific one could not be deleted either (“remove unused” removed all of them at once).
        // The owner's claim, verbatim:
        // “there is probably still a heap of things unavailable from the UI”.
        //
        // They are told apart by the SET OF ADDRESSES, not by a string prefix: pseudo-paths are
        // parsed only by `kt_media::factory` (footnote item 211), while a reel has no
        // `MediaRef` ON PURPOSE (item 378) — if we gave it a path, the watchdog would
        // paint it offline, and the index service would try to build
        // ffmpeg peaks for it.
        let reels: std::collections::HashSet<MediaId> =
            self.model.document().sequences.iter().map(|s| s.id).collect();
        let media: Vec<(MediaId, String)> = self
            .model
            .project()
            .media
            .iter()
            .map(|m| (m.id, m.path.clone()))
            .chain(self.model.document().sequences.iter().map(|s| (s.id, s.name.clone())))
            .collect();
        // the card width is computed ONCE from the panel width: inside a
        // wrapped row the available_width shrinks and the cards came out
        // ragged (item 5 of round 8)
        // the card size is NOT inflated (complaint of round 13): a wide column gives
        // more cards in a row, not giant previews
        let card_w = ui.available_width().clamp(56.0, PREVIEW_W);
        let card_h = card_w * PREVIEW_H / PREVIEW_W;
        let row_h = card_h + 18.0;
        let mut to_place: Option<MediaId> = None;
        let mut to_measure: Option<MediaId> = None;
        let mut to_gen: Option<MediaId> = None;
        // a change of the generator KIND from the card (R.59, stage 2)
        let mut to_kind: Option<(MediaId, kt_model::GeneratorKind)> = None;
        let mut to_remove: Option<MediaId> = None;
        let mut pending = false;
        let mut drag_out: Option<MediaId> = None;

        if self.bin_free {
            // --- free layout: positions live in the project ------------
            let cols = ((ui.available_width() / (card_w + 6.0)).floor() as usize).max(1);
            for (i, (id, _)) in media.iter().enumerate() {
                self.bin_layout.entry(*id).or_insert_with(|| {
                    // a new card goes into the first free cell
                    egui::pos2(
                        (i % cols) as f32 * (card_w + 6.0),
                        (i / cols) as f32 * (row_h + 6.0),
                    )
                });
            }
            let rows = (media.len().div_ceil(cols)).max(1);
            let need_h = self
                .bin_layout
                .values()
                .map(|p| p.y + row_h)
                .fold(rows as f32 * (row_h + 6.0), f32::max);
            // the bin background catches the selection frame; the cards are registered
            // LATER and therefore win the hit test (item 13 of round 11)
            let (area, bg) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), need_h),
                egui::Sense::click_and_drag(),
            );
            let add_mode =
                ui.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
            if bg.drag_started()
                && let Some(pos) = bg.interact_pointer_pos()
            {
                if !add_mode {
                    self.bin_sel.clear();
                }
                self.bin_rubber = Some(pos);
            }
            if bg.clicked() && !add_mode {
                self.bin_sel.clear();
            }
            let mut card_rects = std::mem::take(&mut self.bin_card_rects);
            card_rects.clear();
            // the rectangles are computed IN ADVANCE: the group frames are drawn BEFORE
            // the cards and therefore lie beneath them (complaint of round 13)
            for (id, _) in &media {
                let p = self.bin_layout.get(id).copied().unwrap_or_default();
                let max_x = (area.width() - card_w).max(0.0);
                card_rects.push((
                    *id,
                    egui::Rect::from_min_size(
                        area.min + egui::vec2(p.x.min(max_x), p.y),
                        egui::vec2(card_w, row_h),
                    ),
                ));
            }
            self.draw_bin_groups(ui, &card_rects);
            for (id, path) in &media {
                let p = self.bin_layout.get(id).copied().unwrap_or_default();
                // the position is clamped by the ACTUAL panel width: otherwise the
                // card sticks out to the right, the panel grows to fit the content and
                // its separator drifts away from “its own” border (caught t76)
                let max_x = (area.width() - card_w).max(0.0);
                let rect = egui::Rect::from_min_size(
                    area.min + egui::vec2(p.x.min(max_x), p.y),
                    egui::vec2(card_w, row_h),
                );
                let resp = self.draw_bin_card(
                    ui,
                    *id,
                    path,
                    rect,
                    card_w,
                    card_h,
                    &offline,
                    &reels,
                    &mut pending,
                    &mut to_place,
                    &mut to_measure,
                    &mut to_gen,
                    &mut to_kind,
                    &mut to_remove,
                );
                // dragging inside the bin moves the card (and its whole
                // group/selection); the mouse leaving the panel = carry-out
                if resp.drag_started()
                    && let Some(pos) = resp.interact_pointer_pos()
                    && !self.bin_locked(*id)
                {
                    let mates = self
                        .bin_drag_set(*id)
                        .into_iter()
                        .map(|m| (m, self.bin_layout.get(&m).copied().unwrap_or_default()))
                        .collect();
                    self.bin_drag = Some(BinDragState {
                        id: *id,
                        grab: pos - rect.min,
                        start: self.bin_layout.get(id).copied().unwrap_or(p),
                        escaped: false,
                        mates,
                    });
                }
                if resp.dragged()
                    && let Some(st) = self.bin_drag.clone()
                    && st.id == *id
                    && let Some(pos) = resp.interact_pointer_pos()
                {
                    // The sticky “carry-out” mode: as soon as the cursor has left
                    // the bin's border, we do NOT touch the layout and roll the
                    // positions back — otherwise the cards drifted away even on a successful
                    // drop onto the timeline (item 8 of round 9).
                    //
                    // The border is taken from clip_rect — the VISIBLE band of the
                    // column. `ui.max_rect()` inside a ScrollArea equals the
                    // viewport in CONTENT coordinates and drifts on scroll, so by it the cursor
                    // “had left the bin” already at the bottom edge of the screen: a card
                    // could not be placed below one screen, and the list
                    // never outgrew the viewport — so there was
                    // nothing to scroll, neither by the wheel nor in any other way
                    // (item 1 of round 16).
                    let view = ui.clip_rect();
                    // The wheel DURING a drag: egui mutes the scroll of a
                    // ScrollArea while something is being dragged (the gate
                    // `dragged_id().is_none()`), and the person turns the wheel
                    // precisely then — to reach the needed place
                    // (complaint “the mouse scroll still does not work”,
                    // item 1 of round 17). We read the raw events and scroll ourselves.
                    let wheel = raw_wheel_y(ui);
                    if wheel != 0.0 {
                        ui.scroll_with_delta_animation(
                            egui::vec2(0.0, wheel),
                            egui::style::ScrollAnimation::none(),
                        );
                    }
                    if !st.escaped {
                        // at the edges — we scroll the list rather than carry out the card;
                        // scroll_with_delta goes around egui's wheel gate
                        // (that one mutes the wheel for the duration of any drag)
                        let dy = bin_edge_scroll(view, pos.y);
                        if dy != 0.0 {
                            ui.scroll_with_delta_animation(
                                egui::vec2(0.0, dy),
                                egui::style::ScrollAnimation::none(),
                            );
                            // the cursor may stand still — frames must still run,
                            // otherwise the scroll freezes
                            ui.ctx().request_repaint();
                        }
                    }
                    // We count a “carry-out” only if the cursor has LEFT the band
                    // with a margin (item 1 of round 17): a tap at the edge means
                    // “scroll the list”, and the card must stay where it is being led,
                    // not fly back to its old place.
                    if st.escaped {
                        drag_out = Some(*id);
                    } else if view.expand2(BIN_ESCAPE_SLACK).contains(pos) {
                        let np = pos - st.grab - area.min.to_vec2();
                        let delta = egui::pos2(np.x.max(0.0), np.y.max(0.0)) - st.start;
                        for (m, from) in &st.mates {
                            let to = *from + delta;
                            self.bin_layout.insert(*m, egui::pos2(to.x.max(0.0), to.y.max(0.0)));
                        }
                    } else {
                        if let Some(st) = self.bin_drag.as_mut() {
                            st.escaped = true;
                        }
                        for (m, from) in &st.mates {
                            self.bin_layout.insert(*m, *from);
                        }
                        drag_out = Some(*id);
                    }
                }
                if resp.drag_stopped()
                    && let Some(st) = self.bin_drag.take()
                    && !st.escaped
                    && self.bin_layout.get(id).copied() != Some(st.start)
                {
                    self.dirty_ui = true; // the layout is a convenience, not content
                }
            }

            // selection frame: we draw and on release take the cards
            if let Some(anchor) = self.bin_rubber {
                let cur = ui.ctx().pointer_latest_pos().unwrap_or(anchor);
                let band = egui::Rect::from_two_pos(anchor, cur);
                ui.painter().rect_filled(
                    band,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(90, 140, 220, 40),
                );
                ui.painter().rect_stroke(
                    band,
                    0.0,
                    egui::Stroke::new(1.0, crate::theme::ACCENT),
                    egui::StrokeKind::Middle,
                );
                if !ui.ctx().input(|i| i.pointer.primary_down()) {
                    for (id, r) in &card_rects {
                        if band.intersects(*r) {
                            self.bin_sel.insert(*id);
                        }
                    }
                    self.bin_rubber = None;
                }
            }

            self.draw_bin_selection(ui, &card_rects);
            self.draw_bin_group_name_edit(ui);
            self.bin_card_rects = card_rects;
        } else {
            // --- grid ---------------------------------------------------
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (id, path) in &media {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(card_w, row_h), egui::Sense::hover());
                    let resp = self.draw_bin_card(
                        ui,
                        *id,
                        path,
                        rect,
                        card_w,
                        card_h,
                        &offline,
                        &reels,
                        &mut pending,
                        &mut to_place,
                        &mut to_measure,
                        &mut to_gen,
                        &mut to_kind,
                        &mut to_remove,
                    );
                    if resp.dragged() {
                        drag_out = Some(*id);
                    }
                }
            });
        }
        if media.is_empty() {
            ui.weak(tr(k::bin::EMPTY));
        }
        // carry-out of a card to the timeline (item 5 of round 8): from then on it is led by the usual
        // ghost silhouette of the timeline, as on a file drop from the system
        if let Some(id) = drag_out {
            egui::DragAndDrop::set_payload(ui.ctx(), BinDrag(id));
        }
        if pending {
            // the thumbnail loads — a soft retry, not 60 fps
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
        if let Some(id) = to_measure {
            self.measure_loudness(id);
        }
        if let Some(id) = to_gen {
            self.open_generator_editor(id);
        }
        if let Some((id, kd)) = to_kind {
            // the media path carries ONLY the VIEW, the event values are re-dressed by
            // the model itself (`fit_gen_to_media`) — one command
            self.apply(kt_model::Command::SetMediaPath { id, path: kt_model::gen_kind_path(&kd) });
        }
        if let Some(id) = to_place {
            self.place_media_at_cursor(id);
        }
        // A free strip at the bottom: a place where one can always drop a file and
        // always right-click. Without it the “bin background” on a
        // dense grid is physically absent — the cards have taken everything.
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::hover());
        self.bin_rect = ui.min_rect();
        if let Some(id) = to_remove
            && self.apply(Command::RemoveMedia { id })
        {
            // the layout and the groups lived separately from the media and drifted: we clean
            // them up together with the removal (item 13 of round 11)
            self.bin_layout.remove(&id);
            self.bin_sel.remove(&id);
            let alive: Vec<MediaId> = self.model.project().media.iter().map(|m| m.id).collect();
            kt_model::prune_bin_groups(&mut self.bin_groups, &alive);
            self.dirty_ui = true;
        }
    }

    /// Right click on the FREE BACKGROUND of the bin (R.49, item 365).
    ///
    /// The layout was switched only by two icons at the top of the panel — and
    /// a person used to the right button on the timeline got silence on the bin's
    /// background. The items are the same two actions as the icons:
    /// zero new keys.
    ///
    /// The interaction is registered FIRST, before the icons and cards: in egui
    /// a later-registered item lies HIGHER, and everything the person aims at
    /// takes the click for itself, while the background gets only what lies outside. The border is taken by
    /// `clip_rect`, not `max_rect`: inside `ScrollArea` the latter equals the
    /// viewport in CONTENT coordinates and drifts on scroll (item 118).
    fn bin_background_menu(&mut self, ui: &mut egui::Ui) {
        // The rectangle of the PREVIOUS frame: the bin learns its own height only
        // after drawing the cards, while the interaction must be registered BEFORE them.
        // On the first frame it does not exist yet — the menu appears from the second, and this is
        // unnoticeable (a person does not manage to click within the window's first frame).
        // `ui.clip_rect()` does not work here: inside `ScrollArea` it equals the
        // viewport of the COLUMN, and below the bin lives the performance panel — its right click
        // would go to the bin's menu.
        let view = self.bin_rect.intersect(ui.clip_rect());
        if !view.is_positive() {
            return;
        }
        let resp = ui.interact(view, ui.id().with("bin-bg"), egui::Sense::click());
        let mut free: Option<bool> = None;
        let mut picked = false;
        let mut new_gen: Option<kt_model::GeneratorKind> = None;
        let mut new_reel = false;
        resp.context_menu(|ui| {
            picked = true;
            if ui.button(tr(k::bin::HINT_FREE)).clicked() {
                free = Some(true);
                ui.close();
            }
            if ui.button(tr(k::bin::HINT_GRID)).clicked() {
                free = Some(false);
                ui.close();
            }
            ui.separator();
            // generators (R.52) — THE SAME BODY as in the main menu
            // (rule 17): there is no second list of kinds
            if let Some(kd) = crate::gen_ui::generator_menu(ui) {
                new_gen = Some(kd);
            }
            // AN EMPTY REEL (R.55, C6). Until this round `AddSequence`
            // was constructed only inside the collapse and unlink actions:
            // there was nothing to create a composition from scratch with.
            let r = ui.button(tr(k::bin::menu::NEW_REEL));
            crate::widgets::field_probe::note("bin.new_reel", r.rect);
            if r.clicked() {
                new_reel = true;
                ui.close();
            }
        });
        if picked {
            self.set_panel_target(crate::ctx_menu::PanelTarget::BinEmpty);
        }
        if let Some(kd) = new_gen {
            self.create_generator(kd);
        }
        if new_reel {
            self.create_empty_reel();
        }
        if let Some(v) = free {
            self.bin_free = v;
            self.dirty_ui = true;
        }
    }

    /// Group frames: the union of the members' rectangles, the name IN A BREAK
    /// of the top line (it takes no extra height), the lock as a glyph.
    /// Plus the highlight of selected cards.
    /// The highlight of selected cards is drawn AFTER them (a thin frame
    /// around them, visible), unlike the group frames (those go UNDER the cards).
    fn draw_bin_selection(&mut self, ui: &mut egui::Ui, cards: &[(MediaId, egui::Rect)]) {
        let painter = ui.painter().clone();
        for (id, r) in cards {
            if self.bin_sel.contains(id) {
                painter.rect_stroke(
                    r.expand(1.0),
                    2.0,
                    egui::Stroke::new(1.0, crate::theme::ACCENT),
                    egui::StrokeKind::Middle,
                );
            }
        }
    }

    /// Group frames: drawn BEFORE the cards, so they lie UNDER them — a line
    /// over the preview looked absurd (complaint of round 13).
    fn draw_bin_groups(&mut self, ui: &mut egui::Ui, cards: &[(MediaId, egui::Rect)]) {
        let painter = ui.painter().clone();
        let at = |id: MediaId| cards.iter().find(|(m, _)| *m == id).map(|(_, r)| *r);
        let mut rename: Option<usize> = None;
        for gi in 0..self.bin_groups.len() {
            let g = self.bin_groups[gi].clone();
            let mut frame: Option<egui::Rect> = None;
            for m in &g.members {
                if let Some(r) = at(*m) {
                    frame = Some(frame.map_or(r, |f: egui::Rect| f.union(r)));
                }
            }
            let Some(frame) = frame.map(|f| f.expand(4.0)) else { continue };
            let col = if g.locked {
                egui::Color32::from_rgb(150, 130, 90)
            } else {
                egui::Color32::from_rgb(90, 120, 165)
            };
            let stroke = egui::Stroke::new(1.0, col);
            // a frame without a top line — we'll draw it with a gap for the name
            for seg in [
                [frame.left_bottom(), frame.right_bottom()],
                [frame.left_top(), frame.left_bottom()],
                [frame.right_top(), frame.right_bottom()],
            ] {
                painter.line_segment(seg, stroke);
            }
            let title = if g.locked {
                format!("{} {}", crate::icons::LOCK, g.name)
            } else {
                g.name.clone()
            };
            let title = title.trim().to_string();
            if title.is_empty() {
                painter.line_segment([frame.left_top(), frame.right_top()], stroke);
            } else {
                let galley = painter.layout_no_wrap(title, egui::FontId::proportional(10.5), col);
                let w = galley.size().x.min(frame.width() - 12.0).max(0.0);
                let x0 = frame.left() + 6.0;
                painter.line_segment([frame.left_top(), egui::pos2(x0 - 3.0, frame.top())], stroke);
                painter.line_segment(
                    [egui::pos2(x0 + w + 3.0, frame.top()), frame.right_top()],
                    stroke,
                );
                painter.galley(egui::pos2(x0, frame.top() - galley.size().y * 0.5), galley, col);
            }
            // click on the FRAME (not on a card): select the group, RMB — menu
            let bar = egui::Rect::from_min_max(
                frame.left_top() - egui::vec2(0.0, 5.0),
                egui::pos2(frame.right(), frame.top() + 2.0),
            );
            let r = ui.interact(bar, ui.id().with(("bin-group", gi)), egui::Sense::click());
            let r = self.hint(r, tr(k::bin::HINT_GROUP));
            if r.clicked() {
                self.bin_sel = g.members.iter().copied().collect();
            }
            r.context_menu(|ui| {
                if ui.button(tr(k::bin::menu::RENAME)).clicked() {
                    rename = Some(gi);
                    ui.close();
                }
                let lock_label =
                    if g.locked { tr(k::bin::menu::UNLOCK) } else { tr(k::bin::menu::LOCK) };
                if ui.button(lock_label).clicked() {
                    self.bin_groups[gi].locked = !g.locked;
                    self.dirty_ui = true;
                    ui.close();
                }
                if ui.button(tr(k::bin::menu::UNGROUP)).clicked() {
                    self.bin_ungroup(gi);
                    ui.close();
                }
            });
        }
        if let Some(gi) = rename {
            let name = self.bin_groups[gi].name.clone();
            self.bin_group_edit = Some((gi, name));
            self.bin_group_focus = true;
        }
    }

    /// Inline editing of the group name (on top of everything, like other inline fields).
    fn draw_bin_group_name_edit(&mut self, ui: &mut egui::Ui) {
        if let Some((gi, buf)) = self.bin_group_edit.clone() {
            let mut buf = buf;
            let mut done: Option<bool> = None;
            egui::Area::new(egui::Id::new("bin-group-name"))
                .order(egui::Order::Foreground)
                .fixed_pos(ui.max_rect().left_top() + egui::vec2(8.0, 8.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut buf)
                                .hint_text(tr(k::bin::GROUP_NAME))
                                .desired_width(150.0),
                        );
                        // we request focus EXACTLY once: otherwise the field
                        // never “loses” it, lost_focus does not happen and
                        // Enter does not save the name (complaint of round 13)
                        if !r.has_focus() && self.bin_group_focus {
                            r.request_focus();
                            self.bin_group_focus = false;
                        }
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            done = Some(true);
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            done = Some(false);
                        }
                    });
                });
            match done {
                Some(true) => {
                    if gi < self.bin_groups.len() {
                        self.bin_groups[gi].name = buf.trim().to_string();
                        self.dirty_ui = true;
                    }
                    self.bin_group_edit = None;
                }
                Some(false) => self.bin_group_edit = None,
                None => self.bin_group_edit = Some((gi, buf)),
            }
        }
    }

    /// One bin card: a preview with scrub + duration and name.
    #[allow(clippy::too_many_arguments)]
    fn draw_bin_card(
        &mut self,
        ui: &mut egui::Ui,
        id: MediaId,
        path: &str,
        rect: egui::Rect,
        card_w: f32,
        card_h: f32,
        offline: &std::collections::HashSet<MediaId>,
        // REEL addresses: they have neither a file, nor thumbnails, nor volume, and
        // their verbs are their own (R.55, C6).
        reels: &std::collections::HashSet<MediaId>,
        pending: &mut bool,
        to_place: &mut Option<MediaId>,
        to_measure: &mut Option<MediaId>,
        to_gen: &mut Option<MediaId>,
        to_kind: &mut Option<(MediaId, kt_model::GeneratorKind)>,
        to_remove: &mut Option<MediaId>,
    ) -> egui::Response {
        let is_reel = reels.contains(&id);
        let gen_kind = if is_reel { None } else { kt_model::parse_gen_path(path) };
        // the generator caption — the name of the kind, not the raw gen:-path (rule 7);
        // a reel's caption is its own name, and it has no path
        let name = if is_reel {
            path.to_string()
        } else {
            match &gen_kind {
                Some(kd) => crate::gen_ui::kind_name(kd).to_string(),
                None => std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string()),
            }
        };
        let is_synth = path.starts_with("synth:");
        let is_off = offline.contains(&id);
        let thumb_rect = egui::Rect::from_min_size(rect.min, egui::vec2(card_w, card_h));
        if !self.bin_free {
            // in the grid no one else computes the rectangles, yet the card has to be probed by mouse
            // both by the test and by the live scenario (R.53, item 416)
            self.bin_card_rects.push((id, thumb_rect));
        }
        if gen_kind.is_some() {
            // the generator card — exposed for live scenarios (R.52)
            crate::widgets::field_probe::note("bin.gen", thumb_rect);
        } else if !is_reel && !is_synth {
            // the FILE card — the live conform scenario (R.62) needs placement
            // through the bin, to see the mismatch hint
            crate::widgets::field_probe::note("bin.file", thumb_rect);
        }
        let resp = ui.interact(
            thumb_rect,
            ui.id().with(("bin-card", id.0)),
            egui::Sense::click_and_drag(),
        );
        let p = ui.painter_at(rect);
        p.rect_filled(thumb_rect, 2.0, egui::Color32::from_rgb(10, 11, 13));
        // audio media: instead of a black square — a mini-waveform from the already
        // built peak pyramid (item 5 of round 9)
        if is_reel {
            // MINI-MAP instead of a thumbnail: a reel never has thumbnails PER FILE — it has
            // no file (invariant 8, item 395). The frames for a reel are computed by
            // `reel_preview` (R.56, item 452), and a bin card can take them
            // by the same key — this is deferred work, not a prohibition.
            crate::widgets::field_probe::note("bin.reel", thumb_rect);
            if let Some(seq) = self.model.document().sequence(id) {
                let end = seq.end().0.max(1);
                let lanes = seq.lanes(kt_model::LaneKind::Video).max(1)
                    + seq.lanes(kt_model::LaneKind::Audio).max(1);
                let bar_h = (thumb_rect.height() - 4.0) / lanes as f32;
                let mut mesh = egui::Mesh::default();
                for ch in &seq.clips {
                    let row = match ch.kind {
                        kt_model::ClipKind::Video => ch.lane,
                        kt_model::ClipKind::Audio => {
                            seq.lanes(kt_model::LaneKind::Video).max(1) + ch.lane
                        }
                    };
                    let y = thumb_rect.top() + 2.0 + row as f32 * bar_h;
                    let x0 =
                        thumb_rect.left() + thumb_rect.width() * ch.start.0 as f32 / end as f32;
                    let x1 = thumb_rect.left()
                        + thumb_rect.width() * (ch.start + ch.duration).0 as f32 / end as f32;
                    mesh.add_colored_rect(
                        egui::Rect::from_min_max(
                            egui::pos2(x0, y),
                            egui::pos2(x1.max(x0 + 1.0), y + (bar_h - 1.0).max(1.0)),
                        ),
                        crate::theme::BAND_EDGE.gamma_multiply(0.7),
                    );
                }
                p.add(egui::Shape::mesh(mesh));
                // ×N BADGE — only when there are SEVERAL places (the same rule
                // as on an event, t458): “×1” would be noise
                let uses = self.model.document().sequence_uses(id);
                if uses > 1 {
                    p.text(
                        thumb_rect.right_top() + egui::vec2(-4.0, 2.0),
                        egui::Align2::RIGHT_TOP,
                        format!("×{uses}"),
                        egui::FontId::proportional(11.0),
                        crate::theme::BAND_EDGE,
                    );
                }
                // HOW MANY HANDLES ARE DECLARED (R.59, stage 3) — a pin with
                // a number in a free corner of the card: a template is told apart from an
                // ordinary composition at a glance, and no strip is created under
                // it (rule 13). Zero is not written — “×1” is noise of the same
                // kind.
                let pubs = seq.published.len();
                if pubs > 0 {
                    p.text(
                        thumb_rect.left_bottom() + egui::vec2(4.0, -2.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{}{pubs}", crate::icons::PIN),
                        egui::FontId::proportional(11.0),
                        crate::theme::BAND_EDGE,
                    );
                }
            }
        }
        let kinds = if is_reel { None } else { self.thumbs.kinds(id) };
        let audio_only = kinds == Some((false, true));
        // A KIND WITH DEFAULTS gives an empty frame (R.59, stage 2): the values
        // moved to the event, and for a title the default is an empty string.
        // A black rectangle here would be silence about there being nothing to show —
        // we write the KIND'S NAME (rule 16).
        let blank_gen = gen_kind.as_ref().is_some_and(crate::gen_ui::blank_default);
        if is_reel {
            // everything is already drawn above
        } else if blank_gen {
            crate::widgets::field_probe::note("bin.gen.name", thumb_rect);
            p.text(
                thumb_rect.center(),
                egui::Align2::CENTER_CENTER,
                &name,
                egui::FontId::proportional(12.0),
                ui.visuals().weak_text_color(),
            );
        } else if audio_only {
            self.draw_bin_waveform(&p, thumb_rect, id);
        } else if !is_synth && !is_off {
            let dur = self.thumbs.duration_or_probe(id);
            // moment: hover-x → fraction of the file; without hover — the poster
            let (t, tol) = match (resp.hover_pos(), dur) {
                (Some(pos), Some(d)) if d.0 > 0 => {
                    let frac = ((pos.x - thumb_rect.left()) / thumb_rect.width()).clamp(0.0, 0.999);
                    let step = thumb_source_step(d, BIN_SCRUB_SLOTS * card_w, card_w);
                    (thumb_key_t(0, (frac as f64 * d.0 as f64) as i64, step), step / 2)
                }
                // poster: the most informative frame, not the first (item 4 of round 9)
                _ => (self.thumbs.poster_t_or_request(id).unwrap_or(0), THUMB_STEP_LADDER[0] / 2),
            };
            let key = ThumbKey { media: id, t };
            let tex = self.thumb_texture(ui, key);
            let shown = match &tex {
                Some(t) => Some(t.clone()),
                None => {
                    // Failed is NOT re-ordered and we do NOT keep a repaint loop
                    if self.thumbs.state(&key) != ThumbState::Failed {
                        self.thumbs.request_near(key, tol);
                        *pending = true;
                    }
                    let last = self.bin_last_good.get(&id).copied();
                    last.and_then(|k| self.thumb_texture(ui, k))
                }
            };
            if tex.is_some() {
                self.bin_last_good.insert(id, key);
            }
            if let Some(t) = shown {
                p.image(
                    t.id(),
                    thumb_rect.shrink(1.0),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            if let (Some(pos), Some(d)) = (resp.hover_pos(), dur)
                && d.0 > 0
            {
                p.line_segment(
                    [
                        egui::pos2(pos.x, thumb_rect.top() + 1.0),
                        egui::pos2(pos.x, thumb_rect.bottom() - 1.0),
                    ],
                    egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 170),
                    ),
                );
            }
        } else {
            p.text(
                thumb_rect.center(),
                egui::Align2::CENTER_CENTER,
                if is_off { crate::icons::WARN } else { "♪" },
                egui::FontId::proportional(18.0),
                egui::Color32::from_gray(120),
            );
        }
        // Ctrl/Shift+click — builds the selection set, a plain click — as before
        // (place at the cursor), while also making the card the only selected one:
        // otherwise “selected with a frame, clicked elsewhere — and the selection
        // remained” (item 13 of round 11)
        // The second click of a double-click does NOT place a second clip (R.53, item 416): in
        // egui `clicked()` is true for both, and a double-click on a card
        // gave TWO overlapping events. Now it gives one clip (from the
        // first click) and opens the parameters window — the same verb
        // “open” as the menu item and as the double-click on an event
        // (rule 17).
        if resp.clicked() && !resp.double_clicked() {
            let add = ui.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
            if add {
                if !self.bin_sel.remove(&id) {
                    self.bin_sel.insert(id);
                }
            } else {
                self.bin_sel.clear();
                self.bin_sel.insert(id);
                *to_place = Some(id);
            }
        }
        if resp.double_clicked() && gen_kind.is_some() {
            *to_gen = Some(id);
        }
        let sel_n = self.bin_sel.len();
        let group_of = self.bin_group_of(id);
        let mut group_now = false;
        let mut ungroup: Option<usize> = None;
        let mut lock_toggle: Option<usize> = None;
        let mut reel_verb: Option<ReelVerb> = None;
        resp.clone().context_menu(|ui| {
            let r = ui.button(tr(k::bin::menu::PLACE));
            crate::widgets::field_probe::note("bin.place", r.rect);
            if r.clicked() {
                *to_place = Some(id);
                ui.close();
            }
            // REEL VERBS (R.55, C6). Previously there were none at all: a reel
            // was not shown in the bin whatsoever, and “can be created, cannot be controlled”
            // applied to it entirely.
            if is_reel {
                for (key, verb) in [
                    (k::tl::menu::NEST_EXPAND_HERE, ReelVerb::Expand),
                    (k::tl::menu::NEST_ENTER, ReelVerb::Open),
                    (k::tl::menu::NEST_RENAME, ReelVerb::Rename),
                    // the author's list of handles — THE SAME BODY as the menu item
                    // of the instance (rule 17)
                    (k::pubp::LIST_OPEN, ReelVerb::Published),
                ] {
                    let r = ui.button(tr(key));
                    crate::widgets::field_probe::note(
                        match verb {
                            ReelVerb::Expand => "bin.reel.expand",
                            ReelVerb::Open => "bin.reel.open",
                            ReelVerb::Rename => "bin.reel.rename",
                            ReelVerb::Published => "bin.reel.published",
                            ReelVerb::Remove => "bin.reel.remove",
                        },
                        r.rect,
                    );
                    if r.clicked() {
                        reel_verb = Some(verb);
                        ui.close();
                    }
                }
                ui.separator();
                let r = ui.button(tr(k::bin::menu::REMOVE));
                crate::widgets::field_probe::note("bin.reel.remove", r.rect);
                if r.clicked() {
                    reel_verb = Some(ReelVerb::Remove);
                    ui.close();
                }
                return;
            }
            if gen_kind.is_some() {
                // for a generator, instead of a loudness measurement (there is nothing to measure:
                // there is no audio) — the parameter form of its event; the same
                // item as in the event menu on the timeline (rule 17)
                if ui.button(tr(k::tl::menu::GEN_PARAMS)).clicked() {
                    *to_gen = Some(id);
                    ui.close();
                }
                // KIND is a property of the media, and it can be changed even when
                // events are not yet on the edit (the media has no more values,
                // but it has a kind). The list of kinds is ONE for all entries —
                // the same body as the creation (rule 17).
                if let Some(kd) = crate::gen_ui::generator_kind_menu(ui, tr(k::genw::CHANGE_KIND)) {
                    *to_kind = Some((id, kd));
                    ui.close();
                }
            } else if ui.button(tr(k::bin::menu::MEASURE_LOUDNESS)).clicked() {
                *to_measure = Some(id);
                ui.close();
            }
            ui.separator();
            match group_of {
                Some(gi) => {
                    if ui.button(tr(k::bin::menu::UNGROUP)).clicked() {
                        ungroup = Some(gi);
                        ui.close();
                    }
                    if ui.button(tr(k::bin::menu::LOCK_TOGGLE)).clicked() {
                        lock_toggle = Some(gi);
                        ui.close();
                    }
                }
                None => {
                    let label = if sel_n > 1 {
                        crate::tf!(k::bin::menu::GROUP_SELECTED, sel_n)
                    } else {
                        tr(k::bin::menu::GROUP_ONE).to_string()
                    };
                    if ui.button(label).clicked() {
                        group_now = true;
                        ui.close();
                    }
                }
            }
            ui.separator();
            if ui.button(tr(k::bin::menu::REMOVE)).clicked() {
                *to_remove = Some(id);
                ui.close();
            }
        });
        if let Some(v) = reel_verb {
            self.reel_verb_from_bin(id, v);
        }
        if group_now {
            if self.bin_sel.is_empty() {
                self.bin_sel.insert(id);
            }
            if let Some(gi) = self.bin_group_selection(String::new()) {
                self.bin_group_edit = Some((gi, String::new()));
                self.bin_group_focus = true;
            }
        }
        if let Some(gi) = ungroup {
            self.bin_ungroup(gi);
        }
        if let Some(gi) = lock_toggle
            && gi < self.bin_groups.len()
        {
            self.bin_groups[gi].locked = !self.bin_groups[gi].locked;
            self.dirty_ui = true;
        }
        // caption: duration (mono) + name, under the preview
        let label_y = thumb_rect.bottom() + 2.0;
        // a generator has NO duration (duration == 0) — write neither
        // “0:00:00” nor “1000:00:00” (minus F-TXT-003: the bin must be able to handle
        // the case of “no length”)
        let dur_txt = self.thumbs.duration(id).filter(|d| d.0 > 0).map(|d| {
            let s = d.0 / 90_000;
            format!("{}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
        });
        // “×N” — how many TIMES the media is placed on the edit (R.53, item 416). A generator's
        // parameters are shared across all its places, and editing the colour changes the
        // picture everywhere at once: the number must be visible BEFORE editing, not only
        // in the window. The place is not new — it is the empty duration slot
        // (rule 13), therefore it is shown only where that slot is empty.
        let uses_txt = (dur_txt.is_none())
            .then(|| self.media_uses().get(&id).copied().unwrap_or(0))
            .filter(|n| *n > 0)
            .map(|n| format!("×{n}"));
        let mut x = rect.left();
        if let Some(d) = dur_txt.or(uses_txt) {
            let g =
                p.layout_no_wrap(d, egui::FontId::monospace(10.0), egui::Color32::from_gray(120));
            p.galley(egui::pos2(x, label_y), g.clone(), egui::Color32::from_gray(120));
            x += g.size().x + 4.0;
        }
        let label = if is_off { format!("{} {name}", crate::icons::WARN) } else { name };
        let avail = (rect.right() - x).max(10.0);
        let g =
            p.layout(label, egui::FontId::proportional(11.0), egui::Color32::from_gray(190), avail);
        p.galley(egui::pos2(x, label_y), g, egui::Color32::from_gray(190));
        // The material's loudness — IN THE HINT, without a new strip in the card
        // (R.49, item 373). The display STARTS NOTHING: hovering the mouse over the bin
        // would mean starting decode of dozens of files — a direct relative of invariant 8
        // (“scroll/zoom does not generate decode requests”). Until it is measured, it is said so:
        // “not measured yet”, rather than an invented zero.
        let loud = match self.loudness.lock().unwrap().get(&id) {
            Some((lufs, dbtp, _)) => {
                crate::tf!(k::bin::LOUDNESS, format!("{lufs:.1}"), format!("{dbtp:.1}"))
            }
            None => tr(k::bin::LOUDNESS_PENDING).to_string(),
        };
        self.hint(resp.clone(), &format!("{path}\n{loud}"));
        resp
    }

    /// The mini-waveform of audio media in the card (item 5 of round 9): one mesh from the
    /// already built peak pyramid — an overview of the whole file. If the whole file is below
    /// the silence threshold, we draw a flat line and a mark.
    /// Execute the reel verb selected in the bin (R.55, C6).
    ///
    /// Each verb is THE SAME BODY as the event's menu item (rule 17):
    /// a second way to expand, enter, rename or remove a reel
    /// must not be created, otherwise they would diverge.
    fn reel_verb_from_bin(&mut self, seq: MediaId, verb: ReelVerb) {
        // “expand” and “enter” act ON THE INSTANCE: expanding is a view around a specific place,
        // not a property of the reel. If there is not a single place — say so, rather than
        // stay silent.
        let inst = self.model.project().clips.iter().find(|c| c.media == seq).map(|c| c.id);
        match verb {
            ReelVerb::Expand | ReelVerb::Open => {
                let Some(inst) = inst else {
                    self.warn(tr(k::msg::NEST_NOT_PLACED));
                    return;
                };
                if verb == ReelVerb::Expand {
                    self.expand_in_place(inst);
                } else {
                    self.enter_nest(inst);
                }
            }
            ReelVerb::Rename => {
                let name =
                    self.model.document().sequence(seq).map(|s| s.name.clone()).unwrap_or_default();
                self.seq_name = Some(crate::app::SeqNameDialog { seq, name, focus: true });
            }
            ReelVerb::Published => self.open_published_win(seq),
            ReelVerb::Remove => {
                // ADDRESSED removal: previously there was only the “remove unused” button, that is,
                // all at once. An occupied reel refuses to go away by itself (t429) — a refusal with
                // a reason, not
                // silence.
                if self.apply(Command::RemoveSequence { id: seq }) {
                    self.note(tr(k::msg::NEST_REMOVED));
                }
            }
        }
    }

    fn draw_bin_waveform(&self, p: &egui::Painter, rect: egui::Rect, id: MediaId) {
        let peaks = self.peaks.lock().unwrap();
        let Some(levels) = peaks.get(&id) else {
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "♪",
                egui::FontId::proportional(18.0),
                egui::Color32::from_gray(120),
            );
            return;
        };
        let cols = rect.width().max(1.0) as usize;
        // pyramid level for the card width: fewer blocks — coarser
        let li = levels
            .iter()
            .position(|(l, _)| l.len() <= cols.max(1))
            .unwrap_or(levels.len().saturating_sub(1));
        let Some((lch, rch)) = levels.get(li) else {
            return;
        };
        let n = lch.len().max(1);
        let mid = rect.center().y;
        let half = rect.height() * 0.45;
        let mut mesh = egui::Mesh::default();
        let mut loud = false;
        for c in 0..cols {
            let i = c * n / cols.max(1);
            let (lo, hi) = lch.get(i).copied().unwrap_or((0, 0));
            let (rlo, rhi) = rch.get(i).copied().unwrap_or((0, 0));
            let amp_l = lo.unsigned_abs().max(hi.unsigned_abs()) as f32 / 32767.0;
            let amp_r = rlo.unsigned_abs().max(rhi.unsigned_abs()) as f32 / 32767.0;
            if amp_l.max(amp_r) > 0.02 {
                loud = true;
            }
            let x = rect.left() + c as f32;
            for (amp, dir) in [(amp_l, -1.0f32), (amp_r, 1.0)] {
                let hgt = (amp * half).max(0.5);
                mesh.add_colored_rect(
                    egui::Rect::from_min_max(
                        egui::pos2(x, mid.min(mid + dir * hgt)),
                        egui::pos2(x + 1.0, mid.max(mid + dir * hgt)),
                    ),
                    egui::Color32::from_rgb(96, 168, 220),
                );
            }
        }
        p.add(egui::Shape::mesh(mesh));
        if !loud {
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "♪",
                egui::FontId::proportional(16.0),
                egui::Color32::from_gray(120),
            );
        }
    }

    /// The thumbnail as an egui texture from the application's SHARED cache (the same path
    /// as the timeline filmstrip): get → load_texture → LRU.
    fn thumb_texture(&mut self, ui: &egui::Ui, key: ThumbKey) -> Option<egui::TextureHandle> {
        if let Some(t) = self.thumb_textures.get(&key) {
            return Some(t.clone());
        }
        let img = self.thumbs.get(&key)?;
        let tex = ui.ctx().load_texture(
            format!("thumb-{}-{}", key.media.0, key.t),
            egui::ImageData::from((*img).clone()),
            egui::TextureOptions::LINEAR,
        );
        self.thumb_textures.insert(key, tex.clone());
        self.thumb_lru.retain(|k| k != &key);
        self.thumb_lru.push_back(key);
        while self.thumb_lru.len() > crate::thumbs::MAX_READY {
            if let Some(old) = self.thumb_lru.pop_front() {
                self.thumb_textures.remove(&old);
            }
        }
        Some(tex)
    }
}
