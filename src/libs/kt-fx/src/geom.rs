//! Fusion of geometric video FX (R.44, transform concatenation,
//! F-CMP-001): consecutive Pan/CamMotion/Mirror/EventGrid run in ONE
//! pass of inverse mappings — without intermediate rasterizations and
//! 8-bit quantizations between steps. Non-local FX are barriers.
//!
//! ALL decisions live here (segmentation, program, legacy-vs-fused):
//! kt-engine (CPU) and kt-gpu are executors of one and the same program
//! (invariant 7; precedent — the shared parametrization `pan_frame`/`cam_at`).
//! When you edit the construction or the CPU executor — edit the WGSL mirror
//! `geom_fused.wgsl` in kt-gpu IN THE SAME COMMIT (t181).
//!
//! Byte guarantee (R.44): a series of length 1 of Pan/CamMotion/Mirror goes
//! through the OLD kernels (`GeomExec::Legacy`) — byte-for-byte as before the fusion (t297);
//! a single EventGrid goes through the fused pass, but the arithmetic of the tile-local
//! phase reproduces the old formula verbatim — the golden t26 stands on this.
//! Multi-step series give NEW bytes (better: no losses) — this is
//! sanctioned by the owner (docs/user-feedback.md R.44).

use crate::conform::{ConformRule, LeadCtx, conform_dest};
use crate::video::{Frame, FxScratch};
use kt_model::FxInstance;

/// The ceiling of geo steps in one fused pass: a longer series is cut into
/// two passes at a step boundary (at the cut point the quality equals today’s).
/// Bounded by the capacity of the GPU pass uniform: worst case — 6 EventGrid =
/// 6 Fold (2 vec4 each) + 7 Affine = 13 primitives ≤ MAX_PRIMS.
pub const MAX_GEO_STEPS: usize = 6;
/// Ceiling of LOGICAL primitives of the program (kinds are packed 4 bits each into two
/// u32 headers of the uniform — at most 16).
pub const MAX_PRIMS: usize = 16;

/// The resolved 3D camera of the layer (R.29; since R.44 it lives here — Pan3d is computed
/// by both backends): the active/pinned camera with already interpolated
/// keyframes. It is computed at plan construction — the executors receive
/// ready parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cam3d {
    pub cam: kt_model::CameraParams,
    /// World scale: quality px / project px (the preview is smaller than the project,
    /// and the Pan3d/camera coordinates are in PROJECT px).
    pub world_scale: f64,
}

/// Whether it is a geometric effect (an invertible mapping “output pixel →
/// input entry point”). Pan3d — a pure 3×3 homography (a flat quad, z_clip=0),
/// since R.44 it is a geo step of the fused pass and is computed on BOTH backends.
pub fn is_geo(f: &FxInstance) -> bool {
    matches!(
        f,
        FxInstance::Geometry { .. }
            | FxInstance::CamMotion { .. }
            | FxInstance::Mirror { .. }
            | FxInstance::EventGrid { .. }
            | FxInstance::Pan3d { .. }
            | FxInstance::CornerPin { .. }
            | FxInstance::LensDistort { .. }
            | FxInstance::Twirl { .. }
            | FxInstance::Ripple { .. }
            | FxInstance::Kaleidoscope { .. }
            | FxInstance::Cylinder3d { .. }
    )
}

/// Whether the “Geometry” node IN CONTEXT is identity (R.62). An extra node is
/// identity under identity parameters; the LEADING one is also so only when
/// the conform frame equals the canvas: otherwise it must put the frame of foreign
/// proportions into the frame with transparent margins. Without a context the leading
/// node counts as “stretch” (frame = canvas).
pub fn geometry_is_identity_in(
    f: &FxInstance,
    lead: Option<LeadCtx>,
    out_w: u32,
    out_h: u32,
) -> bool {
    match f {
        FxInstance::Geometry { lead: is_lead, .. } => {
            if !f.geometry_is_identity() {
                return false;
            }
            if !*is_lead {
                return true;
            }
            let Some(l) = lead else { return true };
            let (mow, moh) = (out_w.max(2) as f64, out_h.max(2) as f64);
            let d = conform_dest(l.view, (mow, moh), l.rule);
            (d.0 - mow).abs() < 1e-6 && (d.1 - moh).abs() < 1e-6
        }
        _ => false,
    }
}

/// A geo step that ACTS: a geo effect, except the identity “Geometry” node —
/// that is transparent for segmentation, like audio FX; otherwise a lone
/// Pan/Mirror next to it would lose the old kernel (t297).
pub fn is_geo_active(f: &FxInstance, lead: Option<LeadCtx>, out_w: u32, out_h: u32) -> bool {
    is_geo(f) && !geometry_is_identity_in(f, lead, out_w, out_h)
}

/// Diagonal affine map of index-space: `x_in = ax·x + bx` (the linear part of all
/// our 2D geo FX is axis-aligned; there are no rotations).
#[derive(Debug, Clone, Copy)]
pub struct Aff {
    pub ax: f64,
    pub bx: f64,
    pub ay: f64,
    pub by: f64,
}

impl Aff {
    pub const IDENTITY: Aff = Aff { ax: 1.0, bx: 0.0, ay: 1.0, by: 0.0 };
    fn is_identity(&self) -> bool {
        self.ax == 1.0 && self.bx == 0.0 && self.ay == 1.0 && self.by == 0.0
    }
    /// Composition `m ∘ self` (first self, then m) — computed in f64 at
    /// construction; the GPU receives ALREADY multiplied coefficients (a single
    /// rounding to f32, the risk of f32-chain drift is removed).
    fn then(&self, m: &Aff) -> Aff {
        Aff {
            ax: m.ax * self.ax,
            bx: m.ax * self.bx + m.bx,
            ay: m.ay * self.ay,
            by: m.ay * self.by + m.by,
        }
    }
}

/// PLACEMENT of the “Geometry” node on the canvas (R.64): the frame, rotation and anchor
/// in canvas coordinates (logical output px) plus the crop window of the INPUT.
///
/// This is the ONLY placement arithmetic in the program: the geo program of the render builds its
/// affine/homography from it, and the interface draws the frame from it too. A second copy of the
/// formulas would silently diverge from the first — exactly the class of defects because of which the
/// rule of the slice in R.39 was reduced to
/// `clip_slice`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Top-left of the frame on the canvas (before rotation).
    pub ox: f64,
    pub oy: f64,
    /// Size of the frame on the canvas.
    pub dw: f64,
    pub dh: f64,
    /// Rotation around the anchor, degrees (as in the model).
    pub rot_deg: f64,
    /// Anchor point on the canvas — the center of rotation and scale.
    pub anchor: (f64, f64),
    /// Crop window in the INPUT frame: (x0, y0, w, h) in input px.
    pub crop_in: (f64, f64, f64, f64),
}

impl Placement {
    /// The four corners of the frame ON THE CANVAS, taking rotation into account: TL, TR, BR, BL.
    /// It is exactly these that the interface draws, and the anchor tests check against them:
    /// “the picture did not shift” is about the rotated quad, not about the
    /// axis-aligned rectangle.
    pub fn corners(&self) -> [(f64, f64); 4] {
        let (c, s) = (self.rot_deg.to_radians().cos(), self.rot_deg.to_radians().sin());
        let (ax, ay) = self.anchor;
        let raw = [
            (self.ox, self.oy),
            (self.ox + self.dw, self.oy),
            (self.ox + self.dw, self.oy + self.dh),
            (self.ox, self.oy + self.dh),
        ];
        raw.map(|(x, y)| {
            let (dx, dy) = (x - ax, y - ay);
            (ax + c * dx - s * dy, ay + s * dx + c * dy)
        })
    }

    /// Center of the frame on the canvas (coincides with the center of the rotated quad).
    pub fn center(&self) -> (f64, f64) {
        let cs = self.corners();
        ((cs[0].0 + cs[2].0) / 2.0, (cs[0].1 + cs[2].1) / 2.0)
    }
}

/// Placement of the “Geometry” node on the canvas. `None` — not a geometry node.
///
/// `in_size` — the logical size of the INPUT frame of the step (px), `canvas` —
/// the logical output size. The leading node takes the frame by the conform rule
/// from the VIEW of the source (the crop cuts the view BEFORE the rule), an ordinary one —
/// by the cropped input.
pub fn geometry_placement(
    f: &FxInstance,
    lead: Option<LeadCtx>,
    in_size: (f64, f64),
    canvas: (f64, f64),
) -> Option<Placement> {
    let FxInstance::Geometry {
        lead: is_lead,
        crop_l,
        crop_t,
        crop_r,
        crop_b,
        pos_x,
        pos_y,
        zoom,
        rot,
        anchor_x,
        anchor_y,
    } = f
    else {
        return None;
    };
    let (l, t, r, b) = (
        crop_l.clamp(0.0, 0.95),
        crop_t.clamp(0.0, 0.95),
        crop_r.clamp(0.0, 0.95),
        crop_b.clamp(0.0, 0.95),
    );
    let kx = (1.0 - l - r).max(0.05);
    let ky = (1.0 - t - b).max(0.05);
    let (iw, ih) = (in_size.0.max(1.0), in_size.1.max(1.0));
    let (cx0, cy0, cw, ch) = (l * iw, t * ih, kx * iw, ky * ih);
    let (fw_, fh_) = (canvas.0.max(2.0), canvas.1.max(2.0));
    let dest = if *is_lead {
        let lc = lead.unwrap_or(LeadCtx { view: None, rule: ConformRule::Stretch });
        conform_dest(lc.view.map(|(vw, vh)| (vw * kx, vh * ky)), (fw_, fh_), lc.rule)
    } else {
        (cw, ch)
    };
    let z = zoom.clamp(0.05, 32.0);
    let (dw, dh) = ((dest.0 * z).max(1e-6), (dest.1 * z).max(1e-6));
    let ccx = fw_ / 2.0 + pos_x.clamp(-2.0, 2.0) * fw_;
    let ccy = fh_ / 2.0 + pos_y.clamp(-2.0, 2.0) * fh_;
    let (ox, oy) = (ccx - dw / 2.0, ccy - dh / 2.0);
    Some(Placement {
        ox,
        oy,
        dw,
        dh,
        rot_deg: *rot,
        anchor: (ox + anchor_x.clamp(0.0, 1.0) * dw, oy + anchor_y.clamp(0.0, 1.0) * dh),
        crop_in: (cx0, cy0, cw, ch),
    })
}

/// Handle of the transform frame (R.64). Corners and sides — scale, `Rot` —
/// rotation, `Anchor` — the anchor, `Crop*` — the crop of its own side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Rot,
    Anchor,
    CropLeft,
    CropTop,
    CropRight,
    CropBottom,
}

impl Handle {
    /// The handle point ON THE CANVAS (with rotation taken into account) — both the drawing and the hit test
    /// take it from here; no second list of coordinates is kept.
    /// The OUTER NORMAL of the handle — a unit vector “away from the frame” (R.66).
    ///
    /// Needed by a small frame: when its side on screen is smaller than three grab zones,
    /// the eight scale handles cover it entirely and there is nothing to grab for the body —
    /// the handles are moved outward along these directions. It is computed HERE, next to
    /// the frame itself, because the direction depends on its rotation; the OFFSET itself
    /// lives in the interface — it is measured in screen pixels, and kt-fx does not know
    /// the preview scale.
    ///
    /// The rotation, anchor and crop handles have no normal: they do not move from their place.
    pub fn outward(self, pl: &Placement) -> Option<(f64, f64)> {
        let c = pl.center();
        let p = self.point(pl);
        let (dx, dy) = (p.0 - c.0, p.1 - c.1);
        let n = (dx * dx + dy * dy).sqrt();
        match self {
            Handle::TopLeft
            | Handle::Top
            | Handle::TopRight
            | Handle::Right
            | Handle::BottomRight
            | Handle::Bottom
            | Handle::BottomLeft
            | Handle::Left
                if n > 1e-9 =>
            {
                Some((dx / n, dy / n))
            }
            _ => None,
        }
    }

    pub fn point(self, pl: &Placement) -> (f64, f64) {
        let cs = pl.corners();
        let mid = |a: (f64, f64), b: (f64, f64)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        match self {
            Handle::TopLeft => cs[0],
            Handle::Top | Handle::CropTop => mid(cs[0], cs[1]),
            Handle::TopRight => cs[1],
            Handle::Right | Handle::CropRight => mid(cs[1], cs[2]),
            Handle::BottomRight => cs[2],
            Handle::Bottom | Handle::CropBottom => mid(cs[2], cs[3]),
            Handle::BottomLeft => cs[3],
            Handle::Left | Handle::CropLeft => mid(cs[3], cs[0]),
            Handle::Anchor => pl.anchor,
            // The rotation handle is INSIDE the frame, under the midpoint of the top side.
            //
            // Outside (as in most editors) it sticks out beyond the picture:
            // under the “fit” rule the frame takes the whole height of the canvas, and the
            // handle ended up OUTSIDE the preview — it was impossible to hit it
            // (caught by t760b). Inside — always visible; the arm is shortened
            // on a small frame so that it does not meet the anchor in the center.
            Handle::Rot => {
                let top = mid(cs[0], cs[1]);
                let (cx, cy) = pl.center();
                let (dx, dy) = (cx - top.0, cy - top.1);
                let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                let arm = ROT_ARM.min(pl.dw.min(pl.dh) * 0.3);
                (top.0 + dx / len * arm, top.1 + dy / len * arm)
            }
        }
    }

    /// The opposite point — the one that STAYS IN PLACE when dragging this one.
    fn opposite(self, pl: &Placement) -> Option<(f64, f64)> {
        let cs = pl.corners();
        let mid = |a: (f64, f64), b: (f64, f64)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        Some(match self {
            Handle::TopLeft => cs[2],
            Handle::TopRight => cs[3],
            Handle::BottomRight => cs[0],
            Handle::BottomLeft => cs[1],
            Handle::Top => mid(cs[2], cs[3]),
            Handle::Bottom => mid(cs[0], cs[1]),
            Handle::Left => mid(cs[1], cs[2]),
            Handle::Right => mid(cs[3], cs[0]),
            _ => return None,
        })
    }
}

/// Length of the arm of the rotation handle in CANVAS px (drawing and hit test —
/// one number).
pub const ROT_ARM: f64 = 28.0;

/// Position of the node after moving the frame by `(dx, dy)` canvas px.
pub fn pos_for_drag(pos: (f64, f64), canvas: (f64, f64), d: (f64, f64)) -> (f64, f64) {
    let (fw, fh) = (canvas.0.max(2.0), canvas.1.max(2.0));
    ((pos.0 + d.0 / fw).clamp(-2.0, 2.0), (pos.1 + d.1 / fh).clamp(-2.0, 2.0))
}

/// Zoom and position when dragging a scale handle to the point `p` of the canvas: the OPPOSITE
/// point of the frame stays in place (the industry rule), the zoom
/// is uniform — the node has one zoom for both axes.
pub fn zoom_for_handle(
    pl: &Placement,
    h: Handle,
    p: (f64, f64),
    zoom: f64,
    pos: (f64, f64),
    canvas: (f64, f64),
) -> Option<(f64, (f64, f64))> {
    let fixed = h.opposite(pl)?;
    let from = h.point(pl);
    let (d0x, d0y) = (from.0 - fixed.0, from.1 - fixed.1);
    let (d1x, d1y) = (p.0 - fixed.0, p.1 - fixed.1);
    let len0 = (d0x * d0x + d0y * d0y).sqrt();
    if len0 < 1e-6 {
        return None;
    }
    // projection onto the original direction: dragging sideways does not flip the frame
    let k = ((d1x * d0x + d1y * d0y) / (len0 * len0)).max(0.01);
    let z = (zoom * k).clamp(0.05, 32.0);
    // the real coefficient after the clamp — the shift is computed from it
    let kk = z / zoom.max(1e-9);
    let (cx, cy) = pl.center();
    let nc = (fixed.0 + (cx - fixed.0) * kk, fixed.1 + (cy - fixed.1) * kk);
    Some((z, pos_for_drag(pos, canvas, (nc.0 - cx, nc.1 - cy))))
}

/// The angle of the node at which the rotation handle points to the point `p` of the canvas.
/// The step `step_deg` (Shift) — rounding to the nearest multiple.
pub fn rot_for_point(pl: &Placement, p: (f64, f64), step_deg: Option<f64>) -> f64 {
    let (ax, ay) = pl.anchor;
    // zero angle of the handle — “up” from the anchor in the NON-rotated frame
    let base = (-std::f64::consts::FRAC_PI_2).to_degrees();
    let cur = (p.1 - ay).atan2(p.0 - ax).to_degrees();
    let mut deg = cur - base;
    if let Some(st) = step_deg.filter(|s| *s > 0.0) {
        deg = (deg / st).round() * st;
    }
    // keep within (-360, 360], as the parameter passport does
    deg %= 360.0;
    deg.clamp(-360.0, 360.0)
}

/// Crop after dragging a crop handle to the point `p` of the canvas. Returns
/// `[l, t, r, b]` as fractions of the ORIGINAL view.
pub fn crop_for_handle(
    pl: &Placement,
    h: Handle,
    p: (f64, f64),
    crop: [f64; 4],
) -> Option<[f64; 4]> {
    // transform the point into the system of the NON-rotated frame
    let (c, s) = ((-pl.rot_deg).to_radians().cos(), (-pl.rot_deg).to_radians().sin());
    let (ax, ay) = pl.anchor;
    let (dx, dy) = (p.0 - ax, p.1 - ay);
    let (lx, ly) = (ax + c * dx - s * dy, ay + s * dx + c * dy);
    // fraction along the side of the frame → fraction of the VIEW: the frame already shows the
    // cropped view, so the fraction step is divided by the remaining part
    let (u, v) = ((lx - pl.ox) / pl.dw.max(1e-6), (ly - pl.oy) / pl.dh.max(1e-6));
    let (kx, ky) = ((1.0 - crop[0] - crop[2]).max(0.05), (1.0 - crop[1] - crop[3]).max(0.05));
    let mut out = crop;
    match h {
        Handle::CropLeft => out[0] = (crop[0] + u * kx).clamp(0.0, 0.95),
        Handle::CropRight => out[2] = (crop[2] + (1.0 - u) * kx).clamp(0.0, 0.95),
        Handle::CropTop => out[1] = (crop[1] + v * ky).clamp(0.0, 0.95),
        Handle::CropBottom => out[3] = (crop[3] + (1.0 - v) * ky).clamp(0.0, 0.95),
        _ => return None,
    }
    // mutual limit: the total crop of an axis does not eat the view entirely
    if out[0] + out[2] > 0.95 || out[1] + out[3] > 0.95 {
        return None;
    }
    Some(out)
}

/// ANCHOR to the point `p` of the canvas + POSITION COMPENSATION: the rotated quad
/// stays in place.
///
/// The compensation is derived from the same placement, not from a separate formula:
/// the rotation goes around the anchor, so changing the anchor rotates the picture around
/// another point, and the position must be shifted by `R⁻¹(I − R)(A − A′)`. In
/// After Effects the compensation on a rotated layer is wrong — the guard checks the
/// CORNERS of the quad, not the parameter values.
pub fn anchor_move(
    pl: &Placement,
    p: (f64, f64),
    pos: (f64, f64),
    canvas: (f64, f64),
) -> ((f64, f64), (f64, f64)) {
    // point in the NON-rotated frame’s system → fractions of the frame
    let (c, s) = ((-pl.rot_deg).to_radians().cos(), (-pl.rot_deg).to_radians().sin());
    let (ax, ay) = pl.anchor;
    let (dx, dy) = (p.0 - ax, p.1 - ay);
    let (lx, ly) = (ax + c * dx - s * dy, ay + s * dx + c * dy);
    let na = (
        ((lx - pl.ox) / pl.dw.max(1e-6)).clamp(0.0, 1.0),
        ((ly - pl.oy) / pl.dh.max(1e-6)).clamp(0.0, 1.0),
    );
    // new anchor point at the OLD position
    let a2 = (pl.ox + na.0 * pl.dw, pl.oy + na.1 * pl.dh);
    let (ex, ey) = (ax - a2.0, ay - a2.1);
    // The display of a layer point — `A + R(q − A)`. Shifting the anchor by `E = A′ − A` and
    // the position by `Δ`, we get `A + E + Δ + R(q − A − E)`; equality with
    // the previous display gives `Δ = (R − I)·E`, that is exactly `(I − R)(A − A′)`
    // — without a second rotation. An extra `R⁻¹` here would be exactly the
    // “analytic compensation that goes wrong on rotation”.
    let (cr, sr) = (pl.rot_deg.to_radians().cos(), pl.rot_deg.to_radians().sin());
    let (ddx, ddy) = (ex - (cr * ex - sr * ey), ey - (sr * ex + cr * ey));
    (na, pos_for_drag(pos, canvas, (ddx, ddy)))
}

/// A primitive of the backward pass program. The order of primitives = the order of
/// application (from the LAST effect of the series to the first); after the last
/// primitive the coordinate is in the index-space of the series’ input frame.
#[derive(Debug, Clone, Copy)]
pub enum GeomPrim {
    /// Composition of inverse affine steps (Pan/CamMotion/Mirror and
    /// pre/post-maps of neighbors).
    Affine(Aff),
    /// Positional clamp to the rectangle of the step’s virtual input —
    /// the SEMANTIC “plateau” of Pan at zoom<1/Stretch (the frame wider than the input).
    /// It is NOT applied to a fitting frame: an overshoot ≤ 0.5 px at the edge is an artifact of
    /// step-wise rasterization, not semantics. A disabled axis carries
    /// the sentinel ±BIG.
    Clamp { x0: f64, x1: f64, y0: f64, y1: f64 },
    /// Tile modulo of EventGrid: the input is the “d” coordinate (center-space minus
    /// the grid offset); outside [0,gw)×[0,gh) — opaque black (a short
    /// circuit), inside — the tile-local phase and the exact map into the index-space
    /// of the step’s input: `(r / cell) * in − 0.5` — VERBATIM the arithmetic of the old
    /// event_grid; the byte identity of the single grid (t26) rests on it.
    Fold { gw: f64, gh: f64, cw: f64, ch: f64, in_w: f64, in_h: f64 },
    /// Inverse homography of Pan3d: the input is the CENTER-space of the step output
    /// (the pre-affine +0.5 is composed by the builder), `t = m·(c,1)`; `t.z ≤ 0` —
    /// point behind the camera (rasterizer clipping) → transparent; `q = t.xy/t.z`
    /// outside [0,1]² — outside the quad → transparent; otherwise the output is q (uv),
    /// the post-affine `q·in − 0.5` is composed by the builder (verbatim the old pan3d.wgsl).
    Proj { m: [[f64; 3]; 3] },
    /// Frame bounds (R.62, the “Geometry” node): a coordinate outside
    /// [x0,x1]×[y0,y1] index-space is TRANSPARENT (short circuit).
    /// Unlike Clamp (plateau), it does not invent pixels: the conform
    /// margins must be transparent (lesson from Pitivi #1378).
    Bounds { x0: f64, x1: f64, y0: f64, y1: f64 },
    /// Nonlinear warp (R.47 Kv4): input and output are the NORMALIZED
    /// centered space of the stage (pre/post-affines built by the
    /// builder; r=1 is the short semi-axis, cylinder is normalized by X on
    /// w/2). mode: 0 lens (polynomial), 1 twirl, 2 ripple, 3 kaleidoscope,
    /// 4 cylinder; precomputable values (radians, 2π·freq, sin a, 1/a) — f64
    /// on CPU, into the uniform — once.
    Warp { mode: u32, p: [f64; 7] },
}

/// Sentinel for the disabled Clamp axis (fits f32 uniforms without inf).
pub const CLAMP_OFF: f64 = 1.0e30;

/// Ceiling on per-pixel tap ops in one program (table in the GPU uniform:
/// 8 × 2 vec4 slots behind the prims).
pub const MAX_TAP_OPS: usize = 8;
/// Ceiling on vec4 slots of prims (the tap table lives behind them in the uniform).
pub const MAX_PRIM_SLOTS: usize = 46;

/// Whether the effect is per-pixel (pixel color without neighbours) — candidate for
/// absorption into the fused pass (R.45, item 312). Only INTERNAL ones
/// (between geo steps of the run) are absorbed: leading/trailing ones stay
/// barriers — at K=1 their bytes are indistinguishable from the old ones.
pub fn is_pointwise(f: &FxInstance) -> bool {
    matches!(
        f,
        FxInstance::Greenscreen { .. }
            | FxInstance::Noise { .. }
            | FxInstance::ColorGrade { .. }
            | FxInstance::Duotone { .. }
            | FxInstance::Posterize { .. }
            | FxInstance::Vignette { .. }
    )
}

/// Per-pixel op applied to the VALUE OF THE SUBTAP (after bilinear, before
/// the box average), in chain order (R.45, item 312).
#[derive(Debug, Clone, Copy)]
pub enum TapKind {
    /// Greenscreen: f64/f32 continuum of the integer kernel formula.
    Gs { key: [f64; 3], tol: f64, hard: f64, spill: f64, kc: usize },
    /// Noise: grain is pinned to the grid of ITS OWN step (decision R.45) — the offset
    /// is computed from floor of the stage coordinate; `special` is the special formula
    /// for the user `size == 1 && !color` (it has a different grain).
    Noise { seed: u32, k: i64, cell: u32, color: bool, special: bool, stage_w: u32, stage_h: u32 },
    /// ColorGrade (R.47): cos/sin of the hue are precomputed in f64 —
    /// the GPU receives them as a uniform with a single rounding.
    Grade { brightness: f64, contrast: f64, saturation: f64, hue_cos: f64, hue_sin: f64 },
    /// Duotone (R.47): lerp by luma between two colors.
    Duo { shadow: [f64; 3], high: [f64; 3], amount: f64 },
    /// Posterize (R.47): quantization per channel.
    Poster { levels: u32 },
    /// Vignette (R.47): coordinate of ITS OWN stage (like Noise) — the darkening
    /// zooms with the picture inside the chain.
    Vig { strength: f64, radius: f64, softness: f64, stage_w: u32, stage_h: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct TapOp {
    /// The op coordinate is recorded after applying this number of prims of the
    /// backward pass (the builder splits affine joins at op stages). With a short circuit
    /// at prim d, the op with after_prim ≤ d stays alive.
    pub after_prim: usize,
    pub kind: TapKind,
}

/// Ceiling of supersampling taps per axis (maximum 4×4 = 16 subtaps).
pub const SS_MAX: u32 = 4;

/// Supersampling by Jacobian (R.44, item 309): how many subtaps per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsSpec {
    /// Affine program (incl. with Fold): the Jacobian is CONSTANT over the frame —
    /// the tap count is computed once in f64 at build time and goes into the
    /// uniform. The CPU/GPU parity risk is zero by construction.
    Const { nx: u32, ny: u32 },
    /// The program has Proj (perspective): the Jacobian changes over the frame — both
    /// executors measure it by finite differences (two coordinate probes per pixel) and
    /// compute the taps with ONE formula `ss_taps`.
    PerPixel,
}

/// Number of subtaps per axis from the length of the image of a unit output step
/// in source space. The 1/32 px quantum suppresses float noise on identity:
/// supersampling is enabled strictly when len > 1 + 1/64 (len == 1.0 —
/// exactly one tap, the old path byte-for-byte; guarded by t26
/// pan-stretch-identity and t300).
pub fn ss_taps(len: f64) -> u32 {
    let q = (len * 32.0 + 0.5).floor() / 32.0;
    (q.ceil() as u32).clamp(1, SS_MAX)
}

/// Fused program of the geometry series.
#[derive(Debug, Clone)]
pub struct GeomProgram {
    pub prims: Vec<GeomPrim>,
    /// Size of the input frame of the series (for the final tap).
    pub in_w: u32,
    pub in_h: u32,
    /// Output size (= the plan output size, max(2) as in the old kernels).
    pub out_w: u32,
    pub out_h: u32,
    /// Convolution of the ALPHA RULES over the steps of the series (R.53, item 417).
    ///
    /// Each step relates to alpha in one of three ways:
    /// - **Produces** — can create transparency where there was none
    ///   (Pan3d outside the quad, Greenscreen by key) ⇒ `known=true, alpha=true`;
    /// - **Destroys** — guarantees an opaque output ⇒ `known=true,
    ///   alpha=false`;
    /// - **Preserves** — the input alpha passes straight through (all geometry,
    ///   including EventGrid) ⇒ the convolution is NOT touched.
    ///
    /// The convolution is “the last declarer wins” AMONG Produces and
    /// Destroys, and that is exactly why Preserves must stay silent: a step that silently
    /// declared itself Destroys would erase the Produces of a neighbour higher up the chain.
    /// `known=false` means “nobody has spoken” — the alpha of the
    /// SOURCE is taken.
    pub has_alpha_known: bool,
    pub has_alpha: bool,
    /// Supersampling under minification (item 309).
    pub ss: SsSpec,
    /// Absorbed per-pixel ops (R.45, item 312) in chain order;
    /// their after_prim go in DESCENDING order (the early one in the chain is deeper in
    /// the backward pass).
    pub tap_ops: Vec<TapOp>,
    /// Absolute length of the image of a unit output step along the axes (the scheduler's
    /// magnifier for K× barriers, item 314): for affine programs — exact
    /// (the same as the ss taps); for Proj — estimated by probes (center + corners).
    pub lens: (f64, f64),
}

impl GeomProgram {
    /// vec4 prim slots in the GPU uniform (limit MAX_PRIM_SLOTS; the scheduler checks
    /// the budget — when exceeded, the run is cut at a geo boundary).
    pub fn prim_slots(&self) -> usize {
        self.prims
            .iter()
            .map(|p| match p {
                GeomPrim::Affine(_) | GeomPrim::Clamp { .. } | GeomPrim::Bounds { .. } => 1,
                GeomPrim::Fold { .. } => 2,
                GeomPrim::Proj { .. } => 3,
                GeomPrim::Warp { .. } => 2,
            })
            .sum()
    }
}

/// Execution decision of the geo run. Made HERE and only here — both backends must
/// ask this function rather than decide themselves.
pub enum GeomExec {
    /// Series of length 1 of Pan/CamMotion/Mirror: the old kernels (CPU: pan_zoom /
    /// mirror; GPU: Resample / Mirror) — bytes as before R.44.
    Legacy,
    /// Fused pass of the inverse mappings.
    Fused(GeomProgram),
}

/// Build the solution for the geo steps of the run (`steps` — ONLY geo, in application
/// order; transparent audio has already been discarded by the collector).
#[allow(clippy::too_many_arguments)]
pub fn build_geom_exec(
    steps: &[&FxInstance],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
    burst_index: i64,
    k_in: u32,
    k_out: u32,
) -> GeomExec {
    build_geom_exec_in(steps, in_w, in_h, out_w, out_h, u, cam3d, burst_index, k_in, k_out, None)
}

/// The same with the context of the leading “Geometry” node (R.62): the source view and the
/// conform rule. Product calls go through the scheduler.
#[allow(clippy::too_many_arguments)]
pub fn build_geom_exec_in(
    steps: &[&FxInstance],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
    burst_index: i64,
    k_in: u32,
    k_out: u32,
    lead: Option<LeadCtx>,
) -> GeomExec {
    debug_assert!(!steps.is_empty());
    if steps.len() == 1 {
        // a single resample mirror does not need it — the old kernel (in-place
        // permutation is correct on any UNIFORM physical scale).
        // A change of scale (a run between K× segments, S× input for R.46) requires a
        // resample — then the fused pass; otherwise the plan would assemble Legacy on
        // K≠1 edges (latent defect of R.45, test t308b).
        if k_in == k_out && matches!(steps[0], FxInstance::Mirror { .. }) {
            return GeomExec::Legacy;
        }
        // a single CamMotion with NO minification AND NO K× edges — the old
        // kernel byte-for-byte; otherwise the fused pass (supersampling item 309 /
        // conjugation item 314; legacy pan_zoom can only do 1× output). The “Geometry” node
        // (R.62) is always fused: it has transparent margins.
        if k_in == 1 && k_out == 1 && matches!(steps[0], FxInstance::CamMotion { .. }) {
            let prog =
                build_program(steps, in_w, in_h, out_w, out_h, u, cam3d, burst_index, 1, 1, lead);
            if prog.ss == (SsSpec::Const { nx: 1, ny: 1 }) {
                return GeomExec::Legacy;
            }
            return GeomExec::Fused(prog);
        }
    }
    GeomExec::Fused(build_program(
        steps,
        in_w,
        in_h,
        out_w,
        out_h,
        u,
        cam3d,
        burst_index,
        k_in,
        k_out,
        lead,
    ))
}

/// Direct parameters of one step (assembled by a forward pass — the sizes of each step
/// depend on the previous ones).
enum Step {
    /// Pan/CamMotion: the pan_frame frame in the input coordinates of the step.
    Frame {
        x0: f64,
        y0: f64,
        cw: f64,
        ch: f64,
        iw: u32,
        ih: u32,
    },
    Mirror {
        h: bool,
        v: bool,
        iw: u32,
        ih: u32,
    },
    Grid {
        offx: u32,
        offy: u32,
        gw: u32,
        gh: u32,
        cw: u32,
        ch: u32,
        iw: u32,
        ih: u32,
    },
    /// Pan3d: INVERSE homography (center-space of the output → quad uv).
    Proj {
        m: [[f64; 3]; 3],
        iw: u32,
        ih: u32,
    },
    /// “Geometry” node (R.62): crop of the source → frame on the canvas with
    /// zoom, position and rotation around the anchor; outside the frame — transparent.
    /// Without rotation — a diagonal affine, with rotation — a Proj-affine
    /// (last row (0,0,1)); `bounds` — the crop bounds in the index-space of
    /// the input, `None` — the frame covers the whole canvas (no margins).
    Place {
        aff: Aff,
        m: [[f64; 3]; 3],
        rot0: bool,
        bounds: Option<(f64, f64, f64, f64)>,
        iw: u32,
        ih: u32,
    },
    /// R.47 nonlinear warp: does not change the raster, works in the normalized
    /// centered space of its own stage.
    Warp {
        mode: u32,
        p: [f64; 7],
        iw: u32,
        ih: u32,
    },
    /// Absorbed per-pixel op (R.45): applied to the subtap value
    /// at the coordinate of ITS OWN stage.
    Tap(TapKind),
}

#[allow(clippy::too_many_arguments)]
fn build_program(
    steps: &[&FxInstance],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
    burst_index: i64,
    k_in: u32,
    k_out: u32,
    lead: Option<LeadCtx>,
) -> GeomProgram {
    let (mow, moh) = (out_w.max(2), out_h.max(2));
    // Conjugation K× (R.45, item 314): ALL the arithmetic of the steps lives in
    // 1× semantics (pan_frame, the integer EventGrid grid, pan3d_forward — untouched);
    // the physical scales are added by two affines at the edges, merged into the
    // existing compositions. At K=1 both are the identity (multiplication by 1.0 and
    // addition of 0.0 are exact in f64 — the R.44 bytes are untouched).
    // in_w/in_h arrive as PHYSICAL (k_in×logic).
    let (k_in, k_out) = (k_in.max(1), k_out.max(1));
    debug_assert!(
        in_w.is_multiple_of(k_in) && in_h.is_multiple_of(k_in),
        "physical input is k_in x logical"
    );
    let (lin_w, lin_h) = (in_w / k_in, in_h / k_in);
    // --- forward: LOGICAL sizes and direct parameters of each step ---
    let (mut w, mut h) = (lin_w, lin_h);
    let mut fw: Vec<Step> = Vec::with_capacity(steps.len());
    let mut has_alpha_known = false;
    let mut has_alpha = false;
    for f in steps {
        match f {
            FxInstance::CamMotion { mode, ease, time_scale, keyframes } => {
                let (px, py, z) = crate::video::cam_at(keyframes, u, *ease, *time_scale);
                let (x0, y0, cw, ch) =
                    crate::video::pan_frame(w, h, out_w, out_h, *mode, px, py, z);
                fw.push(Step::Frame { x0, y0, cw, ch, iw: w, ih: h });
                w = mow;
                h = moh;
            }
            // ALL NUMBERS OF THE NODE are parsed by `geometry_placement` — here
            // only the rotation remains (both branches of the map need it)
            FxInstance::Geometry { rot, .. } => {
                // identity node — transparent (the scheduler does not pass it here;
                // here it is for direct calls and tests)
                if geometry_is_identity_in(f, lead, out_w, out_h) {
                    continue;
                }
                // PLACEMENT IS COMPUTED BY ONE FUNCTION (R.64): the same one by which the interface
                // draws the frame over the picture. There are no separate frame formulas
                // here anymore — they would silently diverge.
                let (iw, ih) = (w as f64, h as f64);
                let (fw_, fh_) = (mow as f64, moh as f64);
                let pl = match geometry_placement(f, lead, (iw, ih), (fw_, fh_)) {
                    Some(p) => p,
                    None => continue,
                };
                let (cx0, cy0, cw, ch) = pl.crop_in;
                let (ox, oy, dw, dh) = (pl.ox, pl.oy, pl.dw, pl.dh);
                let (axp, ayp) = pl.anchor;
                let rot0 = *rot == 0.0;
                let covers = rot0 && ox <= 0.0 && oy <= 0.0 && ox + dw >= fw_ && oy + dh >= fh_;
                let bounds =
                    (!covers).then_some((cx0 - 0.5, cx0 + cw - 0.5, cy0 - 0.5, cy0 + ch - 0.5));
                // without rotation: output index → input index (x_in = ax·x + bx)
                let (sx, sy) = (cw / dw, ch / dh);
                let aff = Aff {
                    ax: sx,
                    bx: cx0 - ox * sx + 0.5 * sx - 0.5,
                    ay: sy,
                    by: cy0 - oy * sy + 0.5 * sy - 0.5,
                };
                // with rotation: output center-space p′ → uv of the full input,
                // p = A + R(−rot)(p′ − A), q = (cx0 + (p − o)·c/d) / in
                let (c, sn) = (rot.to_radians().cos(), rot.to_radians().sin());
                let (kx_, ky_) = (cw / (dw * iw), ch / (dh * ih));
                let m = [
                    [c * kx_, sn * kx_, (axp - c * axp - sn * ayp - ox) * kx_ + cx0 / iw],
                    [-sn * ky_, c * ky_, (ayp + sn * axp - c * ayp - oy) * ky_ + cy0 / ih],
                    [0.0, 0.0, 1.0],
                ];
                fw.push(Step::Place { aff, m, rot0, bounds, iw: w, ih: h });
                w = mow;
                h = moh;
                if bounds.is_some() {
                    // frame margins are transparent — the node PRODUCES alpha
                    has_alpha_known = true;
                    has_alpha = true;
                }
            }
            FxInstance::Mirror { horizontal, vertical } => {
                if *horizontal || *vertical {
                    fw.push(Step::Mirror { h: *horizontal, v: *vertical, iw: w, ih: h });
                }
            }
            FxInstance::EventGrid { cols, rows, center } => {
                let cols = (*cols).clamp(1, 512);
                let rows = (*rows).clamp(1, 512);
                let (cw, ch) = ((mow / cols).max(1), (moh / rows).max(1));
                let (gw, gh) = (cw * cols, ch * rows);
                // guard “grid larger than the frame” — shared with the kernels (item 306)
                let (offx, offy) = if *center && gw <= mow && gh <= moh {
                    ((mow - gw) / 2, (moh - gh) / 2)
                } else {
                    (0, 0)
                };
                fw.push(Step::Grid { offx, offy, gw, gh, cw, ch, iw: w, ih: h });
                w = mow;
                h = moh;
                // ALPHA RULE (R.53, item 417): the grid PASSES alpha through.
                // Here it used to say “there is no alpha anymore”, and that was untrue:
                // the tile is a resampled input, the input alpha still lives
                // in it; black (`Mapped::Black`) is filled only in the field
                // OUTSIDE the grid. Because of the false declaration, a title
                // or a greenscreen output under EventGrid landed in the composite as an
                // opaque rectangle.
            }
            FxInstance::Pan3d {
                pos_x,
                pos_y,
                pos_z,
                rot_x,
                rot_y,
                rot_z,
                anchor_x,
                anchor_y,
                scale,
                camera: _, // the camera is already resolved by the engine in cam3d
            } => {
                // no camera — no 3D: the step is skipped entirely, sizes
                // do not change (mirror of the former `continue` in chain_ops)
                let Some(c3) = cam3d else { continue };
                let g = pan3d_forward(
                    &c3,
                    (mow, moh),
                    [*pos_x, *pos_y, *pos_z],
                    [*rot_x, *rot_y, *rot_z],
                    *scale,
                    [*anchor_x, *anchor_y],
                    (w as f64, h as f64),
                );
                // degenerate homography (|det| ≈ 0, quad edge-on) —
                // transparent layer: m with t.z ≡ −1
                let m = invert3(&g).unwrap_or([[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]]);
                fw.push(Step::Proj { m, iw: w, ih: h });
                w = mow;
                h = moh;
                // outside the quad the layer is transparent
                has_alpha_known = true;
                has_alpha = true;
            }
            FxInstance::Greenscreen { key_r, key_g, key_b, tolerance, softness, spill } => {
                let tol = (*tolerance).clamp(1, 200) as f64;
                let soft = (*softness).clamp(0, 100) as f64;
                let key = [*key_r, *key_g, *key_b];
                fw.push(Step::Tap(TapKind::Gs {
                    key: [key[0] as f64, key[1] as f64, key[2] as f64],
                    tol,
                    hard: (tol - soft).max(0.0),
                    spill: spill.clamp(0.0, 1.0),
                    kc: (0..3).max_by_key(|&i| key[i]).unwrap_or(1),
                }));
                has_alpha_known = true;
                has_alpha = true;
            }
            FxInstance::Noise { amount, size, color } => {
                let k = (amount.clamp(0.0, 1.0) * 32.0) as i64;
                // k == 0 — the effect is empty (like the kernel's early exit and the GPU branch)
                if k > 0 {
                    let size = (*size).clamp(1, 64);
                    fw.push(Step::Tap(TapKind::Noise {
                        seed: crate::video::noise_seed(burst_index),
                        k,
                        cell: size,
                        color: *color,
                        special: size == 1 && !*color,
                        stage_w: w,
                        stage_h: h,
                    }));
                }
            }
            FxInstance::ColorGrade { brightness, contrast, saturation, hue } => {
                let h = (*hue as f64).to_radians();
                fw.push(Step::Tap(TapKind::Grade {
                    brightness: *brightness as f64,
                    contrast: *contrast as f64,
                    saturation: *saturation as f64,
                    hue_cos: h.cos(),
                    hue_sin: h.sin(),
                }));
            }
            FxInstance::Duotone {
                shadow_r,
                shadow_g,
                shadow_b,
                high_r,
                high_g,
                high_b,
                amount,
            } => {
                fw.push(Step::Tap(TapKind::Duo {
                    shadow: [*shadow_r as f64, *shadow_g as f64, *shadow_b as f64],
                    high: [*high_r as f64, *high_g as f64, *high_b as f64],
                    amount: (*amount as f64).clamp(0.0, 1.0),
                }));
            }
            FxInstance::Posterize { levels } => {
                fw.push(Step::Tap(TapKind::Poster { levels: (*levels).clamp(2, 32) }));
            }
            FxInstance::Vignette { strength, radius, softness } => {
                fw.push(Step::Tap(TapKind::Vig {
                    strength: (*strength as f64).clamp(0.0, 1.0),
                    radius: (*radius as f64).clamp(0.0, 1.5),
                    softness: (*softness as f64).clamp(0.05, 1.0),
                    stage_w: w,
                    stage_h: h,
                }));
            }
            FxInstance::CornerPin { x0, y0, x1, y1, x2, y2, x3, y3 } => {
                // default is bit-exact: corners at the corners — the step is skipped,
                // the chain stays identity (identity byte-for-byte)
                let ident = *x0 == 0.0
                    && *y0 == 0.0
                    && *x1 == 1.0
                    && *y1 == 0.0
                    && *x2 == 0.0
                    && *y2 == 1.0
                    && *x3 == 1.0
                    && *y3 == 1.0;
                if !ident {
                    let m = corner_pin_inverse(
                        [(*x0, *y0), (*x1, *y1), (*x2, *y2), (*x3, *y3)],
                        mow,
                        moh,
                    );
                    fw.push(Step::Proj { m, iw: w, ih: h });
                    // outside the quad — transparent (as in Pan3d)
                    has_alpha_known = true;
                    has_alpha = true;
                    w = mow;
                    h = moh;
                }
            }
            FxInstance::LensDistort { k1, k2 } => {
                let (k1, k2) = ((*k1 as f64).clamp(-1.0, 1.0), (*k2 as f64).clamp(-1.0, 1.0));
                if k1 != 0.0 || k2 != 0.0 {
                    fw.push(Step::Warp {
                        mode: 0,
                        p: [k1, k2, 0.0, 0.0, 0.0, 0.0, 0.0],
                        iw: w,
                        ih: h,
                    });
                }
            }
            FxInstance::Twirl { angle, radius } => {
                let a = (*angle as f64).clamp(-720.0, 720.0).to_radians();
                let rad = (*radius as f64).clamp(0.1, 1.5);
                if a != 0.0 {
                    fw.push(Step::Warp {
                        mode: 1,
                        p: [a, rad, 0.0, 0.0, 0.0, 0.0, 0.0],
                        iw: w,
                        ih: h,
                    });
                }
            }
            FxInstance::Ripple { amount, freq, phase } => {
                let amp = (*amount as f64).clamp(0.0, 0.5);
                let k = 2.0 * std::f64::consts::PI * (*freq as f64).clamp(1.0, 40.0);
                let ph = (*phase as f64).to_radians();
                if amp != 0.0 {
                    fw.push(Step::Warp {
                        mode: 2,
                        p: [amp, k, ph, 0.0, 0.0, 0.0, 0.0],
                        iw: w,
                        ih: h,
                    });
                }
            }
            FxInstance::Kaleidoscope { segments, angle } => {
                let tau = 2.0 * std::f64::consts::PI / (*segments).clamp(2, 32) as f64;
                let a0 = (*angle as f64).clamp(-180.0, 180.0).to_radians();
                fw.push(Step::Warp {
                    mode: 3,
                    p: [tau, a0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    iw: w,
                    ih: h,
                });
            }
            FxInstance::Cylinder3d { curvature } => {
                let curv = (*curvature as f64).clamp(0.0, 1.0);
                if curv >= 1e-3 {
                    let a = curv * std::f64::consts::FRAC_PI_2 * 0.999;
                    fw.push(Step::Warp {
                        mode: 4,
                        p: [a.sin(), 1.0 / a, 0.0, 0.0, 0.0, 0.0, 0.0],
                        iw: w,
                        ih: h,
                    });
                }
            }
            _ => debug_assert!(false, "non-geo step in build_program"),
        }
    }
    // --- backward: inverse maps, adjacent affines MULTIPLIED in f64 ---
    let mut prims: Vec<GeomPrim> = Vec::new();
    let mut taps_rev: Vec<TapOp> = Vec::new();
    // S_out: physical index of the K×-output → logical (half-texel
    // convention, as bilinear_into); k_out=1 → identity exactly
    let ko = k_out as f64;
    let mut pending = Aff { ax: 1.0 / ko, bx: 0.5 / ko - 0.5, ay: 1.0 / ko, by: 0.5 / ko - 0.5 };
    let flush = |pending: &mut Aff, prims: &mut Vec<GeomPrim>| {
        if !pending.is_identity() {
            prims.push(GeomPrim::Affine(*pending));
        }
        *pending = Aff::IDENTITY;
    };
    for st in fw.iter().rev() {
        match st {
            Step::Frame { x0, y0, cw, ch, iw, ih } => {
                let ax = cw / mow as f64;
                let ay = ch / moh as f64;
                pending =
                    pending.then(&Aff { ax, bx: x0 + 0.5 * ax - 0.5, ay, by: y0 + 0.5 * ay - 0.5 });
                // frame wider than the input (zoom<1 / Stretch): a “plateau” from the stretched
                // edge — the semantics of the old step-by-step tap clamp
                let cx = *cw > *iw as f64;
                let cy = *ch > *ih as f64;
                if cx || cy {
                    flush(&mut pending, &mut prims);
                    prims.push(GeomPrim::Clamp {
                        x0: if cx { 0.0 } else { -CLAMP_OFF },
                        x1: if cx { (*iw - 1) as f64 } else { CLAMP_OFF },
                        y0: if cy { 0.0 } else { -CLAMP_OFF },
                        y1: if cy { (*ih - 1) as f64 } else { CLAMP_OFF },
                    });
                }
            }
            Step::Mirror { h, v, iw, ih } => {
                pending = pending.then(&Aff {
                    ax: if *h { -1.0 } else { 1.0 },
                    bx: if *h { (*iw - 1) as f64 } else { 0.0 },
                    ay: if *v { -1.0 } else { 1.0 },
                    by: if *v { (*ih - 1) as f64 } else { 0.0 },
                });
            }
            Step::Grid { offx, offy, gw, gh, cw, ch, iw, ih } => {
                // index → “d” coordinate of the grid (center-space minus offset);
                // for a single grid composed = ox + (0.5 − offx) — exact
                pending = pending.then(&Aff {
                    ax: 1.0,
                    bx: 0.5 - *offx as f64,
                    ay: 1.0,
                    by: 0.5 - *offy as f64,
                });
                flush(&mut pending, &mut prims);
                prims.push(GeomPrim::Fold {
                    gw: *gw as f64,
                    gh: *gh as f64,
                    cw: *cw as f64,
                    ch: *ch as f64,
                    in_w: *iw as f64,
                    in_h: *ih as f64,
                });
                // output of Fold — already the index-space of the step's input
            }
            Step::Proj { m, iw, ih } => {
                // index → center-space of the step output
                pending = pending.then(&Aff { ax: 1.0, bx: 0.5, ay: 1.0, by: 0.5 });
                flush(&mut pending, &mut prims);
                prims.push(GeomPrim::Proj { m: *m });
                // uv → index-space of the input: sx = q·in − 0.5 (as pan3d.wgsl)
                pending = Aff { ax: *iw as f64, bx: -0.5, ay: *ih as f64, by: -0.5 };
            }
            Step::Place { aff, m, rot0, bounds, iw, ih } => {
                if *rot0 {
                    pending = pending.then(aff);
                } else {
                    pending = pending.then(&Aff { ax: 1.0, bx: 0.5, ay: 1.0, by: 0.5 });
                    flush(&mut pending, &mut prims);
                    prims.push(GeomPrim::Proj { m: *m });
                    pending = Aff { ax: *iw as f64, bx: -0.5, ay: *ih as f64, by: -0.5 };
                }
                if let Some((x0, x1, y0, y1)) = bounds {
                    flush(&mut pending, &mut prims);
                    prims.push(GeomPrim::Bounds { x0: *x0, x1: *x1, y0: *y0, y1: *y1 });
                }
            }
            Step::Warp { mode, p, iw, ih } => {
                let (rw, rh) = (*iw as f64, *ih as f64);
                // r = 1 — short semi-axis; cylinder is normalized by X
                let r = if *mode == 4 { rw / 2.0 } else { rw.min(rh) / 2.0 };
                // index → normalized centered
                pending = pending.then(&Aff {
                    ax: 1.0 / r,
                    bx: (0.5 - rw / 2.0) / r,
                    ay: 1.0 / r,
                    by: (0.5 - rh / 2.0) / r,
                });
                flush(&mut pending, &mut prims);
                prims.push(GeomPrim::Warp { mode: *mode, p: *p });
                // normalized → index of the same raster
                pending = Aff { ax: r, bx: rw / 2.0 - 0.5, ay: r, by: rh / 2.0 - 0.5 };
            }
            Step::Tap(kind) => {
                // stage of the op: the current coordinate of the backward pass is the raster
                // of its step; the affine join is split here
                flush(&mut pending, &mut prims);
                taps_rev.push(TapOp { after_prim: prims.len(), kind: *kind });
            }
        }
    }
    // S_in⁻¹: logical input index → physical K_in×-buffer;
    // k_in=1 → identity exactly
    let ki = k_in as f64;
    pending = pending.then(&Aff { ax: ki, bx: (ki - 1.0) / 2.0, ay: ki, by: (ki - 1.0) / 2.0 });
    flush(&mut pending, &mut prims);
    // chain order of the ops (the backward pass collected them from the end);
    // after_prim decrease in this case
    taps_rev.reverse();
    let tap_ops = taps_rev;
    // the budgets (MAX_PRIMS/MAX_PRIM_SLOTS/MAX_TAP_OPS) are checked by the
    // scheduler, and when exceeded it cuts the run at a geo boundary
    // Physical output = k_out × logical (a series of only Mirror / Pan3d
    // without a camera does not change logical sizes — as the old path).
    let (pout_w, pout_h) = (w * k_out, h * k_out);
    // Supersampling (item 309): for an affine program (incl. with Fold) the Jacobian
    // is CONSTANT — the taps are computed once here, in f64, and go into the uniform
    // (the conjugation affines are already in the prims — the magnifier is automatically physical);
    // Proj (perspective) makes it per-pixel.
    let (ss, lens) =
        if prims.iter().any(|p| matches!(p, GeomPrim::Proj { .. } | GeomPrim::Warp { .. })) {
            // magnifier for the K× scheduler — estimate by probes of the backward map
            (SsSpec::PerPixel, probe_program_lens(&prims, pout_w, pout_h))
        } else {
            let (mut lx, mut ly) = (1.0f64, 1.0f64);
            for p in &prims {
                match p {
                    GeomPrim::Affine(a) => {
                        lx *= a.ax.abs();
                        ly *= a.ay.abs();
                    }
                    GeomPrim::Fold { cw, ch, in_w, in_h, .. } => {
                        lx *= in_w / cw;
                        ly *= in_h / ch;
                    }
                    GeomPrim::Clamp { .. }
                    | GeomPrim::Bounds { .. }
                    | GeomPrim::Proj { .. }
                    | GeomPrim::Warp { .. } => {}
                }
            }
            (SsSpec::Const { nx: ss_taps(lx), ny: ss_taps(ly) }, (lx, ly))
        };
    GeomProgram {
        prims,
        in_w,
        in_h,
        out_w: pout_w,
        out_h: pout_h,
        has_alpha_known,
        has_alpha,
        ss,
        tap_ops,
        lens,
    }
}

/// Magnifier estimate of a Proj program for the K× scheduler (item 314): lengths of the images
/// of unit steps at five points (center + corner neighbourhoods), the maximum over
/// valid probes. Inaccuracy gives softer/costlier, but IDENTICALLY on both
/// backends — K in the plan is one.
fn probe_program_lens(prims: &[GeomPrim], out_w: u32, out_h: u32) -> (f64, f64) {
    let (ow, oh) = (out_w.max(2) as f64, out_h.max(2) as f64);
    let pts =
        [(ow * 0.5, oh * 0.5), (1.0, 1.0), (ow - 2.0, 1.0), (1.0, oh - 2.0), (ow - 2.0, oh - 2.0)];
    let coords = |x: f64, y: f64| -> Option<(f64, f64)> {
        match map_pos(prims, x, y) {
            Mapped::At(sx, sy) => Some((sx, sy)),
            _ => None,
        }
    };
    let (mut lx, mut ly) = (1.0f64, 1.0f64);
    for (px, py) in pts {
        if let (Some(a), Some(bx)) = (coords(px, py), coords(px + 1.0, py)) {
            lx = lx.max(((a.0 - bx.0).powi(2) + (a.1 - bx.1).powi(2)).sqrt());
        }
        if let (Some(a), Some(by)) = (coords(px, py), coords(px, py + 1.0)) {
            ly = ly.max(((a.0 - by.0).powi(2) + (a.1 - by.1).powi(2)).sqrt());
        }
    }
    (lx, ly)
}

// ---- Pan3d: homography (R.44) ----------------------------------------------
// The layer quad is planar (z = 0) and z_clip ≡ 0, therefore the mapping
// (q.x, q.y) → screen is a pure 3×3 homography. The math VERBATIM repeats the
// former pan3d_mvp (glam f32) + the rasterizer viewport, but is computed in f64;
// the world/camera semantics are frozen by the file format (kt-gpu/CLAUDE.md):
// world — project px × world_scale, X right, Y down, Z into depth, rotations
// Rz·Rx·Ry, camera “50 mm” at auto-distance −(H/2)·cot(fov/2).

type V3 = [f64; 3];
type M3 = [[f64; 3]; 3];

fn m3_mul(a: &M3, b: &M3) -> M3 {
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    r
}

fn cross(a: &V3, b: &V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: &V3, b: &V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn rot_z(a: f64) -> M3 {
    let (s, c) = a.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

fn rot_x(a: f64) -> M3 {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

fn rot_y(a: f64) -> M3 {
    let (s, c) = a.sin_cos();
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}

/// Inversion of the 3×3 via adj/det. None — degeneracy (quad edge-on to the camera).
/// The sign of t.z of the inverse image under adj/det is correct: t = G⁻¹·(c,1) = q̃/w,
/// i.e. sign(t.z) = sign(w) — the “behind the camera” test comes out by itself.
/// INVERSE homography of Corner Pin (R.47, F-FX-004): the forward map
/// “unit square → 4 corners in output fractions” is built in closed form
/// (Heckbert), scaled into the pixel center-space of the output, and inverted.
/// Degeneracy (zero denominator / non-invertible matrix) — a transparent
/// matrix `t.z = −1` (precedent: Pan3d behind the camera).
fn corner_pin_inverse(c: [(f64, f64); 4], out_w: u32, out_h: u32) -> [[f64; 3]; 3] {
    let transparent = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]];
    // corners: TL, TR, BL, BR (uv-space of the source: (0,0)..(1,1))
    let (p0, p1, p2, p3) = (c[0], c[1], c[2], c[3]);
    let dx1 = (p1.0 - p3.0, p1.1 - p3.1);
    let dx2 = (p2.0 - p3.0, p2.1 - p3.1);
    let sx = p0.0 - p1.0 - p2.0 + p3.0;
    let sy = p0.1 - p1.1 - p2.1 + p3.1;
    let den = dx1.0 * dx2.1 - dx1.1 * dx2.0;
    if den.abs() < 1e-12 {
        return transparent;
    }
    let g = (sx * dx2.1 - sy * dx2.0) / den;
    let hh = (dx1.0 * sy - dx1.1 * sx) / den;
    // forward map uv(square) -> output fractions
    let fwd = [
        [p1.0 - p0.0 + g * p1.0, p2.0 - p0.0 + hh * p2.0, p0.0],
        [p1.1 - p0.1 + g * p1.1, p2.1 - p0.1 + hh * p2.1, p0.1],
        [g, hh, 1.0],
    ];
    // output fractions -> pixel center-space of the output: the x and y rows are multiplied
    // by mow/moh (as the tail of pan3d_forward)
    let (mw, mh) = (out_w.max(2) as f64, out_h.max(2) as f64);
    let scaled = [
        [fwd[0][0] * mw, fwd[0][1] * mw, fwd[0][2] * mw],
        [fwd[1][0] * mh, fwd[1][1] * mh, fwd[1][2] * mh],
        fwd[2],
    ];
    match invert3(&scaled) {
        Some(inv) => inv,
        None => transparent,
    }
}

fn invert3(g: &M3) -> Option<M3> {
    let adj = [
        [
            g[1][1] * g[2][2] - g[1][2] * g[2][1],
            g[0][2] * g[2][1] - g[0][1] * g[2][2],
            g[0][1] * g[1][2] - g[0][2] * g[1][1],
        ],
        [
            g[1][2] * g[2][0] - g[1][0] * g[2][2],
            g[0][0] * g[2][2] - g[0][2] * g[2][0],
            g[0][2] * g[1][0] - g[0][0] * g[1][2],
        ],
        [
            g[1][0] * g[2][1] - g[1][1] * g[2][0],
            g[0][1] * g[2][0] - g[0][0] * g[2][1],
            g[0][0] * g[1][1] - g[0][1] * g[1][0],
        ],
    ];
    let det = g[0][0] * adj[0][0] + g[0][1] * adj[1][0] + g[0][2] * adj[2][0];
    // the determinant scale of our matrices is ~ (ow·oh·f)² — the threshold is relative
    let scale = g.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs()));
    if det.abs() <= scale * scale * scale * 1e-12 {
        return None;
    }
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = adj[i][j] / det;
        }
    }
    Some(m)
}

/// FORWARD homography of Pan3d: (q.x, q.y, 1) → (c.x·w, c.y·w, w), where q is the uv
/// of the quad 0..1, c is the CENTER-space of the output pixels.
fn pan3d_forward(
    c3: &Cam3d,
    out: (u32, u32),
    pos: [f64; 3],
    rot: [f64; 3],
    scale: f64,
    anchor: [f64; 2],
    quad: (f64, f64),
) -> M3 {
    let s = c3.world_scale;
    let cam = &c3.cam;
    let (ow, oh) = (out.0 as f64, out.1 as f64);
    let sc = scale.max(1e-6);

    // model: world(q) = pos·s + R·(sc·lx, sc·ly, 0), lx = (q.x − ax)·qw
    let r = m3_mul(
        &m3_mul(&rot_z(rot[2].to_radians()), &rot_x(rot[0].to_radians())),
        &rot_y(rot[1].to_radians()),
    );
    let col = |j: usize| -> V3 { [r[0][j], r[1][j], r[2][j]] };
    let (c0, c1) = (col(0), col(1));
    let k1 = sc * quad.0;
    let k2 = sc * quad.1;
    let e1: V3 = [c0[0] * k1, c0[1] * k1, c0[2] * k1];
    let e2: V3 = [c1[0] * k2, c1[1] * k2, c1[2] * k2];
    let e0: V3 = [
        pos[0] * s - anchor[0] * e1[0] - anchor[1] * e2[0],
        pos[1] * s - anchor[0] * e1[1] - anchor[1] * e2[1],
        pos[2] * s - anchor[0] * e1[2] - anchor[1] * e2[2],
    ];

    // camera: auto-distance “50 mm” — the layer at z=0 renders 1:1
    let fov = cam.fov_deg.clamp(1.0, 179.0).to_radians();
    let f = 1.0 / (fov * 0.5).tan();
    let auto_z = -(oh * 0.5) * f;
    let eye: V3 =
        [cam.pos_x * s, cam.pos_y * s, if cam.pos_z == 0.0 { auto_z } else { cam.pos_z * s }];
    // view: at the target (two-node) or along +Z; screen up in the world is −Y
    let dir: V3 = match cam.target {
        Some((tx, ty, tz)) => {
            let d = [tx * s - eye[0], ty * s - eye[1], tz * s - eye[2]];
            let len2 = dot(&d, &d);
            if len2 < 1e-9 {
                [0.0, 0.0, 1.0]
            } else {
                let l = len2.sqrt();
                [d[0] / l, d[1] / l, d[2] / l]
            }
        }
        None => [0.0, 0.0, 1.0],
    };
    let up: V3 = [0.0, -1.0, 0.0];
    let rt = cross(&dir, &up);
    let rl2 = dot(&rt, &rt);
    let right: V3 = if rl2 > 1e-24 {
        let l = rl2.sqrt();
        [rt[0] / l, rt[1] / l, rt[2] / l]
    } else {
        [1.0, 0.0, 0.0]
    };
    let cdown = cross(&dir, &right);
    // basis rows: right / cdown / dir; the orientation rotation is AFTER the target
    let basis: M3 = [right, cdown, dir];
    let orient = m3_mul(
        &m3_mul(&rot_z(-cam.rot_z.to_radians()), &rot_x(-cam.rot_x.to_radians())),
        &rot_y(-cam.rot_y.to_radians()),
    );
    let view = m3_mul(&orient, &basis);
    // directions: v = view·e; point: v0 = view·(e0 − eye)
    let mv = |e: &V3| -> V3 {
        [
            view[0][0] * e[0] + view[0][1] * e[1] + view[0][2] * e[2],
            view[1][0] * e[0] + view[1][1] * e[1] + view[1][2] * e[2],
            view[2][0] * e[0] + view[2][1] * e[1] + view[2][2] * e[2],
        ]
    };
    let v1 = mv(&e1);
    let v2 = mv(&e2);
    let d0: V3 = [e0[0] - eye[0], e0[1] - eye[1], e0[2] - eye[2]];
    let v0 = mv(&d0);

    // projection: perspective x = f/aspect·v.x, y = −f·v.y, w = v.z;
    // ortho — affine (w of points = 1, of directions = 0)
    let aspect = ow / oh;
    let cl = |v: &V3, point: bool| -> V3 {
        if cam.ortho {
            [2.0 / ow * v[0], -2.0 / oh * v[1], if point { 1.0 } else { 0.0 }]
        } else {
            [f / aspect * v[0], -f * v[1], v[2]]
        }
    };
    let p1 = cl(&v1, false);
    let p2 = cl(&v2, false);
    let p0 = cl(&v0, true);

    // screen: c.x·w = ow/2·(clip.x + w); c.y·w = oh/2·(w − clip.y)
    [
        [ow * 0.5 * (p1[0] + p1[2]), ow * 0.5 * (p2[0] + p2[2]), ow * 0.5 * (p0[0] + p0[2])],
        [oh * 0.5 * (p1[2] - p1[1]), oh * 0.5 * (p2[2] - p2[1]), oh * 0.5 * (p0[2] - p0[1])],
        [p1[2], p2[2], p0[2]],
    ]
}

/// Result of the backward coordinate pass.
enum Mapped {
    At(f64, f64),
    /// Outside the EventGrid: black opaque canvas.
    Black,
    /// Outside the Pan3d quad / behind the camera: transparent.
    Transparent,
}

/// Inverse map of the nonlinear warp (R.47): ONE f64 text, read by
/// both CPU loops; the WGSL mirror is geom_fused.wgsl (the same line).
/// Input/output — the normalized centered space of the stage.
#[inline]
fn warp_map(mode: u32, p: &[f64; 7], x: f64, y: f64) -> (f64, f64) {
    match mode {
        0 => {
            // lens: polynomial s = 1 + k1·r² + k2·r⁴ (no trigonometry)
            let r2 = x * x + y * y;
            let s = 1.0 + p[0] * r2 + p[1] * r2 * r2;
            (x * s, y * s)
        }
        1 => {
            // twirl: rotation by a·(1 − r/rad)² inside the radius
            let r = (x * x + y * y).sqrt();
            if r >= p[1] {
                return (x, y);
            }
            let t = 1.0 - r / p[1];
            let (sn, cs) = (p[0] * t * t).sin_cos();
            (x * cs - y * sn, x * sn + y * cs)
        }
        2 => {
            // ripple: r' = r + amp·sin(k·r − φ)
            let r = (x * x + y * y).sqrt();
            if r <= 1e-9 {
                return (x, y);
            }
            let k = (r + p[0] * (p[1] * r - p[2]).sin()) / r;
            (x * k, y * k)
        }
        3 => {
            // kaleidoscope: triangle wave over the sector (mirror
            // repetition without int modulo — floor is the same in both texts)
            let r = (x * x + y * y).sqrt();
            let q = (y.atan2(x) - p[1]) / p[0];
            let h = q / 2.0 - (q / 2.0).floor();
            let m = 1.0 - (2.0 * h - 1.0).abs();
            let (sn, cs) = (p[1] + m * p[0]).sin_cos();
            (r * cs, r * sn)
        }
        _ => {
            // cylinder: u' = asin(u·sin a)/a along X, edges → edges
            let u = (x * p[0]).clamp(-1.0, 1.0).asin() * p[1];
            (u, y)
        }
    }
}

#[inline]
fn map_pos(prims: &[GeomPrim], ox: f64, oy: f64) -> Mapped {
    let (mut x, mut y) = (ox, oy);
    for p in prims {
        match p {
            GeomPrim::Affine(a) => {
                x = a.ax * x + a.bx;
                y = a.ay * y + a.by;
            }
            GeomPrim::Clamp { x0, x1, y0, y1 } => {
                x = x.clamp(*x0, *x1);
                y = y.clamp(*y0, *y1);
            }
            GeomPrim::Bounds { x0, x1, y0, y1 } => {
                if x < *x0 || x > *x1 || y < *y0 || y > *y1 {
                    return Mapped::Transparent;
                }
            }
            GeomPrim::Fold { gw, gh, cw, ch, in_w, in_h } => {
                if x < 0.0 || y < 0.0 || x >= *gw || y >= *gh {
                    return Mapped::Black;
                }
                let kx = (x / cw).floor();
                x = (x - kx * cw) / cw * in_w - 0.5;
                let ky = (y / ch).floor();
                y = (y - ky * ch) / ch * in_h - 0.5;
            }
            GeomPrim::Proj { m } => {
                let tx = m[0][0] * x + m[0][1] * y + m[0][2];
                let ty = m[1][0] * x + m[1][1] * y + m[1][2];
                let tz = m[2][0] * x + m[2][1] * y + m[2][2];
                // t.z ≤ 0 — behind the camera (rasterizer clipping)
                if tz <= 0.0 {
                    return Mapped::Transparent;
                }
                let qx = tx / tz;
                let qy = ty / tz;
                if !(0.0..=1.0).contains(&qx) || !(0.0..=1.0).contains(&qy) {
                    return Mapped::Transparent;
                }
                x = qx;
                y = qy;
            }
            GeomPrim::Warp { mode, p } => {
                let (nx, ny) = warp_map(*mode, p, x, y);
                x = nx;
                y = ny;
            }
        }
    }
    Mapped::At(x, y)
}

/// Final tap WITHOUT rounding: the same arithmetic as the inner loop of
/// `bilinear_into_scalar` (f64 accumulator, tap clamp to the edges).
/// The duplication is deliberate: the scalar bilinear is NOT refactored
/// (kt-fx/CLAUDE.md) — both texts are the definition of the bytes.
#[inline]
pub(crate) fn sample_bilinear_f64(src: &Frame, sx: f64, sy: f64) -> [f64; 4] {
    let (sw, sh) = (src.w as i64, src.h as i64);
    let y = sy.floor() as i64;
    let fy = sy - y as f64;
    let x = sx.floor() as i64;
    let fx = sx - x as f64;
    let mut px = [0f64; 4];
    for (dy, wy) in [(0i64, 1.0 - fy), (1, fy)] {
        for (dx, wx) in [(0i64, 1.0 - fx), (1, fx)] {
            let xi = (x + dx).clamp(0, sw - 1);
            let yi = (y + dy).clamp(0, sh - 1);
            let si = ((yi * sw + xi) * 4) as usize;
            let w = wy * wx;
            for (c, acc) in px.iter_mut().enumerate() {
                *acc += src.rgba[si + c] as f64 * w;
            }
        }
    }
    px
}

/// Backward pass WITH RECORDING of the coordinates of the tap stages (R.45, item 312).
/// Returns the result and the number of APPLIED prims: with a short circuit
/// at prim d the op with after_prim ≤ d stays alive (“noise over the black
/// canvas of the grid” works, “noise before the grid in the cut zone” does not).
#[inline]
fn map_pos_rec(
    prims: &[GeomPrim],
    taps: &[TapOp],
    ox: f64,
    oy: f64,
    rec: &mut [(f64, f64); MAX_TAP_OPS],
) -> (Mapped, usize) {
    let (mut x, mut y) = (ox, oy);
    // the taps are in chain order with DECREASING after_prim — the cursor walks
    // from the end of the list (increasing after_prim) together with the pass
    let mut cursor = taps.len();
    while cursor > 0 && taps[cursor - 1].after_prim == 0 {
        rec[cursor - 1] = (x, y);
        cursor -= 1;
    }
    for (pi, p) in prims.iter().enumerate() {
        match p {
            GeomPrim::Affine(a) => {
                x = a.ax * x + a.bx;
                y = a.ay * y + a.by;
            }
            GeomPrim::Clamp { x0, x1, y0, y1 } => {
                x = x.clamp(*x0, *x1);
                y = y.clamp(*y0, *y1);
            }
            GeomPrim::Bounds { x0, x1, y0, y1 } => {
                if x < *x0 || x > *x1 || y < *y0 || y > *y1 {
                    return (Mapped::Transparent, pi);
                }
            }
            GeomPrim::Fold { gw, gh, cw, ch, in_w, in_h } => {
                if x < 0.0 || y < 0.0 || x >= *gw || y >= *gh {
                    return (Mapped::Black, pi);
                }
                let kx = (x / cw).floor();
                x = (x - kx * cw) / cw * in_w - 0.5;
                let ky = (y / ch).floor();
                y = (y - ky * ch) / ch * in_h - 0.5;
            }
            GeomPrim::Proj { m } => {
                let tx = m[0][0] * x + m[0][1] * y + m[0][2];
                let ty = m[1][0] * x + m[1][1] * y + m[1][2];
                let tz = m[2][0] * x + m[2][1] * y + m[2][2];
                if tz <= 0.0 {
                    return (Mapped::Transparent, pi);
                }
                let qx = tx / tz;
                let qy = ty / tz;
                if !(0.0..=1.0).contains(&qx) || !(0.0..=1.0).contains(&qy) {
                    return (Mapped::Transparent, pi);
                }
                x = qx;
                y = qy;
            }
            GeomPrim::Warp { mode, p } => {
                let (nx, ny) = warp_map(*mode, p, x, y);
                x = nx;
                y = ny;
            }
        }
        while cursor > 0 && taps[cursor - 1].after_prim == pi + 1 {
            rec[cursor - 1] = (x, y);
            cursor -= 1;
        }
    }
    (Mapped::At(x, y), prims.len())
}

/// Greenscreen on the subtap value: f64 continuum of the integer kernel formula
/// (rounding happens once at the end of the pipeline).
#[inline]
fn tap_gs(v: &mut [f64; 4], key: &[f64; 3], tol: f64, hard: f64, spill: f64, kc: usize) {
    let d = ((v[0] - key[0]).powi(2) + (v[1] - key[1]).powi(2) + (v[2] - key[2]).powi(2)).sqrt();
    if d <= hard {
        v[3] = 0.0;
    } else if d < tol {
        v[3] *= (d - hard) / (tol - hard).max(1e-9);
    }
    if spill > 0.0 && v[3] > 0.0 {
        let other = v[(kc + 1) % 3].max(v[(kc + 2) % 3]);
        if v[kc] > other {
            v[kc] -= (v[kc] - other) * spill;
        }
    }
}

/// Noise on the subtap value: offset from the floor of the coordinate of ITS OWN stage
/// (grain pinned to its grid, decision R.45); the hash formulas are verbatim
/// the noise kernel (both branches).
#[inline]
#[allow(clippy::too_many_arguments)]
fn tap_noise(
    v: &mut [f64; 4],
    coord: (f64, f64),
    seed: u32,
    k: i64,
    cell: u32,
    color: bool,
    special: bool,
    stage_w: u32,
    stage_h: u32,
) {
    use crate::video::noise_hash as nh;
    let xi = (coord.0.floor() as i64).clamp(0, stage_w as i64 - 1) as u32;
    let yi = (coord.1.floor() as i64).clamp(0, stage_h as i64 - 1) as u32;
    if special {
        let hsh = nh(xi ^ nh(yi ^ seed));
        let n = ((hsh >> 16) & 0x7fff) as i64 - 16384;
        let off = (n * k / 32767) as f64;
        for c in v.iter_mut().take(3) {
            *c += off;
        }
    } else {
        let (gx, gy) = (xi / cell, yi / cell);
        let off_at = |ch: u32| -> f64 {
            let hsh = nh(gx ^ nh(gy ^ nh(ch ^ seed)));
            let n = ((hsh >> 16) & 0x7fff) as i64 - 16384;
            (n * k / 32767) as f64
        };
        if color {
            for (c, val) in v.iter_mut().take(3).enumerate() {
                *val += off_at(c as u32 + 1);
            }
        } else {
            let off = off_at(0);
            for c in v.iter_mut().take(3) {
                *c += off;
            }
        }
    }
}

/// ColorGrade on the subtap (R.47): the order is saturation and hue BEFORE contrast
/// and brightness (the basic panels of five editors; otherwise contrast recolours the
/// hues). Hue — a rotation around the gray axis (Rodrigues, k = (1,1,1)/√3).
/// The values are an f64 continuum; the final clamp+round — once, at the output.
pub(crate) fn tap_grade(v: &mut [f64; 4], br: f64, con: f64, sat: f64, hc: f64, hs: f64) {
    let l = (77.0 * v[0] + 150.0 * v[1] + 29.0 * v[2]) / 256.0;
    for c in v.iter_mut().take(3) {
        *c = l + (*c - l) * sat;
    }
    if hc != 1.0 || hs != 0.0 {
        let k = 1.0 / 3f64.sqrt();
        let (x, y, z) = (v[0], v[1], v[2]);
        let dot = k * (x + y + z);
        let (cx, cy, cz) = (k * (y - z), k * (z - x), k * (x - y));
        v[0] = x * hc + cx * hs + k * dot * (1.0 - hc);
        v[1] = y * hc + cy * hs + k * dot * (1.0 - hc);
        v[2] = z * hc + cz * hs + k * dot * (1.0 - hc);
    }
    for c in v.iter_mut().take(3) {
        *c = (*c - 128.0) * (1.0 + con) + 128.0 + br * 255.0;
    }
}

/// Duotone on the subtap: lerp of two colors by luma; alpha untouched.
pub(crate) fn tap_duo(v: &mut [f64; 4], shadow: &[f64; 3], high: &[f64; 3], amount: f64) {
    let l = ((77.0 * v[0] + 150.0 * v[1] + 29.0 * v[2]) / 256.0 / 255.0).clamp(0.0, 1.0);
    for (c, (sh, hi)) in v.iter_mut().take(3).zip(shadow.iter().zip(high.iter())) {
        let out = sh + (hi - sh) * l;
        *c += (out - *c) * amount;
    }
}

/// Posterize on the subtap: quantization PER CHANNEL (standard — AE Posterize).
pub(crate) fn tap_poster(v: &mut [f64; 4], levels: u32) {
    let n = levels.clamp(2, 32) as f64;
    for c in v.iter_mut().take(3) {
        let i = (*c * n / 256.0).floor().clamp(0.0, n - 1.0);
        *c = i * 255.0 / (n - 1.0);
    }
}

/// Vignette on the subtap: coordinate of ITS OWN stage; the radius is normalized to
/// the half-diagonal; smoothstep written out by hand (one text in both
/// executors).
pub(crate) fn tap_vig(
    v: &mut [f64; 4],
    coord: (f64, f64),
    strength: f64,
    radius: f64,
    softness: f64,
    stage_w: u32,
    stage_h: u32,
) {
    let dx = (coord.0 + 0.5) / stage_w.max(1) as f64 - 0.5;
    let dy = (coord.1 + 0.5) / stage_h.max(1) as f64 - 0.5;
    let r = ((2.0 * dx) * (2.0 * dx) + (2.0 * dy) * (2.0 * dy)).sqrt() / std::f64::consts::SQRT_2;
    let t = ((r - radius) / softness.max(1e-6)).clamp(0.0, 1.0);
    let w = t * t * (3.0 - 2.0 * t);
    let g = 1.0 - strength * w;
    for c in v.iter_mut().take(3) {
        *c *= g;
    }
}

/// One subtap: the whole inverse chain + the tap (the Fold border and the Proj silhouette
/// are anti-aliased honestly — each subtap decides the short circuits itself);
/// then the absorbed per-pixel ops in chain order (R.45, item 312).
#[inline]
fn sample_at(p: &GeomProgram, src: &Frame, x: f64, y: f64) -> [f64; 4] {
    if p.tap_ops.is_empty() {
        // fast path — exactly R.44
        return match map_pos(&p.prims, x, y) {
            Mapped::At(sx, sy) => sample_bilinear_f64(src, sx, sy),
            Mapped::Black => [0.0, 0.0, 0.0, 255.0],
            Mapped::Transparent => [0.0, 0.0, 0.0, 0.0],
        };
    }
    let mut rec = [(0.0f64, 0.0f64); MAX_TAP_OPS];
    let (mapped, applied) = map_pos_rec(&p.prims, &p.tap_ops, x, y, &mut rec);
    let mut v = match mapped {
        Mapped::At(sx, sy) => sample_bilinear_f64(src, sx, sy),
        Mapped::Black => [0.0, 0.0, 0.0, 255.0],
        Mapped::Transparent => [0.0, 0.0, 0.0, 0.0],
    };
    for (ti, op) in p.tap_ops.iter().enumerate() {
        if op.after_prim > applied {
            continue; // the op's stage is cut by a short circuit
        }
        match op.kind {
            TapKind::Gs { key, tol, hard, spill, kc } => tap_gs(&mut v, &key, tol, hard, spill, kc),
            TapKind::Noise { seed, k, cell, color, special, stage_w, stage_h } => {
                tap_noise(&mut v, rec[ti], seed, k, cell, color, special, stage_w, stage_h)
            }
            TapKind::Grade { brightness, contrast, saturation, hue_cos, hue_sin } => {
                tap_grade(&mut v, brightness, contrast, saturation, hue_cos, hue_sin)
            }
            TapKind::Duo { shadow, high, amount } => tap_duo(&mut v, &shadow, &high, amount),
            TapKind::Poster { levels } => tap_poster(&mut v, levels),
            TapKind::Vig { strength, radius, softness, stage_w, stage_h } => {
                tap_vig(&mut v, rec[ti], strength, radius, softness, stage_w, stage_h)
            }
        }
    }
    v
}

/// Coordinate probe (no tap) for the per-pixel Jacobian of Proj series.
#[inline]
fn coords_at(p: &GeomProgram, x: f64, y: f64) -> Option<(f64, f64)> {
    match map_pos(&p.prims, x, y) {
        Mapped::At(sx, sy) => Some((sx, sy)),
        _ => None,
    }
}

/// Per-pixel taps: finite differences (coordinate probes). A dead forward probe
/// (a neighbour beyond the silhouette/edge) is replaced by a BACKWARD one — otherwise
/// the last live column of the identity map would get extra taps and
/// lose byte-exactness (t298). If both probes are dead, or the centre is dead — one
/// tap: the colour there is decided by the subtap short circuits anyway.
#[inline]
fn probe_taps(p: &GeomProgram, ox: f64, oy: f64) -> (u32, u32) {
    let c0 = coords_at(p, ox, oy);
    let axis = |dx: f64, dy: f64| -> f64 {
        let Some(a) = c0 else { return 1.0 };
        let dist = |b: (f64, f64)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
        if let Some(b) = coords_at(p, ox + dx, oy + dy) {
            return dist(b);
        }
        if let Some(b) = coords_at(p, ox - dx, oy - dy) {
            return dist(b);
        }
        1.0
    };
    (ss_taps(axis(1.0, 0.0)), ss_taps(axis(0.0, 1.0)))
}

/// CPU executor of the fused program: scalar f64 reference (invariant No. 11).
/// The fused pass has NO SIMD in this round (measurement and the 1.3× threshold are separate
/// work, kt-fx/CLAUDE.md). One tap (no minification) — the former bytes;
/// supersampling — box average of the subtaps without intermediate roundings,
/// round once (item 309).
pub fn exec_geom_cpu(p: &GeomProgram, src: &Frame, scratch: &mut FxScratch) -> Frame {
    let (ow, oh) = (p.out_w, p.out_h);
    let mut out = scratch.take((ow * oh * 4) as usize);
    for oy in 0..oh {
        for ox in 0..ow {
            let (nx, ny) = match p.ss {
                SsSpec::Const { nx, ny } => (nx, ny),
                SsSpec::PerPixel => probe_taps(p, ox as f64, oy as f64),
            };
            let px = if nx == 1 && ny == 1 {
                sample_at(p, src, ox as f64, oy as f64)
            } else {
                let mut acc = [0f64; 4];
                for j in 0..ny {
                    let sy = oy as f64 - 0.5 + (j as f64 + 0.5) / ny as f64;
                    for i in 0..nx {
                        let sx = ox as f64 - 0.5 + (i as f64 + 0.5) / nx as f64;
                        let s = sample_at(p, src, sx, sy);
                        for (c, a) in acc.iter_mut().enumerate() {
                            *a += s[c];
                        }
                    }
                }
                let n = (nx * ny) as f64;
                [acc[0] / n, acc[1] / n, acc[2] / n, acc[3] / n]
            };
            let oi = ((oy * ow + ox) * 4) as usize;
            for c in 0..4 {
                out[oi + c] = px[c].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let has_alpha = if p.has_alpha_known { p.has_alpha } else { src.has_alpha };
    Frame { w: ow, h: oh, rgba: out, has_alpha }
}

// Execution of the geo run lives in the plan executors (video.rs / kt-gpu):
// the legacy/fused decision is already made by `plan_chain` (R.45, item 311).
