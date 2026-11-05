//! The volumetric transition kinds as plugins (plugin-api-stage1, stage 5, D12): the 2.5D exchange,
//! the panels and the cylinders. Each kernel is the former `TransMix::of` arm (its `pack`),
//! `kernel()` arm (its `pixel`) and `trans_ext` arm (its `ext`: the panels' inverse homographies in
//! f64, rounded once to f32, so both backends read the same bits) of kt-engine's `transitions.rs`,
//! verbatim; its WGSL is the former `trans()` arm of composite.wgsl. t1019 holds the pixels against
//! the enum, t224 the GPU against the stored frames (FrameTol::GEOM for the panels), t973 the file text.

use kt_plugin_api::math::FrozenMath;
use std::sync::Arc;

use kt_plugin_api::trans::{
    Kind, Row, dir_vec, face_h_inv, flag, kind, smooth,
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
        })
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
        flag_two)
    .with_ext(|p, k| {
        let mut e = [0.0f32; 16];
        let kf = k as f64 / 4096.0;
        let mut th = kf * std::f64::consts::PI;
        // the edge (θ=π/2) degenerates the panel into a line — a deterministic
        // ε-shift, identical on both backends by construction
        if (th - std::f64::consts::FRAC_PI_2).abs() < 1e-4 {
            th += 1e-3;
        }
        let (sn, cs) = th.fsin_cos();
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
        dir_two)
    .with_ext(|p, k| {
        let mut e = [0.0f32; 16];
        let kf = k as f64 / 4096.0;
        // sign of the rotation by direction: the content moves TOWARD dir
        let sgn = if p[0].abs() > 0.5 { -p[0] as f64 } else { -p[1] as f64 };
        let d = dist(p[2]);
        let face = |phi: f64| {
            let (sn, cs) = phi.fsin_cos();
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
        flag_two)
    .with_ext(|p, k| {
        let mut e = [0.0f32; 16];
        let kf = k as f64 / 4096.0;
        let th = kf * (std::f64::consts::FRAC_PI_2 - 1e-3);
        let (sn, cs) = th.fsin_cos();
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
        dir_two)
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
        dir_two)
}
