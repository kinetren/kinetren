//! Export. Invariants 7 and 9:
//! - a SINGLE render path: the same AudioRenderer/VideoRenderer as the preview
//!   (a different clock — “as ready”, a different Quality — the project's resolution);
//! - zero temporary files: rawvideo(RGBA) → ffmpeg stdin, audio f32le →
//!   named pipe; ffmpeg muxes on the fly, backpressure via the pipes.
//!   (The old engine wrote gigabytes of rgb24+s16le into tmpfs — defect o.)
//!
//! `render_raw_*` hands raw streams to the callbacks — the golden tests stand on this
//! (test 11: stream hashes + “preview == export” by construction).

use crate::audio::AudioRenderer;
use crate::fetch::DirectFetcher;
use crate::fxstore::FxStateStore;
use crate::source_id;
use crate::video::{VideoRenderer, video_cache_idx};
use crate::video_plan::VideoGpuExec;
use kt_cache::{BurstCache, CacheConfig};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Export progress (ETA/speed is computed by the UI — wall-clock is not ours, inv. 10).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExportProgress {
    pub audio_frames: u64,
    pub total_audio_frames: u64,
    pub video_frames: u64,
    pub total_video_frames: u64,
    /// Current phase (item 17: visible what the export is busy with).
    pub phase: ExportPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportPhase {
    #[default]
    Video,
    Audio,
    Finalize,
    Done,
}

#[derive(Debug, Clone, Default)]
pub struct ExportStats {
    pub video_frames: u64,
    pub audio_frames: u64,
    pub underruns: u64,
    /// Which video encoder was chosen (for the UI); empty for render_raw_*.
    pub encoder: String,
    /// Video layers not decoded by the frame's time (in export this is an error
    /// of the source, not of timing — the decode is blocking).
    pub missing_video_layers: u64,
    /// Loudness of the MIXED material (R.49, item 371). `None` — audio was not
    /// measured (video pass) or the material is entirely below the absolute gate.
    ///
    /// Measured on the pass that RUNS ANYWAY: the meter is fed the finished
    /// chunk buffer — not a single new decode and not a single new cycle.
    ///
    /// WITHOUT the master gain — and this is not an oversight: the master gain lives only in
    /// `Player`, it is not in `Project`, nor in .otio, export does not
    /// apply it, and it is set by the meter panel's fader. This is the loudness of
    /// CONTROL, not of the material. Consequence: the number in the preview and the
    /// file's number coincide by construction.
    pub loudness: Option<kt_fx::audio::loudness::Loudness>,
}

/// Export range: [0, end of the last clip].
/// Export video cache budget.
pub const EXPORT_VIDEO_CACHE_BYTES: usize = 192 * 1024 * 1024;

/// How many frames to decode in one batch so that they GUARANTEED
/// survive in the cache until composition (R.15 item 1). A batch takes no more than
/// half the budget; `layers` — the maximum of SIMULTANEOUS video layers
/// (R.31 item 4: each moment of the chunk holds one frame PER LAYER in the cache —
/// a batch computed for one layer was evicting the pre-frames of neighbouring tracks
/// before composition, and the render lost layers).
pub fn export_chunk_frames(budget_bytes: usize, width: u32, height: u32, layers: usize) -> i64 {
    let frame = (width.max(1) as usize) * (height.max(1) as usize) * 4 * layers.max(1);
    ((budget_bytes / 2) / frame.max(1)).clamp(1, 120) as i64
}

pub fn project_range(project: &Project) -> Tick {
    let material = project.clips.iter().map(|c| c.start + c.duration).max().unwrap_or(Tick::ZERO);
    // a foreign file's edit length (trailing Gap) is counted as its end: the render
    // reaches it with black/silence, as in the reference and melt (R.25)
    project.timeline_end.map_or(material, |e| e.max(material))
}

/// The export audio stream: interleaved f32 48k/2ch in chunks per burst.
/// The same AudioRenderer as in the player; DirectFetcher = “clock as ready”.
pub fn render_raw_audio(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    emit: impl FnMut(&[f32]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    render_raw_audio_range(project, sources, None, &AtomicBool::new(false).into(), |_| {}, emit)
}

/// The full variant: range, cancel, progress.
pub fn render_raw_audio_range(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    range: Option<(Tick, Tick)>,
    cancel: &Arc<AtomicBool>,
    mut progress: impl FnMut(u64),
    mut emit: impl FnMut(&[f32]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    let mut stats = ExportStats::default();
    let mut fetcher = DirectFetcher::new(None);
    for (id, s) in sources {
        fetcher.add_source(id, s);
    }
    // Generators are opened BY export ITSELF (R.59, stage 2): their addresses
    // are derived from events, and the caller has nowhere to take them from — and the list is
    // the same `gen_sources` by which the player raises them. If the two
    // diverged, the export would render something other than the preview showed.
    for (addr, state) in crate::gen_sources(project) {
        fetcher.add_source(addr, kt_media::factory::open_gen(&state));
    }
    let renderer = AudioRenderer::new(project);
    let mut store = FxStateStore::new();
    renderer.sync_fx(&mut store);
    let (start_frame, end_frame) = match range {
        Some((a, b)) => (a.to_audio_frames(), b.to_audio_frames()),
        None => (0, project_range(project).to_audio_frames()),
    };
    let mut meter = kt_fx::audio::loudness::LoudnessMeter::new();
    let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
    let mut head = start_frame;
    while head < end_frame {
        if cancel.load(Ordering::Acquire) {
            return Err(std::io::Error::other("cancelled"));
        }
        progress(stats.audio_frames);
        let n = ((end_frame - head) as usize).min(AUDIO_BURST_FRAMES);
        let sub = &mut chunk[..n * AUDIO_CHANNELS];
        let s = renderer.render_frames_with(&mut fetcher, &mut store, head, sub);
        stats.underruns += s.underruns;
        meter.feed(sub);
        emit(sub)?;
        stats.audio_frames += n as u64;
        head += n as i64;
    }
    stats.loudness = Some(meter.finish());
    Ok(stats)
}

/// Export render backends (R.29). Test/old calls run without them
/// (CPU — the byte reference, golden tests untouched); the product (kt-app/kt-cli)
/// passes the GPU executor explicitly.
#[derive(Default, Clone)]
pub struct ExportBackends {
    pub video_gpu: Option<Arc<dyn VideoGpuExec>>,
}

/// Export video stream: RGBA frames on the project's fps grid. The composite is
/// THE SAME frame plan (`VideoRenderer::frame_plan`) as in the preview;
/// the executor is CPU (this variant) or GPU (`_with` with backends).
pub fn render_raw_video(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    render_raw_video_range(
        project,
        sources,
        quality,
        None,
        &AtomicBool::new(false).into(),
        |_| {},
        emit,
    )
}

/// The full variant: range, cancel, progress. CPU executor.
pub fn render_raw_video_range(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    cancel: &Arc<AtomicBool>,
    progress: impl FnMut(u64),
    emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    render_raw_video_range_with(
        project,
        sources,
        quality,
        range,
        &ExportBackends::default(),
        cancel,
        progress,
        emit,
    )
}

/// The variant with backend choice. A GPU error IN THE MIDDLE of export is an export
/// error, not a silent switch to CPU: a started file must be rendered by
/// one backend (wording of invariant 7, R.29).
#[allow(clippy::too_many_arguments)]
pub fn render_raw_video_range_with(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    backends: &ExportBackends,
    cancel: &Arc<AtomicBool>,
    mut progress: impl FnMut(u64),
    mut emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    let mut stats = ExportStats::default();
    let mut src: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources.into_iter().collect();
    // see `render_raw_audio_range`: export opens the generators itself
    for (addr, state) in crate::gen_sources(project) {
        src.insert(addr, kt_media::factory::open_gen(&state));
    }
    // source facts: native for the decode scale step (R.46, item 321) and
    // procedural for continuous time (R.56, item 448). Export must ask
    // THE SAME things as the preview — otherwise the preview and the file would diverge.
    let mut facts = crate::video::SourceFacts::default();
    for (m, s) in src.iter() {
        facts.observe(*m, s.info());
    }
    // A cache of a pair of frames: the composite reads from the cache (the same code as the preview),
    // export only fills it with a blocking decode.
    let cache = BurstCache::new(CacheConfig {
        max_audio_bytes: 1,
        max_video_bytes: EXPORT_VIDEO_CACHE_BYTES,
    });
    let mut video = VideoRenderer::new_with_facts(project, &facts);
    let (mut t, end) = match range {
        Some((a, b)) => (video.grid.align_floor(a), b),
        None => (Tick(0), project_range(project)),
    };
    let step = video.grid.frame_step();
    // Frames are taken in BATCHES via read_video_span — by the same path as the
    // preview (item 14 round 14). Previously this was a per-frame read_video_frame:
    // when the project's fps > the source's fps, on every second frame it went into
    // a seek + decoder recreation, and the sampling semantics diverged from the
    // span — hence judder in the finished file and a violation of invariant 7.
    // The chunk size is computed from MEMORY, not “120 and that's it” (R.15 item 1):
    // 120 frames of 1080p are 995 MB; they did not fit into the export cache, and the LRU
    // evicted them BEFORE composition, and the render came out black with rare
    // surviving frames. At 320×180 (where I “checked” this) the chunk fit —
    // hence the missed regression.
    // Computed by the LARGEST decode quality (R.46, item 318): a clip that is taken
    // larger than the Quality occupies S^2 times more in the cache —
    // at Quality the chunk would be underestimated and would bring back the same regression.
    let dq_max = video.max_decode_quality(quality);
    let chunk_frames = export_chunk_frames(
        EXPORT_VIDEO_CACHE_BYTES,
        dq_max.width,
        dq_max.height,
        video.max_concurrent_layers(),
    );
    let mut frame_buf = Vec::new();
    while t < end {
        if cancel.load(Ordering::Acquire) {
            return Err(std::io::Error::other("cancelled"));
        }
        let chunk_end = Tick((t.0 + chunk_frames * step).min(end.0));
        // chunk keys for each source: strictly increasing, duplicates removed
        // group key — (media, DECODE QUALITY): the decoder span does not mix
        // sizes (kt-media), and the request already comes with the quality (R.46)
        let mut per_src: std::collections::BTreeMap<(MediaId, Quality), Vec<Tick>> =
            Default::default();
        for (media, src_pts, dq) in
            video.wanted_frames(kt_time::TickRange::new(t, chunk_end), quality)
        {
            per_src.entry((media, dq)).or_default().push(src_pts);
        }
        for ((media, dq), keys) in per_src {
            let Some(s) = src.get_mut(&media) else {
                stats.missing_video_layers += keys.len() as u64;
                continue;
            };
            let sid = source_id(media);
            let missing: Vec<Tick> = keys
                .into_iter()
                .filter(|k| cache.get_video(sid, Some(dq), video_cache_idx(*k)).is_none())
                .collect();
            if missing.is_empty() {
                continue;
            }
            let ok = s
                .read_video_span(&missing, Some(dq), &mut |k, frame| {
                    cache.insert_video(sid, Some(dq), video_cache_idx(k), frame.into());
                })
                .is_ok();
            if !ok {
                stats.missing_video_layers += 1;
            }
        }
        while t < chunk_end {
            if cancel.load(Ordering::Acquire) {
                return Err(std::io::Error::other("cancelled"));
            }
            progress(stats.video_frames);
            let complete = match &backends.video_gpu {
                Some(gpu) => {
                    let plan = video.frame_plan(&cache, t, quality);
                    gpu.execute_into(&plan, &mut frame_buf)
                        .map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?
                }
                None => video.compose_export_into(&cache, t, quality, &mut frame_buf),
            };
            if !complete {
                stats.missing_video_layers += 1;
            }
            emit(&frame_buf)?;
            stats.video_frames += 1;
            t = Tick(t.0 + step);
        }
    }
    Ok(stats)
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg exited with {0}")]
    FfmpegFailed(std::process::ExitStatus),
    #[error("audio thread panicked")]
    AudioThread,
    #[error("export cancelled")]
    Cancelled,
}
// The GpuRequired refusal (R.29) was lifted in R.44: Pan3d is also computed on CPU
// (the inverse homography in the fused geometry pass) — export without GPU is legal,
// just slower. Test 301.

/// Streaming export to mp4 (h264+aac). `audio_sources`/`video_sources` are
/// independent sets (written from two streams into two pipes in parallel,
/// otherwise a deadlock on backpressure).
pub fn export_mp4(
    project: &Project,
    audio_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    video_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    out_path: &str,
) -> Result<ExportStats, ExportError> {
    export_mp4_with(
        project,
        audio_sources,
        video_sources,
        out_path,
        None,
        &ExportOptions::default(),
        &AtomicBool::new(false).into(),
        &AtomicBool::new(false).into(),
        |_| {},
    )
}

/// Full export: range (region), cancel (kill ffmpeg + deletion of the
/// partial file), progress callback (called from both threads).
/// Export settings (item 5): codec choice, quality, bitrate, resolution.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// CRF/CQ/QP (23 ≈ visually transparent). Ignored when video_bitrate is set.
    pub quality: u32,
    /// Target video bitrate, bit/s (None = quality mode).
    pub video_bitrate: Option<u32>,
    pub audio_bitrate: u32,
    /// None = automatic choice with hw priority; Some = only this encoder
    /// (if it does not open — an explicit error, no fallback).
    pub force_encoder: Option<String>,
    /// Resolution override (None = from the project settings).
    pub resolution: Option<(u32, u32)>,
}

impl Default for ExportOptions {
    fn default() -> ExportOptions {
        ExportOptions {
            quality: 23,
            video_bitrate: None,
            audio_bitrate: 192_000,
            force_encoder: None,
            resolution: None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn export_mp4_with(
    project: &Project,
    audio_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    video_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    out_path: &str,
    range: Option<(Tick, Tick)>,
    opts: &ExportOptions,
    cancel: &Arc<AtomicBool>,
    pause: &Arc<AtomicBool>,
    progress: impl Fn(ExportProgress) + Send + Sync + 'static,
) -> Result<ExportStats, ExportError> {
    export_mp4_with_backends(
        project,
        audio_sources,
        video_sources,
        out_path,
        range,
        opts,
        &ExportBackends::default(),
        cancel,
        pause,
        progress,
    )
}

/// Product variant (kt-app/kt-cli): render backends are passed explicitly —
/// GPU by default, CPU when the engine is absent (R.29).
#[allow(clippy::too_many_arguments)]
pub fn export_mp4_with_backends(
    project: &Project,
    audio_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    video_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    out_path: &str,
    range: Option<(Tick, Tick)>,
    opts: &ExportOptions,
    backends: &ExportBackends,
    cancel: &Arc<AtomicBool>,
    pause: &Arc<AtomicBool>,
    progress: impl Fn(ExportProgress) + Send + Sync + 'static,
) -> Result<ExportStats, ExportError> {
    // pause (port of v1 waitWhilePaused): cooperative wait of 40 ms,
    // cancellable; checked on every frame/chunk via the progress wrappers
    let wait_paused = {
        let pause = pause.clone();
        let cancel = cancel.clone();
        move || {
            while pause.load(Ordering::Acquire) && !cancel.load(Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
        }
    };
    let (ow, oh) = opts.resolution.unwrap_or((project.settings.width, project.settings.height));
    let quality = Quality { width: ow.max(2) & !1, height: oh.max(2) & !1 };
    let fps = project.settings.fps;
    let (r0, r1) = match range {
        Some((a, b)) => (a, b),
        None => (Tick(0), project_range(project)),
    };
    let total_audio = (r1.to_audio_frames() - r0.to_audio_frames()).max(0) as u64;
    let total_video = ((r1.0 - r0.0) / (kt_time::TICK_HZ / fps as i64)).max(0) as u64;

    // In-process muxer: codecs are taken from the SYSTEM FFmpeg,
    // so its hardware encoders are available. An external ffmpeg binary
    // and mkfifo are no longer needed (invariant 9 strengthened: not only without
    // temporary files, but also without external processes).
    let mut muxer = kt_media::muxer::Muxer::open(&kt_media::muxer::MuxConfig {
        path: out_path.to_string(),
        width: quality.width,
        height: quality.height,
        fps,
        quality: opts.quality,
        video_bitrate: opts.video_bitrate,
        audio_bitrate: opts.audio_bitrate,
        force_encoder: opts.force_encoder.clone(),
    })
    .map_err(|e| ExportError::Io(std::io::Error::other(e)))?;

    // Audio and video are rendered sequentially into one muxer (it
    // interleaves packets itself via av_interleaved_write_frame).
    let a_done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let v_done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let progress = Arc::new(progress);
    let report = {
        let progress = progress.clone();
        let a_done = a_done.clone();
        let v_done = v_done.clone();
        move |phase: ExportPhase| {
            progress(ExportProgress {
                audio_frames: a_done.load(Ordering::Relaxed),
                total_audio_frames: total_audio,
                video_frames: v_done.load(Ordering::Relaxed),
                total_video_frames: total_video,
                phase,
            })
        }
    };

    // 1) video (an error/cancel inside the phases — delete the partial file, item 17)
    let cleanup_on_err = |e: ExportError| {
        let _ = std::fs::remove_file(out_path);
        e
    };
    let vstats = {
        let v_done = v_done.clone();
        let report = report.clone();
        render_raw_video_range_with(
            project,
            video_sources,
            quality,
            range,
            backends,
            cancel,
            |done| {
                wait_paused();
                v_done.store(done, Ordering::Relaxed);
                if done % 10 == 0 {
                    report(ExportPhase::Video);
                }
            },
            |rgba| muxer.write_video(rgba).map_err(std::io::Error::other),
        )
        .map_err(|e| cleanup_on_err(e.into()))?
    };
    // 2) audio
    let astats = {
        let a_done = a_done.clone();
        let report = report.clone();
        render_raw_audio_range(
            project,
            audio_sources,
            range,
            cancel,
            |done| {
                wait_paused();
                a_done.store(done, Ordering::Relaxed);
                if done % 48_000 < 1024 {
                    report(ExportPhase::Audio);
                }
            },
            |chunk| muxer.write_audio(chunk).map_err(std::io::Error::other),
        )
        .map_err(|e| cleanup_on_err(e.into()))?
    };

    if cancel.load(Ordering::Acquire) {
        drop(muxer);
        let _ = std::fs::remove_file(out_path);
        return Err(ExportError::Cancelled);
    }
    report(ExportPhase::Finalize);
    let encoder = muxer.encoder_name().to_string();
    muxer.finish().map_err(|e| ExportError::Io(std::io::Error::other(e)))?;
    report(ExportPhase::Done);
    Ok(ExportStats {
        video_frames: vstats.video_frames,
        audio_frames: astats.audio_frames,
        underruns: astats.underruns,
        missing_video_layers: vstats.missing_video_layers,
        encoder,
        // the loudness is computed on the audio pass — with the same meter, the same
        // buffer and without the master gain (item 371)
        loudness: astats.loudness,
    })
}
