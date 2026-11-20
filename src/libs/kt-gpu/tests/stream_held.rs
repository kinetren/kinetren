//! t3814–t3816 (stream-picture-stays D3, found-defects 663): a stream stage's picture left on the
//! compute host's device is a layer input. An engine on that device converts its planes there with
//! the processor's integer arithmetic (the same bytes); an engine on another device reads them back.
//! One test process per test (nextest): the shared device is the process's. Without a wgpu adapter:
//! skip.

use std::sync::Arc;

use kt_engine::VideoGpuExec;
use kt_engine::video_plan::{HeldLayer, LayerInput, PlanLayer};
use kt_media::VideoFrame;
use kt_plugin_api::compute::{Buf, Held, Init, Job, Step};
use kt_plugin_api::stream::{HeldStreamOut, planes_to_rgba};
use kt_time::Tick;

const COPY3: &str = "@group(0) @binding(0) var<uniform> u: vec4<u32>;\n\
    @group(0) @binding(1) var<storage, read> a: array<u32>;\n\
    @group(0) @binding(2) var<storage, read> b: array<u32>;\n\
    @group(0) @binding(3) var<storage, read> c: array<u32>;\n\
    @group(0) @binding(4) var<storage, read_write> x: array<u32>;\n\
    @group(0) @binding(5) var<storage, read_write> y: array<u32>;\n\
    @group(0) @binding(6) var<storage, read_write> z: array<u32>;\n\
    @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
    if id.x < u.x { x[id.x] = a[id.x]; }\n\
    if id.x < u.y { y[id.x] = b[id.x]; }\n\
    if id.x < u.z { z[id.x] = c[id.x]; }\n}\n";

/// The window's device shared with the compute host, or `None` (skip).
fn shared() -> Option<kt_gpu::GpuContext> {
    kt_plugins_builtin::install();
    let Ok(window) = kt_gpu::GpuContext::new_headless() else {
        assert!(std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1"), "KT_REQUIRE_GPU=1, but no device");
        eprintln!("skipped: no device");
        return None;
    };
    let ctx = kt_gpu::compute::share_display(&window.device, &window.queue, window.adapter_info.clone());
    if !kt_gpu::compute::shares_display() {
        eprintln!("skipped: the compute host would open another adapter");
        return None;
    }
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    kt_plugin_api::compute::host().expect("the compute host opened");
    Some(ctx)
}

/// Planes of a padded `pw`×`ph` picture with every value in them, kept on the host's device.
fn planes(pw: u32, ph: u32, seed: u32) -> ([Vec<u8>; 3], [Held; 3]) {
    let host = kt_plugin_api::compute::host().unwrap();
    let mk = |n: u32, s: u32| -> Vec<u8> { (0..n).map(|i| (i.wrapping_mul(2_654_435_761).wrapping_add(s) >> 13) as u8).collect() };
    let (y, cb, cr) = (mk(pw * ph, seed), mk(pw * ph / 4, seed ^ 1), mk(pw * ph / 4, seed ^ 2));
    let lens = [y.len() as u64, cb.len() as u64, cr.len() as u64];
    let uniform: Vec<u8> = lens.iter().flat_map(|n| ((n / 4) as u32).to_le_bytes()).chain(0u32.to_le_bytes()).collect();
    let steps = [Step { name: "t3815:planes", shader: COPY3, uniform: &uniform, read: &[Buf(0), Buf(1), Buf(2)], write: &[Buf(3), Buf(4), Buf(5)], groups: [((lens[0] / 4) as u32).div_ceil(64), 1, 1] }];
    let buffers = [Init::Bytes(&y), Init::Bytes(&cb), Init::Bytes(&cr), Init::Zero(lens[0]), Init::Zero(lens[1]), Init::Zero(lens[2])];
    let (_, held) = host.run_keeping(&Job { buffers: &buffers, steps: &steps, reads: &[] }, &[Buf(3), Buf(4), Buf(5)]).unwrap();
    let held: [Held; 3] = held.try_into().expect("three held planes");
    ([y, cb, cr], held)
}

/// The composite of one layer at `w`×`h`: its input held, or the frame the processor converts.
#[allow(clippy::too_many_arguments)]
fn compose(e: &dyn VideoGpuExec, held: &[Held; 3], bytes: &[Vec<u8>; 3], pw: u32, ph: u32, w: u32, h: u32, as_held: bool) -> Vec<u8> {
    let mut rgba = Vec::new();
    planes_to_rgba(&bytes[0], &bytes[1], &bytes[2], pw as usize, w as usize, h as usize, &mut rgba);
    let frame = Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let mut plan = kt_engine::chain_plan(frame, &[], 3, 0.0, Tick(0), None);
    if as_held {
        let PlanLayer::One { src, .. } = &mut plan.layers[0] else { unreachable!() };
        let out = HeldStreamOut { planes: held.clone(), pw, ph, w, h, key: 7 };
        src.input = LayerInput::Held(Arc::new(HeldLayer { out, pts: Tick(0) }));
    }
    let mut out = Vec::new();
    assert!(e.execute_once(&plan, &mut out).unwrap(), "{w}×{h}: complete");
    out
}

/// Test t3815: on the shared device the planes become the layer's texture in one pass, with the bytes
/// of the processor's conversion — even and odd widths and heights, a crop inside the padding.
#[test]
fn t3815_the_planes_pass_is_the_processors_conversion() {
    let Some(ctx) = shared() else { return };
    let e = kt_gpu::GpuFxEngine::new(ctx).unwrap();
    for (pw, ph, w, h) in [(64, 32, 64, 32), (64, 32, 63, 31), (80, 48, 62, 33), (16, 16, 1, 1)] {
        let (bytes, held) = planes(pw, ph, w * 31 + h);
        let (before, back) = kt_gpu::held_pictures();
        let want = compose(&*e, &held, &bytes, pw, ph, w, h, false);
        let got = compose(&*e, &held, &bytes, pw, ph, w, h, true);
        assert_eq!(got, want, "{w}×{h} of {pw}×{ph}: the device's conversion is the processor's");
        assert_eq!(kt_gpu::held_pictures(), (before + 1, back), "{w}×{h}: converted on the device, nothing read back");
    }
}

/// Test t3816: an engine on another device reads the held planes back and gives the same composite.
#[test]
fn t3816_another_device_reads_the_planes_back() {
    let Some(_ctx) = shared() else { return };
    let Ok(other) = kt_gpu::GpuContext::new_headless() else { return };
    let e = kt_gpu::GpuFxEngine::new(other).unwrap();
    let (pw, ph, w, h) = (64, 48, 61, 45);
    let (bytes, held) = planes(pw, ph, 5);
    let (on, before) = kt_gpu::held_pictures();
    let want = compose(&*e, &held, &bytes, pw, ph, w, h, false);
    let got = compose(&*e, &held, &bytes, pw, ph, w, h, true);
    assert_eq!(got, want, "the read-back composite is the bytes path's");
    assert_eq!(kt_gpu::held_pictures(), (on, before + 1), "read back, not converted on a foreign device");
}
