//! Distorting video effects as plugins (plugin-api-stage1, stage 4, D11.2): the five warps —
//! lens_distort, twirl, ripple, kaleidoscope, cylinder3d — and corner_pin, one `Proj` step (its
//! homography; outside the quad transparent), mirror and event_grid (the host's own steps, the
//! mirror's exact lone path, D11.3). Each warp is a geometry program of one `Warp`
//! step: the inverse map of the normalized centered space, cooked once, and its WGSL mirror that
//! kt-gpu pastes into both primitive loops of the fused pass. The steps, the clamps and the
//! "neutral values skip the step" rule are the former builder's, verbatim (t328's goldens, t330).

use std::sync::Arc;

mod pan3d;

use kt_plugin_api::{
    geo, Decl, GeoCtx, GeoKernel, GeoStep, Gpu, Lone, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin,
    RowDecl, RowRole, Store, UiDecl, VideoClass, WarpNorm,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(Warp::lens_distort()),
        Arc::new(Warp::twirl()),
        Arc::new(Warp::ripple()),
        Arc::new(Warp::kaleidoscope()),
        Arc::new(Warp::cylinder3d()),
        Arc::new(CornerPin::new()),
        Arc::new(HostStep::mirror()),
        Arc::new(HostStep::event_grid()),
        Arc::new(Pan3d::new()),
    ]
}

/// Which map a warp plugin is: the five differ in their numbers and their f64 text only.
#[derive(Clone, Copy)]
enum Map {
    Lens,
    Twirl,
    Ripple,
    Kaleidoscope,
    Cylinder,
}

/// One warp plugin.
pub struct Warp {
    m: Manifest,
    map: Map,
}

fn row(store: Store, omit_default: bool) -> RowDecl {
    RowDecl { store, omit_default }
}

fn smooth(n: &'static str, lo: f64, hi: f64, d: f64, st: f64, u: ParamUnit) -> ParamSpec {
    ParamSpec::num(n, lo, hi, d, st, u, ParamKind::Smooth)
}

impl Warp {
    fn new(
        id: &'static str,
        map: Map,
        passport: Vec<ParamSpec>,
        rows: Vec<RowDecl>,
        wgsl: &'static str,
        ui: UiDecl,
    ) -> Warp {
        Warp {
            m: Manifest {
                id: id.into(),
                media: Media::VideoFx,
                passport,
                rows,
                decl: Decl { video_class: Some(VideoClass::Geometry), gpu: Gpu::Warp(wgsl), ..Decl::default() },
                ui,
            },
            map,
        }
    }

    pub fn lens_distort() -> Warp {
        Warp::new(
            "lens_distort",
            Map::Lens,
            vec![smooth("k1", -1.0, 1.0, 0.35, 0.01, ParamUnit::Norm), smooth("k2", -1.0, 1.0, 0.0, 0.01, ParamUnit::Norm)],
            vec![row(Store::F32, false), row(Store::F32, true)],
            include_str!("lens_distort.wgsl"),
            UiDecl { name: "Lens Distort", category_key: "fxreg.cat.distort", keywords_key: "fxreg.kw.lens_distort" },
        )
    }

    pub fn twirl() -> Warp {
        Warp::new(
            "twirl",
            Map::Twirl,
            vec![smooth("angle", -720.0, 720.0, 120.0, 1.0, ParamUnit::Deg), smooth("radius", 0.1, 1.5, 0.8, 0.01, ParamUnit::Norm)],
            vec![row(Store::F32, false), row(Store::F32, false)],
            include_str!("twirl.wgsl"),
            UiDecl { name: "Twirl", category_key: "fxreg.cat.distort", keywords_key: "fxreg.kw.twirl" },
        )
    }

    pub fn ripple() -> Warp {
        Warp::new(
            "ripple",
            Map::Ripple,
            vec![
                smooth("amount", 0.0, 0.5, 0.08, 0.005, ParamUnit::Norm),
                smooth("freq", 1.0, 40.0, 10.0, 0.5, ParamUnit::Count),
                smooth("phase", 0.0, 3600.0, 0.0, 1.0, ParamUnit::Deg),
            ],
            vec![row(Store::F32, false), row(Store::F32, false), row(Store::F32, true)],
            include_str!("ripple.wgsl"),
            UiDecl { name: "Ripple", category_key: "fxreg.cat.distort", keywords_key: "fxreg.kw.ripple" },
        )
    }

    pub fn kaleidoscope() -> Warp {
        Warp::new(
            "kaleidoscope",
            Map::Kaleidoscope,
            vec![smooth("segments", 2.0, 32.0, 6.0, 1.0, ParamUnit::Count), smooth("angle", -180.0, 180.0, 0.0, 1.0, ParamUnit::Deg)],
            vec![row(Store::U32, false), row(Store::F32, true)],
            include_str!("kaleidoscope.wgsl"),
            UiDecl { name: "Kaleidoscope", category_key: "fxreg.cat.distort", keywords_key: "fxreg.kw.kaleidoscope" },
        )
    }

    pub fn cylinder3d() -> Warp {
        Warp::new(
            "cylinder3d",
            Map::Cylinder,
            vec![smooth("curvature", 0.0, 1.0, 0.5, 0.01, ParamUnit::Norm)],
            vec![row(Store::F32, false)],
            include_str!("cylinder3d.wgsl"),
            UiDecl { name: "Cylinder 3D", category_key: "fxreg.cat.three_d", keywords_key: "fxreg.kw.cylinder3d" },
        )
    }
}

impl Plugin for Warp {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn geo(&self) -> Option<&dyn GeoKernel> {
        Some(self)
    }
}

impl GeoKernel for Warp {
    /// The former builder's arm, verbatim: the clamps, the cooked numbers and the neutral skip.
    fn steps(&self, p: &[f64], _cx: &GeoCtx) -> Vec<GeoStep> {
        let g = |i: usize| p.get(i).copied().unwrap_or(0.0);
        // the values are the f32 fields' values; the variant cast `*x as f64` from the f32
        let short = WarpNorm::ShortSemiAxis;
        match self.map {
            Map::Lens => {
                let (k1, k2) = (g(0).clamp(-1.0, 1.0), g(1).clamp(-1.0, 1.0));
                if k1 != 0.0 || k2 != 0.0 {
                    return vec![GeoStep::Warp { k: [k1, k2, 0.0, 0.0, 0.0, 0.0, 0.0], norm: short }];
                }
            }
            Map::Twirl => {
                let a = g(0).clamp(-720.0, 720.0).to_radians();
                let rad = g(1).clamp(0.1, 1.5);
                if a != 0.0 {
                    return vec![GeoStep::Warp { k: [a, rad, 0.0, 0.0, 0.0, 0.0, 0.0], norm: short }];
                }
            }
            Map::Ripple => {
                let amp = g(0).clamp(0.0, 0.5);
                let k = 2.0 * std::f64::consts::PI * g(1).clamp(1.0, 40.0);
                let ph = g(2).to_radians();
                if amp != 0.0 {
                    return vec![GeoStep::Warp { k: [amp, k, ph, 0.0, 0.0, 0.0, 0.0], norm: short }];
                }
            }
            Map::Kaleidoscope => {
                let segments = (g(0) as u32).clamp(2, 32);
                let tau = 2.0 * std::f64::consts::PI / segments as f64;
                let a0 = g(1).clamp(-180.0, 180.0).to_radians();
                return vec![GeoStep::Warp { k: [tau, a0, 0.0, 0.0, 0.0, 0.0, 0.0], norm: short }];
            }
            Map::Cylinder => {
                let curv = g(0).clamp(0.0, 1.0);
                if curv >= 1e-3 {
                    let a = curv * std::f64::consts::FRAC_PI_2 * 0.999;
                    // the cylinder is normalized by X on w/2
                    return vec![GeoStep::Warp { k: [a.sin(), 1.0 / a, 0.0, 0.0, 0.0, 0.0, 0.0], norm: WarpNorm::HalfWidth }];
                }
            }
        }
        Vec::new()
    }

    /// Inverse map of the nonlinear warp (R.47): ONE f64 text per kind, read by both CPU loops;
    /// the WGSL mirror is the kind's `warp_<id>`. Input/output — the normalized centered space.
    #[inline]
    fn warp(&self, p: &[f64; 7], x: f64, y: f64) -> (f64, f64) {
        match self.map {
            Map::Lens => {
                // lens: polynomial s = 1 + k1·r² + k2·r⁴ (no trigonometry)
                let r2 = x * x + y * y;
                let s = 1.0 + p[0] * r2 + p[1] * r2 * r2;
                (x * s, y * s)
            }
            Map::Twirl => {
                // twirl: rotation by a·(1 − r/rad)² inside the radius
                let r = (x * x + y * y).sqrt();
                if r >= p[1] {
                    return (x, y);
                }
                let t = 1.0 - r / p[1];
                let (sn, cs) = (p[0] * t * t).sin_cos();
                (x * cs - y * sn, x * sn + y * cs)
            }
            Map::Ripple => {
                // ripple: r' = r + amp·sin(k·r − φ)
                let r = (x * x + y * y).sqrt();
                if r <= 1e-9 {
                    return (x, y);
                }
                let k = (r + p[0] * (p[1] * r - p[2]).sin()) / r;
                (x * k, y * k)
            }
            Map::Kaleidoscope => {
                // kaleidoscope: triangle wave over the sector (mirror
                // repetition without int modulo — floor is the same in both texts)
                let r = (x * x + y * y).sqrt();
                let q = (y.atan2(x) - p[1]) / p[0];
                let h = q / 2.0 - (q / 2.0).floor();
                let m = 1.0 - (2.0 * h - 1.0).abs();
                let (sn, cs) = (p[1] + m * p[0]).sin_cos();
                (r * cs, r * sn)
            }
            Map::Cylinder => {
                // cylinder: u' = asin(u·sin a)/a along X, edges → edges
                let u = (x * p[0]).clamp(-1.0, 1.0).asin() * p[1];
                (u, y)
            }
        }
    }
}

/// Four-corner fit (R.47, F-FX-004): the homography quad → quad, the main tool of "the screen in
/// the frame". A geometry plugin of one `Proj` step; the corners at the corners skip the step.
pub struct CornerPin {
    m: Manifest,
}

impl CornerPin {
    pub fn new() -> CornerPin {
        let c = |n, d| ParamSpec::num(n, -1.0, 2.0, d, 0.005, ParamUnit::Norm, ParamKind::Smooth);
        let names = ["x0", "y0", "x1", "y1", "x2", "y2", "x3", "y3"];
        let defaults = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        CornerPin {
            m: Manifest {
                id: "corner_pin".into(),
                media: Media::VideoFx,
                passport: names.iter().zip(defaults).map(|(n, d)| c(*n, d)).collect(),
                rows: vec![row(Store::F64, true); 8],
                decl: Decl { video_class: Some(VideoClass::Geometry), gpu: Gpu::Primitives, ..Decl::default() },
                ui: UiDecl { name: "Corner Pin", category_key: "fxreg.cat.geometry", keywords_key: "fxreg.kw.corner_pin" },
            },
        }
    }
}

impl Default for CornerPin {
    fn default() -> CornerPin {
        CornerPin::new()
    }
}

impl Plugin for CornerPin {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn geo(&self) -> Option<&dyn GeoKernel> {
        Some(self)
    }
}

impl GeoKernel for CornerPin {
    fn steps(&self, p: &[f64], cx: &GeoCtx) -> Vec<GeoStep> {
        let g = |i: usize| p.get(i).copied().unwrap_or(0.0);
        // default is bit-exact: corners at the corners — the step is skipped,
        // the chain stays identity (identity byte-for-byte)
        let ident = g(0) == 0.0
            && g(1) == 0.0
            && g(2) == 1.0
            && g(3) == 0.0
            && g(4) == 0.0
            && g(5) == 1.0
            && g(6) == 1.0
            && g(7) == 1.0;
        if ident {
            return Vec::new();
        }
        let m = corner_pin_inverse([(g(0), g(1)), (g(2), g(3)), (g(4), g(5)), (g(6), g(7))], cx.out_w, cx.out_h);
        vec![GeoStep::Proj { m }]
    }
}

/// Pan3d (R.29): the layer as a flat quad in the 3D world, seen by the resolved camera — one `Proj`
/// step, the inverse of the forward homography. Without a camera there is no 3D (the step is
/// skipped); the defaults with the 50 mm camera are the identity byte for byte (t298).
pub struct Pan3d {
    m: Manifest,
}

impl Pan3d {
    pub fn new() -> Pan3d {
        let px = |n| smooth(n, -100_000.0, 100_000.0, 0.0, 1.0, ParamUnit::Px);
        let deg = |n| smooth(n, -1440.0, 1440.0, 0.0, 0.5, ParamUnit::Deg);
        Pan3d {
            m: Manifest {
                id: "pan3d".into(),
                media: Media::VideoFx,
                passport: vec![
                    px("pos_x"),
                    px("pos_y"),
                    px("pos_z"),
                    deg("rot_x"),
                    deg("rot_y"),
                    deg("rot_z"),
                    smooth("anchor_x", 0.0, 1.0, 0.5, 0.01, ParamUnit::Norm),
                    smooth("anchor_y", 0.0, 1.0, 0.5, 0.01, ParamUnit::Norm),
                    smooth("scale", 0.01, 32.0, 1.0, 0.01, ParamUnit::Ratio),
                    // None — the camera active at the time (default, as in AE); Some(id) — a
                    // pinned camera (there may be several cameras — R.29)
                    ParamSpec::ref_row("camera"),
                ],
                rows: {
                    let mut r = vec![row(Store::F64, false); 9];
                    r.push(row(Store::Ref(RowRole::Camera), true));
                    r
                },
                decl: Decl { video_class: Some(VideoClass::Geometry), gpu: Gpu::Primitives, ..Decl::default() },
                ui: UiDecl { name: "Pan 3D", category_key: "fxreg.cat.three_d", keywords_key: "fxreg.kw.pan3d" },
            },
        }
    }
}

impl Default for Pan3d {
    fn default() -> Pan3d {
        Pan3d::new()
    }
}

impl Plugin for Pan3d {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn geo(&self) -> Option<&dyn GeoKernel> {
        Some(self)
    }
}

impl GeoKernel for Pan3d {
    fn steps(&self, p: &[f64], cx: &GeoCtx) -> Vec<GeoStep> {
        // no camera — no 3D: the step is skipped entirely, sizes do not change
        let Some(c3) = cx.camera else { return Vec::new() };
        let g = |i: usize| p.get(i).copied().unwrap_or(0.0);
        let fwd = pan3d::pan3d_forward(
            &c3,
            (cx.out_w, cx.out_h),
            [g(0), g(1), g(2)],
            [g(3), g(4), g(5)],
            g(8),
            [g(6), g(7)],
            (cx.stage_w as f64, cx.stage_h as f64),
        );
        // degenerate homography (|det| ≈ 0, quad edge-on) — transparent layer: m with t.z ≡ −1
        vec![GeoStep::Proj { m: geo::invert3(&fwd).unwrap_or(geo::TRANSPARENT) }]
    }
}

/// A geometry plugin whose map is one of the host's own steps (mirror, event grid): it only
/// translates its values into the step.
pub struct HostStep {
    m: Manifest,
    kind: HostKind,
}

#[derive(Clone, Copy)]
enum HostKind {
    Mirror,
    Grid,
}

impl HostStep {
    /// Mirror (port of v1): a flip of the stage. Alone and without a scale change it keeps the
    /// exact permutation kernel (`Lone::Mirror`, t297).
    pub fn mirror() -> HostStep {
        HostStep {
            m: Manifest {
                id: "mirror".into(),
                media: Media::VideoFx,
                passport: vec![ParamSpec::flag_row("horizontal", true), ParamSpec::flag_row("vertical", false)],
                rows: vec![row(Store::Bool, false), row(Store::Bool, false)],
                decl: Decl { video_class: Some(VideoClass::Geometry), gpu: Gpu::Primitives, ..Decl::default() },
                ui: UiDecl { name: "Mirror", category_key: "fxreg.cat.geometry", keywords_key: "fxreg.kw.mirror" },
            },
            kind: HostKind::Mirror,
        }
    }

    /// EventGrid (port of v1): the event repeated in a grid of cells over the canvas.
    pub fn event_grid() -> HostStep {
        let count = |n| ParamSpec::num(n, 1.0, 512.0, 1.0, 1.0, ParamUnit::Count, ParamKind::Smooth);
        HostStep {
            m: Manifest {
                id: "event_grid".into(),
                media: Media::VideoFx,
                passport: vec![count("cols"), count("rows"), ParamSpec::flag_row("center", true)],
                rows: vec![row(Store::U32, false), row(Store::U32, false), row(Store::Bool, false)],
                decl: Decl { video_class: Some(VideoClass::Geometry), gpu: Gpu::Primitives, ..Decl::default() },
                ui: UiDecl { name: "EventGrid", category_key: "fxreg.cat.style", keywords_key: "fxreg.kw.eventgrid" },
            },
            kind: HostKind::Grid,
        }
    }
}

impl Plugin for HostStep {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn geo(&self) -> Option<&dyn GeoKernel> {
        Some(self)
    }
}

impl GeoKernel for HostStep {
    fn steps(&self, p: &[f64], _cx: &GeoCtx) -> Vec<GeoStep> {
        let flag = |i: usize| p.get(i).copied().unwrap_or(0.0) != 0.0;
        match self.kind {
            HostKind::Mirror => {
                let (h, v) = (flag(0), flag(1));
                if h || v { vec![GeoStep::Mirror { h, v }] } else { Vec::new() }
            }
            HostKind::Grid => {
                let n = |i: usize| p.get(i).copied().unwrap_or(1.0) as u32;
                vec![GeoStep::Grid { cols: n(0), rows: n(1), center: flag(2) }]
            }
        }
    }

    fn lone(&self, p: &[f64]) -> Option<Lone> {
        let flag = |i: usize| p.get(i).copied().unwrap_or(0.0) != 0.0;
        match self.kind {
            HostKind::Mirror => Some(Lone::Mirror { h: flag(0), v: flag(1) }),
            HostKind::Grid => None,
        }
    }
}

/// INVERSE homography of Corner Pin (R.47, F-FX-004): the forward map
/// “unit square → 4 corners in output fractions” is built in closed form
/// (Heckbert), scaled into the pixel center-space of the output, and inverted.
/// Degeneracy (zero denominator / non-invertible matrix) — a transparent
/// matrix `t.z = −1` (precedent: Pan3d behind the camera).
fn corner_pin_inverse(c: [(f64, f64); 4], out_w: u32, out_h: u32) -> [[f64; 3]; 3] {
    let transparent = geo::TRANSPARENT;
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
    match geo::invert3(&scaled) {
        Some(inv) => inv,
        None => transparent,
    }
}
