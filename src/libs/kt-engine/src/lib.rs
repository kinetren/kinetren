//! Engine: render graph (event selection, mix, composite), MediaWorker scheduler
//! for decode, transport (the clock = the audio callback), prefetch, the Player core.
//!
//! Implemented: M2 (audio graph + transport + fetchers), M3 (workers/prefetch),
//! M6-audio (FX in the graph), M4-core (Player + video composite).
//! Ahead: M7 (export). Architecture and thread ownership — docs/architecture.md.

pub mod audio;
pub mod export;
pub mod fetch;
pub mod flight;
pub mod fxstore;
pub mod player;
pub mod prefetch;
pub mod transitions;
pub mod transport;
pub mod video;
pub mod video_plan;
mod video_simd;

/// Explicit access to both composite kernel paths for parity tests
/// (R.32, t198): the tests call the scalar and AVX2 directly and compare bytes.
#[doc(hidden)]
#[cfg(target_arch = "x86_64")]
pub mod simd_testing {
    fn need_avx2() {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
    }

    /// `x / 255` with eight lanes — a micro-proof of the domain.
    pub fn div255_avx2(v: [u32; 8]) -> [u32; 8] {
        need_avx2();
        // SAFETY: feature checked
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
        // SAFETY: feature checked; lengths are the responsibility of the test
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
        // SAFETY: feature checked
        unsafe { crate::video_simd::dissolve_avx2(dst, a, b, a_has_alpha, b_has_alpha, k, pixels) };
    }

    pub fn scale_alpha_avx2(dst: &mut [u8], src: &[u8], has_alpha: bool, k: u32, pixels: usize) {
        need_avx2();
        // SAFETY: feature checked
        unsafe { crate::video_simd::scale_alpha_avx2(dst, src, has_alpha, k, pixels) };
    }

    // --- scalar references (copies of the bodies from video.rs; a divergence from them
    // is exactly the regression that t198 catches) ---

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

    pub fn scale_alpha_scalar(dst: &mut [u8], src: &[u8], has_alpha: bool, k: u32, pixels: usize) {
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
    export_mp4_with_backends, render_raw_audio, render_raw_video,
};
pub use fetch::{AudioFetcher, CacheFetcher, DirectFetcher, source_id};
pub use flight::{FlightRecorder, Snap as FlightSnap};
pub use fxstore::FxStateStore;
pub use player::{
    AudioShared, Player, VideoBackendStatus, callback_step, lookahead_windows, reverse_frames,
    varispeed_fill,
};
pub use prefetch::prefetch_audio_pass;
pub use transitions::TransMix;
pub use transport::Transport;
pub use video::{
    NestKey, SourceFacts, VideoRenderer, clip_source_addr, clip_source_addr_in, compose_plan_cpu,
    content_hash, gen_salt, gen_source_addr, gen_source_addr_in, gen_sources, instance_content,
    nest_memo_key, nest_source_id, reel_chain_of_clip, resolve_cam3d, video_cache_idx,
};
pub use video_plan::{
    Cam3d, FramePlan, GpuBackendInfo, GpuExecError, LayerSrc, PlanLayer, VideoGpuExec,
};
pub use worker::{DecodeRequest, MediaWorker, RequestKind, Retune};

/// Priorities of requests to the MediaWorker. The order matters (invariant 5).
/// Time stretching of the clip for FOREIGN consumers (R.37 item 3).
///
/// The timeline preview — waveform and filmstrip — must compute the position of
/// the material with THE SAME map as the render: otherwise a slowed clip draws
/// not what it plays. The formula lives here, not in kt-app, by the same rule
/// as the rest of the geometry: do not create a formula in the UI.
///
/// This cannot be built for every pixel column — the caller must
/// cache it (in kt-app this is `warp_maps()` by `model_rev`).
/// The map moved to kt-model (R.39): the constructor is ONE per render, preview
/// and the clip's chunk rule. Here is a re-export — the calls in kt-app are untouched.
pub use kt_model::{ClipWarp, clip_warp as clip_warp_map};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Playback = 0,
    NearPrefetch = 1,
    FarPrefetch = 2,
    Thumbnail = 3,
}
