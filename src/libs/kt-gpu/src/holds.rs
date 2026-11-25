//! Who is inside a device call that can hold it for long (gl-context-holder).
//!
//! On GL every wgpu-hal call takes the adapter's one context lock, and holds it through a fence wait
//! (`client_wait_sync`), a submission and a pipeline link; the UI thread's paint gives up after six
//! seconds ("Could not lock adapter context") and the program dies. The calls of ours that can hold a
//! device that long register here: `hold(role, op)` keeps the thread's name, the operation and its
//! start while it runs. The editor's panic hook prints what is in flight (`in_flight`), and a hold of
//! the window's device that lasted past `NOTE_AFTER` is written to the sink when it ends, so the
//! journal names the holder even when nothing died.
//!
//! A frame that waits says for what (gpu-waits-named): each such call, each lock of a frame's path
//! ([`lock`]) and each file touched by this crate ([`file`]) past `trace::GPU_WAIT_NOTED` is an
//! `Ev::GpuWait` record. Without them a composite that stood 4.8 s was a span with nothing inside it.

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use kt_time::trace;

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
        if self.leaf {
            note_wait(self.op, trace::gpu_wait::CALL, Some(self.role), took);
        }
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

/// `took` was spent waiting for `what` and ends now: a trace record past the threshold.
pub(crate) fn note_wait(what: &str, kind: u64, role: Option<&str>, took: Duration) {
    if took < trace::GPU_WAIT_NOTED || !trace::on() {
        return;
    }
    let ns = took.as_nanos() as u64;
    trace::ev(trace::Ev::GpuWait)
        .at(trace::now_ns().saturating_sub(ns))
        .aux(trace::name(what))
        .a(kind)
        .b(role.map_or(0, |r| trace::name(r) as u64))
        .lasted(ns)
        .emit();
}

/// Take a lock of a frame's path; a wait for it is named `what`.
pub(crate) fn lock<'a, T>(m: &'a Mutex<T>, what: &str, role: Option<&str>) -> MutexGuard<'a, T> {
    match m.try_lock() {
        Ok(g) => return g,
        Err(std::sync::TryLockError::Poisoned(e)) => return e.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => {}
    }
    let t0 = Instant::now();
    let g = m.lock().unwrap_or_else(|e| e.into_inner());
    note_wait(what, trace::gpu_wait::LOCK, role, t0.elapsed());
    g
}

/// A file of this crate read or written (the pass times, the pipeline cache): its length is named.
pub(crate) fn file<R>(what: &str, f: impl FnOnce() -> R) -> R {
    let t0 = Instant::now();
    let r = f();
    note_wait(what, trace::gpu_wait::FILE, None, t0.elapsed());
    r
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

    /// Test t3251 (gpu-waits-named, found-defects 835): a lock somebody else held, a device call and a file
    /// past the threshold are records that name what was waited for and the device's role; a free lock and
    /// a quick call leave nothing.
    #[test]
    fn t3251_a_wait_past_the_threshold_is_a_record_that_names_it() {
        use kt_time::trace::{gpu_wait, testing};
        let m = Mutex::new(0);
        let items = testing::capture(|| {
            drop(lock(&m, "t3251 free", None));
            let (held, is_held) = std::sync::mpsc::channel();
            let (asking, asked) = std::sync::mpsc::channel();
            std::thread::scope(|s| {
                let m = &m;
                s.spawn(move || {
                    let _g = m.lock().unwrap();
                    held.send(()).unwrap();
                    asked.recv().unwrap();
                    std::thread::sleep(Duration::from_millis(200));
                });
                is_held.recv().unwrap();
                asking.send(()).unwrap();
                drop(lock(m, "t3251 lock", Some(crate::errors::role::PREVIEW)));
            });
            file("t3251 file", || std::thread::sleep(Duration::from_millis(2)));
            let h = hold(crate::errors::role::HEADLESS, "t3251 call");
            std::thread::sleep(Duration::from_millis(2));
            drop(h);
            drop(within(crate::errors::role::HEADLESS, "t3251 span"));
        });
        let mine: Vec<(String, u64, String)> = testing::recs(&items)
            .iter()
            .filter(|r| r.ev == trace::Ev::GpuWait as u16 && r.dur > 0)
            .map(|r| (testing::name_of(&items, r.aux), r.a, testing::name_of(&items, r.b as u32)))
            .filter(|r| r.0.starts_with("t3251"))
            .collect();
        let want = [
            ("t3251 lock", gpu_wait::LOCK, crate::errors::role::PREVIEW),
            ("t3251 file", gpu_wait::FILE, ""),
            ("t3251 call", gpu_wait::CALL, crate::errors::role::HEADLESS),
        ];
        assert_eq!(mine, want.map(|(n, k, r)| (n.to_string(), k, r.to_string())));
    }
}
