//! Audio FX: slowed (rubato-varispeed) + reverb (freeverb) + gain.
//!
//! Architectural decisions that close the crackles of the old engine
//! (defects a–f):
//! - all sound is f32 48kHz/2ch; FX do not see the source sample rate (a);
//! - the state lives in one object per clip, owned by the audio-render thread
//!   via FxStateStore in kt-engine, not shared with prefetch (b);
//! - varispeed is rubato SincFixedOut with continuous consumption of the source:
//!   a fractional cursor, no sample is skipped or duplicated (a, c, d);
//! - parameters are smoothed by one-pole (~20 ms) and NEVER reset
//!   the buffers; a full reset — only reset() at a continuity break (e);
//! - tail() reports the reverb tail — the graph extends the clip window (f).

use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};

pub const CH: usize = AUDIO_CHANNELS;

/// Pre-roll at a continuity break: max(tail, 100 ms) is rendered into garbage,
/// so that the state (reverb, resampler) is warmed up deterministically.
pub const MIN_PREROLL_FRAMES: i64 = (AUDIO_SAMPLE_RATE as i64) / 10;

/// The role of an access to the source. `Warmup` — the pre-roll after a seek: these frames
/// are rendered into garbage and do not go to the output, so their cache miss is NOT an
/// underrun. It used to be one, and the counter grew at each playback start
/// and at each loop wrap-around (item 166).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullKind {
    Output,
    Warmup,
}

pub mod conform;
pub mod geom;
pub mod plan;
pub mod simd;
pub mod video;
mod video_simd;

/// Explicit access to BOTH kernel paths for the parity tests (R.32, t198).
/// The tests do not touch the env (KT_SIMD is read once per process) — they call
/// the scalar and AVX2 directly and compare the bytes.
#[doc(hidden)]
pub mod simd_testing {
    use crate::video::Frame;

    /// Scalar reference of the bilinear.
    #[allow(clippy::too_many_arguments)]
    pub fn bilinear_scalar(
        src: &Frame,
        x0: f64,
        y0: f64,
        cw: f64,
        ch: f64,
        ow: u32,
        oh: u32,
        out: &mut [u8],
    ) {
        crate::video::bilinear_into_scalar(src, x0, y0, cw, ch, ow, oh, out);
    }

    /// AVX2 kernel of the bilinear. Panics if the feature is absent — the calling test
    /// must check its presence itself (skip/KT_REQUIRE_SIMD).
    #[cfg(target_arch = "x86_64")]
    #[allow(clippy::too_many_arguments)]
    pub fn bilinear_avx2(
        src: &Frame,
        x0: f64,
        y0: f64,
        cw: f64,
        ch: f64,
        ow: u32,
        oh: u32,
        out: &mut [u8],
    ) {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: the feature is checked by the line above; the buffer lengths are the
        // responsibility of the test (it creates them itself)
        unsafe { crate::video_simd::bilinear_into_avx2(src, x0, y0, cw, ch, ow, oh, out) };
    }

    /// AVX2 kernel of noise (in-place over the whole frame, the column tail is
    /// not touched — the test finishes it with the scalar path).
    #[cfg(target_arch = "x86_64")]
    pub fn noise_avx2(rgba: &mut [u8], w: u32, h: u32, seed: u32, k: i64) {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: the feature is checked; lengths are the responsibility of the test
        unsafe { crate::video_simd::noise_avx2(rgba, w, h, seed, k) };
    }

    /// AVX2 kernel of the glow additive.
    #[cfg(target_arch = "x86_64")]
    pub fn glow_add_avx2(dst: &mut [u8], up: &[u8], s: f64, pixels: usize) {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: the feature is checked
        unsafe { crate::video_simd::glow_add_avx2(dst, up, s, pixels) };
    }

    /// Scalar references of noise/glow (copies of the bodies from video.rs).
    pub fn noise_scalar(rgba: &mut [u8], w: u32, seed: u32, k: i64) {
        for (p, px) in rgba.chunks_exact_mut(4).enumerate() {
            let (x, y) = (p as u32 % w, p as u32 / w);
            let h = crate::video::noise_hash(x ^ crate::video::noise_hash(y ^ seed));
            let n = ((h >> 16) & 0x7fff) as i64 - 16384;
            let off = (n * k / 32767) as i32;
            for c in px.iter_mut().take(3) {
                *c = (*c as i32 + off).clamp(0, 255) as u8;
            }
        }
    }

    /// Scalar reference of the whole edges effect (a copy of the body from video.rs) — t198
    /// checks against it the result of the PUBLIC chain that goes through the SIMD path.
    pub fn edges_scalar(rgba: &[u8], w: usize, h: usize, threshold: u32) -> Vec<u8> {
        let th = threshold.clamp(1, 255) as i32;
        let luma: Vec<i32> = rgba
            .chunks_exact(4)
            .map(|p| (77 * p[0] as i32 + 150 * p[1] as i32 + 29 * p[2] as i32) >> 8)
            .collect();
        let mut out = vec![0u8; rgba.len()];
        for p in out.chunks_exact_mut(4) {
            p[3] = 255;
        }
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let l = |dx: i64, dy: i64| {
                    luma[(y as i64 + dy) as usize * w + (x as i64 + dx) as usize]
                };
                let gx = -l(-1, -1) - 2 * l(-1, 0) - l(-1, 1) + l(1, -1) + 2 * l(1, 0) + l(1, 1);
                let gy = -l(-1, -1) - 2 * l(0, -1) - l(1, -1) + l(-1, 1) + 2 * l(0, 1) + l(1, 1);
                let mag = (((gx * gx + gy * gy) as f64).sqrt() as i32).min(255);
                let v = if mag < th { 0 } else { mag as u8 };
                let i = (y * w + x) * 4;
                out[i] = v;
                out[i + 1] = v;
                out[i + 2] = v;
            }
        }
        out
    }

    /// Scalar reference of the whole vhs effect (a copy of the body from video.rs).
    pub fn vhs_scalar(
        rgba: &mut [u8],
        w: usize,
        h: usize,
        rgb_shift: u32,
        bleed: f64,
        burst_index: i64,
    ) {
        let shift = rgb_shift.clamp(0, 12) as i64;
        let bleed = bleed.clamp(0.0, 1.0);
        let line_off = ((burst_index / 3000) % 7) as usize;
        let mut row = vec![0u8; w * 4];
        for y in 0..h {
            let scan = if (y + line_off).is_multiple_of(4) { 1i64 } else { 0 };
            let r0 = y * w * 4;
            row.copy_from_slice(&rgba[r0..r0 + w * 4]);
            for x in 0..w {
                let i = r0 + x * 4;
                let xr = (x as i64 + shift + scan).clamp(0, w as i64 - 1) as usize;
                let xb = (x as i64 - shift - scan).clamp(0, w as i64 - 1) as usize;
                let r = row[xr * 4] as f64;
                let b = row[xb * 4 + 2] as f64;
                rgba[i] = (row[x * 4] as f64 * (1.0 - bleed) + r * bleed).round() as u8;
                rgba[i + 2] = (row[x * 4 + 2] as f64 * (1.0 - bleed) + b * bleed).round() as u8;
                if scan == 1 {
                    rgba[i + 1] = row[x * 4 + 1].saturating_sub(18);
                }
            }
        }
    }

    pub fn glow_add_scalar(dst: &mut [u8], up: &[u8], s: f64) {
        for (o, u) in dst.chunks_exact_mut(4).zip(up.chunks_exact(4)) {
            for c in 0..3 {
                o[c] = (o[c] as f64 + u[c] as f64 * s).min(255.0) as u8;
            }
        }
    }

    /// Rounding half-away-from-zero: 4 values via AVX2 emulation
    /// (a micro-proof against `f64::round`).
    #[cfg(target_arch = "x86_64")]
    pub fn round_half_away_avx2(v: [f64; 4]) -> [f64; 4] {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: the feature is checked
        unsafe {
            use std::arch::x86_64::*;
            let x = _mm256_loadu_pd(v.as_ptr());
            let r = crate::video_simd::round_half_away_nonneg(x);
            let mut out = [0f64; 4];
            _mm256_storeu_pd(out.as_mut_ptr(), r);
            out
        }
    }
}
pub mod audio;

pub use audio::{
    AddrKnot, WARP_SOURCE_MARGIN_FRAMES, chain_addr_knots, chain_consumption_window_ext,
    chain_fingerprint, chain_lookahead, chain_source_used, chain_source_window,
    chain_source_window_ext, chain_tail,
};
pub use audio::{
    ChainOpts, ClipProcessor, Freeverb, SmoothedParam, TrackProcessor, Warp, eq_mag_db,
};

/// The time-stretch map MOVED TO kt-model (R.39): it must be called by
/// `clip_slice` — the clip-slice rule — and kt-model cannot depend on kt-fx.
/// Here there is only a re-export: the formula still has ONE home, and the engine
/// and the tests keep calling `kt_fx::…`; not a single call has been touched.
pub use kt_model::{
    AutoProfiles, ParamPoints, RATE_AUDIO_FLOOR, RATE_CLAMP, SignKnot, avg_rate, chain_avg_rate,
    chain_rate, consumed_abs, eff_rate, output_time_for_consumption, output_time_for_source,
    param_at, param_integral, rate_at, rate_reach, rate_signed, sign_knots, source_progress,
    span_over, warp_material_ticks, warp_span_ticks,
};
