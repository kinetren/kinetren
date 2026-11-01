//! Masks on the preview picture (shape-masks, kkt-26.11.01 S1): drawing them with the
//! Rectangle / Ellipse / Pen tools and editing their points, handles and feather right over the
//! frame.
//!
//! The same three rules as the R.64 frame (`geomctl.rs`):
//!
//! 1. **One arithmetic with the render.** The overlay maps the mask's reference frame onto the
//!    canvas with `kt_fx::mask::MaskSpace::placed` over the event's `geometry_placement` — the
//!    very map the executors rasterize with; the outline comes from `Mask::cubics`, the shape's
//!    one geometry.
//! 2. **One command per gesture.** Every drag recomputes from the press and the masks AT the
//!    press, and goes through `SetMasks`/`SetTrackMasks` with the gesture token issued on the
//!    press — one history step. A row with a live curve gets a key at the cursor, an animated
//!    path keys every shape row (`Mask::write_gesture`); nothing is keyed without a gesture.
//! 3. **One set of handles at a time.** While a mask owner is edited, the transform frame is not
//!    drawn; Esc leaves the masks and the frame comes back.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_fx::mask::MaskSpace;
use kt_model::{ClipId, ClipKind, Command, Mask, MaskId, MaskShape, TrackId, Vertex};
use kt_time::Tick;

/// The grab zone, as the frame's.
pub const GRAB: f32 = crate::geomctl::GRAB;
const DOT: f32 = 4.0;
/// The rotation handle's distance above the box, screen px.
const ROT_ARM: f32 = 22.0;

/// Whose masks are edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskOwner {
    /// The event's picture (inside its Geometry node).
    Picture(ClipId),
    /// An effect slot of the event (a RAW slot index of `fx`).
    Slot(ClipId, usize),
    Track(TrackId),
}

impl MaskOwner {
    pub fn clip(self) -> Option<ClipId> {
        match self {
            MaskOwner::Picture(c) | MaskOwner::Slot(c, _) => Some(c),
            MaskOwner::Track(_) => None,
        }
    }
}

/// The armed tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskTool {
    Edit,
    Rect,
    Ellipse,
    Pen,
}

/// What a drag holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Grab {
    Body,
    Vertex(u32),
    HandleIn(u32),
    HandleOut(u32),
    /// A box handle of a primitive: the signs of its local corner (0 — the edge's middle).
    Box(i8, i8),
    Rot,
    Feather,
    /// A new rectangle / ellipse being drawn.
    Draw,
    /// A pen vertex being pulled smooth.
    PenSmooth,
}

#[derive(Debug, Clone)]
pub struct MaskDrag {
    pub gesture: u64,
    /// The press in REFERENCE px and on the canvas.
    pub press: (f64, f64),
    pub press_c: (f64, f64),
    pub grab: Grab,
    /// The edited mask at the press, evaluated at the cursor (the gesture computes from it).
    pub start: Option<Mask>,
}

/// The editing state on the preview. `None` in `KtApp.mask_ctl` — the R.64 frame rules.
#[derive(Debug, Clone)]
pub struct MaskEdit {
    pub owner: MaskOwner,
    pub mask: Option<MaskId>,
    pub tool: MaskTool,
    pub sel: Option<u32>,
    /// The unfinished pen path (normalized coordinates).
    pub pen: Vec<Vertex>,
    pub drag: Option<MaskDrag>,
}

impl MaskEdit {
    pub fn new(owner: MaskOwner, tool: MaskTool, mask: Option<MaskId>) -> MaskEdit {
        MaskEdit { owner, mask, tool, sel: None, pen: Vec::new(), drag: None }
    }
}

/// The overlay's map: reference px ↔ canvas px ↔ screen.
struct View {
    sp: MaskSpace,
    inv: [f64; 6],
    img: egui::Rect,
    canvas: (f64, f64),
}

impl View {
    fn to_canvas(&self, p: (f64, f64)) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.sp.xf;
        (a * p.0 + b * p.1 + c, d * p.0 + e * p.1 + f)
    }
    fn to_screen(&self, p: (f64, f64)) -> egui::Pos2 {
        let q = self.to_canvas(p);
        egui::pos2(
            self.img.left() + (q.0 / self.canvas.0) as f32 * self.img.width(),
            self.img.top() + (q.1 / self.canvas.1) as f32 * self.img.height(),
        )
    }
    fn screen_to_ref(&self, s: egui::Pos2) -> (f64, f64) {
        let q = (
            ((s.x - self.img.left()) / self.img.width().max(1.0)) as f64 * self.canvas.0,
            ((s.y - self.img.top()) / self.img.height().max(1.0)) as f64 * self.canvas.1,
        );
        let [a, b, c, d, e, f] = self.inv;
        (a * q.0 + b * q.1 + c, d * q.0 + e * q.1 + f)
    }
    fn canvas_of_screen(&self, s: egui::Pos2) -> (f64, f64) {
        (
            ((s.x - self.img.left()) / self.img.width().max(1.0)) as f64 * self.canvas.0,
            ((s.y - self.img.top()) / self.img.height().max(1.0)) as f64 * self.canvas.1,
        )
    }
    /// Screen px per canvas px.
    fn zoom(&self) -> f64 {
        self.img.width() as f64 / self.canvas.0.max(1.0)
    }
    fn norm(&self, p: (f64, f64)) -> (f64, f64) {
        (p.0 / self.sp.ref_w.max(1e-9), p.1 / self.sp.ref_h.max(1e-9))
    }
}

fn invert(m: [f64; 6]) -> [f64; 6] {
    let [a, b, c, d, e, f] = m;
    let det = a * e - b * d;
    if det.abs() < 1e-12 {
        return [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    }
    let (ia, ib, id, ie) = (e / det, -b / det, -d / det, a / det);
    [ia, ib, -(ia * c + ib * f), id, ie, -(id * c + ie * f)]
}

/// The outline of a mask on the screen: its cubics flattened (display only).
fn outline(m: &Mask, v: &View) -> Vec<egui::Pos2> {
    let mut pts = Vec::new();
    for seg in m.cubics(v.sp.ref_w, v.sp.ref_h) {
        let [p0, p1, p2, p3] = seg;
        for i in 0..16 {
            let t = i as f64 / 16.0;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            let p = (
                a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
                a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
            );
            pts.push(v.to_screen(p));
        }
    }
    pts
}

/// The same outline moved outward by `d` screen px along its normals (the feather's edge).
fn offset(pts: &[egui::Pos2], d: f32) -> Vec<egui::Pos2> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    // the orientation by the signed area: outward is to the right of a clockwise path (y down)
    let area: f32 = (0..n).map(|i| pts[i].x * pts[(i + 1) % n].y - pts[(i + 1) % n].x * pts[i].y).sum();
    let s = if area > 0.0 { 1.0 } else { -1.0 };
    (0..n)
        .map(|i| {
            let t = (pts[(i + 1) % n] - pts[(i + n - 1) % n]).normalized();
            pts[i] + egui::vec2(-t.y, t.x) * (-s * d)
        })
        .collect()
}

fn inside(pts: &[egui::Pos2], p: egui::Pos2) -> bool {
    let mut w = false;
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            w = !w;
        }
    }
    w
}

/// A primitive's local box: centre, half sizes (reference px), rotation (degrees).
type PrimBox = ((f64, f64), (f64, f64), f64);

/// The local box of a primitive in reference px.
fn prim_box(m: &Mask, rw: f64, rh: f64) -> Option<PrimBox> {
    match m.shape {
        MaskShape::Rect { cx, cy, w, h, rot, .. } | MaskShape::Ellipse { cx, cy, w, h, rot } => {
            Some(((cx * rw, cy * rh), ((w * rw / 2.0).abs(), (h * rh / 2.0).abs()), rot))
        }
        MaskShape::Path { .. } => None,
    }
}

fn rotate(p: (f64, f64), deg: f64) -> (f64, f64) {
    let (s, c) = deg.to_radians().sin_cos();
    (p.0 * c - p.1 * s, p.0 * s + p.1 * c)
}

/// The handles of the edited mask, in screen px, with what each grabs.
fn handles(m: &Mask, v: &View, sel: Option<u32>) -> Vec<(Grab, egui::Pos2)> {
    let mut out = Vec::new();
    let (rw, rh) = (v.sp.ref_w, v.sp.ref_h);
    match &m.shape {
        MaskShape::Path { verts } => {
            for p in verts {
                out.push((Grab::Vertex(p.id), v.to_screen((p.x * rw, p.y * rh))));
            }
            if let Some(p) = sel.and_then(|id| verts.iter().find(|p| p.id == id)) {
                out.push((Grab::HandleOut(p.id), v.to_screen(((p.x + p.ox) * rw, (p.y + p.oy) * rh))));
                out.push((Grab::HandleIn(p.id), v.to_screen(((p.x + p.ix) * rw, (p.y + p.iy) * rh))));
            }
        }
        _ => {
            if let Some((c, hs, rot)) = prim_box(m, rw, rh) {
                for (sx, sy) in [(-1, -1), (0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0)] {
                    let l = rotate((sx as f64 * hs.0, sy as f64 * hs.1), rot);
                    out.push((Grab::Box(sx, sy), v.to_screen((c.0 + l.0, c.1 + l.1))));
                }
                let top = v.to_screen({
                    let l = rotate((0.0, -hs.1), rot);
                    (c.0 + l.0, c.1 + l.1)
                });
                let ctr = v.to_screen(c);
                let dir = (top - ctr).normalized();
                let dir = if dir.is_finite() { dir } else { egui::vec2(0.0, -1.0) };
                out.push((Grab::Rot, top + dir * ROT_ARM));
            }
        }
    }
    out
}

/// The probe name of a drawn handle (the live scenario aims by it). Only handles are drawn;
/// a body or a drawing gesture has no rectangle.
fn handle_tag(g: Grab) -> Option<String> {
    Some(match g {
        Grab::Vertex(id) => format!("mask.v.{id}"),
        Grab::HandleOut(id) => format!("mask.out.{id}"),
        Grab::Rot => "mask.rot".into(),
        Grab::Feather => "mask.feather".into(),
        _ => return None,
    })
}

fn hint_for(g: Option<Grab>, tool: MaskTool) -> &'static str {
    match (tool, g) {
        (MaskTool::Pen, _) => tr(k::masks::HINT_PEN),
        (MaskTool::Rect | MaskTool::Ellipse, _) => tr(k::masks::HINT_DRAW),
        (_, Some(Grab::Vertex(_))) => tr(k::masks::HINT_VERTEX),
        (_, Some(Grab::HandleIn(_) | Grab::HandleOut(_))) => tr(k::masks::HINT_TANGENT),
        (_, Some(Grab::Box(_, _))) => tr(k::masks::HINT_SCALE),
        (_, Some(Grab::Rot)) => tr(k::masks::HINT_ROT),
        (_, Some(Grab::Feather)) => tr(k::masks::HINT_FEATHER),
        _ => tr(k::masks::HINT_BODY),
    }
}

/// The parameter of segment `seg` nearest to `p` (screen), and the distance.
fn nearest_on_segment(m: &Mask, v: &View, p: egui::Pos2) -> Option<(usize, f64, f32)> {
    let mut best: Option<(usize, f64, f32)> = None;
    for (si, seg) in m.cubics(v.sp.ref_w, v.sp.ref_h).iter().enumerate() {
        let [p0, p1, p2, p3] = *seg;
        for i in 1..32 {
            let t = i as f64 / 32.0;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            let q = v.to_screen((
                a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
                a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
            ));
            let dist = q.distance(p);
            if best.is_none_or(|b| dist < b.2) {
                best = Some((si, t, dist));
            }
        }
    }
    best
}

impl KtApp {
    /// The masks of an owner as the model holds them.
    pub(crate) fn mask_list_of(&self, owner: MaskOwner) -> Option<Vec<Mask>> {
        match owner {
            MaskOwner::Picture(c) => self.model.clip(c).map(|c| c.masks.picture.clone()),
            MaskOwner::Slot(c, i) => self.model.clip(c).map(|c| c.masks.slot(i).to_vec()),
            MaskOwner::Track(t) => self.model.project().track(t).map(|t| t.masks.clone()),
        }
    }

    /// The cursor in the owner's curve domain: the event's material tick (a track has no
    /// curves — the playhead is only a placeholder there).
    pub(crate) fn mask_curve_at(&self, owner: MaskOwner) -> Tick {
        match owner.clip() {
            Some(c) => self.geom_curve_at(c),
            None => self.player.transport.playhead(),
        }
    }

    /// Write an owner's mask list: ONE command, the given gesture token.
    pub(crate) fn commit_masks(&mut self, owner: MaskOwner, list: Vec<Mask>, gesture: u64) -> bool {
        let cmd = match owner {
            MaskOwner::Picture(c) | MaskOwner::Slot(c, _) => {
                let Some(clip) = self.model.clip(c) else { return false };
                let mut m = clip.masks.clone();
                *m.list_mut(match owner {
                    MaskOwner::Slot(_, i) => Some(i),
                    _ => None,
                }) = list;
                m.canonicalize();
                Command::SetMasks { id: c, masks: m }
            }
            MaskOwner::Track(t) => Command::SetTrackMasks { id: t, masks: list },
        };
        self.apply_coalesced(cmd, gesture)
    }

    /// Arm a tool on an owner (the list's buttons, the preview menu).
    pub(crate) fn arm_mask_tool(&mut self, owner: MaskOwner, tool: MaskTool) {
        let mask = match self.mask_ctl.as_ref() {
            Some(e) if e.owner == owner => e.mask,
            _ => None,
        };
        self.mask_ctl = Some(MaskEdit::new(owner, tool, mask));
        self.geom_drag = None;
    }

    /// Select a mask for editing on the preview.
    pub(crate) fn edit_mask(&mut self, owner: MaskOwner, mask: Option<MaskId>) {
        self.mask_ctl = Some(MaskEdit::new(owner, MaskTool::Edit, mask));
        self.geom_drag = None;
    }

    /// Esc on the preview: an unfinished path is dropped first, then mask editing ends (the
    /// frame comes back). `true` — Esc was used here.
    pub(crate) fn mask_escape(&mut self) -> bool {
        let Some(e) = self.mask_ctl.as_mut() else { return false };
        if !e.pen.is_empty() {
            e.pen.clear();
            return true;
        }
        if e.tool != MaskTool::Edit {
            e.tool = MaskTool::Edit;
            return true;
        }
        self.mask_ctl = None;
        true
    }

    /// The keys of mask editing (Enter closes the pen path, Backspace drops its last point,
    /// Delete removes the selected vertex). `(del_used, enter_used)`.
    pub(crate) fn mask_keys(&mut self, ctx: &egui::Context) -> (bool, bool) {
        let Some(e) = self.mask_ctl.clone() else { return (false, false) };
        if ctx.egui_wants_keyboard_input() {
            return (false, false);
        }
        let (enter, back, del) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Backspace),
                i.key_pressed(egui::Key::Delete),
            )
        });
        if e.tool == MaskTool::Pen {
            if enter && e.pen.len() >= 3 {
                self.finish_pen();
                return (false, true);
            }
            if back || del {
                if let Some(m) = self.mask_ctl.as_mut() {
                    m.pen.pop();
                }
                return (true, false);
            }
            return (false, false);
        }
        if (del || back)
            && let (Some(mid), Some(vid)) = (e.mask, e.sel)
        {
            let Some(mut list) = self.mask_list_of(e.owner) else { return (true, false) };
            let Some(m) = list.iter_mut().find(|m| m.id == mid) else { return (true, false) };
            if !m.remove_vertex(vid) {
                self.refuse(tr(k::masks::THREE_POINTS));
                return (true, false);
            }
            self.gesture_counter += 1;
            let g = self.gesture_counter;
            self.commit_masks(e.owner, list, g);
            if let Some(m) = self.mask_ctl.as_mut() {
                m.sel = None;
            }
            return (true, false);
        }
        (false, false)
    }

    /// Close the pen path into a new mask (one history step).
    fn finish_pen(&mut self) {
        let Some(e) = self.mask_ctl.clone() else { return };
        if e.pen.len() < 3 {
            return;
        }
        let Some(mut list) = self.mask_list_of(e.owner) else { return };
        let id = kt_model::next_mask_id(&list);
        let name = kt_model::next_mask_name(&list, tr(k::masks::STEM));
        let verts = e.pen.iter().enumerate().map(|(i, v)| Vertex { id: i as u32, ..*v }).collect();
        list.push(Mask::new(id, name.clone(), MaskShape::Path { verts }));
        self.gesture_counter += 1;
        let g = self.gesture_counter;
        if self.commit_masks(e.owner, list, g) {
            self.note(crate::tf!(k::masks::ADDED, name));
        }
        self.mask_ctl = Some(MaskEdit::new(e.owner, MaskTool::Edit, Some(id)));
    }

    /// The overlay's map for an owner on this canvas.
    fn mask_view(&self, owner: MaskOwner, img: egui::Rect, canvas: (u32, u32)) -> Option<View> {
        let canvas_f = (canvas.0.max(2) as f64, canvas.1.max(2) as f64);
        let project_h = self.model.project().settings.height.max(1) as f64;
        let sp = match owner {
            MaskOwner::Picture(c) | MaskOwner::Slot(c, _) => {
                let clip = self.model.clip(c)?;
                if clip.kind != ClipKind::Video {
                    return None;
                }
                let pl = self.geom_placement(c, canvas_f)?;
                // the reference frame is the source the node places; unknown — the canvas
                let r = self.player.source_view(clip.media).unwrap_or(canvas_f);
                MaskSpace::placed(&pl, r, canvas_f, canvas_f, project_h)
            }
            MaskOwner::Track(_) => MaskSpace::plain(canvas_f.0, canvas_f.1, canvas.0, canvas.1, project_h),
        };
        Some(View { inv: invert(sp.xf), sp, img, canvas: canvas_f })
    }

    /// A point of the owner's reference frame (normalized) on the screen — the same map the overlay
    /// draws with (for the tests that act where a person acts).
    #[cfg(test)]
    pub(crate) fn mask_point_on_screen(&self, owner: MaskOwner, p: (f64, f64)) -> Option<egui::Pos2> {
        let q = self.player.preview_quality();
        let v = self.mask_view(owner, self.preview_img_rect, (q.width, q.height))?;
        Some(v.to_screen((p.0 * v.sp.ref_w, p.1 * v.sp.ref_h)))
    }

    /// Is the owner on screen now (its event under the playhead)?
    fn mask_owner_live(&self, owner: MaskOwner) -> bool {
        match owner {
            MaskOwner::Picture(c) | MaskOwner::Slot(c, _) => self.model.clip(c).is_some_and(|c| {
                let t = self.player.transport.playhead();
                c.kind == ClipKind::Video && t >= c.start && t < c.start + c.duration
            }),
            MaskOwner::Track(t) => self.model.project().track(t).is_some(),
        }
    }

    /// The masks over the picture: drawing, tools and gestures. `true` — mask editing owns the
    /// preview this frame (the R.64 frame is not drawn). Called from the ONE place in
    /// `draw_preview` where the frame is.
    pub(crate) fn mask_controls(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        img: egui::Rect,
        canvas: (u32, u32),
    ) -> bool {
        let Some(edit) = self.mask_ctl.clone() else { return false };
        // the owner vanished (undo of its creation, the event removed) — editing ends
        if self.mask_list_of(edit.owner).is_none() {
            self.mask_ctl = None;
            return false;
        }
        if !self.mask_owner_live(edit.owner) {
            if ui.rect_contains_pointer(img) {
                crate::widgets::hint_sink::put(tr(k::masks::NO_TARGET));
            }
            return true;
        }
        let Some(v) = self.mask_view(edit.owner, img, canvas) else { return true };
        let at = self.mask_curve_at(edit.owner);
        let raw = self.mask_list_of(edit.owner).unwrap_or_default();
        let shown: Vec<Mask> = raw.iter().map(|m| m.at(at)).collect();
        let playing = self.player.transport.is_playing();
        self.mask_ctl_drawn = true;

        // ---- DRAWING ----------------------------------------------------
        let accent = crate::theme::ACCENT;
        let dim = egui::Color32::from_rgba_unmultiplied(255, 210, 120, 170);
        for (i, m) in shown.iter().enumerate() {
            let pts = outline(m, &v);
            let on = edit.mask == Some(m.id);
            let col = if on { accent } else { dim };
            painter.add(egui::Shape::closed_line(pts.clone(), egui::Stroke::new(if on { 1.6 } else { 1.0 }, col)));
            if m.feather > 0.0 {
                let d = (m.feather / 2.0 * v.sp.px * v.zoom()) as f32;
                let mut o = offset(&pts, d);
                if let Some(f) = o.first().copied() {
                    o.push(f);
                }
                painter.extend(egui::Shape::dashed_line(&o, egui::Stroke::new(1.0, col.gamma_multiply(0.7)), 4.0, 4.0));
            }
            if crate::widgets::field_probe::on() && !pts.is_empty() {
                crate::widgets::field_probe::note(&format!("mask.contour.{i}"), egui::Rect::from_points(&pts));
            }
        }
        // the unfinished pen path
        if !edit.pen.is_empty() {
            let pts: Vec<egui::Pos2> = edit
                .pen
                .iter()
                .map(|p| v.to_screen((p.x * v.sp.ref_w, p.y * v.sp.ref_h)))
                .collect();
            painter.add(egui::Shape::line(pts.clone(), egui::Stroke::new(1.5, accent)));
            for (i, p) in pts.iter().enumerate() {
                painter.rect_filled(egui::Rect::from_center_size(*p, egui::vec2(DOT * 2.0, DOT * 2.0)), 1.0, accent);
                crate::widgets::field_probe::note(&format!("mask.pen.{i}"), egui::Rect::from_center_size(*p, egui::vec2(GRAB, GRAB)));
            }
            if let (Some(last), Some(ptr)) = (pts.last(), ui.ctx().pointer_latest_pos())
                && img.contains(ptr)
            {
                painter.line_segment([*last, ptr], egui::Stroke::new(1.0, accent.gamma_multiply(0.6)));
            }
        }
        crate::widgets::field_probe::note("mask.picture", img);
        // during PLAYBACK only the contours (they follow the animation); the handles wait
        if playing {
            return true;
        }
        let cur = edit.mask.and_then(|id| shown.iter().find(|m| m.id == id)).cloned();
        let mut hs: Vec<(Grab, egui::Pos2)> = Vec::new();
        if edit.tool == MaskTool::Edit
            && let Some(m) = &cur
        {
            hs = handles(m, &v, edit.sel);
            // the feather handle: on the outer edge above the topmost point
            let pts = outline(m, &v);
            if let Some(top) = pts.iter().copied().min_by(|a, b| a.y.total_cmp(&b.y)) {
                let d = (m.feather / 2.0 * v.sp.px * v.zoom()) as f32;
                hs.push((Grab::Feather, top - egui::vec2(0.0, d + 10.0)));
            }
            for (g, p) in &hs {
                match g {
                    Grab::Vertex(id) => {
                        let fill = edit.sel == Some(*id);
                        let r = egui::Rect::from_center_size(*p, egui::vec2(DOT * 2.0, DOT * 2.0));
                        if fill {
                            painter.rect_filled(r, 1.0, accent);
                        } else {
                            painter.rect_stroke(r, 1.0, egui::Stroke::new(1.5, accent), egui::StrokeKind::Middle);
                        };
                    }
                    Grab::HandleIn(id) | Grab::HandleOut(id) => {
                        if let Some((_, vp)) = hs.iter().find(|(g2, _)| *g2 == Grab::Vertex(*id)) {
                            painter.line_segment([*vp, *p], egui::Stroke::new(1.0, accent.gamma_multiply(0.8)));
                        }
                        painter.circle_filled(*p, DOT - 0.5, accent);
                    }
                    Grab::Rot => {
                        painter.circle_stroke(*p, DOT, egui::Stroke::new(1.5, accent));
                    }
                    Grab::Feather => {
                        let r = DOT + 1.0;
                        painter.add(egui::Shape::convex_polygon(
                            vec![*p + egui::vec2(0.0, -r), *p + egui::vec2(r, 0.0), *p + egui::vec2(0.0, r), *p + egui::vec2(-r, 0.0)],
                            egui::Color32::TRANSPARENT,
                            egui::Stroke::new(1.5, accent),
                        ));
                    }
                    _ => {
                        painter.rect_filled(egui::Rect::from_center_size(*p, egui::vec2(DOT * 2.0, DOT * 2.0)), 1.0, accent);
                    }
                }
                if crate::widgets::field_probe::on()
                    && let Some(tag) = handle_tag(*g)
                {
                    crate::widgets::field_probe::note(&tag, egui::Rect::from_center_size(*p, egui::vec2(GRAB, GRAB)));
                }
            }
        }

        // ---- GESTURE -----------------------------------------------------
        // a drawing tool takes the whole picture (an explicit state: the click is the tool's);
        // editing takes the handles and the contours only, so a click elsewhere still plays
        let contours: Vec<Vec<egui::Pos2>> = shown.iter().map(|m| outline(m, &v)).collect();
        let zone = if edit.tool == MaskTool::Edit {
            let mut pts: Vec<egui::Pos2> = hs.iter().map(|(_, p)| *p).collect();
            for c in &contours {
                pts.extend(c.iter().copied());
            }
            if pts.is_empty() {
                return true;
            }
            egui::Rect::from_points(&pts).expand(GRAB).intersect(img.expand(GRAB))
        } else {
            img
        };
        let id = ui.id().with("maskctl");
        let resp = ui.interact(zone, id, egui::Sense::click_and_drag());
        let mut pick = crate::app::PreviewMenuPick::default();
        let (detached, mode_now) = (self.preview_detached, self.geom_ctl);
        resp.clone().context_menu(|ui| KtApp::preview_menu_ui(ui, detached, mode_now, &mut pick));
        self.apply_preview_pick(pick);
        let pointer = resp.interact_pointer_pos().or_else(|| ui.ctx().pointer_latest_pos());
        let alt = ui.input(|i| i.modifiers.alt);
        let shift = ui.input(|i| i.modifiers.shift);
        let near_handle = |p: egui::Pos2| -> Option<Grab> {
            hs.iter()
                .map(|(g, q)| (*g, q.distance(p)))
                .filter(|(_, d)| *d <= GRAB)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(g, _)| g)
        };
        // which mask is under a point: its contour within the grab zone, or its inside
        let mask_under = |p: egui::Pos2| -> Option<MaskId> {
            shown.iter().zip(&contours).rev().find_map(|(m, c)| {
                let near = c.iter().any(|q| q.distance(p) <= GRAB * 0.6);
                (near || inside(c, p)).then_some(m.id)
            })
        };
        if resp.hovered()
            && let Some(p) = pointer
        {
            let over = if edit.tool == MaskTool::Edit { near_handle(p) } else { None };
            ui.ctx().set_cursor_icon(match (edit.tool, over) {
                (MaskTool::Edit, Some(Grab::Rot)) => egui::CursorIcon::Alias,
                (MaskTool::Edit, Some(_)) => egui::CursorIcon::Crosshair,
                (MaskTool::Edit, None) => egui::CursorIcon::Grab,
                _ => egui::CursorIcon::Crosshair,
            });
            let segment = edit.tool == MaskTool::Edit
                && over.is_none()
                && cur.as_ref().is_some_and(|m| {
                    matches!(m.shape, MaskShape::Path { .. })
                        && nearest_on_segment(m, &v, p).is_some_and(|(_, _, d)| d <= GRAB * 0.6)
                });
            crate::widgets::hint_sink::put(if segment { tr(k::masks::HINT_SEGMENT) } else { hint_for(over, edit.tool) });
        }

        // CLICKS (no drag)
        if resp.clicked()
            && let Some(p) = pointer
        {
            match edit.tool {
                MaskTool::Pen => {
                    let first = edit.pen.first().map(|f| v.to_screen((f.x * v.sp.ref_w, f.y * v.sp.ref_h)));
                    if edit.pen.len() >= 3 && first.is_some_and(|f| f.distance(p) <= GRAB) {
                        self.finish_pen();
                    } else {
                        let q = v.norm(v.screen_to_ref(p));
                        let id = edit.pen.len() as u32;
                        if let Some(e) = self.mask_ctl.as_mut() {
                            e.pen.push(Vertex::corner(id, q.0, q.1));
                        }
                    }
                }
                MaskTool::Edit => {
                    let g = near_handle(p);
                    match (g, &cur) {
                        (Some(Grab::Vertex(vid)), Some(m)) if alt => self.toggle_vertex(edit.owner, m.id, vid),
                        (Some(Grab::Vertex(vid)), _) => {
                            if let Some(e) = self.mask_ctl.as_mut() {
                                e.sel = Some(vid);
                            }
                        }
                        (Some(_), _) => {}
                        (None, _) => {
                            let hit = mask_under(p);
                            if let Some(e) = self.mask_ctl.as_mut()
                                && hit.is_some()
                            {
                                if e.mask != hit {
                                    e.sel = None;
                                }
                                e.mask = hit;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if resp.double_clicked()
            && edit.tool == MaskTool::Edit
            && let (Some(p), Some(m)) = (pointer, &cur)
            && matches!(m.shape, MaskShape::Path { .. })
            && near_handle(p).is_none()
            && let Some((seg, t, d)) = nearest_on_segment(m, &v, p)
            && d <= GRAB
        {
            self.insert_mask_vertex(edit.owner, m.id, seg, t);
        }

        // DRAGS: the press decides what is held; the whole gesture computes from the press
        let origin = ui.ctx().input(|i| i.pointer.press_origin()).or(pointer);
        if resp.dragged()
            && let Some(o) = origin
        {
            let oc = v.canvas_of_screen(o);
            let same = edit.drag.as_ref().is_some_and(|d| (d.press_c.0 - oc.0).abs() < 0.01 && (d.press_c.1 - oc.1).abs() < 0.01);
            if !same {
                self.gesture_counter += 1;
                let grab = match edit.tool {
                    MaskTool::Rect | MaskTool::Ellipse => Some(Grab::Draw),
                    MaskTool::Pen => Some(Grab::PenSmooth),
                    MaskTool::Edit => near_handle(o).or_else(|| {
                        let hit = mask_under(o);
                        if hit.is_some() && hit != edit.mask
                            && let Some(e) = self.mask_ctl.as_mut()
                        {
                            e.mask = hit;
                            e.sel = None;
                        }
                        hit.map(|_| Grab::Body)
                    }),
                };
                let mask_now = self.mask_ctl.as_ref().and_then(|e| e.mask);
                let start = mask_now.and_then(|id| shown.iter().find(|m| m.id == id)).cloned();
                if let Grab::Vertex(vid) = grab.unwrap_or(Grab::Body)
                    && let Some(e) = self.mask_ctl.as_mut()
                {
                    e.sel = Some(vid);
                }
                // the pen's smooth vertex is placed at the press
                if grab == Some(Grab::PenSmooth)
                    && let Some(e) = self.mask_ctl.as_mut()
                {
                    let q = v.norm(v.screen_to_ref(o));
                    let id = e.pen.len() as u32;
                    e.pen.push(Vertex::corner(id, q.0, q.1));
                }
                if let Some(e) = self.mask_ctl.as_mut() {
                    e.drag = grab.map(|grab| MaskDrag {
                        gesture: self.gesture_counter,
                        press: v.screen_to_ref(o),
                        press_c: oc,
                        grab,
                        start: if grab == Grab::Draw { None } else { start },
                    });
                }
            }
        }
        if resp.drag_stopped() {
            if let Some(e) = self.mask_ctl.as_mut() {
                if e.drag.as_ref().is_some_and(|d| d.grab == Grab::Draw) {
                    e.tool = MaskTool::Edit;
                }
                e.drag = None;
            }
            return true;
        }
        if !resp.dragged() {
            return true;
        }
        let Some(drag) = self.mask_ctl.as_ref().and_then(|e| e.drag.clone()) else { return true };
        let Some(ptr) = pointer else { return true };
        let p = v.screen_to_ref(ptr);
        let d = (p.0 - drag.press.0, p.1 - drag.press.1);
        let (rw, rh) = (v.sp.ref_w, v.sp.ref_h);
        match drag.grab {
            Grab::Draw => self.drag_draw(edit.owner, edit.tool, &drag, p, shift, alt, (rw, rh)),
            Grab::PenSmooth => {
                if let Some(e) = self.mask_ctl.as_mut()
                    && let Some(last) = e.pen.last_mut()
                {
                    let (ox, oy) = (d.0 / rw, d.1 / rh);
                    last.ox = ox;
                    last.oy = oy;
                    last.ix = -ox;
                    last.iy = -oy;
                }
            }
            grab => {
                let Some(m0) = drag.start.clone() else { return true };
                let mut m = m0.clone();
                match (grab, &mut m.shape) {
                    (Grab::Body, MaskShape::Path { verts }) => {
                        for p in verts.iter_mut() {
                            p.x += d.0 / rw;
                            p.y += d.1 / rh;
                        }
                    }
                    (Grab::Body, MaskShape::Rect { cx, cy, .. } | MaskShape::Ellipse { cx, cy, .. }) => {
                        *cx += d.0 / rw;
                        *cy += d.1 / rh;
                    }
                    (Grab::Vertex(id), MaskShape::Path { verts }) => {
                        if let Some(q) = verts.iter_mut().find(|q| q.id == id) {
                            q.x += d.0 / rw;
                            q.y += d.1 / rh;
                        }
                    }
                    (Grab::HandleOut(id) | Grab::HandleIn(id), MaskShape::Path { verts }) => {
                        if let Some(q) = verts.iter_mut().find(|q| q.id == id) {
                            let rel = (p.0 / rw - q.x, p.1 / rh - q.y);
                            let (mine, other) = if matches!(grab, Grab::HandleOut(_)) {
                                ((&mut q.ox, &mut q.oy), (&mut q.ix, &mut q.iy))
                            } else {
                                ((&mut q.ix, &mut q.iy), (&mut q.ox, &mut q.oy))
                            };
                            *mine.0 = rel.0;
                            *mine.1 = rel.1;
                            // a smooth point keeps its handles in line; Alt breaks them apart
                            if !alt {
                                *other.0 = -rel.0;
                                *other.1 = -rel.1;
                            }
                        }
                    }
                    (Grab::Box(sx, sy), shape) => {
                        if let Some((c, hs, rot)) = prim_box(&m0, rw, rh) {
                            let l = rotate((p.0 - c.0, p.1 - c.1), -rot);
                            let mut hx = if sx != 0 { l.0.abs() } else { hs.0 };
                            let mut hy = if sy != 0 { l.1.abs() } else { hs.1 };
                            if shift && sx != 0 && sy != 0 {
                                let k = (hx / hs.0.max(1e-9)).max(hy / hs.1.max(1e-9));
                                hx = hs.0 * k;
                                hy = hs.1 * k;
                            }
                            if let MaskShape::Rect { w, h, .. } | MaskShape::Ellipse { w, h, .. } = shape {
                                *w = 2.0 * hx / rw;
                                *h = 2.0 * hy / rh;
                            }
                        }
                    }
                    (Grab::Rot, shape) => {
                        if let Some((c, _, rot0)) = prim_box(&m0, rw, rh) {
                            let a0 = (drag.press.1 - c.1).atan2(drag.press.0 - c.0);
                            let a1 = (p.1 - c.1).atan2(p.0 - c.0);
                            let mut r = rot0 + (a1 - a0).to_degrees();
                            if shift {
                                r = (r / 15.0).round() * 15.0;
                            }
                            if let MaskShape::Rect { rot, .. } | MaskShape::Ellipse { rot, .. } = shape {
                                *rot = r;
                            }
                        }
                    }
                    (Grab::Feather, _) => {
                        let pc = v.canvas_of_screen(ptr);
                        let up = drag.press_c.1 - pc.1;
                        m.feather = (m0.feather + 2.0 * up / v.sp.px.max(1e-9)).clamp(0.0, kt_model::FEATHER_MAX);
                    }
                    _ => {}
                }
                self.write_mask_gesture(edit.owner, m, drag.gesture);
            }
        }
        true
    }

    /// Draw a new rectangle / ellipse from the press to `p` (reference px): created on the first
    /// frame of the drag and reshaped by the same gesture token — one history step.
    #[allow(clippy::too_many_arguments)]
    fn drag_draw(
        &mut self,
        owner: MaskOwner,
        tool: MaskTool,
        drag: &MaskDrag,
        p: (f64, f64),
        shift: bool,
        alt: bool,
        (rw, rh): (f64, f64),
    ) {
        let (mut hx, mut hy) = ((p.0 - drag.press.0).abs(), (p.1 - drag.press.1).abs());
        let mut c = ((p.0 + drag.press.0) / 2.0, (p.1 + drag.press.1) / 2.0);
        if alt {
            c = drag.press;
        } else {
            hx /= 2.0;
            hy /= 2.0;
        }
        if shift {
            let m = hx.max(hy);
            if !alt {
                c = (drag.press.0 + (p.0 - drag.press.0).signum() * m, drag.press.1 + (p.1 - drag.press.1).signum() * m);
            }
            hx = m;
            hy = m;
        }
        if hx < 1.0 || hy < 1.0 {
            return; // a press, not a shape yet
        }
        let (cx, cy, w, h) = (c.0 / rw, c.1 / rh, 2.0 * hx / rw, 2.0 * hy / rh);
        let shape = if tool == MaskTool::Ellipse {
            MaskShape::Ellipse { cx, cy, w, h, rot: 0.0 }
        } else {
            MaskShape::Rect { cx, cy, w, h, rot: 0.0, round: 0.0 }
        };
        let Some(mut list) = self.mask_list_of(owner) else { return };
        let existing = self.mask_ctl.as_ref().and_then(|e| e.mask).filter(|_| drag.start.is_none());
        // the mask this drag created (its id is remembered in `mask` on the first frame)
        let made = self.mask_ctl.as_ref().and_then(|e| e.drag.as_ref()).and_then(|d| d.start.as_ref()).map(|m| m.id);
        let _ = existing;
        match made.and_then(|id| list.iter_mut().find(|m| m.id == id)) {
            Some(m) => m.shape = shape,
            None => {
                let id = kt_model::next_mask_id(&list);
                let name = kt_model::next_mask_name(&list, tr(k::masks::STEM));
                let m = Mask::new(id, name.clone(), shape);
                list.push(m.clone());
                if let Some(e) = self.mask_ctl.as_mut() {
                    e.mask = Some(id);
                    if let Some(d) = e.drag.as_mut() {
                        d.start = Some(m);
                    }
                }
                self.note(crate::tf!(k::masks::ADDED, name));
            }
        }
        self.commit_masks(owner, list, drag.gesture);
    }

    /// Write a gesture's mask through `Mask::write_gesture` (keys by gesture only).
    fn write_mask_gesture(&mut self, owner: MaskOwner, edited: Mask, gesture: u64) {
        let Some(mut list) = self.mask_list_of(owner) else { return };
        let at = self.mask_curve_at(owner);
        let shape = self.model.project().settings.default_shape;
        let Some(raw) = list.iter_mut().find(|m| m.id == edited.id) else { return };
        if matches!(owner, MaskOwner::Track(_)) {
            // a track's masks are static: the values themselves
            *raw = Mask { auto: Vec::new(), ..edited };
        } else {
            raw.write_gesture(at, &edited, shape);
        }
        self.commit_masks(owner, list, gesture);
    }

    /// Alt+click on a vertex: corner ↔ smooth (one step).
    fn toggle_vertex(&mut self, owner: MaskOwner, mask: MaskId, vid: u32) {
        let at = self.mask_curve_at(owner);
        let Some(m) = self.mask_list_of(owner).and_then(|l| l.into_iter().find(|m| m.id == mask)) else { return };
        let mut e = m.at(at);
        if let MaskShape::Path { verts } = &mut e.shape {
            let n = verts.len();
            if let Some(i) = verts.iter().position(|v| v.id == vid) {
                let (prev, next) = (verts[(i + n - 1) % n], verts[(i + 1) % n]);
                let q = &mut verts[i];
                if q.is_corner() {
                    let (ox, oy) = ((next.x - prev.x) / 6.0, (next.y - prev.y) / 6.0);
                    *q = Vertex { ox, oy, ix: -ox, iy: -oy, ..*q };
                } else {
                    *q = Vertex::corner(q.id, q.x, q.y);
                }
            }
        }
        self.gesture_counter += 1;
        let g = self.gesture_counter;
        self.write_mask_gesture(owner, e, g);
    }

    /// Double-click on a segment: a vertex there, keyed at every key of an animated path.
    fn insert_mask_vertex(&mut self, owner: MaskOwner, mask: MaskId, seg: usize, t: f64) {
        let Some(mut list) = self.mask_list_of(owner) else { return };
        let shape = self.model.project().settings.default_shape;
        let Some(m) = list.iter_mut().find(|m| m.id == mask) else { return };
        let Some(id) = m.insert_vertex(seg, t, shape) else { return };
        self.gesture_counter += 1;
        let g = self.gesture_counter;
        self.commit_masks(owner, list, g);
        if let Some(e) = self.mask_ctl.as_mut() {
            e.sel = Some(id);
        }
    }
}
