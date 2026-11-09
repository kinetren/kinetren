//! t2253, t2254, t2255, t2260 (gpu-arbiter; capability `video-plan`): one ring per physical queue.
//! A card's ring is reproduced on llvmpipe by putting the display, the offline engine and the compute
//! host on ONE device, which lavapipe runs on one queue thread, submission after submission. Its own
//! test binary: the arbiter's switch and the piece counter are process-wide, so the tests here take
//! one lock. Without a wgpu adapter: skip with a printout.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, PresentOptions, VideoGpuExec};
use kt_gpu::arbiter;
use kt_gpu::compute::ComputeDevice;
use kt_gpu::{GpuContext, GpuFxEngine};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_plugin_api::compute::{Buf, ComputeHost, Init, Job, Step};
use kt_time::sched::{with_class, Class};
use kt_time::Tick;

const W: u32 = 1920;
const H: u32 = 1080;

static ONE: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    ONE.lock().unwrap_or_else(|e| e.into_inner())
}

/// One headless device, as the program opens it; `None` — no adapter (skip).
fn context() -> Option<GpuContext> {
    match GpuContext::new_headless() {
        Ok(c) => Some(c),
        Err(e) if std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1") => {
            eprintln!("skipped: wgpu adapter unavailable ({e})");
            None
        }
        Err(e) => panic!("KT_REQUIRE_GPU=1, but no wgpu adapter: {e}"),
    }
}

fn same(ctx: &GpuContext) -> GpuContext {
    GpuContext { device: ctx.device.clone(), queue: ctx.queue.clone(), adapter_info: ctx.adapter_info.clone() }
}

fn frame(seed: u32) -> Arc<VideoFrame> {
    let rgba: Vec<u8> = (0..W * H).flat_map(|i| [(i % W) as u8 ^ seed as u8, (i / W) as u8, (i * 7 % 251) as u8, 255]).collect();
    Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: false })
}

fn plan<'a>(f: &Arc<VideoFrame>, fx: &'a [FxInstance], id: u64) -> FramePlan<'a> {
    let src = LayerSrc {
        source: SourceId(id),
        cache_idx: 0,
        content: id,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(f.clone())),
        fx: std::borrow::Cow::Borrowed(fx),
        fx_hash: id,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    };
    FramePlan { t: Tick(id as i64), out_w: W, out_h: H, layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }] }
}

fn instance(id: &str) -> FxInstance {
    let reg = kt_plugin_api::installed().unwrap();
    let m = reg.get(kt_plugin_api::Family::Fx, id).unwrap().manifest();
    FxInstance::plugin_with(id, &m.passport.iter().map(|s| s.default).collect::<Vec<_>>()).unwrap()
}

/// The heavy stack of the owner's project.
fn heavy() -> Vec<FxInstance> {
    vec![instance("vhs_tape"), instance("camera_isp")]
}

/// The display and the offline engine on one device, the display registered.
fn shared() -> Option<(GpuContext, Arc<GpuFxEngine>, Arc<GpuFxEngine>)> {
    kt_plugins_builtin::install();
    let ctx = context()?;
    let display = GpuFxEngine::new(same(&ctx)).expect("display engine");
    let offline = GpuFxEngine::new(same(&ctx)).expect("offline engine");
    display.register_display();
    display.settle();
    offline.settle();
    Some((ctx, display, offline))
}

/// Test t2253: on a ring with a display a heavy frame of a lower class is cut into pieces at pass
/// boundaries once its passes are measured, no piece of several passes past the bound, and its
/// bytes equal the uncut frame's.
#[test]
fn t2253_a_long_frame_is_cut_on_a_displayed_ring_with_the_same_bytes() {
    let _one = lock();
    let Some((_ctx, _display, offline)) = shared() else { return };
    let _class = with_class(Class::Offline);
    let fx = heavy();
    let f = frame(1);
    let p = plan(&f, &fx, 21);
    arbiter::set_enabled(false);
    let mut uncut = Vec::new();
    // twice: the second frame's passes are warm, and its timestamps measured
    for _ in 0..2 {
        offline.execute_once(&p, &mut uncut).expect("uncut");
    }
    arbiter::set_enabled(true);
    let (pieces, over) = (arbiter::pieces_cut(), arbiter::pieces_over());
    let mut cut = Vec::new();
    offline.execute_once(&p, &mut cut).expect("cut");
    arbiter::set_enabled(true);
    let pieces = arbiter::pieces_cut() - pieces;
    assert!(pieces >= 1, "a heavy 1080p frame was not cut (bound {} ns)", arbiter::bound());
    assert_eq!(arbiter::pieces_over() - over, 0, "a piece of several passes went past the bound");
    assert!(cut == uncut, "the cut frame's bytes differ from the uncut frame's");
    eprintln!("t2253: {pieces} cuts, bound {:.1} ms", arbiter::bound() as f64 / 1e6);
}

/// What the display's presents waited, in a run with an offline heavy chain and a long compute job
/// on the same device.
fn display_latency(display: &Arc<GpuFxEngine>, offline: &Arc<GpuFxEngine>, host: &Arc<ComputeDevice>, run: Duration) -> Vec<Duration> {
    let stop = Arc::new(AtomicBool::new(false));
    let off = {
        let (offline, stop) = (offline.clone(), stop.clone());
        std::thread::spawn(move || {
            let _class = with_class(Class::Offline);
            let fx = heavy();
            let f = frame(2);
            let mut out = Vec::new();
            let mut i = 100;
            while !stop.load(Ordering::Acquire) {
                // a new key each frame: the readback memo must not answer it
                offline.execute_once(&plan(&f, &fx, i), &mut out).expect("offline frame");
                i += 1;
            }
        })
    };
    let near = {
        let (host, stop) = (host.clone(), stop.clone());
        std::thread::spawn(move || {
            let _class = with_class(Class::Near);
            while !stop.load(Ordering::Acquire) {
                spin(&*host, "test:near", 200_000, 3).expect("compute job");
            }
        })
    };
    let _class = with_class(Class::Now);
    let none: Vec<FxInstance> = Vec::new();
    let f = frame(3);
    let mut lat = Vec::new();
    let start = Instant::now();
    let mut i = 10_000;
    while start.elapsed() < run {
        let t0 = Instant::now();
        display.present(&plan(&f, &none, i), PresentOptions { show_incomplete: true, glow: false }).expect("present");
        display.wait_presented();
        lat.push(t0.elapsed());
        i += 1;
        std::thread::sleep(Duration::from_millis(40).saturating_sub(t0.elapsed()));
    }
    stop.store(true, Ordering::Release);
    off.join().unwrap();
    near.join().unwrap();
    lat.sort();
    lat
}

/// A compute job of `steps` dispatches named `name`, each spinning `n` rounds in 64 invocations (the
/// ring predicts a dispatch by its name).
fn spin(host: &dyn ComputeHost, name: &str, n: u32, steps: usize) -> Result<Vec<Vec<u8>>, String> {
    const SPIN: &str = "@group(0) @binding(0) var<uniform> u: vec4<u32>;\n\
        @group(0) @binding(1) var<storage, read> a: array<u32>;\n\
        @group(0) @binding(2) var<storage, read_write> b: array<u32>;\n\
        @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
        var x = a[id.x] + id.x;\n\
        for (var i = 0u; i < u.x; i = i + 1u) { x = x * 1664525u + 1013904223u; }\n\
        b[id.x] = x;\n}\n";
    let words = vec![0u8; 256];
    let uniform: Vec<u8> = [n, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let step = Step { name, shader: SPIN, uniform: &uniform, read: &[Buf(0)], write: &[Buf(1)], groups: [1, 1, 1] };
    let steps = vec![step; steps];
    host.run(&Job { buffers: &[Init::Bytes(&words), Init::Zero(256)], steps: &steps, reads: &[Buf(1)] })
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// Test t2254: the display's frame on a device shared with an offline heavy chain and a long compute
/// job waits for at most one piece with the arbiter, for whole offline frames without it. The limit
/// IS the check (t1671's list): the two modes are compared inside one run.
#[test]
fn t2254_the_displays_latency_on_a_shared_device_is_bounded() {
    let _one = lock();
    let Some((ctx, display, offline)) = shared() else { return };
    let host = ComputeDevice::on_context(&ctx);
    // warm: every pipeline drawn, every pass measured
    {
        let _class = with_class(Class::Offline);
        let fx = heavy();
        let f = frame(4);
        let mut out = Vec::new();
        for i in 0..2 {
            offline.execute_once(&plan(&f, &fx, 50 + i), &mut out).unwrap();
        }
        spin(&*host, "test:near", 200_000, 3).unwrap();
        spin(&*host, "test:near", 200_000, 3).unwrap();
    }
    let run = Duration::from_secs(4);
    arbiter::set_enabled(false);
    let before = display_latency(&display, &offline, &host, run);
    arbiter::set_enabled(true);
    let after = display_latency(&display, &offline, &host, run);
    let p = |v: &[Duration], q: f64| v[((v.len() - 1) as f64 * q) as usize];
    eprintln!(
        "t2254: display latency without the arbiter p50 {:.1} p95 {:.1} max {:.1} ms ({} frames); with it p50 {:.1} p95 {:.1} max {:.1} ms ({} frames)",
        ms(p(&before, 0.5)),
        ms(p(&before, 0.95)),
        ms(*before.last().unwrap()),
        before.len(),
        ms(p(&after, 0.5)),
        ms(p(&after, 0.95)),
        ms(*after.last().unwrap()),
        after.len()
    );
    assert!(p(&after, 0.5) < p(&before, 0.5), "the arbiter did not shorten the display's wait");
}

/// Test t2255: on the compute host's ring the frame on screen's job is admitted before a waiting
/// look-ahead job, whatever came first.
#[test]
fn t2255_the_compute_host_admits_the_frame_on_screen_first() {
    let _one = lock();
    let Some(ctx) = context() else { return };
    let host = ComputeDevice::on_context(&ctx);
    spin(&*host, "test:short", 1000, 1).unwrap();
    let order = Arc::new(Mutex::new(Vec::new()));
    let long = {
        let host = host.clone();
        std::thread::spawn(move || {
            let _class = with_class(Class::Near);
            spin(&*host, "test:long", 20_000_000, 1).unwrap();
        })
    };
    let ring = host.ring().clone();
    let deadline = kt_time::test_deadline();
    while ring.in_flight_ns() == 0 {
        assert!(Instant::now() < deadline, "the long job never went in flight");
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut hs = Vec::new();
    for c in [Class::Near, Class::Now] {
        let (host, order) = (host.clone(), order.clone());
        hs.push(std::thread::spawn(move || {
            let _class = with_class(c);
            // never measured: each claims the whole bound, so one runs at a time; the look-ahead's
            // is long, so the order they finish in is the order they were admitted in
            match c {
                Class::Now => spin(&*host, "test:now", 1000, 1).unwrap(),
                _ => spin(&*host, "test:near-long", 5_000_000, 1).unwrap(),
            };
            order.lock().unwrap().push(c);
        }));
        while !ring.higher_waits(Class::Offline) || (c == Class::Now && !ring.higher_waits(Class::Near)) {
            assert!(Instant::now() < deadline, "{c:?} never waited");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    long.join().unwrap();
    for h in hs {
        h.join().unwrap();
    }
    assert_eq!(*order.lock().unwrap(), vec![Class::Now, Class::Near], "the look-ahead's job went first");
}

/// Test t2260: a present is waited for by a few non-blocking checks, not one a millisecond.
#[test]
fn t2260_a_present_is_waited_for_by_a_few_checks() {
    let _one = lock();
    let Some((_ctx, display, _offline)) = shared() else { return };
    let fx = vec![instance("glow"), instance("radial_blur")];
    let f = frame(5);
    let o = PresentOptions { show_incomplete: true, glow: false };
    for i in 0..3 {
        display.present(&plan(&f, &fx, 300 + i), o).unwrap();
        display.wait_presented();
    }
    let (checks, n) = (kt_gpu::present_checks(), 10);
    let t0 = Instant::now();
    for i in 0..n {
        display.present(&plan(&f, &fx, 400 + i), o).unwrap();
        display.wait_presented();
    }
    let spent = t0.elapsed();
    let checks = kt_gpu::present_checks() - checks;
    eprintln!("t2260: {checks} checks over {n} presents, {:.1} ms a frame", ms(spent) / n as f64);
    assert!(checks <= 4 * n, "{checks} checks for {n} presents of {:.1} ms each", ms(spent) / n as f64);
}
