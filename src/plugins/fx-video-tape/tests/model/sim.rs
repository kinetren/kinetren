//! The chain's f32 model, one function per stage, for the physics tests (compute-kernels-render, D7):
//! the shaders give no intermediate grid to read, and the tests assert what the signal does between
//! the stages. It never ships. t1186 holds the shaders to the frames this model made.
#![allow(dead_code)]

use fx_video_tape::deck::{lt, Deck, LT_SCALARS, LT_W, WORDS};
use fx_video_tape::sig::{atan2_cyc, cos_cyc, gauss, hash3, q16, sin_cyc, ENV_OFF, ENV_SCALE, LEVEL_OFF, LEVEL_SCALE, UV_OFF, UV_SCALE};
pub use fx_video_tape::sim::{Uni, BURST_HALF, DOC_THRESHOLD};
use fx_video_tape::standard::{Kernels, Standard, KN, KR};

/// The level of a black picture on the grid's margins (blanking).
const BLANK: f32 = 0.0;

/// Every intermediate grid of one frame (the tests read them).
pub struct Sim {
    pub st: &'static Standard,
    pub words: [u32; WORDS],
    pub uni: Uni,
    /// The resampled picture, A×H RGBA8.
    pub rgb: Vec<u8>,
    pub lt: Vec<f32>,
    pub inc: Vec<u32>,
    pub phase: Vec<u32>,
    /// The magnetisation written on the tape.
    pub m: Vec<f32>,
    /// The RF read back.
    pub r: Vec<f32>,
    /// The demodulated level and the RF envelope (u16-quantised).
    pub level: Vec<f32>,
    pub env: Vec<f32>,
    pub luma: Vec<f32>,
    pub u: Vec<f32>,
    pub v: Vec<f32>,
}

/// Run `f(y, row)` over the rows of a `w×h` buffer, in bands on the machine's threads.
pub fn rows<T: Send + Default + Clone>(w: usize, h: usize, f: impl Fn(usize, &mut [T]) + Sync) -> Vec<T> {
    let mut out = vec![T::default(); w * h];
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(16);
    let band = h.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (bi, chunk) in out.chunks_mut(band * w).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (i, row) in chunk.chunks_mut(w).enumerate() {
                    f(bi * band + i, row);
                }
            });
        }
    });
    out
}

#[inline]
fn at<T: Copy>(v: &[T], w: usize, x: i32, y: usize) -> T {
    v[y * w + x.clamp(0, w as i32 - 1) as usize]
}

/// The luma of a grid sample: Rec. 601 on the resampled bytes, blanking outside the active line.
#[inline]
pub fn luma_at(rgb: &[u8], u: &Uni, j: i32, y: usize) -> f32 {
    let x = j - u.m as i32;
    if x < 0 || x >= u.a as i32 {
        return BLANK;
    }
    let i = (y * u.a as usize + x as usize) * 4;
    (0.299 * rgb[i] as f32 + 0.587 * rgb[i + 1] as f32 + 0.114 * rgb[i + 2] as f32) * (1.0 / 255.0)
}

/// U and V of a grid sample (0 outside the active line).
#[inline]
pub fn uv_at(rgb: &[u8], u: &Uni, j: i32, y: usize) -> (f32, f32) {
    let x = j - u.m as i32;
    if x < 0 || x >= u.a as i32 {
        return (0.0, 0.0);
    }
    let i = (y * u.a as usize + x as usize) * 4;
    let r = rgb[i] as f32 * (1.0 / 255.0);
    let g = rgb[i + 1] as f32 * (1.0 / 255.0);
    let b = rgb[i + 2] as f32 * (1.0 / 255.0);
    let l = 0.299 * r + 0.587 * g + 0.114 * b;
    (0.492 * (b - l), 0.877 * (r - l))
}

/// Stage 1: the record luma → the DDS phase increment.
pub fn s1(rgb: &[u8], k: &Kernels, u: &Uni, y: usize, n: i32) -> u32 {
    let mut e = 0.0f32;
    for (i, c) in k.rec.iter().enumerate() {
        e += c * luma_at(rgb, u, n + k.rec_c - i as i32, y);
    }
    let e = e.clamp(u.dark, u.white);
    let f = u.fsync_n + (e - u.l_sync) * u.dev_n;
    (f * 4_294_967_296.0) as u32
}

/// Stage 2: the block of 32 increments ending at `n`.
pub fn s2(inc: &[u32], w: usize, y: usize, n: i32) -> u32 {
    let mut s = 0u32;
    for i in 0..32 {
        let j = n - i;
        if j >= 0 {
            s = s.wrapping_add(inc[y * w + j as usize]);
        }
    }
    s
}

/// The FM's starting phase of a row (a counter hash: the tape position).
#[inline]
pub fn start_phase(u: &Uni, y: usize) -> u32 {
    hash3(u.frame, y as u32, 0xF00D)
}

/// The colour-under carrier's phase in cycles at sample `j` of a line `yf` of a track rotating by
/// `rot` quarter turns per line.
#[inline]
pub fn cu_phase(u: &Uni, j: i32, yf: f32, rot: f32) -> f32 {
    let l = u.cu_line * yf + rot * yf * 0.25;
    u.fcu_n * j as f32 + (l - l.floor())
}

/// Stage 4: write — FM plus the colour-under, through the tape's saturation in its in-band form
/// (the carrier compressed, the chroma recorded with the carrier as bias and compressed by its own
/// level).
pub fn s4(phase: u32, rgb: &[u8], k: &Kernels, u: &Uni, y: usize, n: i32) -> f32 {
    let fm = cos_cyc((phase >> 8) as f32 * (1.0 / 16_777_216.0));
    let (mut uc, mut vc) = (0.0f32, 0.0f32);
    for (i, c) in k.chroma.iter().enumerate() {
        let (a, b) = uv_at(rgb, u, n + k.chroma_c - i as i32, y);
        uc += c * a;
        vc += c * b;
    }
    let yf = (y >> 1) as u32;
    if u.v_switch && yf & 1 == 1 {
        vc = -vc;
    }
    let th = cu_phase(u, n, yf as f32, u.rot[y & 1]);
    let c = u.a_c * (uc * cos_cyc(th) + vc * sin_cyc(th));
    u.sat_fm * fm + u.sat_c * (c / (1.0 + u.sat * c.abs()))
}

/// Stage 5: read — the own track and the neighbour through the row's kernels, the dropout, the noise.
pub fn s5(m: &[f32], lt_row: &[f32], u: &Uni, y: usize, n: i32) -> f32 {
    let w = u.w as usize;
    let cr = lt_row[lt::CONTENT] as usize;
    let adj = (lt_row[lt::ADJ] as usize).min(u.h as usize - 1);
    let (mut own, mut xt) = (0.0f32, 0.0f32);
    for k in 0..KN {
        let j = n + KR as i32 - k as i32;
        own += lt_row[LT_SCALARS + k] * at(m, w, j, cr);
        xt += lt_row[LT_SCALARS + KN + k] * at(m, w, j, adj);
    }
    let sig = lt_row[lt::G_OWN] * own + lt_row[lt::G_X] * xt;
    let nf = n as f32;
    let ds = lt_row[lt::DROP_START];
    let mut env = 1.0f32;
    if ds >= 0.0 {
        let inside = (((nf - ds).min(ds + lt_row[lt::DROP_LEN] - nf)) / 3.0 + 0.5).clamp(0.0, 1.0);
        env = 1.0 - 0.98 * inside;
    }
    let mut noise = lt_row[lt::SIGMA] * gauss(hash3(u.seed, y as u32, n as u32));
    let b = lt_row[lt::BURST];
    if b >= 0.0 && (nf - b).abs() < BURST_HALF {
        noise += u.burst * gauss(hash3(u.seed ^ 0xB0B0, y as u32, n as u32));
    }
    env * sig + noise
}

/// The analytic signal of the RF at `j` (the Hilbert FIR on odd taps).
#[inline]
fn analytic(r: &[f32], k: &Kernels, w: usize, y: usize, j: i32) -> (f32, f32) {
    let mut q = 0.0f32;
    for (i, c) in k.hilb.iter().enumerate() {
        let d = 2 * i as i32 + 1;
        q += c * (at(r, w, j - d, y) - at(r, w, j + d, y));
    }
    (at(r, w, j, y), q)
}

/// Stage 6: FM demodulation — the discriminator's phase step → level; the RF envelope.
pub fn s6(r: &[f32], k: &Kernels, u: &Uni, y: usize, n: i32) -> (f32, f32) {
    let w = u.w as usize;
    let (x0, y0) = analytic(r, k, w, y, n);
    let (x1, y1) = analytic(r, k, w, y, n - 1);
    let re = x0 * x1 + y0 * y1;
    let im = y0 * x1 - x0 * y1;
    let d = atan2_cyc(im, re);
    let level = u.l_sync + (d - u.fsync_n) / u.dev_n;
    let env = (x0 * x0 + y0 * y0).sqrt();
    (q16(level, LEVEL_OFF, LEVEL_SCALE), q16(env, ENV_OFF, ENV_SCALE))
}

/// Stage 7: the dropout compensator (the line before in the field), then LPF ∗ de-emphasis.
pub fn s7(level: &[f32], env: &[f32], k: &Kernels, u: &Uni, y: usize, n: i32) -> f32 {
    let w = u.w as usize;
    let mut s = 0.0f32;
    for (i, c) in k.play.iter().enumerate() {
        let j = n + k.play_c - i as i32;
        let mut v = at(level, w, j, y);
        if u.doc && y >= 2 && at(env, w, j, y) < DOC_THRESHOLD {
            v = at(level, w, j, y - 2);
        }
        s += c * v;
    }
    s
}

/// Stage 8: the colour-under demodulated with the track's rotation undone, the V switch undone, ACC.
pub fn s8(r: &[f32], lt_row: &[f32], k: &Kernels, u: &Uni, y: usize, n: i32) -> (f32, f32) {
    let w = u.w as usize;
    let yf = lt_row[lt::YF];
    let rot = lt_row[lt::ROT];
    let (mut i0, mut q0) = (0.0f32, 0.0f32);
    for (i, c) in k.chroma.iter().enumerate() {
        let j = n + k.chroma_c - i as i32;
        let th = cu_phase(u, j, yf, rot);
        let x = at(r, w, j, y);
        i0 += c * (x * cos_cyc(th));
        q0 += c * (x * sin_cyc(th));
    }
    let acc = lt_row[lt::ACC];
    let uu = 2.0 * i0 * acc;
    let vv = 2.0 * q0 * acc * lt_row[lt::VSIGN];
    (q16(uu, UV_OFF, UV_SCALE), q16(vv, UV_OFF, UV_SCALE))
}

/// One output pixel: the time-base shift per grid row, the bilinear read, the comb, back to RGB.
pub fn fin(s: &Sim, ow: u32, oh: u32, xo: u32, yo: u32) -> [u8; 4] {
    let u = &s.uni;
    let (w, h) = (u.w as usize, u.h as usize);
    let gy = ((yo as f32 + 0.5) * (u.h as f32 / oh as f32) - 0.5).clamp(0.0, (h - 1) as f32);
    let y0 = gy.floor() as usize;
    let y1 = (y0 + 1).min(h - 1);
    let wy = gy - y0 as f32;
    let cd = u.comb_rows as usize;
    let row = |yr: usize| -> (f32, f32, f32) {
        let gx = (u.m as f32 + (xo as f32 + 0.5) * (u.a as f32 / ow as f32) - 0.5 + s.lt[yr * LT_W + lt::TBE])
            .clamp(0.0, (w - 1) as f32);
        let x0 = gx.floor() as i32;
        let x1 = x0 + 1;
        let wx = gx - x0 as f32;
        let uc = |x: i32, ch: &[f32]| -> f32 {
            let a = at(ch, w, x, yr);
            if cd > 0 && yr >= cd { 0.5 * (a + at(ch, w, x, yr - cd)) } else { a }
        };
        let l0 = at(&s.luma, w, x0, yr);
        let l1 = at(&s.luma, w, x1, yr);
        let (u0, u1) = (uc(x0, &s.u), uc(x1, &s.u));
        let (v0, v1) = (uc(x0, &s.v), uc(x1, &s.v));
        (l0 + (l1 - l0) * wx, u0 + (u1 - u0) * wx, v0 + (v1 - v0) * wx)
    };
    let (ya, ua, va) = row(y0);
    let (yb, ub, vb) = row(y1);
    let yy = ya + (yb - ya) * wy;
    let uu = ua + (ub - ua) * wy;
    let vv = va + (vb - va) * wy;
    let r = yy + vv * (1.0 / 0.877);
    let b = yy + uu * (1.0 / 0.492);
    let g = (yy - 0.299 * r - 0.114 * b) * (1.0 / 0.587);
    let q = |c: f32| (c * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8;
    [q(r), q(g), q(b), 255]
}

/// The whole chain over a picture already resampled to the standard's A×H.
pub fn simulate(d: &Deck, rgb: Vec<u8>, burst_index: i64) -> Sim {
    let st = d.st;
    let c = d.words(burst_index);
    let u = Uni::new(d, &c);
    let k = st.kernels();
    let (w, h) = (u.w as usize, u.h as usize);
    let lt = rows(LT_W, h, |y, o: &mut [f32]| fx_video_tape::deck::line_row(&c, y as u32, o));
    let inc = rows(w, h, |y, o: &mut [u32]| {
        for (n, v) in o.iter_mut().enumerate() {
            *v = s1(&rgb, k, &u, y, n as i32);
        }
    });
    // the DDS: the CPU's running sum is exactly the GPU's block prefix (wrapping u32 is associative)
    let phase = rows(w, h, |y, o: &mut [u32]| {
        let mut p = start_phase(&u, y);
        for (n, v) in o.iter_mut().enumerate() {
            p = p.wrapping_add(inc[y * w + n]);
            *v = p;
        }
    });
    let m = rows(w, h, |y, o: &mut [f32]| {
        for (n, v) in o.iter_mut().enumerate() {
            *v = s4(phase[y * w + n], &rgb, k, &u, y, n as i32);
        }
    });
    let r = rows(w, h, |y, o: &mut [f32]| {
        let lr = &lt[y * LT_W..(y + 1) * LT_W];
        for (n, v) in o.iter_mut().enumerate() {
            *v = s5(&m, lr, &u, y, n as i32);
        }
    });
    let dem = rows(w, h, |y, o: &mut [(f32, f32)]| {
        for (n, v) in o.iter_mut().enumerate() {
            *v = s6(&r, k, &u, y, n as i32);
        }
    });
    let (level, env): (Vec<f32>, Vec<f32>) = dem.into_iter().unzip();
    let luma = rows(w, h, |y, o: &mut [f32]| {
        for (n, v) in o.iter_mut().enumerate() {
            *v = s7(&level, &env, k, &u, y, n as i32);
        }
    });
    let uv = rows(w, h, |y, o: &mut [(f32, f32)]| {
        let lr = &lt[y * LT_W..(y + 1) * LT_W];
        for (n, v) in o.iter_mut().enumerate() {
            *v = s8(&r, lr, k, &u, y, n as i32);
        }
    });
    let (uu, vv): (Vec<f32>, Vec<f32>) = uv.into_iter().unzip();
    Sim { st, words: c, uni: u, rgb, lt, inc, phase, m, r, level, env, luma, u: uu, v: vv }
}

/// The frame of `ow×oh` read out of the simulation (RGBA8, opaque).
pub fn read_out(s: &Sim, ow: u32, oh: u32, out: &mut [u8]) {
    let o = rows(ow as usize * 4, oh as usize, |y, row: &mut [u8]| {
        for x in 0..ow as usize {
            row[x * 4..x * 4 + 4].copy_from_slice(&fin(s, ow, oh, x as u32, y as u32));
        }
    });
    out.copy_from_slice(&o);
}
