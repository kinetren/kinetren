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
pub use transport::Transport;
pub use video::{VideoRenderer, compose_plan_cpu, video_cache_idx};
pub use video_plan::{
    Cam3d, FramePlan, GpuBackendInfo, GpuExecError, LayerSrc, PlanLayer, VideoGpuExec,
};
pub use worker::{DecodeRequest, MediaWorker, RequestKind};

/// Приоритеты запросов к MediaWorker. Порядок значим (инвариант №5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Playback = 0,
    NearPrefetch = 1,
    FarPrefetch = 2,
    Thumbnail = 3,
}

/// Момент таймлайна → позиция в МАТЕРИАЛЕ события и обратно (Р.33).
///
/// Единая правда для UI: в этих координатах живут ключи автоматизации, и
/// именно этот перевод учитывает и реверс, и скорость (у slowed&reverb
/// время идёт неравномерно — карта варпа кусочно-аналитическая). Рендер
/// пользуется теми же функциями через `kt_model::clip_material_at`,
/// поэтому «где ключ» и «что звучит» не могут разойтись.
pub fn material_at(
    clip: &kt_model::ClipEvent,
    frame_step: i64,
    t: kt_time::Tick,
) -> kt_time::Tick {
    if let Some((base, prof)) = clip_warp(clip) {
        let len_f = clip.duration.to_audio_frames().max(1);
        let tn = (t.0 - clip.start.0) as f64 / clip.duration.0.max(1) as f64;
        let s = kt_fx::source_progress(base, &prof, tn);
        return kt_time::Tick::from_audio_frames(
            clip.source_in.to_audio_frames() + (s * len_f as f64).round() as i64,
        );
    }
    kt_model::clip_material_at(
        clip.source_in,
        clip.start,
        clip.duration,
        clip.reversed,
        frame_step,
        t,
    )
}

/// Обратный перевод: позиция в материале → момент таймлайна.
pub fn timeline_at_material(
    clip: &kt_model::ClipEvent,
    frame_step: i64,
    mat: kt_time::Tick,
) -> kt_time::Tick {
    if let Some((base, prof)) = clip_warp(clip) {
        let len_f = clip.duration.to_audio_frames().max(1);
        let s = (mat.to_audio_frames() - clip.source_in.to_audio_frames()) as f64 / len_f as f64;
        let tn = kt_fx::output_time_for_source(base, &prof, s);
        return kt_time::Tick(clip.start.0 + (tn * clip.duration.0 as f64).round() as i64);
    }
    if clip.reversed {
        kt_time::Tick(clip.start.0 + (clip.source_in.0 + clip.duration.0 - frame_step - mat.0))
    } else {
        kt_time::Tick(clip.start.0 + (mat.0 - clip.source_in.0))
    }
}

/// Базовая скорость и профиль её кривой (None — варпа в цепочке нет).
fn clip_warp(clip: &kt_model::ClipEvent) -> Option<(f64, Vec<(f64, f64)>)> {
    let fx = clip.active_fx();
    let base = fx.iter().find_map(|f| match f {
        kt_model::FxInstance::SlowedReverb { rate, .. } => Some(*rate),
        _ => None,
    })?;
    let prof = kt_fx::AutoProfiles::from_curves(
        &clip.active_fx_auto(),
        clip.source_in.to_audio_frames(),
        clip.duration.to_audio_frames().max(1),
    );
    Some((base, prof.rate_points(&fx).to_vec()))
}
