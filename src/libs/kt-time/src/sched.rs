//! One scheduler for the program's background work (one-scheduler, `docs/engine-audit.md` §2b).
//!
//! Before it every service had a thread of its own with its own queue discipline and its own timer:
//! nothing knew what ran, nothing paused it while playing, nothing cancelled it by one rule. Here:
//!
//! - **Classes** in a fixed priority order ([`Class`]); a free worker always takes the highest
//!   runnable class first. The order is general, never tuned to a machine.
//! - **Lanes**: a producer (the thumbnail service, the poster service, Ctrl+S, …) posts to its own
//!   [`Lane`]; a lane runs ONE job at a time, in its own FIFO order, so a service keeps the
//!   serialization its thread gave it. A lane either takes one-shot jobs ([`Lane::post`]) or runs a
//!   pump ([`Lane::service`] + [`Lane::kick`]): the service's own queue drained by one function
//!   that is scheduled again while it says [`Pump::More`].
//! - **A small fixed pool**, sized at run time from this machine's cores ([`workers`]); the
//!   pausable and offline classes may hold all but one worker, so a person's answer (a save, a
//!   media open) never waits behind background work.
//! - **Pause votes** ([`Vote`]): while any vote is up the pausable classes ([`Class::pausable`])
//!   start nothing, and a running job's [`Ctx::checkpoint`] waits on a condvar — the owner's "Да"
//!   (kkt-26.11.08 H15): thumbnails, analysis and the index pause while playing.
//! - **Cancellation tokens** ([`Token`]): a lane's own token (closed with its owner) and a job's.
//! - **Events, never timers**: a worker parks on a condvar until a post, a resume or an exit; a
//!   checkpoint waits the same way. Nothing here ever wakes on a clock.
//! - **Counted and traced**: every post, run, cancel, pause and resume is a `trace` record
//!   (`Ev::SchedQueue`/`SchedRun`/`SchedCancel`/`SchedPause`) and a per-class counter
//!   ([`stats`]); a job's records carry it as their cause.
//!
//! Decoder owners stay dedicated threads (invariant 5): a lane's job may open a short-lived source
//! of its own, and it drops it before the job returns, so a source never outlives the one thread
//! that runs the job.

use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use crate::trace::{self, Ev};
use crate::work::{self, Work};

/// A job's class, in priority order (the lower number runs first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Class {
    /// Audible or visible now: what playback needs at this moment.
    Now = 0,
    /// The answer to a person's gesture: a save, a media open, a decoder released.
    Answer = 1,
    /// Near work: the look-ahead, the loop's next lap.
    Near = 2,
    /// Visible background: thumbnails on screen, effect cells of an open picker, reel strips.
    Visible = 3,
    /// Background: posters, analysis, the index, the media watch.
    Background = 4,
    /// Offline: an export.
    Offline = 5,
}

impl Class {
    pub const ALL: [Class; 6] =
        [Class::Now, Class::Answer, Class::Near, Class::Visible, Class::Background, Class::Offline];

    pub const fn name(self) -> &'static str {
        match self {
            Class::Now => "now",
            Class::Answer => "answer",
            Class::Near => "near",
            Class::Visible => "visible",
            Class::Background => "background",
            Class::Offline => "offline",
        }
    }

    /// Paused while a [`Vote`] is up (playing, an export).
    pub const fn pausable(self) -> bool {
        matches!(self, Class::Visible | Class::Background)
    }

    /// Held to all workers but one: the person's answer always finds a worker.
    const fn capped(self) -> bool {
        matches!(self, Class::Visible | Class::Background | Class::Offline)
    }
}

/// Why the pausable classes pause (`Vote` reasons, the `SchedPause.a` mask).
pub mod why {
    /// A player is playing.
    pub const PLAYING: u32 = 1;
    /// An export is running.
    pub const EXPORT: u32 = 2;
    /// A test holds them.
    pub const HELD: u32 = 4;
    pub(super) const ALL: [u32; 3] = [PLAYING, EXPORT, HELD];
}

/// Why a job was cancelled (`SchedCancel.aux`).
pub mod why_cancel {
    /// The lane's owner closed it (a service dropped with its project).
    pub const CLOSED: u32 = 1;
    /// A newer job replaced it.
    pub const SUPERSEDED: u32 = 2;
    /// Its token was cancelled by whoever posted it.
    pub const ASKED: u32 = 3;
}

/// What a run ended in (`SchedRun.c`).
pub mod outcome {
    pub const DONE: u64 = 0;
    /// A pump that has more: scheduled again (after higher classes, and after a pause).
    pub const MORE: u64 = 1;
    /// Its token was cancelled; it was dropped unrun.
    pub const CANCELLED: u64 = 2;
    /// It panicked; the worker lives on.
    pub const PANICKED: u64 = 3;
}

/// What a pump says when it returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pump {
    /// Its queue is empty: it runs again only on [`Lane::kick`].
    Idle,
    /// It has more (or it yielded to a pause): scheduled again at once.
    More,
}

/// A cancellation token. Clones share it.
#[derive(Clone, Default, Debug)]
pub struct Token(Arc<AtomicU32>);

impl Token {
    pub fn new() -> Token {
        Token::default()
    }

    /// Cancel with a [`why_cancel`] reason. Wakes a job waiting at a checkpoint.
    pub fn cancel(&self, why: u32) {
        if self.0.compare_exchange(0, why.max(1), Ordering::AcqRel, Ordering::Acquire).is_ok() {
            let s = sched();
            let _g = s.lock();
            s.gate.notify_all();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire) != 0
    }

    /// The reason, 0 when live.
    pub fn reason(&self) -> u32 {
        self.0.load(Ordering::Acquire)
    }
}

/// What a running job sees.
pub struct Ctx {
    id: u64,
    class: Class,
    lane: Token,
    job: Token,
}

impl Ctx {
    /// The job's id (its records carry it as their cause).
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn class(&self) -> Class {
        self.class
    }

    /// Its lane was closed or its own token cancelled.
    pub fn cancelled(&self) -> bool {
        self.lane.is_cancelled() || self.job.is_cancelled()
    }

    /// Its class is paused now: a pump returns [`Pump::More`] at its next item.
    pub fn paused(&self) -> bool {
        self.class.pausable() && sched().paused.load(Ordering::Acquire) != 0
    }

    /// "Is it time to stop?" — and while its class is paused, WAITS (a condvar, no clock) until
    /// it resumes or the job is cancelled. `true` = stop.
    pub fn checkpoint(&self) -> bool {
        if self.cancelled() {
            return true;
        }
        if !self.paused() {
            return false;
        }
        let s = sched();
        let mut g = s.lock();
        while self.paused() && !self.cancelled() {
            g = s.gate.wait(g).unwrap_or_else(|e| e.into_inner());
        }
        drop(g);
        self.cancelled()
    }

    /// A token that is cancelled when either this job's or its lane's is (for a body that keeps
    /// its own flag, such as a `BgCtl`).
    pub fn tokens(&self) -> (Token, Token) {
        (self.lane.clone(), self.job.clone())
    }
}

type OnceFn = Box<dyn FnOnce(&Ctx) + Send>;
type PumpFn = Arc<dyn Fn(&Ctx) -> Pump + Send + Sync>;

struct Queued {
    id: u64,
    f: OnceFn,
    token: Token,
    at: Instant,
    key: u64,
}

#[derive(Default)]
struct LaneSt {
    once: VecDeque<Queued>,
    pump: Option<PumpFn>,
    pump_due: bool,
    pump_at: Option<Instant>,
    pump_id: u64,
    running: bool,
    in_ready: bool,
    closed: bool,
}

impl LaneSt {
    fn has_work(&self) -> bool {
        !self.closed && (!self.once.is_empty() || (self.pump_due && self.pump.is_some()))
    }
}

/// A producer's place in the scheduler: one job at a time, FIFO. Closed when its [`LaneRef`]
/// drops.
pub struct Lane {
    name: &'static str,
    class: Class,
    token: Token,
    st: Mutex<LaneSt>,
    idle: Condvar,
}

/// The owner's handle of a lane: dropping it closes the lane (its queued jobs are cancelled, its
/// pump never runs again, a running job sees `Ctx::cancelled`).
pub struct LaneRef(Arc<Lane>);

impl std::ops::Deref for LaneRef {
    type Target = Lane;
    fn deref(&self) -> &Lane {
        &self.0
    }
}

impl Drop for LaneRef {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl LaneRef {
    fn me(&self) -> Arc<Lane> {
        self.0.clone()
    }

    /// Post a one-shot job. Returns its id (0 when the lane is closed).
    pub fn post(&self, f: impl FnOnce(&Ctx) + Send + 'static) -> u64 {
        self.post_keyed(0, Token::new(), f)
    }

    /// Post a job with its own token; a nonzero `key` REPLACES a queued job of the same key (the
    /// older one is cancelled as superseded: a series of Ctrl+S writes the last).
    pub fn post_keyed(&self, key: u64, token: Token, f: impl FnOnce(&Ctx) + Send + 'static) -> u64 {
        let id = trace::next_id();
        let mut st = self.0.st();
        if st.closed {
            return 0;
        }
        if key != 0 {
            let lane = self.0.name;
            st.once.retain(|q| {
                let keep = q.key != key;
                if !keep {
                    cancelled(q.id, lane, self.0.class, why_cancel::SUPERSEDED);
                }
                keep
            });
        }
        st.once.push_back(Queued { id, f: Box::new(f), token, at: Instant::now(), key });
        queued(id, self.0.name, self.0.class);
        let push = !st.running && !st.in_ready;
        if push {
            st.in_ready = true;
        }
        drop(st);
        if push {
            sched().ready(self.me());
        }
        id
    }

    /// Schedule the pump (a service lane). Coalesced: a kick while one is due adds nothing, a kick
    /// while it runs runs it once more after.
    pub fn kick(&self) {
        let mut st = self.0.st();
        if st.closed || st.pump.is_none() || st.pump_due {
            return;
        }
        st.pump_due = true;
        st.pump_at = Some(Instant::now());
        st.pump_id = trace::next_id();
        queued(st.pump_id, self.0.name, self.0.class);
        let push = !st.running && !st.in_ready;
        if push {
            st.in_ready = true;
        }
        drop(st);
        if push {
            sched().ready(self.me());
        }
    }

}

impl Lane {
    /// A lane of one-shot jobs.
    pub fn open(name: &'static str, class: Class) -> LaneRef {
        LaneRef(Arc::new(Lane {
            name,
            class,
            token: Token::new(),
            st: Mutex::new(LaneSt::default()),
            idle: Condvar::new(),
        }))
    }

    /// A service lane: `pump` drains the service's own queue; [`Lane::kick`] schedules it.
    pub fn service(name: &'static str, class: Class, pump: impl Fn(&Ctx) -> Pump + Send + Sync + 'static) -> LaneRef {
        let l = Lane::open(name, class);
        l.st().pump = Some(Arc::new(pump));
        l
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn class(&self) -> Class {
        self.class
    }

    fn st(&self) -> MutexGuard<'_, LaneSt> {
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Cancel every queued one-shot job (a running one sees its token only if the poster cancels
    /// it).
    pub fn cancel_queued(&self, why: u32) {
        let mut st = self.st();
        for q in st.once.drain(..) {
            cancelled(q.id, self.name, self.class, why);
        }
        if st.pump_due {
            st.pump_due = false;
            cancelled(st.pump_id, self.name, self.class, why);
        }
        drop(st);
        self.idle.notify_all();
    }

    /// Close: the token is cancelled, queued jobs are dropped, the pump is released.
    pub fn close(&self) {
        self.token.cancel(why_cancel::CLOSED);
        let pump = {
            let mut st = self.st();
            st.closed = true;
            st.pump.take()
        };
        drop(pump);
        self.cancel_queued(why_cancel::CLOSED);
    }

    /// Wait until the lane is idle, with no bound: an owner's drop waiting for its running job (a
    /// frame on the device must end before the process does).
    pub fn wait_until_idle(&self) {
        let mut st = self.st();
        while st.running || !st.once.is_empty() || (st.pump_due && st.pump.is_some() && !st.closed) {
            st = self.idle.wait(st).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Nothing queued, due or running.
    pub fn is_idle(&self) -> bool {
        let st = self.st();
        !st.running && st.once.is_empty() && !(st.pump_due && st.pump.is_some())
    }

    /// Wait until the lane is idle, at most until `deadline`. `false` on the deadline. For an exit
    /// and for tests; never on the UI thread's frame path.
    pub fn wait_idle(&self, deadline: Instant) -> bool {
        let mut st = self.st();
        loop {
            if !st.running && st.once.is_empty() && !(st.pump_due && st.pump.is_some() && !st.closed) {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            st = self.idle.wait_timeout(st, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
        }
    }
}

fn queued(id: u64, lane: &'static str, class: Class) {
    let s = sched();
    s.counts[class as usize][0].fetch_add(1, Ordering::Relaxed);
    trace::ev(Ev::SchedQueue).id(id).aux(class as u32).a(trace::name(lane) as u64).emit();
}

fn cancelled(id: u64, lane: &'static str, class: Class, why: u32) {
    let s = sched();
    s.counts[class as usize][3].fetch_add(1, Ordering::Relaxed);
    trace::ev(Ev::SchedCancel).id(id).aux(why).a(trace::name(lane) as u64).emit();
}

// ---- the pool -------------------------------------------------------------------------------------

struct St {
    ready: [VecDeque<Arc<Lane>>; 6],
    capped_running: usize,
    running: usize,
    votes: [u32; 3],
    spawned: bool,
}

struct Sched {
    st: Mutex<St>,
    /// Workers wait here for runnable work.
    work: Condvar,
    /// Checkpoints wait here for a resume or a cancel.
    gate: Condvar,
    /// The `why` mask of the votes up now.
    paused: AtomicU32,
    workers: usize,
    /// Per class: queued, started, done, cancelled.
    counts: [[AtomicU64; 4]; 6],
}

fn sched() -> &'static Sched {
    static S: OnceLock<Sched> = OnceLock::new();
    S.get_or_init(|| Sched {
        st: Mutex::new(St {
            ready: Default::default(),
            capped_running: 0,
            running: 0,
            votes: [0; 3],
            spawned: false,
        }),
        work: Condvar::new(),
        gate: Condvar::new(),
        paused: AtomicU32::new(0),
        workers: workers(),
        counts: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU64::new(0))),
    })
}

/// The pool's size, from this machine's cores at run time: a quarter of them, at least three (one
/// is always kept for the person's answers, and two background lanes may run side by side), at
/// most six (the background work is decode- and device-bound; more threads only wait).
pub fn workers() -> usize {
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    (cores / 4).clamp(3, 6)
}

impl Sched {
    fn lock(&self) -> MutexGuard<'_, St> {
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn ready(&'static self, lane: Arc<Lane>) {
        let mut st = self.lock();
        if !st.spawned {
            st.spawned = true;
            for i in 0..self.workers {
                let _ = std::thread::Builder::new().name(format!("sched-{i}")).spawn(move || self.worker());
            }
        }
        st.ready[lane.class as usize].push_back(lane);
        drop(st);
        self.work.notify_one();
    }

    /// The highest runnable lane, under the lock.
    fn pick(&self, st: &mut St) -> Option<Arc<Lane>> {
        let paused = self.paused.load(Ordering::Acquire) != 0;
        for c in Class::ALL {
            if c.pausable() && paused {
                continue;
            }
            if c.capped() && st.capped_running + 1 >= self.workers {
                continue;
            }
            if let Some(l) = st.ready[c as usize].pop_front() {
                if c.capped() {
                    st.capped_running += 1;
                }
                st.running += 1;
                return Some(l);
            }
        }
        None
    }

    fn worker(&'static self) {
        loop {
            let lane = {
                let mut st = self.lock();
                loop {
                    if let Some(l) = self.pick(&mut st) {
                        break l;
                    }
                    st = self.work.wait(st).unwrap_or_else(|e| e.into_inner());
                }
            };
            self.run(&lane);
            let again = {
                let mut ls = lane.st();
                ls.running = false;
                let again = ls.has_work();
                ls.in_ready = again;
                again
            };
            lane.idle.notify_all();
            let mut st = self.lock();
            st.running -= 1;
            if lane.class.capped() {
                st.capped_running -= 1;
            }
            if again {
                st.ready[lane.class as usize].push_back(lane);
            }
            drop(st);
            // a capped slot came free, or the lane has more: another worker may take it
            self.work.notify_one();
        }
    }

    /// One unit of a lane: its oldest one-shot job, else its pump.
    fn run(&self, lane: &Arc<Lane>) {
        enum Unit {
            Once(Queued),
            Pump(PumpFn, u64, Instant),
        }
        let unit = {
            let mut ls = lane.st();
            ls.running = true;
            if ls.closed {
                None
            } else if let Some(q) = ls.once.pop_front() {
                Some(Unit::Once(q))
            } else if ls.pump_due && let Some(p) = ls.pump.clone() {
                ls.pump_due = false;
                Some(Unit::Pump(p, ls.pump_id, ls.pump_at.take().unwrap_or_else(Instant::now)))
            } else {
                None
            }
        };
        let Some(unit) = unit else { return };
        let (id, at, job) = match &unit {
            Unit::Once(q) => (q.id, q.at, q.token.clone()),
            Unit::Pump(_, id, at) => (*id, *at, Token::new()),
        };
        let counts = &self.counts[lane.class as usize];
        if job.is_cancelled() || lane.token.is_cancelled() {
            counts[3].fetch_add(1, Ordering::Relaxed);
            let why = if job.is_cancelled() { job.reason() } else { why_cancel::CLOSED };
            trace::ev(Ev::SchedCancel).id(id).aux(why).a(trace::name(lane.name) as u64).emit();
            return;
        }
        counts[1].fetch_add(1, Ordering::Relaxed);
        work::bump(Work::SchedJob);
        let ctx = Ctx { id, class: lane.class, lane: lane.token.clone(), job };
        let mut span = trace::span(Ev::SchedRun);
        span.id(id)
            .aux(lane.class as u32)
            .a(trace::name(lane.name) as u64)
            .b(at.elapsed().as_micros() as u64);
        let _cause = trace::with_cause(id);
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| match unit {
            Unit::Once(q) => {
                (q.f)(&ctx);
                Pump::Idle
            }
            Unit::Pump(p, _, _) => p(&ctx),
        }));
        let out = match res {
            Ok(Pump::More) => {
                let mut ls = lane.st();
                if !ls.closed && !ls.pump_due {
                    ls.pump_due = true;
                    ls.pump_at = Some(Instant::now());
                    ls.pump_id = trace::next_id();
                }
                outcome::MORE
            }
            Ok(Pump::Idle) => outcome::DONE,
            Err(_) => outcome::PANICKED,
        };
        span.c(out);
        counts[2].fetch_add(1, Ordering::Relaxed);
    }

    fn vote(&'static self, why: u32, up: bool) {
        let Some(i) = why::ALL.iter().position(|w| *w == why) else { return };
        let mut st = self.lock();
        if up {
            st.votes[i] += 1;
        } else {
            st.votes[i] = st.votes[i].saturating_sub(1);
        }
        let mask = why::ALL.iter().zip(st.votes).filter(|(_, n)| *n > 0).fold(0, |m, (w, _)| m | w);
        let was = self.paused.swap(mask, Ordering::AcqRel);
        if (was == 0) != (mask == 0) {
            let waiting: usize = Class::ALL
                .iter()
                .filter(|c| c.pausable())
                .map(|c| st.ready[*c as usize].len())
                .sum();
            trace::ev(Ev::SchedPause).aux((mask != 0) as u32).a(mask as u64).b(waiting as u64).emit();
        }
        drop(st);
        if mask == 0 && was != 0 {
            self.work.notify_all();
            self.gate.notify_all();
        }
    }
}

/// A pause vote: while it is up the pausable classes start nothing and their checkpoints wait.
/// Dropping it takes it down, so a player or an export that dies never leaves the pause behind.
pub struct Vote {
    why: u32,
    up: AtomicBool,
}

impl Vote {
    pub fn new(why: u32) -> Vote {
        Vote { why, up: AtomicBool::new(false) }
    }

    /// Up or down; a repeat is nothing.
    pub fn set(&self, up: bool) {
        if self.up.swap(up, Ordering::AcqRel) != up {
            sched().vote(self.why, up);
        }
    }

    pub fn is_up(&self) -> bool {
        self.up.load(Ordering::Acquire)
    }
}

impl Drop for Vote {
    fn drop(&mut self) {
        self.set(false);
    }
}

/// Wait on the scheduler's gate (a condvar, no clock) while `still()` holds. The gate is notified by
/// every resume, every token cancel and [`notify_gate`]: a holder whose own condition is a flag
/// calls `notify_gate` when it changes the flag.
pub fn wait_gate(still: impl Fn() -> bool) {
    let s = sched();
    let mut g = s.lock();
    while still() {
        g = s.gate.wait(g).unwrap_or_else(|e| e.into_inner());
    }
}

/// Wake every [`wait_gate`] to look at its condition again.
pub fn notify_gate() {
    let s = sched();
    let _g = s.lock();
    s.gate.notify_all();
}

/// The `why` mask of the votes up now (0 = nothing paused).
pub fn paused() -> u32 {
    sched().paused.load(Ordering::Acquire)
}

/// One class's counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassStats {
    pub queued: u64,
    pub started: u64,
    pub done: u64,
    pub cancelled: u64,
}

/// What the scheduler did, per class (in [`Class::ALL`] order), and what runs now.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub classes: [ClassStats; 6],
    pub running: usize,
    pub waiting: usize,
    pub paused: u32,
}

pub fn stats() -> Stats {
    let s = sched();
    let st = s.lock();
    let classes = std::array::from_fn(|i| {
        let c = &s.counts[i];
        ClassStats {
            queued: c[0].load(Ordering::Relaxed),
            started: c[1].load(Ordering::Relaxed),
            done: c[2].load(Ordering::Relaxed),
            cancelled: c[3].load(Ordering::Relaxed),
        }
    });
    Stats {
        classes,
        running: st.running,
        waiting: st.ready.iter().map(|q| q.len()).sum(),
        paused: s.paused.load(Ordering::Acquire),
    }
}

/// A result a job hands back to whoever asked (the UI takes it in a later frame, or waits a grace).
pub struct Answer<T> {
    slot: Arc<(Mutex<Option<T>>, Condvar)>,
}

/// The job's side of an [`Answer`].
pub struct Reply<T> {
    slot: Arc<(Mutex<Option<T>>, Condvar)>,
}

impl<T> Reply<T> {
    pub fn send(self, v: T) {
        let (m, cv) = &*self.slot;
        *m.lock().unwrap_or_else(|e| e.into_inner()) = Some(v);
        cv.notify_all();
    }
}

impl<T> Answer<T> {
    pub fn new() -> (Answer<T>, Reply<T>) {
        let slot = Arc::new((Mutex::new(None), Condvar::new()));
        (Answer { slot: slot.clone() }, Reply { slot })
    }

    /// The answer if it came, waiting at most `grace` for it (a few ms on the UI thread; the
    /// harness gives seconds so a test composes in its frame, as the preview's `COMPOSE_GRACE`).
    pub fn take_within(&self, grace: Duration) -> Option<T> {
        let (m, cv) = &*self.slot;
        let mut g = m.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() && !grace.is_zero() {
            let until = Instant::now() + grace;
            while g.is_none() {
                let now = Instant::now();
                if now >= until {
                    break;
                }
                g = cv.wait_timeout(g, until - now).unwrap_or_else(|e| e.into_inner()).0;
            }
        }
        g.take()
    }

    /// The reply was dropped unsent (the job was cancelled or panicked).
    pub fn abandoned(&self) -> bool {
        Arc::strong_count(&self.slot) == 1 && self.slot.0.lock().map(|g| g.is_none()).unwrap_or(true)
    }
}

#[cfg(test)]
mod tests;
