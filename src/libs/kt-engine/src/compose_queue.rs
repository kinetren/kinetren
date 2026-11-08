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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
}

type Wake = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct Composer {
    ask: Mutex<Option<Ask>>,
    ask_cv: Condvar,
    done: Mutex<Option<Composed>>,
    done_cv: Condvar,
    spare: Mutex<Vec<u8>>,
    seq: AtomicU64,
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
            seq: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            wake: Mutex::new(None),
            slow: Mutex::new(None),
            nudged: AtomicBool::new(false),
        })
    }

    /// The newest ask replaces the waiting one. Its number comes back.
    pub(crate) fn ask(&self, t: Tick, present: Option<PresentOptions>, versions: (u64, u64)) -> u64 {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        *self.ask.lock().unwrap() = Some(Ask { t, present, seq, versions });
        self.ask_cv.notify_one();
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

    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.ask_cv.notify_all();
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
    let mut route = Route::default();
    let mut checked_gen = u64::MAX;
    loop {
        let ask = {
            let mut g = c.ask.lock().unwrap();
            loop {
                if c.stop.load(Ordering::Acquire) {
                    return;
                }
                if let Some(a) = g.take() {
                    break Some(a);
                }
                if c.nudged.swap(false, Ordering::AcqRel) {
                    break None;
                }
                g = c.ask_cv.wait(g).unwrap();
            }
        };
        let Some(ask) = ask else {
            recheck(&c, &parts, &mut checked_gen);
            continue;
        };
        let t0 = Instant::now();
        let mut rgba = std::mem::take(&mut *c.spare.lock().unwrap());
        let display = ask.present.filter(|_| parts.can_present());
        let slow = c.slow.lock().unwrap().clone();
        let via = slow.filter(|_| display.is_some() && route.through_slow(ask.versions.0));
        let timed = via.is_some();
        let on_display = display.is_some() && via.is_none();
        let out = match display.filter(|_| on_display) {
            Some(o) => {
                let r = parts.compose(ask.t, Target::Display(o));
                parts.wait_presented();
                r
            }
            None => parts.compose_via(via.as_ref(), ask.t, Target::Bytes(&mut rgba)),
        };
        let spent = t0.elapsed();
        // the executor's own time: the plan (a stream stage's kernel included) holds no queue
        let plan = out.as_ref().map_or(0, |o| o.4);
        let exec = spent.saturating_sub(Duration::from_nanos(plan));
        if timed {
            route.timed(exec);
        } else if on_display {
            route.slow = exec >= SLOW_CHAIN;
        }
        let answer = out.map(|(quality, complete, gpu, cost, plan_ns, stages)| {
            parts.note_cost(VideoCost { gpu, cost, compose_ns: spent.as_nanos() as u64, plan_ns, stages });
            (quality, complete)
        });
        // a picture of a state the model has left is nobody's: an Undo must not see it land
        if parts.versions() != ask.versions {
            c.give_back(rgba);
            continue;
        }
        let (quality, complete) = answer.unwrap_or((parts.quality(), false));
        let frame = Composed { seq: ask.seq, t: ask.t, quality, complete, on_display, rgba };
        if let Some(old) = c.done.lock().unwrap().replace(frame) {
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
