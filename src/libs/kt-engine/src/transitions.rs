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
use kt_model::{Dir, IrisShape, TransitionKind};

/// Modes shared by CPU and WGSL. The value goes into the uniform, so the
/// numbers are frozen: add a new mode ONLY AT THE END.
pub const MODE_CROSSFADE: u32 = 0;
pub const MODE_WIPE: u32 = 1;
pub const MODE_IRIS: u32 = 2;
pub const MODE_CLOCK: u32 = 3;
pub const MODE_BARN: u32 = 4;
pub const MODE_LUMA: u32 = 5;
pub const MODE_PUSH: u32 = 6;
pub const MODE_SLIDE: u32 = 7;
pub const MODE_ZOOM: u32 = 8;
pub const MODE_SPIN: u32 = 9;
pub const MODE_WHIP: u32 = 10;
pub const MODE_BLUR: u32 = 11;
pub const MODE_PIXELATE: u32 = 12;
pub const MODE_GLITCH: u32 = 13;
pub const MODE_FLASH: u32 = 14;
pub const MODE_DIP: u32 = 15;
// R.51 (item 400): patterns, shift-symbioses, stylized ones. The numbers are a contract
// ONLY between transitions.rs and composite.wgsl (edited in one commit):
// the project file receives tag(), the cache keys receive the ready-made TransMix.
pub const MODE_BLINDS: u32 = 16;
pub const MODE_CHECKER: u32 = 17;
pub const MODE_BLOCKS: u32 = 18;
pub const MODE_SPIRAL: u32 = 19;
pub const MODE_SPLIT: u32 = 20;
pub const MODE_SQUEEZE: u32 = 21;
pub const MODE_STRETCH: u32 = 22;
pub const MODE_SWAP: u32 = 23;
pub const MODE_SHAKE: u32 = 24;
pub const MODE_BURN: u32 = 25;
pub const MODE_LEAKS: u32 = 26;
pub const MODE_ZOOMBLUR: u32 = 27;
// Volumetric (item 401): panels via inverse homographies in ext (see trans_ext),
// peel/roll cylinders — analytics right in the core.
pub const MODE_FLIP: u32 = 28;
pub const MODE_CUBE: u32 = 29;
pub const MODE_DOOR: u32 = 30;
pub const MODE_PEEL: u32 = 31;
pub const MODE_ROLL: u32 = 32;

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
            TransitionKind::Wipe { dir, angle, softness } => {
                let (dx, dy) = d(*dir);
                let a = (*angle as f32).to_radians();
                let (s, c) = (a.sin(), a.cos());
                // the angle turns the direction — the diagonal is obtained by it
                p[0] = dx * c - dy * s;
                p[1] = dx * s + dy * c;
                p[2] = *softness as f32;
                MODE_WIPE
            }
            TransitionKind::Iris { shape, cx, cy, softness } => {
                p[0] = match shape {
                    IrisShape::Circle => 0.0,
                    IrisShape::Diamond => 1.0,
                    IrisShape::Box => 2.0,
                };
                p[1] = *softness as f32;
                p[2] = *cx as f32;
                p[3] = *cy as f32;
                MODE_IRIS
            }
            TransitionKind::Clock { reverse, cx, cy, softness } => {
                p[0] = if *reverse { 1.0 } else { 0.0 };
                p[1] = *softness as f32;
                p[2] = *cx as f32;
                p[3] = *cy as f32;
                MODE_CLOCK
            }
            TransitionKind::Barn { vertical, softness } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                p[1] = *softness as f32;
                MODE_BARN
            }
            TransitionKind::Luma { invert, softness } => {
                p[0] = if *invert { 1.0 } else { 0.0 };
                p[1] = *softness as f32;
                MODE_LUMA
            }
            TransitionKind::Push { dir } => {
                (p[0], p[1]) = d(*dir);
                MODE_PUSH
            }
            TransitionKind::Slide { dir } => {
                (p[0], p[1]) = d(*dir);
                MODE_SLIDE
            }
            TransitionKind::Zoom { amount, cx, cy } => {
                p[0] = *amount as f32;
                p[1] = *cx as f32;
                p[2] = *cy as f32;
                MODE_ZOOM
            }
            TransitionKind::Spin { turns, amount, cx, cy } => {
                p[0] = *turns as f32;
                p[1] = *amount as f32;
                p[2] = *cx as f32;
                p[3] = *cy as f32;
                MODE_SPIN
            }
            TransitionKind::Whip { dir, blur } => {
                (p[0], p[1]) = d(*dir);
                p[2] = *blur as f32;
                MODE_WHIP
            }
            TransitionKind::BlurDissolve { amount } => {
                p[0] = *amount as f32;
                MODE_BLUR
            }
            TransitionKind::Pixelate { amount } => {
                p[0] = *amount as f32;
                MODE_PIXELATE
            }
            TransitionKind::Glitch { amount } => {
                p[0] = *amount as f32;
                MODE_GLITCH
            }
            TransitionKind::Flash { color_r, color_g, color_b, strength } => {
                p[0] = *color_r as f32;
                p[1] = *color_g as f32;
                p[2] = *color_b as f32;
                p[3] = *strength as f32;
                MODE_FLASH
            }
            TransitionKind::Dip { color_r, color_g, color_b } => {
                p[0] = *color_r as f32;
                p[1] = *color_g as f32;
                p[2] = *color_b as f32;
                MODE_DIP
            }
            TransitionKind::Blinds { count, angle, softness } => {
                // the angle rotates the axis of the stripes — the same scheme as wipe
                let a = (*angle as f32).to_radians();
                let (sn, cs) = (a.sin(), a.cos());
                p[0] = cs;
                p[1] = sn;
                p[2] = *count as f32;
                p[3] = *softness as f32;
                MODE_BLINDS
            }
            TransitionKind::Checker { cols, rows, softness } => {
                p[0] = *cols as f32;
                p[1] = *rows as f32;
                p[2] = *softness as f32;
                MODE_CHECKER
            }
            TransitionKind::Blocks { cols, rows, softness } => {
                p[0] = *cols as f32;
                p[1] = *rows as f32;
                p[2] = *softness as f32;
                MODE_BLOCKS
            }
            TransitionKind::Spiral { turns, cx, cy, softness } => {
                p[0] = *turns as f32;
                p[1] = *softness as f32;
                p[2] = *cx as f32;
                p[3] = *cy as f32;
                MODE_SPIRAL
            }
            TransitionKind::Split { vertical } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                MODE_SPLIT
            }
            TransitionKind::Squeeze { dir } => {
                (p[0], p[1]) = d(*dir);
                MODE_SQUEEZE
            }
            TransitionKind::Stretch { vertical } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                MODE_STRETCH
            }
            TransitionKind::Swap { vertical, depth } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                p[1] = *depth as f32;
                MODE_SWAP
            }
            TransitionKind::Shake { amount, steps } => {
                p[0] = *amount as f32;
                p[1] = *steps as f32;
                MODE_SHAKE
            }
            TransitionKind::FilmBurn { scale, softness, tint_r, tint_g, tint_b } => {
                p[0] = *scale as f32;
                p[1] = *softness as f32;
                p[2] = *tint_r as f32;
                p[3] = *tint_g as f32;
                p[4] = *tint_b as f32;
                MODE_BURN
            }
            TransitionKind::LightLeaks { strength, tint_r, tint_g, tint_b } => {
                p[0] = *strength as f32;
                p[1] = *tint_r as f32;
                p[2] = *tint_g as f32;
                p[3] = *tint_b as f32;
                MODE_LEAKS
            }
            TransitionKind::ZoomBlur { amount, cx, cy } => {
                p[0] = *amount as f32;
                p[1] = *cx as f32;
                p[2] = *cy as f32;
                MODE_ZOOMBLUR
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

/// Inverse homography of the panel: the panel point (s, t) lies in the world as
/// `x(s) = x0 + s·xs`, `z(s) = z0 + s·zs`, `y = t − 0.5`; the camera at
/// (0.5, 0.5, −d). Returns 8 numbers (normalization h22 = 1) of the mapping
/// “screen (X, Y) → panel (s·w, t·w, w)”.
fn face_h_inv(x0: f64, xs: f64, z0: f64, zs: f64, d: f64) -> Option<[f32; 8]> {
    // forward matrix (s, t, 1) → (X·w, Y·w, w)
    let m = [
        xs * d + 0.5 * zs,
        0.0,
        (x0 - 0.5) * d + 0.5 * (d + z0),
        0.5 * zs,
        d,
        0.5 * z0,
        zs,
        0.0,
        d + z0,
    ];
    let inv = inv3(m)?;
    let n = inv[8];
    if n.abs() < 1e-12 {
        return None;
    }
    let mut out = [0.0f32; 8];
    for i in 0..8 {
        out[i] = (inv[i] / n) as f32;
    }
    Some(out)
}

fn inv3(m: [f64; 9]) -> Option<[f64; 9]> {
    let det = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
        + m[2] * (m[3] * m[7] - m[4] * m[6]);
    if det.abs() < 1e-12 {
        return None;
    }
    let id = 1.0 / det;
    Some([
        (m[4] * m[8] - m[5] * m[7]) * id,
        (m[2] * m[7] - m[1] * m[8]) * id,
        (m[1] * m[5] - m[2] * m[4]) * id,
        (m[5] * m[6] - m[3] * m[8]) * id,
        (m[0] * m[8] - m[2] * m[6]) * id,
        (m[2] * m[3] - m[0] * m[5]) * id,
        (m[3] * m[7] - m[4] * m[6]) * id,
        (m[1] * m[6] - m[0] * m[7]) * id,
        (m[0] * m[4] - m[1] * m[3]) * id,
    ])
}

/// A frame as a sampling source. A frame without alpha is read as opaque —
/// the same policy as `dissolve_into`. `Clear` — the EMPTY side of a
/// solo fade (transparency): thanks to it, a fade at the track's edge is
/// computed by the same core as a pair, and a wipe “from nothing” works.
enum Img<'a> {
    Frame { px: &'a [u8], w: i32, h: i32, has_alpha: bool },
    Clear,
}

impl Img<'_> {
    fn of(f: Option<&VideoFrame>) -> Img<'_> {
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

    fn texel(&self, x: i32, y: i32) -> [f32; 4] {
        let Img::Frame { px, w, h, has_alpha } = self else { return [0.0; 4] };
        let x = x.clamp(0, w - 1);
        let y = y.clamp(0, h - 1);
        let i = ((y * w + x) * 4) as usize;
        [
            px[i] as f32,
            px[i + 1] as f32,
            px[i + 2] as f32,
            if *has_alpha { px[i + 3] as f32 } else { 255.0 },
        ]
    }

    /// Bilinear sampling over normalized coordinates (clamp at the edges) —
    /// the same coordinate scheme as `bilinear_into_scalar`.
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let Img::Frame { w, h, .. } = self else { return [0.0; 4] };
        let sx = u * *w as f32 - 0.5;
        let sy = v * *h as f32 - 0.5;
        let x = sx.floor();
        let y = sy.floor();
        let fx = sx - x;
        let fy = sy - y;
        let (x, y) = (x as i32, y as i32);
        let p00 = self.texel(x, y);
        let p10 = self.texel(x + 1, y);
        let p01 = self.texel(x, y + 1);
        let p11 = self.texel(x + 1, y + 1);
        let mut out = [0.0f32; 4];
        for c in 0..4 {
            let top = p00[c] * (1.0 - fx) + p10[c] * fx;
            let bot = p01[c] * (1.0 - fx) + p11[c] * fx;
            out[c] = top * (1.0 - fy) + bot * fy;
        }
        out
    }
}

fn mix4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut o = [0.0f32; 4];
    for c in 0..4 {
        o[c] = a[c] + (b[c] - a[c]) * t;
    }
    o
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x < e0 { 0.0 } else { 1.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The front of “masking” transitions: 1 — B is already shown, 0 — still A.
/// `s` — the front coordinate 0..1, `k` — the progress 0..1.
fn front_mask(s: f32, k: f32, soft: f32) -> f32 {
    let soft = soft.clamp(0.0, 1.0);
    if soft < 1e-4 {
        return if s < k { 1.0 } else { 0.0 };
    }
    // the front moves out beyond both edges by the blur width — otherwise at k=0 and k=1
    // a visible edge would remain
    let front = k * (1.0 + soft) - soft;
    1.0 - smoothstep(front, front + soft, s)
}

/// Integer weight k (0..4096) in fractions.
fn kf(k: u32) -> f32 {
    k as f32 / 4096.0
}

/// Hash for the “digital glitch”: the same integer hash as the noise of
/// kt-fx (and it is repeated in WGSL) — otherwise the glitch on CPU and GPU would land
/// in different places and the parity test would catch noise instead of an error.
fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn hash01(x: u32) -> f32 {
    (hash_u32(x) >> 8) as f32 / 16_777_216.0
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
    let ia = Img::of(a);
    let ib = Img::of(b);
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
        MODE_WIPE => {
            // projection onto the direction, normalized by the square's span
            let (dx, dy) = (p[0], p[1]);
            let ext = (dx.abs() + dy.abs()).max(1e-6);
            let s = ((u - 0.5) * dx + (v - 0.5) * dy + ext * 0.5) / ext;
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[2]))
        }
        MODE_IRIS => {
            // the centre (p2, p3) is arbitrary (R.51); rmax — the distance to the
            // FARTHEST corner of the frame in the same metric, so that the front
            // always covers the frame entirely.
            let (cx, cy) = ((u - p[2]) * aspect, v - p[3]);
            let metric = |dx: f32, dy: f32| match p[0] as i32 {
                1 => dx.abs() + dy.abs(),
                2 => dx.abs().max(dy.abs()),
                _ => (dx * dx + dy * dy).sqrt(),
            };
            let r = metric(cx, cy);
            let ex = (p[2].max(1.0 - p[2])) * aspect;
            let ey = p[3].max(1.0 - p[3]);
            let rmax = metric(ex, ey).max(1e-6);
            mix4(a.sample(u, v), b.sample(u, v), front_mask(r / rmax, k, p[1]))
        }
        MODE_CLOCK => {
            let (cx, cy) = (u - p[2], v - p[3]);
            // 0 at the top, clockwise; reverse — counter-clockwise
            let mut ang = cx.atan2(-cy);
            if ang < 0.0 {
                ang += std::f32::consts::TAU;
            }
            if p[0] > 0.5 {
                ang = std::f32::consts::TAU - ang;
            }
            let s = ang / std::f32::consts::TAU;
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[1]))
        }
        MODE_BARN => {
            let s = if p[0] > 0.5 { (u - 0.5).abs() * 2.0 } else { (v - 0.5).abs() * 2.0 };
            // the leaves go OUT FROM the centre: the front is computed from it outward
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[1]))
        }
        MODE_LUMA => {
            let c = a.sample(u, v);
            let mut l = (0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]) / 255.0;
            if p[0] > 0.5 {
                l = 1.0 - l;
            }
            mix4(c, b.sample(u, v), front_mask(l, k, p[1]))
        }
        MODE_PUSH => {
            // the content MOVES toward dir, so the screen pixel
            // shows the source BEHIND itself at dir·k; B stands right
            // BEHIND the outgoing A — a full screen further ALONG the direction
            // of motion (`+dir`, item 396: with `−dir` the coordinate went past
            // [0,1) and the clamp drew a smeared edge instead of B).
            let (dx, dy) = (p[0], p[1]);
            let (au, av) = (u - dx * k, v - dy * k);
            if (0.0..1.0).contains(&au) && (0.0..1.0).contains(&av) {
                a.sample(au, av)
            } else {
                b.sample(au + dx, av + dy)
            }
        }
        MODE_SLIDE => {
            let (bu, bv) = (u + p[0] * (1.0 - k), v + p[1] * (1.0 - k));
            if (0.0..1.0).contains(&bu) && (0.0..1.0).contains(&bv) {
                b.sample(bu, bv)
            } else {
                a.sample(u, v)
            }
        }
        MODE_ZOOM => {
            let amt = p[0];
            let (ox, oy) = (p[1], p[2]);
            let za = 1.0 + amt * k; // the outgoing one spreads apart
            let zb = 1.0 + amt * (1.0 - k); // the incoming one rushes in
            let sa = a.sample((u - ox) / za + ox, (v - oy) / za + oy);
            let sb = b.sample((u - ox) / zb + ox, (v - oy) / zb + oy);
            mix4(sa, sb, k)
        }
        MODE_SPIN => {
            let (turns, amt) = (p[0], p[1]);
            let (ox, oy) = (p[2], p[3]);
            let sa = rot_sample(
                a,
                u,
                v,
                turns * k * std::f32::consts::TAU,
                1.0 + amt * k,
                aspect,
                ox,
                oy,
            );
            let sb = rot_sample(
                b,
                u,
                v,
                -turns * (1.0 - k) * std::f32::consts::TAU,
                1.0 + amt * (1.0 - k),
                aspect,
                ox,
                oy,
            );
            mix4(sa, sb, k)
        }
        MODE_WHIP => {
            // a flyover like push, but with blur along the motion; the blur strength is
            // maximal in the middle of the gesture
            let (dx, dy) = (p[0], p[1]);
            let blur = p[2] * (std::f32::consts::PI * k).sin();
            let mut acc = [0.0f32; 4];
            for i in 0..WHIP_TAPS {
                let o = (i as f32 / (WHIP_TAPS - 1) as f32 - 0.5) * blur * 0.25;
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
                *ch /= WHIP_TAPS as f32;
            }
            acc
        }
        MODE_BLUR => {
            // the radius grows toward the middle of the transition and goes out there
            let r = p[0] * (std::f32::consts::PI * k).sin();
            let sa = box9(a, u, v, r / fw, r / fh);
            let sb = box9(b, u, v, r / fw, r / fh);
            mix4(sa, sb, k)
        }
        MODE_PIXELATE => {
            let s = (p[0] * (std::f32::consts::PI * k).sin()).max(1.0);
            let (bu, bv) = (quant(u, s / fw), quant(v, s / fh));
            mix4(a.sample(bu, bv), b.sample(bu, bv), k)
        }
        MODE_GLITCH => {
            let amt = p[0] * (std::f32::consts::PI * k).sin();
            // the bands of rows travel horizontally, the channels diverge
            let band = (v * 24.0).floor() as u32;
            let shift = (hash01(band.wrapping_mul(1973)) - 0.5) * amt * 0.25;
            let ch = amt * 8.0 / fw;
            let (su, sv) = (u + shift, v);
            let pick = |img: &Img<'_>| {
                let r = img.sample(su + ch, sv);
                let g = img.sample(su, sv);
                let bl = img.sample(su - ch, sv);
                [r[0], g[1], bl[2], g[3]]
            };
            mix4(pick(a), pick(b), k)
        }
        MODE_FLASH => {
            let base = mix4(a.sample(u, v), b.sample(u, v), k);
            let w = (p[3] * (std::f32::consts::PI * k).sin()).clamp(0.0, 1.0);
            [
                base[0] + (p[0] - base[0]) * w,
                base[1] + (p[1] - base[1]) * w,
                base[2] + (p[2] - base[2]) * w,
                base[3],
            ]
        }
        MODE_DIP => {
            // two phases: A → colour, colour → B (classic dip-to-colour)
            let col = [p[0], p[1], p[2]];
            let (src, t) =
                if k < 0.5 { (a.sample(u, v), k * 2.0) } else { (b.sample(u, v), 2.0 - k * 2.0) };
            [
                src[0] + (col[0] - src[0]) * t,
                src[1] + (col[1] - src[1]) * t,
                src[2] + (col[2] - src[2]) * t,
                src[3],
            ]
        }
        MODE_BLINDS => {
            // projection onto the rotated axis (as wipe), the fraction INSIDE the band:
            // all bands open simultaneously
            let (dx, dy) = (p[0], p[1]);
            let ext = (dx.abs() + dy.abs()).max(1e-6);
            let c = ((u - 0.5) * dx + (v - 0.5) * dy + ext * 0.5) / ext;
            let f = fracf(c * p[2].max(1.0));
            mix4(a.sample(u, v), b.sample(u, v), front_mask(f, k, p[3]))
        }
        MODE_CHECKER => {
            // local wipe in a cell, even and odd ones — toward each other
            let (cols, rows) = (p[0].max(1.0), p[1].max(1.0));
            let (i, j) = ((u * cols).floor(), (v * rows).floor());
            let fx = fracf(u * cols);
            let s = if ((i + j) as i32) % 2 == 0 { fx } else { 1.0 - fx };
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[2]))
        }
        MODE_BLOCKS => {
            // the order of cells — the hash of the cell: determinism without a seed, the same
            // trick as glitch. softness — a per-cell crossfade
            let (cols, rows) = (p[0].max(1.0), p[1].max(1.0));
            let (i, j) = ((u * cols).floor() as u32, (v * rows).floor() as u32);
            let s = hash01(hash_u32(i.wrapping_mul(1973)) ^ hash_u32(j.wrapping_mul(9277)));
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[2]))
        }
        MODE_SPIRAL => {
            // Archimedean front: the angle (as clock) + turns along the radius
            let (cx, cy) = (u - p[2], v - p[3]);
            let mut ang = cx.atan2(-cy);
            if ang < 0.0 {
                ang += std::f32::consts::TAU;
            }
            let theta = ang / std::f32::consts::TAU;
            let ex = (p[2].max(1.0 - p[2])) * aspect;
            let ey = p[3].max(1.0 - p[3]);
            let rmax = (ex * ex + ey * ey).sqrt().max(1e-6);
            let r = ((cx * aspect) * (cx * aspect) + cy * cy).sqrt() / rmax;
            let turns = p[0].max(0.25);
            let sfrac = (theta + turns * r) / (1.0 + turns);
            mix4(a.sample(u, v), b.sample(u, v), front_mask(sfrac, k, p[1]))
        }
        MODE_SPLIT => {
            // symbiosis: the halves of A part, B opens in the gap
            let c = if p[0] > 0.5 { v - 0.5 } else { u - 0.5 };
            let h = k * 0.5;
            if c.abs() >= h {
                let off = h * c.signum();
                if p[0] > 0.5 { a.sample(u, v - off) } else { a.sample(u - off, v) }
            } else {
                b.sample(u, v)
            }
        }
        MODE_SQUEEZE => {
            // symbiosis: A compresses toward the far edge, B straightens out from the
            // near one; movement along dir
            let (dx, dy) = (p[0], p[1]);
            let c = if dx > 0.5 {
                u
            } else if dx < -0.5 {
                1.0 - u
            } else if dy > 0.5 {
                v
            } else {
                1.0 - v
            };
            let back = |c2: f32| -> (f32, f32) {
                if dx > 0.5 {
                    (c2, v)
                } else if dx < -0.5 {
                    (1.0 - c2, v)
                } else if dy > 0.5 {
                    (u, c2)
                } else {
                    (u, 1.0 - c2)
                }
            };
            if c < k {
                let (su, sv) = back(c / k.max(1e-4));
                b.sample(su, sv)
            } else {
                let (su, sv) = back((c - k) / (1.0 - k).max(1e-4));
                a.sample(su, sv)
            }
        }
        MODE_STRETCH => {
            // symbiosis: B stretches out from the central line over A
            let c = if p[0] > 0.5 { v - 0.5 } else { u - 0.5 };
            let h = k * 0.5;
            if c.abs() < h {
                let c2 = c / k.max(1e-4) + 0.5;
                if p[0] > 0.5 { b.sample(u, c2) } else { b.sample(c2, v) }
            } else {
                a.sample(u, v)
            }
        }
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
        MODE_SHAKE => {
            // frame jerks: the step index by progress, the frame is a cut at 0.5
            let steps = p[1].max(1.0);
            let i = ((k * steps) as u32).min(steps as u32 - 1);
            let amp = p[0] * 0.2 * (std::f32::consts::PI * k).sin();
            let ox = (hash01(i.wrapping_mul(2).wrapping_add(1)) - 0.5) * amp;
            let oy = (hash01(i.wrapping_mul(2).wrapping_add(2)) - 0.5) * amp;
            let img = if k < 0.5 { a } else { b };
            img.sample(u + ox, v + oy)
        }
        MODE_BURN => {
            // the front by coarse noise, at the edge — a hot admixture of colour;
            // the contribution is gated by alpha (otherwise a solo fade would light up emptiness)
            let n = value_noise(u * p[0] * aspect, v * p[0]);
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
        }
        MODE_LEAKS => {
            // two coloured spots fly along the diagonals over the dissolve
            let base = mix4(a.sample(u, v), b.sample(u, v), k);
            let wave = (std::f32::consts::PI * k).sin();
            let blob = |bx: f32, by: f32| -> f32 {
                let dx = (u - bx) * aspect;
                let dy = v - by;
                let d = (dx * dx + dy * dy).sqrt() / 0.45;
                let w = (1.0 - d).max(0.0);
                w * w
            };
            let w = (blob(k * 1.2 - 0.1, 0.3 + 0.4 * k) + blob(1.1 - 1.2 * k, 0.7 - 0.4 * k))
                * p[0]
                * wave;
            let gate = w.clamp(0.0, 1.0) * base[3] / 255.0;
            [
                base[0] + (p[1] - base[0]) * gate,
                base[1] + (p[2] - base[1]) * gate,
                base[2] + (p[3] - base[2]) * gate,
                base[3],
            ]
        }
        MODE_ZOOMBLUR => {
            // rays from the centre: 7 taps along the ray, the side — a cut at 0.5
            let (ox, oy) = (p[1], p[2]);
            let amt = p[0] * (std::f32::consts::PI * k).sin();
            let img = if k < 0.5 { a } else { b };
            let mut acc = [0.0f32; 4];
            for i in 0..WHIP_TAPS {
                let z = 1.0 + amt * 0.6 * (i as f32 / (WHIP_TAPS - 1) as f32);
                let c = img.sample((u - ox) / z + ox, (v - oy) / z + oy);
                for ch in 0..4 {
                    acc[ch] += c[ch];
                }
            }
            for ch in acc.iter_mut() {
                *ch /= WHIP_TAPS as f32;
            }
            acc
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

const WHIP_TAPS: usize = 7;

/// Panel by the inverse homography (8 numbers from `trans_ext`): screen → (s, t).
/// None — behind the camera (denominator ≤ 0).
fn hpanel(h: &[f32], x: f32, y: f32) -> Option<(f32, f32)> {
    let den = h[6] * x + h[7] * y + 1.0;
    if den <= 1e-6 {
        return None;
    }
    Some(((h[0] * x + h[1] * y + h[2]) / den, (h[3] * x + h[4] * y + h[5]) / den))
}

fn shade3(mut c: [f32; 4], sh: f32) -> [f32; 4] {
    c[0] *= sh;
    c[1] *= sh;
    c[2] *= sh;
    c
}

/// Coordinate along the curl axis: the front (c = 0) moves TOWARD dir.
fn axis_c(u: f32, v: f32, dx: f32, dy: f32) -> f32 {
    if dx > 0.5 {
        1.0 - u
    } else if dx < -0.5 {
        u
    } else if dy > 0.5 {
        1.0 - v
    } else {
        v
    }
}

/// Sample with the AXIAL coordinate substituted (the transverse one is untouched).
fn axis_sample(img: &Img<'_>, u: f32, v: f32, dx: f32, dy: f32, m: f32) -> [f32; 4] {
    if dx > 0.5 {
        img.sample(1.0 - m, v)
    } else if dx < -0.5 {
        img.sample(m, v)
    } else if dy > 0.5 {
        img.sample(u, 1.0 - m)
    } else {
        img.sample(u, m)
    }
}

/// The fractional part “as in WGSL”: x − floor(x) (f32::fract has a different sign
/// on negatives — the mirrors would drift apart).
fn fracf(x: f32) -> f32 {
    x - x.floor()
}

/// Straight-alpha “top over bottom” in f32 — for the 2.5D exchange, where the
/// sides move separately and real transparency appears.
fn over_straight(top: [f32; 4], bot: [f32; 4]) -> [f32; 4] {
    let ta = (top[3] / 255.0).clamp(0.0, 1.0);
    let ba = (bot[3] / 255.0).clamp(0.0, 1.0);
    let oa = ta + ba * (1.0 - ta);
    if oa <= 0.0 {
        return [0.0; 4];
    }
    let mut o = [0.0f32; 4];
    for c in 0..3 {
        o[c] = (top[c] * ta + bot[c] * ba * (1.0 - ta)) / oa;
    }
    o[3] = oa * 255.0;
    o
}

/// Cellular value-noise on the shared integer hash (the same as glitch/blocks)
/// — WGSL repeats it verbatim.
fn value_noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let h = |i: f32, j: f32| -> f32 {
        let (i, j) = (i as i32 as u32, j as i32 as u32);
        hash01(hash_u32(i.wrapping_mul(1973)) ^ hash_u32(j.wrapping_mul(9277)))
    };
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let top = h(xi, yi) + (h(xi + 1.0, yi) - h(xi, yi)) * sx;
    let bot = h(xi, yi + 1.0) + (h(xi + 1.0, yi + 1.0) - h(xi, yi + 1.0)) * sx;
    top + (bot - top) * sy
}

fn quant(x: f32, step: f32) -> f32 {
    if step <= 0.0 {
        return x;
    }
    (x / step).floor() * step + step * 0.5
}

#[allow(clippy::too_many_arguments)]
fn rot_sample(
    img: &Img<'_>,
    u: f32,
    v: f32,
    ang: f32,
    zoom: f32,
    aspect: f32,
    ox: f32,
    oy: f32,
) -> [f32; 4] {
    let (s, c) = (ang.sin(), ang.cos());
    let (cx, cy) = ((u - ox) * aspect / zoom, (v - oy) / zoom);
    let rx = cx * c - cy * s;
    let ry = cx * s + cy * c;
    img.sample(rx / aspect + ox, ry + oy)
}

/// A cheap blur with 9 taps (3×3 with step r) — the form is chosen so that
/// it repeats exactly in WGSL.
fn box9(img: &Img<'_>, u: f32, v: f32, rx: f32, ry: f32) -> [f32; 4] {
    if rx <= 0.0 && ry <= 0.0 {
        return img.sample(u, v);
    }
    let mut acc = [0.0f32; 4];
    for dy in -1..=1 {
        for dx in -1..=1 {
            let c = img.sample(u + dx as f32 * rx, v + dy as f32 * ry);
            for ch in 0..4 {
                acc[ch] += c[ch];
            }
        }
    }
    for ch in acc.iter_mut() {
        *ch /= 9.0;
    }
    acc
}
