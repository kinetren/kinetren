//! Движок: render graph (выбор событий, микс, композит), MediaWorker-планировщик
//! декода, transport (часы = аудио-колбэк), prefetch, Player-ядро.
//!
//! Реализовано: M2 (аудио-граф + transport + фетчеры), M3 (воркеры/prefetch),
//! M6-аудио (FX в графе), M4-ядро (Player + видео-композит).
//! Впереди: M7 (экспорт). Архитектура и владение потоками — docs/architecture.md.

pub mod flight;
pub mod audio;
pub mod export;
pub mod fetch;
pub mod fxstore;
pub mod player;
pub mod prefetch;
pub mod transitions;
pub mod transport;
pub mod video;
pub mod video_plan;
mod video_simd;

/// Явный доступ к обоим путям ядер композита для parity-тестов
/// (Р.32, t198): тесты зовут скаляр и AVX2 напрямую и сверяют байты.
#[doc(hidden)]
#[cfg(target_arch = "x86_64")]
pub mod simd_testing {
    fn need_avx2() {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
    }

    /// `x / 255` восемью лейнами — микро-доказательство домена.
    pub fn div255_avx2(v: [u32; 8]) -> [u32; 8] {
        need_avx2();
        // SAFETY: фича проверена
        unsafe {
            use std::arch::x86_64::*;
            let x = _mm256_loadu_si256(v.as_ptr().cast());
            let r = crate::video_simd::div255(x);
            let mut out = [0u32; 8];
            _mm256_storeu_si256(out.as_mut_ptr().cast(), r);
            out
        }
    }

    pub fn alpha_over_avx2(dst: &mut [u8], src: &[u8], pixels: usize) {
        need_avx2();
        // SAFETY: фича проверена; длины — на ответственности теста
        unsafe { crate::video_simd::alpha_over_avx2(dst, src, pixels) };
    }

    pub fn dissolve_avx2(
        dst: &mut [u8],
        a: &[u8],
        b: &[u8],
        a_has_alpha: bool,
        b_has_alpha: bool,
        k: u32,
        pixels: usize,
    ) {
        need_avx2();
        // SAFETY: фича проверена
        unsafe {
            crate::video_simd::dissolve_avx2(dst, a, b, a_has_alpha, b_has_alpha, k, pixels)
        };
    }

    pub fn scale_alpha_avx2(dst: &mut [u8], src: &[u8], has_alpha: bool, k: u32, pixels: usize) {
        need_avx2();
        // SAFETY: фича проверена
        unsafe { crate::video_simd::scale_alpha_avx2(dst, src, has_alpha, k, pixels) };
    }

    // --- скалярные эталоны (копии тел из video.rs; расхождение с ними
    // и есть регрессия, которую ловит t198) ---

    pub fn alpha_over_scalar(dst: &mut [u8], src: &[u8], pixels: usize) {
        for p in 0..pixels {
            let i = p * 4;
            let sa = src[i + 3] as u32;
            if sa == 255 {
                dst[i..i + 4].copy_from_slice(&src[i..i + 4]);
                continue;
            }
            if sa == 0 {
                continue;
            }
            let da = dst[i + 3] as u32;
            let inv = 255 - sa;
            for ch in 0..3 {
                let s = src[i + ch] as u32;
                let d = dst[i + ch] as u32;
                dst[i + ch] = ((s * sa + d * inv * da / 255) / 255).min(255) as u8;
            }
            dst[i + 3] = (sa + da * inv / 255).min(255) as u8;
        }
    }

    pub fn dissolve_scalar(
        dst: &mut [u8],
        a: &[u8],
        b: &[u8],
        a_has_alpha: bool,
        b_has_alpha: bool,
        k: u32,
        pixels: usize,
    ) {
        let ik = 4096 - k;
        for p in 0..pixels {
            let i = p * 4;
            for ch in 0..4 {
                let av = if ch == 3 && !a_has_alpha { 255 } else { a[i + ch] } as u32;
                let bv = if ch == 3 && !b_has_alpha { 255 } else { b[i + ch] } as u32;
                dst[i + ch] = ((av * ik + bv * k + 2048) >> 12) as u8;
            }
        }
    }

    pub fn scale_alpha_scalar(
        dst: &mut [u8],
        src: &[u8],
        has_alpha: bool,
        k: u32,
        pixels: usize,
    ) {
        for p in 0..pixels {
            let i = p * 4;
            dst[i..i + 3].copy_from_slice(&src[i..i + 3]);
            let a = if has_alpha { src[i + 3] as u32 } else { 255 };
            dst[i + 3] = ((a * k + 2048) >> 12) as u8;
        }
    }
}
pub mod worker;

pub use audio::{AudioRenderer, RenderStats, db_to_linear};
pub use export::{
    ExportBackends, ExportOptions, ExportStats, export_mp4, export_mp4_with,
    export_mp4_with_backends, gpu_required_clips, render_raw_audio, render_raw_video,
};
pub use flight::{FlightRecorder, Snap as FlightSnap};
pub use fetch::{AudioFetcher, CacheFetcher, DirectFetcher, source_id};
pub use fxstore::FxStateStore;
pub use player::{
    AudioShared, Player, VideoBackendStatus, callback_step, lookahead_windows, reverse_frames,
    varispeed_fill,
};
pub use prefetch::prefetch_audio_pass;
pub use transitions::TransMix;
pub use transport::Transport;
pub use video::{VideoRenderer, compose_plan_cpu, video_cache_idx};
pub use video_plan::{
    Cam3d, FramePlan, GpuBackendInfo, GpuExecError, LayerSrc, PlanLayer, VideoGpuExec,
};
pub use worker::{DecodeRequest, MediaWorker, RequestKind};

/// Приоритеты запросов к MediaWorker. Порядок значим (инвариант №5).
/// Растяжение времени клипа для ЧУЖИХ потребителей (Р.37 п.3).
///
/// Превью таймлайна — волноформа и филмстрип — обязано считать позицию
/// материала ТОЙ ЖЕ картой, что рендер: иначе замедленный клип рисует не
/// то, что играет. Формула живёт здесь, а не в kt-app, по тому же правилу,
/// что и остальная геометрия: своей формулы в UI не заводить.
///
/// Строить это на каждый пиксельный столбец нельзя — вызывающий обязан
/// кэшировать (в kt-app это `warp_maps()` по `model_rev`).
/// Карта переехала в kt-model (Р.39): конструктор ОДИН на рендер, превью
/// и правило куска клипа. Здесь реэкспорт — вызовы в kt-app не тронуты.
pub use kt_model::{ClipWarp, clip_warp as clip_warp_map};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Playback = 0,
    NearPrefetch = 1,
    FarPrefetch = 2,
    Thumbnail = 3,
}

