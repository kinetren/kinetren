//! `kt-cli gpu-share <project.kt> [--seconds N] [--arbiter on|off|both] [--display S] [--offline S]
//! [--mosh S] [--size WxH]` (gpu-arbiter, D11): row 149 reproduced on this machine. A hardware card
//! runs the display's frame, the offline work and the compute host's jobs through ONE ring; a
//! software rasterizer gives every device a queue thread of its own and hides it. So here the three
//! share ONE device:
//!
//! - the display presents the project's frames from `--display` on, one every frame period, and
//!   measures how long each waited for the device (class now);
//! - the offline engine composes the moment at `--offline` again and again (class offline);
//! - the look-ahead advances the stream history of the moment at `--mosh` and on (class near), its
//!   Datamosh jobs on a compute host on the same device.
//!
//! Each mode runs `--seconds`; the report is the display's wait (p50/p95/max) and what the others
//! got done. With `--trace DIR` the run's admissions and pieces are in the trace (`kt-cli trace DIR
//! gpu`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{PresentOptions, VideoGpuExec, VideoRenderer};
use kt_gpu::compute::ComputeDevice;
use kt_gpu::{arbiter, GpuContext, GpuFxEngine};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::sched::{with_class, Class};
use kt_time::Tick;

static SHARED: OnceLock<Arc<ComputeDevice>> = OnceLock::new();

/// The compute host the bench installs: on the display's device.
fn open_shared() -> Result<Arc<dyn kt_plugin_api::compute::ComputeHost>, String> {
    SHARED.get().cloned().map(|h| h as _).ok_or_else(|| "the bench's device is not open".into())
}

/// Every source of the project, opened on the calling thread (a decoder is owned by one thread).
fn sources(project: &Project) -> HashMap<MediaId, Box<dyn MediaSource>> {
    let mut out: HashMap<MediaId, Box<dyn MediaSource>> = HashMap::new();
    for m in &project.media {
        let s = match kt_media::factory::open_pseudo(&m.path) {
            Some(r) => r.ok(),
            None => kt_media::ffmpeg::FfmpegSource::open(&m.path).ok().map(|s| Box::new(s) as Box<dyn MediaSource>),
        };
        if let Some(s) = s {
            out.insert(m.id, s);
        }
    }
    for (addr, state) in kt_engine::gen_sources(project) {
        out.insert(addr, kt_media::factory::open_gen(&state));
    }
    out
}

fn facts(src: &HashMap<MediaId, Box<dyn MediaSource>>) -> kt_engine::SourceFacts {
    let mut f = kt_engine::SourceFacts::default();
    for (m, s) in src {
        f.observe(*m, s.info());
    }
    f
}

/// Whether a stream stage is active anywhere in `frames` moments from `t` (the display's range must
/// have none: it would compute a history instead of presenting).
fn has_stream(video: &VideoRenderer, t: Tick, frames: i64) -> bool {
    let step = video.grid.frame_step();
    (0..frames).any(|i| video.stream_frame_bytes(Tick(t.0 + i * step), Quality { width: 2, height: 2 }) > 0)
}

/// Decode what the plans of `frames` moments from `t` read (their stream histories advanced first)
/// into `cache`, one span per source and size.
fn decode_range(
    video: &VideoRenderer,
    src: &mut HashMap<MediaId, Box<dyn MediaSource>>,
    cache: &BurstCache,
    t: Tick,
    frames: i64,
    q: Quality,
) {
    let step = video.grid.frame_step();
    if video.has_stream_stages() {
        for i in 0..frames {
            kt_engine::stream_preroll(video, cache, src, Tick(t.0 + i * step), q);
        }
    }
    let mut per: HashMap<(MediaId, Quality), Vec<Tick>> = HashMap::new();
    for (m, pts, dq) in video.wanted_frames(kt_time::TickRange::new(t, Tick(t.0 + frames * step)), q) {
        per.entry((m, dq)).or_default().push(pts);
    }
    for ((m, dq), keys) in per {
        let Some(s) = src.get_mut(&m) else { continue };
        let sid = kt_engine::source_id(m);
        let mut keys = keys;
        keys.sort();
        keys.dedup();
        let _ = s.read_video_span(&keys, Some(dq), &mut |k, frame| {
            cache.insert_video(sid, Some(dq), kt_engine::video_cache_idx(k), frame.into());
        });
    }
}

struct Mode {
    lat: Vec<Duration>,
    offline_frames: u64,
    mosh_positions: u64,
    pieces: u64,
}

fn pct(v: &[Duration], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v[((v.len() - 1) as f64 * q) as usize].as_secs_f64() * 1e3
}

#[allow(clippy::too_many_arguments)]
fn one_mode(
    project: &Arc<Project>,
    display: &Arc<GpuFxEngine>,
    offline: &Arc<GpuFxEngine>,
    q: Quality,
    at: (Tick, Tick, Tick),
    run: Duration,
    on: bool,
) -> Mode {
    arbiter::set_enabled(on);
    let stop = Arc::new(AtomicBool::new(false));
    let pieces = arbiter::pieces_cut();
    let offline_frames = Arc::new(AtomicU64::new(0));
    let mosh_positions = Arc::new(AtomicU64::new(0));
    let off = {
        let (p, offline, stop, n) = (project.clone(), offline.clone(), stop.clone(), offline_frames.clone());
        std::thread::spawn(move || {
            let _class = with_class(Class::Offline);
            let mut src = sources(&p);
            let video = VideoRenderer::new_with_facts(&p, &facts(&src));
            let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: 1 << 30 });
            decode_range(&video, &mut src, &cache, at.1, 1, q);
            let mut buf = Vec::new();
            while !stop.load(Ordering::Acquire) {
                // execute_once: the readback memo would answer the same moment for free
                let plan = video.frame_plan(&cache, at.1, q);
                let _ = offline.execute_once(&plan, &mut buf);
                n.fetch_add(1, Ordering::Relaxed);
            }
        })
    };
    let mosh = {
        let (p, stop, n) = (project.clone(), stop.clone(), mosh_positions.clone());
        std::thread::spawn(move || {
            let _class = with_class(Class::Near);
            let mut src = sources(&p);
            let mut video = VideoRenderer::new_with_facts(&p, &facts(&src));
            // a memo of its own each mode: the history is computed anew
            video.set_stream_memo(Arc::new(kt_plugin_api::stream::StreamMemo::new(256 << 20)));
            let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: 256 << 20 });
            let mut t = at.2;
            while !stop.load(Ordering::Acquire) && video.has_stream_stages() {
                kt_engine::stream_preroll(&video, &cache, &mut src, t, q);
                n.fetch_add(1, Ordering::Relaxed);
                t = Tick(t.0 + video.grid.frame_step());
            }
        })
    };
    let _class = with_class(Class::Now);
    let mut src = sources(project);
    let video = VideoRenderer::new_with_facts(project, &facts(&src));
    let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: 1 << 30 });
    let step = video.grid.frame_step();
    let frames = 50;
    let t = Instant::now();
    decode_range(&video, &mut src, &cache, at.0, frames, q);
    eprintln!("  display: {frames} frames decoded in {:.1} s", t.elapsed().as_secs_f64());
    let period = Duration::from_nanos(kt_time::sched::frame_period_ns());
    let mut lat = Vec::new();
    let start = Instant::now();
    let mut i = 0;
    while start.elapsed() < run {
        let t0 = Instant::now();
        let plan = video.frame_plan(&cache, Tick(at.0.0 + (i % frames) * step), q);
        if display.present(&plan, PresentOptions { show_incomplete: true, glow: false }).is_ok() {
            display.wait_presented();
            let waited = t0.elapsed();
            kt_time::trace::ev(kt_time::trace::Ev::FrameWait)
                .id(i as u64)
                .aux(kt_time::trace::wait::DEVICE)
                .lasted(waited.as_nanos() as u64)
                .emit();
            lat.push(waited);
        }
        i += 1;
        std::thread::sleep(period.saturating_sub(t0.elapsed()));
    }
    stop.store(true, Ordering::Release);
    off.join().unwrap();
    mosh.join().unwrap();
    lat.sort();
    Mode {
        lat,
        offline_frames: offline_frames.load(Ordering::Relaxed),
        mosh_positions: mosh_positions.load(Ordering::Relaxed),
        pieces: arbiter::pieces_cut() - pieces,
    }
}

pub fn run(args: &[String]) -> i32 {
    let Some(input) = args.get(1).filter(|a| !a.starts_with("--")) else {
        eprintln!(
            "usage: kt-cli gpu-share <project.kt> [--seconds N] [--arbiter on|off|both] [--display S] [--offline S] [--mosh S] [--size WxH]"
        );
        return 2;
    };
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let secs = |name: &str, d: f64| opt(name).and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
    let project = match crate::load_project_resolved(input) {
        Ok(p) => Arc::new(p),
        Err(c) => return c,
    };
    let (w, h) = opt("--size")
        .and_then(|v| v.split_once('x'))
        .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        .unwrap_or((project.settings.width, project.settings.height));
    let q = Quality { width: w & !1, height: h & !1 };
    let tick = |s: f64| Tick((s * kt_time::TICK_HZ as f64) as i64);
    let at = (tick(secs("--display", 0.0)), tick(secs("--offline", 1.0)), tick(secs("--mosh", 1.0)));
    let run = Duration::from_secs_f64(secs("--seconds", 6.0));
    kt_time::sched::set_frame_period_ns(1_000_000_000 / project.settings.fps.max(1) as u64);
    let ctx = match GpuContext::new_headless() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("no device: {e}");
            return 1;
        }
    };
    let same = || GpuContext { device: ctx.device.clone(), queue: ctx.queue.clone(), adapter_info: ctx.adapter_info.clone() };
    let (Ok(display), Ok(offline)) = (GpuFxEngine::new(same()), GpuFxEngine::new(same())) else {
        eprintln!("the engine failed to build");
        return 1;
    };
    display.register_display();
    let _ = SHARED.set(ComputeDevice::on_context(&ctx));
    kt_plugin_api::compute::install(open_shared);
    let ids: Vec<String> = kt_engine::prewarm::effect_ids(&project).into_iter().map(String::from).collect();
    for e in [&display, &offline] {
        e.prewarm_effects(&ids);
    }
    kt_engine::prewarm::compute(&ids);
    display.settle();
    offline.settle();
    {
        let src = sources(&project);
        let video = VideoRenderer::new_with_facts(&project, &facts(&src));
        if has_stream(&video, at.0, 50) {
            eprintln!("--display {:.2}: a stream stage is active in its 50 frames; pick a moment without one", at.0.0 as f64 / kt_time::TICK_HZ as f64);
            return 2;
        }
        if !has_stream(&video, at.2, 1) {
            eprintln!("--mosh: no stream stage at that moment");
            return 2;
        }
        if opt("--check").is_some() || args.iter().any(|a| a == "--check") {
            eprintln!("the moments fit");
            return 0;
        }
    }
    println!(
        "-- gpu-share {}x{} on ONE device: {} (bound {:.1} ms) --",
        q.width,
        q.height,
        ctx.adapter_info.name,
        arbiter::bound() as f64 / 1e6
    );
    // a lap untimed: every pipeline drawn, every pass measured, every source's decoder warm
    eprintln!("  warming");
    let _ = one_mode(&project, &display, &offline, q, at, Duration::from_secs(2), true);
    let modes: &[bool] = match opt("--arbiter").map(String::as_str) {
        Some("on") => &[true],
        Some("off") => &[false],
        _ => &[false, true],
    };
    for &on in modes {
        let m = one_mode(&project, &display, &offline, q, at, run, on);
        println!(
            "arbiter {:<3}: display waited p50 {:.1} p95 {:.1} max {:.1} ms over {} presents; offline {:.2} frames/s; history {:.1} positions/s; {} pieces",
            if on { "on" } else { "off" },
            pct(&m.lat, 0.5),
            pct(&m.lat, 0.95),
            m.lat.last().map_or(0.0, |d| d.as_secs_f64() * 1e3),
            m.lat.len(),
            m.offline_frames as f64 / run.as_secs_f64(),
            m.mosh_positions as f64 / run.as_secs_f64(),
            m.pieces
        );
    }
    arbiter::set_enabled(true);
    0
}
