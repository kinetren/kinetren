//! t2510, t2511 (gpu-leftovers, capability `video-plan`, found-defects 511 and 520): the compute host
//! admits every submission on its ring, and a job writes its uniforms once and binds by offset.
//! Its own test binary: the bind-group counter is process-wide. Without a wgpu adapter: skip.

use std::sync::Mutex;

use kt_gpu::compute::ComputeDevice;
use kt_plugin_api::compute::{Buf, ComputeHost, Init, Job, Step};
use kt_time::trace::{self, Ev};

static ONE: Mutex<()> = Mutex::new(());

fn device() -> Option<std::sync::Arc<ComputeDevice>> {
    match kt_gpu::compute::open_device() {
        Ok(d) => Some(d),
        Err(e) if std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1") => {
            eprintln!("skipped: no compute device ({e})");
            None
        }
        Err(e) => panic!("KT_REQUIRE_GPU=1, but no compute device: {e}"),
    }
}

/// `b[i] += u.x * k` over 64 words, for the shader's own `k`.
fn add(k: u32) -> String {
    format!(
        "@group(0) @binding(0) var<uniform> u: vec4<u32>;\n\
         @group(0) @binding(1) var<storage, read_write> b: array<u32>;\n\
         @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {{\n\
         b[id.x] = b[id.x] + u.x * {k}u;\n}}\n"
    )
}

/// Test t2510: the clock calibration a traced job makes is admitted on the compute host's ring, as
/// every piece of the job is — no submission of the host goes around its ring.
#[test]
fn t2510_the_calibration_is_admitted_on_the_ring() {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(host) = device() else { return };
    let shader = add(1);
    let uniform: Vec<u8> = [1u32, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let job = |host: &ComputeDevice| {
        let steps = [Step { name: "test:calib", shader: &shader, uniform: &uniform, read: &[], write: &[Buf(0)], groups: [1, 1, 1] }];
        host.run(&Job { buffers: &[Init::Zero(256)], steps: &steps, reads: &[Buf(0)] }).unwrap();
    };
    // untraced: no calibration, the job's own pieces only
    let before = host.ring().admitted();
    job(&host);
    let pieces = host.ring().admitted() - before;
    assert!(pieces >= 1, "the job's piece is admitted");
    let mut admitted = 0;
    let items = trace::testing::capture(|| {
        let before = host.ring().admitted();
        job(&host);
        admitted = host.ring().admitted() - before;
    });
    let timed = trace::testing::recs(&items).iter().any(|r| r.kind() == Some(Ev::GpuStart) && r.aux < u32::MAX - 1);
    if !timed {
        eprintln!("t2510: no timestamps on this device, nothing to calibrate");
        return;
    }
    // the first traced job brackets the clock three times, each its own admitted submission
    assert_eq!(admitted, pieces + 3, "the calibration's three submissions are admitted ({pieces} pieces)");
}

/// Test t2511: a job of 240 dispatches over two shaders, each with its own uniform, gives the sum
/// every dispatch's uniform asks (the dynamic offsets point at each step's own slot), and creates
/// one bind group per shader, not one per dispatch.
#[test]
fn t2511_a_job_binds_its_uniforms_by_offset() {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(host) = device() else { return };
    let (one, two) = (add(1), add(2));
    let uniforms: Vec<Vec<u8>> = (0..240u32).map(|i| [i + 1, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect()).collect();
    let steps: Vec<Step<'_>> = uniforms
        .iter()
        .enumerate()
        .map(|(i, u)| Step {
            name: if i % 2 == 0 { "test:one" } else { "test:two" },
            shader: if i % 2 == 0 { &one } else { &two },
            uniform: u,
            read: &[],
            write: &[Buf(0)],
            groups: [1, 1, 1],
        })
        .collect();
    let made = kt_gpu::compute::bind_groups_made();
    let out = host.run(&Job { buffers: &[Init::Zero(256)], steps: &steps, reads: &[Buf(0)] }).unwrap();
    let made = kt_gpu::compute::bind_groups_made() - made;
    let want: u32 = (0..240u32).map(|i| (i + 1) * if i % 2 == 0 { 1 } else { 2 }).sum();
    let got: Vec<u32> = out[0].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    assert!(got.iter().all(|&v| v == want), "every word is the sum of its dispatches' uniforms: {want}, got {:?}", &got[..4]);
    assert_eq!(made, 2, "one bind group per shader, not per dispatch");
}
