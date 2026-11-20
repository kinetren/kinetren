//! t3817, t3818 (stream-picture-stays D2, found-defects 663; the owner, kkt-26.11.20 Q51): the compute
//! host runs on the window's device where the backend computes and the host would pick that adapter,
//! so what it keeps is the preview engine's to read. One test process per test (nextest): the shared
//! device is the process's. Without a wgpu adapter: skip.

use kt_engine::{HeldOut, VideoGpuExec};
use kt_gpu::AdapterFact;
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_plugin_api::compute::{Buf, Init, Job, Step};
use kt_time::Tick;
use std::sync::Arc;

fn fact(name: &str, kind: wgpu::DeviceType, backend: wgpu::Backend, device: u32) -> AdapterFact {
    AdapterFact { name: name.into(), kind, backend, vendor: 0x1002, device }
}

fn info(f: &AdapterFact) -> wgpu::AdapterInfo {
    wgpu::AdapterInfo {
        name: f.name.clone(),
        vendor: f.vendor,
        device: f.device,
        device_type: f.kind,
        device_pci_bus_id: String::new(),
        driver: String::new(),
        driver_info: String::new(),
        backend: f.backend,
        subgroup_min_size: 4,
        subgroup_max_size: 64,
        transient_saves_memory: None,
        limit_bucket: None,
    }
}

/// Test t3818: the decision is pure — the window's Vulkan device of the adapter the host would open is
/// shared; a GL window, a window on the integrated chip while the host picks the card, and a GL-only
/// list are not; the host's pick skips WARP and prefers a card.
#[test]
fn t3818_one_device_where_the_window_computes() {
    use wgpu::{Backend::*, DeviceType::*};
    let card = fact("Radeon RX", DiscreteGpu, Vulkan, 0x73bf);
    let igpu = fact("Raphael", IntegratedGpu, Vulkan, 0x164e);
    let warp = fact("Microsoft Basic Render Driver", Cpu, Dx12, 0x8c);
    let gl_card = fact("Radeon RX", DiscreteGpu, Gl, 0x73bf);
    let list = vec![warp.clone(), igpu.clone(), gl_card.clone(), card.clone()];
    assert_eq!(kt_gpu::compute::compute_pick(&list).map(|a| a.name.as_str()), Some("Radeon RX"));
    assert_eq!(kt_gpu::compute::compute_pick(&list).map(|a| a.backend), Some(Vulkan), "never a GL adapter");
    assert_eq!(kt_gpu::compute::compute_pick(std::slice::from_ref(&warp)), None, "WARP is never the host's");
    let pick = kt_gpu::compute::compute_pick(&list);
    assert_eq!(kt_gpu::compute::share_refusal(&info(&card), pick), None, "the window on the card the host picks");
    assert!(kt_gpu::compute::share_refusal(&info(&gl_card), pick).is_some(), "a GL window computes nothing");
    assert!(kt_gpu::compute::share_refusal(&info(&igpu), pick).is_some(), "the window on the integrated chip, the host on the card");
    assert!(kt_gpu::compute::share_refusal(&info(&card), None).is_some(), "no adapter for the host");
    let lone = [igpu.clone()];
    assert_eq!(kt_gpu::compute::share_refusal(&info(&igpu), kt_gpu::compute::compute_pick(&lone)), None, "a lone integrated chip");
}

const COPY: &str = "@group(0) @binding(0) var<uniform> u: vec4<u32>;\n\
    @group(0) @binding(1) var<storage, read> a: array<u32>;\n\
    @group(0) @binding(2) var<storage, read_write> b: array<u32>;\n\
    @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
    if id.x < u.x { b[id.x] = a[id.x]; }\n}\n";

/// Test t3817: with the window's device shared, the compute host opens on that very device (the same
/// `Arc`), and a frame the window's engine leaves on the device is read by a compute job as it lies —
/// the bytes `execute_once` reads back.
#[test]
fn t3817_the_compute_host_runs_on_the_windows_device() {
    kt_plugins_builtin::install();
    let Ok(window) = kt_gpu::GpuContext::new_headless() else {
        if std::env::var("KT_REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("KT_REQUIRE_GPU=1, but no device");
        }
        eprintln!("skipped: no device");
        return;
    };
    let ctx = kt_gpu::compute::share_display(&window.device, &window.queue, window.adapter_info.clone());
    if !kt_gpu::compute::shares_display() {
        eprintln!("skipped: {:?}", kt_gpu::compute::share_refusal(&window.adapter_info, kt_gpu::compute::compute_pick(&kt_gpu::adapter_facts())));
        return;
    }
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let host = kt_plugin_api::compute::host().expect("the compute host opened").clone();
    let dev = kt_gpu::compute::host_device().expect("this crate's host");
    assert!(Arc::ptr_eq(&dev.context().device, &ctx.device), "the host's device is the window's");
    // the window's context a second time: the same one
    let again = kt_gpu::compute::share_display(&window.device, &window.queue, window.adapter_info.clone());
    assert!(Arc::ptr_eq(&again.device, &ctx.device));
    let e = kt_gpu::front_on_compute().expect("an engine on the host's device");
    let (w, h) = (64u32, 24u32);
    let rgba: Vec<u8> = (0..w * h * 4).map(|i| (i * 37 % 251) as u8).collect();
    let frame = Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let mut slot = FxInstance::plugin("posterize").unwrap();
    slot.set_param("levels", 3.0);
    let chain = [slot];
    let plan = kt_engine::chain_plan(frame, &chain, 7, 0.0, Tick(0), None);
    let mut bytes = vec![];
    assert!(e.execute_once(&plan, &mut bytes).unwrap());
    let HeldOut::Held(held) = e.execute_held(&plan).unwrap() else { panic!("the frame stays on the device") };
    let n = held.len;
    let uniform: Vec<u8> = [(n / 4) as u32, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let steps = [Step { name: "t3817:copy", shader: COPY, uniform: &uniform, read: &[Buf(0)], write: &[Buf(1)], groups: [((n / 4) as u32).div_ceil(64), 1, 1] }];
    let got = host.run(&Job { buffers: &[Init::Held(&held), Init::Zero(n)], steps: &steps, reads: &[Buf(1)] }).unwrap().remove(0);
    assert_eq!(got, bytes, "the window's held frame is read by the host where it lies");
}
