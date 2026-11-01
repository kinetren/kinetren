//! The stylised transition kinds as plugins (plugin-api-stage1, stage 5, D12): the smears, the
//! digital ones and the light. Each kernel is the former `TransMix::of` arm (its `pack`) and
//! `kernel()` arm (its `pixel`) of kt-engine's `transitions.rs`, verbatim; its WGSL is the former
//! `trans()` arm of composite.wgsl. t1019 holds the pixels against the enum, t224 the GPU against
//! the CPU, t973 the file text.

use kt_plugin_api::math::FrozenMath;
use std::f32::consts::PI;
use std::sync::Arc;

use kt_plugin_api::trans::{
    Img, Kind, Row, box9, count, dir_vec, direct, front_mask, hash01, kind, mix4, quant, rgb, smooth, soft, value_noise,
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

/// The taps of the smears (whip, zoom blur).
const TAPS: usize = 7;

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
        },
        |a, b, u, v, k, p, _, _| {
            // a flyover like push, but with blur along the motion; the blur strength is
            // maximal in the middle of the gesture
            let (dx, dy) = (p[0], p[1]);
            let blur = p[2] * (PI * k).fsin();
            let mut acc = [0.0f32; 4];
            for i in 0..TAPS {
                let o = (i as f32 / (TAPS - 1) as f32 - 0.5) * blur * 0.25;
                let (su, sv) = (u + dx * o, v + dy * o);
                let (au, av) = (su - dx * k, sv - dy * k);
                let c = if (0.0..1.0).contains(&au) && (0.0..1.0).contains(&av) {
                    a.sample(au, av)
                } else {
                    // B behind the outgoing A — `+dir`, as with push (item 396)
                    b.sample(au + dx, av + dy)
                };
                for ch in 0..4 {
                    acc[ch] += c[ch];
                }
            }
            for ch in acc.iter_mut() {
                *ch /= TAPS as f32;
            }
            acc
        },
    )
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
        direct,
        |a, b, u, v, k, p, cx, _| {
            // the radius grows toward the middle of the transition and goes out there
            let r = p[0] * (PI * k).fsin();
            let sa = box9(a, u, v, r / cx.w, r / cx.h);
            let sb = box9(b, u, v, r / cx.w, r / cx.h);
            mix4(sa, sb, k)
        },
    )
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
        direct,
        |a, b, u, v, k, p, cx, _| {
            let s = (p[0] * (PI * k).fsin()).max(1.0);
            let (bu, bv) = (quant(u, s / cx.w), quant(v, s / cx.h));
            mix4(a.sample(bu, bv), b.sample(bu, bv), k)
        },
    )
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
        direct,
        |a, b, u, v, k, p, cx, _| {
            let amt = p[0] * (PI * k).fsin();
            // the bands of rows travel horizontally, the channels diverge
            let band = (v * 24.0).floor() as u32;
            let shift = (hash01(band.wrapping_mul(1973)) - 0.5) * amt * 0.25;
            let ch = amt * 8.0 / cx.w;
            let (su, sv) = (u + shift, v);
            let pick = |img: &Img<'_>| {
                let r = img.sample(su + ch, sv);
                let g = img.sample(su, sv);
                let bl = img.sample(su - ch, sv);
                [r[0], g[1], bl[2], g[3]]
            };
            mix4(pick(a), pick(b), k)
        },
    )
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
        direct,
        |a, b, u, v, k, p, _, _| {
            // frame jerks: the step index by progress, the frame is a cut at 0.5
            let steps = p[1].max(1.0);
            let i = ((k * steps) as u32).min(steps as u32 - 1);
            let amp = p[0] * 0.2 * (PI * k).fsin();
            let ox = (hash01(i.wrapping_mul(2).wrapping_add(1)) - 0.5) * amp;
            let oy = (hash01(i.wrapping_mul(2).wrapping_add(2)) - 0.5) * amp;
            let img = if k < 0.5 { a } else { b };
            img.sample(u + ox, v + oy)
        },
    )
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
        direct,
        |a, b, u, v, k, p, cx, _| {
            // the front by coarse noise, at the edge — a hot admixture of colour;
            // the contribution is gated by alpha (otherwise a solo fade would light up emptiness)
            let n = value_noise(u * p[0] * cx.aspect, v * p[0]);
            let mask = front_mask(n, k, p[1].max(0.02));
            let base = mix4(a.sample(u, v), b.sample(u, v), mask);
            let rim = (mask * (1.0 - mask) * 4.0).clamp(0.0, 1.0);
            let gate = rim * base[3] / 255.0;
            [
                base[0] + (p[2] - base[0]) * gate,
                base[1] + (p[3] - base[1]) * gate,
                base[2] + (p[4] - base[2]) * gate,
                base[3],
            ]
        },
    )
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
        direct,
        |a, b, u, v, k, p, cx, _| {
            // two coloured spots fly along the diagonals over the dissolve
            let aspect = cx.aspect;
            let base = mix4(a.sample(u, v), b.sample(u, v), k);
            let wave = (PI * k).fsin();
            let blob = |bx: f32, by: f32| -> f32 {
                let dx = (u - bx) * aspect;
                let dy = v - by;
                let d = (dx * dx + dy * dy).sqrt() / 0.45;
                let w = (1.0 - d).max(0.0);
                w * w
            };
            let w = (blob(k * 1.2 - 0.1, 0.3 + 0.4 * k) + blob(1.1 - 1.2 * k, 0.7 - 0.4 * k)) * p[0] * wave;
            let gate = w.clamp(0.0, 1.0) * base[3] / 255.0;
            [
                base[0] + (p[1] - base[0]) * gate,
                base[1] + (p[2] - base[1]) * gate,
                base[2] + (p[3] - base[2]) * gate,
                base[3],
            ]
        },
    )
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
        direct,
        |a, b, u, v, k, p, _, _| {
            // rays from the centre: 7 taps along the ray, the side — a cut at 0.5
            let (ox, oy) = (p[1], p[2]);
            let amt = p[0] * (PI * k).fsin();
            let img = if k < 0.5 { a } else { b };
            let mut acc = [0.0f32; 4];
            for i in 0..TAPS {
                let z = 1.0 + amt * 0.6 * (i as f32 / (TAPS - 1) as f32);
                let c = img.sample((u - ox) / z + ox, (v - oy) / z + oy);
                for ch in 0..4 {
                    acc[ch] += c[ch];
                }
            }
            for ch in acc.iter_mut() {
                *ch /= TAPS as f32;
            }
            acc
        },
    )
}
