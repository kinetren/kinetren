//! Transition cores (R.33 C4): the CPU reference and packing of parameters for the GPU.
//!
//! Invariant 7 requires ONE composition code for the preview and export, and the backend
//! counts as execution. Therefore the kind of transition is turned into a `TransMix`
//! — a flat pair (mode, 8 numbers) — EXACTLY once, here; the CPU core below and the
//! WGSL mirror (`kt-gpu/src/shaders/composite.wgsl`) read the same numbers and repeat the same formulas. Any
//! edit of a formula must go into both files in one commit (t181/t182).
//!
//! The crossfade (`mode == 0`) does NOT enter here: it stays
//! on the integer path `dissolve_into`, byte-for-byte as before
//! (GOLDEN_VIDEO does not move).

use kt_media::VideoFrame;
use kt_model::{Dir, TransitionKind};
use kt_plugin_api::trans::{
    Img, axis_c, axis_sample, face_h_inv, hpanel, kf, mix4, over_straight, shade3,
};

/// Modes shared by CPU and WGSL. The value goes into the uniform, so the
/// numbers are frozen: add a new mode ONLY AT THE END.
pub const MODE_CROSSFADE: u32 = 0;
// 1-9 and 14-22 were the basic kinds, plugins since plugin-api-stage1 (D12).
// R.51 (item 400): patterns, shift-symbioses, stylized ones. The numbers are a contract
// ONLY between transitions.rs and composite.wgsl (edited in one commit):
// the project file receives tag(), the cache keys receive the ready-made TransMix.
pub const MODE_SWAP: u32 = 23;
// Volumetric (item 401): panels via inverse homographies in ext (see trans_ext),
// peel/roll cylinders — analytics right in the core.
pub const MODE_FLIP: u32 = 28;
pub const MODE_CUBE: u32 = 29;
pub const MODE_DOOR: u32 = 30;
pub const MODE_PEEL: u32 = 31;
pub const MODE_ROLL: u32 = 32;
/// A plugin kind's mode is `MODE_PLUGIN + its index` in the registry's transition list (D12) —
/// a function of the registry, never hashed into a key (`plan_key` carries the kind's id).
pub const MODE_PLUGIN: u32 = 64;

/// The installed transition kernels by their mode's index, resolved once (the registry does not
/// change after the install, and a pixel must not search it).
fn plugin_kernels() -> &'static [(&'static str, &'static dyn kt_plugin_api::TransKernel)] {
    static K: std::sync::OnceLock<Vec<(&'static str, &'static dyn kt_plugin_api::TransKernel)>> = std::sync::OnceLock::new();
    let build = || -> Vec<(&'static str, &'static dyn kt_plugin_api::TransKernel)> {
        kt_model::plugin_slot::registry()
            .transitions()
            .into_iter()
            .filter_map(|p| Some((&*p.manifest().id, p.trans()?)))
            .collect()
    };
    // a list built before the install would be empty for good: only a filled one is kept
    if let Some(k) = K.get() {
        return k;
    }
    let k = build();
    if k.is_empty() {
        return Box::leak(k.into_boxed_slice());
    }
    K.get_or_init(|| k)
}

/// The kernel of a plugin mode.
fn plugin_kernel(mode: u32) -> Option<&'static dyn kt_plugin_api::TransKernel> {
    plugin_kernels().get(mode.checked_sub(MODE_PLUGIN)? as usize).map(|(_, k)| *k)
}

/// The resolved transition for the executors: mode + 8 numbers (exactly the f0/f1
/// uniforms of the GPU). The fields have no semantics of their own — the cores below define it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransMix {
    pub mode: u32,
    pub p: [f32; 8],
}

impl Default for TransMix {
    fn default() -> TransMix {
        TransMix::CROSSFADE
    }
}

impl TransMix {
    pub const CROSSFADE: TransMix = TransMix { mode: MODE_CROSSFADE, p: [0.0; 8] };

    pub fn is_crossfade(&self) -> bool {
        self.mode == MODE_CROSSFADE
    }

    /// The only translation “kind from the model → numbers for the cores”.
    pub fn of(kind: &TransitionKind) -> TransMix {
        let d = |dir: Dir| {
            let (x, y) = dir.vec();
            (x as f32, y as f32)
        };
        let mut p = [0.0f32; 8];
        let mode = match kind {
            TransitionKind::Crossfade => MODE_CROSSFADE,
            TransitionKind::Plugin(slot) => {
                let i = plugin_kernels().iter().position(|(id, _)| *id == slot.id);
                match i {
                    Some(i) => {
                        p = plugin_kernels()[i].1.pack(&slot.values);
                        MODE_PLUGIN + i as u32
                    }
                    None => MODE_CROSSFADE,
                }
            }
            // a kind this build lacks plays as the crossfade
            TransitionKind::Missing(_) => MODE_CROSSFADE,
            TransitionKind::Swap { vertical, depth } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                p[1] = *depth as f32;
                MODE_SWAP
            }
            TransitionKind::Flip { vertical, persp, shade } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                p[1] = *persp as f32;
                p[2] = *shade as f32;
                MODE_FLIP
            }
            TransitionKind::Cube { dir, persp, shade } => {
                (p[0], p[1]) = d(*dir);
                p[2] = *persp as f32;
                p[3] = *shade as f32;
                MODE_CUBE
            }
            TransitionKind::Door { vertical, persp, shade } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                p[1] = *persp as f32;
                p[2] = *shade as f32;
                MODE_DOOR
            }
            TransitionKind::PagePeel { dir, curl, shadow } => {
                (p[0], p[1]) = d(*dir);
                p[2] = *curl as f32;
                p[3] = *shadow as f32;
                MODE_PEEL
            }
            TransitionKind::PageRoll { dir, curl, shadow } => {
                (p[0], p[1]) = d(*dir);
                p[2] = *curl as f32;
                p[3] = *shadow as f32;
                MODE_ROLL
            }
        };
        TransMix { mode, p }
    }
}

/// Precomputed frame scalars for VOLUMETRIC kinds (R.51, item 401):
/// inverse homographies of the panels, computed in f64 and rounded ONCE
/// to f32. The CPU core and WGSL read the same bits — parity
/// by construction, a separate f64 reference is not needed. They go into the field `m`
/// of the composite's uniform (free since R.44). For the other modes — zeros;
/// they are not and must not be in the cache keys: ext is a deterministic function of the
/// already hashed (mode, p, k) — see the comment at `put_mix` in kt-gpu.
pub fn trans_ext(m: &TransMix, k: u32) -> [f32; 16] {
    if m.mode >= MODE_PLUGIN {
        return plugin_kernel(m.mode).map_or([0.0; 16], |kern| kern.ext(&m.p, k));
    }
    let mut e = [0.0f32; 16];
    let kf = k as f64 / 4096.0;
    let p = m.p;
    let dist = |persp: f32| 1.5 / (persp as f64).max(0.05);
    match m.mode {
        MODE_FLIP => {
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
        }
        MODE_CUBE => {
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
        }
        MODE_DOOR => {
            let th = kf * (std::f64::consts::FRAC_PI_2 - 1e-3);
            let (sn, cs) = th.sin_cos();
            let d = dist(p[1]);
            // left leaf: hinge at the edge, it swings open inward;
            // the right one — its mirror, computed in the core
            if let Some(h) = face_h_inv(0.0, cs, 0.0, sn, d) {
                e[..8].copy_from_slice(&h);
            }
            e[8] = (1.0 - p[2] as f64 * sn) as f32;
        }
        _ => {}
    }
    e
}











/// The engine's frame as a sampling side (`trans::Img`): a frame or the empty side of a fade.
fn img_of(f: Option<&VideoFrame>) -> Img<'_> {
    match f {
        Some(f) => Img::Frame {
            px: &f.rgba,
            w: f.width as i32,
            h: f.height as i32,
            has_alpha: f.has_alpha,
        },
        None => Img::Clear,
    }
}


/// Transition core: `dst = mix(a, b)` by the kind `m` with progress `k`
/// (0..4096). Returns “the result has alpha”. The crossfade does not come
/// here — it is computed by the integer `dissolve_into`.
pub fn mix_into(
    dst: &mut Vec<u8>,
    a: Option<&VideoFrame>,
    b: Option<&VideoFrame>,
    k: u32,
    m: TransMix,
    w: usize,
    h: usize,
) -> bool {
    let n = w * h * 4;
    dst.clear();
    dst.resize(n, 0);
    let ia = img_of(a);
    let ib = img_of(b);
    let ext = trans_ext(&m, k);
    let k = kf(k);
    let (fw, fh) = (w as f32, h as f32);
    let aspect = fw / fh;
    for y in 0..h {
        let v = (y as f32 + 0.5) / fh;
        for x in 0..w {
            let u = (x as f32 + 0.5) / fw;
            let px = kernel(&ia, &ib, u, v, k, m, aspect, fw, fh, &ext);
            let i = (y * w + x) * 4;
            for c in 0..4 {
                dst[i + c] = (px[c] + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
    }
    true
}

/// One transition pixel. An exact mirror of `trans()` from composite.wgsl.
#[allow(clippy::too_many_arguments)]
fn kernel(
    a: &Img<'_>,
    b: &Img<'_>,
    u: f32,
    v: f32,
    k: f32,
    m: TransMix,
    aspect: f32,
    fw: f32,
    fh: f32,
    ext: &[f32; 16],
) -> [f32; 4] {
    let p = m.p;
    match m.mode {
        m if m >= MODE_PLUGIN => match plugin_kernel(m) {
            Some(kern) => kern.pixel(a, b, u, v, k, &p, &kt_plugin_api::trans::Canvas { aspect, w: fw, h: fh }, ext),
            None => mix4(a.sample(u, v), b.sample(u, v), k),
        },
        MODE_SWAP => {
            // 2.5D exchange: the sides travel along an arc, compressing inward; the order
            // switches at the midpoint — occlusion is analytic, without
            // depth (there is no depth buffer in the pipeline and none is needed)
            let vert = p[0] > 0.5;
            let wave = (std::f32::consts::PI * k).sin();
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
        }
        MODE_FLIP => {
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
        }
        MODE_CUBE => {
            let vert = p[1].abs() > 0.5;
            let (sx, sy) = if vert { (v, u) } else { (u, v) };
            let sh = 1.0 - p[3] * (std::f32::consts::PI * k).sin() * 0.5;
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
        }
        MODE_DOOR => {
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
        }
        MODE_PEEL => {
            // the sheet curls back around a cylinder of radius r: the reverse-side tail
            // flies over the part still lying, under the lifted one — the incoming
            let (dx, dy) = (p[0], p[1]);
            let r = p[2].max(0.01);
            let shadow = p[3];
            let c = axis_c(u, v, dx, dy);
            let f = 1.0 - k * (1.0 + std::f32::consts::PI * r);
            let back = 1.0 - shadow * 0.35;
            // 1. flat tail of the reverse side (θ > π): x = 2f + πr − c
            let m_tail = 2.0 * f + std::f32::consts::PI * r - c;
            if c <= f && m_tail >= f + std::f32::consts::PI * r && m_tail <= 1.0 {
                return shade3(axis_sample(a, u, v, dx, dy, m_tail), back);
            }
            // 2. the cylinder over [f, f+r]
            let dc = c - f;
            if (0.0..=r).contains(&dc) {
                let th1 = dc.atan2((r * r - dc * dc).max(0.0).sqrt());
                let th2 = std::f32::consts::PI - th1;
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
        }
        MODE_ROLL => {
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
                let th =
                    if dc >= 0.0 { std::f32::consts::PI - th1 } else { std::f32::consts::PI + th1 };
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
        }
        _ => mix4(a.sample(u, v), b.sample(u, v), k),
    }
}











