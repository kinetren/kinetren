//! The AVX2 twins of the family (moved from kt-fx `video_simd.rs`, plugin-api-barriers 4.1–4.2):
//! edges' luma and Sobel row, noise's grain, vhs's row interior. BYTE-FOR-BYTE the scalar code (R.32): integer arithmetic, the square
//! root in f64 truncated like `as i32`. The host decides when they may run (`BarrierCtx::simd`).

#![cfg(target_arch = "x86_64")]

use std::arch::x86_64::*;

/// Frame luma for `edges`: `(77R + 150G + 29B) >> 8`, 8 pixels per iteration. Integer —
/// byte-for-byte with the scalar.
///
/// # Safety
/// Requires AVX2; `luma` of length >= pixels, `rgba` >= pixels*4.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn luma_avx2(luma: &mut [i32], rgba: &[u8], pixels: usize) {
    unsafe {
        let byte = _mm256_set1_epi32(0xFF);
        let (c77, c150, c29) = (_mm256_set1_epi32(77), _mm256_set1_epi32(150), _mm256_set1_epi32(29));
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
            *pl.add(p) = (77 * (*px) as i32 + 150 * (*px.add(1)) as i32 + 29 * (*px.add(2)) as i32) >> 8;
        }
    }
}

/// Sobel of one row of `edges` for x ∈ [1, w-1): 8 pixels per iteration. The threshold and the
/// output in RGB — as in the scalar (alpha is set by the caller).
///
/// # Safety
/// Requires AVX2; `luma` contains >= (y+2)*w values, `out` >= (y*w+w)*4.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn sobel_row_avx2(out: &mut [u8], luma: &[i32], w: usize, y: usize, th: i32, x_end: usize) -> usize {
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
            _mm256_loadu_si256(pl.add((row * w) as isize as usize + (x as i64 + dx) as usize).cast())
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
            let lo = _mm256_cvttpd_epi32(_mm256_min_pd(_mm256_sqrt_pd(_mm256_cvtepi32_pd(_mm256_castsi256_si128(sq))), c255d));
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

#[target_feature(enable = "avx2")]
#[inline]
unsafe fn round_half_away_nonneg(x: __m256d) -> __m256d {
    let fl = _mm256_floor_pd(x);
    let frac = _mm256_sub_pd(x, fl);
    let ge_half = _mm256_cmp_pd(frac, _mm256_set1_pd(0.5), _CMP_GE_OQ);
    _mm256_add_pd(fl, _mm256_and_pd(ge_half, _mm256_set1_pd(1.0)))
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
            let rowh = kt_plugin_api::barrier::noise_hash(y ^ seed);
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
