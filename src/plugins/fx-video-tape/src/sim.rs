//! The numbers the chain's WGSL passes read (`wgsl/s1.wgsl` … `wgsl/fin.wgsl`): the deck's words as
//! the uniforms of one frame. The passes live in the shaders; the f32 model of the same stages, for
//! the physics tests, lives in `tests/model/sim.rs`.

use crate::deck::{wf, Deck, F_COMB, F_DOC, WORDS};

/// The RF envelope below which the dropout compensator takes the line before.
pub const DOC_THRESHOLD: f32 = 0.12;
/// Half-width (samples) of the head-switching transient.
pub const BURST_HALF: f32 = 12.0;

/// The per-frame scalars of the record and playback stages (the passes' uniforms).
#[derive(Debug, Clone, Copy)]
pub struct Uni {
    pub w: u32,
    pub h: u32,
    pub m: u32,
    pub a: u32,
    pub dark: f32,
    pub white: f32,
    /// FM: the sync tip in cycles per sample, the deviation per level unit in cycles per sample.
    pub fsync_n: f32,
    pub dev_n: f32,
    pub l_sync: f32,
    pub a_c: f32,
    /// The tape's saturation `k` of the transfer x/(1 + k|x|), and its in-band equivalents under the
    /// FM carrier (design D9): the carrier's first-harmonic gain and the chroma's gain with the
    /// carrier as its AC bias.
    pub sat: f32,
    pub sat_fm: f32,
    pub sat_c: f32,
    pub fcu_n: f32,
    pub cu_line: f32,
    pub rot: [f32; 2],
    pub v_switch: bool,
    pub seed: u32,
    pub frame: u32,
    pub burst: f32,
    pub doc: bool,
    /// The comb's distance in grid rows (0: off).
    pub comb_rows: u32,
}

impl Uni {
    pub fn new(d: &Deck, c: &[u32; WORDS]) -> Uni {
        let st = d.st;
        let v = &d.v;
        use crate::deck::row;
        let k = v[row::SATURATION].clamp(0.0, 1.0) * 0.6;
        let (fm, cg) = saturation_gains(k);
        Uni {
            w: st.w,
            h: st.h,
            m: st.m,
            a: st.a,
            dark: v[row::DARK_CLIP] as f32,
            white: v[row::WHITE_CLIP] as f32,
            fsync_n: (st.f_sync / st.fs) as f32,
            dev_n: ((st.f_white - st.f_sync) / st.fs / (1.0 - st.l_sync)) as f32,
            l_sync: st.l_sync as f32,
            a_c: d.a_c() as f32,
            sat: k as f32,
            sat_fm: fm as f32,
            sat_c: cg as f32,
            fcu_n: wf(c, crate::deck::w::FCU_N),
            cu_line: st.cu_line as f32,
            rot: [st.rot[0] as f32, st.rot[1] as f32],
            v_switch: st.v_switch,
            seed: c[crate::deck::w::SEED],
            frame: c[crate::deck::w::FRAME],
            burst: wf(c, crate::deck::w::BURST),
            doc: c[crate::deck::w::FLAGS] & F_DOC != 0,
            comb_rows: if c[crate::deck::w::FLAGS] & F_COMB != 0 { 2 * st.comb } else { 0 },
        }
    }
}

/// The in-band effect of the memoryless transfer f(x) = x/(1 + k|x|) on a unit FM carrier plus a
/// small chroma: the carrier's first harmonic a₁ = (1/π)∮ f(cos φ) cos φ dφ, and the chroma's gain
/// under the carrier as bias g = (1/2π)∮ f′(cos φ) dφ. The harmonics themselves (3 fc ≈ 13 MHz) lie
/// above what the heads read and would ALIAS on the 4 fsc grid, so the stage applies these gains
/// instead of the transfer (design D9).
pub fn saturation_gains(k: f64) -> (f64, f64) {
    use kt_plugin_api::math::FrozenMath;
    let n = 512;
    let (mut a1, mut g) = (0.0, 0.0);
    for i in 0..n {
        let ph = std::f64::consts::TAU * (i as f64 + 0.5) / n as f64;
        let x = ph.fcos();
        a1 += x / (1.0 + k * x.abs()) * x;
        g += 1.0 / ((1.0 + k * x.abs()) * (1.0 + k * x.abs()));
    }
    (2.0 * a1 / n as f64, g / n as f64)
}
