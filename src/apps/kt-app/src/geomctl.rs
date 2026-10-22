//! On-screen geometry controls over the preview frame (R.64).
//!
//! The owner (R.63 item 6): “the fx geometry is still very poor, solid dull input fields… AT A MINIMUM we should have
//! editing right in a convenient preview”. A clarification of the same round — the controls are IN ADDITION to the fields,
//! not instead of them.
//!
//! Three rules on which everything here holds:
//!
//! 1. **The placement arithmetic is ONE** — `kt_fx::geom::geometry_placement`
//!    and its inverse pair. The frame is drawn with the
//!    same rectangle by which the composite places the layer; a
//!    gesture returns values by the same functions by which the render
//!    reads them. There is not a single second formula here.
//! 2. **The gesture goes through the EXISTING command** `SetGeometry`
//!    via `apply_coalesced` with the token issued on the press:
//!    the live picture and one history step per gesture come by themselves (the same trick as the title drag in the preview
//!    and the level line on the timeline).
//! 3. **Exactly ONE set is active** (transform OR crop) — the
//!    Resolve rule: two sets over one frame fight for the same mouse.

use crate::app::KtApp;
use eframe::egui;
use kt_fx::conform::LeadCtx;
use kt_fx::geom::{
    Handle, Placement, anchor_move, crop_for_handle, geometry_placement, pos_for_drag,
    rot_for_point, zoom_for_handle,
};
use kt_model::{ClipId, ClipKind, Command, EventGeometry, FxInstance, HasParams};

/// The grab zone of a handle in screen px (rule 1 of the checklist:
/// one should not need to aim at a caption the height of a line).
pub const GRAB: f32 = 14.0;
/// The grab zone of the ANCHOR without a modifier: it stands in the centre of the frame, and a full zone
/// would take the picture’s move for itself.
pub const ANCHOR_GRAB: f32 = 8.0;
/// The drawn radius of the handle.
const DOT: f32 = 4.0;

/// The set of controls. Exactly one is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeomMode {
    Off,
    Transform,
    Crop,
}

impl GeomMode {
    pub fn from_u32(v: u32) -> GeomMode {
        match v {
            1 => GeomMode::Transform,
            2 => GeomMode::Crop,
            _ => GeomMode::Off,
        }
    }
    pub fn as_u32(self) -> u32 {
        match self {
            GeomMode::Off => 0,
            GeomMode::Transform => 1,
            GeomMode::Crop => 2,
        }
    }
    /// The next set in a cycle — one switch for all states.
    pub fn next(self) -> GeomMode {
        match self {
            GeomMode::Off => GeomMode::Transform,
            GeomMode::Transform => GeomMode::Crop,
            GeomMode::Crop => GeomMode::Off,
        }
    }
    pub fn handles(self) -> &'static [Handle] {
        match self {
            GeomMode::Off => &[],
            GeomMode::Transform => &[
                Handle::TopLeft,
                Handle::Top,
                Handle::TopRight,
                Handle::Right,
                Handle::BottomRight,
                Handle::Bottom,
                Handle::BottomLeft,
                Handle::Left,
                Handle::Rot,
                Handle::Anchor,
            ],
            GeomMode::Crop => {
                &[Handle::CropLeft, Handle::CropTop, Handle::CropRight, Handle::CropBottom]
            }
        }
    }
}

/// The gesture in progress: what is pulled, from where and with which history token.
///
/// The gesture is ABSOLUTE: it is computed from the press point and the geometry AT THE MOMENT of the press,
/// not from the per-frame delta. The incremental variant
/// applied twice the delta of the frame in which the drag started
/// (caught by t760), and in addition would accumulate an error on every frame.
#[derive(Debug, Clone, Copy)]
pub struct GeomDrag {
    pub clip: ClipId,
    pub handle: Option<Handle>,
    pub gesture: u64,
    /// The geometry at the moment of the press — the WHOLE gesture is computed from it.
    pub start: EventGeometry,
    /// The press point in the CANVAS coordinates.
    pub press: (f64, f64),
}

/// Why there is no frame — the reason is named in a word (rule 16).
pub enum NoFrame {
    NothingSelected,
    ManySelected,
    NotVideo,
    NotUnderCursor,
}

impl NoFrame {
    pub fn key(&self) -> &'static str {
        use crate::i18n::{k, tr};
        match self {
            NoFrame::NothingSelected => tr(k::geomctl::NO_SELECTION),
            NoFrame::ManySelected => tr(k::geomctl::MANY_SELECTED),
            NoFrame::NotVideo => tr(k::geomctl::NOT_VIDEO),
            NoFrame::NotUnderCursor => tr(k::geomctl::NOT_UNDER_CURSOR),
        }
    }
}

impl KtApp {
    pub(crate) fn geom_mode(&self) -> GeomMode {
        self.geom_ctl
    }

    /// The default for showing the frame: ON (the “Transform” mode), until the person switches it off himself (R.67).
    /// The owner: “instead of control in the preview we have only poor
    /// control inside the fx widget” — the frame started switched off,
    /// was not remembered, and was switched on only by a right-click
    /// menu item, that is, for the person it did not exist.
    pub(crate) fn default_geom_mode(on: Option<bool>) -> GeomMode {
        if on.unwrap_or(true) { GeomMode::Transform } else { GeomMode::Off }
    }

    /// Cycle the set. Convenience, not content: `dirty_ui`.
    ///
    /// The display state is remembered by the workspace (R.67): the person
    /// switched the frame off — it does not come back on the next launch.
    pub(crate) fn cycle_geom_mode(&mut self) {
        self.geom_ctl = self.geom_ctl.next();
        self.workspace.geom_controls = Some(self.geom_ctl != GeomMode::Off);
        self.dirty_ui = true;
        self.note(crate::tf!(crate::i18n::k::geomctl::TOGGLE, mode_name(self.geom_ctl)));
    }

    /// SWITCH to show the frame outline, placed in the corner of the frame itself (R.67).
    ///
    /// Previously the outline was toggled only by an item in the preview's context menu:
    /// the owner did not find it (“instead of controls in the preview, we have only
    /// meager controls inside the fx widget”). The button is an icon, not a checkbox (rule 9),
    /// and is drawn ONLY while paused: during playback there are no controls, and a button
    /// over the frame would be the only thing that blinks.
    pub(crate) fn geom_toggle_button(&mut self, ui: &mut egui::Ui, img: egui::Rect) {
        if self.player.transport.is_playing() || img.width() < 60.0 {
            return;
        }
        let size = egui::vec2(22.0, 22.0);
        let rect = egui::Rect::from_min_size(
            egui::pos2(img.right() - size.x - 6.0, img.top() + 6.0),
            size,
        );
        let on = self.geom_mode() != GeomMode::Off;
        let text = egui::RichText::new(crate::icons::SELECTION)
            .color(if on { crate::theme::ACCENT } else { egui::Color32::from_gray(150) });
        let r = ui.put(rect, egui::Button::new(text).frame(on));
        crate::widgets::field_probe::note("preview.geom_toggle", r.rect);
        let hint = if on {
            crate::i18n::tr(crate::i18n::k::geomctl::TOGGLE_OFF_HINT)
        } else {
            crate::i18n::tr(crate::i18n::k::geomctl::TOGGLE_ON_HINT)
        };
        // ACCESSIBLE NAME — in words, not a glyph (the same trick as the R.59 reset buttons):
        // otherwise the button is indistinguishable from the selection tool, which has
        // the same icon, neither for a screen reader nor for the harness.
        {
            let name = hint.to_string();
            r.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.clone())
            });
        }
        crate::widgets::hint(&r, hint);
        if r.clicked() {
            // Turning off goes to Off, turning on goes to “Transform” (the useful
            // set), not cycling: the button is a display SWITCH.
            self.geom_ctl = if on { GeomMode::Off } else { GeomMode::Transform };
            self.workspace.geom_controls = Some(!on);
            self.dirty_ui = true;
            self.note(crate::tf!(crate::i18n::k::geomctl::TOGGLE, mode_name(self.geom_ctl)));
        }
    }

    /// The event whose outline is drawn, or the reason why it is not.
    pub(crate) fn geom_target(&self) -> Result<ClipId, NoFrame> {
        let sel: Vec<ClipId> = self.selected.iter().copied().collect();
        if sel.is_empty() {
            return Err(NoFrame::NothingSelected);
        }
        if sel.len() > 1 {
            return Err(NoFrame::ManySelected);
        }
        let id = sel[0];
        let Some(c) = self.model.clip(id) else { return Err(NoFrame::NothingSelected) };
        if c.kind != ClipKind::Video {
            return Err(NoFrame::NotVideo);
        }
        let t = self.player.transport.playhead();
        if t < c.start || t >= c.start + c.duration {
            return Err(NoFrame::NotUnderCursor);
        }
        Ok(id)
    }

    /// Placement of the leading node of the event on the preview canvas.
    pub(crate) fn geom_placement(&self, clip: ClipId, canvas: (f64, f64)) -> Option<Placement> {
        let g = self.geom_effective(clip)?;
        self.geom_placement_of(&g, clip, canvas)
    }

    /// Cursor position in the event's CURVE DOMAIN — a MATERIAL tick (R.37: the axis
    /// is linear, speed does not compress it). The same rule governs the FX window field.
    pub(crate) fn geom_curve_at(&self, clip: ClipId) -> kt_time::Tick {
        let frame_step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        let playhead = self.player.transport.playhead();
        match self.model.clip(clip) {
            Some(c) => kt_model::clip_material_at(
                c.source_in,
                c.start,
                c.duration,
                c.reversed,
                frame_step,
                playhead,
            ),
            None => playhead,
        }
    }

    /// Geometry THAT IS VISIBLE: the base over which the values of the live
    /// curves under the cursor lie. The outline must show the same thing as the render, and
    /// the render samples `geometry_auto` as the automation of a regular slot.
    pub(crate) fn geom_effective(&self, clip: ClipId) -> Option<EventGeometry> {
        let c = self.model.clip(clip)?;
        let mut g = c.geometry;
        if c.geometry_auto.is_empty() {
            return Some(g);
        }
        let at = self.geom_curve_at(clip);
        let mut curves = c.geometry_auto.clone();
        let bind = crate::widgets::CurveBind { curves: &mut curves, sel: None, at };
        let names: Vec<String> = g.param_specs().iter().map(|s| s.name.to_string()).collect();
        for n in names {
            let v = bind.shown(&g, &n);
            g.set_param(&n, v);
        }
        Some(g)
    }

    /// The same for an ARBITRARY event geometry (the gesture computes from the snapshot
    /// taken at the moment of press).
    pub(crate) fn geom_placement_of(
        &self,
        g_of: &EventGeometry,
        clip: ClipId,
        canvas: (f64, f64),
    ) -> Option<Placement> {
        let c = self.model.clip(clip)?;
        let rule = g_of.rule.resolve(self.model.project().settings.conform);
        let view = self.player.source_view(c.media);
        let lead = LeadCtx::new(view, rule);
        // input of the node — the source view (the leading node stands first in the chain);
        // unknown — the canvas, and the rule degenerates into “stretch”
        let in_size = view.unwrap_or(canvas);
        geometry_placement(&g_of.node(), Some(lead), in_size, canvas)
    }

    /// Controls over the frame: drawing and gestures. Called from `draw_preview`
    /// BEFORE the title drag — the smaller and more specific target has priority.
    pub(crate) fn geometry_controls(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        img: egui::Rect,
        canvas: (u32, u32),
    ) {
        let mode = self.geom_mode();
        if mode == GeomMode::Off {
            return;
        }
        // during PLAYBACK there are no controls: they cover what the person
        // is watching the frame for (industry rule — FCP always hides them)
        if self.player.transport.is_playing() {
            return;
        }
        let canvas_f = (canvas.0.max(2) as f64, canvas.1.max(2) as f64);
        let clip = match self.geom_target() {
            Ok(id) => id,
            Err(why) => {
                // reason — only when the person is looking at the frame
                if ui.rect_contains_pointer(img) {
                    crate::widgets::hint_sink::put(why.key());
                }
                return;
            }
        };
        let Some(pl) = self.geom_placement(clip, canvas_f) else { return };
        let to_screen = |p: (f64, f64)| -> egui::Pos2 {
            egui::pos2(
                img.left() + (p.0 / canvas_f.0) as f32 * img.width(),
                img.top() + (p.1 / canvas_f.1) as f32 * img.height(),
            )
        };
        let to_canvas = |p: egui::Pos2| -> (f64, f64) {
            (
                ((p.x - img.left()) / img.width().max(1.0)) as f64 * canvas_f.0,
                ((p.y - img.top()) / img.height().max(1.0)) as f64 * canvas_f.1,
            )
        };

        // ---- DRAWING ---------------------------------------------------
        let accent = crate::theme::ACCENT;
        let cs = pl.corners().map(to_screen);
        painter.add(egui::Shape::closed_line(
            cs.to_vec(),
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(120, 190, 255, 200)),
        ));
        if mode == GeomMode::Transform {
            // shoulder of the rotation handle — a line from the top side
            let top = handle_screen(&pl, Handle::Top, &to_screen);
            let rot = handle_screen(&pl, Handle::Rot, &to_screen);
            painter.line_segment(
                [top, rot],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(120, 190, 255, 140)),
            );
        }
        for h in mode.handles() {
            let p = handle_screen(&pl, *h, &to_screen);
            match h {
                Handle::Anchor => {
                    painter.circle_stroke(p, DOT + 2.0, egui::Stroke::new(1.5, accent));
                    painter.line_segment(
                        [p - egui::vec2(DOT + 4.0, 0.0), p + egui::vec2(DOT + 4.0, 0.0)],
                        egui::Stroke::new(1.0, accent),
                    );
                    painter.line_segment(
                        [p - egui::vec2(0.0, DOT + 4.0), p + egui::vec2(0.0, DOT + 4.0)],
                        egui::Stroke::new(1.0, accent),
                    );
                }
                Handle::Rot => {
                    painter.circle_stroke(p, DOT, egui::Stroke::new(1.5, accent));
                }
                _ => {
                    painter.rect_filled(
                        egui::Rect::from_center_size(p, egui::vec2(DOT * 2.0, DOT * 2.0)),
                        1.0,
                        accent,
                    );
                }
            }
            // the rectangle of the handle — for live scenarios (R.27)
            if crate::widgets::field_probe::on() {
                crate::widgets::field_probe::note(
                    &format!("geom.{}", handle_tag(*h)),
                    egui::Rect::from_center_size(p, egui::vec2(GRAB, GRAB)),
                );
            }
        }
        if crate::widgets::field_probe::on() {
            crate::widgets::field_probe::note("geom.frame", egui::Rect::from_points(&cs));
        }

        // ---- GESTURE ------------------------------------------------------
        // DIAGNOSTICS for the scene, for live scenarios and tests: the control reached
        // gesture parsing (otherwise “no outline” and “gesture not caught” look
        // the same)
        self.geom_ctl_drawn = true;
        let id = ui.id().with(("geomctl", clip.0));
        // the gesture rectangle covers the HANDLES too: the rotation handle hangs
        // ABOVE the top side, and an outline made only of corners does not include it —
        // a press would go to play/pause of the preview (caught by t760b).
        let mut pts: Vec<egui::Pos2> = cs.to_vec();
        for h in mode.handles() {
            pts.push(handle_screen(&pl, *h, &to_screen));
        }
        let body = egui::Rect::from_points(&pts);
        let resp = ui.interact(body.expand(GRAB), id, egui::Sense::click_and_drag());
        let pointer = resp.interact_pointer_pos().or_else(|| ui.ctx().pointer_latest_pos());
        let alt = ui.input(|i| i.modifiers.alt);

        // The GESTURE is keyed by the PRESS POINT (`press_origin`), not by the fact
        // that a “drag is going on”. Two reasons, both caught by tests:
        //
        // 1. a HANDLE can be selected only by the PRESS point: by the first
        //    frame that reaches us, the pointer has already left the grab zone,
        //    and the handle gesture turned into moving the body (t760b);
        // 2. the geometry snapshot is taken ONCE per press: egui reports
        //    stop/start in the middle of a movement, and re-taking it while
        //    `press_origin` is alive counted the distance traveled twice (t760).
        let origin = ui.ctx().input(|i| i.pointer.press_origin()).or(pointer);
        if resp.dragged()
            && let Some(o) = origin
        {
            let oc = to_canvas(o);
            let same = self.geom_drag.is_some_and(|d| {
                d.clip == clip
                    && (d.press.0 - oc.0).abs() < 0.01
                    && (d.press.1 - oc.1).abs() < 0.01
            });
            if !same {
                self.gesture_counter += 1;
                let handle = nearest_handle(&pl, mode, o, &to_screen, alt);
                let start = self.geom_effective(clip).unwrap_or_default();
                self.geom_drag = Some(GeomDrag {
                    clip,
                    handle,
                    gesture: self.gesture_counter,
                    start,
                    press: oc,
                });
            }
        }
        if resp.hovered() {
            let over = pointer.and_then(|p| nearest_handle(&pl, mode, p, &to_screen, alt));
            ui.ctx().set_cursor_icon(match over {
                Some(Handle::Rot) => egui::CursorIcon::Alias,
                Some(Handle::Anchor) => egui::CursorIcon::Crosshair,
                Some(_) => egui::CursorIcon::ResizeNwSe,
                None => egui::CursorIcon::Grab,
            });
            crate::widgets::hint_sink::put(hint_for(over));
        }
        if resp.drag_stopped() {
            self.geom_drag = None;
        }
        if !resp.dragged() {
            return;
        }
        let Some(drag) = self.geom_drag.filter(|d| d.clip == clip) else { return };
        let Some(ptr) = pointer else { return };
        let Some(g0) = self.geom_effective(clip) else { return };
        // EVERYTHING is computed from the START of the gesture: the placement at the moment of press and
        // the full pointer offset. A per-frame delta applied the movement
        // of the frame in which the drag started twice (t760).
        let start = drag.start;
        let Some(pl0) = self.geom_placement_of(&start, clip, canvas_f) else { return };
        let mut g = start;
        let p = to_canvas(ptr);
        let dc = (p.0 - drag.press.0, p.1 - drag.press.1);
        let shift = ui.input(|i| i.modifiers.shift);
        match drag.handle {
            None => {
                let pos = pos_for_drag((start.pos_x, start.pos_y), canvas_f, dc);
                g.pos_x = pos.0;
                g.pos_y = pos.1;
            }
            Some(Handle::Rot) => g.rot = rot_for_point(&pl0, p, shift.then_some(15.0)),
            Some(Handle::Anchor) => {
                let (a, pos) = anchor_move(&pl0, p, (start.pos_x, start.pos_y), canvas_f);
                g.anchor_x = a.0;
                g.anchor_y = a.1;
                g.pos_x = pos.0;
                g.pos_y = pos.1;
            }
            Some(h) if matches!(h, Handle::CropLeft | Handle::CropTop | Handle::CropRight | Handle::CropBottom) => {
                if let Some(cr) = crop_for_handle(
                    &pl0,
                    h,
                    p,
                    [start.crop_l, start.crop_t, start.crop_r, start.crop_b],
                )
                {
                    g.crop_l = cr[0];
                    g.crop_t = cr[1];
                    g.crop_r = cr[2];
                    g.crop_b = cr[3];
                }
            }
            Some(h) => {
                if let Some((z, pos)) =
                    zoom_for_handle(&pl0, h, p, start.zoom, (start.pos_x, start.pos_y), canvas_f)
                {
                    g.zoom = z;
                    g.pos_x = pos.0;
                    g.pos_y = pos.1;
                }
            }
        }
        if g != g0 {
            self.set_geometry_coalesced(clip, g, drag.gesture);
        }
    }

    /// Record the event geometry by the same path as the FX window: one command, one
    /// gesture token — so also one history step (rule 17).
    pub(crate) fn set_geometry_coalesced(&mut self, clip: ClipId, g: EventGeometry, gesture: u64) {
        let Some(c) = self.model.clip(clip) else { return };
        let pos = c.geometry_pos;
        let mut base = c.geometry;
        let mut auto = c.geometry_auto.clone();
        let at = self.geom_curve_at(clip);
        // A parameter with a LIVE CURVE is edited by a KEYFRAME at the cursor — by the same
        // rule (`CurveBind`) as the FX window field. There must be no second rule in the program
        // for “where to write the value” (rule 17): otherwise the gesture
        // would erase the animation while showing its own value.
        {
            let mut bind = crate::widgets::CurveBind { curves: &mut auto, sel: None, at };
            let names: Vec<String> = base.param_specs().iter().map(|s| s.name.to_string()).collect();
            for n in names {
                let Some(v) = g.get_param(&n) else { continue };
                if bind.shown(&base, &n) == v {
                    continue; // keyframes are not seeded “just in case” by the gesture
                }
                if bind.has_curve(&base, &n) {
                    bind.apply(&mut base, &n, v);
                } else {
                    base.set_param(&n, v);
                }
            }
        }
        base.rule = g.rule; // the snap rule — not a number, it has no curve
        for c in auto.iter_mut() {
            c.keys.sort_by_key(|k| k.t.0); // the model requires key order
        }
        self.apply_coalesced(Command::SetGeometry { id: clip, geometry: base, pos, auto }, gesture);
    }
}

/// The SCREEN point of the handle — taking into account the OUTWARD push of a small outline (R.66).
///
/// When a side of the outline on screen is less than three grab zones, the eight scale handles
/// cover it entirely, leaving no room to grab the body: for the owner, on
/// `paintitblack.otio` the outline came out 45×25 px (the “as is” rule plus a
/// placeholder instead of unavailable media). Then the handles move out to the grab zone
/// OUTWARD along the normal of their edge, and the body stays under the move.
///
/// One function for the drawing, the rectangles for live scenarios, and the hit test:
/// if they diverged, a handle would be drawn in one place and caught in another.
pub(crate) fn handle_screen(
    pl: &Placement,
    h: Handle,
    to_screen: &dyn Fn((f64, f64)) -> egui::Pos2,
) -> egui::Pos2 {
    let p = to_screen(h.point(pl));
    if !small_frame(pl, to_screen) {
        return p;
    }
    // The ROTATION HANDLE on a small outline also moves outward — above the top
    // edge: inside, it sits at the center and eats the move (t781). The rule
    // R.64 “rotation inside” stays for a regular outline, where outside it
    // would stick out past the frame.
    if h == Handle::Rot {
        let top = handle_screen(pl, Handle::Top, to_screen);
        let Some(n) = Handle::Top.outward(pl) else { return p };
        let a = Handle::Top.point(pl);
        let d = to_screen((a.0 + n.0, a.1 + n.1)) - to_screen(a);
        let len = d.length();
        return if len < 1e-6 { p } else { top + d / len * GRAB };
    }
    let Some(n) = h.outward(pl) else { return p };
    // the direction is taken ON SCREEN: the preview scale can be anything
    let a = h.point(pl);
    let b = (a.0 + n.0, a.1 + n.1);
    let d = to_screen(b) - p;
    let len = d.length();
    if len < 1e-6 {
        return p;
    }
    p + d / len * GRAB
}

/// Is the outline small ON SCREEN: a side less than three grab zones.
pub(crate) fn small_frame(pl: &Placement, to_screen: &dyn Fn((f64, f64)) -> egui::Pos2) -> bool {
    let cs = pl.corners().map(&to_screen);
    let w = (cs[1] - cs[0]).length();
    let hgt = (cs[3] - cs[0]).length();
    w.min(hgt) < GRAB * 3.0
}

/// The handle of the active set nearest to the pointer — or `None` (the body).
///
/// Priority goes to the nearest, not to the “first one found”: on a small outline the grab zones
/// overlap, and the rule “whoever is closer” is the only unambiguous one.
pub fn nearest_handle(
    pl: &Placement,
    mode: GeomMode,
    p: egui::Pos2,
    to_screen: &dyn Fn((f64, f64)) -> egui::Pos2,
    alt: bool,
) -> Option<Handle> {
    let mut best: Option<(Handle, f32)> = None;
    for h in mode.handles() {
        // The ANCHOR takes the POINT ITSELF (a narrow zone), the other handles take the
        // full grab zone. The reason for the narrowness: by default the anchor sits at the
        // CENTER of the outline, which is the most natural place to grab the picture —
        // with a full zone it would take the move for itself. Alt widens the zone to the
        // common one (more convenient when the anchor must be caught for sure).
        //
        // ONLY Alt cannot be used here: Alt+drag is intercepted by almost every window manager
        // (openbox, GNOME, KDE) for moving the WINDOW — the gesture never reaches
        // the program at all.
        // Caught by the live scenario `geomctl`; the harness does not show this.
        let lim = if *h == Handle::Anchor && !alt { ANCHOR_GRAB } else { GRAB };
        let d = handle_screen(pl, *h, to_screen).distance(p);
        if d <= lim && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((*h, d));
        }
    }
    best.map(|(h, _)| h)
}

fn handle_tag(h: Handle) -> &'static str {
    match h {
        Handle::TopLeft => "tl",
        Handle::Top => "t",
        Handle::TopRight => "tr",
        Handle::Right => "r",
        Handle::BottomRight => "br",
        Handle::Bottom => "b",
        Handle::BottomLeft => "bl",
        Handle::Left => "l",
        Handle::Rot => "rot",
        Handle::Anchor => "anchor",
        Handle::CropLeft => "crop_l",
        Handle::CropTop => "crop_t",
        Handle::CropRight => "crop_r",
        Handle::CropBottom => "crop_b",
    }
}

/// Hint — into the status line (rule 10: no pop-ups).
fn hint_for(h: Option<Handle>) -> &'static str {
    use crate::i18n::{k, tr};
    match h {
        None => tr(k::geomctl::HINT_BODY),
        Some(Handle::Rot) => tr(k::geomctl::HINT_ROT),
        Some(Handle::Anchor) => tr(k::geomctl::HINT_ANCHOR),
        Some(
            Handle::CropLeft | Handle::CropTop | Handle::CropRight | Handle::CropBottom,
        ) => tr(k::geomctl::HINT_CROP),
        Some(_) => tr(k::geomctl::HINT_SCALE),
    }
}

/// The human-readable name of the set — for the status line and the menu.
pub fn mode_name(m: GeomMode) -> &'static str {
    use crate::i18n::{k, tr};
    match m {
        GeomMode::Off => tr(k::geomctl::MODE_OFF),
        GeomMode::Transform => tr(k::geomctl::MODE_TRANSFORM),
        GeomMode::Crop => tr(k::geomctl::MODE_CROP),
    }
}

// ---- FIELD FORM “Transform” (R.64, section 5) -----------------------
//
// The owner's claim, verbatim (R.63 item 6): “the geometry fx is still very meager,
// plain boring input fields, even Vegas does it better”.
// The form is assembled from EXISTING bricks (`param_row_bound_ex`, the publish door
// at the label, `CurveBind`): no new rules for “where to write the value”
// are introduced; grouping, units, sliders and a scheme are added.

/// The share is shown as a PERCENT (item 415): the recalculation lives in `param_value`,
/// in the model and in the file the value stays a share.
fn shown_spec(s: &kt_model::ParamSpec) -> kt_model::ParamSpec {
    let mut s = s.clone();
    if matches!(
        &*s.name,
        "crop_l" | "crop_t" | "crop_r" | "crop_b" | "pos_x" | "pos_y" | "anchor_x" | "anchor_y"
    ) {
        s.unit = kt_model::ParamUnit::Percent;
    }
    s
}

/// The unit in which the parameter is SHOWN in the form (for test t759: the unit
/// lives in the descriptor, not in the label text).
#[allow(dead_code)] // test t759 checks: the unit lives in the descriptor
pub(crate) fn shown_spec_unit(name: &str) -> Option<kt_model::ParamUnit> {
    let g = EventGeometry::default().node();
    let s = g.param_specs().iter().find(|s| &*s.name == name)?.clone();
    Some(shown_spec(&s).unit)
}

/// A form row: a pair of fields on one line (X and Y side by side, as in Pr/AE).
fn geom_row(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    specs: &[kt_model::ParamSpec],
    names: &[&str],
    slider: Option<f32>,
    bind: &mut Option<&mut crate::widgets::CurveBind<'_>>,
    pubc: Option<&crate::widgets::PublishCtx>,
) -> crate::widgets::ParamsChanged {
    let mut ch = crate::widgets::ParamsChanged::default();
    ui.horizontal(|ui| {
        for n in names {
            let Some(s) = specs.iter().find(|s| &*s.name == *n) else { continue };
            let s = shown_spec(s);
            ch |= crate::widgets::param_row_bound_ex(
                ui,
                f,
                &s,
                bind.as_deref_mut(),
                pubc,
                slider,
            );
        }
    });
    ch
}

/// The scheme rectangle and the anchor point — by the SAME placement function as the render
/// and the on-screen controls, computed on a canvas the size of the scheme.
/// So the crop, zoom, rotation and anchor on it are not a drawing “in the spirit of”, but the
/// very same numbers. Kept apart from drawing, so that a test, not the eye, confirms it.
pub(crate) type SchemaShape = ([(f64, f64); 4], (f64, f64));

pub(crate) fn schema_quad(f: &FxInstance, size: egui::Vec2) -> Option<SchemaShape> {
    let canvas = (size.x.max(4.0) as f64, size.y.max(4.0) as f64);
    let g = EventGeometry::default().with_node(f);
    // the source view is unknown here, so the input equals the canvas: the scheme
    // shows the outline RELATIVE to the frame, and the conform rule on such
    // an input degenerates into the identity for any of its own values.
    let lead = LeadCtx::new(None, kt_fx::conform::ConformRule::Fit);
    let pl = geometry_placement(&g.node(), Some(lead), canvas, canvas)?;
    Some((pl.corners(), pl.anchor))
}

/// The SCHEME FITS the outline and the frame into its rectangle (R.67).
///
/// Owner: “if you scale even 2x, you simply have nothing in the preview”. The scheme drew the outline
/// in canvas coordinates and clipped it by its own rectangle: at a zoom above one there was nothing to see.
/// Now the scale is chosen by the bounds of `union(frame, outline)`,
/// and the frame is drawn inside the same scheme as a stroke:
/// both are visible.
fn schema_fit(quad: &[(f64, f64); 4], anchor: (f64, f64), size: egui::Vec2) -> (f32, egui::Vec2) {
    let (mut lo_x, mut hi_x) = (0.0f64, size.x as f64);
    let (mut lo_y, mut hi_y) = (0.0f64, size.y as f64);
    for c in quad.iter().chain(std::iter::once(&anchor)) {
        lo_x = lo_x.min(c.0);
        hi_x = hi_x.max(c.0);
        lo_y = lo_y.min(c.1);
        hi_y = hi_y.max(c.1);
    }
    let (w, h) = ((hi_x - lo_x).max(1e-6), (hi_y - lo_y).max(1e-6));
    let pad = 3.0;
    let k = (((size.x - 2.0 * pad) as f64 / w).min((size.y - 2.0 * pad) as f64 / h)).min(1.0) as f32;
    // center the bounds inside the scheme rectangle
    let off = egui::vec2(
        (size.x - (w as f32) * k) * 0.5 - (lo_x as f32) * k,
        (size.y - (h as f32) * k) * 0.5 - (lo_y as f32) * k,
    );
    (k, off)
}

fn frame_schema(ui: &mut egui::Ui, f: &FxInstance, size: egui::Vec2) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    let dim = egui::Color32::from_rgb(90, 92, 100);
    p.rect_filled(rect, 2.0, egui::Color32::from_rgb(18, 18, 22));
    p.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, dim), egui::StrokeKind::Inside);
    let Some((quad, anchor)) = schema_quad(f, rect.size()) else { return resp };
    let (k, off) = schema_fit(&quad, anchor, rect.size());
    let to = |q: (f64, f64)| {
        egui::pos2(rect.left() + q.0 as f32 * k + off.x, rect.top() + q.1 as f32 * k + off.y)
    };
    // the FRAME — inside the same scheme: without it “an outline twice the frame” cannot be
    // told apart from “the outline fitted to the frame”
    let frame = egui::Rect::from_min_max(to((0.0, 0.0)), to((size.x as f64, size.y as f64)));
    p.rect_stroke(frame, 0.0, egui::Stroke::new(1.0, dim), egui::StrokeKind::Middle);
    let cs: Vec<egui::Pos2> = quad.iter().map(|c| to(*c)).collect();
    p.add(egui::Shape::closed_line(cs, egui::Stroke::new(1.5, crate::theme::ACCENT)));
    let a = to(anchor);
    p.circle_stroke(a, 3.0, egui::Stroke::new(1.5, crate::theme::ACCENT));
    p.line_segment([a - egui::vec2(5.0, 0.0), a + egui::vec2(5.0, 0.0)], (1.0, dim));
    p.line_segment([a - egui::vec2(0.0, 5.0), a + egui::vec2(0.0, 5.0)], (1.0, dim));
    resp
}

/// The corners of the outline and of the frame in the SCHEME's coordinates — for the
/// test: the assertion “visible at 2×” is made on what is drawn.
#[cfg_attr(not(test), allow(dead_code))] // the scheme is checked by test t765b
pub(crate) fn schema_points(
    f: &FxInstance,
    size: egui::Vec2,
) -> Option<([egui::Pos2; 4], egui::Rect)> {
    let (quad, anchor) = schema_quad(f, size)?;
    let (k, off) = schema_fit(&quad, anchor, size);
    let to = |q: (f64, f64)| egui::pos2(q.0 as f32 * k + off.x, q.1 as f32 * k + off.y);
    let cs = quad.map(to);
    Some((cs, egui::Rect::from_min_max(to((0.0, 0.0)), to((size.x as f64, size.y as f64)))))
}

/// The “Transform” section of the leading node: groups, X/Y pairs on one line,
/// sliders for zoom and rotation, the outline scheme, and a reset in one history step.
pub(crate) fn transform_form(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    mut bind: Option<&mut crate::widgets::CurveBind<'_>>,
    hover: &mut Option<String>,
    pubc: Option<&crate::widgets::PublishCtx>,
) -> crate::widgets::ParamsChanged {
    use crate::i18n::{k, tr};
    let mut ch = crate::widgets::ParamsChanged::default();
    let specs: Vec<kt_model::ParamSpec> = f.param_specs().to_vec();
    // collapsing of groups is the WINDOW state (egui memory), not the project's
    let group = |ui: &mut egui::Ui, id: &str, title: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
        egui::CollapsingHeader::new(title)
            .id_salt(("geoform", id))
            .default_open(true)
            .show(ui, |ui| body(ui));
    };

    group(ui, "frame", tr(k::geomctl::GROUP_FRAME), &mut |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ch |= geom_row(ui, f, &specs, &["zoom"], Some(90.0), &mut bind, pubc);
                ch |= geom_row(ui, f, &specs, &["rot"], Some(90.0), &mut bind, pubc);
            });
            let r = frame_schema(ui, f, egui::vec2(116.0, 66.0));
            crate::widgets::field_probe::note("geo.schema", r.rect);
            crate::app::hint_into(hover, r, tr(k::geomctl::SCHEMA_HINT));
        });
    });
    group(ui, "pos", tr(k::geomctl::GROUP_POS), &mut |ui| {
        ch |= geom_row(ui, f, &specs, &["pos_x", "pos_y"], None, &mut bind, pubc);
        ch |= geom_row(ui, f, &specs, &["anchor_x", "anchor_y"], None, &mut bind, pubc);
    });
    group(ui, "crop", tr(k::geomctl::GROUP_CROP), &mut |ui| {
        ch |= geom_row(ui, f, &specs, &["crop_l", "crop_r"], None, &mut bind, pubc);
        ch |= geom_row(ui, f, &specs, &["crop_t", "crop_b"], None, &mut bind, pubc);
    });

    let r = ui.button(tr(k::geomctl::RESET));
    crate::widgets::field_probe::note("geo.reset", r.rect);
    let reset = r.clicked();
    crate::app::hint_into(hover, r, tr(k::geomctl::RESET_HINT));
    if reset {
        // the reset goes by the SAME rule as the field: for an animated parameter
        // it is a keyframe at the cursor, not an erased curve (rule 20 — nothing
        // switches itself off)
        for s in &specs {
            if let Some(b) = bind.as_deref_mut()
                && b.has_curve(f, &s.name)
            {
                b.apply(f, &s.name, s.default);
                ch.curve = true;
                continue;
            }
            f.set_param(&s.name, s.default);
            ch.base = true;
        }
    }
    ch
}
