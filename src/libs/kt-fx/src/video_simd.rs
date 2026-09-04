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

/// AVX2-близнец `noise`: counter-based hash по (x, y, seed) и смещение
/// RGB с клампом. Всё целочисленное — байт-в-байт со скаляром на любом
/// CPU. 8 пикселей за итерацию; хвост строки дорисовывает вызывающий.
///
/// Деление `n*k/32767` (усечение К НУЛЮ, n бывает отрицательным)
/// считается как `sign(v) * ((|v| + (|v|>>15) + 1) >> 15)` — тождество
/// проверено перебором всего домена (n ∈ ±16384, k ≤ 32) в t198.
///
/// # Safety
/// Требует AVX2; `rgba` длиной >= `w*h*4`.
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
                // h = noise_hash(x ^ rowh) — lowbias32 в векторе
                let mut hv = _mm256_xor_si256(
                    _mm256_add_epi32(_mm256_set1_epi32(x as i32), lane_x),
                    rowh_v,
                );
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
                // sign_epi32(q, v) = q * sign(v), и 0 при v == 0
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
                        // альфа не трогается (скаляр правит только RGB)
                        _mm256_and_si256(px, _mm256_set1_epi32(0xFF00_0000u32 as i32)),
                    ),
                );
                _mm256_storeu_si256(p.add(row + x * 4).cast(), out);
                x += 8;
            }
        }
    }
}

/// AVX2-близнец аддитивного финала `glow`: `o + u*s` с потолком 255 и
/// УСЕЧЕНИЕМ (скаляр делает `as u8` без round). Лейны = 4 канала
/// пикселя; альфа-байт возвращается из оригинала (скаляр её не трогает).
///
/// # Safety
/// Требует AVX2; `dst`/`up` длиной >= `pixels*4`.
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
            // порядок скаляра: сначала u*s, потом o + ...
            let r = _mm256_min_pd(_mm256_add_pd(o, _mm256_mul_pd(u, sv)), max255);
            let packed = pack_rgba(r);
            // альфа — из оригинала
            pd.add(off)
                .cast::<u32>()
                .write_unaligned((packed & 0x00FF_FFFF) | (orig & 0xFF00_0000));
        }
    }
}

/// Люма кадра для `edges`: `(77R + 150G + 29B) >> 8`, 8 пикселей за
/// итерацию. Целочисленно — байт-в-байт со скаляром.
///
/// # Safety
/// Требует AVX2; `luma` длиной >= pixels, `rgba` >= pixels*4.
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
            let g = _mm256_mullo_epi32(
                _mm256_and_si256(_mm256_srli_epi32::<8>(v), byte),
                c150,
            );
            let b = _mm256_mullo_epi32(
                _mm256_and_si256(_mm256_srli_epi32::<16>(v), byte),
                c29,
            );
            let l = _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_add_epi32(r, g), b));
            _mm256_storeu_si256(pl.add(p).cast(), l);
            p += 8;
        }
        for p in n..pixels {
            let i = p * 4;
            let px = ps.add(i);
            *pl.add(p) = (77 * (*px) as i32 + 150 * (*px.add(1)) as i32
                + 29 * (*px.add(2)) as i32)
                >> 8;
        }
    }
}

/// Собель одной строки `edges` для x ∈ [1, w-1): 8 пикселей за итерацию.
/// Порог и тираж в RGB — как у скаляра (альфу выставляет вызывающий).
///
/// # Safety
/// Требует AVX2; `luma` содержит >= (y+2)*w значений, `out` >= (y*w+w)*4.
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
        // грузим строки y-1, y, y+1 со смещениями -1, 0, +1
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
            let sq = _mm256_add_epi32(
                _mm256_mullo_epi32(gx, gx),
                _mm256_mullo_epi32(gy, gy),
            );
            // sqrt в f64 двумя половинами, усечение как `as i32`
            let lo = _mm256_cvttpd_epi32(_mm256_min_pd(
                _mm256_sqrt_pd(_mm256_cvtepi32_pd(_mm256_castsi256_si128(sq))),
                c255d,
            ));
            let hi = _mm256_cvttpd_epi32(_mm256_min_pd(
                _mm256_sqrt_pd(_mm256_cvtepi32_pd(_mm256_extracti128_si256::<1>(sq))),
                c255d,
            ));
            let mag = _mm256_min_epi32(
                _mm256_set_m128i(hi, lo),
                c255i,
            );
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

/// Лерп 8 целых лейнов в f64 с округлением half-away: возвращает
/// `round(a*(1-bleed) + b*bleed)` — порядок операций скалярный.
///
/// # Safety
/// Требует AVX2.
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
        let hi = mix(
            _mm256_extracti128_si256::<1>(a),
            _mm256_extracti128_si256::<1>(b),
        );
        _mm256_set_m128i(hi, lo)
    }
}

/// AVX2-близнец `vhs` для ИНТЕРЬЕРА строки (где клампы xr/xb неактивны):
/// RGB-сдвиг с лерпом и скан-линии. Возвращает x, на котором остановился
/// — хвосты и кромки дорисовывает скалярный цикл вызывающего.
///
/// # Safety
/// Требует AVX2; `row` — копия строки длиной w*4, `dst_row` — та же
/// строка кадра.
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
            return x_from; // интерьера нет — всё скаляром
        }
        let byte = _mm256_set1_epi32(0xFF);
        let inv = _mm256_set1_pd(1.0 - bleed);
        let bl = _mm256_set1_pd(bleed);
        let g_sub = _mm256_set1_epi32(18);
        let zero = _mm256_setzero_si256();
        let pr = row.as_ptr();
        let pd = dst_row.as_mut_ptr();
        let x_end = w - d; // при x < w-d индекс x+d <= w-1 без клампа
        let mut x = x_from.max(d);
        while x + 8 <= x_end {
            let cur = _mm256_loadu_si256(pr.add(x * 4).cast());
            let right = _mm256_loadu_si256(pr.add((x + d) * 4).cast());
            let left = _mm256_loadu_si256(pr.add((x - d) * 4).cast());
            let r = lerp8_round(
                _mm256_and_si256(cur, byte),
                _mm256_and_si256(right, byte),
                inv,
                bl,
            );
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
                    // альфа не трогается — берём из копии строки
                    _mm256_and_si256(cur, _mm256_set1_epi32(0xFF00_0000u32 as i32)),
                ),
            );
            _mm256_storeu_si256(pd.add(x * 4).cast(), out);
            x += 8;
        }
        x
    }
}
