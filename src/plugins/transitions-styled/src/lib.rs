//! The stylised transition kinds as plugins (plugin-api-stage1, stage 5, D12): the smears, the
//! digital ones and the light. Each kernel is the former `TransMix::of` arm (its `pack`) and
//! `kernel()` arm (its `pixel`) of kt-engine's `transitions.rs`, verbatim; its WGSL is the former
//! `trans()` arm of composite.wgsl. t1019 holds the pixels against the enum, t224 the GPU against
//! the stored frames, t973 the file text.

use std::sync::Arc;

use kt_plugin_api::trans::{
    Kind, Row, count, dir_vec, direct, kind, rgb, smooth, soft,
};
use kt_plugin_api::{ParamUnit, Plugin, Store};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(whip()),
        Arc::new(blur_dissolve()),
        Arc::new(pixelate()),
        Arc::new(glitch()),
        Arc::new(shake()),
        Arc::new(film_burn()),
        Arc::new(light_leaks()),
        Arc::new(zoom_blur()),
    ]
}

const MOTION: &str = "trn.cat.motion";
const STYLE: &str = "trn.cat.style";

fn norm(n: &'static str, d: f64) -> Row {
    smooth(n, 0.0, 1.0, d, 0.01, ParamUnit::Norm, Store::F64)
}

/// A sharp camera pass with a smear: a flyover like push, the blur maximal in the middle.
pub fn whip() -> Kind {
    kind(
        "whip",
        "Whip pan",
        "trn.kind.whip",
        MOTION,
        false,
        vec![Row::Dir, norm("blur", 0.6)],
        include_str!("whip.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            (p[0], p[1]) = dir_vec(v[0]);
            p[2] = v[1] as f32;
            p
        })
}

/// Dissolve through the blur of both frames.
pub fn blur_dissolve() -> Kind {
    kind(
        "blur_dissolve",
        "Blur dissolve",
        "trn.kind.blur",
        STYLE,
        true,
        vec![smooth("amount", 0.0, 64.0, 24.0, 1.0, ParamUnit::Px, Store::F64)],
        include_str!("blur_dissolve.wgsl"),
        direct)
}

/// Dissolve through enlarging the pixels.
pub fn pixelate() -> Kind {
    kind(
        "pixelate",
        "Pixelate",
        "trn.kind.pixelate",
        STYLE,
        true,
        vec![smooth("amount", 1.0, 256.0, 48.0, 1.0, ParamUnit::Px, Store::F64)],
        include_str!("pixelate.wgsl"),
        direct)
}

/// A digital glitch: row shifts and channel divergence.
pub fn glitch() -> Kind {
    kind(
        "glitch",
        "Glitch",
        "trn.kind.glitch",
        STYLE,
        true,
        vec![norm("amount", 0.5)],
        include_str!("glitch.wgsl"),
        direct)
}

/// Shake: jerks of the frame, a change of side in the middle.
pub fn shake() -> Kind {
    kind(
        "shake",
        "Shake",
        "trn.kind.shake",
        STYLE,
        false,
        vec![norm("amount", 0.5), count("steps", 1.0, 30.0, 12.0)],
        include_str!("shake.wgsl"),
        direct)
}

/// Film burn: a front by large noise with a hot edge.
pub fn film_burn() -> Kind {
    kind(
        "film_burn",
        "Film burn",
        "trn.kind.film_burn",
        STYLE,
        false,
        vec![
            smooth("scale", 1.0, 24.0, 6.0, 0.5, ParamUnit::Ratio, Store::F64),
            soft(0.2),
            rgb("tint_r", 255.0),
            rgb("tint_g", 120.0),
            rgb("tint_b", 0.0),
        ],
        include_str!("film_burn.wgsl"),
        direct)
}

/// Light flares: coloured spots fly across the frame over the dissolve.
pub fn light_leaks() -> Kind {
    kind(
        "light_leaks",
        "Light leaks",
        "trn.kind.light_leaks",
        STYLE,
        false,
        vec![norm("strength", 0.6), rgb("tint_r", 255.0), rgb("tint_g", 90.0), rgb("tint_b", 40.0)],
        include_str!("light_leaks.wgsl"),
        direct)
}

/// Radial smear: rays from the centre, a change of side in the middle (the strength of the rays
/// sin(πk) and the cut at 0.5 are symmetric).
pub fn zoom_blur() -> Kind {
    kind(
        "zoom_blur",
        "Zoom blur",
        "trn.kind.zoom_blur",
        STYLE,
        true,
        vec![norm("amount", 0.6), Row::Center("cx"), Row::Center("cy")],
        include_str!("zoom_blur.wgsl"),
        direct)
}
