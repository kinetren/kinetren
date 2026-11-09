//! Nothing on the device when the process exits (gpu-render-hardening D6, pipeline-prewarm D10).
//! `exit` runs libLLVM's destructors while llvmpipe's threads may still compile or draw, and LLVM
//! segfaults. The frames are synchronous and the editor parks the offline engine in `on_exit`; the
//! background GPU work — the warm worker, its shadow device, the compute host's preparation, an
//! engine built on a thread — is not, and a test process returning while a warm-up compiled died by
//! SIGSEGV after "test result: ok" (row 172). Such work holds a `Busy` guard; at the first guard an
//! `atexit` hook is installed that stops new work and waits for the guards in flight.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, Once};

static BUSY: Mutex<usize> = Mutex::new(0);
static DONE: Condvar = Condvar::new();
static EXITING: AtomicBool = AtomicBool::new(false);
static HOOK: Once = Once::new();

unsafe extern "C" {
    fn atexit(cb: extern "C" fn()) -> std::ffi::c_int;
}

extern "C" fn at_exit() {
    EXITING.store(true, Ordering::Release);
    let mut n = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    while *n > 0 {
        n = DONE.wait(n).unwrap_or_else(|e| e.into_inner());
    }
}

/// The process is exiting: background GPU work starts nothing more.
pub(crate) fn exiting() -> bool {
    EXITING.load(Ordering::Acquire)
}

/// Background GPU work in flight (tests).
pub fn background_gpu_work() -> usize {
    *BUSY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Background GPU work: the exit waits for it to end.
pub(crate) struct Busy(());

impl Drop for Busy {
    fn drop(&mut self) {
        let mut n = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        *n -= 1;
        if *n == 0 {
            DONE.notify_all();
        }
    }
}

/// Enter background GPU work; `None` once the process is exiting.
pub(crate) fn busy() -> Option<Busy> {
    HOOK.call_once(|| {
        // SAFETY: `at_exit` is a plain C function of the process, registered once
        let _ = unsafe { atexit(at_exit) };
    });
    let mut n = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    if exiting() {
        return None;
    }
    *n += 1;
    Some(Busy(()))
}
