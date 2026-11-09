//! The pipelined export (export-throughput D2): the stages of ONE output file run concurrently,
//! joined by bounded, order-preserving queues.
//!
//! ```text
//! decode (a chunk ahead, a thread per source) ─► compose (one device | a device per worker)
//!     ─► convert (frame workers, round robin) ─► writer (encoder + muxer, frame order)
//! audio (its own thread) ─────────────────────────────────────────► writer (fixed interleave)
//! ```
//!
//! Every frame is the same function of (cache, t, quality) the serial export computes
//! (`render_raw_video_range_with`), the writer takes them in frame order, and the audio is the
//! same renderer's stream; so the encoder receives exactly the serial export's frames and samples
//! (t1451). What changes is only WHEN each is computed:
//! - every chunk not yet written HOLDS its frames in the cache (export-pipeline-determinism), and
//!   decode starts a chunk only while the bytes held stay under the look-ahead: a frame a chunk
//!   needs is never evicted, whatever the stages' relative speed. LRU cannot promise that: the
//!   composite's reads refresh frames never needed again, and they outlived frames decoded and not
//!   yet read. The look-ahead is counted in bytes, not chunks: at 4K decode sizes a chunk is one
//!   frame, and a look-ahead of two chunks kept two of sixteen CPU workers busy;
//! - the writer interleaves the audio by a fixed rule (after video frame `i`, the audio up to that
//!   frame's end), so the call order into the muxer — and the file — does not depend on timing.
//!
//! Cancellation and errors set one `halt` flag every stage checks per frame or burst; a stage that
//! stops drops its queue ends, and every neighbour blocked on them returns (t1452).

use crate::export::{
    ExportBackends, ExportStats, chunk_requests, decode_requests, project_range, render_raw_audio_range,
};
use crate::video::VideoRenderer;
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use kt_cache::{BurstCache, BurstKey, BurstKind, CacheConfig};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::{AUDIO_CHANNELS, Tick};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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
    /// Inside `compose_ns`: building the plan, and the executor's own stages (a device per worker).
    pub plan_ns: AtomicU64,
    pub upload_ns: AtomicU64,
    pub encode_passes_ns: AtomicU64,
    pub gpu_wait_ns: AtomicU64,
    pub copy_ns: AtomicU64,
    /// Waits (render-speed-no-card): the decoder held back by the look-ahead, the frame workers
    /// with no frame to take, and the writer waiting for a frame and for the audio.
    pub wait_lookahead_ns: AtomicU64,
    pub wait_workers_ns: AtomicU64,
    pub wait_frame_ns: AtomicU64,
    pub wait_audio_ns: AtomicU64,
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
        out.push_str("  of compose:\n");
        for (name, v) in [
            ("plan", &self.plan_ns),
            ("upload", &self.upload_ns),
            ("passes", &self.encode_passes_ns),
            ("gpu wait", &self.gpu_wait_ns),
            ("copy out", &self.copy_ns),
        ] {
            let ns = v.load(Ordering::Relaxed) as f64;
            out.push_str(&format!("    {name:<11}{:>9.2} s\n", ns / 1e9));
        }
        out.push_str("  waits:\n");
        for (name, v) in [
            ("look-ahead", &self.wait_lookahead_ns),
            ("no frame", &self.wait_workers_ns),
            ("writer/frame", &self.wait_frame_ns),
            ("writer/audio", &self.wait_audio_ns),
        ] {
            let ns = v.load(Ordering::Relaxed) as f64;
            out.push_str(&format!("    {name:<13}{:>7.2} s\n", ns / 1e9));
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
    /// The serial export's budget, which the cache and the look-ahead are counted from
    /// (`EXPORT_VIDEO_CACHE_BYTES`). A test scales it with its frame size: the pressure is the
    /// ratio of the two.
    pub budget: usize,
    /// A frame came out without a layer: its time and the media that lacked, in frame order.
    pub incomplete: &'a (dyn Fn(Tick, &[MediaId]) + Sync),
}

/// How many frame workers convert on a single device.
fn frame_workers() -> usize {
    (std::thread::available_parallelism().map_or(4, |n| n.get()) / 4).clamp(2, 4)
}

/// The pipelined export's frame cache: three serial budgets. Five were never reached while a chunk
/// was one frame, because the chunk queue stopped decode first; with chunks of several frames they
/// were, and the 3 GB render died of it (render-speed-no-card D5).
fn pipe_cache_bytes(budget: usize) -> usize {
    3 * budget
}
/// A chunk's bytes at most, unless its one frame asks more.
fn chunk_bytes(budget: usize) -> usize {
    budget / 2
}
/// A chunk's frames at most, the serial loop's limit.
const MAX_CHUNK_FRAMES: u64 = 120;
/// What the unwritten chunks may hold before decode waits: a serial budget below the cache, so
/// the next chunk (at most half a budget, `chunk_bytes`) fits in the rest.
fn lookahead_bytes(budget: usize) -> u64 {
    (pipe_cache_bytes(budget) - budget) as u64
}

/// The writer's progress, which also gates the decoder's look-ahead. It waits on the scheduler's
/// gate (gpu-arbiter, row 271): the writer's progress and a halt notify it, and so does the person's
/// cancel (the render window cancels a token, which notifies the gate) — no timer. A cancel flag set
/// with no notify is seen at the writer's next frame.
#[derive(Default)]
struct Gate {
    written: Mutex<u64>,
    /// Bumped by every `set` and `wake`: what a waiter has not seen yet.
    moved: std::sync::atomic::AtomicU64,
}

impl Gate {
    fn set(&self, n: u64) {
        *self.written.lock().unwrap() = n;
        self.wake();
    }
    fn wake(&self) {
        self.moved.fetch_add(1, Ordering::AcqRel);
        kt_time::sched::notify_gate();
    }
    /// Wait until `ready(frames written)`, or the export halts.
    fn wait(&self, ready: &mut dyn FnMut(u64) -> bool, halted: &dyn Fn() -> bool) {
        loop {
            let seen = self.moved.load(Ordering::Acquire);
            if ready(*self.written.lock().unwrap()) || halted() {
                return;
            }
            kt_time::sched::wait_gate(|| self.moved.load(Ordering::Acquire) == seen && !halted());
        }
    }
}

/// A frame for a worker: compose it or only convert the GPU's readback.
enum Job {
    Compose { t: Tick },
    Convert { t: Tick, rgba: Vec<u8>, complete: bool },
}

struct Done<F> {
    frame: F,
    /// The frame came out without a layer: its time and the media that lacked.
    lost: Option<(Tick, Vec<MediaId>)>,
}

/// A chunk of frames decoded into the cache: frame indices `[first, end)`.
#[derive(Clone, Copy)]
struct Chunk {
    first: u64,
    end: u64,
    t0: Tick,
    t1: Tick,
}

/// The chunks of a range, as frame indices: frames in a row up to `limit` bytes of what they really
/// ask. The largest decode size of the project, the serial loop's measure, made a chunk of one frame
/// on the showcase: the sources advanced in step and no span had two pictures to convert at once.
fn pipe_chunks(video: &VideoRenderer, (start, end): (Tick, Tick), quality: Quality, limit: usize) -> Vec<Chunk> {
    let step = video.grid.frame_step();
    let mut chunks = Vec::new();
    let (mut t, mut idx) = (start, 0u64);
    while t < end {
        let (first, mut bytes, mut u) = (idx, 0usize, t);
        while u < end {
            let next = Tick((u.0 + step).min(end.0));
            let asked: usize = video
                .wanted_frames(kt_time::TickRange::new(u, next), quality)
                .iter()
                .map(|(_, _, dq)| dq.width as usize * dq.height as usize * 4)
                .sum();
            if idx > first && (bytes + asked > limit || idx - first >= MAX_CHUNK_FRAMES) {
                break;
            }
            bytes += asked;
            idx += 1;
            u = Tick(u.0 + step);
        }
        chunks.push(Chunk { first, end: idx, t0: t, t1: Tick(u.0.min(end.0)) });
        t = u;
    }
    chunks
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
    let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: pipe_cache_bytes(hooks.budget) });
    let mut video = VideoRenderer::new_with_facts(project, &facts);
    video.set_stream_memo(Arc::new(kt_plugin_api::stream::StreamMemo::new(64 << 20)));
    let video = video;
    let (t_first, end) = match range {
        Some((a, b)) => (video.grid.align_floor(a), b),
        None => (Tick(0), project_range(project)),
    };
    let step = video.grid.frame_step();
    let chunks = pipe_chunks(&video, (t_first, end), quality, chunk_bytes(hooks.budget));
    let total = chunks.last().map_or(0, |c| c.end);
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
    // several devices: each worker composes on its own engine
    let engines: Vec<Arc<dyn crate::VideoGpuExec>> = if backends.video_gpu_more.is_empty() {
        Vec::new()
    } else {
        backends.video_gpu.iter().chain(&backends.video_gpu_more).cloned().collect()
    };
    let gpu = backends.video_gpu.clone().filter(|_| engines.is_empty());
    if gpu.is_none() && engines.is_empty() {
        return Err(std::io::Error::other("no video device"));
    }
    let workers = if engines.is_empty() { frame_workers() } else { engines.len() };
    if let Some(p) = prof {
        p.frame_workers.store(workers as u64, Ordering::Relaxed);
    }

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
            let _class = kt_time::sched::with_class(kt_time::sched::Class::Offline);
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
        let chunks_ref = &chunks;
        let decode = scope.spawn(move || -> std::io::Result<()> {
            let _class = kt_time::sched::with_class(kt_time::sched::Class::Offline);
            let lookahead = lookahead_bytes(hooks.budget) as usize;
            // (end frame, keys held) of the chunks decoded and not yet written
            let mut ledger: std::collections::VecDeque<(u64, Vec<BurstKey>)> = Default::default();
            for ch in chunks_ref.iter() {
                let waited = Instant::now();
                gate_ref.wait(
                    &mut |written| {
                        while ledger.front().is_some_and(|(end, _)| *end <= written) {
                            let (_, held) = ledger.pop_front().expect("just seen");
                            cache_ref.release_video(held);
                        }
                        cache_ref.held_video_bytes() <= lookahead
                    },
                    &halted,
                );
                ExportProfile::add(prof.map(|p| &p.wait_lookahead_ns), waited);
                if halted() {
                    return Err(cancelled());
                }
                let mark = Instant::now();
                let held = decode_chunk(video_ref, cache_ref, &mut src, *ch, quality);
                ledger.push_back((ch.end, held));
                ExportProfile::add(prof.map(|p| &p.decode_ns), mark);
                if chunk_tx.send(*ch).is_err() {
                    break;
                }
            }
            Ok(())
        });

        // --- frame workers: compose (a device per worker) and convert ---
        let mut worker_handles = Vec::new();
        for (n, (jr, dt, br, mut conv)) in worker_parts.into_iter().enumerate() {
            let pool_tx = rgba_pool_tx.clone();
            let pool_rx = rgba_pool_rx.clone();
            let engine = engines.get(n).cloned();
            worker_handles.push(scope.spawn(move || -> std::io::Result<()> {
            let _class = kt_time::sched::with_class(kt_time::sched::Class::Offline);
                loop {
                    let idle = Instant::now();
                    let Ok(job) = jr.recv() else { break };
                    ExportProfile::add(prof.map(|p| &p.wait_workers_ns), idle);
                    let (t, rgba, complete) = match job {
                        Job::Compose { t } => {
                            let mut buf = pool_rx.try_recv().unwrap_or_default();
                            let mark = Instant::now();
                            let e = engine.as_ref().expect("only a worker with an engine composes");
                            let plan = video_ref.frame_plan(cache_ref, t, quality);
                            ExportProfile::add(prof.map(|p| &p.plan_ns), mark);
                            let complete = e
                                .execute_once(&plan, &mut buf)
                                .map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                            ExportProfile::add(prof.map(|p| &p.compose_ns), mark);
                            if let Some(p) = prof {
                                let s = e.last_stages();
                                p.upload_ns.fetch_add(s.upload_ns, Ordering::Relaxed);
                                p.encode_passes_ns.fetch_add(s.encode_ns, Ordering::Relaxed);
                                p.gpu_wait_ns.fetch_add(s.wait_ns, Ordering::Relaxed);
                                p.copy_ns.fetch_add(s.copy_ns, Ordering::Relaxed);
                            }
                            (t, buf, complete)
                        }
                        Job::Convert { t, rgba, complete } => (t, rgba, complete),
                    };
                    // asked here: the chunk still holds what it has, so what is absent never came
                    let lost = (!complete).then(|| (t, video_ref.absent_layers(cache_ref, t, quality)));
                    let Ok(mut frame) = br.recv() else { break };
                    let mark = Instant::now();
                    conv.convert(&rgba, &mut frame)?;
                    ExportProfile::add(prof.map(|p| &p.convert_ns), mark);
                    let _ = pool_tx.send(rgba);
                    if dt.send(Done { frame, lost }).is_err() {
                        break;
                    }
                }
                Ok(())
            }));
        }
        drop(rgba_pool_tx);

        // --- compose: one device on one thread (submit ahead, collect in order), or the
        //     dispatcher handing frames to the workers' devices round robin ---
        let compose = scope.spawn(move || -> std::io::Result<()> {
            let _class = kt_time::sched::with_class(kt_time::sched::Class::Offline);
            let mut pending: std::collections::VecDeque<(u64, u64)> = Default::default();
            let depth = gpu.as_ref().map_or(0, |g| g.readback_depth());
            let send = |i: u64, job: Job| job_txs[(i % workers as u64) as usize].send(job).is_ok();
            let collect = |gpu: &Arc<dyn crate::VideoGpuExec>, i: u64, ticket: u64| -> std::io::Result<bool> {
                let mut buf = rgba_pool_rx.try_recv().unwrap_or_default();
                let mark = Instant::now();
                let mut span = kt_time::trace::span(kt_time::trace::Ev::Job);
                span.aux(kt_time::trace::job::EXPORT_FRAME).a(i);
                let complete =
                    gpu.collect(ticket, &mut buf).map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                ExportProfile::add(prof.map(|p| &p.readback_ns), mark);
                Ok(send(i, Job::Convert { t: frame_tick(i), rgba: buf, complete }))
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
                            .execute_once(&plan, &mut buf)
                            .map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                        ExportProfile::add(prof.map(|p| &p.compose_ns), mark);
                        if !send(i, Job::Convert { t, rgba: buf, complete }) {
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
        // the first cause wins: a stage's own error over the "stopped" it caused on either side
        let derived = written.as_ref().is_err_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe);
        let stages = compose.and(decode).and(worker_err);
        let incomplete = match (written, stages) {
            (Ok(n), Ok(())) => n,
            (Err(_), Err(s)) if derived => return Err(s),
            (Err(e), _) | (Ok(_), Err(e)) => return Err(e),
        };
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
        missing_video_layers: incomplete,
        ..Default::default()
    };
    Ok((vstats, astats))
}

/// The writer: video frame `i` from worker `i % W`, then the audio up to the end of that frame.
/// Returns how many layers frames were composed without.
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
        let waited = Instant::now();
        let Ok(done) = done_rxs[w].recv() else {
            return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "a frame worker stopped"));
        };
        ExportProfile::add(prof.map(|p| &p.wait_frame_ns), waited);
        if let Some((t, media)) = &done.lost {
            incomplete += media.len().max(1) as u64;
            (hooks.incomplete)(*t, media);
        }
        let mark = Instant::now();
        sink.write_video(&done.frame)?;
        ExportProfile::add(prof.map(|p| &p.encode_ns), mark);
        let _ = back_txs[w].send(done.frame);
        gate.set(i + 1);
        (hooks.video_done)(i + 1);
        let want = audio_until(i);
        while audio_open && audio_written < want {
            let waited = Instant::now();
            let next = audio_rx.recv();
            ExportProfile::add(prof.map(|p| &p.wait_audio_ns), waited);
            match next {
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
/// `chunk_requests` + `decode_requests`), and HOLD every frame it names: the ones found there at the
/// moment they are asked for, the decoded ones as they are inserted. Each source is used by one
/// thread. Returns the keys held, each to be released once.
fn decode_chunk(
    video: &VideoRenderer,
    cache: &BurstCache,
    src: &mut HashMap<MediaId, Box<dyn MediaSource>>,
    ch: Chunk,
    quality: Quality,
) -> Vec<BurstKey> {
    let mut held = Vec::new();
    let mut jobs: HashMap<MediaId, Vec<(Tick, Quality)>> =
        chunk_requests(video, (ch.t0, ch.t1), quality, |m| src.contains_key(&m), |sid, dq, idx| {
            let found = cache.hold_video(sid, Some(dq), idx);
            if found {
                held.push(BurstKey { source: sid, kind: BurstKind::video(Some(dq)), idx });
            }
            found
        })
        .into_iter()
        .collect();
    if jobs.len() <= 1 {
        for (media, keys) in jobs {
            decode_requests(src.get_mut(&media).unwrap(), media, &keys, cache, Some(&mut held));
        }
        return held;
    }
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (media, s) in src.iter_mut() {
            if let Some(keys) = jobs.remove(media) {
                let media = *media;
                handles.push(scope.spawn(move || {
            let _class = kt_time::sched::with_class(kt_time::sched::Class::Offline);
                    let mut held = Vec::new();
                    decode_requests(s, media, &keys, cache, Some(&mut held));
                    held
                }));
            }
        }
        for h in handles {
            held.extend(h.join().expect("decode thread"));
        }
    });
    held
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t1767 (render-speed-no-card D5): a chunk is counted by what its frames ask. The demo has a
    /// stretch of many layers and stretches of few, so the measure of the worst moment gives a
    /// chunk of one frame where the bytes hold several; every frame still lands in one chunk, and
    /// no chunk of several frames passes the limit.
    #[test]
    fn t1767_a_chunk_is_counted_by_what_its_frames_ask() {
        kt_plugins_builtin::install();
        let p = kt_model::demo::demo_project(kt_model::demo::DemoSources::Synth);
        let video = VideoRenderer::new(&p);
        let q = Quality { width: 320, height: 180 };
        let frame = 320 * 180 * 4;
        // the budget of one frame of the worst moment
        let limit = (video.max_concurrent_layers() + 1) * frame;
        let worst = crate::export::export_chunk_frames(2 * limit, 320, 180, video.max_concurrent_layers() + 1);
        assert_eq!(worst, 1, "the serial measure gives one frame here");
        let end = project_range(&p);
        let chunks = pipe_chunks(&video, (Tick(0), end), q, limit);
        let mut next = 0;
        let mut at = Tick(0);
        for c in &chunks {
            assert_eq!((c.first, c.t0), (next, at), "the chunks follow each other");
            next = c.end;
            at = c.t1;
            if c.end - c.first > 1 {
                let asked: usize = video
                    .wanted_frames(kt_time::TickRange::new(c.t0, c.t1), q)
                    .iter()
                    .map(|(_, _, dq)| dq.width as usize * dq.height as usize * 4)
                    .sum();
                assert!(asked <= limit, "a chunk of several frames passed the limit");
            }
        }
        assert_eq!(at, end);
        assert_eq!(next, crate::export::video_frames_in(p.settings.fps, Tick(0), end));
        assert!(chunks.iter().any(|c| c.end - c.first > 1), "every chunk is one frame: {} chunks", chunks.len());
    }
}
