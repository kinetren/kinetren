//! AVX2 kernels of the video FX (R.32). All the unsafe of the crate lives here.
//!
//! THE LAW (kt-fx/CLAUDE.md): each kernel is BYTE-FOR-BYTE equal to the scalar twin
//! from video.rs on any CPU — the CPU path stays the determinism reference
//! (invariant No. 11), the goldens t26/t11/t67 are not touched.
//! Hence the bans: FMA (a different rounding), reassociation of sums,
//! rcp/rsqrt, f32 instead of f64, `_mm256_round_pd(NEAREST)` instead of
//! the half-away emulation, changing MXCSR. Lanes are only independent
//! pixels/channels; the order of operations INSIDE a pixel is exactly scalar.
//!
//! Each function is called ONLY after `simd_level() == Avx2`.

#![cfg(target_arch = "x86_64")]

use std::arch::x86_64::*;

use crate::video::Frame;

/// Rounding to nearest, half AWAY from zero — like `f64::round`, only
/// for x >= 0 (our accumulators are non-negative: weights and pixels are >= 0).
///
/// The naive `floor(x + 0.5)` is WRONG: for x = 0.49999999999999994 the sum
/// rounds up to 1.0, and floor gives 1 instead of 0. Therefore we take the
/// fractional part: `x - floor(x)` is computed EXACTLY (both terms are
/// representable, the result is in [0,1)), and the comparison with 0.5 is honest. The exact
/// half (2.5 → 3) goes up — this is what half-away means.
/// `_mm256_round_pd(NEAREST)` is unsuitable here: it is half-to-EVEN (2.5 → 2).
///
/// # Safety
/// Requires AVX (included in AVX2).
#[target_feature(enable = "avx2")]
#[inline]
pub(crate) unsafe fn round_half_away_nonneg(x: __m256d) -> __m256d {
    let fl = _mm256_floor_pd(x);
    let frac = _mm256_sub_pd(x, fl);
    let ge_half = _mm256_cmp_pd(frac, _mm256_set1_pd(0.5), _CMP_GE_OQ);
    _mm256_add_pd(fl, _mm256_and_pd(ge_half, _mm256_set1_pd(1.0)))
}

/// One bilinear tap: 4 channels of the pixel × weight, accumulated in the same
/// order as the scalar `*acc += src[si + c] as f64 * w` (mul, then
/// add — WITHOUT fma).
///
/// # Safety
/// `off + 4 <= len` of the buffer pointed to by `p`; requires AVX2.
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn tap(acc: __m256d, p: *const u8, off: usize, w: f64) -> __m256d {
    unsafe {
        let raw = p.add(off).cast::<u32>().read_unaligned();
        let px = _mm256_cvtepi32_pd(_mm_cvtepu8_epi32(_mm_cvtsi32_si128(raw as i32)));
        _mm256_add_pd(acc, _mm256_mul_pd(px, _mm256_set1_pd(w)))
    }
}

/// Packing 4×f64 (already integers in 0..=255) into 4 RGBA bytes.
///
/// # Safety
/// Requires AVX2.
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn pack_rgba(v: __m256d) -> u32 {
    // values are already integers — the cvtt truncation is equivalent to any mode
    let i32x4 = _mm256_cvttpd_epi32(v);
    let u16x8 = _mm_packus_epi32(i32x4, i32x4);
    let u8x16 = _mm_packus_epi16(u16x8, u16x8);
    _mm_cvtsi128_si32(u8x16) as u32
}

/// AVX2 twin of `bilinear_into` (video.rs): the lanes = 4 CHANNELS of one
/// pixel, four taps in scalar order (0,0) (0,1) (1,0) (1,1).
/// The indices/weights along X are precomputed by the same formula — these are exactly the same
/// values that the scalar recomputes on each row.
///
/// # Safety
/// Requires AVX2 (checked by the calling dispatcher) and correct buffer lengths:
/// `src.rgba.len() >= sw*sh*4`, `out.len() >= ow*oh*4`.
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn bilinear_into_avx2(
    src: &Frame,
    x0: f64,
    y0: f64,
    cw: f64,
    ch: f64,
    ow: u32,
    oh: u32,
    out: &mut [u8],
) {
    unsafe {
        let (sw, sh) = (src.w as i64, src.h as i64);
        // precomputation along X: (index of the left tap, of the right one, fractional part)
        let mut xs: Vec<(usize, usize, f64)> = Vec::with_capacity(ow as usize);
        for ox in 0..ow as i64 {
            let sx = x0 + (ox as f64 + 0.5) / ow as f64 * cw - 0.5;
            let x = sx.floor() as i64;
            let fx = sx - x as f64;
            xs.push((
                (x.clamp(0, sw - 1) * 4) as usize,
                ((x + 1).clamp(0, sw - 1) * 4) as usize,
                fx,
            ));
        }
        let zero = _mm256_setzero_pd();
        let max255 = _mm256_set1_pd(255.0);
        let sptr = src.rgba.as_ptr();
        let optr = out.as_mut_ptr();
        for oy in 0..oh as i64 {
            let sy = y0 + (oy as f64 + 0.5) / oh as f64 * ch - 0.5;
            let y = sy.floor() as i64;
            let fy = sy - y as f64;
            let (wy0, wy1) = (1.0 - fy, fy);
            let row0 = (y.clamp(0, sh - 1) * sw * 4) as usize;
            let row1 = ((y + 1).clamp(0, sh - 1) * sw * 4) as usize;
            let orow = (oy * ow as i64 * 4) as usize;
            for (ox, &(xa, xb, fx)) in xs.iter().enumerate() {
                let (wx0, wx1) = (1.0 - fx, fx);
                let mut acc = _mm256_setzero_pd();
                acc = tap(acc, sptr, row0 + xa, wy0 * wx0);
                acc = tap(acc, sptr, row0 + xb, wy0 * wx1);
                acc = tap(acc, sptr, row1 + xa, wy1 * wx0);
                acc = tap(acc, sptr, row1 + xb, wy1 * wx1);
                let r = round_half_away_nonneg(acc);
                let c = _mm256_min_pd(_mm256_max_pd(r, zero), max255);
                optr.add(orow + ox * 4).cast::<u32>().write_unaligned(pack_rgba(c));
            }
        }
    }
}

