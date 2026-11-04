//! The pipelined export (export-throughput D2): the stages of ONE output file run concurrently,
//! joined by bounded, order-preserving queues.
//!
//! ```text
//! decode (a chunk ahead, a thread per source) ─► compose (GPU thread | CPU workers)
//!     ─► convert (frame workers, round robin) ─► writer (encoder + muxer, frame order)
//! audio (its own thread) ─────────────────────────────────────────► writer (fixed interleave)
//! ```
//!
//! Every frame is the same function of (cache, t, quality) the serial export computes
//! (`render_raw_video_range_with`), the writer takes them in frame order, and the audio is the
//! same renderer's stream; so the encoder receives exactly the serial export's frames and samples
//! (t1451). What changes is only WHEN each is computed:
//! - the cache holds `PIPE_CACHE_BYTES`, and decode starts a chunk only while the frames it
//!   decoded for the chunks not yet written stay under `LOOKAHEAD_BYTES` — a serial budget below
//!   the cache, more than any chunk: a chunk being composed is never evicted before its composite
//!   (the serial export's reason for its chunk bound, R.15 item 1). The look-ahead is counted in
//!   bytes, not chunks: at 4K decode sizes a chunk is one frame, and a look-ahead of two chunks
//!   kept two of sixteen CPU workers busy;
//! - the writer interleaves the audio by a fixed rule (after video frame `i`, the audio up to that
//!   frame's end), so the call order into the muxer — and the file — does not depend on timing.
//!
//! Cancellation and errors set one `halt` flag every stage checks per frame or burst; a stage that
//! stops drops its queue ends, and every neighbour blocked on them returns (t1452).

use crate::export::{
    EXPORT_VIDEO_CACHE_BYTES, ExportBackends, ExportStats, chunk_requests, decode_requests, export_chunk_frames,
    project_range, render_raw_audio_range,
};
use crate::video::{ExportScratch, VideoRenderer};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use kt_cache::{BurstCache, CacheConfig};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::{AUDIO_CHANNELS, Tick};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

/// Where an export's time goes (export-throughput D6): the BUSY nanoseconds of every stage,
/// summed over the threads of the stage, and the wall time of the whole export. Read by
/// `kt-cli export --time`. The clock is read only into this record, which the caller asked for;
/// it never steers the render (invariant 11).
#[derive(Debug, Default)]
pub struct ExportProfile {
    pub decode_ns: AtomicU64,
    pub compose_ns: AtomicU64,
    /// GPU only: waiting for a submitted frame and copying it out (pipelined readback).
    pub readback_ns: AtomicU64,
    pub convert_ns: AtomicU64,
    /// The video encoder and the muxer (`write_video_frame`).
    pub encode_ns: AtomicU64,
    /// The audio renderer (mix + FX).
    pub audio_ns: AtomicU64,
    /// The audio encoder and the muxer (`write_audio`).
    pub audio_encode_ns: AtomicU64,
    pub wall_ns: AtomicU64,
    /// How many threads each concurrent stage had (0 — the serial export).
    pub frame_workers: AtomicU64,
}

impl ExportProfile {
    pub(crate) fn add(slot: Option<&AtomicU64>, since: Instant) {
        if let Some(s) = slot {
            s.fetch_add(since.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
    }

    /// The record as text: one line per stage, busy seconds and the share of the wall time.
    pub fn report(&self) -> String {
        let wall = self.wall_ns.load(Ordering::Relaxed).max(1) as f64;
        let mut out = String::new();
        for (name, v) in [
            ("decode", &self.decode_ns),
            ("compose", &self.compose_ns),
            ("readback", &self.readback_ns),
            ("convert", &self.convert_ns),
            ("encode", &self.encode_ns),
            ("audio", &self.audio_ns),
            ("audio encode", &self.audio_encode_ns),
        ] {
            let ns = v.load(Ordering::Relaxed) as f64;
            out.push_str(&format!("  {name:<13}{:>9.2} s busy  {:>6.1} % of wall\n", ns / 1e9, ns / wall * 100.0));
        }
        let w = self.frame_workers.load(Ordering::Relaxed);
        out.push_str(&format!(
            "  wall         {:>9.2} s  ({})\n",
            wall / 1e9,
            if w == 0 { "serial".to_string() } else { format!("pipelined, {w} frame workers") }
        ));
        out
    }
}

/// Where the pipelined export's streams go: the muxer in the product, a recorder in a test.
pub trait ExportSink {
    /// A frame in the encoder's format, filled by a converter on a worker thread.
    type Frame: Send;
    type Converter: ExportConverter<Frame = Self::Frame>;
    fn converter(&self) -> std::io::Result<Self::Converter>;
    fn alloc_frame(&self) -> std::io::Result<Self::Frame>;
    /// The next video frame, in frame order.
    fn write_video(&mut self, f: &Self::Frame) -> std::io::Result<()>;
    /// The next audio samples (48 kHz stereo f32 interleaved), in stream order.
    fn write_audio(&mut self, samples: &[f32]) -> std::io::Result<()>;
    /// Encoded (video, audio) bytes so far — the progress' size estimate.
    fn bytes(&self) -> (u64, u64) {
        (0, 0)
    }
}

/// RGBA → the sink's frame format, one per worker thread.
pub trait ExportConverter: Send {
    type Frame;
    fn convert(&mut self, rgba: &[u8], out: &mut Self::Frame) -> std::io::Result<()>;
}

impl ExportSink for kt_media::muxer::Muxer {
    type Frame = kt_media::muxer::VideoFrame;
    type Converter = kt_media::muxer::VideoConverter;
    fn converter(&self) -> std::io::Result<Self::Converter> {
        self.video_converter().map_err(std::io::Error::other)
    }
    fn alloc_frame(&self) -> std::io::Result<Self::Frame> {
        self.alloc_video_frame().map_err(std::io::Error::other)
    }
    fn write_video(&mut self, f: &Self::Frame) -> std::io::Result<()> {
        self.write_video_frame(f).map_err(std::io::Error::other)
    }
    fn write_audio(&mut self, samples: &[f32]) -> std::io::Result<()> {
        kt_media::muxer::Muxer::write_audio(self, samples).map_err(std::io::Error::other)
    }
    fn bytes(&self) -> (u64, u64) {
        kt_media::muxer::Muxer::bytes(self)
    }
}

impl ExportConverter for kt_media::muxer::VideoConverter {
    type Frame = kt_media::muxer::VideoFrame;
    fn convert(&mut self, rgba: &[u8], out: &mut Self::Frame) -> std::io::Result<()> {
        kt_media::muxer::VideoConverter::convert(self, rgba, out).map_err(std::io::Error::other)
    }
}

/// What the caller hears from a running pipelined export (all called from worker threads).
pub(crate) struct Hooks<'a> {
    /// The cooperative pause (checked per composed frame and per audio burst).
    pub wait_paused: &'a (dyn Fn() + Sync),
    /// Video frames written to the sink so far.
    pub video_done: &'a (dyn Fn(u64) + Sync),
    /// Audio frames written to the sink so far — counted by the writer, so a progress event
    /// carries the same numbers on every run (t955), whatever the audio thread's lead.
    pub audio_done: &'a (dyn Fn(u64) + Sync),
    /// The sink's byte counts after a write.
    pub bytes: &'a (dyn Fn((u64, u64)) + Sync),
    /// The last video frame is written; the audio may still be running.
    pub video_finished: &'a (dyn Fn() + Sync),
    pub profile: Option<&'a ExportProfile>,
}

/// How many frame workers: the CPU composite is the heavy stage, so it gets the cores but two
/// (decode and the writer), bounded by memory — each worker holds a composite's scratch, about
/// eight output frames, and the workers together stay under `WORKER_MEMORY`. The GPU path's
/// workers only convert.
fn frame_workers(cpu_compose: bool, quality: Quality) -> usize {
    const WORKER_MEMORY: usize = 1 << 30;
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    if !cpu_compose {
        return (cores / 4).clamp(2, 4);
    }
    let per_worker = quality.width as usize * quality.height as usize * 4 * 8;
    let by_memory = (WORKER_MEMORY / per_worker.max(1)).max(2);
    cores.saturating_sub(2).clamp(2, 16).min(by_memory)
}

/// The pipelined export's frame cache: five serial budgets.
const PIPE_CACHE_BYTES: usize = 5 * EXPORT_VIDEO_CACHE_BYTES;
/// What the unwritten chunks' decoded frames may hold before decode waits: a serial budget below
/// the cache — the next chunk (at most half a budget, `export_chunk_frames`) and the frames
/// older chunks share with them (a held frame is decoded once) fit in the rest.
const LOOKAHEAD_BYTES: u64 = (PIPE_CACHE_BYTES - EXPORT_VIDEO_CACHE_BYTES) as u64;

/// The writer's progress, which also gates the decoder's look-ahead.
#[derive(Default)]
struct Gate {
    written: Mutex<u64>,
    cv: Condvar,
}

impl Gate {
    fn set(&self, n: u64) {
        *self.written.lock().unwrap() = n;
        self.cv.notify_all();
    }
    fn wake(&self) {
        self.cv.notify_all();
    }
    /// Wait until `ready(frames written)`, or the export halts.
    fn wait(&self, ready: &mut dyn FnMut(u64) -> bool, halted: &dyn Fn() -> bool) {
        let mut w = self.written.lock().unwrap();
        while !ready(*w) && !halted() {
            w = self.cv.wait_timeout(w, std::time::Duration::from_millis(50)).unwrap().0;
        }
    }
}

/// A frame for a worker: compose it (CPU) or only convert the GPU's readback.
enum Job {
    Compose { t: Tick },
    Convert { rgba: Vec<u8>, complete: bool },
}

struct Done<F> {
    frame: F,
    complete: bool,
}

/// A chunk of frames decoded into the cache: frame indices `[first, end)`.
#[derive(Clone, Copy)]
struct Chunk {
    first: u64,
    end: u64,
    t0: Tick,
    t1: Tick,
}

fn cancelled() -> std::io::Error {
    std::io::Error::other("cancelled")
}

/// The pipelined export of one file into `sink` (export-throughput D2). Returns the video and the
/// audio stats, the same numbers the serial passes report.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run<S: ExportSink>(
    project: &Project,
    video_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    audio_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    backends: &ExportBackends,
    cancel: &Arc<AtomicBool>,
    hooks: &Hooks<'_>,
    sink: &mut S,
) -> std::io::Result<(ExportStats, ExportStats)> {
    let started = Instant::now();
    let prof = hooks.profile;
    let mut src: HashMap<MediaId, Box<dyn MediaSource>> = video_sources.into_iter().collect();
    // the same source set, facts and renderer as `render_raw_video_range_with`
    for (addr, state) in crate::gen_sources(project) {
        src.insert(addr, kt_media::factory::open_gen(&state));
    }
    let mut facts = crate::video::SourceFacts::default();
    for (m, s) in src.iter() {
        facts.observe(*m, s.info());
    }
    // the chunks being composed and the ones decoded ahead (see the module doc)
    let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: PIPE_CACHE_BYTES });
    let mut video = VideoRenderer::new_with_facts(project, &facts);
    video.set_stream_memo(Arc::new(kt_plugin_api::stream::StreamMemo::new(64 << 20)));
    let video = video;
    let (t_first, end) = match range {
        Some((a, b)) => (video.grid.align_floor(a), b),
        None => (Tick(0), project_range(project)),
    };
    let step = video.grid.frame_step();
    let dq_max = video.max_decode_quality(quality);
    let chunk_frames =
        export_chunk_frames(EXPORT_VIDEO_CACHE_BYTES, dq_max.width, dq_max.height, video.max_concurrent_layers());
    // the serial loop's frames and chunks, as indices
    let mut chunks = Vec::new();
    let (mut t, mut idx) = (t_first, 0u64);
    while t < end {
        let t1 = Tick((t.0 + chunk_frames * step).min(end.0));
        let first = idx;
        let mut u = t;
        while u < t1 {
            idx += 1;
            u = Tick(u.0 + step);
        }
        chunks.push(Chunk { first, end: idx, t0: t, t1 });
        t = u;
    }
    let total = idx;
    let frame_tick = |i: u64| Tick(t_first.0 + i as i64 * step);
    let audio_start = match range {
        Some((a, _)) => a.to_audio_frames(),
        None => 0,
    };

    let halt = Arc::new(AtomicBool::new(false));
    let halted = || {
        if cancel.load(Ordering::Acquire) {
            halt.store(true, Ordering::Release);
        }
        halt.load(Ordering::Acquire)
    };
    let gate = Gate::default();
    // several devices: the workers compose as the CPU's do, each on its own engine
    let engines: Vec<Arc<dyn crate::VideoGpuExec>> = if backends.video_gpu_more.is_empty() {
        Vec::new()
    } else {
        backends.video_gpu.iter().chain(&backends.video_gpu_more).cloned().collect()
    };
    let gpu = backends.video_gpu.clone().filter(|_| engines.is_empty());
    let workers = if engines.is_empty() { frame_workers(gpu.is_none(), quality) } else { engines.len() };
    if let Some(p) = prof {
        p.frame_workers.store(workers as u64, Ordering::Relaxed);
    }
    let decode_missing = AtomicU64::new(0);

    let result = std::thread::scope(|scope| {
        // --- the queues ---
        let (chunk_tx, chunk_rx) = bounded::<Chunk>(4);
        let mut job_txs = Vec::new();
        let mut done_rxs = Vec::new();
        let mut back_txs = Vec::new();
        let mut worker_parts = Vec::new();
        for _ in 0..workers {
            let (jt, jr) = bounded::<Job>(1);
            let (dt, dr) = bounded::<Done<S::Frame>>(2);
            let (bt, br) = unbounded::<S::Frame>();
            for _ in 0..2 {
                bt.send(sink.alloc_frame()?).expect("own channel");
            }
            job_txs.push(jt);
            done_rxs.push(dr);
            back_txs.push(bt);
            worker_parts.push((jr, dt, br, sink.converter()?));
        }
        let (rgba_pool_tx, rgba_pool_rx) = unbounded::<Vec<u8>>();
        let (audio_tx, audio_rx) = bounded::<Vec<f32>>(96);

        // --- audio: its own thread, the same renderer and stream as the serial pass ---
        let audio_halt = halt.clone();
        let audio = scope.spawn(move || {
            let mark = std::cell::Cell::new(Instant::now());
            let busy = prof.map(|p| &p.audio_ns);
            render_raw_audio_range(
                project,
                audio_sources,
                range,
                &audio_halt,
                |done| {
                    (hooks.wait_paused)();
                    let _ = done;
                    if cancel.load(Ordering::Acquire) {
                        audio_halt.store(true, Ordering::Release);
                    }
                    mark.set(Instant::now());
                },
                |chunk| {
                    ExportProfile::add(busy, mark.get());
                    audio_tx.send(chunk.to_vec()).map_err(|_| std::io::Error::other("the writer stopped"))
                },
            )
        });

        // --- decode: a chunk ahead, a thread per source within the chunk ---
        let video_ref = &video;
        let cache_ref = &cache;
        let gate_ref = &gate;
        let missing_ref = &decode_missing;
        let chunks_ref = &chunks;
        let decode = scope.spawn(move || -> std::io::Result<()> {
            // (end frame, bytes decoded) of the chunks decoded and not yet written
            let mut ledger: std::collections::VecDeque<(u64, u64)> = Default::default();
            for ch in chunks_ref.iter() {
                gate_ref.wait(
                    &mut |written| {
                        while ledger.front().is_some_and(|(end, _)| *end <= written) {
                            ledger.pop_front();
                        }
                        ledger.iter().map(|(_, b)| b).sum::<u64>() <= LOOKAHEAD_BYTES
                    },
                    &halted,
                );
                if halted() {
                    return Err(cancelled());
                }
                let mark = Instant::now();
                let bytes = decode_chunk(video_ref, cache_ref, &mut src, *ch, quality, missing_ref);
                ledger.push_back((ch.end, bytes));
                ExportProfile::add(prof.map(|p| &p.decode_ns), mark);
                if chunk_tx.send(*ch).is_err() {
                    break;
                }
            }
            Ok(())
        });

        // --- frame workers: compose (CPU) and convert ---
        let mut worker_handles = Vec::new();
        for (n, (jr, dt, br, mut conv)) in worker_parts.into_iter().enumerate() {
            let pool_tx = rgba_pool_tx.clone();
            let pool_rx = rgba_pool_rx.clone();
            let engine = engines.get(n).cloned();
            worker_handles.push(scope.spawn(move || -> std::io::Result<()> {
                let mut scratch = ExportScratch::default();
                while let Ok(job) = jr.recv() {
                    let (rgba, complete) = match job {
                        Job::Compose { t } => {
                            let mut buf = pool_rx.try_recv().unwrap_or_default();
                            let mark = Instant::now();
                            let complete = match &engine {
                                Some(e) => e
                                    .execute_into(&video_ref.frame_plan(cache_ref, t, quality), &mut buf)
                                    .map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?,
                                None => video_ref.compose_export_with(cache_ref, t, quality, &mut scratch, &mut buf),
                            };
                            ExportProfile::add(prof.map(|p| &p.compose_ns), mark);
                            (buf, complete)
                        }
                        Job::Convert { rgba, complete } => (rgba, complete),
                    };
                    let Ok(mut frame) = br.recv() else { break };
                    let mark = Instant::now();
                    conv.convert(&rgba, &mut frame)?;
                    ExportProfile::add(prof.map(|p| &p.convert_ns), mark);
                    let _ = pool_tx.send(rgba);
                    if dt.send(Done { frame, complete }).is_err() {
                        break;
                    }
                }
                Ok(())
            }));
        }
        drop(rgba_pool_tx);

        // --- compose: the GPU on one thread (submit ahead, collect in order), or the CPU
        //     dispatcher handing frames to the workers round robin ---
        let compose = scope.spawn(move || -> std::io::Result<()> {
            let mut pending: std::collections::VecDeque<(u64, u64)> = Default::default();
            let depth = gpu.as_ref().map_or(0, |g| g.readback_depth());
            let send = |i: u64, job: Job| job_txs[(i % workers as u64) as usize].send(job).is_ok();
            let collect = |gpu: &Arc<dyn crate::VideoGpuExec>, i: u64, ticket: u64| -> std::io::Result<bool> {
                let mut buf = rgba_pool_rx.try_recv().unwrap_or_default();
                let mark = Instant::now();
                let complete =
                    gpu.collect(ticket, &mut buf).map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                ExportProfile::add(prof.map(|p| &p.readback_ns), mark);
                Ok(send(i, Job::Convert { rgba: buf, complete }))
            };
            loop {
                // a submitted frame stays in flight across chunks while the next chunk is ready;
                // before WAITING for one, the in-flight frames leave: the decoder of the next
                // chunk may be waiting for the writer to pass them
                let ch = match chunk_rx.try_recv() {
                    Ok(c) => c,
                    Err(_) => {
                        if let Some(gpu) = &gpu {
                            while let Some((j, tk)) = pending.pop_front() {
                                if !collect(gpu, j, tk)? {
                                    return Ok(());
                                }
                            }
                        }
                        match chunk_rx.recv() {
                            Ok(c) => c,
                            Err(_) => break,
                        }
                    }
                };
                for i in ch.first..ch.end {
                    (hooks.wait_paused)();
                    if halted() {
                        return Err(cancelled());
                    }
                    let t = frame_tick(i);
                    let Some(gpu) = &gpu else {
                        if !send(i, Job::Compose { t }) {
                            return Ok(());
                        }
                        continue;
                    };
                    let mark = Instant::now();
                    let plan = video_ref.frame_plan(cache_ref, t, quality);
                    if depth == 0 {
                        let mut buf = rgba_pool_rx.try_recv().unwrap_or_default();
                        let complete = gpu
                            .execute_into(&plan, &mut buf)
                            .map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                        ExportProfile::add(prof.map(|p| &p.compose_ns), mark);
                        if !send(i, Job::Convert { rgba: buf, complete }) {
                            return Ok(());
                        }
                        continue;
                    }
                    let ticket = gpu.submit(&plan).map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                    ExportProfile::add(prof.map(|p| &p.compose_ns), mark);
                    pending.push_back((i, ticket));
                    if pending.len() >= depth {
                        let (j, tk) = pending.pop_front().unwrap();
                        if !collect(gpu, j, tk)? {
                            return Ok(());
                        }
                    }
                }
            }
            Ok(())
        });

        // --- the writer, on this thread: frames in order, the audio by the fixed rule ---
        let written = write_all(
            sink,
            total,
            done_rxs,
            back_txs,
            audio_rx,
            |i| (frame_tick(i + 1).to_audio_frames() - audio_start).max(0) as u64,
            &gate,
            &halted,
            hooks,
        );
        if written.is_err() {
            halt.store(true, Ordering::Release);
        }
        gate.wake();
        let compose = compose.join().expect("compose thread");
        let decode = decode.join().expect("decode thread");
        let mut worker_err = Ok(());
        for h in worker_handles {
            if let Err(e) = h.join().expect("frame worker") {
                worker_err = Err(e);
            }
        }
        let audio = audio.join().expect("audio thread");
        // the first cause wins: a stage's own error over the "the writer stopped" it caused
        let incomplete = written?;
        compose?;
        decode?;
        worker_err?;
        let astats = audio?;
        Ok::<_, std::io::Error>((incomplete, astats))
    });
    let (incomplete, astats) = match result {
        Ok(r) => r,
        Err(e) => {
            return Err(if cancel.load(Ordering::Acquire) { cancelled() } else { e });
        }
    };
    if let Some(p) = prof {
        p.wall_ns.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
    let vstats = ExportStats {
        video_frames: total,
        missing_video_layers: incomplete + decode_missing.load(Ordering::Relaxed),
        ..Default::default()
    };
    Ok((vstats, astats))
}

/// The writer: video frame `i` from worker `i % W`, then the audio up to the end of that frame.
/// Returns how many frames were composed incomplete.
#[allow(clippy::too_many_arguments)]
fn write_all<S: ExportSink>(
    sink: &mut S,
    total: u64,
    done_rxs: Vec<Receiver<Done<S::Frame>>>,
    back_txs: Vec<Sender<S::Frame>>,
    audio_rx: Receiver<Vec<f32>>,
    audio_until: impl Fn(u64) -> u64,
    gate: &Gate,
    halted: &dyn Fn() -> bool,
    hooks: &Hooks<'_>,
) -> std::io::Result<u64> {
    let prof = hooks.profile;
    let workers = done_rxs.len() as u64;
    let mut incomplete = 0u64;
    let mut audio_written = 0u64;
    let mut audio_open = true;
    let write_audio = |sink: &mut S, chunk: &[f32]| -> std::io::Result<()> {
        let mark = Instant::now();
        sink.write_audio(chunk)?;
        ExportProfile::add(prof.map(|p| &p.audio_encode_ns), mark);
        Ok(())
    };
    for i in 0..total {
        if halted() {
            return Err(cancelled());
        }
        let w = (i % workers) as usize;
        let Ok(done) = done_rxs[w].recv() else {
            return Err(std::io::Error::other("a frame worker stopped"));
        };
        if !done.complete {
            incomplete += 1;
        }
        let mark = Instant::now();
        sink.write_video(&done.frame)?;
        ExportProfile::add(prof.map(|p| &p.encode_ns), mark);
        let _ = back_txs[w].send(done.frame);
        gate.set(i + 1);
        (hooks.video_done)(i + 1);
        let want = audio_until(i);
        while audio_open && audio_written < want {
            match audio_rx.recv() {
                Ok(chunk) => {
                    write_audio(sink, &chunk)?;
                    audio_written += (chunk.len() / AUDIO_CHANNELS) as u64;
                    (hooks.audio_done)(audio_written);
                }
                Err(_) => audio_open = false,
            }
        }
        (hooks.bytes)(sink.bytes());
    }
    (hooks.video_finished)();
    while let Ok(chunk) = audio_rx.recv() {
        if halted() {
            return Err(cancelled());
        }
        write_audio(sink, &chunk)?;
        audio_written += (chunk.len() / AUDIO_CHANNELS) as u64;
        (hooks.audio_done)(audio_written);
    }
    (hooks.bytes)(sink.bytes());
    Ok(incomplete)
}

/// Decode one chunk's frames into the cache, a thread per source (the serial export's
/// `chunk_requests` + `decode_requests`). Each source is used by one thread. Returns the bytes
/// of the frames it asked for.
fn decode_chunk(
    video: &VideoRenderer,
    cache: &BurstCache,
    src: &mut HashMap<MediaId, Box<dyn MediaSource>>,
    ch: Chunk,
    quality: Quality,
    missing: &AtomicU64,
) -> u64 {
    let mut lost = 0u64;
    let mut jobs: HashMap<MediaId, Vec<(Tick, Quality)>> =
        chunk_requests(video, cache, (ch.t0, ch.t1), quality, |m| src.contains_key(&m), &mut lost).into_iter().collect();
    let bytes = jobs.values().flatten().map(|(_, q)| q.width as u64 * q.height as u64 * 4).sum();
    if jobs.len() <= 1 {
        for (media, keys) in jobs {
            decode_requests(src.get_mut(&media).unwrap(), media, &keys, cache, &mut lost);
        }
        missing.fetch_add(lost, Ordering::Relaxed);
        return bytes;
    }
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (media, s) in src.iter_mut() {
            if let Some(keys) = jobs.remove(media) {
                let media = *media;
                handles.push(scope.spawn(move || {
                    let mut lost = 0u64;
                    decode_requests(s, media, &keys, cache, &mut lost);
                    lost
                }));
            }
        }
        for h in handles {
            lost += h.join().expect("decode thread");
        }
    });
    missing.fetch_add(lost, Ordering::Relaxed);
    bytes
}
