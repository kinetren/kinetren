//! The exit is bounded once the close's work is safe (close-while-busy, kkt-26.11.18 T1).
//!
//! The owner closed the window while a loop caught up, and the process never ended: he killed it. The
//! close's durability (the autosave in flight, a Ctrl+S being written, the recovery file, the lock) is
//! done first, each wait bounded; from then on nothing the program owes the person is left, and
//! everything after it (a poster, the offline engine's park, the telemetry's last event, the drops
//! that join threads, eframe's teardown, the C exit hooks) gets `EXIT_BOUND` in all. Past it a
//! watcher names in the trace what was still waited for, flushes the trace, and ends the process
//! with `_exit`: the C `exit` would run the hooks that wait for device work (kt-gpu's) and LLVM's
//! destructors under threads still drawing.

use std::time::Duration;

/// From the close's durability to the end of the process, at most: the poster's 3 s, the
/// telemetry's 2 s and the player's bounded joins, with room.
pub const EXIT_BOUND: Duration = Duration::from_secs(8);

/// Arm the watcher: past `bound` it records what the close still waited for and calls `fire`
/// (the product's: `end_now`). Once per process; a second arm is ignored.
pub fn arm(bound: Duration, fire: impl FnOnce() + Send + 'static) {
    static ARMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !ARMED.swap(true, std::sync::atomic::Ordering::AcqRel) && !watch(bound, fire) {
        // no thread: the exit goes unwatched, as before
        ARMED.store(false, std::sync::atomic::Ordering::Release);
    }
}

/// The watcher's thread: past `bound`, the forced exit's record, then `fire`. `false` — no thread.
pub fn watch(bound: Duration, fire: impl FnOnce() + Send + 'static) -> bool {
    std::thread::Builder::new()
        .name("exit-watch".into())
        .spawn(move || {
            // the one clock of a closing process: nothing else wakes this thread
            std::thread::sleep(bound);
            forced();
            fire();
        })
        .is_ok()
}

/// The record of a forced exit: what the close left behind, in the journal and the trace.
fn forced() {
    let left = kt_time::shutdown::left_behind();
    let names = if left.is_empty() { "nothing named".to_string() } else { left.join(", ") };
    kt_time::shutdown::mark(kt_time::trace::shutdown::FORCED, &names);
    crate::tele_journal::record(
        crate::tele_journal::Kind::Note,
        &format!("the exit passed {} s and was forced; still waited for: {names}", EXIT_BOUND.as_secs()),
    );
    crate::journal::flush(Duration::from_millis(500));
    crate::later::flush(Duration::from_millis(500));
    kt_time::trace::flush();
}

/// The product's end of a forced exit: no C exit hooks, no destructors.
pub fn end_now() {
    unsafe extern "C" {
        fn _exit(code: std::ffi::c_int) -> !;
    }
    // SAFETY: `_exit` ends the process; the trace and the journal were flushed by `forced`
    unsafe { _exit(0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t2951 (close-while-busy): past its bound the watcher records the forced exit naming what the
    /// close left behind, flushes the trace, and only then ends the process (here: a test's `fire`).
    #[test]
    fn t2951_a_stuck_exit_is_forced_and_named() {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let items = kt_time::trace::testing::capture(|| {
            let stuck = std::thread::spawn(move || {
                let _ = rx.recv();
            });
            let until = std::time::Instant::now() + Duration::from_millis(20);
            assert!(!kt_time::shutdown::join("t2951-prefetch", stuck, until));
            let (fired_tx, fired) = std::sync::mpsc::channel();
            assert!(watch(Duration::from_millis(50), move || fired_tx.send(()).unwrap()));
            fired.recv_timeout(kt_time::TEST_WAIT).expect("the watcher never fired");
        });
        let _ = tx.send(());
        let recs = kt_time::trace::testing::recs(&items);
        let forced: Vec<_> = recs
            .iter()
            .filter(|r| r.ev == kt_time::trace::Ev::Shutdown as u16 && r.id == kt_time::trace::shutdown::FORCED)
            .collect();
        assert_eq!(forced.len(), 1, "one forced-exit record");
        let names = kt_time::trace::testing::name_of(&items, forced[0].aux);
        assert!(names.contains("t2951-prefetch"), "the forced exit names what it waited for: {names}");
    }
}
