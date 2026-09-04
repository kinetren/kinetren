//! AVX2-ядра видео-FX (Р.32). Весь unsafe крейта живёт здесь.
//!
//! ЗАКОН (kt-fx/CLAUDE.md): каждое ядро БАЙТ-В-БАЙТ равно скалярному
//! близнецу из video.rs на любом CPU — CPU-путь остаётся эталоном
//! детерминизма (инвариант №11), golden'ы t26/t11/t67 не трогаются.
//! Отсюда запреты: FMA (другое округление), переассоциация сумм,
//! rcp/rsqrt, f32 вместо f64, `_mm256_round_pd(NEAREST)` вместо
//! эмуляции half-away, смена MXCSR. Лейны — только независимые
//! пиксели/каналы; порядок операций ВНУТРИ пикселя ровно скалярный.
//!
//! Каждая функция вызывается ТОЛЬКО после `simd_level() == Avx2`.

#![cfg(target_arch = "x86_64")]

use std::arch::x86_64::*;

use crate::video::Frame;

/// Округление к ближайшему, половина ОТ нуля — как `f64::round`, только
/// для x >= 0 (наши аккумуляторы неотрицательны: веса и пиксели >= 0).
///
/// Наивное `floor(x + 0.5)` НЕВЕРНО: для x = 0.49999999999999994 сумма
/// округляется вверх до 1.0, и floor даёт 1 вместо 0. Поэтому берём
/// дробную часть: `x - floor(x)` вычисляется ТОЧНО (оба слагаемых
/// представимы, результат в [0,1)), и сравнение с 0.5 честное. Точная
/// половина (2.5 → 3) уходит вверх — это и есть half-away.
/// `_mm256_round_pd(NEAREST)` здесь непригоден: он half-to-EVEN (2.5 → 2).
///
/// # Safety
/// Требует AVX (входит в AVX2).
#[target_feature(enable = "avx2")]
#[inline]
pub(crate) unsafe fn round_half_away_nonneg(x: __m256d) -> __m256d {
    let fl = _mm256_floor_pd(x);
    let frac = _mm256_sub_pd(x, fl);
    let ge_half = _mm256_cmp_pd(frac, _mm256_set1_pd(0.5), _CMP_GE_OQ);
    _mm256_add_pd(fl, _mm256_and_pd(ge_half, _mm256_set1_pd(1.0)))
}

/// Один билинейный тап: 4 канала пикселя × вес, аккумулируется в том же
/// порядке, что скалярное `*acc += src[si + c] as f64 * w` (mul, затем
/// add — БЕЗ fma).
///
/// # Safety
/// `off + 4 <= len` буфера, на который указывает `p`; требует AVX2.
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn tap(acc: __m256d, p: *const u8, off: usize, w: f64) -> __m256d {
    unsafe {
        let raw = p.add(off).cast::<u32>().read_unaligned();
        let px = _mm256_cvtepi32_pd(_mm_cvtepu8_epi32(_mm_cvtsi32_si128(raw as i32)));
        _mm256_add_pd(acc, _mm256_mul_pd(px, _mm256_set1_pd(w)))
    }
}

/// Упаковка 4×f64 (уже целые в 0..=255) в 4 байта RGBA.
///
/// # Safety
/// Требует AVX2.
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn pack_rgba(v: __m256d) -> u32 {
    // значения уже целые — усечение cvtt эквивалентно любому режиму
    let i32x4 = _mm256_cvttpd_epi32(v);
    let u16x8 = _mm_packus_epi32(i32x4, i32x4);
    let u8x16 = _mm_packus_epi16(u16x8, u16x8);
    _mm_cvtsi128_si32(u8x16) as u32
}

/// AVX2-близнец `bilinear_into` (video.rs): лейны = 4 КАНАЛА одного
/// пикселя, четыре тапа в скалярном порядке (0,0) (0,1) (1,0) (1,1).
/// Индексы/веса по X предвычисляются той же формулой — это ровно те же
/// величины, что скаляр пересчитывает на каждой строке.
///
/// # Safety
/// Требует AVX2 (проверено вызывающим диспетчером) и корректных длин
/// буферов: `src.rgba.len() >= sw*sh*4`, `out.len() >= ow*oh*4`.
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
        // предвычисление по X: (индекс левого тапа, правого, дробная часть)
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
