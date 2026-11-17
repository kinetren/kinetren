//! The close is bounded and says what it waited for (close-while-busy, kkt-26.11.18 T1).
//!
//! The owner closed the window while a loop caught up and had to kill the process: a prefetch pass
//! walked a Datamosh history for up to 37 s, and the player's drop joined it; the sound device's
//! thread, the effect-cell lane and the GPU exit hook waited with no bound either. And the trace
//! could not say so: nothing recorded the close. Every wait of a close goes through here: it waits
//! up to a deadline, leaves what is still running behind, and writes a `Shutdown` record (who, how
//! long, ended or left). What is left behind is named again by a forced exit.

use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::trace::{self, Ev};

/// How long one owner's drop waits for its threads (the player's three threads and its decoders,
/// the sound device's thread, a service's running job). A thread past it is left behind: it ends
/// by itself at its next stop check, or with the process.
pub const WAIT: Duration = Duration::from_secs(2);

/// What a close left running, by name (a forced exit names them).
static LEFT: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// The deadline of a wait that starts now.
pub fn deadline() -> Instant {
    Instant::now() + WAIT
}

/// Record a wait of the close that started at `since`: `ended` — what it waited for ended.
pub fn record(who: &'static str, since: Instant, ended: bool) {
    let ns = since.elapsed().as_nanos() as u64;
    if !ended {
        LEFT.lock().unwrap_or_else(|e| e.into_inner()).push(who);
    }
    if trace::on() {
        trace::ev(Ev::Shutdown)
            .id(trace::shutdown::WAIT)
            .aux(trace::name(who))
            .a(ns)
            .b(!ended as u64)
            .lasted(ns)
            .emit();
    }
}

/// Record a moment of the close (`trace::shutdown::ASKED`, `DURABLE`, `FORCED`) with `who`.
pub fn mark(kind: u64, who: &str) {
    if trace::on() {
        trace::ev(Ev::Shutdown).id(kind).aux(trace::name(who)).emit();
    }
}

/// Wait until `done` says so, at most until `until` (polled every millisecond: a close is no
/// service at rest). Recorded as `who`. `true` — it is done.
pub fn wait_until(who: &'static str, until: Instant, mut done: impl FnMut() -> bool) -> bool {
    let since = Instant::now();
    let ended = loop {
        if done() {
            break true;
        }
        if Instant::now() >= until {
            break false;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    record(who, since, ended);
    ended
}

/// Join `h` if it ends by `until`; else leave it running (detached). Recorded as `who`.
pub fn join<T>(who: &'static str, h: JoinHandle<T>, until: Instant) -> bool {
    let ended = wait_until(who, until, || h.is_finished());
    if ended {
        let _ = h.join();
    }
    ended
}

/// What closes so far left running (a forced exit names them), in order.
pub fn left_behind() -> Vec<&'static str> {
    LEFT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t2950 (close-while-busy): a join that waits past its bound leaves the thread behind within the
    /// bound and records it as left; a thread that ends is joined and recorded as ended.
    #[test]
    fn t2950_a_close_wait_is_bounded_and_recorded() {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let items = trace::testing::capture(|| {
            let stuck = std::thread::spawn(move || {
                let _ = rx.recv();
            });
            let t0 = Instant::now();
            assert!(!join("t2950-stuck", stuck, Instant::now() + Duration::from_millis(50)));
            // the bound IS the check here (a close that waits forever is the defect)
            assert!(t0.elapsed() < Duration::from_secs(5), "the wait passed its bound: {:?}", t0.elapsed());
            let quick = std::thread::spawn(|| ());
            assert!(join("t2950-quick", quick, crate::test_deadline()));
        });
        let _ = tx.send(());
        let recs = trace::testing::recs(&items);
        let of = |who: &str| recs.iter().find(|r| r.ev == Ev::Shutdown as u16 && r.aux == trace::name(who)).copied();
        let stuck = of("t2950-stuck").expect("the left wait is in the trace");
        assert_eq!((stuck.id, stuck.b), (trace::shutdown::WAIT, 1));
        assert!(stuck.dur >= 50_000_000, "it waited its bound: {}", stuck.dur);
        assert_eq!(of("t2950-quick").expect("the joined wait is in the trace").b, 0);
        assert!(left_behind().contains(&"t2950-stuck"));
    }
}
