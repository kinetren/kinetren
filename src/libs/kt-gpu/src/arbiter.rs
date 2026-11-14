//! One arbiter per ring (gpu-arbiter, `docs/engine-audit.md` M5, rows 134 and 149).
//!
//! A hardware card runs every `wgpu::Device` the program opens on it — the display's, the offline
//! engine's, the export's, the compute host's — through ONE graphics ring, interleaved by
//! submission: a 300 ms offline submission stood in front of the display's frame. A software
//! rasterizer gives each device a queue thread of its own, so there each device is its own ring.
//!
//! - **Admission** ([`Ring::admit`]): every submission but the display's present asks its ring; the
//!   waiting submission of the highest [`Class`] goes first, and none of a class goes while one of a
//!   higher class waits.
//! - **The bound**: a ring that carries a display (or a compute host's ring, where the frame on
//!   screen and the look-ahead meet) holds at most [`bound`] of predicted device time in flight; one
//!   piece always fits an idle ring. Half a frame period of the preview — a general rule, never a
//!   number of one machine.
//! - **Pieces** ([`Piece`]): a lower-class frame on a displayed ring is cut at pass boundaries where
//!   the passes' device time, as the pass timer MEASURED them on this machine, would pass the bound;
//!   on any bounded ring it is cut at the next boundary while a higher class waits. A piece is submitted
//!   and the next is admitted against the time in flight, never after a wait for the piece itself
//!   (stream-history-on-device, found-defects 392). The passes, their order and their targets do not change: the same bytes
//!   (invariant 11).
//! - **No submitter has to come back to free its piece**: a piece's completion is a work-done
//!   callback, and a waiter completes the oldest piece itself by a blocking wait on that piece's
//!   device when the device carries no surface (a blocking wait on the display device would hold
//!   egui's present, which takes the device's lock for writing).
//!
//! Every admission and piece is a trace record (`GpuAdmit`, `GpuPiece`).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use kt_time::sched::Class;
use kt_time::trace::{self, Ev};

static ENABLED: AtomicU32 = AtomicU32::new(UNSET);
const UNSET: u32 = 2;
static NEXT_RING: AtomicU64 = AtomicU64::new(1);

/// What a bounded ring holds in flight at most: half a frame period of the preview
/// (`kt_time::sched::frame_period_ns`, the player sets it from its project's frame rate), so the
/// display's own frame and one piece fit one period.
pub fn bound() -> u64 {
    kt_time::sched::frame_period_ns() / 2
}

/// The bench's A/B switch (`kt-cli gpu-share --arbiter off`): off, every ring admits at once and
/// cuts nothing — the program as it was before.
pub fn set_enabled(on: bool) {
    ENABLED.store(on as u32, Ordering::Relaxed);
}

/// On unless `set_enabled(false)` or `KT_GPU_ARBITER=off` (an A/B of a run, as `KT_FUSE_RUNS`).
pub fn enabled() -> bool {
    match ENABLED.load(Ordering::Relaxed) {
        UNSET => {
            let on = std::env::var("KT_GPU_ARBITER").as_deref() != Ok("off");
            let _ = ENABLED.compare_exchange(UNSET, on as u32, Ordering::Relaxed, Ordering::Relaxed);
            ENABLED.load(Ordering::Relaxed) == 1
        }
        v => v == 1,
    }
}

/// Which ring a device runs on (D1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RingKey {
    /// A software rasterizer's device: its own queue thread. The address of the device's `Arc`.
    Device(usize),
    /// A hardware card by its PCI ids; `(0, 0)` — the adapter named none.
    Card(u32, u32),
}

impl RingKey {
    pub fn of(info: &wgpu::AdapterInfo, device: &Arc<wgpu::Device>) -> RingKey {
        if info.device_type == wgpu::DeviceType::Cpu {
            RingKey::Device(Arc::as_ptr(device) as usize)
        } else {
            RingKey::Card(info.vendor, info.device)
        }
    }
}

/// One admitted piece: in flight from its admission until its device says it is done.
struct Flight {
    id: u64,
    ns: u64,
    class: Class,
    /// The display's present (counted, never admitted: gpu-throughput D1).
    display: bool,
    /// When it was submitted; `None` — admitted, still being encoded.
    at: Option<Instant>,
    /// Its device and submission once submitted; a device with a surface is never waited on.
    sub: Option<(Arc<wgpu::Device>, wgpu::SubmissionIndex, bool)>,
    done: Arc<AtomicBool>,
}

#[derive(Default)]
struct State {
    flight: Vec<Flight>,
    waiting: [u32; Class::ALL.len()],
    next: u64,
    /// When the display last submitted a present.
    last_display: Option<Instant>,
}

/// Lower-class pieces of one ring uncompleted at once at most (gpu-throughput D1): one running, one
/// queued behind it and one being encoded. A prediction far below the real time cannot pile more in
/// front of the display.
pub const AHEAD_PIECES: usize = 3;

/// The ring's queue as predicted at `now` (gpu-throughput D1): it drains in submission order, each
/// flight's predicted time after the one before (a flight still being encoded is taken as submitted
/// now; one past its predicted end and not done has half its time left).
struct Drain {
    /// When the whole queue is predicted done.
    end: Instant,
    /// When the display's last present in flight is predicted done.
    display_end: Option<Instant>,
}

fn drain(st: &State, now: Instant, skip_display: bool) -> Drain {
    let mut end: Option<Instant> = None;
    let mut display_end = None;
    for f in st.flight.iter().filter(|f| !f.done.load(Ordering::Acquire) && !(skip_display && f.display)) {
        let ns = Duration::from_nanos(f.ns);
        let at = f.at.unwrap_or(now);
        let start = end.map_or(at, |e| e.max(at));
        let mut e = start + ns;
        if e <= now {
            e = now + ns / 2;
        }
        end = Some(e);
        if f.display {
            display_end = Some(e);
        }
    }
    Drain { end: end.map_or(now, |e| e.max(now)), display_end }
}

pub struct Ring {
    id: u64,
    key: RingKey,
    displays: AtomicU32,
    computes: AtomicU32,
    st: Mutex<State>,
    cv: Condvar,
}

static RINGS: Mutex<Vec<(RingKey, Weak<Ring>)>> = Mutex::new(Vec::new());

/// The ring of a device: every caller with the same key gets the same ring while one lives. A card
/// that names no PCI id joins the live card ring, and a named card joins an unnamed one: two cards
/// merged cost throughput, never a picture.
pub fn ring_for(key: RingKey) -> Arc<Ring> {
    let mut rings = RINGS.lock().unwrap_or_else(|e| e.into_inner());
    rings.retain(|(_, w)| w.strong_count() > 0);
    let found = rings.iter().find(|(k, _)| *k == key).or_else(|| match key {
        RingKey::Card(..) => rings.iter().find(|(k, _)| matches!((k, key), (RingKey::Card(0, 0), _) | (RingKey::Card(..), RingKey::Card(0, 0)))),
        RingKey::Device(_) => None,
    });
    if let Some(r) = found.and_then(|(_, w)| w.upgrade()) {
        if !rings.iter().any(|(k, _)| *k == key) {
            rings.push((key, Arc::downgrade(&r)));
        }
        return r;
    }
    let r = Arc::new(Ring {
        id: NEXT_RING.fetch_add(1, Ordering::Relaxed),
        key,
        displays: AtomicU32::new(0),
        computes: AtomicU32::new(0),
        st: Mutex::default(),
        cv: Condvar::new(),
    });
    rings.push((key, Arc::downgrade(&r)));
    r
}

/// A display or a compute host registered on a ring, while it lives.
pub struct Registered(Arc<Ring>, bool);

impl Drop for Registered {
    fn drop(&mut self) {
        let c = if self.1 { &self.0.displays } else { &self.0.computes };
        c.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Ring {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn key(&self) -> RingKey {
        self.key
    }

    /// The editor's display device (or the bench's) runs on this ring: from now on the ring bounds
    /// what it holds in front of it.
    pub fn register_display(self: &Arc<Self>) -> Registered {
        self.displays.fetch_add(1, Ordering::AcqRel);
        Registered(self.clone(), true)
    }

    /// A compute host runs on this ring: the frame on screen and the look-ahead meet here.
    pub fn register_compute(self: &Arc<Self>) -> Registered {
        self.computes.fetch_add(1, Ordering::AcqRel);
        Registered(self.clone(), false)
    }

    pub fn has_display(&self) -> bool {
        self.displays.load(Ordering::Acquire) > 0
    }

    /// The ring bounds its flight: it carries a display or a compute host.
    pub fn bounded(&self) -> bool {
        enabled() && (self.has_display() || self.computes.load(Ordering::Acquire) > 0)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A class above `class` waits for this ring now.
    pub fn higher_waits(&self, class: Class) -> bool {
        let st = self.lock();
        st.waiting[..class as usize].iter().any(|&n| n > 0)
    }

    /// When the display's next present is expected: a frame period after its last while it presents,
    /// else now (a press of Play may come at any moment).
    fn display_next(&self, st: &State, now: Instant) -> Instant {
        let period = Duration::from_nanos(kt_time::sched::frame_period_ns());
        match st.last_display {
            Some(t) if now.saturating_duration_since(t) < 2 * period => (t + period).max(now),
            _ => now,
        }
    }

    /// Whether a submission of `class` predicted at `ns` may go now; `Err` — how long until the
    /// prediction alone makes room (`None` — only a completion will).
    fn admissible(&self, st: &State, class: Class, ns: u64, now: Instant) -> Result<(), Option<Duration>> {
        if st.waiting[..class as usize].iter().any(|&n| n > 0) {
            return Err(None);
        }
        let live = |f: &&Flight| !f.done.load(Ordering::Acquire);
        // one piece always fits an idle ring; the frame on screen's work counts no display present
        // (its own frames) as busy
        let idle = !st.flight.iter().any(|f| live(&f) && (class != Class::Now || !f.display));
        if !self.bounded() || idle {
            return Ok(());
        }
        let bound = Duration::from_nanos(bound());
        let ns = Duration::from_nanos(ns);
        if class == Class::Now {
            // one bound of predicted time ahead of it
            let d = drain(st, now, true);
            let left = d.end.saturating_duration_since(now);
            return if left + ns <= bound { Ok(()) } else { Err(Some(left + ns - bound)) };
        }
        // a lower class: never more than AHEAD_PIECES uncompleted, and what it leaves in front of
        // the display's next present within the bound (gpu-throughput D1)
        if st.flight.iter().filter(|f| live(f) && f.class != Class::Now).count() >= AHEAD_PIECES {
            return Err(None);
        }
        let d = drain(st, now, false);
        let reference = if self.has_display() { self.display_next(st, now).max(d.display_end.unwrap_or(now)) } else { now };
        let fits_by = reference + bound;
        let ends = d.end + ns;
        if ends <= fits_by { Ok(()) } else { Err(Some(ends - fits_by)) }
    }

    /// Wait until a submission of `class` predicted at `ns` of device time may go. The returned
    /// ticket is in flight until it is [`Ticket::submitted`] and its device finishes it, or until it
    /// drops unsubmitted.
    pub fn admit(self: &Arc<Self>, class: Class, ns: u64) -> Ticket {
        let mut span = trace::span(Ev::GpuAdmit);
        span.id(self.id).aux(class as u32).a(ns).c(self.has_display() as u64);
        let done = Arc::new(AtomicBool::new(false));
        let mut st = self.lock();
        st.flight.retain(|f| !f.done.load(Ordering::Acquire));
        let now = Instant::now();
        span.b(drain(&st, now, false).end.saturating_duration_since(now).as_nanos() as u64);
        if enabled() && self.admissible(&st, class, ns, now).is_err() {
            st.waiting[class as usize] += 1;
            loop {
                if self.admissible(&st, class, ns, Instant::now()).is_ok() {
                    break;
                }
                // a completion the device holds is noticed only by a poll: drive every device with
                // flights, never blocking
                let devices: Vec<Arc<wgpu::Device>> = st
                    .flight
                    .iter()
                    .filter(|f| !f.done.load(Ordering::Acquire))
                    .filter_map(|f| f.sub.as_ref().map(|s| s.0.clone()))
                    .fold(Vec::new(), |mut v, d| {
                        if !v.iter().any(|x: &Arc<wgpu::Device>| Arc::ptr_eq(x, &d)) {
                            v.push(d);
                        }
                        v
                    });
                drop(st);
                for d in &devices {
                    let _ = d.poll(wgpu::PollType::Poll);
                }
                st = self.lock();
                st.flight.retain(|f| !f.done.load(Ordering::Acquire));
                let wait = match self.admissible(&st, class, ns, Instant::now()) {
                    Ok(()) => break,
                    Err(w) => w,
                };
                match wait {
                    // the prediction makes room by itself: sleep until then (or a completion)
                    Some(w) => {
                        let w = w.clamp(Duration::from_micros(250), Duration::from_millis(20));
                        st = self.cv.wait_timeout(st, w).unwrap_or_else(|e| e.into_inner()).0;
                    }
                    None => {
                        // only a completion makes room: the oldest piece with a device nobody's
                        // present waits on is completed here
                        let oldest = st
                            .flight
                            .iter()
                            .find(|f| !f.done.load(Ordering::Acquire) && f.sub.as_ref().is_some_and(|s| !s.2))
                            .and_then(|f| f.sub.as_ref().map(|s| (s.0.clone(), s.1.clone(), f.done.clone())));
                        match oldest {
                            Some((device, sub, done)) if !st.waiting[..class as usize].iter().any(|&n| n > 0) => {
                                drop(st);
                                let _ = device.poll(wgpu::PollType::Wait { submission_index: Some(sub), timeout: None });
                                done.store(true, Ordering::Release);
                                st = self.lock();
                            }
                            // a piece on a device with a surface (driven by polls above), one still
                            // being encoded, or a higher class waiting
                            _ => st = self.cv.wait_timeout(st, Duration::from_millis(2)).unwrap_or_else(|e| e.into_inner()).0,
                        }
                    }
                }
                st.flight.retain(|f| !f.done.load(Ordering::Acquire));
            }
            st.waiting[class as usize] -= 1;
            // a lower class may have been held only by this one
            self.cv.notify_all();
        }
        let ticket = self.push(&mut st, class, ns, false, done);
        drop(st);
        ticket
    }

    fn push(self: &Arc<Self>, st: &mut State, class: Class, ns: u64, display: bool, done: Arc<AtomicBool>) -> Ticket {
        st.next += 1;
        let id = st.next;
        st.flight.push(Flight { id, ns, class, display, at: None, sub: None, done: done.clone() });
        Ticket { ring: self.clone(), id, done, class, submitted: false }
    }

    /// The display's present predicted at `ns` goes into the flight with no wait (gpu-throughput D1):
    /// the lower classes see it. Submit it with [`Ticket::submitted`] (`surface` true).
    pub fn note_display(self: &Arc<Self>, ns: u64) -> Ticket {
        let mut st = self.lock();
        st.flight.retain(|f| !f.done.load(Ordering::Acquire));
        st.last_display = Some(Instant::now());
        self.push(&mut st, Class::Now, ns, true, Arc::new(AtomicBool::new(false)))
    }

    fn finish(&self, id: u64) {
        let mut st = self.lock();
        if let Some(f) = st.flight.iter().find(|f| f.id == id) {
            f.done.store(true, Ordering::Release);
        }
        st.flight.retain(|f| !f.done.load(Ordering::Acquire));
        drop(st);
        self.cv.notify_all();
    }

    /// Device time in flight (tests, the bench).
    pub fn in_flight_ns(&self) -> u64 {
        self.lock().flight.iter().filter(|f| !f.done.load(Ordering::Acquire)).map(|f| f.ns).sum()
    }

    /// Before a pass predicted at `next` (None — never measured): whether the piece holding `acc`
    /// over `passes` passes ends here (D4).
    pub fn cut_before(&self, class: Class, acc: u64, passes: u32, next: Option<u64>) -> bool {
        if passes == 0 || !enabled() {
            return false;
        }
        (self.has_display() && acc + next.unwrap_or(u64::MAX / 4) > self.piece_limit(class)) || (self.bounded() && self.higher_waits(class))
    }

    /// What one piece of `class` holds at most: on a displayed ring a lower class's piece is half the
    /// bound, so one runs while the next is queued in front of the display (gpu-throughput D1).
    pub fn piece_limit(&self, class: Class) -> u64 {
        if self.has_display() && class != Class::Now { bound() / 2 } else { bound() }
    }
}

/// An admitted submission.
pub struct Ticket {
    ring: Arc<Ring>,
    id: u64,
    done: Arc<AtomicBool>,
    class: Class,
    submitted: bool,
}

impl Ticket {
    pub fn class(&self) -> Class {
        self.class
    }

    /// The submission went to `queue` as `sub`: it leaves the flight when the device finishes it.
    /// `ns` — what the piece actually holds (the admission predicted before encoding). `surface` —
    /// the device presents to a window, so nobody may block-wait on it.
    pub fn submitted(mut self, device: &Arc<wgpu::Device>, queue: &wgpu::Queue, sub: wgpu::SubmissionIndex, ns: u64, surface: bool) -> Submitted {
        self.submitted = true;
        {
            let mut st = self.ring.lock();
            if let Some(f) = st.flight.iter_mut().find(|f| f.id == self.id) {
                f.sub = Some((device.clone(), sub.clone(), surface));
                f.ns = ns;
                f.at = Some(Instant::now());
            }
        }
        let (ring, id) = (Arc::downgrade(&self.ring), self.id);
        queue.on_submitted_work_done(move || {
            if let Some(r) = ring.upgrade() {
                r.finish(id);
            }
        });
        Submitted { ring: self.ring.clone(), id: self.id, done: self.done.clone(), device: device.clone(), sub, surface }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        if !self.submitted {
            self.ring.finish(self.id);
        }
    }
}

/// A submitted piece.
pub struct Submitted {
    ring: Arc<Ring>,
    id: u64,
    done: Arc<AtomicBool>,
    device: Arc<wgpu::Device>,
    sub: wgpu::SubmissionIndex,
    surface: bool,
}

impl Submitted {
    /// Wait for the device to finish it (a blocking wait on a device with no surface; with one, the
    /// callback another submission's maintain fires).
    pub fn wait(self) {
        if self.done.load(Ordering::Acquire) {
            return;
        }
        if !self.surface {
            let _ = self.device.poll(wgpu::PollType::Wait { submission_index: Some(self.sub.clone()), timeout: None });
            self.ring.finish(self.id);
            return;
        }
        let mut st = self.ring.lock();
        while !self.done.load(Ordering::Acquire) {
            drop(st);
            if self.device.poll(wgpu::PollType::Poll).is_err() {
                return;
            }
            st = self.ring.lock();
            if self.done.load(Ordering::Acquire) {
                break;
            }
            st = self.ring.cv.wait_timeout(st, std::time::Duration::from_millis(2)).unwrap_or_else(|e| e.into_inner()).0;
        }
    }
}

/// A frame (or a compute job) being cut into pieces on its ring (D4).
pub struct Piece {
    ring: Arc<Ring>,
    class: Class,
    ticket: Option<Ticket>,
    /// Predicted device time and passes in the current piece.
    acc: u64,
    passes: u32,
    index: u32,
}

impl Piece {
    /// Admit the first piece of a frame of the caller's class, predicted at `ns` (capped by the
    /// bound: a piece holds no more).
    pub fn begin(ring: &Arc<Ring>, ns: u64) -> Piece {
        let class = kt_time::sched::current_class();
        // a frame never measured claims the whole bound
        let ns = if ns == 0 { bound() } else { ns };
        let claim = if ring.bounded() { ns.min(ring.piece_limit(class)) } else { ns };
        let ticket = ring.admit(class, claim);
        Piece { ring: ring.clone(), class, ticket: Some(ticket), acc: 0, passes: 0, index: 0 }
    }

    pub fn class(&self) -> Class {
        self.class
    }

    /// Before a pass predicted at `next`: `true` — finish the encoder, submit it with
    /// [`Piece::cut`], and go on in a new one.
    pub fn due(&self, next: Option<u64>) -> bool {
        self.ring.cut_before(self.class, self.acc, self.passes, next)
    }

    /// A pass predicted at `next` joins the piece (an unmeasured one counts as the whole bound).
    pub fn add(&mut self, next: Option<u64>) {
        self.acc += next.unwrap_or(bound());
        self.passes += 1;
    }

    /// The current piece was submitted as `sub`: it leaves the flight when the device finishes it, and
    /// the next piece, whose first pass is predicted at `next`, is admitted against the device time
    /// still in flight (stream-history-on-device D6). Nothing waits for the piece itself: the
    /// admission waits only while the ring's bound is full or a higher class waits (found-defects 392:
    /// a wait per piece was ~45 % of a Datamosh walk).
    pub fn cut(&mut self, device: &Arc<wgpu::Device>, queue: &wgpu::Queue, sub: wgpu::SubmissionIndex, surface: bool, next: Option<u64>) {
        trace::ev(Ev::GpuPiece)
            .id(self.ring.id)
            .aux(self.class as u32)
            .a(self.index as u64)
            .b(self.acc)
            .c(self.passes as u64)
            .emit();
        PIECES.fetch_add(1, Ordering::Relaxed);
        self.note_exec();
        self.note_over();
        if let Some(t) = self.ticket.take() {
            drop(t.submitted(device, queue, sub, self.acc, surface));
        }
        self.index += 1;
        self.acc = 0;
        self.passes = 0;
        self.ticket = Some(self.ring.admit(self.class, claim_after_cut(&self.ring, self.class, next)));
    }

    /// The piece goes to the device (gpu-throughput D6).
    fn note_exec(&self) {
        trace::ev(Ev::GpuExec).id(self.ring.id).aux(0).a(self.index as u64).b(self.acc).c(self.passes as u64).emit();
    }

    fn note_over(&self) {
        if self.passes > 1 && self.acc > self.ring.piece_limit(self.class) && self.ring.has_display() {
            OVER.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// The frame's last submission: it leaves the flight when the device finishes it.
    pub fn end(mut self, device: &Arc<wgpu::Device>, queue: &wgpu::Queue, sub: wgpu::SubmissionIndex, surface: bool) -> Option<Submitted> {
        self.note_exec();
        if self.index > 0 {
            self.note_over();
            trace::ev(Ev::GpuPiece)
                .id(self.ring.id)
                .aux(self.class as u32)
                .a(self.index as u64)
                .b(self.acc)
                .c(self.passes as u64)
                .emit();
        }
        self.ticket.take().map(|t| t.submitted(device, queue, sub, self.acc, surface))
    }

    /// How many pieces the frame was cut into so far.
    pub fn pieces(&self) -> u32 {
        self.index + 1
    }
}

/// What the piece after a cut claims on `ring`: its first pass's measured time, capped by the bound
/// on a bounded ring; an unmeasured pass claims the whole bound, as the first piece of an unmeasured
/// frame does. The whole bound for every next piece meant waiting until nothing else was in flight —
/// the display's own frame included.
fn claim_after_cut(ring: &Ring, class: Class, next: Option<u64>) -> u64 {
    let claim = next.unwrap_or(bound());
    if ring.bounded() { claim.min(ring.piece_limit(class)) } else { claim }
}

static PIECES: AtomicU64 = AtomicU64::new(0);
static OVER: AtomicU64 = AtomicU64::new(0);

/// Pieces of several passes whose prediction went past the bound on a displayed ring (t2253: none).
pub fn pieces_over() -> u64 {
    OVER.load(Ordering::Relaxed)
}

/// Pieces cut in the process since it started (tests, the bench).
pub fn pieces_cut() -> u64 {
    PIECES.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn ring() -> Arc<Ring> {
        ring_for(RingKey::Device(NEXT_RING.fetch_add(1, Ordering::Relaxed) as usize + 1_000_000))
    }

    /// Test t2250: the highest waiting class is admitted first, whatever order the waiters came in.
    #[test]
    fn t2250_the_highest_waiting_class_is_admitted_first() {
        let r = ring();
        let _d = r.register_display();
        let first = r.admit(Class::Background, bound());
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut hs = Vec::new();
        for c in [Class::Offline, Class::Near, Class::Now] {
            let (r2, order) = (r.clone(), order.clone());
            hs.push(std::thread::spawn(move || {
                let t = r2.admit(c, bound());
                order.lock().unwrap().push(c);
                drop(t);
            }));
            // each waits before the next comes
            let deadline = kt_time::test_deadline();
            while r.lock().waiting[c as usize] == 0 {
                assert!(std::time::Instant::now() < deadline, "{c:?} never waited");
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        drop(first);
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(*order.lock().unwrap(), vec![Class::Now, Class::Near, Class::Offline]);
    }

    /// Test t2251: a ring with a display holds at most the bound in flight (one piece always fits
    /// an idle ring); a ring with neither a display nor a compute host admits everything.
    #[test]
    fn t2251_the_in_flight_bound_holds_only_with_a_display() {
        let free = ring();
        let a = free.admit(Class::Offline, bound());
        let b = free.admit(Class::Offline, bound());
        assert_eq!(free.in_flight_ns(), 2 * bound(), "no display: both admitted at once");
        drop((a, b));

        let shown = ring();
        let _d = shown.register_display();
        let big = shown.admit(Class::Offline, 3 * bound());
        assert_eq!(shown.in_flight_ns(), 3 * bound(), "one piece always fits an idle ring");
        let admitted = Arc::new(AtomicBool::new(false));
        let h = {
            let (r, admitted) = (shown.clone(), admitted.clone());
            std::thread::spawn(move || {
                let t = r.admit(Class::Offline, bound() / 2);
                admitted.store(true, Ordering::Release);
                drop(t);
            })
        };
        std::thread::sleep(Duration::from_millis(50));
        assert!(!admitted.load(Ordering::Acquire), "admitted past the bound with a display");
        drop(big);
        h.join().unwrap();
        assert!(admitted.load(Ordering::Acquire));
        // two that fit the bound together go together
        let x = shown.admit(Class::Offline, bound() / 2);
        let y = shown.admit(Class::Offline, bound() / 2);
        assert_eq!(shown.in_flight_ns(), bound());
        drop((x, y));
    }

    /// Test t2356 (stream-history-on-device, found-defects 392): the piece after a cut claims its
    /// first pass's predicted time, so it is admitted while the earlier piece and the display's frame
    /// are still in flight when they fit the bound together; one that does not fit waits for them.
    #[test]
    fn t2356_a_cut_goes_on_while_the_bound_has_room() {
        let r = ring();
        let _d = r.register_display();
        let b = bound();
        // the display's frame and the earlier piece, both submitted and not yet done
        let display = r.admit(Class::Now, b * 3 / 10);
        let earlier = r.admit(Class::Offline, b * 3 / 10);
        let claim = claim_after_cut(&r, Class::Offline, Some(b / 5));
        assert_eq!(claim, b / 5);
        let admitted = Arc::new(AtomicBool::new(false));
        let h = {
            let (r2, admitted) = (r.clone(), admitted.clone());
            std::thread::spawn(move || {
                let t = r2.admit(Class::Offline, claim);
                admitted.store(true, Ordering::Release);
                t
            })
        };
        let deadline = kt_time::test_deadline();
        while !admitted.load(Ordering::Acquire) {
            assert!(std::time::Instant::now() < deadline, "the next piece waited although it fits the bound");
            std::thread::sleep(Duration::from_millis(1));
        }
        let next = h.join().unwrap();
        assert_eq!(r.in_flight_ns(), b * 3 / 10 * 2 + b / 5, "the three are in flight together");
        drop(next);
        // an unmeasured pass claims a whole lower-class piece: half the bound (gpu-throughput D1)
        let unmeasured = claim_after_cut(&r, Class::Offline, None);
        assert_eq!(unmeasured, b / 2);
        let waited = Arc::new(AtomicBool::new(false));
        let h = {
            let (r2, waited) = (r.clone(), waited.clone());
            std::thread::spawn(move || {
                let t = r2.admit(Class::Offline, unmeasured);
                waited.store(true, Ordering::Release);
                drop(t);
            })
        };
        std::thread::sleep(Duration::from_millis(50));
        assert!(!waited.load(Ordering::Acquire), "a half-bound piece was admitted past the bound");
        drop((display, earlier));
        h.join().unwrap();
        assert!(waited.load(Ordering::Acquire));
    }

    /// Marks a ticket submitted at `at` without a device (the drain's clock).
    fn submitted_at(r: &Ring, t: &Ticket, at: Instant) {
        if let Some(f) = r.lock().flight.iter_mut().find(|f| f.id == t.id) {
            f.at = Some(at);
        }
    }

    /// Test t2430 (gpu-throughput, found-defects 480): on a displayed ring with no display traffic a
    /// lower class keeps pieces queued while their predicted drain fits the bound — more than one
    /// uncompleted at once — and never more than `AHEAD_PIECES`.
    #[test]
    fn t2430_a_lower_class_keeps_a_piece_queued_while_one_runs() {
        let r = ring();
        let _d = r.register_display();
        let b = bound();
        let held: Vec<Ticket> = (0..AHEAD_PIECES).map(|_| r.admit(Class::Near, b / 4)).collect();
        assert_eq!(r.in_flight_ns(), AHEAD_PIECES as u64 * b / 4, "pieces that fit together were not admitted together");
        let admitted = Arc::new(AtomicBool::new(false));
        let h = {
            let (r2, admitted) = (r.clone(), admitted.clone());
            std::thread::spawn(move || {
                let t = r2.admit(Class::Near, b / 4);
                admitted.store(true, Ordering::Release);
                drop(t);
            })
        };
        std::thread::sleep(Duration::from_millis(50));
        assert!(!admitted.load(Ordering::Acquire), "more than {AHEAD_PIECES} lower pieces uncompleted");
        let mut held = held;
        drop(held.remove(0));
        h.join().unwrap();
        assert!(admitted.load(Ordering::Acquire));
    }

    /// Test t2431 (gpu-throughput, found-defects 480): a waiter is admitted when the predicted drain
    /// makes room, with no completion: an earlier piece submitted at the whole bound and never done
    /// lets a half-bound piece in once half of it has passed.
    #[test]
    fn t2431_a_waiter_is_admitted_when_the_drain_makes_room() {
        let r = ring();
        let _d = r.register_display();
        let b = bound();
        let earlier = r.admit(Class::Near, b);
        let start = Instant::now();
        submitted_at(&r, &earlier, start);
        let t = r.admit(Class::Near, b / 2);
        let waited = start.elapsed();
        assert!(!earlier.done.load(Ordering::Acquire), "the earlier piece completed");
        assert!(waited >= Duration::from_nanos(b * 45 / 100), "admitted after {waited:?}, before the drain made room");
        drop((t, earlier));
    }

    /// Test t2433 (gpu-throughput D1), the ring's part: the display's present goes into the flight
    /// with no wait; a lower class may queue what drains before the display's NEXT present plus the
    /// bound (a period away while it presents), not more; a display behind its frames holds lower
    /// work only for the bound past its own frame's end.
    #[test]
    fn t2433_the_display_is_counted_never_admitted() {
        let r = ring();
        let _d = r.register_display();
        let b = bound();
        let period = Duration::from_nanos(kt_time::sched::frame_period_ns());
        // a lower piece holds the bound, the display's present still goes at once
        let low = r.admit(Class::Near, b);
        let shown = r.note_display(b / 4);
        assert_eq!(r.in_flight_ns(), b + b / 4, "the present is not in the flight");
        drop(low);
        let now = Instant::now();
        submitted_at(&r, &shown, now);
        {
            let st = r.lock();
            // the next present is a period away: two bounds of lower work drain before it plus the bound
            assert!(r.display_next(&st, now) >= now + period / 2);
            let room = (period + Duration::from_nanos(b)).as_nanos() as u64 - b / 4;
            assert!(r.admissible(&st, Class::Near, room - 1_000_000, now).is_ok(), "room in front of the next present refused");
            assert!(r.admissible(&st, Class::Near, room + 1_000_000, now).is_err(), "more than the room admitted");
        }
        drop(shown);
        // the display behind: its frame ends long past its next present
        let late = r.note_display(8 * b);
        submitted_at(&r, &late, Instant::now());
        let st = r.lock();
        let now = Instant::now();
        let end = drain(&st, now, false).display_end.unwrap();
        assert!(end > r.display_next(&st, now));
        assert!(r.admissible(&st, Class::Near, b - 1000, now).is_ok(), "a piece behind the display's late frame was refused");
        drop(st);
        drop(late);
    }

    /// Test t2252: devices of one card share a ring whatever their backend; a card that names no
    /// PCI id joins it; a software device is a ring of its own.
    #[test]
    fn t2252_devices_of_one_card_share_a_ring() {
        let vk = ring_for(RingKey::Card(0x1002, 0x73bf));
        let gl = ring_for(RingKey::Card(0x1002, 0x73bf));
        assert!(Arc::ptr_eq(&vk, &gl));
        let unnamed = ring_for(RingKey::Card(0, 0));
        assert!(Arc::ptr_eq(&vk, &unnamed), "a card with no id joins the live card ring");
        let a = ring();
        let b = ring();
        assert!(!Arc::ptr_eq(&a, &b), "two software devices, two rings");
        assert!(!Arc::ptr_eq(&a, &vk));
    }
}
