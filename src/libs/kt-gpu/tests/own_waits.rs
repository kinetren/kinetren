//! t2864, t2865, t2866 (datamosh-history-wait; capability `video-plan`): a completion is seen when it
//! comes. Every wait names its own submission, a ring's waiter looks at every device in flight, and the
//! preview's present checks never drift apart. Its own test binary: the wait counter is process-wide.
//! Without a wgpu adapter: skip with a printout.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec};
use kt_gpu::arbiter::{self, RingKey};
use kt_gpu::{GpuContext, GpuFxEngine};
use kt_media::VideoFrame;
use kt_plugin_api::compute::{Buf, ComputeHost, Init, Job, Step};
use kt_time::sched::Class;
use kt_time::Tick;

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

const DOUBLE: &str = "@group(0) @binding(0) var<uniform> u: vec4<u32>;\n\
    @group(0) @binding(1) var<storage, read> a: array<u32>;\n\
    @group(0) @binding(2) var<storage, read_write> b: array<u32>;\n\
    @compute @workgroup_size(4) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
    if id.x < u.x { b[id.x] = a[id.x] * 2u; }\n}\n";

fn plan(f: &Arc<VideoFrame>, id: u64) -> FramePlan<'static> {
    let src = LayerSrc {
        source: kt_cache::SourceId(id),
        cache_idx: 0,
        content: id,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(f.clone())),
        fx: kt_engine::LayerChain::Borrowed(&[]),
        fx_hash: id,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    };
    FramePlan { t: Tick(id as i64), out_w: f.width, out_h: f.height, layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }] }
}

/// Test t2864 (datamosh-history-wait D2, found-defects 693): a read-back frame, the warm-up of its
/// pipelines and a compute job each wait for their own submission: none waits for everything submitted
/// to the device (which also waits for every other thread's work and holds the device's lock).
#[test]
fn t2864_no_frame_or_job_waits_for_everything() {
    kt_plugins_builtin::install();
    let Some(ctx) = context() else { return };
    let before = kt_gpu::waits_for_all();
    let engine = GpuFxEngine::new(ctx).expect("engine");
    engine.settle();
    let rgba: Vec<u8> = (0..64 * 36u32).flat_map(|i| [i as u8, (i / 64) as u8, 7, 255]).collect();
    let f = Arc::new(VideoFrame { pts: Tick(0), width: 64, height: 36, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let mut out = Vec::new();
    for i in 0..3 {
        engine.execute_into(&plan(&f, 10 + i), &mut out).expect("a frame");
        assert_eq!(out.len(), 64 * 36 * 4);
    }
    let host = kt_gpu::compute::open_device().expect("compute device");
    let words: Vec<u8> = (0u32..9).flat_map(|v| v.to_le_bytes()).collect();
    let uniform: Vec<u8> = [9u32, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let step = Step { name: "t2864", shader: DOUBLE, uniform: &uniform, read: &[Buf(0)], write: &[Buf(1)], groups: [3, 1, 1] };
    let got = host.run(&Job { buffers: &[Init::Bytes(&words), Init::Zero(36)], steps: &[step], reads: &[Buf(1)] }).expect("a job");
    assert_eq!(&got[0][32..36], &16u32.to_le_bytes());
    assert_eq!(kt_gpu::waits_for_all(), before, "a frame path waited for everything submitted to its device");
}

/// Copies `n` times between two buffers of `bytes` on `ctx`'s queue: work that keeps a software
/// device's queue thread busy for a while.
fn busy(ctx: &GpuContext, bytes: u64, n: u32) -> wgpu::SubmissionIndex {
    let make = |usage| ctx.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: bytes, usage, mapped_at_creation: false });
    let a = make(wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST);
    let b = make(wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST);
    let mut enc = ctx.device.create_command_encoder(&Default::default());
    for i in 0..n {
        let (s, d) = if i % 2 == 0 { (&a, &b) } else { (&b, &a) };
        enc.copy_buffer_to_buffer(s, 0, d, 0, bytes);
    }
    ctx.queue.submit([enc.finish()])
}

/// Test t2865 (datamosh-history-wait D3, found-defects 662): a waiter of a ring whose flights are on two
/// devices sees the room a short piece on one makes while a long piece on the other still runs; it does
/// not block on the long one until it ends.
#[test]
fn t2865_a_waiter_sees_a_completion_on_another_device() {
    let (Some(long), Some(short)) = (context(), context()) else { return };
    let ring = arbiter::ring_for(RingKey::Device(0x2865_0000));
    let _display = ring.register_display();
    let ms = 1_000_000;
    let long_done = Arc::new(AtomicBool::new(false));
    // the oldest flight: seconds of copies on the first device
    let first = ring.admit(Class::Near, ms);
    let sub = busy(&long, 256 << 20, 48);
    let flag = long_done.clone();
    long.queue.on_submitted_work_done(move || flag.store(true, Ordering::Release));
    // its time a guess: it makes room only by its completion, never by the clock
    let _long_flight = first.submitted_as(&long.device, &long.queue, sub, ms, false, true);
    // a short piece on the second device, still running when the waiter comes
    let second = ring.admit(Class::Near, ms);
    let sub = busy(&short, 64 << 20, 6);
    let _short_flight = second.submitted_as(&short.device, &short.queue, sub, ms, false, true);
    // the rest of the bound, still being encoded: only a completion makes room for the waiter
    // (render-ahead D5: the bound alone limits what stands in front of the display, no count)
    let held = ring.admit(Class::Near, arbiter::bound() - 2 * ms);
    let t0 = Instant::now();
    let ticket = ring.admit(Class::Near, ms);
    let waited = t0.elapsed();
    let _ = long.device.poll(wgpu::PollType::Poll);
    assert!(
        !long_done.load(Ordering::Acquire),
        "admitted after {waited:?}, only when the long piece ended: the short piece's completion went unseen"
    );
    drop((ticket, held));
    let _ = long.device.poll(wgpu::PollType::wait_indefinitely());
    eprintln!("t2865: admitted after {waited:?}");
}

/// Test t2866 (datamosh-history-wait D4, found-defects 693): the preview's present checks never grow
/// past the frame's own time (at least 4 ms): a frame the device ends late is seen within that.
#[test]
fn t2866_present_checks_stop_growing() {
    let ms = Duration::from_millis(1);
    for expect in [0u64, 300_000, 2_000_000, 20_000_000] {
        let waits: Vec<Duration> = kt_gpu::present_waits(expect).take(60).collect();
        let own = if expect == 0 { 2 * ms } else { Duration::from_nanos(expect) };
        assert_eq!(waits[0], own.max(ms), "the first check after the frame's own time");
        let cap = own.max(4 * ms);
        assert!(waits.iter().all(|w| *w <= cap && *w >= ms), "{expect}: {waits:?}");
        // a few checks a frame, not one a millisecond: the waits do grow up to the cap
        assert_eq!(*waits.last().unwrap(), cap, "{expect}: {waits:?}");
    }
}
