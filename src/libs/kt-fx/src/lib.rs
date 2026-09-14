//! Аудио-FX: slowed (rubato-варispeed) + reverb (freeverb) + gain.
//!
//! Архитектурные решения, закрывающие хрипы старого движка
//! (дефекты a–f, docs/porting-notes.md):
//! - весь звук — f32 48kHz/2ch; FX не видят sample-rate источника (a);
//! - состояние живёт в одном объекте на клип, владелец — аудио-render-поток
//!   через FxStateStore в kt-engine, без шаринга с prefetch (b);
//! - варispeed — rubato SincFixedOut с непрерывным потреблением источника:
//!   дробный курсор, ни один сэмпл не пропускается и не дублируется (a, c, d);
//! - параметры сглаживаются one-pole (~20 мс) и НИКОГДА не сбрасывают
//!   буферы; полный сброс — только reset() при разрыве непрерывности (e);
//! - tail() сообщает хвост реверба — граф продлевает окно клипа (f).

use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};

pub const CH: usize = AUDIO_CHANNELS;

/// Пре-ролл при разрыве непрерывности: max(tail, 100 мс) рендерится в мусор,
/// чтобы состояние (реверб, ресемплер) было прогрето детерминированно.
pub const MIN_PREROLL_FRAMES: i64 = (AUDIO_SAMPLE_RATE as i64) / 10;

/// Роль обращения к источнику. `Warmup` — пре-ролл после seek: эти кадры
/// рендерятся в мусор и на выход не идут, поэтому их промах кэша НЕ
/// underrun. Раньше он им был, и счётчик рос на каждом старте
/// воспроизведения и на каждом завороте лупа (п.166).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullKind {
    Output,
    Warmup,
}

pub mod geom;
pub mod plan;
pub mod simd;
pub mod video;
mod video_simd;

/// Явный доступ к ОБОИМ путям ядер для parity-тестов (Р.32, t198).
/// Тесты не трогают env (KT_SIMD читается раз на процесс) — они зовут
/// скаляр и AVX2 напрямую и сравнивают байты.
#[doc(hidden)]
pub mod simd_testing {
    use crate::video::Frame;

    /// Скалярный эталон билинейки.
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

    /// AVX2-ядро билинейки. Паникует, если фичи нет — вызывающий тест
    /// обязан сам проверить наличие (skip/KT_REQUIRE_SIMD).
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
        // SAFETY: фича проверена строкой выше, длины буферов — на
        // ответственности теста (он их и создаёт)
        unsafe { crate::video_simd::bilinear_into_avx2(src, x0, y0, cw, ch, ow, oh, out) };
    }

    /// AVX2-ядро noise (in-place по всему кадру, хвост столбцов не
    /// трогается — тест дорисовывает его скаляром).
    #[cfg(target_arch = "x86_64")]
    pub fn noise_avx2(rgba: &mut [u8], w: u32, h: u32, seed: u32, k: i64) {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: фича проверена; длины — на ответственности теста
        unsafe { crate::video_simd::noise_avx2(rgba, w, h, seed, k) };
    }

    /// AVX2-ядро аддитива glow.
    #[cfg(target_arch = "x86_64")]
    pub fn glow_add_avx2(dst: &mut [u8], up: &[u8], s: f64, pixels: usize) {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: фича проверена
        unsafe { crate::video_simd::glow_add_avx2(dst, up, s, pixels) };
    }

    /// Скалярные эталоны noise/glow (копии тел из video.rs).
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

    /// Скалярный эталон edges целиком (копия тела из video.rs) — t198
    /// сверяет с ним результат ПУБЛИЧНОЙ цепочки, идущей SIMD-путём.
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

    /// Скалярный эталон vhs целиком (копия тела из video.rs).
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

    /// Округление half-away-from-zero: 4 значения через AVX2-эмуляцию
    /// (микро-доказательство против `f64::round`).
    #[cfg(target_arch = "x86_64")]
    pub fn round_half_away_avx2(v: [f64; 4]) -> [f64; 4] {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: фича проверена
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

pub use audio::{ChainOpts, ClipProcessor, Freeverb, SmoothedParam, Warp, eq_mag_db};
pub use audio::{
    WARP_SOURCE_MARGIN_FRAMES, chain_fingerprint, chain_lookahead, chain_source_used,
    chain_source_window, chain_source_window_ext, chain_tail,
};

/// Карта растяжения времени ПЕРЕЕХАЛА В kt-model (Р.39): её обязан звать
/// `clip_slice` — правило куска клипа, — а kt-model зависеть от kt-fx не
/// может. Здесь только реэкспорт: у формулы по-прежнему ОДИН дом, а движок
/// и тесты продолжают звать `kt_fx::…`, и ни один вызов не тронут.
pub use kt_model::{
    AutoProfiles, ParamPoints, RATE_CLAMP, avg_rate, chain_avg_rate, chain_rate, eff_rate,
    output_time_for_source, param_at, param_integral, rate_at, source_progress,
    warp_material_ticks,
};
