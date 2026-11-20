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

// ---- the end of the process: before the native libraries' destructors (found-defects 784/785) ----
//
// `exit` runs every hook registered before it, newest first: the destructors of the native libraries
// the process loaded (libLLVM under llvmpipe, a libva driver, the C++ statics of a GPU driver), while
// the threads a bounded close left behind may still draw, compile or decode in them. A kt-app test
// process died so after its test passed ("double free or corruption", SIGSEGV), and row 172 before
// it (LLVM under a warm-up compile). [`end_before_libraries`] is called right after such a library is
// loaded: its hook is then registered AFTER that library's destructors, so it runs before them — it
// runs the program's own last steps ([`before_exit`]), flushes C's stdio, and ends the process with
// `_exit` and the status `exit` was given. Nothing a library frees at exit is the program's: the
// memory goes with the process.

type Last = std::sync::Mutex<Vec<fn()>>;
static LAST: Last = std::sync::Mutex::new(Vec::new());
static ARMED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// A last step of the process (the GPU's background work waited for, the trace flushed): run once
/// by the hook of [`end_before_libraries`], in the order registered. Registering one twice runs it once.
pub fn before_exit(step: fn()) {
    let mut l = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if !l.iter().any(|f| std::ptr::fn_addr_eq(*f, step)) {
        l.push(step);
    }
}

/// How many times the end was registered (tests; 0 — the process ends by the C `exit` as before).
pub fn end_registrations() -> u32 {
    ARMED.load(std::sync::atomic::Ordering::Acquire)
}

#[cfg(target_os = "linux")]
mod end {
    use std::ffi::{c_int, c_void};
    unsafe extern "C" {
        fn on_exit(f: extern "C" fn(c_int, *mut c_void), arg: *mut c_void) -> c_int;
        fn fflush(f: *mut c_void) -> c_int;
        fn _exit(code: c_int) -> !;
    }

    extern "C" fn end(status: c_int, _: *mut c_void) {
        let steps: Vec<fn()> = super::LAST.lock().unwrap_or_else(|e| e.into_inner()).clone();
        for s in steps {
            s();
        }
        super::super::trace::flush();
        // SAFETY: plain C calls at the end of the process; `fflush(NULL)` flushes every stream
        unsafe {
            fflush(core::ptr::null_mut());
            _exit(status)
        }
    }

    pub(super) fn register() -> bool {
        // SAFETY: `end` is a plain C function of the process
        unsafe { on_exit(end, core::ptr::null_mut()) == 0 }
    }
}

/// Make the process end before the destructors of every native library loaded so far (see above).
/// Called by the code that loads one — kt-gpu after it opens a device, kt-ffmpeg after it loads
/// FFmpeg and after a hardware device opens. Each call registers the end again, so it stands before
/// the libraries loaded since; registering is cheap, but a caller calls once per library it loads,
/// never per frame. Linux (glibc's `on_exit`); elsewhere nothing changes.
pub fn end_before_libraries() {
    #[cfg(target_os = "linux")]
    if end::register() {
        ARMED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }
}
