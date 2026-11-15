//! Who is inside a device call that can hold it for long (gl-context-holder).
//!
//! On GL every wgpu-hal call takes the adapter's one context lock, and holds it through a fence wait
//! (`client_wait_sync`), a submission and a pipeline link; the UI thread's paint gives up after six
//! seconds ("Could not lock adapter context") and the program dies. The calls of ours that can hold a
//! device that long register here: `hold(role, op)` keeps the thread's name, the operation and its
//! start while it runs. The editor's panic hook prints what is in flight (`in_flight`), and a hold of
//! the window's device that lasted past `NOTE_AFTER` is written to the sink when it ends, so the
//! journal names the holder even when nothing died.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A hold of the window's device past this is written when it ends.
pub const NOTE_AFTER: Duration = Duration::from_millis(1000);

struct Entry {
    id: u64,
    thread: String,
    role: &'static str,
    op: &'static str,
    since: Instant,
}

static IN_FLIGHT: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A call of ours inside a device; dropped when it returns.
pub struct Hold {
    id: u64,
    role: &'static str,
    op: &'static str,
    since: Instant,
    /// One wgpu call (it holds the device throughout): written when long. Otherwise the span of an
    /// entry point, listed for context only.
    leaf: bool,
}

/// Register ONE wgpu call into a device of `role` (`errors::role`) doing `op`: on GL it holds the
/// adapter's context lock for its whole length, and a long one on the window's device is written.
pub fn hold(role: &'static str, op: &'static str) -> Hold {
    enter(role, op, true)
}

/// Register an entry point's span (several calls, waits between them): listed in flight, never written.
pub fn within(role: &'static str, op: &'static str) -> Hold {
    enter(role, op, false)
}

fn enter(role: &'static str, op: &'static str, leaf: bool) -> Hold {
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let since = Instant::now();
    let thread = std::thread::current().name().unwrap_or("<unnamed>").to_string();
    IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner()).push(Entry { id, thread, role, op, since });
    Hold { id, role, op, since, leaf }
}

impl Drop for Hold {
    fn drop(&mut self) {
        let thread = {
            let mut v = IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
            let at = v.iter().position(|e| e.id == self.id);
            at.map(|i| v.swap_remove(i).thread)
        };
        let took = self.since.elapsed();
        if self.leaf && self.role == crate::errors::role::DISPLAY && took >= NOTE_AFTER {
            crate::errors::emit(&format!(
                "display device held {} ms by thread {} in {}",
                took.as_millis(),
                thread.as_deref().unwrap_or("?"),
                self.op
            ));
        }
    }
}

/// The calls in flight now, oldest first: "<role> device: thread <name> in <op> for <ms> ms".
pub fn in_flight() -> Vec<String> {
    let v = IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
    let mut rows: Vec<&Entry> = v.iter().collect();
    rows.sort_by_key(|e| e.since);
    rows.iter()
        .map(|e| format!("{} device: thread {} in {} for {} ms", e.role, e.thread, e.op, e.since.elapsed().as_millis()))
        .collect()
}

/// Tests and the live bench (gl-context-holder): hold the GL context lock of `device` until `release`
/// answers — what a GL call of ours stalled by the system does to every other caller of that
/// device. `false` — not a GL device (nothing held).
#[doc(hidden)]
pub fn hold_gl_context(device: &wgpu::Device, release: impl FnOnce()) -> bool {
    // SAFETY: the hal device is only borrowed to take its context lock; nothing is created through it
    let Some(hal) = (unsafe { device.as_hal::<wgpu::hal::api::Gles>() }) else { return false };
    let _held = hal.context().lock();
    release();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t2740 (gl-context-holder): a call in flight is listed with its thread, role and operation,
    /// oldest first, and leaves the list when it returns.
    #[test]
    fn t2740_a_call_in_flight_names_its_thread_and_operation() {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let a = std::thread::Builder::new()
            .name("t2740-holder".into())
            .spawn(move || {
                let h = hold(crate::errors::role::HEADLESS, "t2740 wait");
                let _ = wait.recv();
                drop(h);
            })
            .unwrap();
        let deadline = kt_time::test_deadline();
        let mine = |l: &String| l.contains("t2740-holder") && l.contains("t2740 wait");
        while !in_flight().iter().any(mine) {
            assert!(Instant::now() < deadline, "the hold never showed");
            std::thread::yield_now();
        }
        let row = in_flight().into_iter().find(mine).unwrap();
        assert!(row.starts_with("headless device: thread t2740-holder in t2740 wait for "), "{row}");
        release.send(()).unwrap();
        a.join().unwrap();
        assert!(!in_flight().iter().any(mine), "a returned call leaves the list");
    }
}
