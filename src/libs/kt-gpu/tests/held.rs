//! t2621, t2622 (stream-front-on-device, found-defects 543, capability `video-fx`): an engine on the
//! compute host's device leaves a frame there, and a compute job reads it with no readback or upload.
//! Without a wgpu adapter: skip.

use std::sync::Arc;

use kt_engine::{HeldOut, VideoGpuExec};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_plugin_api::compute::{Buf, ComputeHost, Init, Job, Step};
use kt_time::Tick;

/// A copy of the first buffer into the second, word by word.
const COPY: &str = "@group(0) @binding(0) var<uniform> u: vec4<u32>;\n\
    @group(0) @binding(1) var<storage, read> a: array<u32>;\n\
    @group(0) @binding(2) var<storage, read_write> b: array<u32>;\n\
    @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
    if id.x < u.x { b[id.x] = a[id.x]; }\n}\n";

fn frame(w: u32, h: u32) -> Arc<VideoFrame> {
    let rgba: Vec<u8> = (0..w * h * 4).map(|i| (i * 37 % 251) as u8).collect();
    Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false })
}

fn read_held(host: &dyn ComputeHost, init: Init<'_>) -> Result<Vec<u8>, String> {
    let n = init.len();
    let uniform: Vec<u8> = [(n / 4) as u32, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let steps = [Step { name: "t2621:copy", shader: COPY, uniform: &uniform, read: &[Buf(0)], write: &[Buf(1)], groups: [((n / 4) as u32).div_ceil(64), 1, 1] }];
    Ok(host.run(&Job { buffers: &[init, Init::Zero(n)], steps: &steps, reads: &[Buf(1)] })?.remove(0))
}

/// Test t2621: `execute_held` on the compute host's device gives the bytes `execute_once` gives — at
/// a width whose rows are and are not a multiple of the copy alignment — read by a compute job from
/// the held buffer; the held buffer's spares make no new buffer for the same size after the first.
#[test]
fn t2621_a_held_frame_is_the_read_back_frame_on_the_compute_device() {
    kt_plugins_builtin::install();
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let Some(e) = kt_gpu::front_on_compute() else {
        if std::env::var("KT_REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("KT_REQUIRE_GPU=1, but no compute device");
        }
        eprintln!("skipped: no compute device");
        return;
    };
    let host = kt_plugin_api::compute::host().expect("the compute host opened").clone();
    let mut slot = FxInstance::plugin("posterize").unwrap();
    slot.set_param("levels", 3.0);
    let chain = [slot];
    for (w, h) in [(64, 24), (100, 30)] {
        let plan = kt_engine::chain_plan(frame(w, h), &chain, 7, 0.0, Tick(0), None);
        let mut bytes = vec![];
        assert!(e.execute_once(&plan, &mut bytes).unwrap(), "{w}×{h}: complete");
        let HeldOut::Held(held) = e.execute_held(&plan).unwrap() else { panic!("{w}×{h}: the frame stays on the compute device") };
        assert_eq!(held.len, (w * h * 4) as u64, "{w}×{h}: tightly packed");
        assert_eq!(read_held(&*host, Init::Held(&held)).unwrap(), bytes, "{w}×{h}: the held frame is the read-back frame");
    }
}

/// Test t2622: a held buffer of another device is refused by the job — an error, not a panic, and
/// nothing of the other device is read.
#[test]
fn t2622_a_held_frame_of_another_device_is_refused() {
    kt_plugins_builtin::install();
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let (Some(host), Some(other)) = (kt_plugin_api::compute::host().cloned(), kt_gpu::test_engine()) else { return };
    other.hold_for_compute();
    let mut slot = FxInstance::plugin("posterize").unwrap();
    slot.set_param("levels", 3.0);
    let chain = [slot];
    let plan = kt_engine::chain_plan(frame(64, 24), &chain, 7, 0.0, Tick(0), None);
    let HeldOut::Held(held) = other.execute_held(&plan).unwrap() else { panic!("the engine held its frame") };
    let err = read_held(&*host, Init::Held(&held)).expect_err("another device's buffer is refused");
    assert!(err.contains("another device"), "{err}");
}

/// Test t2708 (stream-round-trips D1, found-defects 401/621): a buffer a job keeps stays on the compute
/// device, a later job reads it as held with nothing uploaded, and a dropped one comes back as a spare
/// the next job of its size takes instead of creating a buffer.
#[test]
fn t2708_a_kept_buffer_feeds_the_next_job_on_the_device() {
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let Some(host) = kt_plugin_api::compute::host().cloned() else {
        if std::env::var("KT_REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("KT_REQUIRE_GPU=1, but no compute device");
        }
        eprintln!("skipped: no compute device");
        return;
    };
    assert!(host.keeps(), "kt-gpu's host keeps buffers");
    let src: Vec<u8> = (0..4096u32).map(|i| (i * 13 % 251) as u8).collect();
    let n = src.len() as u64;
    let uniform: Vec<u8> = [(n / 4) as u32, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let steps = [Step { name: "t2708:copy", shader: COPY, uniform: &uniform, read: &[Buf(0)], write: &[Buf(1)], groups: [((n / 4) as u32).div_ceil(64), 1, 1] }];
    let kept0 = kt_gpu::compute::kept_buffers();
    let (reads, held) = host.run_keeping(&Job { buffers: &[Init::Bytes(&src), Init::Zero(n)], steps: &steps, reads: &[] }, &[Buf(1)]).unwrap();
    assert!(reads.is_empty());
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].len, n);
    assert_eq!(kt_gpu::compute::kept_buffers() - kept0, 1);
    let held0 = kt_gpu::compute::held_reads();
    assert_eq!(read_held(&*host, Init::Held(&held[0])).unwrap(), src, "the kept buffer holds what the job wrote");
    assert_eq!(kt_gpu::compute::held_reads() - held0, 1, "read where it lies");
    drop(held);
    // its buffer is a spare now: the same job again creates nothing
    let c0 = kt_gpu::resources_created();
    let (_, again) = host.run_keeping(&Job { buffers: &[Init::Bytes(&src), Init::Zero(n)], steps: &steps, reads: &[] }, &[Buf(1)]).unwrap();
    assert_eq!(kt_gpu::resources_created(), c0, "a kept buffer's size made a new buffer after the first was dropped");
    assert_eq!(read_held(&*host, Init::Held(&again[0])).unwrap(), src);
}
