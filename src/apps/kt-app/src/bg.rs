//! Background TASK — the common framework (R.49, item 367).
//!
//! **The distinction everything follows from.** The program has TWO forms of background
//! work, and confusing them is costly:
//!
//! - **task** — finite, with progress, cancellation and pause; it has an end,
//!   and the user is entitled to wait for it. The reference — rendering;
//! - **service** — an endless stream with a queue, which lives as long as
//!   the project lives. The reference — the thumbnail service.
//!
//! `spawn_index_worker` was a task PRETENDING to be a service: each call
//! started a new thread, and on one project three of them piled up, and they kept writing
//! into orphaned `Arc`s that nobody read any more.
//!
//! The framework was extracted FROM `RenderJob`, not written alongside it: a copy would diverge
//! from the original at the first edit.
//!
//! **`Drop` stops the thread.** This is a cure for the disease, not a convenience: until
//! `WatchdogHandle` a project re-open left the previous threads alive forever.
//! The thread checks the flag itself — it cannot be interrupted from outside, and this is honest:
//! a task that never asks “is it time to stop?” simply cannot be stopped
//! at all.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The progress sender. It is OWNED by the task body, not lent to it:
/// consumers (the same export) require a `Fn(..) + Send + Sync +
/// 'static` callback and store it, and a reference cannot make such a promise.
pub struct BgSend<P>(std::sync::mpsc::Sender<P>);

impl<P> Clone for BgSend<P> {
    fn clone(&self) -> Self {
        BgSend(self.0.clone())
    }
}

impl<P> BgSend<P> {
    /// Send a step. A closed channel is not an error: it means the task was forgotten,
    /// and it is about to stop anyway by the cancel flag.
    pub fn send(&self, p: P) {
        let _ = self.0.send(p);
    }
}

/// The levers that the task MUST poll itself.
#[derive(Clone)]
pub struct BgCtl {
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
}

impl BgCtl {
    /// “Is it time to finish”. Also WAITS until the pause is lifted: a pause is not
    /// a stop, and the one asking about cancellation must still respect it.
    pub fn should_stop(&self) -> bool {
        while self.pause.load(Ordering::Acquire) && !self.cancel.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        self.cancel.load(Ordering::Acquire)
    }

    pub fn cancel_flag(&self) -> &Arc<AtomicBool> {
        &self.cancel
    }

    pub fn pause_flag(&self) -> &Arc<AtomicBool> {
        &self.pause
    }
}

/// A running background task: `P` — progress step, `R` — result.
pub struct BgJob<P, R> {
    prog_rx: std::sync::mpsc::Receiver<P>,
    done_rx: std::sync::mpsc::Receiver<R>,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    pub started: std::time::Instant,
    /// The last progress step received.
    pub progress: Option<P>,
    /// The result. `Some` — the task is finished.
    pub done: Option<R>,
}

impl<P: Send + 'static, R: Send + 'static> BgJob<P, R> {
    /// Start. `body` gets the levers and the “progress sender”.
    pub fn spawn<F>(name: &str, body: F) -> BgJob<P, R>
    where
        F: FnOnce(BgCtl, BgSend<P>) -> R + Send + 'static,
    {
        let cancel = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        let (prog_tx, prog_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let ctl = BgCtl { cancel: cancel.clone(), pause: pause.clone() };
        std::thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                let r = body(ctl, BgSend(prog_tx));
                let _ = done_tx.send(r);
            })
            .expect("spawn background job");
        BgJob {
            prog_rx,
            done_rx,
            cancel,
            pause,
            started: std::time::Instant::now(),
            progress: None,
            done: None,
        }
    }

    /// Collect everything that has arrived. `true` — something changed (a reason to redraw a frame).
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok(p) = self.prog_rx.try_recv() {
            any = true;
            self.progress = Some(p);
        }
        if let Ok(r) = self.done_rx.try_recv() {
            any = true;
            self.done = Some(r);
        }
        any
    }

    /// The task is still counting. It is exactly this, and not `is_some()` of the job itself,
    /// that must keep the repaint going: otherwise a window left on screen after
    /// completion would heat the system round the clock (a repeat of the t38 defect).
    pub fn is_running(&self) -> bool {
        self.done.is_none()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    pub fn set_paused(&self, on: bool) {
        self.pause.store(on, Ordering::Release);
    }

    pub fn is_paused(&self) -> bool {
        self.pause.load(Ordering::Acquire)
    }
}

impl<P, R> Drop for BgJob<P, R> {
    fn drop(&mut self) {
        // the task was forgotten — the task must finish
        self.cancel.store(true, Ordering::Release);
        self.pause.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t398 (R.49 B1): the task framework — progress arrives, the pause holds,
    /// cancel stops, and `Drop` stops the thread even if the task was
    /// simply forgotten.
    #[test]
    fn t398_bg_job_reports_pauses_cancels_and_dies_with_drop() {
        // 1) progress and result
        let mut job: BgJob<u32, u32> = BgJob::spawn("t398-a", |_ctl, send: BgSend<u32>| {
            for i in 1..=5 {
                send.send(i);
            }
            42
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while job.done.is_none() && std::time::Instant::now() < deadline {
            job.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(job.done, Some(42), "the total did not arrive");
        assert_eq!(job.progress, Some(5), "progress did not arrive");
        assert!(!job.is_running(), "a finished task counts as running");

        // 2) cancel: the body asks `should_stop` and exits
        let steps = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let s = steps.clone();
        let job2: BgJob<(), &'static str> =
            BgJob::spawn("t398-b", move |ctl, _send: BgSend<()>| {
                loop {
                    if ctl.should_stop() {
                        return "stopped";
                    }
                    s.fetch_add(1, Ordering::Release);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            });
        std::thread::sleep(std::time::Duration::from_millis(40));
        job2.cancel();
        let mut job2 = job2;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while job2.done.is_none() && std::time::Instant::now() < deadline {
            job2.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(job2.done, Some("stopped"), "the cancelled task did not finish");

        // 3) Drop stops the thread: the task was forgotten — it must stop
        let ticks = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let t = ticks.clone();
        {
            let _job3: BgJob<(), ()> = BgJob::spawn("t398-c", move |ctl, _send: BgSend<()>| {
                while !ctl.should_stop() {
                    t.fetch_add(1, Ordering::Release);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            });
            std::thread::sleep(std::time::Duration::from_millis(40));
        } // here Drop
        std::thread::sleep(std::time::Duration::from_millis(80));
        let a = ticks.load(Ordering::Acquire);
        std::thread::sleep(std::time::Duration::from_millis(120));
        assert_eq!(
            ticks.load(Ordering::Acquire),
            a,
            "after Drop the thread keeps spinning — exactly the leak for which \
             the harness is set up"
        );
    }

    /// The PAUSE holds, and cancel releases it and lets the thread go: otherwise a task
    /// cancelled while paused would hang forever, and with it the progress window.
    #[test]
    fn t398b_cancel_releases_a_paused_job() {
        let ticks = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let t = ticks.clone();
        let mut job: BgJob<(), u32> = BgJob::spawn("t398-d", move |ctl, _send: BgSend<()>| {
            let mut n = 0;
            while !ctl.should_stop() {
                n += 1;
                t.fetch_add(1, Ordering::Release);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            n
        });
        std::thread::sleep(std::time::Duration::from_millis(40));
        job.set_paused(true);
        std::thread::sleep(std::time::Duration::from_millis(60));
        let held = ticks.load(Ordering::Acquire);
        std::thread::sleep(std::time::Duration::from_millis(80));
        assert_eq!(ticks.load(Ordering::Acquire), held, "the pause does not hold");

        job.cancel();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while job.done.is_none() && std::time::Instant::now() < deadline {
            job.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(job.done.is_some(), "a task cancelled during pause did not exit — the window would hang forever");
    }
}
