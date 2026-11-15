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
        fx: kt_engine::LayerChain::Borrowed(fx),
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
        display.present(&plan(&f, &none, i), PresentOptions { glow: false }).expect("present");
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
/// look-ahead job, whatever came first. The ring is held by an admitted piece never submitted (it
/// never drains) until both wait, and the order is the arbiter's own: its admission records. A long
/// job as the holder and the order the jobs finished in were the machine's (stage 11: under the full
/// run the holder ended before the frame's job came to wait, or the frame's thread recorded late).
#[test]
fn t2255_the_compute_host_admits_the_frame_on_screen_first() {
    use kt_time::trace::{self, Ev};
    let _one = lock();
    let Some(ctx) = context() else { return };
    let host = ComputeDevice::on_context(&ctx);
    // traced: the host's clock calibration (admitted on the ring since gpu-leftovers D1) is made here,
    // not by the first job of the capture below
    trace::testing::capture(|| {
        spin(&*host, "test:short", 1000, 1).unwrap();
    });
    let ring = host.ring().clone();
    let deadline = kt_time::test_deadline();
    let held = Mutex::new(0u64);
    let items = trace::testing::capture(|| {
        let holder = ring.admit(Class::Near, arbiter::bound());
        *held.lock().unwrap() = trace::now_ns();
        let mut hs = Vec::new();
        for c in [Class::Near, Class::Now] {
            let host = host.clone();
            hs.push(std::thread::spawn(move || {
                let _class = with_class(c);
                // never measured: each claims the whole bound
                match c {
                    Class::Now => spin(&*host, "test:now", 1000, 1).unwrap(),
                    _ => spin(&*host, "test:near-long", 5_000_000, 1).unwrap(),
                };
            }));
            while !ring.higher_waits(Class::Offline) || (c == Class::Now && !ring.higher_waits(Class::Near)) {
                assert!(Instant::now() < deadline, "{c:?} never waited");
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        drop(holder);
        for h in hs {
            h.join().unwrap();
        }
    });
    // when each waiter was admitted: its admission span's end (the holder's came before `since`)
    let since = *held.lock().unwrap();
    let mut admitted: Vec<(u64, u32)> = trace::testing::recs(&items)
        .into_iter()
        .filter(|r| r.kind() == Some(Ev::GpuAdmit) && r.id == ring.id() && r.t >= since)
        .filter(|r| r.aux == Class::Now as u32 || r.aux == Class::Near as u32)
        .map(|r| (r.t + r.dur, r.aux))
        .collect();
    admitted.sort_unstable();
    let order: Vec<u32> = admitted.iter().map(|a| a.1).collect();
    assert_eq!(order, vec![Class::Now as u32, Class::Near as u32], "the look-ahead's job was admitted first (waiting admissions by when they ended)");
}

/// Test t2260: a present is waited for by a few non-blocking checks, not one a millisecond.
#[test]
fn t2260_a_present_is_waited_for_by_a_few_checks() {
    let _one = lock();
    let Some((_ctx, display, _offline)) = shared() else { return };
    let fx = vec![instance("glow"), instance("radial_blur")];
    let f = frame(5);
    let o = PresentOptions { glow: false };
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

/// The offline device as the program has it: the slow route's engine serving the preview, and its
/// sibling for effect cells, reel strips and posters.
fn slow_and_side() -> Option<(Arc<GpuFxEngine>, Arc<GpuFxEngine>)> {
    kt_plugins_builtin::install();
    let ctx = context()?;
    let slow = GpuFxEngine::new(ctx).expect("offline engine");
    slow.serve_preview();
    slow.settle();
    let side = slow.sibling();
    Some((slow, side))
}

/// Test t2338 (scheduler-followups D6): the side engine is made from the offline engine's compiled
/// set (it compiles nothing) and composes the same bytes.
#[test]
fn t2338_the_side_engine_shares_the_compiled_pipelines() {
    let _one = lock();
    let Some((slow, side)) = slow_and_side() else { return };
    assert!(side.shares_compiled(&slow), "the sibling compiled a set of its own");
    let fx = heavy();
    let f = frame(3);
    let p = plan(&f, &fx, 31);
    let (mut a, mut b) = (Vec::new(), Vec::new());
    slow.execute_once(&p, &mut a).expect("slow");
    side.execute_once(&p, &mut b).expect("side");
    assert!(a == b, "the sibling's frame differs from its engine's");
}

/// Test t2337 (scheduler-followups D6, row 290): side work on the offline device holds the slow
/// route's frame for one piece at most. With the side engine's lock held (a cell mid-composite) a
/// slow-route frame composes; with heavy side frames running at class background, the route's
/// frames wait less than one heavy frame takes, the heavy frames are cut, and both kinds of frame
/// keep their solo bytes.
#[test]
fn t2337_side_work_holds_the_slow_route_for_one_piece() {
    let _one = lock();
    let Some((slow, side)) = slow_and_side() else { return };
    arbiter::set_enabled(true);
    let fx = heavy();
    let (fh, fl) = (frame(5), frame(6));
    let heavy_p = plan(&fh, &fx, 41);
    let light_p = plan(&fl, &[], 42);
    // solo: the bytes, and the heavy frame's own time once its passes are measured
    let (mut heavy_solo, mut light_solo) = (Vec::new(), Vec::new());
    {
        let _c = with_class(Class::Background);
        for _ in 0..2 {
            side.execute_once(&heavy_p, &mut heavy_solo).expect("heavy");
        }
    }
    let t0 = Instant::now();
    {
        let _c = with_class(Class::Background);
        side.execute_once(&heavy_p, &mut heavy_solo).expect("heavy");
    }
    let heavy_time = t0.elapsed();
    {
        let _c = with_class(Class::Now);
        slow.execute_once(&light_p, &mut light_solo).expect("light");
    }

    // a cell holds the side engine's lock: the route's frame does not wait for it
    {
        let held = side.hold();
        let done = AtomicBool::new(false);
        std::thread::scope(|s| {
            s.spawn(|| {
                let _c = with_class(Class::Now);
                let mut out = Vec::new();
                slow.execute_once(&light_p, &mut out).expect("light beside a held cell");
                assert!(out == light_solo, "the route's frame changed beside a held cell");
                done.store(true, Ordering::Release);
            });
            let deadline = kt_time::test_deadline();
            while !done.load(Ordering::Acquire) {
                assert!(Instant::now() < deadline, "the route's frame waited for the side engine's lock");
                std::thread::sleep(Duration::from_millis(2));
            }
        });
        drop(held);
    }

    // heavy side frames run; the route's frames wait one piece, not a heavy frame. `on`: the
    // engine the heavy frames compose on (the former arrangement: the route's own)
    let route_waits = |on: &Arc<GpuFxEngine>| {
        let stop = AtomicBool::new(false);
        let mut waits = Vec::new();
        std::thread::scope(|s| {
            s.spawn(|| {
                let _c = with_class(Class::Background);
                let mut out = Vec::new();
                while !stop.load(Ordering::Acquire) {
                    on.execute_once(&heavy_p, &mut out).expect("heavy beside the route");
                    assert!(out == heavy_solo, "a cut heavy frame changed its bytes");
                }
            });
            let _c = with_class(Class::Now);
            let mut out = Vec::new();
            // the heavy frame is on the device before the route asks
            std::thread::sleep(heavy_time / 4);
            for _ in 0..6 {
                let t = Instant::now();
                slow.execute_once(&light_p, &mut out).expect("light beside heavy");
                waits.push(t.elapsed());
                assert!(out == light_solo, "the route's frame changed beside heavy work");
                std::thread::sleep(heavy_time / 3);
            }
            stop.store(true, Ordering::Release);
        });
        waits
    };
    // before (row 290): cells, strips and posters on the route's own engine
    let before = route_waits(&slow);
    let cuts0 = arbiter::pieces_cut();
    let waits = route_waits(&side);
    let cuts = arbiter::pieces_cut() - cuts0;
    let worst = waits.iter().max().copied().unwrap_or_default();
    let round = |w: &[Duration]| w.iter().map(|d| ms(*d).round()).collect::<Vec<_>>();
    eprintln!(
        "t2337: heavy frame {:.1} ms; route frames beside heavy work on its own engine {:?} ms, on the side engine {:?} ms, {cuts} cuts",
        ms(heavy_time),
        round(&before),
        round(&waits)
    );
    assert!(cuts > 0, "no heavy side frame was cut on the route's ring");
    assert!(worst < heavy_time, "a route frame waited {:.1} ms, a whole heavy frame is {:.1} ms", ms(worst), ms(heavy_time));
}
