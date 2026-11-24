//! t3610 (close-while-busy, found-defects 972): the exit hook's wait is its own. After the close began
//! (`begin_exit`) it still waits for the background work in flight, never to the exit watcher's moment, and
//! says in the trace that it left it; a second call waits for nothing. No device. One process per test.

use kt_time::trace::{Ev, shutdown};
use std::time::{Duration, Instant};

#[test]
fn t3610_the_hook_waits_after_the_close_began_and_under_the_watcher() {
    let guard = kt_gpu::busy().expect("nothing exits yet");
    assert_eq!(kt_time::shutdown::left(Duration::from_secs(10)), Duration::from_secs(10), "no watcher: the step's own bound");
    kt_gpu::begin_exit();
    // the watcher fires in 1.2 s: a second is kept for the end, 0.2 s is the wait's
    let end = Instant::now() + Duration::from_millis(1200);
    kt_time::shutdown::ends_by(end);
    assert!(kt_time::shutdown::ending() && kt_gpu::busy().is_none());
    let items = kt_time::trace::testing::capture(|| {
        kt_gpu::wait_background();
        kt_gpu::wait_background();
    });
    let recs = kt_time::trace::testing::recs(&items);
    let name = kt_time::trace::name("gpu-background-work");
    let waits: Vec<_> = recs.iter().filter(|r| r.ev == Ev::Shutdown as u16 && r.id == shutdown::WAIT && r.aux == name).collect();
    assert_eq!(waits.len(), 1, "one wait");
    assert_eq!(waits[0].b, 1, "the guard still up is named as left");
    // the bound IS the check: less than the watcher's whole moment, where it was 10 s
    assert!(waits[0].dur < 1_200_000_000, "{} ns", waits[0].dur);
    drop(guard);
}
