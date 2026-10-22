//! AVX2 composite kernels (R.32). All the unsafe of the crate lives here.
//!
//! The LAW is the same as in kt-fx/src/video_simd.rs: byte-for-byte with the scalar
//! from video.rs. The composite is integer, so “bit for bit” here is
//! a question of exactly repeating the divisions and BRANCHES (in alpha_over the branch
//! sa == 0 leaves dst untouched — a branchless formula would spoil it;
//! see the select below).
//!
//! Each function is called ONLY after `simd_level() == Avx2`.

#![cfg(target_arch = "x86_64")]

use std::arch::x86_64::*;

/// Integer `x / 255` for 8 u32 lanes — the magic multiplication
/// `(x * 0x8080_8081) >> 39` in 64-bit width, exact for ALL of u32
/// (the same constant the compiler generates for `x / 255`).
///
/// The popular “fast” formula `t = x+1; (t + (t>>8)) >> 8` is NOT suitable here: it is wrong
/// already at 255·255·255 (gives 65024 instead of 65025) —
/// caught by the exhaustive t198_div255_exhaustive.
///
/// # Safety
/// Requires AVX2.
#[target_feature(enable = "avx2")]
#[inline]
pub(crate) unsafe fn div255(x: __m256i) -> __m256i {
    let magic = _mm256_set1_epi64x(0x8080_8081u32 as i64);
    // mul_epu32 multiplies the EVEN 32-bit lanes, giving 4×u64
    let even = _mm256_srli_epi64::<39>(_mm256_mul_epu32(x, magic));
    let odd = _mm256_srli_epi64::<39>(_mm256_mul_epu32(_mm256_srli_epi64::<32>(x), magic));
    _mm256_blend_epi32::<0b1010_1010>(even, _mm256_slli_epi64::<32>(odd))
}

/// AVX2 twin of `dissolve_into`: `(av*ik + bv*k + 2048) >> 12` over all
/// four channels, 4 pixels per iteration. `av*ik + bv*k` is computed by
/// one `madd_epi16` — exactly the same integer sum.
/// Substituting A = 255 when `!has_alpha` is done with an `or` with the alpha mask
/// (255|x == 255 in the alpha byte) — algebraically the same as the scalar
/// `if ch == 3 && !has_alpha { 255 }`.
///
/// # Safety
/// Requires AVX2; `a`/`b`/`dst` of length >= `pixels * 4`.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn dissolve_avx2(
    dst: &mut [u8],
    a: &[u8],
    b: &[u8],
    a_has_alpha: bool,
    b_has_alpha: bool,
    k: u32,
    pixels: usize,
) {
    unsafe {
        let ik = 4096 - k;
        // pairs [ik, k] as i16 inside an i32 lane: madd will sum them
        let coef = _mm256_set1_epi32(((k as i32) << 16) | (ik as i32));
        let round = _mm256_set1_epi32(2048);
        let alpha_fill = _mm_set1_epi32(0xFF00_0000u32 as i32);
        let (pa, pb, pd) = (a.as_ptr(), b.as_ptr(), dst.as_mut_ptr());
        let vec_pixels = pixels & !3; // the tail — scalar in the caller
        let mut p = 0usize;
        while p < vec_pixels {
            let off = p * 4;
            let mut va = _mm_loadu_si128(pa.add(off).cast());
            let mut vb = _mm_loadu_si128(pb.add(off).cast());
            if !a_has_alpha {
                va = _mm_or_si128(va, alpha_fill);
            }
            if !b_has_alpha {
                vb = _mm_or_si128(vb, alpha_fill);
            }
            // interleave bytes a and b, widen to u16 and reduce with madd
            let lo = _mm256_madd_epi16(_mm256_cvtepu8_epi16(_mm_unpacklo_epi8(va, vb)), coef);
            let hi = _mm256_madd_epi16(_mm256_cvtepu8_epi16(_mm_unpackhi_epi8(va, vb)), coef);
            let lo = _mm256_srli_epi32::<12>(_mm256_add_epi32(lo, round));
            let hi = _mm256_srli_epi32::<12>(_mm256_add_epi32(hi, round));
            // packing via the 128-bit halves — the element order
            // is preserved (a 256-bit packus would shuffle the lanes)
            let lo16 =
                _mm_packus_epi32(_mm256_castsi256_si128(lo), _mm256_extracti128_si256::<1>(lo));
            let hi16 =
                _mm_packus_epi32(_mm256_castsi256_si128(hi), _mm256_extracti128_si256::<1>(hi));
            _mm_storeu_si128(pd.add(off).cast(), _mm_packus_epi16(lo16, hi16));
            p += 4;
        }
    }
}

/// AVX2 twin of `scale_alpha_into`: RGB is copied, alpha is multiplied by
/// k/4096 with rounding `(a*k + 2048) >> 12`; a frame without alpha is considered
/// opaque (A = 255). 8 pixels per iteration.
///
/// # Safety
/// Requires AVX2; `src`/`dst` of length >= `pixels * 4`.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn scale_alpha_avx2(
    dst: &mut [u8],
    src: &[u8],
    has_alpha: bool,
    k: u32,
    pixels: usize,
) {
    unsafe {
        let kv = _mm256_set1_epi32(k as i32);
        let round = _mm256_set1_epi32(2048);
        let rgb_mask = _mm256_set1_epi32(0x00FF_FFFF);
        let opaque = _mm256_set1_epi32(255);
        let (ps, pd) = (src.as_ptr(), dst.as_mut_ptr());
        let vec_pixels = pixels & !7;
        let mut p = 0usize;
        while p < vec_pixels {
            let off = p * 4;
            let v = _mm256_loadu_si256(ps.add(off).cast());
            let a = if has_alpha { _mm256_srli_epi32::<24>(v) } else { opaque };
            let na = _mm256_srli_epi32::<12>(_mm256_add_epi32(_mm256_mullo_epi32(a, kv), round));
            let out = _mm256_or_si256(_mm256_and_si256(v, rgb_mask), _mm256_slli_epi32::<24>(na));
            _mm256_storeu_si256(pd.add(off).cast(), out);
            p += 8;
        }
    }
}

/// AVX2 twin of `alpha_over` (straight alpha, integer):
/// `dst = (s*sa + d*inv*da/255)/255`, alpha `sa + da*inv/255`, both with a
/// ceiling of 255. 8 pixels per iteration.
///
/// The scalar's branches are reproduced as follows:
/// - `sa == 0` — a REQUIRED select: the scalar leaves dst untouched,
///   while the formula would give `d*da/255 != d`;
/// - `sa == 255` — a separate select is NOT needed: at `inv == 0` the formula
///   collapses to a copy exactly (`(s*255+0)/255 == s`, alpha `255+0`),
///   which is nailed down by enumeration of (sa, da) in t198.
///
/// # Safety
/// Requires AVX2; `src`/`dst` of length >= `pixels * 4`.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn alpha_over_avx2(dst: &mut [u8], src: &[u8], pixels: usize) {
    unsafe {
        let byte = _mm256_set1_epi32(0xFF);
        let c255 = _mm256_set1_epi32(255);
        let zero = _mm256_setzero_si256();
        let (ps, pd) = (src.as_ptr(), dst.as_mut_ptr());
        let vec_pixels = pixels & !7;
        let mut p = 0usize;
        while p < vec_pixels {
            let off = p * 4;
            let s = _mm256_loadu_si256(ps.add(off).cast());
            let d = _mm256_loadu_si256(pd.add(off).cast());
            let sa = _mm256_srli_epi32::<24>(s);
            let da = _mm256_srli_epi32::<24>(d);
            let inv = _mm256_sub_epi32(c255, sa);
            // colour: (s*sa + (d*inv*da)/255) / 255, ceiling 255.
            // The channels are unrolled explicitly: an AVX2 shift requires a constant, and a
            // closure would not inherit target_feature (it would not inline).
            let sr = _mm256_and_si256(s, byte);
            let dr = _mm256_and_si256(d, byte);
            let sg = _mm256_and_si256(_mm256_srli_epi32::<8>(s), byte);
            let dg = _mm256_and_si256(_mm256_srli_epi32::<8>(d), byte);
            let sb = _mm256_and_si256(_mm256_srli_epi32::<16>(s), byte);
            let db = _mm256_and_si256(_mm256_srli_epi32::<16>(d), byte);
            let mix = |sc, dc| {
                let back = div255(_mm256_mullo_epi32(_mm256_mullo_epi32(dc, inv), da));
                let num = _mm256_add_epi32(_mm256_mullo_epi32(sc, sa), back);
                _mm256_min_epu32(div255(num), c255)
            };
            let r = mix(sr, dr);
            let g = mix(sg, dg);
            let b = mix(sb, db);
            let na =
                _mm256_min_epu32(_mm256_add_epi32(sa, div255(_mm256_mullo_epi32(da, inv))), c255);
            let out = _mm256_or_si256(
                _mm256_or_si256(r, _mm256_slli_epi32::<8>(g)),
                _mm256_or_si256(_mm256_slli_epi32::<16>(b), _mm256_slli_epi32::<24>(na)),
            );
            // sa == 0 → dst untouched (the scalar branch)
            let keep = _mm256_cmpeq_epi32(sa, zero);
            let res = _mm256_blendv_epi8(out, d, keep);
            _mm256_storeu_si256(pd.add(off).cast(), res);
            p += 8;
        }
    }
}
