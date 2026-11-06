//! The two VHS standards as numbers (note F-FX-009, steps 1–16; design D1) and the fixed filters
//! designed from them once (design D4). Every per-standard constant the stages read lives here; the
//! WGSL text gets the same f32 values pasted in (`wgsl_consts`).

use std::sync::OnceLock;

use kt_plugin_api::math::FrozenMath;

/// The grid's half-width of the channel kernels (±40 samples, 81 taps).
pub const KR: usize = 40;
pub const KN: usize = 2 * KR + 1;
/// Taps of the fixed filters' symmetric parts.
const LUMA_LPF_R: i32 = 12;
const CHROMA_LPF_R: i32 = 32;
const HILBERT_R: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Std {
    Pal,
    Ntsc,
}

/// The numbers of one standard.
#[derive(Debug, Clone)]
pub struct Standard {
    pub std: Std,
    /// The sample rate of the grid, 4 fsc (Hz).
    pub fs: f64,
    /// Samples per grid line (the whole line), active samples, the samples before the active part
    /// (sync and back porch, at the blanking level); rows of the frame.
    pub w: u32,
    pub a: u32,
    pub m: u32,
    pub h: u32,
    /// The line period (s) and the frame rate (frames/s).
    pub t_line: f64,
    pub frame_rate: f64,
    /// FM: the sync tip and white carriers (Hz); the sync tip's level (white = 1, black = 0).
    pub f_sync: f64,
    pub f_white: f64,
    pub l_sync: f64,
    /// The colour-under carrier (Hz), its phase advance per line (cycles, the fraction of fcu/fH),
    /// the per-line rotation of tracks A and B (quarter turns: vhs-decode `chroma_rotation`).
    pub fcu: f64,
    pub cu_line: f64,
    pub rot: [i32; 2],
    /// Comb distance in lines of a field (1 H NTSC, 2 H PAL).
    pub comb: u32,
    /// PAL's V switch: V alternates sign every line.
    pub v_switch: bool,
    /// Writing speed of the head over the tape (m/s), the SP track pitch (m), the drum rate (rev/s).
    pub v_write: f64,
    pub pitch_sp: f64,
    pub drum_rate: f64,
}

impl Standard {
    pub fn get(s: Std) -> &'static Standard {
        static PAL: OnceLock<Standard> = OnceLock::new();
        static NTSC: OnceLock<Standard> = OnceLock::new();
        match s {
            Std::Pal => PAL.get_or_init(|| {
                let fh = 15_625.0;
                Standard {
                    std: s,
                    fs: 4.0 * 4_433_618.75,
                    // the full 64 µs line at 4 fsc: 922 active samples (52 µs), the front porch
                    // (1.65 µs, 29) on the right, sync and back porch (10.4 µs, 184) on the left
                    w: 1135,
                    a: 922,
                    m: 184,
                    h: 576,
                    t_line: 1.0 / fh,
                    frame_rate: 25.0,
                    f_sync: 3.8e6,
                    f_white: 4.8e6,
                    l_sync: -0.43,
                    // 40.125 fH (vhs-decode: (625·25)·40 + 1953)
                    fcu: 626_953.125,
                    cu_line: 0.125,
                    rot: [0, -1],
                    comb: 2,
                    v_switch: true,
                    v_write: 4.85,
                    pitch_sp: 49e-6,
                    drum_rate: 25.0,
                }
            }),
            Std::Ntsc => NTSC.get_or_init(|| {
                let fh = 15_734.264;
                Standard {
                    std: s,
                    fs: 4.0 * 3_579_545.45,
                    // the full 63.56 µs line at 4 fsc: 754 active, front porch 21, sync + back porch 135
                    w: 910,
                    a: 754,
                    m: 135,
                    h: 480,
                    t_line: 1.0 / fh,
                    frame_rate: 30.0 / 1.001,
                    f_sync: 3.4e6,
                    f_white: 4.4e6,
                    l_sync: -0.40,
                    fcu: 40.0 * fh,
                    cu_line: 0.0,
                    rot: [-1, 1],
                    comb: 1,
                    v_switch: false,
                    v_write: 5.80,
                    pitch_sp: 58e-6,
                    drum_rate: 30.0 / 1.001,
                }
            }),
        }
    }

    /// The fixed filters of this standard, designed once (D4).
    pub fn kernels(&self) -> &'static Kernels {
        static PAL: OnceLock<Kernels> = OnceLock::new();
        static NTSC: OnceLock<Kernels> = OnceLock::new();
        match self.std {
            Std::Pal => PAL.get_or_init(|| Kernels::design(self)),
            Std::Ntsc => NTSC.get_or_init(|| Kernels::design(self)),
        }
    }

    /// Samples per second as an f32, and the samples of a µs.
    pub fn per_us(&self) -> f64 {
        self.fs * 1e-6
    }
}

/// The fixed filters: the record luma (LPF 3 MHz ∗ pre-emphasis), the playback luma (LPF ∗
/// de-emphasis), the chroma LPF (0.5 MHz) and the Hilbert FIR (odd taps 1, 3, …, 15).
/// `rec[k]` weighs `Y(n + rec_c − k)`, `play[k]` weighs `v(n + play_c − k)`, `chroma[k]` weighs
/// `x(n + CHROMA_LPF_R − k)`, `hilb[i]` weighs `r(n − (2i+1)) − r(n + 2i + 1)`.
#[derive(Debug, Clone)]
pub struct Kernels {
    pub rec: Vec<f32>,
    pub rec_c: i32,
    pub play: Vec<f32>,
    pub play_c: i32,
    pub chroma: Vec<f32>,
    pub chroma_c: i32,
    pub hilb: Vec<f32>,
}

/// The emphasis: τ₁ = 1.3 µs and a ×5 (13.98 dB) HF gain (vhs-decode `deemph_tau`, `deemph_gain`).
pub const EMPH_TAU1: f64 = 1.3e-6;
pub const EMPH_GAIN: f64 = 5.0;

fn hann(k: i32, r: i32) -> f64 {
    0.5 + 0.5 * (std::f64::consts::PI * k as f64 / (r as f64 + 1.0)).fcos()
}

/// A Hann-windowed sinc low-pass of cut-off `fc` at `fs`, ±r taps, unit DC gain.
fn lowpass(fc: f64, fs: f64, r: i32) -> Vec<f64> {
    let x = 2.0 * fc / fs;
    let mut h: Vec<f64> = (-r..=r)
        .map(|k| {
            let s = if k == 0 { 1.0 } else { (std::f64::consts::PI * x * k as f64).fsin() / (std::f64::consts::PI * x * k as f64) };
            x * s * hann(k, r)
        })
        .collect();
    let sum: f64 = h.iter().sum();
    h.iter_mut().for_each(|v| *v /= sum);
    h
}

/// A causal first-order low-pass of time constant `tau`, truncated at `n` taps, unit DC gain.
fn exp_lp(tau: f64, fs: f64, n: usize) -> Vec<f64> {
    let r = (-1.0 / (fs * tau)).fexp();
    let mut h: Vec<f64> = (0..n).map(|j| (1.0 - r) * r.fpowf(j as f64)).collect();
    let sum: f64 = h.iter().sum();
    h.iter_mut().for_each(|v| *v /= sum);
    h
}

fn conv(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut o = vec![0.0; a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            o[i + j] += x * y;
        }
    }
    o
}

impl Kernels {
    fn design(s: &Standard) -> Kernels {
        let l = lowpass(3.0e6, s.fs, LUMA_LPF_R);
        let g = EMPH_GAIN;
        // pre-emphasis H = G − (G−1)/(1 + sτ₂), τ₂ = τ₁/G: e = G·y − (G−1)·LP_τ₂(y), on the LPF'd luma
        let tau2 = EMPH_TAU1 / g;
        let lp2 = exp_lp(tau2, s.fs, (8.0 * s.fs * tau2).ceil() as usize);
        let l2 = conv(&l, &lp2);
        let rec: Vec<f32> = (0..l2.len())
            .map(|k| {
                let a = if k < l.len() { l[k] } else { 0.0 };
                (g * a - (g - 1.0) * l2[k]) as f32
            })
            .collect();
        // de-emphasis H = 1/G + (1 − 1/G)/(1 + sτ₁)
        let lp1 = exp_lp(EMPH_TAU1, s.fs, (5.0 * s.fs * EMPH_TAU1).ceil() as usize);
        let l1 = conv(&l, &lp1);
        let play: Vec<f32> = (0..l1.len())
            .map(|k| {
                let a = if k < l.len() { l[k] } else { 0.0 };
                (a / g + (1.0 - 1.0 / g) * l1[k]) as f32
            })
            .collect();
        let chroma: Vec<f32> = lowpass(0.5e6, s.fs, CHROMA_LPF_R).iter().map(|v| *v as f32).collect();
        // Hilbert: 2/(πk) on odd k, Blackman window
        let hilb: Vec<f32> = (0..=HILBERT_R / 2)
            .map(|i| {
                let k = (2 * i + 1) as f64;
                let w = 0.42 + 0.5 * (std::f64::consts::PI * k / (HILBERT_R as f64 + 1.0)).fcos()
                    + 0.08 * (2.0 * std::f64::consts::PI * k / (HILBERT_R as f64 + 1.0)).fcos();
                (2.0 / (std::f64::consts::PI * k) * w) as f32
            })
            .collect();
        Kernels { rec, rec_c: LUMA_LPF_R, play, play_c: LUMA_LPF_R, chroma, chroma_c: CHROMA_LPF_R, hilb }
    }
}

/// The constants every pass shares: `KR`, `KN` and the centres of the kernels.
pub fn wgsl_consts() -> String {
    format!(
        "const K_REC_C: i32 = {LUMA_LPF_R};\nconst K_PLAY_C: i32 = {LUMA_LPF_R};\nconst K_CHROMA_C: i32 = {CHROMA_LPF_R};\nconst KR: i32 = {KR};\nconst KN: i32 = {KN};\n"
    )
}

/// A pass's text with its taps written out, one statement per tap (render-speed-no-card D3):
/// `// @TAPS REC <statement>` repeats the statement for the kernel of `std` with `{i}` the index and
/// `{c}` the coefficient, `// @TAPS KN <statement>` for the row kernel's `KN` taps. The f32 values
/// are printed with `{:?}`, which round-trips exactly: the GPU convolves with the same bits. A loop
/// over a table costs a software rasterizer ten times this: unrolled, the index is a constant and
/// the taps are plain arithmetic; a bound the compiler cannot see, or one past its limit of 32,
/// stays a masked loop with a per-lane table read.
pub fn wgsl_taps(pass: &str, std: Std) -> String {
    let k = Standard::get(std).kernels();
    let mut out = String::new();
    for line in pass.lines() {
        let Some(rest) = line.trim_start().strip_prefix("// @TAPS ") else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let (name, body) = rest.split_once(' ').expect("a tap line names its kernel");
        let taps: Vec<f32> = match name {
            "REC" => k.rec.clone(),
            "PLAY" => k.play.clone(),
            "CHROMA" => k.chroma.clone(),
            "HILB" => k.hilb.clone(),
            "KN" => vec![0.0; KN],
            other => panic!("no kernel {other}"),
        };
        for (i, c) in taps.iter().enumerate() {
            out.push_str(&body.replace("{i}", &i.to_string()).replace("{c}", &format!("{c:?}")));
            out.push('\n');
        }
    }
    out
}
