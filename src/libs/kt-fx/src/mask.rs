//! The mask rasterizer (shape-masks): ONE CPU function turns an ordered mask list into an 8-bit
//! coverage; the GPU uploads the bytes and applies them in one integer
//! pass (`kt-gpu/src/shaders/mask.wgsl`) — so the conformance class is EXACT.
//!
//! Per mask: the cubic outline (`kt_model::Mask::cubics`, the shape's one geometry) is mapped by
//! the affine of the application point, flattened, and then
//! - the INSIDE is the nonzero winding at the sample centres (a scanline over crossings);
//! - the SIGNED DISTANCE to the polyline is computed only in a band around it;
//! - the coverage is a ramp of the distance: one pixel without feather, the feather centred on
//!   the contour with it (falloff linear or smoothstep), the expansion moving the edge.
//!
//! A wide band is sampled on a coarser grid (step `s`, the distance bilinearly interpolated):
//! the distance field is 1-Lipschitz, a feather of 80 px does not need 1-px samples, and the cost
//! stays O(perimeter) instead of O(perimeter × feather²).
//!
//! Only IEEE-exact operations (`+ − × ÷ sqrt floor`) and the frozen math of `kt_model::mask`
//! (the rotation) are used: the bytes are the same on every platform (invariant 11).

use std::sync::{Arc, Mutex};

use kt_model::{Falloff, Mask, MaskMode};

/// The coverage of a mask list on one raster: 0 — outside, 255 — inside.
#[derive(Debug, Clone, PartialEq)]
pub struct Coverage {
    pub w: u32,
    pub h: u32,
    pub cov: Vec<u8>,
}

/// Where a mask list lands on a raster: the mask's reference frame in px (`ref_w × ref_h`,
/// normalized coordinates × these), the affine reference px → raster px
/// (`x' = a·x + b·y + c`, `y' = d·x + e·y + f`) and how many raster px one MASK px is (feather
/// and expansion are mask px).
///
/// ONE MASK PX is a project pixel of the reference frame's HEIGHT: the frame's height on the
/// raster divided by the project's height (`px_of`). It does not depend on which reference size a
/// caller works in (the engine's decode raster, the interface's source view): only on where the
/// frame lands — so the preview at ¼ and the export at 1/1 feather alike, and an event zoomed ×2
/// has a feather twice as wide, as in After Effects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaskSpace {
    pub ref_w: f64,
    pub ref_h: f64,
    pub xf: [f64; 6],
    pub px: f64,
}

/// Raster px per mask px for an affine `xf` of a reference frame `ref_h` high, `project_h` — the
/// project's logical height.
pub fn px_of(xf: &[f64; 6], ref_h: f64, project_h: f64) -> f64 {
    let (vx, vy) = (xf[1] * ref_h, xf[4] * ref_h);
    (vx * vx + vy * vy).sqrt() / project_h.max(1e-9)
}

impl MaskSpace {
    /// The reference frame IS the raster (a track mask, a slot before the Geometry node) —
    /// scaled to the raster's size.
    pub fn plain(ref_w: f64, ref_h: f64, w: u32, h: u32, project_h: f64) -> MaskSpace {
        let (sx, sy) = (w as f64 / ref_w.max(1e-9), h as f64 / ref_h.max(1e-9));
        let xf = [sx, 0.0, 0.0, 0.0, sy, 0.0];
        MaskSpace { ref_w, ref_h, xf, px: px_of(&xf, ref_h, project_h) }
    }

    /// The reference frame placed by a Geometry node's `Placement` (input px of the node → the
    /// logical canvas), then scaled to a `raster` — the same map the composite places the layer
    /// with. The interface draws the overlay with it, the executors rasterize with it.
    pub fn placed(
        pl: &crate::geom::Placement,
        ref_size: (f64, f64),
        canvas: (f64, f64),
        raster: (f64, f64),
        project_h: f64,
    ) -> MaskSpace {
        use kt_plugin_api::math::FrozenMath;
        let (rw, rh) = ref_size;
        let (cx0, cy0, cw, ch) = pl.crop_in;
        let (sx, sy) = (pl.dw / cw.max(1e-9), pl.dh / ch.max(1e-9));
        let (tx, ty) = (pl.ox - cx0 * sx, pl.oy - cy0 * sy);
        let (c, s) = if pl.rot_deg == 0.0 {
            (1.0, 0.0)
        } else {
            (pl.rot_deg.to_radians().fcos(), pl.rot_deg.to_radians().fsin())
        };
        let (ax, ay) = pl.anchor;
        let (kx, ky) = (raster.0 / canvas.0.max(1e-9), raster.1 / canvas.1.max(1e-9));
        // q = K·(R·(L·p + t) + A − R·A)
        let rt = (c * tx - s * ty + ax - (c * ax - s * ay), s * tx + c * ty + ay - (s * ax + c * ay));
        let xf = [kx * c * sx, -kx * s * sy, kx * rt.0, ky * s * sx, ky * c * sy, ky * rt.1];
        MaskSpace { ref_w: rw, ref_h: rh, xf, px: px_of(&xf, rh, project_h) }
    }

    fn map(&self, p: (f64, f64)) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.xf;
        (a * p.0 + b * p.1 + c, d * p.0 + e * p.1 + f)
    }
}

type Pt = (f64, f64);

/// Flatten a cubic: the number of steps from the control polygon's length (chord error well
/// under a quarter of a sample); a straight segment is one step.
fn flatten(seg: &[Pt; 4], step: f64, out: &mut Vec<Pt>) {
    let [p0, p1, p2, p3] = *seg;
    let d = |a: Pt, b: Pt| ((a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)).sqrt();
    // distance of the control points from the chord — a line has none
    let chord = d(p0, p3);
    let off = |p: Pt| {
        if chord < 1e-12 {
            d(p, p0)
        } else {
            ((p3.0 - p0.0) * (p0.1 - p.1) - (p0.0 - p.0) * (p3.1 - p0.1)).abs() / chord
        }
    };
    let bend = off(p1).max(off(p2));
    let n = if bend < 1e-3 * step {
        1
    } else {
        let len = d(p0, p1) + d(p1, p2) + d(p2, p3);
        ((len / step).sqrt() * 1.5).ceil().clamp(2.0, 512.0) as usize
    };
    for i in 1..=n {
        let t = i as f64 / n as f64;
        let u = 1.0 - t;
        let (a, b, c, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        out.push((
            a * p0.0 + b * p1.0 + c * p2.0 + e * p3.0,
            a * p0.1 + b * p1.1 + c * p2.1 + e * p3.1,
        ));
    }
}

/// The closed polyline of one mask in raster px.
fn polyline(m: &Mask, sp: &MaskSpace, step: f64) -> Vec<Pt> {
    let cubics = m.cubics(sp.ref_w, sp.ref_h);
    let mut pts = Vec::new();
    if let Some(first) = cubics.first() {
        pts.push(sp.map(first[0]));
    }
    for c in &cubics {
        let mapped = c.map(|p| sp.map(p));
        flatten(&mapped, step, &mut pts);
    }
    pts
}

/// Sorted crossings of the horizontal line `y` with the closed polyline: (x, winding dir).
fn crossings(poly: &[Pt], y: f64, out: &mut Vec<(f64, i32)>) {
    out.clear();
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if a.1 == b.1 {
            continue;
        }
        // half-open on y: the lower end counts, the upper does not (no double hits at vertices)
        let (lo, hi, dir) = if a.1 < b.1 { (a, b, 1) } else { (b, a, -1) };
        if y >= lo.1 && y < hi.1 {
            let t = (y - lo.1) / (hi.1 - lo.1);
            out.push((lo.0 + (hi.0 - lo.0) * t, dir));
        }
    }
    out.sort_by(|p, q| p.0.total_cmp(&q.0));
}

/// Squared distance from `p` to the segment `a..b`.
fn seg_dist2(p: Pt, a: Pt, b: Pt) -> f64 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let (wx, wy) = (p.0 - a.0, p.1 - a.1);
    let l2 = vx * vx + vy * vy;
    let t = if l2 > 0.0 { ((wx * vx + wy * vy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let (dx, dy) = (wx - vx * t, wy - vy * t);
    dx * dx + dy * dy
}

/// The coverage of ONE mask as f32 0..1 on the raster (before the mode).
fn one_mask(m: &Mask, sp: &MaskSpace, w: u32, h: u32) -> Vec<f32> {
    let (wu, hu) = (w as usize, h as usize);
    let feather = (m.feather.max(0.0) * sp.px).max(0.0);
    let expansion = m.expansion * sp.px;
    // the ramp's width: at least one pixel — the antialiasing
    let ramp = feather.max(1.0);
    // the band where the ramp is neither 0 nor 1, plus the interpolation margin
    let core = expansion.abs() + ramp / 2.0 + 1.5;
    // the sample step of the distance grid: 1 for narrow bands, coarser for wide ones
    let s = ((core / 8.0).floor()).clamp(1.0, 32.0);
    let band = core + 2.0 * s;
    let poly = polyline(m, sp, 0.25 * s);
    let mut out = vec![0f32; wu * hu];
    if poly.len() < 3 {
        return out;
    }
    let si = s as usize;
    // the node grid: nodes at ((i + 0.5)·s, (j + 0.5)·s); for s = 1 — exactly the pixel centres
    let gw = wu.div_ceil(si) + 1;
    let gh = hu.div_ceil(si) + 1;
    let node = |i: usize| (i as f64 + 0.5) * s;
    // signed distance at the nodes: start with "far" (± band by the inside flag)
    let mut d = vec![0f32; gw * gh];
    let mut cross = Vec::new();
    for j in 0..gh {
        crossings(&poly, node(j), &mut cross);
        let mut k = 0usize;
        let mut wind = 0i32;
        for i in 0..gw {
            let x = node(i);
            while k < cross.len() && cross[k].0 <= x {
                wind += cross[k].1;
                k += 1;
            }
            d[j * gw + i] = if wind != 0 { band as f32 } else { -band as f32 };
        }
    }
    // the band: each edge visits the nodes of its own expanded box, keeping the nearest
    let n = poly.len();
    let mut near = vec![f64::INFINITY; gw * gh];
    for e in 0..n {
        let (a, b) = (poly[e], poly[(e + 1) % n]);
        let to_i = |v: f64| ((v / s) - 0.5).floor();
        let (x0, x1) = (to_i(a.0.min(b.0) - band).max(0.0), to_i(a.0.max(b.0) + band) + 1.0);
        let (y0, y1) = (to_i(a.1.min(b.1) - band).max(0.0), to_i(a.1.max(b.1) + band) + 1.0);
        if x1 < 0.0 || y1 < 0.0 {
            continue;
        }
        let (x0, x1) = (x0 as usize, (x1 as usize).min(gw - 1));
        let (y0, y1) = (y0 as usize, (y1 as usize).min(gh - 1));
        for j in y0..=y1 {
            for i in x0..=x1 {
                let q = seg_dist2((node(i), node(j)), a, b);
                let c = &mut near[j * gw + i];
                if q < *c {
                    *c = q;
                }
            }
        }
    }
    for (v, q) in d.iter_mut().zip(&near) {
        if q.is_finite() {
            let dist = q.sqrt().min(band);
            *v = if *v > 0.0 { dist as f32 } else { -dist as f32 };
        }
    }
    // the coverage per pixel
    let smooth = m.falloff == Falloff::Smooth;
    let cov_of = |dd: f64| {
        let t = (0.5 + (dd + expansion) / ramp).clamp(0.0, 1.0);
        if smooth { t * t * (3.0 - 2.0 * t) } else { t }
    };
    for y in 0..hu {
        let row = &mut out[y * wu..(y + 1) * wu];
        if si == 1 {
            let dr = &d[y * gw..y * gw + gw];
            for (x, o) in row.iter_mut().enumerate() {
                *o = cov_of(dr[x] as f64) as f32;
            }
            continue;
        }
        let gy = (y as f64 + 0.5) / s - 0.5;
        let j0 = (gy.floor().max(0.0) as usize).min(gh - 2);
        let fy = (gy - j0 as f64).clamp(0.0, 1.0);
        for (x, o) in row.iter_mut().enumerate() {
            let gx = (x as f64 + 0.5) / s - 0.5;
            let i0 = (gx.floor().max(0.0) as usize).min(gw - 2);
            let fx = (gx - i0 as f64).clamp(0.0, 1.0);
            let at = |i: usize, j: usize| d[j * gw + i] as f64;
            let top = at(i0, j0) + (at(i0 + 1, j0) - at(i0, j0)) * fx;
            let bot = at(i0, j0 + 1) + (at(i0 + 1, j0 + 1) - at(i0, j0 + 1)) * fx;
            *o = cov_of(top + (bot - top) * fy) as f32;
        }
    }
    out
}

/// Rasterize an ordered mask list (already evaluated at the frame's tick) on a `w × h` raster.
/// The modes are the soft Boolean forms, top to bottom; the start is empty, or full when the
/// first mask subtracts or intersects.
pub fn rasterize(masks: &[Mask], sp: &MaskSpace, w: u32, h: u32) -> Coverage {
    let len = w as usize * h as usize;
    let full = masks.first().is_some_and(|m| matches!(m.mode, MaskMode::Subtract | MaskMode::Intersect));
    let mut acc = vec![if full { 1f32 } else { 0f32 }; len];
    for m in masks {
        let c = one_mask(m, sp, w, h);
        let op = m.opacity.clamp(0.0, 1.0) as f32;
        for (a, v) in acc.iter_mut().zip(c) {
            let mut v = if m.invert { 1.0 - v } else { v };
            v *= op;
            *a = match m.mode {
                MaskMode::Add => *a + v - *a * v,
                MaskMode::Subtract => *a * (1.0 - v),
                MaskMode::Intersect => *a * v,
                MaskMode::Difference => *a + v - 2.0 * *a * v,
            };
        }
    }
    let cov = acc.iter().map(|a| (a.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8).collect();
    Coverage { w, h, cov }
}

/// The memo of the last coverages (the owner's rule "caches where they pay"): keyed by the
/// FULL description of what decides the bytes (the static masks, the space, the size), never
/// by a hash that could collide; bounded; invisible to the bytes (t1269).
const MEMO_CAP: usize = 16;
type MemoEntry = (String, Arc<Coverage>);
static MEMO: Mutex<Vec<MemoEntry>> = Mutex::new(Vec::new());

/// `rasterize` through the memo: a static mask on a playing event is rasterized once.
pub fn rasterize_cached(masks: &[Mask], sp: &MaskSpace, w: u32, h: u32) -> Arc<Coverage> {
    let key = format!("{masks:?}|{sp:?}|{w}x{h}");
    if let Ok(mut memo) = MEMO.lock()
        && let Some(i) = memo.iter().position(|(k, _)| *k == key)
    {
        let hit = memo.remove(i);
        let c = hit.1.clone();
        memo.push(hit);
        return c;
    }
    let c = Arc::new(rasterize(masks, sp, w, h));
    if let Ok(mut memo) = MEMO.lock() {
        if memo.len() >= MEMO_CAP {
            memo.remove(0);
        }
        memo.push((key, c.clone()));
    }
    c
}

/// What a mask application point does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskOpKind {
    /// The picture's or the track's masks: the frame's alpha × coverage.
    Alpha,
    /// An effect slot's masks: the frame at chain position `from` (the slot's input) is the dry
    /// side, the frame here (after the slot) the wet one, mixed per pixel by the coverage.
    Mix { from: usize },
}

/// One mask application point of a layer's chain, built by kt-engine when the layer is built
/// and EXECUTED by both backends. `at` — the chain position (applied before `fx[at]`; `fx.len()`
/// — after the whole chain). The masks are already evaluated at the frame's tick.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskOp {
    pub at: usize,
    pub kind: MaskOpKind,
    pub masks: Arc<Vec<Mask>>,
    /// The raster here is the canvas after the LEADING Geometry node: the path goes through
    /// its placement. Otherwise the raster is the reference frame itself.
    pub follow_lead: bool,
    /// The project's logical height — what one mask px is measured against (`px_of`).
    pub project_h: f64,
}

/// The chain positions mask ops cut the chain at (the dry snapshots and the application
/// points). Both executors feed the chain in whole slices between them.
pub fn mask_stops(ops: &[MaskOp]) -> Vec<usize> {
    let mut v: Vec<usize> = ops
        .iter()
        .flat_map(|o| match o.kind {
            MaskOpKind::Alpha => [o.at, o.at],
            MaskOpKind::Mix { from } => [from, o.at],
        })
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Where op `op` lands on a `raster` — ONE function for both executors. `in_logical` — the
/// chain input's logical size (the event's source frame at this quality, or the canvas of a
/// track), `out` — the logical output size (the canvas). A path that follows the leading node
/// goes through `geometry_placement`, the very function the composite and the R.64 frame use.
pub fn mask_space(
    op: &MaskOp,
    fx: &[kt_model::FxInstance],
    in_logical: (u32, u32),
    out: (u32, u32),
    raster: (u32, u32),
    lead: Option<crate::conform::LeadCtx>,
) -> MaskSpace {
    let (rw, rh) = (in_logical.0.max(1) as f64, in_logical.1.max(1) as f64);
    let node = op.follow_lead.then(|| {
        fx[..op.at.min(fx.len())]
            .iter()
            .rev()
            .find(|f| matches!(f, kt_model::FxInstance::Geometry { lead: true, .. }))
    });
    let canvas = (out.0.max(2) as f64, out.1.max(2) as f64);
    let placement =
        node.flatten().and_then(|f| crate::geom::geometry_placement(f, lead, (rw, rh), canvas));
    match placement {
        None => MaskSpace::plain(rw, rh, raster.0, raster.1, op.project_h),
        Some(pl) => {
            MaskSpace::placed(&pl, (rw, rh), canvas, (raster.0 as f64, raster.1 as f64), op.project_h)
        }
    }
}
