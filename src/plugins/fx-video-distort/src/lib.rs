//! Distorting video effects as plugins (plugin-api-stage1, stage 4, D11.2): the five warps —
//! lens_distort, twirl, ripple, kaleidoscope, cylinder3d. Each is a geometry program of one `Warp`
//! step: the inverse map of the normalized centered space, cooked once, and its WGSL mirror that
//! kt-gpu pastes into both primitive loops of the fused pass. The steps, the clamps and the
//! "neutral values skip the step" rule are the former builder's, verbatim (t328's goldens, t330).

use std::sync::Arc;

use kt_plugin_api::{
    Decl, GeoCtx, GeoKernel, GeoStep, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin,
    RowDecl, Store, UiDecl, VideoClass, WarpNorm,
};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(Warp::lens_distort()),
        Arc::new(Warp::twirl()),
        Arc::new(Warp::ripple()),
        Arc::new(Warp::kaleidoscope()),
        Arc::new(Warp::cylinder3d()),
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
