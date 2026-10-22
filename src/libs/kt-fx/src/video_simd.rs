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

/// AVX2 twin of `noise`: counter-based hash over (x, y, seed) and the offset of
/// RGB with a clamp. All integer — byte-for-byte with the scalar on any
/// CPU. 8 pixels per iteration; the row tail is finished by the caller.
///
/// The division `n*k/32767` (truncation TOWARD ZERO, n can be negative)
/// is computed as `sign(v) * ((|v| + (|v|>>15) + 1) >> 15)` — the identity
/// is verified by enumerating the whole domain (n ∈ ±16384, k ≤ 32) in t198.
///
/// # Safety
/// Requires AVX2; `rgba` of length >= `w*h*4`.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn noise_avx2(rgba: &mut [u8], w: u32, h: u32, seed: u32, k: i64) {
    unsafe {
        let kv = _mm256_set1_epi32(k as i32);
        let byte = _mm256_set1_epi32(0xFF);
        let c255 = _mm256_set1_epi32(255);
        let zero = _mm256_setzero_si256();
        let lane_x = _mm256_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7);
        let p = rgba.as_mut_ptr();
        let vec_w = (w & !7) as usize;
        for y in 0..h {
            let rowh = crate::video::noise_hash(y ^ seed);
            let rowh_v = _mm256_set1_epi32(rowh as i32);
            let row = (y as usize) * (w as usize) * 4;
            let mut x = 0usize;
            while x < vec_w {
                // h = noise_hash(x ^ rowh) — lowbias32 in the vector
                let mut hv =
                    _mm256_xor_si256(_mm256_add_epi32(_mm256_set1_epi32(x as i32), lane_x), rowh_v);
                hv = _mm256_xor_si256(hv, _mm256_srli_epi32::<16>(hv));
                hv = _mm256_mullo_epi32(hv, _mm256_set1_epi32(0x7feb_352du32 as i32));
                hv = _mm256_xor_si256(hv, _mm256_srli_epi32::<15>(hv));
                hv = _mm256_mullo_epi32(hv, _mm256_set1_epi32(0x846c_a68bu32 as i32));
                hv = _mm256_xor_si256(hv, _mm256_srli_epi32::<16>(hv));
                // n = ((h >> 16) & 0x7fff) - 16384; v = n * k
                let n = _mm256_sub_epi32(
                    _mm256_and_si256(_mm256_srli_epi32::<16>(hv), _mm256_set1_epi32(0x7fff)),
                    _mm256_set1_epi32(16384),
                );
                let v = _mm256_mullo_epi32(n, kv);
                let a = _mm256_abs_epi32(v);
                let q = _mm256_srli_epi32::<15>(_mm256_add_epi32(
                    _mm256_add_epi32(a, _mm256_srli_epi32::<15>(a)),
                    _mm256_set1_epi32(1),
                ));
                // sign_epi32(q, v) = q * sign(v), and 0 when v == 0
                let off = _mm256_sign_epi32(q, v);
                let px = _mm256_loadu_si256(p.add(row + x * 4).cast());
                let clamp = |c: __m256i| {
                    _mm256_min_epi32(_mm256_max_epi32(_mm256_add_epi32(c, off), zero), c255)
                };
                let r = clamp(_mm256_and_si256(px, byte));
                let g = clamp(_mm256_and_si256(_mm256_srli_epi32::<8>(px), byte));
                let b = clamp(_mm256_and_si256(_mm256_srli_epi32::<16>(px), byte));
                let out = _mm256_or_si256(
                    _mm256_or_si256(r, _mm256_slli_epi32::<8>(g)),
                    _mm256_or_si256(
                        _mm256_slli_epi32::<16>(b),
                        // alpha is untouched (the scalar only corrects RGB)
                        _mm256_and_si256(px, _mm256_set1_epi32(0xFF00_0000u32 as i32)),
                    ),
                );
                _mm256_storeu_si256(p.add(row + x * 4).cast(), out);
                x += 8;
            }
        }
    }
}

/// AVX2 twin of the additive finish of `glow`: `o + u*s` with a ceiling of 255 and
/// TRUNCATION (the scalar does `as u8` without round). The lanes = 4 channels of a
/// pixel; the alpha byte is taken back from the original (the scalar does not touch it).
///
/// # Safety
/// Requires AVX2; `dst`/`up` of length >= `pixels*4`.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn glow_add_avx2(dst: &mut [u8], up: &[u8], s: f64, pixels: usize) {
    unsafe {
        let sv = _mm256_set1_pd(s);
        let max255 = _mm256_set1_pd(255.0);
        let (pd, pu) = (dst.as_mut_ptr(), up.as_ptr());
        for p in 0..pixels {
            let off = p * 4;
            let orig = pd.add(off).cast::<u32>().read_unaligned();
            let o = _mm256_cvtepi32_pd(_mm_cvtepu8_epi32(_mm_cvtsi32_si128(orig as i32)));
            let u = _mm256_cvtepi32_pd(_mm_cvtepu8_epi32(_mm_cvtsi32_si128(
                pu.add(off).cast::<u32>().read_unaligned() as i32,
            )));
            // scalar order: first u*s, then o + ...
            let r = _mm256_min_pd(_mm256_add_pd(o, _mm256_mul_pd(u, sv)), max255);
            let packed = pack_rgba(r);
            // alpha — from the original
            pd.add(off)
                .cast::<u32>()
                .write_unaligned((packed & 0x00FF_FFFF) | (orig & 0xFF00_0000));
        }
    }
}

/// Frame luma for `edges`: `(77R + 150G + 29B) >> 8`, 8 pixels per
/// iteration. Integer — byte-for-byte with the scalar.
///
/// # Safety
/// Requires AVX2; `luma` of length >= pixels, `rgba` >= pixels*4.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn luma_avx2(luma: &mut [i32], rgba: &[u8], pixels: usize) {
    unsafe {
        let byte = _mm256_set1_epi32(0xFF);
        let (c77, c150, c29) =
            (_mm256_set1_epi32(77), _mm256_set1_epi32(150), _mm256_set1_epi32(29));
        let (ps, pl) = (rgba.as_ptr(), luma.as_mut_ptr());
        let n = pixels & !7;
        let mut p = 0usize;
        while p < n {
            let v = _mm256_loadu_si256(ps.add(p * 4).cast());
            let r = _mm256_mullo_epi32(_mm256_and_si256(v, byte), c77);
            let g = _mm256_mullo_epi32(_mm256_and_si256(_mm256_srli_epi32::<8>(v), byte), c150);
            let b = _mm256_mullo_epi32(_mm256_and_si256(_mm256_srli_epi32::<16>(v), byte), c29);
            let l = _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_add_epi32(r, g), b));
            _mm256_storeu_si256(pl.add(p).cast(), l);
            p += 8;
        }
        for p in n..pixels {
            let i = p * 4;
            let px = ps.add(i);
            *pl.add(p) =
                (77 * (*px) as i32 + 150 * (*px.add(1)) as i32 + 29 * (*px.add(2)) as i32) >> 8;
        }
    }
}

/// Sobel of one row of `edges` for x ∈ [1, w-1): 8 pixels per iteration.
/// The threshold and the output in RGB — as in the scalar (alpha is set by the caller).
///
/// # Safety
/// Requires AVX2; `luma` contains >= (y+2)*w values, `out` >= (y*w+w)*4.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn sobel_row_avx2(
    out: &mut [u8],
    luma: &[i32],
    w: usize,
    y: usize,
    th: i32,
    x_end: usize,
) -> usize {
    unsafe {
        let pl = luma.as_ptr();
        let po = out.as_mut_ptr();
        let thv = _mm256_set1_epi32(th);
        let c255i = _mm256_set1_epi32(255);
        let c255d = _mm256_set1_pd(255.0);
        let two = _mm256_set1_epi32(2);
        let alpha = _mm256_set1_epi32(0xFF00_0000u32 as i32);
        let mut x = 1usize;
        // load rows y-1, y, y+1 with offsets -1, 0, +1
        let ld = |row: usize, dx: i64, x: usize| -> __m256i {
            _mm256_loadu_si256(
                pl.add((row * w) as isize as usize + (x as i64 + dx) as usize).cast(),
            )
        };
        while x + 8 <= x_end {
            let (ym, yc, yp) = (y - 1, y, y + 1);
            let a_ul = ld(ym, -1, x);
            let a_uc = ld(ym, 0, x);
            let a_ur = ld(ym, 1, x);
            let a_cl = ld(yc, -1, x);
            let a_cr = ld(yc, 1, x);
            let a_dl = ld(yp, -1, x);
            let a_dc = ld(yp, 0, x);
            let a_dr = ld(yp, 1, x);
            // gx = -ul - 2*cl - dl + ur + 2*cr + dr
            let gx = _mm256_add_epi32(
                _mm256_sub_epi32(
                    _mm256_sub_epi32(_mm256_sub_epi32(a_ur, a_ul), _mm256_mullo_epi32(a_cl, two)),
                    a_dl,
                ),
                _mm256_add_epi32(_mm256_mullo_epi32(a_cr, two), a_dr),
            );
            // gy = -ul - 2*uc - ur + dl + 2*dc + dr
            let gy = _mm256_add_epi32(
                _mm256_sub_epi32(
                    _mm256_sub_epi32(_mm256_sub_epi32(a_dl, a_ul), _mm256_mullo_epi32(a_uc, two)),
                    a_ur,
                ),
                _mm256_add_epi32(_mm256_mullo_epi32(a_dc, two), a_dr),
            );
            let sq = _mm256_add_epi32(_mm256_mullo_epi32(gx, gx), _mm256_mullo_epi32(gy, gy));
            // sqrt in f64 in two halves, truncation like `as i32`
            let lo = _mm256_cvttpd_epi32(_mm256_min_pd(
                _mm256_sqrt_pd(_mm256_cvtepi32_pd(_mm256_castsi256_si128(sq))),
                c255d,
            ));
            let hi = _mm256_cvttpd_epi32(_mm256_min_pd(
                _mm256_sqrt_pd(_mm256_cvtepi32_pd(_mm256_extracti128_si256::<1>(sq))),
                c255d,
            ));
            let mag = _mm256_min_epi32(_mm256_set_m128i(hi, lo), c255i);
            // v = if mag < th { 0 } else { mag }
            let keep = _mm256_cmpgt_epi32(thv, mag); // th > mag
            let v = _mm256_andnot_si256(keep, mag);
            let px = _mm256_or_si256(
                _mm256_or_si256(v, _mm256_slli_epi32::<8>(v)),
                _mm256_or_si256(_mm256_slli_epi32::<16>(v), alpha),
            );
            _mm256_storeu_si256(po.add((y * w + x) * 4).cast(), px);
            x += 8;
        }
        x
    }
}

/// Lerp of 8 integer lanes in f64 with half-away rounding: returns
/// `round(a*(1-bleed) + b*bleed)` — the order of operations is scalar.
///
/// # Safety
/// Requires AVX2.
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn lerp8_round(a: __m256i, b: __m256i, inv: __m256d, bl: __m256d) -> __m256i {
    unsafe {
        let mix = |ai: __m128i, bi: __m128i| -> __m128i {
            let av = _mm256_cvtepi32_pd(ai);
            let bv = _mm256_cvtepi32_pd(bi);
            let r = _mm256_add_pd(_mm256_mul_pd(av, inv), _mm256_mul_pd(bv, bl));
            _mm256_cvttpd_epi32(round_half_away_nonneg(r))
        };
        let lo = mix(_mm256_castsi256_si128(a), _mm256_castsi256_si128(b));
        let hi = mix(_mm256_extracti128_si256::<1>(a), _mm256_extracti128_si256::<1>(b));
        _mm256_set_m128i(hi, lo)
    }
}

/// AVX2 twin of `vhs` for the INTERIOR of a row (where the xr/xb clamps are inactive):
/// RGB shift with lerp and scan lines. Returns the x at which it stopped
/// — the tails and edges are finished by the caller's scalar loop.
///
/// # Safety
/// Requires AVX2; `row` — a copy of the row of length w*4, `dst_row` — the same
/// row of the frame.
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn vhs_row_avx2(
    dst_row: &mut [u8],
    row: &[u8],
    w: usize,
    d: usize,
    bleed: f64,
    scan: bool,
    x_from: usize,
) -> usize {
    unsafe {
        if w <= 2 * d + 8 {
            return x_from; // there is no interior — everything by scalar
        }
        let byte = _mm256_set1_epi32(0xFF);
        let inv = _mm256_set1_pd(1.0 - bleed);
        let bl = _mm256_set1_pd(bleed);
        let g_sub = _mm256_set1_epi32(18);
        let zero = _mm256_setzero_si256();
        let pr = row.as_ptr();
        let pd = dst_row.as_mut_ptr();
        let x_end = w - d; // when x < w-d the index x+d <= w-1 without a clamp
        let mut x = x_from.max(d);
        while x + 8 <= x_end {
            let cur = _mm256_loadu_si256(pr.add(x * 4).cast());
            let right = _mm256_loadu_si256(pr.add((x + d) * 4).cast());
            let left = _mm256_loadu_si256(pr.add((x - d) * 4).cast());
            let r =
                lerp8_round(_mm256_and_si256(cur, byte), _mm256_and_si256(right, byte), inv, bl);
            let b = lerp8_round(
                _mm256_and_si256(_mm256_srli_epi32::<16>(cur), byte),
                _mm256_and_si256(_mm256_srli_epi32::<16>(left), byte),
                inv,
                bl,
            );
            let g0 = _mm256_and_si256(_mm256_srli_epi32::<8>(cur), byte);
            let g = if scan {
                _mm256_max_epi32(_mm256_sub_epi32(g0, g_sub), zero) // saturating_sub(18)
            } else {
                g0
            };
            let out = _mm256_or_si256(
                _mm256_or_si256(r, _mm256_slli_epi32::<8>(g)),
                _mm256_or_si256(
                    _mm256_slli_epi32::<16>(b),
                    // alpha is untouched — taken from the copy of the row
                    _mm256_and_si256(cur, _mm256_set1_epi32(0xFF00_0000u32 as i32)),
                ),
            );
            _mm256_storeu_si256(pd.add(x * 4).cast(), out);
            x += 8;
        }
        x
    }
}
