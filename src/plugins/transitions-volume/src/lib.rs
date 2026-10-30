//! The volumetric transition kinds as plugins (plugin-api-stage1, stage 5, D12): the 2.5D exchange,
//! the panels and the cylinders. Each kernel is the former `TransMix::of` arm (its `pack`),
//! `kernel()` arm (its `pixel`) and `trans_ext` arm (its `ext`: the panels' inverse homographies in
//! f64, rounded once to f32, so both backends read the same bits) of kt-engine's `transitions.rs`,
//! verbatim; its WGSL is the former `trans()` arm of composite.wgsl. t1019 holds the pixels against
//! the enum, t224 the GPU against the CPU (FrameTol::GEOM for the panels), t973 the file text.

use std::f32::consts::PI;
use std::sync::Arc;

use kt_plugin_api::trans::{
    Img, Kind, Row, axis_c, axis_sample, dir_vec, face_h_inv, flag, hpanel, kind, over_straight, shade3, smooth,
};
use kt_plugin_api::{ParamUnit, Plugin, Store};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(swap()),
        Arc::new(flip()),
        Arc::new(cube()),
        Arc::new(door()),
        Arc::new(page_peel()),
        Arc::new(page_roll()),
    ]
}

const MOTION: &str = "trn.cat.motion";
const VOLUME: &str = "trn.cat.volume";

fn norm(n: &'static str, d: f64) -> Row {
    smooth(n, 0.0, 1.0, d, 0.01, ParamUnit::Norm, Store::F64)
}

fn curl() -> Row {
    smooth("curl", 0.05, 0.45, 0.18, 0.01, ParamUnit::Norm, Store::F64)
}

/// The camera distance of a panel's perspective.
fn dist(persp: f32) -> f64 {
    1.5 / (persp as f64).max(0.05)
}

/// `vertical` (a flag), then two numbers: flip and door.
fn flag_two(v: &[f64]) -> [f32; 8] {
    let mut p = [0.0f32; 8];
    p[0] = flag(v[0]);
    p[1] = v[1] as f32;
    p[2] = v[2] as f32;
    p
}

/// `dir`, then two numbers: cube and the pages.
fn dir_two(v: &[f64]) -> [f32; 8] {
    let mut p = [0.0f32; 8];
    (p[0], p[1]) = dir_vec(v[0]);
    p[2] = v[1] as f32;
    p[3] = v[2] as f32;
    p
}

/// Exchange: the frames swap places along an arc, the order of the sides switches in the middle
/// (2.5D without depth).
pub fn swap() -> Kind {
    kind(
        "swap",
        "Swap",
        "trn.kind.swap",
        MOTION,
        false,
        vec![Row::Flag("vertical"), norm("depth", 0.3)],
        include_str!("swap.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            p[0] = flag(v[0]);
            p[1] = v[1] as f32;
            p
        },
        |a, b, u, v, k, p, _, _| {
            // 2.5D exchange: the sides travel along an arc, compressing inward; the order
            // switches at the midpoint — occlusion is analytic, without
            // depth (there is no depth buffer in the pipeline and none is needed)
            let vert = p[0] > 0.5;
            let wave = (PI * k).sin();
            let off = 0.3 * wave;
            let scale = (1.0 - p[1] * wave).max(0.05);
            let quad = |img: &Img<'_>, side: f32| -> [f32; 4] {
                let (qx, qy) = if vert { (0.5, 0.5 + side * off) } else { (0.5 + side * off, 0.5) };
                let su = (u - qx) / scale + 0.5;
                let sv = (v - qy) / scale + 0.5;
                if (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv) {
                    let mut c = img.sample(su, sv);
                    // the far side is slightly darker — the depth is read
                    let shade = 1.0 - 0.25 * wave;
                    c[0] *= shade;
                    c[1] *= shade;
                    c[2] *= shade;
                    c
                } else {
                    [0.0; 4]
                }
            };
            let sa = quad(a, 1.0);
            let sb = quad(b, -1.0);
            // k < 0.5 — the outgoing one is still ON TOP, further on — the incoming one
            let (top, bot) = if k < 0.5 { (sa, sb) } else { (sb, sa) };
            over_straight(top, bot)
        },
    )
}

/// Flip: the panel turns around its middle, on the back the reverse side is visible with the
/// incoming frame.
pub fn flip() -> Kind {
    kind(
        "flip",
        "Flip",
        "trn.kind.flip",
        VOLUME,
        false,
        vec![Row::Flag("vertical"), norm("persp", 0.5), norm("shade", 0.5)],
        include_str!("flip.wgsl"),
        flag_two,
        |a, b, u, v, _, p, _, ext| {
            // panel by the inverse homography from ext; on the back — the reverse side
            // with the incoming one (mirrored along the movement axis)
            let vert = p[0] > 0.5;
            let (sx, sy) = if vert { (v, u) } else { (u, v) };
            match hpanel(&ext[0..8], sx, sy) {
                Some((sp, t)) if (0.0..=1.0).contains(&sp) && (0.0..=1.0).contains(&t) => {
                    let front = ext[8] >= 0.0;
                    let ms = if front { sp } else { 1.0 - sp };
                    let (mu, mv) = if vert { (t, ms) } else { (ms, t) };
                    let img = if front { a } else { b };
                    shade3(img.sample(mu, mv), ext[9])
                }
                _ => [0.0; 4],
            }
        },
    )
    .with_ext(|p, k| {
        let mut e = [0.0f32; 16];
        let kf = k as f64 / 4096.0;
        let mut th = kf * std::f64::consts::PI;
        // the edge (θ=π/2) degenerates the panel into a line — a deterministic
        // ε-shift, identical on both backends by construction
        if (th - std::f64::consts::FRAC_PI_2).abs() < 1e-4 {
            th += 1e-3;
        }
        let (sn, cs) = th.sin_cos();
        let d = dist(p[1]);
        if let Some(h) = face_h_inv(0.5 - 0.5 * cs, cs, -0.5 * sn, sn, d) {
            e[..8].copy_from_slice(&h);
        }
        e[8] = if cs >= 0.0 { 1.0 } else { -1.0 };
        e[9] = (1.0 - p[2] as f64 * sn.abs()) as f32;
        e
    })
}

/// Cube: the outgoing and the incoming — two faces, the cube turns.
pub fn cube() -> Kind {
    kind(
        "cube",
        "Cube",
        "trn.kind.cube",
        VOLUME,
        false,
        vec![Row::Dir, norm("persp", 0.5), norm("shade", 0.5)],
        include_str!("cube.wgsl"),
        dir_two,
        |a, b, u, v, k, p, _, ext| {
            let vert = p[1].abs() > 0.5;
            let (sx, sy) = if vert { (v, u) } else { (u, v) };
            let sh = 1.0 - p[3] * (PI * k).sin() * 0.5;
            let pick = |h: &[f32], img: &Img<'_>| -> Option<[f32; 4]> {
                match hpanel(h, sx, sy) {
                    Some((sp, t)) if (0.0..=1.0).contains(&sp) && (0.0..=1.0).contains(&t) => {
                        let (mu, mv) = if vert { (t, sp) } else { (sp, t) };
                        Some(shade3(img.sample(mu, mv), sh))
                    }
                    _ => None,
                }
            };
            // the faces of the cube do not intersect the interior — the order is fixed
            pick(&ext[8..16], b).or_else(|| pick(&ext[0..8], a)).unwrap_or([0.0; 4])
        },
    )
    .with_ext(|p, k| {
        let mut e = [0.0f32; 16];
        let kf = k as f64 / 4096.0;
        // sign of the rotation by direction: the content moves TOWARD dir
        let sgn = if p[0].abs() > 0.5 { -p[0] as f64 } else { -p[1] as f64 };
        let d = dist(p[2]);
        let face = |phi: f64| {
            let (sn, cs) = phi.sin_cos();
            face_h_inv(0.5 - 0.5 * cs - 0.5 * sn, cs, 0.5 + 0.5 * sn - 0.5 * cs, -sn, d)
        };
        let phi_a = sgn * kf * std::f64::consts::FRAC_PI_2;
        let phi_b = phi_a - sgn * std::f64::consts::FRAC_PI_2;
        if let Some(h) = face(phi_a) {
            e[..8].copy_from_slice(&h);
        }
        if let Some(h) = face(phi_b) {
            e[8..16].copy_from_slice(&h);
        }
        e
    })
}

/// Doors: two leaves of the outgoing one swing open inward, behind them the incoming one.
pub fn door() -> Kind {
    kind(
        "door",
        "Doors",
        "trn.kind.door",
        VOLUME,
        false,
        vec![Row::Flag("vertical"), norm("persp", 0.5), norm("shade", 0.5)],
        include_str!("door.wgsl"),
        flag_two,
        |a, b, u, v, _, p, _, ext| {
            let vert = p[0] > 0.5;
            let (sx, sy) = if vert { (v, u) } else { (u, v) };
            let sh = ext[8];
            let door = |x: f32, mirror: bool| -> Option<[f32; 4]> {
                match hpanel(&ext[0..8], x, sy) {
                    Some((sp, t)) if (0.0..=0.5).contains(&sp) && (0.0..=1.0).contains(&t) => {
                        let ms = if mirror { 1.0 - sp } else { sp };
                        let (mu, mv) = if vert { (t, ms) } else { (ms, t) };
                        Some(shade3(a.sample(mu, mv), sh))
                    }
                    _ => None,
                }
            };
            door(sx, false).or_else(|| door(1.0 - sx, true)).unwrap_or_else(|| b.sample(u, v))
        },
    )
    .with_ext(|p, k| {
        let mut e = [0.0f32; 16];
        let kf = k as f64 / 4096.0;
        let th = kf * (std::f64::consts::FRAC_PI_2 - 1e-3);
        let (sn, cs) = th.sin_cos();
        let d = dist(p[1]);
        // left leaf: hinge at the edge, it swings open inward;
        // the right one — its mirror, computed in the core
        if let Some(h) = face_h_inv(0.0, cs, 0.0, sn, d) {
            e[..8].copy_from_slice(&h);
        }
        e[8] = (1.0 - p[2] as f64 * sn) as f32;
        e
    })
}

/// Page turn: the sheet wraps back around a cylinder, its back-side tail is visible; under the
/// sheet the incoming one opens.
pub fn page_peel() -> Kind {
    kind(
        "page_peel",
        "Page peel",
        "trn.kind.page_peel",
        VOLUME,
        false,
        vec![Row::Dir, curl(), norm("shadow", 0.5)],
        include_str!("page_peel.wgsl"),
        dir_two,
        |a, b, u, v, k, p, _, _| {
            // the sheet curls back around a cylinder of radius r: the reverse-side tail
            // flies over the part still lying, under the lifted one — the incoming
            let (dx, dy) = (p[0], p[1]);
            let r = p[2].max(0.01);
            let shadow = p[3];
            let c = axis_c(u, v, dx, dy);
            let f = 1.0 - k * (1.0 + PI * r);
            let back = 1.0 - shadow * 0.35;
            // 1. flat tail of the reverse side (θ > π): x = 2f + πr − c
            let m_tail = 2.0 * f + PI * r - c;
            if c <= f && m_tail >= f + PI * r && m_tail <= 1.0 {
                return shade3(axis_sample(a, u, v, dx, dy, m_tail), back);
            }
            // 2. the cylinder over [f, f+r]
            let dc = c - f;
            if (0.0..=r).contains(&dc) {
                let th1 = dc.atan2((r * r - dc * dc).max(0.0).sqrt());
                let th2 = PI - th1;
                let m2 = f + r * th2;
                if m2 <= 1.0 {
                    return shade3(axis_sample(a, u, v, dx, dy, m2), back);
                }
                let m1 = f + r * th1;
                if m1 <= 1.0 {
                    let sh = 1.0 - shadow * 0.3 * th1.sin();
                    return shade3(axis_sample(a, u, v, dx, dy, m1), sh);
                }
            }
            // 3. base: the lying sheet or an open incoming with a shadow at the front
            if c <= f {
                a.sample(u, v)
            } else {
                let sh = 1.0 - shadow * 0.5 * (1.0 - (dc / r.max(1e-4)).min(1.0)).max(0.0);
                shade3(b.sample(u, v), sh)
            }
        },
    )
}

/// Scroll: the sheet winds onto a roll, the top coil is visible.
pub fn page_roll() -> Kind {
    kind(
        "page_roll",
        "Page roll",
        "trn.kind.page_roll",
        VOLUME,
        false,
        vec![Row::Dir, curl(), norm("shadow", 0.5)],
        include_str!("page_roll.wgsl"),
        dir_two,
        |a, b, u, v, k, p, _, _| {
            // a roll of radius r rolls, the upper turn of the reverse side is visible outside
            let (dx, dy) = (p[0], p[1]);
            let r = p[2].max(0.01);
            let shadow = p[3];
            let c = axis_c(u, v, dx, dy);
            let f = 1.0 - k * (1.0 + 2.0 * r);
            let back = 1.0 - shadow * 0.35;
            let dc = f - c;
            if dc.abs() <= r {
                // front (c < f): the top point of the turn θ₂ = π − asin;
                // back (c > f): continuation of the turn θ₃ = π + asin
                let ad = dc.abs();
                let th1 = ad.atan2((r * r - ad * ad).max(0.0).sqrt());
                let th = if dc >= 0.0 { PI - th1 } else { PI + th1 };
                let mw = f + r * th;
                if mw <= 1.0 {
                    let sh = back * (1.0 - 0.2 * (1.0 - th.sin().abs()));
                    return shade3(axis_sample(a, u, v, dx, dy, mw), sh);
                }
                let m1 = f + r * th1;
                if dc >= 0.0 && m1 <= 1.0 {
                    return shade3(axis_sample(a, u, v, dx, dy, m1), back);
                }
            }
            if c <= f {
                a.sample(u, v)
            } else {
                let sh = 1.0 - shadow * 0.5 * (1.0 - ((c - f) / r.max(1e-4)).min(1.0)).max(0.0);
                shade3(b.sample(u, v), sh)
            }
        },
    )
}
