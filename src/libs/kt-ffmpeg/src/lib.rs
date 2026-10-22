//! kt-ffmpeg: runtime layer over the system FFmpeg (majors 6/7/8).
//!
//! INVARIANT: the binary does NOT link to libav* (`ldd kt-app | grep libav` is empty).
//! Libraries are loaded via dlopen on first use, the version is determined by
//! `avcodec_version()`, and the struct types are taken from the
//! corresponding version module (bindgen over the vendored headers
//! `vendor/ffmpeg-N/include`). Thanks to this, one and the same executable
//! works on a system with any supported FFmpeg and uses
//! all of its hardware acceleration.

pub mod api;
pub mod loader;

/// Cumulative activity counters of the FFmpeg layer (performance panel; port of the
/// avcpp metrics of v1: gAvcppOpenCalls/gAvcppPacketReads). Global per process, monotonic;
/// the UI differentiates them over time.
pub mod iostats {
    use std::sync::atomic::{AtomicU64, Ordering};
    /// Opens of demux contexts and decoders (including re-creations on seek).
    pub static OPENS: AtomicU64 = AtomicU64::new(0);
    /// Read packets (av_read_frame over all paths, including the scan).
    pub static PACKET_READS: AtomicU64 = AtomicU64::new(0);
    /// (opens, packet_reads)
    pub fn snapshot() -> (u64, u64) {
        (OPENS.load(Ordering::Relaxed), PACKET_READS.load(Ordering::Relaxed))
    }
}

use kt_time::Tick;

#[derive(Debug, thiserror::Error)]
pub enum FfError {
    #[error(
        "FFmpeg not found on this system. Install libavcodec/libavformat (FFmpeg 6, 7 or 8).\n{0}"
    )]
    NotFound(String),
    #[error("loading FFmpeg: {0}")]
    Load(String),
    #[error("{0}")]
    Av(String),
    #[error("no such stream or position")]
    OutOfRange,
}

/// What was successfully loaded (for the performance panel and diagnostics).
#[derive(Debug, Clone)]
pub struct BackendInfo {
    pub major: u32,
    pub avcodec_version: (u32, u32, u32),
}

/// Media file info (duplicates kt_media::MediaInfo, but kt-ffmpeg
/// does not depend on kt-media — the bridge is done by kt-media).
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub duration: Tick,
    pub has_video: bool,
    pub has_audio: bool,
    pub fps: Option<(u32, u32)>,
    /// Source frame size (0,0 if there is no video).
    pub width: u32,
    pub height: u32,
    /// Pixel aspect ratio of the stream (num/den), 1:1 if not set.
    pub sar: (u32, u32),
    /// SINGLE IMAGE (R.67): an image demuxer, video without sound and without its own duration.
    /// It has neither a frame grid nor an end of material —
    /// one frame is returned for any requested tick, like a stub.
    pub still: bool,
    /// The stream’s pixel format HAS ALPHA (R.67): PNG with transparency,
    /// ProRes 4444, WebM VP9 with alpha. The composite can blend it —
    /// before, it simply did not know about it (`has_alpha: false` was hard-coded).
    pub has_alpha: bool,
}

/// Export configuration.
#[derive(Debug, Clone)]
pub struct MuxConfig {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// CRF/CQ/QP — the meaning depends on the encoder (23 ≈ visually lossless).
    /// Ignored if video_bitrate is set.
    pub quality: u32,
    /// Target video bitrate, bit/s (None = CRF/CQ quality mode).
    pub video_bitrate: Option<u32>,
    pub audio_bitrate: u32,
    /// Forced encoder (otherwise automatic choice with hw priority).
    pub force_encoder: Option<String>,
}

/// Chosen encoder (for display in the UI).
#[derive(Debug, Clone)]
pub struct EncoderChoice {
    pub name: String,
    pub hardware: bool,
}

/// Ready packet index (from the kt-media sidecar DB): pts of audio packets and
/// pts of video keyframes in the timebase of the corresponding streams, sorted.
/// Valid only for a byte-for-byte identical file (identity is checked by the
/// caller via xxhash/size) — the pts are bound to the container.
#[derive(Debug, Clone, Default)]
pub struct PrebuiltIndex {
    pub audio_pts: Vec<i64>,
    pub video_keyframes: Vec<i64>,
}

/// Frame in RGBA.
pub struct RgbaFrame {
    pub pts: Tick,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decoder implementation shared by all versions (included into each
/// version module with its own types).
macro_rules! version_module {
    ($name:ident, $types:literal, $major:literal) => {
        pub mod $name {
            #![allow(
                non_upper_case_globals,
                non_camel_case_types,
                non_snake_case,
                dead_code,
                unsafe_op_in_unsafe_fn,
                clippy::all
            )]
            pub mod ffi {
                #![allow(warnings)]
                include!(concat!(env!("OUT_DIR"), "/", $types));
            }
            crate::ffmpeg_api!();
            include!("source_impl.rs");
            include!("muxer_impl.rs");
        }
    };
}

version_module!(v6, "types_v6.rs", 6);
version_module!(v7, "types_v7.rs", 7);
version_module!(v8, "types_v8.rs", 8);

/// Single entry point: loads the system FFmpeg (once per process)
/// and opens the file with the implementation of the matching version.
pub enum Backend {
    V6(&'static v6::Api),
    V7(&'static v7::Api),
    V8(&'static v8::Api),
}

static BACKEND: std::sync::OnceLock<Result<(Backend, BackendInfo), String>> =
    std::sync::OnceLock::new();

fn init() -> &'static Result<(Backend, BackendInfo), String> {
    BACKEND.get_or_init(|| {
        let libs = loader::load().map_err(|e| e.to_string())?;
        let info = BackendInfo { major: libs.major, avcodec_version: libs.avcodec_version };
        // Libraries live until the end of the process — the leak is intentional (otherwise
        // the function pointers would dangle).
        let libs: &'static loader::Libs = Box::leak(Box::new(libs));
        let backend = unsafe {
            match libs.major {
                8 => Backend::V8(Box::leak(Box::new(
                    v8::Api::load(libs).map_err(|e| e.to_string())?,
                ))),
                7 => Backend::V7(Box::leak(Box::new(
                    v7::Api::load(libs).map_err(|e| e.to_string())?,
                ))),
                _ => Backend::V6(Box::leak(Box::new(
                    v6::Api::load(libs).map_err(|e| e.to_string())?,
                ))),
            }
        };
        Ok((backend, info))
    })
}

/// Version of the loaded FFmpeg (None if not found).
pub fn backend_info() -> Option<BackendInfo> {
    init().as_ref().ok().map(|(_, i)| i.clone())
}

pub fn backend_error() -> Option<String> {
    init().as_ref().err().cloned()
}

/// List of video encoders available in the system (for the UI).
pub fn available_encoders() -> Vec<String> {
    match init().as_ref() {
        Ok((Backend::V6(api), _)) => v6::available_encoders(api),
        Ok((Backend::V7(api), _)) => v7::available_encoders(api),
        Ok((Backend::V8(api), _)) => v8::available_encoders(api),
        Err(_) => Vec::new(),
    }
}

/// MP4 muxer — dynamic dispatch by version.
pub enum Muxer {
    V6(v6::Muxer),
    V7(v7::Muxer),
    V8(v8::Muxer),
}

impl Muxer {
    pub fn open(cfg: &MuxConfig) -> Result<Muxer, FfError> {
        let (backend, _) = init().as_ref().map_err(|e| FfError::NotFound(e.clone()))?;
        match backend {
            Backend::V6(api) => v6::Muxer::open(api, cfg).map(Muxer::V6),
            Backend::V7(api) => v7::Muxer::open(api, cfg).map(Muxer::V7),
            Backend::V8(api) => v8::Muxer::open(api, cfg).map(Muxer::V8),
        }
    }

    pub fn encoder_name(&self) -> &str {
        match self {
            Muxer::V6(m) => m.encoder_name(),
            Muxer::V7(m) => m.encoder_name(),
            Muxer::V8(m) => m.encoder_name(),
        }
    }

    /// One RGBA frame of size width×height from the configuration.
    pub fn write_video(&mut self, rgba: &[u8]) -> Result<(), FfError> {
        match self {
            Muxer::V6(m) => m.write_video(rgba),
            Muxer::V7(m) => m.write_video(rgba),
            Muxer::V8(m) => m.write_video(rgba),
        }
    }

    /// Audio 48 kHz stereo f32 interleaved (any length).
    pub fn write_audio(&mut self, samples: &[f32]) -> Result<(), FfError> {
        match self {
            Muxer::V6(m) => m.write_audio(samples),
            Muxer::V7(m) => m.write_audio(samples),
            Muxer::V8(m) => m.write_audio(samples),
        }
    }

    pub fn finish(self) -> Result<(), FfError> {
        match self {
            Muxer::V6(m) => m.finish(),
            Muxer::V7(m) => m.finish(),
            Muxer::V8(m) => m.finish(),
        }
    }
}

/// Open source — dynamic dispatch by version.
pub enum Source {
    V6(v6::Source),
    V7(v7::Source),
    V8(v8::Source),
}

impl Source {
    pub fn open(path: &str) -> Result<Source, FfError> {
        Self::open_with_index(path, None)
    }

    /// Opening with a ready packet index: the full scan pass over the file is skipped
    /// (only the header read remains) — fast opening of
    /// large files. The index must match the current contents of the
    /// file (identity is checked by the caller).
    pub fn open_with_index(path: &str, prebuilt: Option<PrebuiltIndex>) -> Result<Source, FfError> {
        let (backend, _) = init().as_ref().map_err(|e| FfError::NotFound(e.clone()))?;
        match backend {
            Backend::V6(api) => v6::Source::open_with_index(api, path, prebuilt).map(Source::V6),
            Backend::V7(api) => v7::Source::open_with_index(api, path, prebuilt).map(Source::V7),
            Backend::V8(api) => v8::Source::open_with_index(api, path, prebuilt).map(Source::V8),
        }
    }

    /// (audio packet pts, video keyframe pts) — for persisting in the sidecar DB.
    pub fn packet_index(&self) -> (&[i64], &[i64]) {
        match self {
            Source::V6(s) => s.packet_index(),
            Source::V7(s) => s.packet_index(),
            Source::V8(s) => s.packet_index(),
        }
    }

    /// Index came ready (no scan was performed) — diagnostics/test 34.
    pub fn index_was_prebuilt(&self) -> bool {
        match self {
            Source::V6(s) => s.index_was_prebuilt(),
            Source::V7(s) => s.index_was_prebuilt(),
            Source::V8(s) => s.index_was_prebuilt(),
        }
    }

    pub fn probe(&self) -> &Probe {
        match self {
            Source::V6(s) => s.probe(),
            Source::V7(s) => s.probe(),
            Source::V8(s) => s.probe(),
        }
    }

    /// Audio burst 48 kHz stereo f32 interleaved, exactly `out.len()` values.
    pub fn read_audio_burst(&mut self, idx: i64, out: &mut [f32]) -> Result<(), FfError> {
        match self {
            Source::V6(s) => s.read_audio_burst(idx, out),
            Source::V7(s) => s.read_audio_burst(idx, out),
            Source::V8(s) => s.read_audio_burst(idx, out),
        }
    }

    pub fn read_video_frame(
        &mut self,
        pts: Tick,
        width: u32,
        height: u32,
    ) -> Result<RgbaFrame, FfError> {
        match self {
            Source::V6(s) => s.read_video_frame(pts, width, height),
            Source::V7(s) => s.read_video_frame(pts, width, height),
            Source::V8(s) => s.read_video_frame(pts, width, height),
        }
    }

    /// Span decode: see source_impl::read_video_span.
    pub fn read_video_span(
        &mut self,
        keys: &[Tick],
        width: u32,
        height: u32,
        emit: &mut dyn FnMut(Tick, RgbaFrame),
    ) -> Result<(), FfError> {
        match self {
            Source::V6(s) => s.read_video_span(keys, width, height, emit),
            Source::V7(s) => s.read_video_span(keys, width, height, emit),
            Source::V8(s) => s.read_video_span(keys, width, height, emit),
        }
    }

    /// (backward_reopens, frames_decoded) — performance diagnostics.
    pub fn video_debug_stats(&self) -> (u64, u64) {
        match self {
            Source::V6(s) => s.video_debug_stats(),
            Source::V7(s) => s.video_debug_stats(),
            Source::V8(s) => s.video_debug_stats(),
        }
    }

    pub fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        match self {
            Source::V6(s) => s.keyframe_before(pts),
            Source::V7(s) => s.keyframe_before(pts),
            Source::V8(s) => s.keyframe_before(pts),
        }
    }
}
