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
use std::time::Instant;

pub use crate::export_pipeline::{ExportConverter, ExportProfile, ExportSink};

/// Export progress (ETA/speed is computed by the UI — wall-clock is not ours, inv. 10).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExportProgress {
    pub audio_frames: u64,
    pub total_audio_frames: u64,
    pub video_frames: u64,
    pub total_video_frames: u64,
    /// Current phase (item 17: visible what the export is busy with).
    pub phase: ExportPhase,
    /// Encoded bytes handed to the muxer so far, per stream (render-window-v1-parity, D2):
    /// counts, never times — the size, the estimate and the bitrate are the caller's arithmetic.
    pub video_bytes: u64,
    pub audio_bytes: u64,
}

/// What an export reports (render-window-v1-parity, D1): EVENTS, not a state — the encoder that
/// opened (once, before the first frame), the progress steps, and log facts. None carries a clock
/// (invariant 11): two runs of one export give the same stream (t955).
#[derive(Debug, Clone, PartialEq)]
pub enum ExportEvent {
    Opened(kt_media::muxer::OpenReport),
    Progress(ExportProgress),
    Log(ExportLog),
}

/// A log FACT, not preformatted text: the UI translates it, a test compares values.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportLog {
    /// A candidate of the cascade exists in this FFmpeg but did not open.
    NotOpened { name: String, code: i32 },
    /// A device candidate skipped before its open (hw-encode-context D3): no device, the
    /// hardware context functions missing, or the frames context refused.
    Skipped { name: String, reason: String },
    /// A source is rendered as its placeholder (the caller's list).
    Placeholder { label: String },
    /// Consecutive frames were composed without a layer (export-pipeline-determinism D4): the first
    /// and the last frame's time, and the media that gave no picture.
    IncompleteFrames { from: Tick, to: Tick, media: Vec<String> },
    Phase(ExportPhase),
    Finalizing,
    Error(String),
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
    /// Layers frames were written without: counted at the frame, so a span that failed and healed
    /// is not a loss, and one loss is counted once.
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

/// Export render backends (R.29): the product (kt-app/kt-cli) passes the executor; without one
/// an export fails with `NO_DEVICE`.
#[derive(Default, Clone)]
pub struct ExportBackends {
    pub video_gpu: Option<Arc<dyn VideoGpuExec>>,
    /// More executors on devices of their own: a software adapter runs one frame per queue, so
    /// each frame worker composes on its own (compute-kernels-render, D4). Empty on a real card.
    pub video_gpu_more: Vec<Arc<dyn VideoGpuExec>>,
}

/// How many frames an export of `[r0, r1)` renders at `fps`: the grid frames from the one that
/// CONTAINS `r0` (the frame the preview shows there, invariant 7) up to, not including, `r1`.
/// The progress total and the render loop both follow this rule (render-crash-and-recovery D1).
pub fn video_frames_in(fps: u32, r0: Tick, r1: Tick) -> u64 {
    let grid = kt_time::FpsGrid::new(fps);
    let step = grid.frame_step();
    let first = grid.align_floor(r0).0;
    if r1.0 <= first {
        return 0;
    }
    ((r1.0 - first + step - 1) / step) as u64
}

/// The export's video stream: RGBA frames on the project's fps grid, the composite is THE SAME
/// frame plan (`VideoRenderer::frame_plan`) as in the preview. A GPU error is an export error.
#[allow(clippy::too_many_arguments)]
pub fn render_raw_video_range_with(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    backends: &ExportBackends,
    cancel: &Arc<AtomicBool>,
    progress: impl FnMut(u64),
    emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    render_raw_video_profiled(project, sources, quality, range, backends, cancel, None, &|_, _| {}, progress, emit)
}

/// The serial video pass with its stage times (export-throughput D6): `profile` only receives
/// clock readings, the render does not see them.
#[allow(clippy::too_many_arguments)]
fn render_raw_video_profiled(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    backends: &ExportBackends,
    cancel: &Arc<AtomicBool>,
    profile: Option<&ExportProfile>,
    incomplete: &(dyn Fn(Tick, &[MediaId]) + Sync),
    mut progress: impl FnMut(u64),
    mut emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    let gpu = backends.video_gpu.as_ref().ok_or_else(|| std::io::Error::other("no video device"))?;
    // while it runs the preview's memos keep only their bases (memory-ledger D6)
    let _room = kt_time::mem::claim();
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
    // an export owns its stream memo (datamosh-codec-path, D3): a preview on another thread cannot
    // evict what this export is about to read
    video.set_stream_memo(std::sync::Arc::new(kt_plugin_api::stream::StreamMemo::new(64 << 20)));
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
        let mark = Instant::now();
        let requests = chunk_requests(&video, (t, chunk_end), quality, |m| src.contains_key(&m), |sid, dq, idx| {
            cache.get_video(sid, Some(dq), idx).is_some()
        });
        for (media, keys) in requests {
            decode_requests(src.get_mut(&media).unwrap(), media, &keys, &cache, None);
        }
        ExportProfile::add(profile.map(|p| &p.decode_ns), mark);
        while t < chunk_end {
            if cancel.load(Ordering::Acquire) {
                return Err(std::io::Error::other("cancelled"));
            }
            progress(stats.video_frames);
            stream_preroll(&video, &cache, &mut src, t, quality);
            let mark = Instant::now();
            let mut span = kt_time::trace::span(kt_time::trace::Ev::Job);
            span.aux(kt_time::trace::job::EXPORT_FRAME).a(t.0 as u64);
            let plan = video.frame_plan(&cache, t, quality);
            let complete = gpu
                .execute_into(&plan, &mut frame_buf)
                .map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
            ExportProfile::add(profile.map(|p| &p.compose_ns), mark);
            if !complete {
                let absent = video.absent_layers(&cache, t, quality);
                stats.missing_video_layers += absent.len().max(1) as u64;
                incomplete(t, &absent);
            }
            emit(&frame_buf)?;
            stats.video_frames += 1;
            t = Tick(t.0 + step);
        }
    }
    Ok(stats)
}

/// One chunk's decode requests per source (export-throughput D7): the frames the chunk names
/// and the cache lacks, as (key, decode quality) ordered by key — one forward pass per source
/// whatever qualities it is asked at. A request of a source not in `has` counts its keys missing.
/// The serial and the pipelined export both decode through this and `decode_requests`.
pub(crate) fn chunk_requests(
    video: &VideoRenderer,
    (t0, t1): (Tick, Tick),
    quality: Quality,
    has: impl Fn(MediaId) -> bool,
    mut in_cache: impl FnMut(kt_cache::SourceId, Quality, i64) -> bool,
) -> Vec<(MediaId, Vec<(Tick, Quality)>)> {
    let mut per_media: std::collections::BTreeMap<MediaId, Vec<(Tick, Quality)>> = Default::default();
    for (media, src_pts, dq) in video.wanted_frames(kt_time::TickRange::new(t0, t1), quality) {
        per_media.entry(media).or_default().push((src_pts, dq));
    }
    let mut out = Vec::new();
    for (media, mut keys) in per_media {
        if !has(media) {
            continue;
        }
        let sid = source_id(media);
        keys.sort();
        keys.dedup();
        keys.retain(|(k, dq)| !in_cache(sid, *dq, video_cache_idx(*k)));
        if !keys.is_empty() {
            out.push((media, keys));
        }
    }
    out
}

/// Decode one source's requests into the cache. What counts as lost is decided at the frame, by what
/// it lacked: a span that failed may still give all its frames. With `held` (the pipelined export) each frame is inserted held and its key noted: only a frame that
/// really arrived has a hold to give back. And a frame the span did not give is asked again alone,
/// before anything composes it: a failed span drops every key after the failure, and the serial
/// export gets those back from its pre-roll (`stream_preroll`), which a pipeline stage cannot run.
pub(crate) fn decode_requests(
    s: &mut Box<dyn MediaSource>,
    media: MediaId,
    keys: &[(Tick, Quality)],
    cache: &BurstCache,
    mut held: Option<&mut Vec<kt_cache::BurstKey>>,
) {
    let sid = source_id(media);
    let retry = held.is_some();
    let mut arrived = std::collections::HashSet::new();
    let mut insert = |k: Tick, dq: Quality, frame: kt_media::VideoFrame, arrived: &mut std::collections::HashSet<(Tick, Quality)>| {
        let idx = video_cache_idx(k);
        arrived.insert((k, dq));
        match held.as_mut() {
            Some(held) => {
                cache.insert_video_held(sid, Some(dq), idx, frame.into());
                held.push(kt_cache::BurstKey { source: sid, kind: kt_cache::BurstKind::video(Some(dq)), idx });
            }
            None => cache.insert_video(sid, Some(dq), idx, frame.into()),
        }
    };
    let _ = s.read_video_span_qualities(keys, &mut |k, dq, frame| insert(k, dq, frame, &mut arrived));
    if retry {
        for &(k, dq) in keys.iter().filter(|key| !arrived.contains(key)).collect::<Vec<_>>() {
            let mut again = std::collections::HashSet::new();
            let _ = s.read_video_span(&[k], Some(dq), &mut |k, frame| insert(k, dq, frame, &mut again));
        }
    }
}

/// A run of consecutive frames composed without a layer, and what was missing.
struct IncompleteRun {
    from: Tick,
    to: Tick,
    media: std::collections::BTreeSet<MediaId>,
}

impl IncompleteRun {
    fn into_log(self, project: &Project) -> ExportLog {
        let name = |id: MediaId| match project.media.iter().find(|m| m.id == id) {
            Some(m) => std::path::Path::new(&m.path)
                .file_name()
                .map_or_else(|| m.path.clone(), |f| f.to_string_lossy().into_owned()),
            None => format!("media {}", id.0),
        };
        ExportLog::IncompleteFrames { from: self.from, to: self.to, media: self.media.into_iter().map(name).collect() }
    }
}

/// Joins consecutive incomplete frames into runs, in the frame order they are told.
struct IncompleteRuns {
    step: Tick,
    run: std::sync::Mutex<Option<IncompleteRun>>,
}

impl IncompleteRuns {
    fn new(step: Tick) -> IncompleteRuns {
        IncompleteRuns { step, run: std::sync::Mutex::new(None) }
    }

    /// The run this frame ended, if it does not continue the last one.
    fn note(&self, t: Tick, media: &[MediaId]) -> Option<IncompleteRun> {
        let mut run = self.run.lock().unwrap();
        if let Some(r) = run.as_mut()
            && t.0 <= r.to.0 + self.step.0
        {
            r.to = t;
            r.media.extend(media);
            return None;
        }
        run.replace(IncompleteRun { from: t, to: t, media: media.iter().copied().collect() })
    }

    fn finish(&self) -> Option<IncompleteRun> {
        self.run.lock().unwrap().take()
    }
}

/// The stream stages' PRE-ROLL before frame `t` (datamosh-codec-path, D6): while the frame names
/// source frames the cache lacks (a range starting inside a mosh, an evicted memo entry), decode the
/// EARLIEST of them in a batch bounded by half the cache's video budget and advance the codecs over
/// it. Every batch leaves its results in the memo, so the loop ends; a frame no source can give
/// ends it too. Every offline path with its own sources runs it before a composite — the export,
/// and the reel strip and project poster (decode-request-storm: they read the whole history into a
/// 16–64 MB cache at once, which dropped its front before the composite read it).
pub fn stream_preroll(
    video: &VideoRenderer,
    cache: &BurstCache,
    src: &mut std::collections::HashMap<MediaId, Box<dyn MediaSource>>,
    t: Tick,
    quality: Quality,
) {
    let mut last: Option<Vec<(MediaId, Tick, Quality)>> = None;
    loop {
        let missing: Vec<(MediaId, Tick, Quality)> = video
            .layers_at(t, quality)
            .into_iter()
            .filter(|(m, pts, q)| cache.get_video(source_id(*m), Some(*q), video_cache_idx(*pts)).is_none())
            .collect();
        if missing.is_empty() || last.as_ref() == Some(&missing) {
            return;
        }
        let frame_bytes = missing.iter().map(|(_, _, q)| q.width as usize * q.height as usize * 4).max().unwrap_or(1);
        let batch = (cache.max_video_bytes() / 2 / frame_bytes.max(1)).max(1);
        let mut per_src: std::collections::BTreeMap<(MediaId, Quality), Vec<Tick>> = Default::default();
        for &(m, pts, q) in missing.iter().take(batch) {
            per_src.entry((m, q)).or_default().push(pts);
        }
        for ((m, q), mut keys) in per_src {
            let Some(s) = src.get_mut(&m) else { continue };
            keys.sort();
            keys.dedup();
            let sid = source_id(m);
            let _ = s.read_video_span(&keys, Some(q), &mut |k, frame| {
                cache.insert_video(sid, Some(q), video_cache_idx(k), frame.into());
            });
        }
        video.stream_advance(cache, t, quality);
        last = Some(missing);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    // the message alone: a person reads it in the render window, and "io" names a Rust type
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg exited with {0}")]
    FfmpegFailed(std::process::ExitStatus),
    #[error("audio thread panicked")]
    AudioThread,
    #[error("export cancelled")]
    Cancelled,
    #[error("no video device: {0}")]
    NoDevice(String),
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
    /// Labels of the sources the caller opened as placeholders: each becomes a log fact
    /// (render-window-v1-parity, D1) — export cannot tell a placeholder from a source itself.
    pub placeholders: Vec<String>,
    /// The former one-thread export (export-throughput D2): video pass, then audio pass, the
    /// encoder on one thread. The reference the pipelined export is held to (t1451) and the
    /// "before" of `kt-cli export --serial --time`.
    pub serial: bool,
    /// The video encoder's threads, when not the export's own choice (`auto`, serial `1`): at the
    /// same count the pipelined and the serial streams can be compared packet by packet.
    pub encoder_threads: Option<u32>,
    /// The machine is short of memory: half the frame cache, four encoder threads (gpu-render-hardening, D1).
    pub tight: bool,
    /// Collects the busy time of every stage (`kt-cli export --time`).
    pub profile: Option<Arc<ExportProfile>>,
}

impl Default for ExportOptions {
    fn default() -> ExportOptions {
        ExportOptions {
            quality: 23,
            video_bitrate: None,
            audio_bitrate: 192_000,
            force_encoder: None,
            resolution: None,
            placeholders: Vec::new(),
            serial: false,
            encoder_threads: None,
            tight: false,
            profile: None,
        }
    }
}

/// Product variant (kt-app/kt-cli): render backends are passed explicitly —
/// the GPU engine composes (R.29).
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
    events: impl Fn(ExportEvent) + Send + Sync + 'static,
) -> Result<ExportStats, ExportError> {
    if backends.video_gpu.is_none() {
        return Err(ExportError::NoDevice("no Vulkan, DX12 or Metal device".into()));
    }
    // compile what the project reaches on every device before the first frame (pipeline-prewarm
    // D5): the same work, but no frame worker stands in a compiler while the ring waits for it
    {
        let ids: Vec<String> = crate::prewarm::effect_ids(project).into_iter().map(String::from).collect();
        let execs: Vec<&Arc<dyn VideoGpuExec>> = backends.video_gpu.iter().chain(&backends.video_gpu_more).collect();
        let runs = crate::prewarm::run_list(&crate::prewarm::run_shapes(project));
        for e in &execs {
            e.prewarm(&ids);
            e.prewarm_runs(&runs);
        }
        crate::prewarm::compute(&ids);
        for e in &execs {
            e.settle_prewarm();
        }
    }
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
    // the SAME count the loop of `render_raw_video_range_with` renders (render-crash-and-recovery
    // D1): the floored `(r1 - r0) / step` was one frame short on an off-grid region, and the
    // progress window's fraction went past 1
    let total_video = video_frames_in(fps, r0, r1);

    // In-process muxer: codecs are taken from the SYSTEM FFmpeg,
    // so its hardware encoders are available. An external ffmpeg binary
    // and mkfifo are no longer needed (invariant 9 strengthened: not only without
    // temporary files, but also without external processes).
    let muxer = kt_media::muxer::Muxer::open(&kt_media::muxer::MuxConfig {
        path: out_path.to_string(),
        width: quality.width,
        height: quality.height,
        fps,
        quality: opts.quality,
        video_bitrate: opts.video_bitrate,
        audio_bitrate: opts.audio_bitrate,
        force_encoder: opts.force_encoder.clone(),
        threads: opts.encoder_threads.unwrap_or(if opts.serial { 1 } else if opts.tight { 4 } else { 0 }),
    });
    let events = Arc::new(events);
    let mut muxer = match muxer {
        Ok(m) => m,
        Err(e) => {
            events(ExportEvent::Log(ExportLog::Error(e.to_string())));
            return Err(ExportError::Io(std::io::Error::other(e)));
        }
    };
    // THE ENCODER IS NAMED BEFORE THE FIRST FRAME (D1): the cascade's failures first, then the
    // one that opened
    let opened = muxer.open_report().clone();
    for (name, code) in &opened.tried {
        events(ExportEvent::Log(ExportLog::NotOpened { name: name.clone(), code: *code }));
    }
    for (name, reason) in &opened.skipped {
        events(ExportEvent::Log(ExportLog::Skipped { name: name.clone(), reason: reason.clone() }));
    }
    events(ExportEvent::Opened(opened));
    for label in &opts.placeholders {
        events(ExportEvent::Log(ExportLog::Placeholder { label: label.clone() }));
    }

    // Audio and video are rendered sequentially into one muxer (it
    // interleaves packets itself via av_interleaved_write_frame).
    let a_done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let v_done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    // the muxer's byte counts, stored after each write (the writer owns the muxer)
    let v_bytes = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let a_bytes = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let report = {
        let events = events.clone();
        let a_done = a_done.clone();
        let v_done = v_done.clone();
        let v_bytes = v_bytes.clone();
        let a_bytes = a_bytes.clone();
        move |phase: ExportPhase| {
            events(ExportEvent::Progress(ExportProgress {
                audio_frames: a_done.load(Ordering::Relaxed),
                total_audio_frames: total_audio,
                video_frames: v_done.load(Ordering::Relaxed),
                total_video_frames: total_video,
                phase,
                video_bytes: v_bytes.load(Ordering::Relaxed),
                audio_bytes: a_bytes.load(Ordering::Relaxed),
            }))
        }
    };
    let store_bytes = |m: &kt_media::muxer::Muxer| {
        let (v, a) = m.bytes();
        v_bytes.store(v, Ordering::Relaxed);
        a_bytes.store(a, Ordering::Relaxed);
    };

    // 1) video (an error/cancel inside the phases — delete the partial file, item 17)
    let cleanup_on_err = |e: ExportError| {
        if !matches!(e, ExportError::Cancelled) {
            events(ExportEvent::Log(ExportLog::Error(e.to_string())));
        }
        let _ = std::fs::remove_file(out_path);
        e
    };
    let profile = opts.profile.as_deref();
    let started = Instant::now();
    events(ExportEvent::Log(ExportLog::Phase(ExportPhase::Video)));
    // a frame without a layer is told where it happens, in frame order (D4)
    let incomplete_runs = IncompleteRuns::new(Tick(kt_time::FpsGrid::new(fps).frame_step()));
    let incomplete = |t: Tick, media: &[MediaId]| {
        if let Some(run) = incomplete_runs.note(t, media) {
            events(ExportEvent::Log(run.into_log(project)));
        }
    };
    // a stream stage's pre-roll decodes on the compose thread (datamosh-codec-path D6): such a
    // project keeps the serial export (export-throughput D2)
    let serial = opts.serial || VideoRenderer::new(project).has_stream_stages();
    let (vstats, astats) = if serial {
        let vstats = {
            let v_done = v_done.clone();
            let report = report.clone();
            let mut conv = muxer.video_converter().map_err(|e| cleanup_on_err(ExportError::Io(std::io::Error::other(e))))?;
            let mut vf = muxer.alloc_video_frame().map_err(|e| cleanup_on_err(ExportError::Io(std::io::Error::other(e))))?;
            render_raw_video_profiled(
                project,
                video_sources,
                quality,
                range,
                backends,
                cancel,
                profile,
                &incomplete,
                |done| {
                    wait_paused();
                    v_done.store(done, Ordering::Relaxed);
                    if done % 10 == 0 {
                        report(ExportPhase::Video);
                    }
                },
                |rgba| {
                    let mark = Instant::now();
                    conv.convert(rgba, &mut vf).map_err(std::io::Error::other)?;
                    ExportProfile::add(profile.map(|p| &p.convert_ns), mark);
                    let mark = Instant::now();
                    muxer.write_video_frame(&vf).map_err(std::io::Error::other)?;
                    ExportProfile::add(profile.map(|p| &p.encode_ns), mark);
                    store_bytes(&muxer);
                    Ok(())
                },
            )
            .map_err(|e| cleanup_on_err(e.into()))?
        };
        // the renderer reports the frames done BEFORE each frame: the last frame is counted here, or
        // the finished window would read "179 / 180" (live scenario `render`)
        v_done.store(vstats.video_frames, Ordering::Relaxed);
        // 2) audio
        events(ExportEvent::Log(ExportLog::Phase(ExportPhase::Audio)));
        let audio_started = Instant::now();
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
                |chunk| {
                    let mark = Instant::now();
                    muxer.write_audio(chunk).map_err(std::io::Error::other)?;
                    ExportProfile::add(profile.map(|p| &p.audio_encode_ns), mark);
                    store_bytes(&muxer);
                    Ok(())
                },
            )
            .map_err(|e| cleanup_on_err(e.into()))?
        };
        if let Some(p) = profile {
            let pass = audio_started.elapsed().as_nanos() as u64;
            let enc = p.audio_encode_ns.load(Ordering::Relaxed);
            p.audio_ns.fetch_add(pass.saturating_sub(enc), Ordering::Relaxed);
            p.wall_ns.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
        (vstats, astats)
    } else {
        // 1+2) the stages concurrently (export-throughput D2): the same frames and samples
        let paused = &wait_paused;
        let video_done = |done: u64| {
            v_done.store(done, Ordering::Relaxed);
            if done.is_multiple_of(10) {
                report(ExportPhase::Video);
            }
        };
        // the writer's counts, in its fixed order (t955): a second of audio written after the
        // last video frame is a step of the audio phase
        let audio_done = |done: u64| {
            let before = a_done.swap(done, Ordering::Relaxed);
            if done / 48_000 != before / 48_000 && v_done.load(Ordering::Relaxed) >= total_video {
                report(ExportPhase::Audio);
            }
        };
        let bytes = |(v, a): (u64, u64)| {
            v_bytes.store(v, Ordering::Relaxed);
            a_bytes.store(a, Ordering::Relaxed);
        };
        let video_finished = || {
            events(ExportEvent::Log(ExportLog::Phase(ExportPhase::Audio)));
            report(ExportPhase::Audio);
        };
        let hooks = crate::export_pipeline::Hooks {
            wait_paused: paused,
            video_done: &video_done,
            audio_done: &audio_done,
            bytes: &bytes,
            video_finished: &video_finished,
            profile,
            budget: EXPORT_VIDEO_CACHE_BYTES / if opts.tight { 2 } else { 1 },
            incomplete: &incomplete,
        };
        crate::export_pipeline::run(
            project,
            video_sources,
            audio_sources,
            quality,
            range,
            backends,
            cancel,
            &hooks,
            &mut muxer,
        )
        .map_err(|e| cleanup_on_err(e.into()))?
    };
    v_done.store(vstats.video_frames, Ordering::Relaxed);
    if let Some(run) = incomplete_runs.finish() {
        events(ExportEvent::Log(run.into_log(project)));
    }

    if cancel.load(Ordering::Acquire) {
        drop(muxer);
        let _ = std::fs::remove_file(out_path);
        return Err(ExportError::Cancelled);
    }
    a_done.store(astats.audio_frames, Ordering::Relaxed);
    store_bytes(&muxer);
    events(ExportEvent::Log(ExportLog::Finalizing));
    report(ExportPhase::Finalize);
    let encoder = muxer.encoder_name().to_string();
    if let Err(e) = muxer.finish() {
        events(ExportEvent::Log(ExportLog::Error(e.to_string())));
        return Err(ExportError::Io(std::io::Error::other(e)));
    }
    events(ExportEvent::Log(ExportLog::Phase(ExportPhase::Done)));
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

/// The pipelined export of `project` into any sink (export-throughput D2, t1451): the product
/// passes the muxer, a test a recorder. No pause, no progress.
#[allow(clippy::too_many_arguments)]
pub fn render_pipelined<S: ExportSink>(
    project: &Project,
    audio_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    video_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    backends: &ExportBackends,
    cancel: &Arc<AtomicBool>,
    profile: Option<&ExportProfile>,
    sink: &mut S,
) -> std::io::Result<ExportStats> {
    render_pipelined_budget(
        project,
        audio_sources,
        video_sources,
        quality,
        range,
        backends,
        cancel,
        profile,
        EXPORT_VIDEO_CACHE_BYTES,
        sink,
    )
}

/// `render_pipelined` on another serial budget: the cache's pressure is the ratio of the budget to
/// the frame size, so a test gets the export's pressure with small frames.
#[allow(clippy::too_many_arguments)]
pub fn render_pipelined_budget<S: ExportSink>(
    project: &Project,
    audio_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    video_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    backends: &ExportBackends,
    cancel: &Arc<AtomicBool>,
    profile: Option<&ExportProfile>,
    budget: usize,
    sink: &mut S,
) -> std::io::Result<ExportStats> {
    let hooks = crate::export_pipeline::Hooks {
        wait_paused: &|| {},
        video_done: &|_| {},
        audio_done: &|_| {},
        bytes: &|_| {},
        video_finished: &|| {},
        profile,
        budget,
        incomplete: &|_, _| {},
    };
    let (v, a) = crate::export_pipeline::run(
        project,
        video_sources,
        audio_sources,
        quality,
        range,
        backends,
        cancel,
        &hooks,
        sink,
    )?;
    Ok(ExportStats {
        video_frames: v.video_frames,
        missing_video_layers: v.missing_video_layers,
        audio_frames: a.audio_frames,
        underruns: a.underruns,
        loudness: a.loudness,
        encoder: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_plan::{FramePlan, GpuBackendInfo, GpuExecError};
    use kt_model::ProjectSettings;

    /// An executor of blank frames: these tests count frames.
    struct Blank(GpuBackendInfo);
    impl VideoGpuExec for Blank {
        fn execute_into(&self, p: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
            out.clear();
            out.resize((p.out_w * p.out_h * 4) as usize, 0);
            Ok(true)
        }
        fn info(&self) -> &GpuBackendInfo {
            &self.0
        }
    }

    /// Test 1070 (render-crash-and-recovery D1): the frames the raw render delivers equal the
    /// announced total on any region, and the progress never reports more. The owner's render
    /// crashed on an off-grid region: the floored total was one frame short.
    #[test]
    fn t1070_announced_total_is_the_rendered_frames() {
        let p = Project::new(ProjectSettings {
            fps: 30,
            width: 32,
            height: 18,
            sample_rate: 48000,
            pitch_engine: Default::default(),
            default_shape: Default::default(),
            conform: Default::default(),
            settings_explicit: false,
        });
        let step = 3000; // 90000 / 30
        let regions = [
            (Tick(10 * step + step / 2), Tick(40 * step + step / 3)), // both edges off the grid
            (Tick(10 * step), Tick(40 * step)),                       // on the grid
            (Tick(5 * step), Tick(5 * step)),                         // empty
            (Tick(7 * step + 100), Tick(7 * step + 900)),             // shorter than a frame
            (Tick(7 * step + 2900), Tick(8 * step + 100)),            // across one grid line
        ];
        for (r0, r1) in regions {
            let total = video_frames_in(30, r0, r1);
            let mut emitted = 0u64;
            let mut max_progress = 0u64;
            let info = GpuBackendInfo { name: String::new(), backend: String::new(), software: true };
            let backends = ExportBackends { video_gpu: Some(Arc::new(Blank(info))), ..Default::default() };
            let stats = render_raw_video_range_with(
                &p,
                Vec::new(),
                Quality { width: 32, height: 18 },
                Some((r0, r1)),
                &backends,
                &AtomicBool::new(false).into(),
                |done| max_progress = max_progress.max(done),
                |_| {
                    emitted += 1;
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(emitted, total, "{r0:?}..{r1:?}: rendered {emitted}, announced {total}");
            assert_eq!(stats.video_frames, total, "{r0:?}..{r1:?}");
            assert!(max_progress <= total, "{r0:?}..{r1:?}: progress {max_progress} > {total}");
        }
        // the owner's case in numbers: the former floored count is short of what is rendered
        // (29 announced, 31 rendered here) — the fraction went past 1
        let (r0, r1) = regions[0];
        assert_eq!(((r1.0 - r0.0) / step) as u64, 29);
        assert_eq!(video_frames_in(30, r0, r1), 31);
    }
}
