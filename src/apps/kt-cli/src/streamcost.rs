//! `kt-cli stream-cost <p.kt> <from_s> <to_s> [--laps N] [--size WxH] [--loop]` (stream-layout-per-build):
//! what a stream stage (Datamosh) costs over a range, lap after lap, the way the prefetch thread
//! advances it: the source frames are decoded into a cache FIRST (never timed), then each frame of the
//! range is advanced (`VideoRenderer::stream_advance`) and measured — the advancing thread's processor
//! time (the stage's own work: schedule, keys, colour conversion, packing), the compute jobs' wall
//! time, their count by kind, and the bytes each job uploaded and read back. Every few frames the
//! prefetch's question for its 2 s window (`wanted_frames`) is timed too (found-defects 144). The memo
//! is the preview's own (`stream_memo`), so lap 2 shows what a loop recomputes (found-defects 143);
//! `--loop` sizes it as the player does for the range as its loop region.
//! Judge the numbers on a RELEASE build; the device's time depends on the device.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Instant;

use kt_plugin_api::compute::{ComputeHost, Init, Job, Shader};
use kt_time::Tick;

static UP: AtomicU64 = AtomicU64::new(0);
static DOWN: AtomicU64 = AtomicU64::new(0);
static JOB_NS: AtomicU64 = AtomicU64::new(0);
static JOB_CPU_NS: AtomicU64 = AtomicU64::new(0);
static ENCODES: AtomicU64 = AtomicU64::new(0);
static DECODES: AtomicU64 = AtomicU64::new(0);
static OTHER: AtomicU64 = AtomicU64::new(0);

/// The process's compute device, counting what each job moves (kt-cli's only host; a pass-through).
struct Counting(Arc<dyn ComputeHost>);

impl ComputeHost for Counting {
    fn run(&self, job: &Job<'_>) -> Result<Vec<Vec<u8>>, String> {
        let up: usize = job.buffers.iter().map(|b| if let Init::Bytes(b) = b { b.len() } else { 0 }).sum();
        let first = job.steps.first().map_or("", |s| s.name);
        let kind = if job.steps.iter().any(|s| s.name.ends_with(":residual")) {
            &ENCODES
        } else if first.ends_with(":decode") {
            &DECODES
        } else {
            &OTHER
        };
        kind.fetch_add(1, Relaxed);
        let (t, c) = (Instant::now(), thread_cpu_ns());
        let got = self.0.run(job);
        JOB_NS.fetch_add(t.elapsed().as_nanos() as u64, Relaxed);
        JOB_CPU_NS.fetch_add(thread_cpu_ns().saturating_sub(c), Relaxed);
        UP.fetch_add(up as u64, Relaxed);
        if let Ok(g) = &got {
            DOWN.fetch_add(g.iter().map(|v| v.len() as u64).sum(), Relaxed);
        }
        got
    }
    fn prepare(&self, shader: &Shader) -> Result<(), String> {
        self.0.prepare(shader)
    }
}

/// What `kt_plugin_api::compute::install` takes in kt-cli: the GPU's compute host, counted.
pub fn open() -> Result<Arc<dyn ComputeHost>, String> {
    kt_gpu::compute::open().map(|h| Arc::new(Counting(h)) as Arc<dyn ComputeHost>)
}

/// The calling thread's processor time.
#[cfg(unix)]
pub fn thread_cpu_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: a valid out pointer; the clock id is a constant
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

#[cfg(not(unix))]
pub fn thread_cpu_ns() -> u64 {
    0
}

#[derive(Default, Clone, Copy)]
struct Snap {
    up: u64,
    down: u64,
    job_ns: u64,
    job_cpu_ns: u64,
    enc: u64,
    dec: u64,
    other: u64,
}

fn snap() -> Snap {
    Snap {
        up: UP.load(Relaxed),
        down: DOWN.load(Relaxed),
        job_ns: JOB_NS.load(Relaxed),
        job_cpu_ns: JOB_CPU_NS.load(Relaxed),
        enc: ENCODES.load(Relaxed),
        dec: DECODES.load(Relaxed),
        other: OTHER.load(Relaxed),
    }
}

fn since(a: Snap) -> Snap {
    let b = snap();
    Snap {
        up: b.up - a.up,
        down: b.down - a.down,
        job_ns: b.job_ns - a.job_ns,
        job_cpu_ns: b.job_cpu_ns - a.job_cpu_ns,
        enc: b.enc - a.enc,
        dec: b.dec - a.dec,
        other: b.other - a.other,
    }
}

pub fn run(project: &kt_model::Project, path: &str, from: f64, to: f64, laps: usize, size: Option<kt_media::Quality>, sized: bool) -> Result<(), String> {
    let mut fetcher = crate::open_sources(project, path, false, None)?;
    let mut facts = kt_engine::SourceFacts::default();
    for m in &project.media {
        if let Some(s) = fetcher.source_mut(m.id) {
            facts.observe(m.id, s.info());
        }
    }
    let t_build = Instant::now();
    let video = kt_engine::VideoRenderer::new_with_facts(project, &facts);
    let build_ms = t_build.elapsed().as_secs_f64() * 1e3;
    let q = size.unwrap_or(kt_media::Quality { width: project.settings.width, height: project.settings.height });
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig { max_audio_bytes: 1, max_video_bytes: 3 << 30 });
    let step = video.grid.frame_step().max(1);
    let t0 = video.grid.align_floor(Tick::from_secs_f64(from));
    let t1 = Tick::from_secs_f64(to);
    let memo = kt_engine::video::stream_memo();
    if sized {
        // the player's sizing for this range as its loop region
        let positions = ((t1.0 - t0.0).max(0) / video.grid.frame_step().max(1)) as usize + 1;
        let per = 2 * video.stream_frame_bytes(t0, q);
        let cap = kt_engine::video::size_stream_memo(per, Some(positions));
        let free = kt_engine::video::memory_available().unwrap_or(0);
        println!("  memo sized for a loop of {positions} positions: {} MB (available {} MB)", cap >> 20, free >> 20);
    }
    println!(
        "-- stream cost {:.2}–{:.2} s at {}x{} (renderer built in {build_ms:.0} ms, memo {} MB) --",
        from,
        to,
        q.width,
        q.height,
        memo.bytes_cap() >> 20
    );
    if !video.has_stream_stages() {
        return Err("no stream stage in the project".into());
    }
    // the frames an advance at `t` still lacks, decoded into the cache (never timed)
    let mut feed = |t: Tick| -> usize {
        let mut per: std::collections::BTreeMap<(kt_model::MediaId, kt_media::Quality), Vec<Tick>> = Default::default();
        let mut n = 0;
        for (m, pts, dq) in video.layers_at(t, q) {
            if cache.get_video(kt_engine::source_id(m), Some(dq), kt_engine::video_cache_idx(pts)).is_none() {
                per.entry((m, dq)).or_default().push(pts);
                n += 1;
            }
        }
        for ((m, dq), keys) in per {
            if let Some(s) = fetcher.source_mut(m) {
                let sid = kt_engine::source_id(m);
                let _ = s.read_video_span(&keys, Some(dq), &mut |k, f| {
                    cache.insert_video(sid, Some(dq), kt_engine::video_cache_idx(k), f.into());
                });
            }
        }
        n
    };
    let window = Tick(2 * kt_time::TICK_HZ);
    for lap in 0..laps.max(1) {
        let (mut frames, mut stage_cpu, mut wall, mut stuck) = (0u64, 0u64, 0u64, 0u64);
        let mut needs_ms: Vec<f64> = Vec::new();
        let s0 = snap();
        let mut t = t0;
        while t < t1 {
            let mut tries = 0;
            loop {
                let fed = feed(t);
                let (w, c) = (Instant::now(), thread_cpu_ns());
                let pending = video.stream_advance(&cache, t, q);
                wall += w.elapsed().as_nanos() as u64;
                stage_cpu += thread_cpu_ns() - c;
                tries += 1;
                if !pending {
                    break;
                }
                if fed == 0 || tries > 64 {
                    stuck += 1;
                    break;
                }
            }
            if frames % 15 == 0 {
                let c = thread_cpu_ns();
                let _ = video.wanted_frames(kt_time::TickRange::new(t, Tick(t.0 + window.0)), q);
                needs_ms.push((thread_cpu_ns() - c) as f64 / 1e6);
            }
            frames += 1;
            t = Tick(t.0 + step);
        }
        let d = since(s0);
        let f = frames.max(1) as f64;
        let own_cpu = stage_cpu.saturating_sub(d.job_cpu_ns);
        needs_ms.sort_by(f64::total_cmp);
        let med = needs_ms.get(needs_ms.len() / 2).copied().unwrap_or(0.0);
        let max = needs_ms.last().copied().unwrap_or(0.0);
        println!(
            "  lap {}: {} frames in {:.1} s of advance ({:.1} ms a frame); encodes {} ({:.2} a frame), decodes {}, other jobs {}{}",
            lap + 1,
            frames,
            wall as f64 / 1e9,
            wall as f64 / 1e6 / f,
            d.enc,
            d.enc as f64 / f,
            d.dec,
            d.other,
            if stuck > 0 { format!(", {stuck} frames left short") } else { String::new() }
        );
        println!(
            "    processor time of the advancing thread: {:.2} ms a frame outside the jobs, {:.2} inside them (submission, copies); jobs' wall {:.2} ms a frame",
            own_cpu as f64 / 1e6 / f,
            d.job_cpu_ns as f64 / 1e6 / f,
            d.job_ns as f64 / 1e6 / f
        );
        let jobs = (d.enc + d.dec + d.other).max(1) as f64;
        println!(
            "    bytes: uploaded {:.2} MB a frame, read back {:.2} MB a frame ({:.2} / {:.2} MB a job)",
            d.up as f64 / f / 1e6,
            d.down as f64 / f / 1e6,
            d.up as f64 / jobs / 1e6,
            d.down as f64 / jobs / 1e6
        );
        println!("    prefetch window question (2 s, `wanted_frames`): p50 {med:.2} ms, max {max:.2} ms ({} asked)", needs_ms.len());
    }
    println!("  memo holds {} MB of {} MB", memo.bytes() >> 20, memo.bytes_cap() >> 20);
    Ok(())
}
