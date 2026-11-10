//! t2357 (stream-history-on-device, capability `telemetry`, found-defects 398): the compute host
//! records when a job's submission was made and when it ran on the device, on the trace's clock.
//! Its own test binary: the arbiter's switch is process-wide. Without a wgpu adapter: skip.

use std::time::{Duration, Instant};

use kt_gpu::arbiter;
use kt_gpu::compute::ComputeDevice;
use kt_plugin_api::compute::{Buf, ComputeHost, Init, Job, Step};
use kt_time::trace::{self, Ev};

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

/// A job of one dispatch spinning `n` steps a lane, named `name`.
fn spin(host: &dyn ComputeHost, name: &str, n: u32) {
    const SPIN: &str = "@group(0) @binding(0) var<uniform> u: vec4<u32>;\n\
        @group(0) @binding(1) var<storage, read> a: array<u32>;\n\
        @group(0) @binding(2) var<storage, read_write> b: array<u32>;\n\
        @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
        var x = a[id.x] + id.x;\n\
        for (var i = 0u; i < u.x; i = i + 1u) { x = x * 1664525u + 1013904223u; }\n\
        b[id.x] = x;\n}\n";
    let words = vec![0u8; 256];
    let uniform: Vec<u8> = [n, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let steps = [Step { name, shader: SPIN, uniform: &uniform, read: &[Buf(0)], write: &[Buf(1)], groups: [1, 1, 1] }];
    host.run(&Job { buffers: &[Init::Bytes(&words), Init::Zero(256)], steps: &steps, reads: &[Buf(1)] }).unwrap();
}

/// Test t2357: each compute job leaves a `gpu_start` record whose device start lies at or after its
/// submission and whose end at or before its buffers came back (within the calibration's error); a
/// job submitted behind a long one starts late by about the long one's run.
#[test]
fn t2357_a_jobs_device_start_is_on_the_cpu_clock() {
    let Some(host) = device() else { return };
    // both jobs go in flight at once: the late start is the queue's, not an admission's
    arbiter::set_enabled(false);
    spin(&*host, "test:warm", 1000);
    let mut long_dev = Duration::ZERO;
    let items = trace::testing::capture(|| {
        spin(&*host, "test:alone", 1000);
        let h = {
            let host = host.clone();
            std::thread::spawn(move || {
                let t = Instant::now();
                spin(&*host, "test:long", 30_000_000);
                t.elapsed()
            })
        };
        // the long job is submitted first; the short one queues behind it on the device
        std::thread::sleep(Duration::from_millis(20));
        spin(&*host, "test:behind", 1000);
        long_dev = h.join().unwrap();
    });
    let recs: Vec<_> = trace::testing::recs(&items).into_iter().filter(|r| r.kind() == Some(Ev::GpuStart)).collect();
    assert!(recs.len() >= 3, "a gpu_start record per job: {}", recs.len());
    if recs.iter().all(|r| r.aux == u32::MAX) {
        eprintln!("t2357: the device gives no timestamps; the records say so");
        return;
    }
    for r in &recs {
        assert!(r.aux < u32::MAX - 1, "a device with timestamps is calibrated: {r:?}");
        let err = r.aux as u64 * 1000 + 1000;
        assert!(r.b + err >= r.a, "started before it was submitted: {r:?}");
        assert!(r.c >= r.b, "ended before it started: {r:?}");
        assert!(r.c <= r.t + err, "ended after its buffers came back: {r:?}");
    }
    // the job behind the long one: submitted while the long one ran, so it waited to start
    let mut late: Vec<u64> = recs.iter().map(|r| r.b.saturating_sub(r.a)).collect();
    late.sort_unstable();
    let longest = *late.last().unwrap();
    let runs: Vec<u64> = recs.iter().map(|r| r.c - r.b).collect();
    let long_run = *runs.iter().max().unwrap();
    eprintln!("t2357: start delays {late:?} ns, runs {runs:?} ns, long job wall {long_dev:?}");
    assert!(
        longest * 4 >= long_run,
        "the job behind the long one did not start late: start delays {late:?}, the long run {long_run} ns"
    );
}
