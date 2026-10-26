//! kt-ffmpeg: runtime layer over the system FFmpeg (majors 6/7/8/9).
//!
//! INVARIANT: the binary does NOT link to libav* (`ldd kt-app | grep libav` is empty).
//! Libraries are loaded via dlopen on first use, the version is determined by
//! `avcodec_version()`, and the struct types are taken from the
//! corresponding version module (bindgen over the vendored headers
//! `vendor/ffmpeg-N/include`). Thanks to this, one and the same executable
//! works on a system with any supported FFmpeg and uses
//! all of its hardware acceleration.

pub mod api;
pub mod encoders;
pub mod loader;
pub mod search;

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
    /// The text is the search report plus the hint of the OS (`search::not_found_hint`).
    #[error("FFmpeg not found.\n{0}")]
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
version_module!(v9, "types_v9.rs", 9);

/// The majors that have a version module above — t862 checks that this equals the rows of
/// `search::MAJORS` and that each row equals its module's `LIB*_VERSION_MAJOR` constants.
pub const VERSION_MODULES: [u32; 4] = [6, 7, 8, 9];

/// Single entry point: loads the system FFmpeg (once per process)
/// and opens the file with the implementation of the matching version.
pub enum Backend {
    V6(&'static v6::Api),
    V7(&'static v7::Api),
    V8(&'static v8::Api),
    V9(&'static v9::Api),
}

/// A SUCCESSFUL load lives for the whole process (the function pointers point into these
/// libraries); a failure does not (windows-build): after the person installs, downloads or
/// locates FFmpeg, `rescan()` searches again without a restart.
static LOADED: std::sync::OnceLock<(Backend, BackendInfo)> = std::sync::OnceLock::new();
static MODULES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

#[derive(Default)]
struct SearchState {
    attempted: bool,
    /// The text of the last failure (what `backend_error` and opening media report).
    failure: Option<String>,
    report: Option<search::Report>,
    user_choice: Option<String>,
    download_dir: Option<String>,
}

static STATE: std::sync::Mutex<SearchState> = std::sync::Mutex::new(SearchState {
    attempted: false,
    failure: None,
    report: None,
    user_choice: None,
    download_dir: None,
});

fn state() -> std::sync::MutexGuard<'static, SearchState> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The loaded backend, or the stored failure. Only the FIRST call searches the disk; later
/// calls after a failure return it without touching the disk (opening media must never walk
/// PATH) — `rescan()` searches again.
fn init() -> Result<&'static (Backend, BackendInfo), String> {
    if let Some(l) = LOADED.get() {
        return Ok(l);
    }
    {
        let st = state();
        if st.attempted {
            return Err(st.failure.clone().unwrap_or_else(|| "FFmpeg not loaded".into()));
        }
    }
    rescan().map(|_| LOADED.get().expect("loaded"))
}

/// The folders only the program knows: the person's choice (Locate…) and Kinetren's download
/// folder. kt-app and kt-cli set them before the first media open; they are the first and the
/// fourth step of the Windows plan.
pub fn set_search_dirs(user_choice: Option<String>, download_dir: Option<String>) {
    let mut st = state();
    st.user_choice = user_choice;
    st.download_dir = download_dir;
}

/// Search again (startup, Locate…, a finished download, the Rescan button). A remembered
/// success is returned as is.
pub fn rescan() -> Result<BackendInfo, String> {
    let env = {
        let st = state();
        loader::SystemEnv { user_choice: st.user_choice.clone(), download_dir: st.download_dir.clone() }
    };
    rescan_with(&env)
}

/// `rescan` over a given view of the machine (tests).
pub fn rescan_with(env: &dyn search::Env) -> Result<BackendInfo, String> {
    // one search at a time: two threads must not both load and leak a set
    static SEARCH: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = SEARCH.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, info)) = LOADED.get() {
        return Ok(info.clone());
    }
    let (libs, report) = loader::load(env);
    let result = libs.map_err(|e| e.to_string()).and_then(|libs| {
        let info = BackendInfo { major: libs.major, avcodec_version: libs.avcodec_version };
        // Libraries live until the end of the process — the leak is intentional (otherwise
        // the function pointers would dangle).
        let libs: &'static loader::Libs = Box::leak(Box::new(libs));
        let _ = MODULES.set(libs.module_paths());
        let backend = unsafe {
            match libs.major {
                9 => Backend::V9(Box::leak(Box::new(v9::Api::load(libs).map_err(|e| e.to_string())?))),
                8 => Backend::V8(Box::leak(Box::new(v8::Api::load(libs).map_err(|e| e.to_string())?))),
                7 => Backend::V7(Box::leak(Box::new(v7::Api::load(libs).map_err(|e| e.to_string())?))),
                6 => Backend::V6(Box::leak(Box::new(v6::Api::load(libs).map_err(|e| e.to_string())?))),
                // ffmpeg-9 (design D2): never bind one major's struct layouts to another
                // major's libraries — a table row without a module is an error, not a guess
                m => return Err(format!("FFmpeg {m}: no version module for this major")),
            }
        };
        let _ = LOADED.set((backend, info.clone()));
        Ok(info)
    });
    let mut st = state();
    st.attempted = true;
    st.report = Some(report);
    st.failure = result.as_ref().err().cloned();
    result
}

/// The last search, for people (View → FFmpeg…, `kt-cli ffmpeg`).
pub fn search_report() -> String {
    let _ = init();
    let st = state();
    match &st.report {
        Some(r) if LOADED.get().is_some() => r.to_string(),
        Some(r) => format!("{r}{}", search::not_found_hint(search::Os::NATIVE)),
        None => "FFmpeg search has not run".into(),
    }
}

/// The last search as data (the FFmpeg window draws it).
pub fn search_report_data() -> Option<search::Report> {
    let _ = init();
    state().report.clone()
}

/// Where each loaded module came from, in load order (t846: all from the chosen folder).
pub fn loaded_modules() -> Vec<String> {
    MODULES.get().cloned().unwrap_or_default()
}

/// Can this folder be used (Locate…)? The same judgement as the search applies to it.
pub fn check_folder(dir: &str) -> Result<u32, search::Reject> {
    let env = loader::SystemEnv { user_choice: None, download_dir: None };
    search::judge(search::Os::NATIVE, search::Env::list(&env, dir).as_deref())
}

/// Where the loaded FFmpeg came from (a folder, or the system SONAME), if it is loaded.
pub fn loaded_place() -> Option<String> {
    LOADED.get()?;
    state().report.as_ref()?.loaded.as_ref().map(|(place, _, _)| place.clone())
}

/// Libraries opened by this process so far (t847: a remembered success opens nothing again).
pub fn library_opens() -> u64 {
    loader::LIB_OPENS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Is FFmpeg already loaded? Unlike `backend_info`, never starts a search.
pub fn is_loaded() -> bool {
    LOADED.get().is_some()
}

/// Version of the loaded FFmpeg (None if not found).
pub fn backend_info() -> Option<BackendInfo> {
    LOADED.get().map(|(_, i)| i.clone()).or_else(|| init().ok().map(|(_, i)| i.clone()))
}

pub fn backend_error() -> Option<String> {
    init().err()
}

/// List of video encoders available in the system (for the UI).
pub fn available_encoders() -> Vec<String> {
    match init() {
        Ok((Backend::V6(api), _)) => v6::available_encoders(api),
        Ok((Backend::V7(api), _)) => v7::available_encoders(api),
        Ok((Backend::V8(api), _)) => v8::available_encoders(api),
        Ok((Backend::V9(api), _)) => v9::available_encoders(api),
        Err(_) => Vec::new(),
    }
}

/// MP4 muxer — dynamic dispatch by version.
pub enum Muxer {
    V6(v6::Muxer),
    V7(v7::Muxer),
    V8(v8::Muxer),
    V9(v9::Muxer),
}

impl Muxer {
    pub fn open(cfg: &MuxConfig) -> Result<Muxer, FfError> {
        let (backend, _) = init().map_err(FfError::NotFound)?;
        match backend {
            Backend::V6(api) => v6::Muxer::open(api, cfg).map(Muxer::V6),
            Backend::V7(api) => v7::Muxer::open(api, cfg).map(Muxer::V7),
            Backend::V8(api) => v8::Muxer::open(api, cfg).map(Muxer::V8),
            Backend::V9(api) => v9::Muxer::open(api, cfg).map(Muxer::V9),
        }
    }

    pub fn encoder_name(&self) -> &str {
        match self {
            Muxer::V6(m) => m.encoder_name(),
            Muxer::V7(m) => m.encoder_name(),
            Muxer::V8(m) => m.encoder_name(),
            Muxer::V9(m) => m.encoder_name(),
        }
    }

    /// One RGBA frame of size width×height from the configuration.
    pub fn write_video(&mut self, rgba: &[u8]) -> Result<(), FfError> {
        match self {
            Muxer::V6(m) => m.write_video(rgba),
            Muxer::V7(m) => m.write_video(rgba),
            Muxer::V8(m) => m.write_video(rgba),
            Muxer::V9(m) => m.write_video(rgba),
        }
    }

    /// Audio 48 kHz stereo f32 interleaved (any length).
    pub fn write_audio(&mut self, samples: &[f32]) -> Result<(), FfError> {
        match self {
            Muxer::V6(m) => m.write_audio(samples),
            Muxer::V7(m) => m.write_audio(samples),
            Muxer::V8(m) => m.write_audio(samples),
            Muxer::V9(m) => m.write_audio(samples),
        }
    }

    pub fn finish(self) -> Result<(), FfError> {
        match self {
            Muxer::V6(m) => m.finish(),
            Muxer::V7(m) => m.finish(),
            Muxer::V8(m) => m.finish(),
            Muxer::V9(m) => m.finish(),
        }
    }
}

/// Open source — dynamic dispatch by version.
pub enum Source {
    V6(v6::Source),
    V7(v7::Source),
    V8(v8::Source),
    V9(v9::Source),
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
        let (backend, _) = init().map_err(FfError::NotFound)?;
        match backend {
            Backend::V6(api) => v6::Source::open_with_index(api, path, prebuilt).map(Source::V6),
            Backend::V7(api) => v7::Source::open_with_index(api, path, prebuilt).map(Source::V7),
            Backend::V8(api) => v8::Source::open_with_index(api, path, prebuilt).map(Source::V8),
            Backend::V9(api) => v9::Source::open_with_index(api, path, prebuilt).map(Source::V9),
        }
    }

    /// (audio packet pts, video keyframe pts) — for persisting in the sidecar DB.
    pub fn packet_index(&self) -> (&[i64], &[i64]) {
        match self {
            Source::V6(s) => s.packet_index(),
            Source::V7(s) => s.packet_index(),
            Source::V8(s) => s.packet_index(),
            Source::V9(s) => s.packet_index(),
        }
    }

    /// Index came ready (no scan was performed) — diagnostics/test 34.
    pub fn index_was_prebuilt(&self) -> bool {
        match self {
            Source::V6(s) => s.index_was_prebuilt(),
            Source::V7(s) => s.index_was_prebuilt(),
            Source::V8(s) => s.index_was_prebuilt(),
            Source::V9(s) => s.index_was_prebuilt(),
        }
    }

    pub fn probe(&self) -> &Probe {
        match self {
            Source::V6(s) => s.probe(),
            Source::V7(s) => s.probe(),
            Source::V8(s) => s.probe(),
            Source::V9(s) => s.probe(),
        }
    }

    /// Audio burst 48 kHz stereo f32 interleaved, exactly `out.len()` values.
    pub fn read_audio_burst(&mut self, idx: i64, out: &mut [f32]) -> Result<(), FfError> {
        match self {
            Source::V6(s) => s.read_audio_burst(idx, out),
            Source::V7(s) => s.read_audio_burst(idx, out),
            Source::V8(s) => s.read_audio_burst(idx, out),
            Source::V9(s) => s.read_audio_burst(idx, out),
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
            Source::V9(s) => s.read_video_frame(pts, width, height),
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
            Source::V9(s) => s.read_video_span(keys, width, height, emit),
        }
    }

    /// (backward_reopens, frames_decoded) — performance diagnostics.
    pub fn video_debug_stats(&self) -> (u64, u64) {
        match self {
            Source::V6(s) => s.video_debug_stats(),
            Source::V7(s) => s.video_debug_stats(),
            Source::V8(s) => s.video_debug_stats(),
            Source::V9(s) => s.video_debug_stats(),
        }
    }

    pub fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        match self {
            Source::V6(s) => s.keyframe_before(pts),
            Source::V7(s) => s.keyframe_before(pts),
            Source::V8(s) => s.keyframe_before(pts),
            Source::V9(s) => s.keyframe_before(pts),
        }
    }
}
