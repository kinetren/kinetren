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
    /// The video encoder and the muxer (`write_video_frame`), without a device encoder's surface upload.
    pub encode_ns: AtomicU64,
    /// A device encoder's surface get and upload (export-host-copy D1).
    pub surface_upload_ns: AtomicU64,
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
    /// Frames dispatched while `n + 1` devices composed (render-honest D1): which counts the
    /// measured time chose. Empty on one device.
    pub frames_on_devices: [AtomicU64; MAX_DEVICES],
    /// The readback ring (export-readback D2): its bound, the frames in flight summed at every
    /// submission and the most at once, the submissions.
    pub ring_depth: AtomicU64,
    pub in_flight_sum: AtomicU64,
    pub in_flight_max: AtomicU64,
    pub submissions: AtomicU64,
    /// A frame's time from its submission until it was collected: summed, the longest, the frames.
    pub latency_ns: AtomicU64,
    pub latency_max_ns: AtomicU64,
    pub latency_frames: AtomicU64,
}

/// The most devices the profile counts frames for; more are counted with the last.
pub const MAX_DEVICES: usize = 8;

impl ExportProfile {
    pub(crate) fn add(slot: Option<&AtomicU64>, since: Instant) {
        if let Some(s) = slot {
            s.fetch_add(since.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
    }

    /// A frame's write since `since`, of which `upload` ns were the device encoder's surface upload:
    /// counted apart (export-host-copy D1).
    pub(crate) fn encode_apart(prof: Option<&ExportProfile>, since: Instant, upload: u64) {
        if let Some(p) = prof {
            let all = since.elapsed().as_nanos() as u64;
            p.encode_ns.fetch_add(all.saturating_sub(upload), Ordering::Relaxed);
            p.surface_upload_ns.fetch_add(upload.min(all), Ordering::Relaxed);
        }
    }

    /// The stages in the order of `kt_time::trace::export_stage`.
    fn stages(&self) -> [&AtomicU64; 16] {
        [
            &self.decode_ns,
            &self.compose_ns,
            &self.readback_ns,
            &self.convert_ns,
            &self.encode_ns,
            &self.audio_ns,
            &self.audio_encode_ns,
            &self.plan_ns,
            &self.upload_ns,
            &self.encode_passes_ns,
            &self.gpu_wait_ns,
            &self.copy_ns,
            &self.wait_lookahead_ns,
            &self.wait_workers_ns,
            &self.wait_frame_ns,
            &self.wait_audio_ns,
        ]
    }

    /// The record into the trace (render-speed D2): an `ExportStage` per stage, then the wall with the
    /// timeline nanoseconds the export covered. `kt-cli trace DIR export` reads them back.
    pub fn trace(&self, frames: u64, timeline_ns: u64) {
        use kt_time::trace::{Ev, export_stage};
        let wall = self.wall_ns.load(Ordering::Relaxed);
        let workers = self.frame_workers.load(Ordering::Relaxed);
        for (i, v) in self.stages().into_iter().enumerate() {
            kt_time::trace::ev(Ev::ExportStage)
                .id(workers)
                .aux(i as u32 + 1)
                .a(v.load(Ordering::Relaxed))
                .b(wall)
                .c(frames)
                .emit();
        }
        let submissions = self.submissions.load(Ordering::Relaxed);
        if submissions > 0 {
            kt_time::trace::ev(Ev::ExportStage)
                .id(self.in_flight_max.load(Ordering::Relaxed))
                .aux(export_stage::RING)
                .a(self.ring_depth.load(Ordering::Relaxed))
                .b(self.in_flight_sum.load(Ordering::Relaxed))
                .c(submissions)
                .emit();
            kt_time::trace::ev(Ev::ExportStage)
                .id(workers)
                .aux(export_stage::LATENCY)
                .a(self.latency_ns.load(Ordering::Relaxed))
                .b(self.latency_max_ns.load(Ordering::Relaxed))
                .c(self.latency_frames.load(Ordering::Relaxed))
                .emit();
        }
        kt_time::trace::ev(Ev::ExportStage)
            .id(workers)
            .aux(export_stage::SURFACE_UPLOAD)
            .a(self.surface_upload_ns.load(Ordering::Relaxed))
            .b(wall)
            .c(frames)
            .emit();
        kt_time::trace::ev(Ev::ExportStage).id(workers).aux(export_stage::WALL).a(wall).b(timeline_ns).c(frames).emit();
    }

    /// `; readback: up to 12 in flight of 16 (mean 9.8), a frame back 52.7 ms after its submission`
    /// — the ring of one device (export-readback D2); empty when no frame went through it.
    pub fn ring_text(&self) -> String {
        let n = self.submissions.load(Ordering::Relaxed);
        let back = self.latency_frames.load(Ordering::Relaxed);
        if n == 0 || back == 0 {
            return String::new();
        }
        format!(
            "; readback: up to {} in flight of {} (mean {:.1}), a frame back {:.1} ms after its submission",
            self.in_flight_max.load(Ordering::Relaxed),
            self.ring_depth.load(Ordering::Relaxed),
            self.in_flight_sum.load(Ordering::Relaxed) as f64 / n as f64,
            self.latency_ns.load(Ordering::Relaxed) as f64 / back as f64 / 1e6
        )
    }

    /// The record as one line for the telemetry journal (render-speed D2): the real-time factor, the
    /// output size, and every busy stage's share of the wall, the largest first (summed over a stage's
    /// threads, so a share past 100 % is several threads busy at once).
    pub fn summary(&self, frames: u64, timeline_ns: u64, (w, h): (u32, u32)) -> String {
        let wall = self.wall_ns.load(Ordering::Relaxed).max(1);
        let workers = self.frame_workers.load(Ordering::Relaxed);
        let mut busy: Vec<(&str, u64)> = self
            .stages()
            .into_iter()
            .enumerate()
            .map(|(i, v)| (kt_time::trace::export_stage::name(i as u32 + 1), v.load(Ordering::Relaxed)))
            .chain([("surface upload", self.surface_upload_ns.load(Ordering::Relaxed))])
            .filter(|(_, ns)| *ns * 100 >= wall)
            .collect();
        busy.sort_by_key(|(_, ns)| std::cmp::Reverse(*ns));
        let shares: Vec<String> = busy.iter().map(|(n, ns)| format!("{n} {:.0} %", *ns as f64 * 100.0 / wall as f64)).collect();
        format!(
            "export: {frames} frames at {w}×{h}, {:.1} s of the timeline in {:.1} s ({:.2}× real time, {}); busy: {}{}",
            timeline_ns as f64 / 1e9,
            wall as f64 / 1e9,
            timeline_ns as f64 / wall as f64,
            if workers == 0 { "serial".to_string() } else { format!("{workers} frame workers") },
            shares.join(", "),
            self.devices_text() + &self.ring_text()
        )
    }

    /// `; devices: 1 ×40, 2 ×20` — the frames each device count composed, when several devices could.
    pub fn devices_text(&self) -> String {
        let used: Vec<String> = self
            .frames_on_devices
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                let n = n.load(Ordering::Relaxed);
                (n > 0).then(|| format!("{} ×{n}", i + 1))
            })
            .collect();
        if used.is_empty() { String::new() } else { format!("; devices: {}", used.join(", ")) }
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
            ("surface up", &self.surface_upload_ns),
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
    /// A device encoder's surface upload so far, ns (export-host-copy D1): taken out of the encode.
    fn surface_upload_ns(&self) -> u64 {
        0
    }
    /// The planes the encoder takes (export-host-copy D3): the device converts a frame to them and
    /// the worker only copies them. `None` — the frames stay RGBA.
    fn yuv_layout(&self) -> Option<crate::yuv::YuvLayout> {
        None
    }
}

/// RGBA → the sink's frame format, one per worker thread.
pub trait ExportConverter: Send {
    type Frame;
    fn convert(&mut self, rgba: &[u8], out: &mut Self::Frame) -> std::io::Result<()>;
    /// The sink's `yuv_layout` planes, packed, into its frame: a copy.
    fn fill_planes(&mut self, _planes: &[u8], _out: &mut Self::Frame) -> std::io::Result<()> {
        Err(std::io::Error::other("this sink takes RGBA"))
    }
}

/// The muxer's frames by the exact conversion (export-host-copy D2): RGBA through the reference,
/// planes copied as they came from the device. swscale no longer converts an export.
pub struct MuxConverter {
    layout: crate::yuv::YuvLayout,
    w: u32,
    h: u32,
    planes: Vec<u8>,
}

impl ExportConverter for MuxConverter {
    type Frame = kt_media::muxer::VideoFrame;
    fn convert(&mut self, rgba: &[u8], out: &mut Self::Frame) -> std::io::Result<()> {
        crate::yuv::convert(&self.layout, rgba, self.w, self.h, &mut self.planes);
        out.fill_packed(&self.planes).map_err(std::io::Error::other)
    }
    fn fill_planes(&mut self, planes: &[u8], out: &mut Self::Frame) -> std::io::Result<()> {
        out.fill_packed(planes).map_err(std::io::Error::other)
    }
}

/// The exact layout of what a muxer's encoder takes.
pub fn mux_layout(m: &kt_media::muxer::Muxer) -> crate::yuv::YuvLayout {
    use crate::yuv::{YuvLayout, YuvMatrix, YuvPix};
    use kt_media::muxer::PixFmt;
    let (pix, matrix, range) = m.input_layout();
    YuvLayout {
        pix: match pix {
            PixFmt::Yuv420p => YuvPix::Yuv420p,
            PixFmt::Nv12 => YuvPix::Nv12,
            PixFmt::Yuv420p10 => YuvPix::Yuv420p10,
            PixFmt::P010 => YuvPix::P010,
            PixFmt::Yuv422p10 => YuvPix::Yuv422p10,
        },
        matrix: match matrix {
            kt_media::muxer::format::Matrix::Bt601 => YuvMatrix::Bt601,
            kt_media::muxer::format::Matrix::Bt709 => YuvMatrix::Bt709,
        },
        full: range == kt_media::muxer::format::Range::Full,
    }
}

/// A muxer's converter: the exact conversion of its layout.
pub fn mux_converter(m: &kt_media::muxer::Muxer, w: u32, h: u32) -> MuxConverter {
    MuxConverter { layout: mux_layout(m), w, h, planes: Vec::new() }
}

impl ExportSink for kt_media::muxer::Muxer {
    type Frame = kt_media::muxer::VideoFrame;
    type Converter = MuxConverter;
    fn converter(&self) -> std::io::Result<Self::Converter> {
        let (w, h) = self.video_size();
        Ok(mux_converter(self, w, h))
    }
    fn yuv_layout(&self) -> Option<crate::yuv::YuvLayout> {
        Some(mux_layout(self))
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
    fn surface_upload_ns(&self) -> u64 {
        kt_media::muxer::Muxer::surface_upload_ns(self)
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

/// How many of an export's devices compose (render-honest D1, found-defects 710). A software
/// adapter's export opens as many devices as the cores and the memory allow; this decides how many
/// of them are GIVEN frames, from the frames dispatched per second of wall, measured over windows:
/// one device first, one more while it wins by more than `GAIN`, back on a loss, the fewer on a tie,
/// and every `HOLD` windows one more and then one fewer are tried again, so a light stretch and a
/// heavy one each get their count. The numbers are the queues' depths and a margin over a window's
/// noise, not measurements of any machine. The frames do not depend on the count (t1573, t2901): the
/// clock steers only which device composes a frame (invariant 11).
pub(crate) struct DeviceRule {
    max: usize,
    k: usize,
    phase: Phase,
    script: Option<(Vec<usize>, usize)>,
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    /// The first window, at one device.
    Start,
    /// `k` is held; the window is its fresh rate. After `HOLD` windows a probe starts.
    Hold(u32),
    /// `k` is a trial from `from`, which ran at `base`; `up` — one more; `then_down` — a probe that
    /// tries one fewer after a failed one more.
    Try { from: usize, base: f64, up: bool, then_down: bool },
}

/// A step is a win only past this share of the rate before it (a window's own noise).
const GAIN: f64 = 0.10;
/// Windows a count is held before one more and one fewer are tried again.
const HOLD: u32 = 4;
/// The fewest frames a window measures.
const MIN_WINDOW: u64 = 8;

impl DeviceRule {
    pub(crate) fn new(max: usize, script: Option<Vec<usize>>) -> DeviceRule {
        let max = max.max(1);
        let script = script.filter(|s| !s.is_empty()).map(|s| (s, 0));
        let mut r = DeviceRule { max, k: 1, phase: Phase::Start, script };
        if let Some((s, _)) = &r.script {
            r.k = s[0].clamp(1, max);
        }
        r
    }

    /// The devices that compose now.
    pub(crate) fn count(&self) -> usize {
        self.k
    }

    /// (frames to let pass before the window, frames the window measures) at the current count: a
    /// fresh worker fills its job slot and its two done slots first, which is not throughput.
    pub(crate) fn window(&self) -> (u64, u64) {
        if self.script.is_some() {
            return (0, 4);
        }
        let k = self.k as u64;
        (2 * k, MIN_WINDOW.max(4 * k))
    }

    /// A window at `count()` ran at `rate` frames a second; returns the count of the next window.
    pub(crate) fn measured(&mut self, rate: f64) -> usize {
        if let Some((s, at)) = &mut self.script {
            *at = (*at + 1) % s.len();
            self.k = s[*at].clamp(1, self.max);
            return self.k;
        }
        let (k, max) = (self.k, self.max);
        let (next, phase) = match self.phase {
            Phase::Start => {
                if k < max {
                    (k + 1, Phase::Try { from: k, base: rate, up: true, then_down: false })
                } else {
                    (k, Phase::Hold(0))
                }
            }
            Phase::Hold(n) if n + 1 < HOLD => (k, Phase::Hold(n + 1)),
            Phase::Hold(_) if k < max => (k + 1, Phase::Try { from: k, base: rate, up: true, then_down: true }),
            Phase::Hold(_) if k > 1 => (k - 1, Phase::Try { from: k, base: rate, up: false, then_down: false }),
            Phase::Hold(_) => (k, Phase::Hold(0)),
            Phase::Try { up: true, base, .. } if rate > base * (1.0 + GAIN) => {
                // a win: keep climbing from here
                if k < max {
                    (k + 1, Phase::Try { from: k, base: rate, up: true, then_down: false })
                } else {
                    (k, Phase::Hold(0))
                }
            }
            Phase::Try { up: true, from, base, then_down: true } if from > 1 => {
                (from - 1, Phase::Try { from, base, up: false, then_down: false })
            }
            Phase::Try { up: true, from, .. } => (from, Phase::Hold(0)),
            // fewer devices are kept unless they lose more than the margin
            Phase::Try { up: false, base, .. } if rate * (1.0 + GAIN) >= base => (k, Phase::Hold(0)),
            Phase::Try { up: false, from, .. } => (from, Phase::Hold(0)),
        };
        self.k = next;
        self.phase = phase;
        next
    }
}

/// The compose thread's choice of worker for each frame: round robin over the devices the rule lets
/// compose, measuring the dispatch rate in windows (render-honest D1). Without a rule, round robin
/// over every worker.
struct Dispatch {
    workers: usize,
    rule: Option<DeviceRule>,
    next: usize,
    /// Frames dispatched at the current count, and when its window began.
    n: u64,
    began: Option<Instant>,
    /// The person's pause inside the window: not throughput.
    paused: std::time::Duration,
}

impl Dispatch {
    fn new(workers: usize, rule: Option<DeviceRule>) -> Dispatch {
        Dispatch { workers, rule, next: 0, n: 0, began: None, paused: Default::default() }
    }

    fn active(&self) -> usize {
        self.rule.as_ref().map_or(self.workers, |r| r.count().min(self.workers))
    }

    /// The worker of the next frame.
    fn pick(&mut self, prof: Option<&ExportProfile>) -> usize {
        let active = self.active();
        let w = self.next % active;
        self.next = (w + 1) % active;
        let Some(rule) = &mut self.rule else { return w };
        if let Some(p) = prof {
            p.frames_on_devices[(active - 1).min(MAX_DEVICES - 1)].fetch_add(1, Ordering::Relaxed);
        }
        let (settle, frames) = rule.window();
        if self.n == settle {
            self.began = Some(Instant::now());
            self.paused = Default::default();
        }
        self.n += 1;
        if self.n == settle + frames + 1
            && let Some(began) = self.began.take()
        {
            let secs = began.elapsed().saturating_sub(self.paused).as_secs_f64().max(1e-9);
            let before = rule.count();
            if rule.measured(frames as f64 / secs) != before {
                self.next = 0;
            }
            self.n = 0;
        }
        w
    }

    fn paused(&mut self, d: std::time::Duration) {
        self.paused += d;
    }
}

/// A frame for a worker: compose it or only convert the GPU's readback.
enum Job<'a> {
    Compose { t: Tick },
    /// A frame whose plan the compose thread built: a project with a stream stage, whose history
    /// advances in frame order (stream-round-trips D6, found-defects 490).
    Planned { t: Tick, plan: crate::video_plan::FramePlan<'a> },
    /// The device's readback: RGBA, or the encoder's packed planes (export-host-copy D3).
    Convert { t: Tick, rgba: Vec<u8>, planes: bool, complete: bool },
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
    // while it runs the preview's memos keep only their bases (memory-ledger D6)
    let _room = kt_time::mem::claim();
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
    video.set_stream_memo(Arc::new(kt_plugin_api::stream::StreamMemo::new(crate::export::EXPORT_STREAM_MEMO_BYTES)));
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
    let streams = video.has_stream_stages();
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
        // the planes the encoder takes, made on the composing device (export-host-copy D3)
        let layout = sink.yuv_layout();
        for _ in 0..workers {
            let (jt, jr) = bounded::<Job<'_>>(1);
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
        // the worker of each frame, in frame order, for the writer (render-honest D1)
        let (order_tx, order_rx) = unbounded::<usize>();

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
                // a range starting inside a stream's history decodes and walks it first, in bounded
                // batches, as the serial export's pre-roll does (stream-round-trips D6); later chunks
                // continue the walk the compose thread advances
                if streams && ch.first == 0 {
                    crate::export::stream_preroll(video_ref, cache_ref, &mut src, frame_tick(0), quality);
                }
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
                    let (t, rgba, planes, complete) = match job {
                        Job::Compose { t } | Job::Planned { t, .. } => {
                            let mut buf = pool_rx.try_recv().unwrap_or_default();
                            let mark = Instant::now();
                            let e = engine.as_ref().expect("only a worker with an engine composes");
                            let plan = match job {
                                Job::Planned { plan, .. } => plan,
                                _ => video_ref.frame_plan(cache_ref, t, quality),
                            };
                            ExportProfile::add(prof.map(|p| &p.plan_ns), mark);
                            // the encoder's planes made on this worker's device (export-host-copy D3)
                            let once = |plan: &crate::video_plan::FramePlan<'_>, buf: &mut Vec<u8>| match &layout {
                                Some(l) => e.execute_once_yuv(plan, l, buf),
                                None => e.execute_once(plan, buf),
                            };
                            let mut complete =
                                once(&plan, &mut buf).map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                            // the chunk holds every frame it could decode (each asked alone again):
                            // an incomplete frame is composed again before it is written (gpu-leftovers D5)
                            for _ in 0..crate::export::RETRIES {
                                if complete {
                                    break;
                                }
                                let plan = video_ref.frame_plan(cache_ref, t, quality);
                                complete =
                                    once(&plan, &mut buf).map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                            }
                            ExportProfile::add(prof.map(|p| &p.compose_ns), mark);
                            if let Some(p) = prof {
                                let s = e.last_stages();
                                p.upload_ns.fetch_add(s.upload_ns, Ordering::Relaxed);
                                p.encode_passes_ns.fetch_add(s.encode_ns, Ordering::Relaxed);
                                p.gpu_wait_ns.fetch_add(s.wait_ns, Ordering::Relaxed);
                                p.copy_ns.fetch_add(s.copy_ns, Ordering::Relaxed);
                            }
                            (t, buf, layout.is_some(), complete)
                        }
                        Job::Convert { t, rgba, planes, complete } => (t, rgba, planes, complete),
                    };
                    // asked here: the chunk still holds what it has, so what is absent never came
                    let lost = (!complete).then(|| (t, video_ref.absent_layers(cache_ref, t, quality)));
                    let Ok(mut frame) = br.recv() else { break };
                    let mark = Instant::now();
                    if planes && !rgba.is_empty() {
                        conv.fill_planes(&rgba, &mut frame)?;
                    } else {
                        conv.convert(&rgba, &mut frame)?;
                    }
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
            // (frame, ticket, submitted at — read into the profile only)
            let mut pending: std::collections::VecDeque<(u64, u64, Instant)> = Default::default();
            // several devices: as many compose as the measured time says (render-honest D1)
            let rule = (engines.len() > 1).then(|| {
                let script = match &backends.devices {
                    crate::export::DeviceCount::Script(s) => Some(s.clone()),
                    crate::export::DeviceCount::Measured => None,
                };
                DeviceRule::new(engines.len(), script)
            });
            let dispatch = std::cell::RefCell::new(Dispatch::new(workers, rule));
            let send = |_i: u64, job| {
                let w = dispatch.borrow_mut().pick(prof);
                order_tx.send(w).is_ok() && job_txs[w].send(job).is_ok()
            };
            // the frames in flight come back as the encoder's planes when the device converts
            // (export-host-copy D3); `planes` is fixed for the export by the first frame's size
            let ring_planes = std::cell::Cell::new(false);
            let once = |gpu: &Arc<dyn crate::VideoGpuExec>, plan: &crate::video_plan::FramePlan<'_>, buf: &mut Vec<u8>, planes: bool| {
                match (&layout, planes) {
                    (Some(l), true) => gpu.execute_once_yuv(plan, l, buf),
                    _ => gpu.execute_once(plan, buf),
                }
            };
            let collect = |gpu: &Arc<dyn crate::VideoGpuExec>, i: u64, ticket: u64, at: Instant| -> std::io::Result<bool> {
                let mut buf = rgba_pool_rx.try_recv().unwrap_or_default();
                let mark = Instant::now();
                let mut span = kt_time::trace::span(kt_time::trace::Ev::Job);
                span.aux(kt_time::trace::job::EXPORT_FRAME).a(i);
                let mut complete =
                    gpu.collect(ticket, &mut buf).map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                if let Some(p) = prof {
                    let s = gpu.last_stages();
                    p.gpu_wait_ns.fetch_add(s.wait_ns, Ordering::Relaxed);
                    p.copy_ns.fetch_add(s.copy_ns, Ordering::Relaxed);
                    let back = at.elapsed().as_nanos() as u64;
                    p.latency_ns.fetch_add(back, Ordering::Relaxed);
                    p.latency_max_ns.fetch_max(back, Ordering::Relaxed);
                    p.latency_frames.fetch_add(1, Ordering::Relaxed);
                }
                // composed again before it is written (gpu-leftovers D5)
                for _ in 0..crate::export::RETRIES {
                    if complete {
                        break;
                    }
                    let plan = video_ref.frame_plan(cache_ref, frame_tick(i), quality);
                    complete = once(gpu, &plan, &mut buf, ring_planes.get())
                        .map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                }
                ExportProfile::add(prof.map(|p| &p.readback_ns), mark);
                Ok(send(i, Job::Convert { t: frame_tick(i), rgba: buf, planes: ring_planes.get(), complete }))
            };
            loop {
                // a submitted frame stays in flight across chunks while the next chunk is ready;
                // before WAITING for one, the in-flight frames leave: the decoder of the next
                // chunk may be waiting for the writer to pass them
                let ch = match chunk_rx.try_recv() {
                    Ok(c) => c,
                    Err(_) => {
                        if let Some(gpu) = &gpu {
                            while let Some((j, tk, at)) = pending.pop_front() {
                                if !collect(gpu, j, tk, at)? {
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
                    let paused = Instant::now();
                    (hooks.wait_paused)();
                    dispatch.borrow_mut().paused(paused.elapsed());
                    if halted() {
                        return Err(cancelled());
                    }
                    let t = frame_tick(i);
                    let Some(gpu) = &gpu else {
                        // a stream stage's history is walked here, frame after frame, never by two
                        // workers at once over each other's positions
                        let job = if streams {
                            let mark = Instant::now();
                            let plan = video_ref.frame_plan(cache_ref, t, quality);
                            ExportProfile::add(prof.map(|p| &p.plan_ns), mark);
                            Job::Planned { t, plan }
                        } else {
                            Job::Compose { t }
                        };
                        if !send(i, job) {
                            return Ok(());
                        }
                        continue;
                    };
                    let mark = Instant::now();
                    let plan = video_ref.frame_plan(cache_ref, t, quality);
                    // the ring's bound for this size (export-readback D1), by the planes' bytes when the
                    // device converts (export-host-copy D3); 0 — no ring
                    let yuv_depth = layout.as_ref().map_or(0, |l| gpu.readback_depth_yuv(plan.out_w, plan.out_h, l));
                    let (depth, planes) =
                        if yuv_depth > 0 { (yuv_depth, true) } else { (gpu.readback_depth(plan.out_w, plan.out_h), false) };
                    if depth == 0 {
                        let mut buf = rgba_pool_rx.try_recv().unwrap_or_default();
                        let planes = layout.is_some();
                        let mut complete =
                            once(gpu, &plan, &mut buf, planes).map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                        for _ in 0..crate::export::RETRIES {
                            if complete {
                                break;
                            }
                            let plan = video_ref.frame_plan(cache_ref, t, quality);
                            complete = once(gpu, &plan, &mut buf, planes)
                                .map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                        }
                        ExportProfile::add(prof.map(|p| &p.compose_ns), mark);
                        if !send(i, Job::Convert { t, rgba: buf, planes, complete }) {
                            return Ok(());
                        }
                        continue;
                    }
                    ExportProfile::add(prof.map(|p| &p.plan_ns), mark);
                    // a frame in flight comes back in the form it was submitted in
                    if planes != ring_planes.get() {
                        while let Some((j, tk, at)) = pending.pop_front() {
                            if !collect(gpu, j, tk, at)? {
                                return Ok(());
                            }
                        }
                        ring_planes.set(planes);
                    }
                    let ticket = match (&layout, planes) {
                        (Some(l), true) => gpu.submit_yuv(&plan, l),
                        _ => gpu.submit(&plan),
                    }
                    .map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?;
                    ExportProfile::add(prof.map(|p| &p.compose_ns), mark);
                    if let Some(p) = prof {
                        let s = gpu.last_stages();
                        p.upload_ns.fetch_add(s.upload_ns, Ordering::Relaxed);
                        p.encode_passes_ns.fetch_add(s.encode_ns, Ordering::Relaxed);
                    }
                    pending.push_back((i, ticket, Instant::now()));
                    if let Some(p) = prof {
                        p.ring_depth.fetch_max(depth as u64, Ordering::Relaxed);
                        p.in_flight_sum.fetch_add(pending.len() as u64, Ordering::Relaxed);
                        p.in_flight_max.fetch_max(pending.len() as u64, Ordering::Relaxed);
                        p.submissions.fetch_add(1, Ordering::Relaxed);
                    }
                    // every frame the device finished goes on now; a full ring waits for its oldest
                    // (export-readback D1): the frames in flight follow the device's latency
                    while let Some(&(j, tk, at)) = pending.front() {
                        if pending.len() < depth && !gpu.done(tk) {
                            break;
                        }
                        pending.pop_front();
                        if !collect(gpu, j, tk, at)? {
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
            order_rx,
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

/// The writer: video frame `i` from the worker the compose thread named for it (in frame order), then
/// the audio up to the end of that frame.
/// Returns how many layers frames were composed without.
#[allow(clippy::too_many_arguments)]
fn write_all<S: ExportSink>(
    sink: &mut S,
    total: u64,
    done_rxs: Vec<Receiver<Done<S::Frame>>>,
    back_txs: Vec<Sender<S::Frame>>,
    order_rx: Receiver<usize>,
    audio_rx: Receiver<Vec<f32>>,
    audio_until: impl Fn(u64) -> u64,
    gate: &Gate,
    halted: &dyn Fn() -> bool,
    hooks: &Hooks<'_>,
) -> std::io::Result<u64> {
    let prof = hooks.profile;
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
        let waited = Instant::now();
        let Some((w, Ok(done))) = order_rx.recv().ok().map(|w| (w, done_rxs[w].recv())) else {
            return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "a frame worker stopped"));
        };
        ExportProfile::add(prof.map(|p| &p.wait_frame_ns), waited);
        if let Some((t, media)) = &done.lost {
            incomplete += media.len().max(1) as u64;
            (hooks.incomplete)(*t, media);
        }
        let mark = Instant::now();
        let up = sink.surface_upload_ns();
        sink.write_video(&done.frame)?;
        ExportProfile::encode_apart(prof, mark, sink.surface_upload_ns() - up);
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

    /// Feeds `windows` windows to the rule, each at the rate `rate(count)`; returns the counts asked.
    fn drive(rule: &mut DeviceRule, windows: usize, rate: impl Fn(usize) -> f64) -> Vec<usize> {
        (0..windows)
            .map(|_| {
                let k = rule.count();
                assert!((1..=4).contains(&k), "a count outside 1..=4: {k}");
                rule.measured(rate(k))
            })
            .collect()
    }

    /// t2900 (render-honest D1, found-defects 710): the device count follows the measured time — it
    /// climbs while one more device wins by more than a tenth, keeps the fewer on a tie, comes down
    /// when a stretch turns even and goes back up when more devices win again; never none, never
    /// more than it has.
    #[test]
    fn t2900_the_count_follows_the_measured_time() {
        let mut r = DeviceRule::new(4, None);
        assert_eq!(r.count(), 1, "the first window runs on one device");
        // a heavy stretch: three devices win, the fourth adds little
        let heavy = |k: usize| [10.0, 15.0, 20.0, 21.0][k - 1];
        let seen = drive(&mut r, 30, heavy);
        assert_eq!(&seen[..3], &[2, 3, 4], "it climbs while a step wins: {seen:?}");
        assert_eq!(r.count(), 3, "and stays at three: {seen:?}");
        assert!(seen[4..].iter().filter(|&&k| k == 3).count() >= 18, "the probes are short: {seen:?}");
        // an even stretch: every count runs at the same rate — it comes down to one
        let seen = drive(&mut r, 30, |_| 10.0);
        assert_eq!(r.count(), 1, "an even stretch keeps the fewest: {seen:?}");
        // two win again
        let seen = drive(&mut r, 30, |k| [10.0, 15.0, 15.0, 15.0][k - 1]);
        assert_eq!(r.count(), 2, "more devices come back when they win: {seen:?}");
        // a rise under a tenth adds no device
        let mut r = DeviceRule::new(4, None);
        let seen = drive(&mut r, 20, |k| 10.0 + 0.5 * (k - 1) as f64);
        assert_eq!(r.count(), 1, "a gain under a tenth is a tie: {seen:?}");
        // one device has nothing to choose
        let mut r = DeviceRule::new(1, None);
        assert!(drive(&mut r, 10, |_| 10.0).iter().all(|&k| k == 1));
        // a script ignores the rate and stays within what there is
        let mut r = DeviceRule::new(3, Some(vec![1, 5, 2]));
        assert_eq!((r.count(), r.window()), (1, (0, 4)));
        assert_eq!(drive(&mut r, 4, |_| 1.0), vec![3, 2, 1, 3]);
    }

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
