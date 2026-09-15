//! AVX2-ядра композита (Р.32). Весь unsafe крейта живёт здесь.
//!
//! ЗАКОН тот же, что в kt-fx/src/video_simd.rs: байт-в-байт со скаляром
//! из video.rs. Композит целочисленный, поэтому «бит-в-бит» здесь —
//! вопрос точного повторения делений и ВЕТВЛЕНИЙ (у alpha_over ветка
//! sa == 0 оставляет dst нетронутым — branchless-формула его бы
//! испортила; см. select ниже).
//!
//! Каждая функция вызывается ТОЛЬКО после `simd_level() == Avx2`.

#![cfg(target_arch = "x86_64")]

use std::arch::x86_64::*;

/// Целочисленное `x / 255` для 8 лейнов u32 — magic-умножение
/// `(x * 0x8080_8081) >> 39` в 64-битной ширине, точное для ВСЕГО u32
/// (та же константа, что генерирует компилятор для `x / 255`).
///
/// Ходовая «быстрая» формула `t = x+1; (t + (t>>8)) >> 8` здесь НЕ
/// годится: она врёт уже на 255·255·255 (даёт 65024 вместо 65025) —
/// поймано исчерпывающим t198_div255_exhaustive.
///
/// # Safety
/// Требует AVX2.
#[target_feature(enable = "avx2")]
#[inline]
pub(crate) unsafe fn div255(x: __m256i) -> __m256i {
    let magic = _mm256_set1_epi64x(0x8080_8081u32 as i64);
    // mul_epu32 умножает ЧЁТНЫЕ 32-битные лейны, давая 4×u64
    let even = _mm256_srli_epi64::<39>(_mm256_mul_epu32(x, magic));
    let odd = _mm256_srli_epi64::<39>(_mm256_mul_epu32(_mm256_srli_epi64::<32>(x), magic));
    _mm256_blend_epi32::<0b1010_1010>(even, _mm256_slli_epi64::<32>(odd))
}

/// AVX2-близнец `dissolve_into`: `(av*ik + bv*k + 2048) >> 12` по всем
/// четырём каналам, 4 пикселя за итерацию. `av*ik + bv*k` считается
/// одним `madd_epi16` — это ровно та же целочисленная сумма.
/// Подстановка A = 255 при `!has_alpha` делается `or` с маской альфы
/// (255|x == 255 в альфа-байте) — алгебраически то же, что скалярное
/// `if ch == 3 && !has_alpha { 255 }`.
///
/// # Safety
/// Требует AVX2; `a`/`b`/`dst` длиной >= `pixels * 4`.
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
        // пары [ik, k] как i16 внутри i32-лейна: madd их и сложит
        let coef = _mm256_set1_epi32(((k as i32) << 16) | (ik as i32));
        let round = _mm256_set1_epi32(2048);
        let alpha_fill = _mm_set1_epi32(0xFF00_0000u32 as i32);
        let (pa, pb, pd) = (a.as_ptr(), b.as_ptr(), dst.as_mut_ptr());
        let vec_pixels = pixels & !3; // хвост — скаляром у вызывающего
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
            // чередуем байты a и b, расширяем до u16 и сворачиваем madd'ом
            let lo = _mm256_madd_epi16(_mm256_cvtepu8_epi16(_mm_unpacklo_epi8(va, vb)), coef);
            let hi = _mm256_madd_epi16(_mm256_cvtepu8_epi16(_mm_unpackhi_epi8(va, vb)), coef);
            let lo = _mm256_srli_epi32::<12>(_mm256_add_epi32(lo, round));
            let hi = _mm256_srli_epi32::<12>(_mm256_add_epi32(hi, round));
            // упаковка через 128-битные половины — порядок элементов
            // сохраняется (packus 256-бит перемешал бы полосы)
            let lo16 =
                _mm_packus_epi32(_mm256_castsi256_si128(lo), _mm256_extracti128_si256::<1>(lo));
            let hi16 =
                _mm_packus_epi32(_mm256_castsi256_si128(hi), _mm256_extracti128_si256::<1>(hi));
            _mm_storeu_si128(pd.add(off).cast(), _mm_packus_epi16(lo16, hi16));
            p += 4;
        }
    }
}

/// AVX2-близнец `scale_alpha_into`: RGB копируются, альфа умножается на
/// k/4096 с округлением `(a*k + 2048) >> 12`; кадр без альфы считается
/// непрозрачным (A = 255). 8 пикселей за итерацию.
///
/// # Safety
/// Требует AVX2; `src`/`dst` длиной >= `pixels * 4`.
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

/// AVX2-близнец `alpha_over` (straight alpha, целочисленный):
/// `dst = (s*sa + d*inv*da/255)/255`, альфа `sa + da*inv/255`, обе с
/// потолком 255. 8 пикселей за итерацию.
///
/// Ветвления скаляра воспроизведены так:
/// - `sa == 0` — ОБЯЗАТЕЛЬНЫЙ select: скаляр оставляет dst нетронутым,
///   а формула дала бы `d*da/255 != d`;
/// - `sa == 255` — отдельный select НЕ нужен: при `inv == 0` формула
///   схлопывается в копию точно (`(s*255+0)/255 == s`, альфа `255+0`),
///   что прибито перебором (sa, da) в t198.
///
/// # Safety
/// Требует AVX2; `src`/`dst` длиной >= `pixels * 4`.
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
            // цвет: (s*sa + (d*inv*da)/255) / 255, потолок 255.
            // Каналы развёрнуты явно: сдвиг AVX2 требует константу, а
            // замыкание не унаследовало бы target_feature (не инлайнится).
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
            // sa == 0 → dst нетронут (ветка скаляра)
            let keep = _mm256_cmpeq_epi32(sa, zero);
            let res = _mm256_blendv_epi8(out, d, keep);
            _mm256_storeu_si256(pd.add(off).cast(), res);
            p += 8;
        }
    }
}
