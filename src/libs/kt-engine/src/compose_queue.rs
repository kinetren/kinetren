//! The preview's own thread (ui-never-waits; kkt-26.11.07 item E8: an anomalous parameter lagged
//! the interface until the effect could not be undone; kkt-26.11.08 H9: the UI thread stood 0.3–4.5 s
//! inside single frames at a quarter of a core, waiting for a Datamosh stream stage's compute jobs
//! inside the plan). The UI asks for a frame and goes on: the
//! newest ask replaces the waiting one, one composite runs at a time, and what comes out for a
//! state the model has left is abandoned. So a chain that takes seconds costs frames, never the
//! Undo. A composite cannot be stopped inside the device, so "cancelled" means nobody reads it.
//!
//! Two devices, because a thread alone is not enough: on the display's device a slow chain stands
//! in the queue in front of the UI's own frame, and the swapchain then waits for it. So the first
//! frame of each new project state goes to the offline device and is timed; a chain that passes
//! `SLOW_CHAIN` stays there, and the picture comes back as bytes. A fast chain goes on to the
//! display target with no readback. Only the executor's time counts: a stream stage runs inside
//! the plan, its compute jobs on a device of their own, and it holds no display queue.
//!
//! Between frames the thread also checks the kept moments against an edit (`recheck_shown`): that
//! builds plans, and the bars ask for it on the UI thread.

use crate::player::{PreviewParts, Target, VideoCost};
use crate::video_plan::{PresentOptions, VideoGpuExec};
use kt_media::Quality;
use kt_time::Tick;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Past this a chain leaves the display's device. A UI frame behind it would be seen as a stall.
pub const SLOW_CHAIN: Duration = Duration::from_millis(150);

struct Ask {
    t: Tick,
    present: Option<PresentOptions>,
    seq: u64,
    versions: (u64, u64),
}

/// A finished preview frame.
#[derive(Debug)]
pub struct Composed {
    /// The ask it answers (`Player::ask_preview`).
    pub seq: u64,
    pub t: Tick,
    pub quality: Quality,
    pub complete: bool,
    /// The picture is on the executor's display target; else in `rgba`.
    pub on_display: bool,
    pub rgba: Vec<u8>,
    /// The positions a stream stage of the frame still lacks (0 — none): the frame is incomplete, and
    /// the preview writes "catching up" where it writes Loading (catch-up-shows-loading).
    pub catching_up: u64,
}

type Wake = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct Composer {
    ask: Mutex<Option<Ask>>,
    ask_cv: Condvar,
    done: Mutex<Option<Composed>>,
    done_cv: Condvar,
    spare: Mutex<Vec<u8>>,
    stop: AtomicBool,
    wake: Mutex<Option<Wake>>,
    slow: Mutex<Option<Arc<dyn VideoGpuExec>>>,
    /// The bars found kept moments presented before an edit: check them between frames.
    nudged: AtomicBool,
}

impl Composer {
    pub(crate) fn new() -> Arc<Composer> {
        Arc::new(Composer {
            ask: Mutex::new(None),
            ask_cv: Condvar::new(),
            done: Mutex::new(None),
            done_cv: Condvar::new(),
            spare: Mutex::new(Vec::new()),
            stop: AtomicBool::new(false),
            wake: Mutex::new(None),
            slow: Mutex::new(None),
            nudged: AtomicBool::new(false),
        })
    }

    /// The newest ask replaces the waiting one. Its number comes back: the frame's id in the trace
    /// (process-wide, so two players never share one).
    pub(crate) fn ask(&self, t: Tick, present: Option<PresentOptions>, versions: (u64, u64), flags: u32) -> u64 {
        use kt_time::trace::{self, Ev};
        let seq = trace::next_id();
        let old = self.ask.lock().unwrap().replace(Ask { t, present, seq, versions });
        self.ask_cv.notify_one();
        let replaced = old.map_or(0, |o| o.seq);
        trace::ev(Ev::FrameAsk).id(seq).a(t.0 as u64).b(replaced).aux(flags).emit();
        if replaced != 0 {
            trace::ev(Ev::FrameDrop).id(replaced).aux(trace::drop::SUPERSEDED).emit();
        }
        seq
    }

    /// The newest finished frame, waiting for it at most `wait`.
    pub(crate) fn take(&self, wait: Duration) -> Option<Composed> {
        let mut g = self.done.lock().unwrap();
        if g.is_none() && !wait.is_zero() {
            g = self.done_cv.wait_timeout_while(g, wait, |d| d.is_none()).unwrap().0;
        }
        g.take()
    }

    pub(crate) fn give_back(&self, buf: Vec<u8>) {
        *self.spare.lock().unwrap() = buf;
    }

    /// Wake the UI for news of another thread (the probe's renderer arrived).
    pub(crate) fn wake_ui(&self) {
        wake(self);
    }

    pub(crate) fn set_wake(&self, f: Wake) {
        *self.wake.lock().unwrap() = Some(f);
    }

    pub(crate) fn set_slow(&self, exec: Option<Arc<dyn VideoGpuExec>>) {
        *self.slow.lock().unwrap() = exec;
    }

    /// Ask for a check of the kept moments (`recheck_shown`) when no frame is waiting.
    pub(crate) fn nudge(&self) {
        if !self.nudged.swap(true, Ordering::AcqRel) {
            let _g = self.ask.lock().unwrap();
            self.ask_cv.notify_one();
        }
    }

    /// The flag is set and the waiter notified UNDER the ask lock (play-start-and-prefetch D2): set
    /// between the waiter's look at it and its wait, an unlocked notify was lost and the thread
    /// parked for good, holding the player's preview parts (it is not joined).
    pub(crate) fn stop(&self) {
        let _g = self.ask.lock().unwrap();
        self.stop.store(true, Ordering::Release);
        self.ask_cv.notify_all();
    }

    /// The next ask, waited for: `Some(None)` — a nudge with no ask, `None` — stopped. `between` runs
    /// after each look at the state, before the wait (a test's seam; the product passes nothing).
    fn next_ask(&self, between: &dyn Fn()) -> Option<Option<Ask>> {
        let mut g = self.ask.lock().unwrap();
        loop {
            if self.stop.load(Ordering::Acquire) {
                return None;
            }
            if let Some(a) = g.take() {
                return Some(Some(a));
            }
            if self.nudged.swap(false, Ordering::AcqRel) {
                return Some(None);
            }
            between();
            g = self.ask_cv.wait(g).unwrap();
        }
    }
}

/// Where the current project state composes.
#[derive(Default)]
struct Route {
    version: u64,
    /// It has been timed on the offline device.
    probed: bool,
    slow: bool,
}

impl Route {
    fn through_slow(&mut self, version: u64) -> bool {
        if self.version != version {
            *self = Route { version, probed: false, slow: false };
        }
        !self.probed || self.slow
    }

    fn timed(&mut self, spent: Duration) {
        self.probed = true;
        self.slow = spent >= SLOW_CHAIN;
    }
}

pub(crate) fn spawn(composer: Arc<Composer>, parts: PreviewParts) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("preview-compose".into())
        .spawn(move || run(composer, parts))
        .expect("spawn the preview's thread")
}

/// Kept moments checked per pass between frames: a plan with a stream stage runs its kernel.
const RECHECK_PER_PASS: usize = 8;

fn run(c: Arc<Composer>, parts: PreviewParts) {
    // the frame the person looks at: its stage runs and slow-route frames come first on every ring
    // (gpu-arbiter D5); the kept-moment recheck between frames runs as near
    let _class = kt_time::sched::with_class(kt_time::sched::Class::Now);
    let mut route = Route::default();
    let mut checked_gen = u64::MAX;
    loop {
        let Some(ask) = c.next_ask(&|| {}) else { return };
        let Some(ask) = ask else {
            recheck(&c, &parts, &mut checked_gen);
            continue;
        };
        let t0 = Instant::now();
        // every record of this composite, on this thread and in the requests it makes, names the frame
        let _cause = kt_time::trace::with_cause(ask.seq);
        let mut rgba = std::mem::take(&mut *c.spare.lock().unwrap());
        let display = ask.present.filter(|_| parts.can_present());
        let slow = c.slow.lock().unwrap().clone();
        let via = slow.filter(|_| display.is_some() && route.through_slow(ask.versions.0));
        let timed = via.is_some();
        let mut on_display = display.is_some() && via.is_none();
        // a stream history this frame runs stops once nobody wants the frame (gpu-arbiter, row 147):
        // a newer ask for another moment, an edit, the composer's end
        let left = {
            let (c, (pv, cv), t, versions) = (c.clone(), parts.version_cells(), ask.t, ask.versions);
            move || {
                c.stop.load(Ordering::Acquire)
                    || (pv.load(Ordering::Acquire), cv.load(Ordering::Acquire)) != versions
                    || c.ask.lock().unwrap().as_ref().is_some_and(|a| a.t != t)
            }
        };
        let out = crate::video::stop_when(kt_time::trace::stop::COMPOSE, left, || match display.filter(|_| on_display) {
            Some(o) => {
                let r = parts.compose(ask.t, Target::Display(o));
                let w = Instant::now();
                parts.wait_presented();
                let waited = w.elapsed().as_nanos() as u64;
                kt_time::trace::ev(kt_time::trace::Ev::FrameWait)
                    .id(ask.seq)
                    .aux(kt_time::trace::wait::DEVICE)
                    .lasted(waited)
                    .emit();
                r
            }
            None => parts.compose_via(via.as_ref(), display, ask.t, Target::Bytes(&mut rgba)),
        });
        let spent = t0.elapsed();
        // the executor's own time: the plan (a stream stage's kernel included) holds no queue
        let plan = out.as_ref().map_or(0, |o| o.4);
        let exec = spent.saturating_sub(Duration::from_nanos(plan));
        // a kept composite was shown on the display: it timed no chain (structured-telemetry)
        let replayed = out.as_ref().is_some_and(|o| o.5.replayed);
        let catching_up = out.as_ref().map_or(0, |o| o.5.stream_short);
        let shown_kept = out.as_ref().is_some_and(|o| o.5.shown_kept);
        if replayed {
            on_display = display.is_some();
        } else if timed {
            route.timed(exec);
            // the slow route's picture was shown from the display's memo (row 622)
            on_display = shown_kept && display.is_some();
        } else if on_display {
            route.slow = exec >= SLOW_CHAIN;
        }
        let answer = out.map(|(quality, complete, gpu, cost, plan_ns, stages)| {
            parts.note_cost(VideoCost { gpu, cost, compose_ns: spent.as_nanos() as u64, plan_ns, stages });
            (quality, complete)
        });
        // a picture of a state the model has left is nobody's: an Undo must not see it land
        if parts.versions() != ask.versions {
            kt_time::trace::ev(kt_time::trace::Ev::FrameDrop).id(ask.seq).aux(kt_time::trace::drop::STATE_LEFT).emit();
            c.give_back(rgba);
            continue;
        }
        if answer.is_none() {
            kt_time::trace::ev(kt_time::trace::Ev::FrameDrop).id(ask.seq).aux(kt_time::trace::drop::NO_PICTURE).emit();
        }
        let (quality, complete) = answer.unwrap_or((parts.quality(), false));
        kt_time::trace::ev(kt_time::trace::Ev::FrameDone)
            .id(ask.seq)
            .a(ask.t.0 as u64)
            .aux(complete as u32)
            .lasted(spent.as_nanos() as u64)
            .emit();
        let frame = Composed { seq: ask.seq, t: ask.t, quality, complete, on_display, rgba, catching_up };
        if let Some(old) = c.done.lock().unwrap().replace(frame) {
            kt_time::trace::ev(kt_time::trace::Ev::FrameDrop).id(old.seq).aux(kt_time::trace::drop::UNTAKEN).emit();
            c.give_back(old.rgba);
        }
        c.done_cv.notify_all();
        wake(&c);
        // a frame kept on the display may have pushed older ones out of the executor's memo
        if on_display && parts.shown_gen() != checked_gen {
            recheck(&c, &parts, &mut checked_gen);
        }
    }
}

fn wake(c: &Composer) {
    let wake = c.wake.lock().unwrap().clone();
    if let Some(w) = wake {
        w();
    }
}

/// One pass of `recheck_shown`, yielding to a frame that is asked meanwhile.
fn recheck(c: &Composer, parts: &PreviewParts, checked_gen: &mut u64) {
    let _class = kt_time::sched::with_class(kt_time::sched::Class::Near);
    let id = kt_time::trace::next_id();
    let _cause = kt_time::trace::with_cause(id);
    let mut span = kt_time::trace::span(kt_time::trace::Ev::Job);
    span.aux(kt_time::trace::job::RECHECK_SHOWN).id(id);
    let (changed, pending) =
        parts.recheck_shown(RECHECK_PER_PASS, || c.ask.lock().unwrap().is_some() || c.stop.load(Ordering::Acquire));
    *checked_gen = parts.shown_gen();
    if pending {
        c.nudged.store(true, Ordering::Release);
    }
    if changed || pending {
        wake(c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test t2640 (play-start-and-prefetch D2): a stop that lands between the compose thread's look
    /// at the flag and its wait still ends the wait. The seam holds the thread there; an unlocked
    /// notify was lost and the thread parked for good.
    #[test]
    fn t2640_a_stop_between_the_look_and_the_wait_is_not_lost() {
        let c = Composer::new();
        let (looked_tx, looked) = std::sync::mpsc::channel();
        let (done_tx, done) = std::sync::mpsc::channel();
        let waiter = {
            let c = c.clone();
            std::thread::spawn(move || {
                let first = std::sync::atomic::AtomicBool::new(true);
                let got = c.next_ask(&|| {
                    if first.swap(false, Ordering::AcqRel) {
                        looked_tx.send(()).unwrap();
                        // the stop comes now, while the flag was already read as unset
                        std::thread::sleep(Duration::from_millis(100));
                    }
                });
                done_tx.send(got.is_none()).unwrap();
            })
        };
        looked.recv().unwrap();
        c.stop();
        let left = kt_time::test_deadline().saturating_duration_since(std::time::Instant::now());
        assert_eq!(done.recv_timeout(left), Ok(true), "the compose thread missed the stop");
        waiter.join().unwrap();
    }

    /// Test t1841 (idle-cpu): a new project state is timed on the offline device first; a fast chain
    /// then goes to the display target, a slow one stays away from it, and an edit asks again.
    #[test]
    fn t1841_a_slow_chain_stays_off_the_display_device() {
        let (fast, slow) = (SLOW_CHAIN / 10, SLOW_CHAIN * 3);
        let mut r = Route::default();
        assert!(r.through_slow(1), "a new state is timed where it cannot hold the UI");
        r.timed(fast);
        assert!(!r.through_slow(1), "a fast chain goes to the display target");
        assert!(r.through_slow(2), "an edit is a new state");
        r.timed(slow);
        assert!(r.through_slow(2), "a slow chain stays off the display device");
        r.timed(slow);
        assert!(r.through_slow(2));
        r.timed(fast);
        assert!(!r.through_slow(2), "it comes back when it is fast again");
    }
}
