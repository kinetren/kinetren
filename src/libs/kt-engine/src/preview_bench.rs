//! Playing a range the way the program plays it, without a window (preview-throughput): the real
//! `Player` (workers, prefetch, audio render) against an output clock made of the wall clock, and
//! a display loop doing what `draw_preview` does — the same ratchet (`preview_key_step`), the same
//! "shown" and "skipped" counts the performance panel prints as V-fps and V-skipped.
//!
//! `kt-cli preview` prints it on real footage; the tests run it on sources with a set decode time.
//! The wall clock is legal here: this is the preview's path, not an offline one (invariant 11).

use crate::player::{Player, varispeed_fill};
use crate::video_plan::{GpuStages, PresentOptions};
use kt_time::{AUDIO_SAMPLE_RATE, Tick};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// What to play and how the imaginary machine behaves.
#[derive(Debug, Clone, Copy)]
pub struct BenchOptions {
    pub from: Tick,
    pub to: Tick,
    /// The output device's callback, in frames (PipeWire: 1024–2048).
    pub quantum: usize,
    /// The display's refresh.
    pub display_hz: u32,
    /// What the rest of a UI frame costs after the preview: layout, painting.
    pub ui_ns: u64,
    /// Warm the window before the clock starts, as a person who pressed Play on a paused frame.
    pub settle: Duration,
    /// Do not wait for the device between UI frames: the queue then backs up into the next
    /// submit, as it does in a real window on a software rasterizer.
    pub free_running: bool,
    /// Keep a hash of every frame shown from the bytes path, for a test to compare.
    pub hash_frames: bool,
}

impl Default for BenchOptions {
    fn default() -> Self {
        BenchOptions {
            from: Tick(0),
            to: Tick(0),
            quantum: 2048,
            display_hz: 60,
            ui_ns: 0,
            settle: Duration::from_millis(1500),
            free_running: false,
            hash_frames: false,
        }
    }
}

/// The layers a composite did not find: the media and the decode size.
pub type MissingLayers = Vec<(kt_model::MediaId, (u32, u32))>;

/// The result, in the performance panel's terms.
#[derive(Debug, Default, Clone)]
pub struct BenchReport {
    pub wall_s: f64,
    /// Frames of the range on the project grid.
    pub grid_frames: u64,
    /// NEW frames on screen (the panel's V-fps = shown / wall_s).
    pub shown: u64,
    /// Grid cells jumped over (the panel's V-skipped).
    pub skipped: u64,
    /// Composites that came out incomplete and were not shown.
    pub incomplete: u64,
    /// How long the UI thread stood in the preview per UI frame.
    pub ui_stall_ms: Percentiles,
    pub compose_ms: Percentiles,
    /// Why composites came out incomplete: (moment, the media whose frame was missing).
    pub starved: Vec<(Tick, MissingLayers)>,
    /// (moment, hash of its bytes), under `hash_frames`.
    pub frames: Vec<(Tick, u64)>,
    /// When audio went missing: (moment, frames of underrun since the last UI frame).
    pub underrun_at: Vec<(Tick, u64)>,
    /// The moment of the slowest composite.
    pub worst_at: Tick,
    pub composites: u64,
    pub composed_on_gpu: u64,
    /// Sums over all composites.
    pub plan_ns: u64,
    /// Waiting for the device at the end of the UI frames, as a swapchain would.
    pub device_ns: u64,
    pub stages: GpuStages,
    pub underruns: u64,
    pub video_hits: u64,
    pub video_misses: u64,
    pub unused_evictions: u64,
    pub abandoned_evictions: u64,
    pub video_decodes: u64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Percentiles {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

impl Percentiles {
    pub fn of(mut v: Vec<f64>) -> Percentiles {
        if v.is_empty() {
            return Percentiles::default();
        }
        v.sort_by(f64::total_cmp);
        let at = |f: f64| v[((v.len() - 1) as f64 * f).round() as usize];
        Percentiles { p50: at(0.5), p95: at(0.95), max: at(1.0) }
    }
}

/// The output device: takes `quantum` frames from the ring every `quantum` frames of wall time,
/// through the callback's own kernel, so starvation is silence that does not stop time.
fn output_clock(
    player_shared: Arc<crate::player::AudioShared>,
    transport: Arc<crate::Transport>,
    mut ring: rtrb::Consumer<f32>,
    quantum: usize,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let period = Duration::from_secs_f64(quantum as f64 / AUDIO_SAMPLE_RATE as f64);
        let mut next_at = Instant::now();
        let (mut prev, mut cur, mut primed, mut phase) = ((0.0, 0.0), (0.0, 0.0), false, 0.0);
        while !stop.load(Ordering::Acquire) {
            next_at += period;
            if let Some(d) = next_at.checked_duration_since(Instant::now()) {
                std::thread::sleep(d);
            }
            if player_shared.flush.swap(false, Ordering::AcqRel) {
                while ring.pop().is_ok() {}
                primed = false;
            }
            if !transport.is_playing() || (!primed && ring.slots() < 2) {
                continue;
            }
            let mut next = || -> Option<(f32, f32)> {
                if ring.slots() < 2 {
                    player_shared.callback_underruns.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
                match (ring.pop(), ring.pop()) {
                    (Ok(l), Ok(r)) => Some((l, r)),
                    _ => None,
                }
            };
            let consumed = varispeed_fill(
                &mut next,
                1.0,
                quantum,
                &mut prev,
                &mut cur,
                &mut primed,
                &mut phase,
                |_, _| {},
            );
            player_shared.consumed_frames.fetch_add(consumed, Ordering::AcqRel);
        }
    })
}

/// Play `[from, to)` once. The player is left paused.
pub fn play_range(player: &Player, ring: rtrb::Consumer<f32>, o: BenchOptions) -> BenchReport {
    let stop = Arc::new(AtomicBool::new(false));
    let clock = output_clock(
        player.shared.clone(),
        player.transport.clone(),
        ring,
        o.quantum,
        stop.clone(),
    );
    let grid = player.video_grid();
    player.seek(o.from);
    // the paused warm-up a person gives it by standing on the frame before pressing Play
    let mut buf = Vec::new();
    let settled = Instant::now() + o.settle;
    while Instant::now() < settled {
        let (_, c) = player.compose_preview_into(o.from, &mut buf);
        if c {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(settled.saturating_duration_since(Instant::now()));

    let cache0 = player.cache.stats();
    let dec0 = player.decode_stats.video_decodes.load(Ordering::Relaxed);
    let under0 = player.underruns.load(Ordering::Relaxed);
    let mut r = BenchReport {
        grid_frames: ((o.to.0 - o.from.0) / grid.frame_step().max(1)) as u64,
        ..BenchReport::default()
    };
    let (mut stalls, mut composes) = (Vec::new(), Vec::new());
    let frame = Duration::from_secs_f64(1.0 / o.display_hz.max(1) as f64);
    let started = Instant::now();
    player.play();
    let mut key: Option<(i64, bool)> = None;
    let mut gen_lap = (player.transport.generation(), 0u64);
    let mut next_vsync = Instant::now();
    let mut seen_under = under0;
    loop {
        next_vsync += frame;
        let ui0 = Instant::now();
        let (t, lap) = player.video_playhead_lap();
        if t >= o.to {
            break;
        }
        let raw = grid.align_floor(t).0;
        let tgen = (player.transport.generation(), lap);
        let reanchor = tgen != gen_lap;
        gen_lap = tgen;
        let (k, skipped, _) =
            crate::player::preview_key_step(key.map(|k| k.0), raw, grid.frame_step(), 1, reanchor);
        r.skipped += skipped;
        let need = match key {
            Some((pk, was_complete)) => pk != k || !was_complete,
            None => true,
        };
        if need {
            let new_frame = key.map(|x| x.0) != Some(k);
            // the program's choice: on the display target while the executor has one
            let shown_yet = key.is_some();
            let present = PresentOptions { show_incomplete: !shown_yet, glow: false };
            let c = match player.present_preview(Tick(k), present) {
                Some((_, c)) => c,
                None => player.compose_preview_into(Tick(k), &mut buf).1,
            };
            let cost = player.video_cost();
            let ms = cost.compose_ns as f64 / 1e6;
            if composes.iter().all(|w: &f64| *w < ms) {
                r.worst_at = Tick(k);
            }
            composes.push(ms);
            r.composites += 1;
            r.composed_on_gpu += u64::from(cost.gpu);
            r.plan_ns += cost.plan_ns;
            r.stages.upload_ns += cost.stages.upload_ns;
            r.stages.upload_bytes += cost.stages.upload_bytes;
            r.stages.encode_ns += cost.stages.encode_ns;
            r.stages.wait_ns += cost.stages.wait_ns;
            r.stages.copy_ns += cost.stages.copy_ns;
            if c && new_frame {
                r.shown += 1;
                if o.hash_frames {
                    r.frames.push((Tick(k), xxhash_rust::xxh3::xxh3_64(&buf)));
                }
            }
            r.incomplete += u64::from(!c);
            if !c {
                r.starved.push((Tick(k), player.missing_layers(Tick(k))));
            }
            key = Some((k, c));
        }
        stalls.push(ui0.elapsed().as_secs_f64() * 1e3);
        let under = player.underruns.load(Ordering::Relaxed);
        if under != seen_under {
            r.underrun_at.push((t, under - seen_under));
            seen_under = under;
        }
        // a real window's swapchain holds the UI back here, inside the paint, not in the preview
        let device0 = Instant::now();
        if !o.free_running {
            player.finish_present();
        }
        r.device_ns += device0.elapsed().as_nanos() as u64;
        let busy_until = ui0 + Duration::from_nanos(o.ui_ns);
        let wake = next_vsync.max(busy_until);
        if let Some(d) = wake.checked_duration_since(Instant::now()) {
            std::thread::sleep(d);
        } else {
            next_vsync = Instant::now();
        }
    }
    r.wall_s = started.elapsed().as_secs_f64();
    player.pause();
    stop.store(true, Ordering::Release);
    let _ = clock.join();
    let cache = player.cache.stats();
    r.video_hits = cache.video_hits - cache0.video_hits;
    r.video_misses = cache.video_misses - cache0.video_misses;
    r.unused_evictions = cache.unused_evictions - cache0.unused_evictions;
    r.abandoned_evictions = cache.abandoned_evictions - cache0.abandoned_evictions;
    r.video_decodes = player.decode_stats.video_decodes.load(Ordering::Relaxed) - dec0;
    r.underruns = player.underruns.load(Ordering::Relaxed) - under0;
    r.ui_stall_ms = Percentiles::of(stalls);
    r.compose_ms = Percentiles::of(composes);
    r
}
