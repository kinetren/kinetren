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

const SPIN_MANY: &str = "@group(0) @binding(0) var<uniform> u: vec4<u32>;\n\
    @group(0) @binding(1) var<storage, read_write> b: array<u32>;\n\
    @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
    var x = b[id.x] + u.y;\n\
    for (var i = 0u; i < u.x; i = i + 1u) { x = x * 1664525u + 1013904223u; }\n\
    b[id.x] = x;\n}\n";

/// A job of `n` dependent dispatches whose names cycle through `names`, each spinning `work` steps a
/// lane over `groups` workgroups; its output words.
fn walk(host: &dyn ComputeHost, names: &[&str], n: usize, work: u32, groups: u32) -> Vec<u8> {
    let uniforms: Vec<Vec<u8>> = (0..n).map(|i| [work, i as u32, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect()).collect();
    let steps: Vec<Step<'_>> = (0..n)
        .map(|i| Step { name: names[i % names.len()], shader: SPIN_MANY, uniform: &uniforms[i], read: &[], write: &[Buf(0)], groups: [groups, 1, 1] })
        .collect();
    let size = groups as u64 * 64 * 4;
    host.run(&Job { buffers: &[Init::Zero(size)], steps: &steps, reads: &[Buf(0)] }).unwrap().remove(0)
}

/// Test t2435 (gpu-throughput D2, found-defects 481): a compute job never measured, on a ring that
/// carries a display, learns its dispatches' times from its own first pieces while it runs: it cuts
/// for an unmeasured dispatch at most once a name (the old claim of the whole bound per unmeasured
/// dispatch cut at every one until a map happened to come back), and its bytes equal a measured run's.
#[test]
fn t2435_a_cold_walk_is_cut_by_what_its_first_pieces_measured() {
    let Some(host) = device() else { return };
    if !host_has_timestamps(&host) {
        eprintln!("t2435: no timestamps on this device: nothing to learn from");
        return;
    }
    let _shown = host.ring().register_display();
    let names = ["t2435:a", "t2435:b", "t2435:c", "t2435:d"];
    let n = 64;
    let (before, guesses) = (arbiter::pieces_cut(), kt_gpu::compute::guess_cuts());
    let cold = walk(&*host, &names, n, 2000, 4);
    let cold_pieces = arbiter::pieces_cut() - before;
    let cold_guesses = kt_gpu::compute::guess_cuts() - guesses;
    let before = arbiter::pieces_cut();
    let warm = walk(&*host, &names, n, 2000, 4);
    let warm_pieces = arbiter::pieces_cut() - before;
    eprintln!(
        "t2435: {n} dispatches, cold {cold_pieces} cuts ({cold_guesses} before an unmeasured dispatch, {} learned), measured {warm_pieces} cuts",
        kt_gpu::compute::pieces_learned()
    );
    // a dispatch is a guess only until the first piece of its name came back: one cut a name at most.
    // The cuts the measured times call for follow the machine's speed (a software device's time is
    // processor time: under a full parallel run a dispatch here passes half the bound), not the arbiter
    assert!(
        cold_guesses <= names.len() as u64,
        "a cold walk went on guessing after its first pieces came back: {cold_guesses} cuts before an unmeasured dispatch over {} names",
        names.len()
    );
    assert_eq!(cold, warm, "the cuts changed the bytes");
}

fn host_has_timestamps(host: &ComputeDevice) -> bool {
    let items = trace::testing::capture(|| spin(host, "probe:stamps", 10));
    trace::testing::recs(&items).iter().any(|r| r.kind() == Some(Ev::GpuStart) && r.aux < u32::MAX - 1)
}

/// Test t2436 (gpu-throughput D1/D5, found-defects 480, 485): a walk keeps the device fed — on a ring
/// that carries a display with no display traffic, the next piece of a measured walk is submitted while
/// the one before still runs (counted from each piece's own `gpu_start`: its submit against the earlier
/// piece's device end), and the device stands idle with nothing queued for under a fifth of the walk.
/// Waiting for each piece's completion before admitting the next does neither.
#[test]
fn t2436_a_walk_keeps_the_device_fed() {
    let Some(host) = device() else { return };
    if !host_has_timestamps(&host) {
        eprintln!("t2436: no timestamps on this device");
        return;
    }
    let _shown = host.ring().register_display();
    let names = ["t2436:x", "t2436:y"];
    // measure the dispatches, then size the work so a piece holds several of them
    walk(&*host, &names, 8, 20_000, 16);
    let items = trace::testing::capture(|| {
        walk(&*host, &names, 160, 20_000, 16);
    });
    let recs = trace::testing::recs(&items);
    let starts: Vec<_> = recs.iter().filter(|r| r.kind() == Some(Ev::GpuStart)).collect();
    let execs = recs.iter().filter(|r| r.kind() == Some(Ev::GpuExec)).count();
    assert!(starts.len() >= 3, "the walk was not cut into pieces: {} gpu_start records", starts.len());
    assert_eq!(execs, starts.len(), "a gpu_exec per piece");
    let mut ordered: Vec<(u64, u64, u64)> = starts.iter().map(|r| (r.a, r.b, r.c)).collect();
    ordered.sort_unstable();
    let pairs = ordered.len() - 1;
    let ahead = ordered.windows(2).filter(|w| w[1].0 < w[0].2).count();
    let gaps: u64 = ordered.windows(2).map(|w| w[1].1.saturating_sub(w[0].2)).sum();
    // the idle the arbiter owns: after a piece whose successor was not yet submitted when it ended.
    // A gap behind a piece already queued is a software device's queue thread waiting for the
    // processor (a loaded machine), not a waiter that came late (stage 11, D1b)
    let unfed: u64 = ordered.windows(2).filter(|w| w[1].0 >= w[0].2).map(|w| w[1].1.saturating_sub(w[0].2)).sum();
    let span = ordered.last().unwrap().2.saturating_sub(ordered[0].1);
    eprintln!(
        "t2436: {} pieces, {ahead} of {pairs} submitted before the earlier ended, idle gaps {gaps} ns of {span}, {unfed} ns of them with nothing queued",
        ordered.len()
    );
    assert!(ahead * 2 >= pairs, "pieces waited for the earlier one's end: {ahead} of {pairs} submitted ahead");
    assert!(unfed * 5 < span, "the device stood idle with nothing queued: {unfed} ns of {span}");
}

/// Test t2437 (gpu-throughput D6, found-defects 486): two jobs at once keep their own timestamps —
/// each piece's `gpu_start` carries its own submit, its device start is at or after it, and no two
/// pieces share a start.
#[test]
fn t2437_concurrent_jobs_keep_their_own_starts() {
    let Some(host) = device() else { return };
    if !host_has_timestamps(&host) {
        eprintln!("t2437: no timestamps on this device");
        return;
    }
    let _shown = host.ring().register_display();
    walk(&*host, &["t2437:p"], 4, 20_000, 16);
    walk(&*host, &["t2437:q"], 4, 1000, 1);
    let items = trace::testing::capture(|| {
        let h = {
            let host = host.clone();
            std::thread::spawn(move || {
                walk(&*host, &["t2437:p"], 60, 20_000, 16);
            })
        };
        for _ in 0..4 {
            walk(&*host, &["t2437:q"], 4, 1000, 1);
        }
        h.join().unwrap();
    });
    let recs = trace::testing::recs(&items);
    let starts: Vec<_> = recs.iter().filter(|r| r.kind() == Some(Ev::GpuStart) && r.aux < u32::MAX - 1).collect();
    assert!(starts.len() >= 5, "records of both jobs: {}", starts.len());
    for r in &starts {
        let err = r.aux as u64 * 1000 + 1000;
        assert!(r.b + err >= r.a, "a piece started before its own submit: {r:?}");
        assert!(r.c >= r.b, "a piece ended before it started: {r:?}");
    }
    let mut b: Vec<u64> = starts.iter().map(|r| r.b).collect();
    b.sort_unstable();
    b.dedup();
    assert_eq!(b.len(), starts.len(), "two pieces share a device start (one timestamp set for both jobs)");
}
