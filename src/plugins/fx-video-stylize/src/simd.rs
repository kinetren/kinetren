//! The AVX2 twins of edges (moved from kt-fx `video_simd.rs`, plugin-api-barriers 4.1): the frame's
//! luma and one Sobel row. BYTE-FOR-BYTE the scalar code (R.32): integer arithmetic, the square
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
