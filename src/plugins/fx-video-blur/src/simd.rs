//! The AVX2 twin of glow's additive finish (moved from kt-fx `video_simd.rs`, plugin-api-barriers).
//! It is BYTE-FOR-BYTE the scalar loop (R.32): the lanes are a pixel's four channels, the order of
//! operations inside a pixel is the scalar one, no FMA. The host decides when it may run
//! (`BarrierCtx::simd`); t198 compares it with the scalar loop through `simd_testing`.

#![cfg(target_arch = "x86_64")]

use std::arch::x86_64::*;

#[target_feature(enable = "avx2")]
#[inline]
unsafe fn pack_rgba(v: __m256d) -> u32 {
    // values are already integers — the cvtt truncation is equivalent to any mode
    let i32x4 = _mm256_cvttpd_epi32(v);
    let u16x8 = _mm_packus_epi32(i32x4, i32x4);
    let u8x16 = _mm_packus_epi16(u16x8, u16x8);
    _mm_cvtsi128_si32(u8x16) as u32
}

/// AVX2 twin of the additive finish of glow: `o + u*s` with a ceiling of 255 and TRUNCATION (the
/// scalar does `as u8` without round). The lanes = 4 channels of a pixel; the alpha byte is taken
/// back from the original (the scalar does not touch it).
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
            pd.add(off).cast::<u32>().write_unaligned((packed & 0x00FF_FFFF) | (orig & 0xFF00_0000));
        }
    }
}
