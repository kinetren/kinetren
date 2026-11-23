//! t3604 (close-while-busy, found-defects 962): once the close is bounded (`begin_exit`), background GPU
//! work stops between its steps. An engine built on a thread then gives up between two pipelines instead
//! of compiling the whole set, and no background work is left for the exit hook to wait out — a stream
//! front's engine compiling for seconds under load held the exit past the close's bound, and the exit's
//! watcher forced the end. One process per test (nextest): the flag is the process's. Without an
//! adapter: skip.

#[test]
fn t3604_a_bounded_close_stops_an_engine_build() {
    kt_plugins_builtin::install();
    let Ok(ctx) = kt_gpu::GpuContext::new_headless() else {
        assert!(std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1"), "KT_REQUIRE_GPU=1, but no device");
        eprintln!("skipped: no device");
        return;
    };
    // the close becomes bounded while the build compiles (its guard is up)
    use std::sync::atomic::{AtomicBool, Ordering};
    static BUILT: AtomicBool = AtomicBool::new(false);
    let watcher = std::thread::spawn(|| {
        let deadline = kt_time::test_deadline();
        while kt_gpu::background_gpu_work() == 0 && !BUILT.load(Ordering::Acquire) {
            assert!(std::time::Instant::now() < deadline, "the build never started");
            std::thread::yield_now();
        }
        let late = BUILT.load(Ordering::Acquire);
        kt_gpu::begin_exit();
        late
    });
    let built = kt_gpu::GpuFxEngine::new(ctx);
    BUILT.store(true, Ordering::Release);
    let late = watcher.join().unwrap();
    match built {
        Err(e) => assert!(e.to_string().contains("exiting"), "the build gave up for the exit: {e}"),
        Ok(_) if late => {
            eprintln!("skipped: the build ended before the close began");
            return;
        }
        Ok(_) => panic!("an engine was built whole after the close was bounded mid-build"),
    }
    assert_eq!(kt_gpu::background_gpu_work(), 0, "nothing left for the exit hook to wait for");
}
