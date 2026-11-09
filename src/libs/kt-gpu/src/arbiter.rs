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
//!   on any bounded ring it is cut at the next boundary while a higher class waits. A piece waits for
//!   the one before. The passes, their order and their targets do not change: the same bytes
//!   (invariant 11).
//! - **No submitter has to come back to free its piece**: a piece's completion is a work-done
//!   callback, and a waiter completes the oldest piece itself by a blocking wait on that piece's
//!   device when the device carries no surface (a blocking wait on the display device would hold
//!   egui's present, which takes the device's lock for writing).
//!
//! Every admission and piece is a trace record (`GpuAdmit`, `GpuPiece`).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};

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
    /// Its device and submission once submitted; a device with a surface is never waited on.
    sub: Option<(Arc<wgpu::Device>, wgpu::SubmissionIndex, bool)>,
    done: Arc<AtomicBool>,
}

#[derive(Default)]
struct State {
    flight: Vec<Flight>,
    waiting: [u32; Class::ALL.len()],
    next: u64,
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

    fn admissible(&self, st: &State, class: Class, ns: u64) -> bool {
        if st.waiting[..class as usize].iter().any(|&n| n > 0) {
            return false;
        }
        let live: Vec<&Flight> = st.flight.iter().filter(|f| !f.done.load(Ordering::Acquire)).collect();
        if live.is_empty() || !self.bounded() {
            return true;
        }
        live.iter().map(|f| f.ns).sum::<u64>() + ns <= bound()
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
        span.b(st.flight.iter().map(|f| f.ns).sum());
        if enabled() && !self.admissible(&st, class, ns) {
            st.waiting[class as usize] += 1;
            while !self.admissible(&st, class, ns) {
                // the oldest piece with a device nobody's present waits on: complete it here
                let oldest = st.flight.iter().find(|f| !f.done.load(Ordering::Acquire)).and_then(|f| {
                    f.sub.as_ref().filter(|s| !s.2).map(|s| (s.0.clone(), s.1.clone(), f.done.clone()))
                });
                match oldest {
                    Some((device, sub, done)) => {
                        drop(st);
                        let _ = device.poll(wgpu::PollType::Wait { submission_index: Some(sub), timeout: None });
                        done.store(true, Ordering::Release);
                        st = self.lock();
                    }
                    None => st = self.cv.wait(st).unwrap_or_else(|e| e.into_inner()),
                }
                st.flight.retain(|f| !f.done.load(Ordering::Acquire));
            }
            st.waiting[class as usize] -= 1;
            // a lower class may have been held only by this one
            self.cv.notify_all();
        }
        st.next += 1;
        let id = st.next;
        st.flight.push(Flight { id, ns, sub: None, done: done.clone() });
        drop(st);
        Ticket { ring: self.clone(), id, done, class, submitted: false }
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
        (self.has_display() && acc + next.unwrap_or(u64::MAX / 4) > bound()) || (self.bounded() && self.higher_waits(class))
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
        let claim = if ring.bounded() { ns.min(bound()) } else { ns };
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

    /// The current piece was submitted as `sub`: wait for it, then admit the next.
    pub fn cut(&mut self, device: &Arc<wgpu::Device>, queue: &wgpu::Queue, sub: wgpu::SubmissionIndex, surface: bool) {
        trace::ev(Ev::GpuPiece)
            .id(self.ring.id)
            .aux(self.class as u32)
            .a(self.index as u64)
            .b(self.acc)
            .c(self.passes as u64)
            .emit();
        PIECES.fetch_add(1, Ordering::Relaxed);
        self.note_over();
        if let Some(t) = self.ticket.take() {
            t.submitted(device, queue, sub, self.acc, surface).wait();
        }
        self.index += 1;
        self.acc = 0;
        self.passes = 0;
        self.ticket = Some(self.ring.admit(self.class, bound()));
    }

    fn note_over(&self) {
        if self.passes > 1 && self.acc > bound() && self.ring.has_display() {
            OVER.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// The frame's last submission: it leaves the flight when the device finishes it.
    pub fn end(mut self, device: &Arc<wgpu::Device>, queue: &wgpu::Queue, sub: wgpu::SubmissionIndex, surface: bool) -> Option<Submitted> {
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
