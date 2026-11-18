//! The render-ahead producer (render-ahead, `docs/architecture-review-26.11.17.md` R2; the owner,
//! kkt-26.11.17 S4). While the transport plays, the display only presents composites its executor keeps
//! (`compose_queue`); this one producer composes the moments ahead of the frame on screen and keeps them
//! there. It replaced the loop fill (its lanes, width, claims, narrow mode and display gate).
//!
//! - **Order** (D2): from the frame on screen plus the measured lead (the wall a moment takes, at least a
//!   frame) forward — inside a loop region round the whole region, outside one until the first moment
//!   whose layers are not all decoded (the look-ahead's window, sized by the memory ledger, is the
//!   horizon) or the first moment with no layer (a gap or the end: the display clears those itself).
//!   Moments the display holds are noted for the bars; a moment the memo refused is skipped for the
//!   lap. A moment that came out incomplete WAITS for what it lacked to move (ahead-kept D1): a stream
//!   stage short of `k` positions until the walks computed `k` less what a composite computes itself,
//!   anything else until a walk or a decode moved. Skipped for the lap, a moment the walk reached a
//!   second later stayed unmade until the next lap: the owner's loop filled from lap 15 (kkt-26.11.18
//!   T2). It asks no decode and withdraws no request. Nothing runs while nothing plays.
//! - **Where** (D3): on the display's own device (a sibling engine: its own lock), the composite handed
//!   to the display's memo on the device with no readback and no wait — on a card and on a software
//!   rasterizer alike (measured: the side device's readback and upload cost more than they freed). The
//!   offline device's side engine, by bytes, only when the display has no sibling or for an A/B.
//! - **Depth** (D4): moments in flight at most `ceil(lat / own)` — a moment's wall from its plan to its
//!   completion over the producer's own time a moment — at least one, at most what the memory ledger has
//!   room for. Measured on the machine that runs it, never a constant of one machine (owner rule H13).
//!
//! `KT_RENDER_AHEAD=off` turns it off and the display composes on demand while playing (an A/B).

use crate::player::PreviewParts;
use crate::video_plan::{Ahead, VideoGpuExec};
use kt_media::Quality;
use kt_time::sched::{Class, Ctx, Lane, LaneRef, Pump};
use kt_time::Tick;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// `KT_RENDER_AHEAD=off`: no producer; the display composes on demand while playing.
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("KT_RENDER_AHEAD").as_deref() != Ok("off"))
}

/// `KT_RENDER_AHEAD_DEVICE=side`: compose on the offline device's side engine, by bytes (the A/B of the
/// display's own device; measured on llvmpipe the display's device showed more of a heavy loop's frames
/// in its first laps with a third less device work — the readback and the upload cost more than the side
/// device's own queue thread gave, render-ahead design "Measurements").
fn on_side_device() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("KT_RENDER_AHEAD_DEVICE").as_deref() == Ok("side"))
}

/// Moments in flight at most (D4): a moment's wall `lat_ns` over the producer's own time a moment
/// `own_ns`, rounded up, at least one; never more than `room` moments (what the memory ledger has room
/// for). Nothing measured yet — one.
pub fn depth(lat_ns: u64, own_ns: u64, room: Option<usize>) -> usize {
    let d = if own_ns == 0 { 1 } else { lat_ns.div_ceil(own_ns).max(1) as usize };
    match room {
        Some(r) => d.min(r.max(1)),
        None => d,
    }
}

/// An exponential mean of `ns` samples, the first taken as it is.
fn ema(old: u64, ns: u64) -> u64 {
    if old == 0 { ns } else { (3 * old + ns) / 4 }
}

/// The state a lap's skips belong to: versions, quality, region, build.
type LapKey = ((u64, u64), Quality, Option<(i64, i64)>, u64);

/// What an incomplete moment waits for before it is tried again (ahead-kept D1): the process's walked
/// history positions (`kt_plugin_api::stream::positions_walked`) and the frames cache's decoded count
/// (`BurstCache::video_inserted`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Waits {
    /// The walked count at or past which the moment may be complete.
    walked: u64,
    /// `Some(n)` — any walk or decode past these counts moves it (it lacked no history, or lacked it
    /// by what a composite computes): `(walked, inserted)` at the try.
    moved: Option<(u64, u64)>,
}

impl Waits {
    /// A try at counts `(walked, inserted)` came out incomplete, its stream stages short by `short`
    /// positions of which a composite computes `steps` itself.
    pub(crate) fn after(walked: u64, inserted: u64, short: u64, steps: u64) -> Waits {
        if short > steps {
            Waits { walked: walked + (short - steps), moved: None }
        } else {
            Waits { walked, moved: Some((walked, inserted)) }
        }
    }

    /// Whether the moment is worth another try at counts `(walked, inserted)`.
    pub(crate) fn ready(&self, walked: u64, inserted: u64) -> bool {
        match self.moved {
            None => walked >= self.walked,
            Some((w, i)) => walked != w || inserted != i,
        }
    }
}

/// A moment handed to the device and not yet finished.
struct Flight {
    done: Arc<AtomicBool>,
    /// When its plan began: its wall ends when the flag is seen.
    at: Instant,
}

#[derive(Default)]
struct St {
    key: Option<LapKey>,
    /// Moments skipped this lap (refused by the memo, no picture for the plan).
    skipped: BTreeSet<i64>,
    /// Moments that came out incomplete and what they wait for (D1), until the lap ends.
    waiting: BTreeMap<i64, Waits>,
    /// Moments the disk tier did not hold this lap (render-cache-disk D3), and the store's writes then.
    disk_miss: BTreeSet<i64>,
    disk_stores: u64,
    /// The playhead last seen: falling back is a new lap.
    head: i64,
    /// A moment's wall from its plan to its completion, and the producer's own time a moment (EMAs).
    lat_ns: u64,
    own_ns: u64,
    flights: VecDeque<Flight>,
    /// Composites kept, and the most moments in flight at once (tests, the bench).
    kept: u64,
    deepest: usize,
}

impl St {
    /// Retire the finished moments at the front: each one's wall is a sample of `lat`.
    fn retire(&mut self) {
        while let Some(f) = self.flights.front() {
            if !f.done.load(Ordering::Acquire) {
                break;
            }
            let ns = f.at.elapsed().as_nanos() as u64;
            self.lat_ns = ema(self.lat_ns, ns);
            self.flights.pop_front();
        }
    }
}

/// The executors the producer may compose on.
#[derive(Default)]
struct Execs {
    /// The display's executor it last chose for, by address.
    display: Option<usize>,
    /// The display's sibling (a card) — or the side engine.
    exec: Option<Arc<dyn VideoGpuExec>>,
    /// The offline device's side engine (kt-app, the bench, tests): a display with no sibling's producer.
    side: Option<Arc<dyn VideoGpuExec>>,
    /// A test's choice over the rule: `Some(None)` — no producer, `Some(Some(e))` — this executor.
    forced: Option<Option<Arc<dyn VideoGpuExec>>>,
}

/// The player's producer: its executors, its state and its lane.
#[derive(Default)]
pub(crate) struct RenderAhead {
    execs: Mutex<Execs>,
    st: Mutex<St>,
    lane: Mutex<Option<LaneRef>>,
}

impl RenderAhead {
    /// The service on the scheduler, over the player's preview parts and loop region.
    pub(crate) fn start(self: &Arc<Self>, parts: PreviewParts, region: Arc<Mutex<Option<(i64, i64)>>>) {
        let me = self.clone();
        let lane = Lane::service("render-ahead", Class::Near, move |ctx| pump(&me, &parts, &region, ctx));
        *self.lane.lock().unwrap() = Some(lane);
    }

    /// The side engine of the offline device (a software display composes ahead there).
    pub(crate) fn set_side(&self, side: Option<Arc<dyn VideoGpuExec>>) {
        let old = {
            let mut e = self.execs.lock().unwrap();
            let old = std::mem::replace(&mut e.side, side);
            e.display = None;
            old
        };
        drop(old);
    }

    /// The executor to compose on for `display`: the display's sibling (made once per display, on the
    /// caller's thread), else the side engine; `None` — none.
    pub(crate) fn exec_for(&self, display: &Arc<dyn VideoGpuExec>) -> Option<Arc<dyn VideoGpuExec>> {
        let id = Arc::as_ptr(display) as *const () as usize;
        let mut e = self.execs.lock().unwrap();
        if let Some(f) = &e.forced {
            return f.clone();
        }
        if e.display != Some(id) {
            let made = if on_side_device() { e.side.clone() } else { display.sibling_exec().or_else(|| e.side.clone()) };
            let old = std::mem::replace(&mut e.exec, made);
            e.display = Some(id);
            drop(e);
            // the old executor drops outside the lock
            drop(old);
            e = self.execs.lock().unwrap();
        }
        e.exec.clone()
    }

    /// Tests: the producer's executor whatever the rule says (`Some(None)` — none), or the rule again.
    pub(crate) fn force(&self, forced: Option<Option<Arc<dyn VideoGpuExec>>>) {
        let old = std::mem::replace(&mut self.execs.lock().unwrap().forced, forced);
        drop(old);
    }

    /// The producer runs for `display` now: the display only presents while playing (D1).
    pub(crate) fn active(&self, display: &Arc<dyn VideoGpuExec>) -> bool {
        enabled() && self.exec_for(display).is_some()
    }

    /// Look for work.
    pub(crate) fn kick(&self) {
        if enabled()
            && let Some(l) = self.lane.lock().unwrap().as_ref()
        {
            l.kick();
        }
    }

    /// The producer skipped the moment `t` this lap (refused by the display's memo) or it waits for
    /// what it lacked (D1): the display composes it on demand, as it did a moment skipped incomplete.
    pub(crate) fn skipped(&self, t: i64) -> bool {
        let st = self.st.lock().unwrap();
        st.skipped.contains(&t) || st.waiting.contains_key(&t)
    }

    /// Composites kept so far.
    pub(crate) fn kept(&self) -> u64 {
        self.st.lock().unwrap().kept
    }

    /// The most moments that were in flight at once.
    pub(crate) fn deepest(&self) -> usize {
        self.st.lock().unwrap().deepest
    }

    /// The player drops: the lane closes (its pump holds this producer, so the cycle ends here).
    pub(crate) fn close(&self) {
        let lane = self.lane.lock().unwrap().take();
        drop(lane);
    }
}

/// The moments to look at, in order: from `start` round the region `[first, first + n·step)`, or
/// forward from `start` while `warm` says the moment's layers are decoded.
fn order(first: i64, n: usize, step: i64, start: i64) -> impl Iterator<Item = i64> {
    let k = if n == 0 { 0 } else { ((start - first).max(0) / step) as usize % n };
    (0..n).map(move |i| first + ((k + i) % n) as i64 * step)
}

/// One moment, or nothing to do.
fn pump(ra: &RenderAhead, parts: &PreviewParts, region: &Mutex<Option<(i64, i64)>>, ctx: &Ctx) -> Pump {
    if !enabled() || ctx.cancelled() || !parts.transport.is_playing() {
        return Pump::Idle;
    }
    let Some(display) = parts.display_exec() else { return Pump::Idle };
    let Some(exec) = ra.exec_for(&display) else { return Pump::Idle };
    // a build in flight: its version's moments are the ones to make, and the prefetch kicks again
    let built = parts.builds.latest();
    if built.seq < parts.builds.want_seq() {
        return Pump::Idle;
    }
    let video = built.video.clone();
    let q = parts.quality();
    let versions = parts.versions();
    let step = video.grid.frame_step().max(1);
    let head = parts.transport.playhead();
    let region = *region.lock().unwrap();
    let inside = region
        .map(|(s, e)| (Tick::from_audio_frames(s).0, Tick::from_audio_frames(e).0))
        .filter(|&(s, e)| s <= head.0 && head.0 < e);
    let (lead, room) = {
        let mut st = ra.st.lock().unwrap();
        let key = (versions, q, inside, built.seq);
        if st.key != Some(key) || head.0 < st.head {
            st.key = Some(key);
            st.skipped.clear();
            // a new lap tries every waiting moment again: never later than the skip it replaced, if a
            // walk ever moves a history without counting its positions
            st.waiting.clear();
            st.disk_miss.clear();
        }
        // the prerender wrote since: a moment missed on disk may be there now
        let stores = parts.rc.counts().0;
        if st.disk_stores != stores {
            st.disk_stores = stores;
            st.disk_miss.clear();
        }
        st.head = head.0;
        st.retire();
        // the moment the display reaches when this one is done: a moment's wall ahead, at least a frame
        let lead = (st.lat_ns as i128 * kt_time::TICK_HZ as i128 / 1_000_000_000) as i64;
        let bytes = (q.width as u64 * q.height as u64 * 4).max(1);
        (lead.max(step), kt_time::mem::room().map(|r| (r / bytes) as usize))
    };
    let start = head.0 + lead;
    let kept: BTreeSet<i64> = parts
        .shown
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, (_, p, c))| (*p, *c) == versions)
        .map(|(t, _)| *t)
        .collect();
    let warm = |t: Tick| parts.layers_cached(&video, t, q);
    let moments: Box<dyn Iterator<Item = i64>> = match inside {
        Some((s, e)) => {
            let mut first = video.grid.align_floor(Tick(s)).0;
            if first < s {
                first += step;
            }
            let n = if e > first { ((e - first + step - 1) / step) as usize } else { 0 };
            Box::new(order(first, n, step, start))
        }
        None => {
            let from = video.grid.align_floor(Tick(start)).0;
            Box::new((0..).map(move |i| from + i * step))
        }
    };
    let linear = inside.is_none();
    let mut buf = Vec::new();
    let progress = || (kt_plugin_api::stream::positions_walked(), parts.cache.video_inserted());
    let steps = video.stream_compose_steps().unwrap_or(0) as u64;
    for t in moments {
        if ctx.cancelled() || !parts.transport.is_playing() {
            return Pump::Idle;
        }
        // linear play: a moment with no layer (a gap, the end) ends the walk; the display composes an
        // empty moment itself (a clear)
        if linear && video.layers_at(Tick(t), q).is_empty() {
            return Pump::Idle;
        }
        {
            let st = ra.st.lock().unwrap();
            if kept.contains(&t) || st.skipped.contains(&t) {
                continue;
            }
            // incomplete before: nothing it lacked has moved since
            if let Some(w) = st.waiting.get(&t) {
                let (walked, inserted) = progress();
                if !w.ready(walked, inserted) {
                    continue;
                }
            }
        }
        // the disk tier holds it (render-cache-disk D3): restored into the display's memo, no decode
        if parts.rc.is_on() && !ra.st.lock().unwrap().disk_miss.contains(&t) {
            if parts.restore_from_disk(&display, &video, Tick(t), q).is_some() {
                return Pump::More;
            }
            ra.st.lock().unwrap().disk_miss.insert(t);
        }
        if !warm(Tick(t)) {
            // linear play: past the decoded window nothing is composable yet
            if linear {
                return Pump::Idle;
            }
            continue;
        }
        // the display may hold it with the bars' map behind: noted, no composite
        if let Some(k) = display.present_key(&video.probe_plan(&parts.cache, Tick(t), q))
            && display.holds(k)
        {
            parts.note_kept(Tick(t), k);
            continue;
        }
        // room for one more in flight: wait for the oldest, polling the device without blocking
        loop {
            let (full, len) = {
                let mut st = ra.st.lock().unwrap();
                st.retire();
                (st.flights.len() >= depth(st.lat_ns, st.own_ns, room), st.flights.len())
            };
            if !full {
                break;
            }
            if ctx.cancelled() || !parts.transport.is_playing() {
                return Pump::Idle;
            }
            exec.poll_done();
            if len > 0 {
                std::thread::sleep(Duration::from_micros(250));
            }
        }
        let id = kt_time::trace::next_id();
        let _cause = kt_time::trace::with_cause(id);
        let mut span = kt_time::trace::span(kt_time::trace::Ev::Job);
        // the moments in flight with this one
        let in_flight = ra.st.lock().unwrap().flights.len() + 1;
        span.aux(kt_time::trace::job::RENDER_AHEAD).id(id).a(t as u64).c(in_flight as u64);
        let at = Instant::now();
        let (walked, inserted) = progress();
        // a stream history this moment walks stops when the lane closes (close-while-busy): the
        // player's drop closes it, and a walk of seconds held the producer's thread past the close
        let out = crate::video::stop_when(kt_time::trace::stop::CLOSED, ctx.cancel_check(), || {
            parts.compose_ahead(&exec, &display, &video, Tick(t), q, &mut buf)
        });
        let short = video.stream_short();
        let own = at.elapsed().as_nanos() as u64;
        let Some(out) = out else {
            span.cancel();
            continue;
        };
        let mut st = ra.st.lock().unwrap();
        st.own_ns = ema(st.own_ns, own);
        match out {
            Ok(Ahead { complete, kept: Some(k), done }) if complete => {
                span.b(0);
                st.kept += 1;
                st.waiting.remove(&t);
                drop(st);
                parts.note_kept(Tick(t), k);
                let mut st = ra.st.lock().unwrap();
                match done {
                    Some(done) => st.flights.push_back(Flight { done, at }),
                    None => st.lat_ns = ema(st.lat_ns, own),
                }
                // a moment composed while the call lasted was one in flight
                st.deepest = st.deepest.max(st.flights.len()).max(1);
            }
            Ok(a) => {
                if a.complete {
                    // refused by the display's memo: skipped for the lap
                    span.b(3);
                    st.skipped.insert(t);
                } else {
                    // incomplete: it waits for what it lacked (D1); `b` 4 — its history was short
                    span.b(if short > steps { 4 } else { 1 });
                    st.waiting.insert(t, Waits::after(walked, inserted, short, steps));
                }
                if a.done.is_none() {
                    st.lat_ns = ema(st.lat_ns, own);
                }
            }
            // no picture for this plan (too large, refused): skipped for the lap
            Err(_) => {
                span.b(1);
                st.skipped.insert(t);
            }
        }
        return Pump::More;
    }
    Pump::Idle
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test t2921 (render-ahead D4): the depth is a moment's wall over the producer's own time a moment,
    /// rounded up, at least one; a synchronous moment (wall = own time) is one; the memory's room caps it.
    #[test]
    fn t2921_the_depth_comes_from_measurement_and_memory() {
        assert_eq!(depth(0, 0, None), 1, "nothing measured: one");
        assert_eq!(depth(12_000_000, 12_000_000, None), 1, "a synchronous moment: one");
        assert_eq!(depth(40_000_000, 4_000_000, None), 10, "a 40 ms round trip fed at 4 ms a moment");
        assert_eq!(depth(41_000_000, 4_000_000, None), 11, "rounded up");
        assert_eq!(depth(40_000_000, 4_000_000, Some(3)), 3, "the room caps it");
        assert_eq!(depth(40_000_000, 4_000_000, Some(0)), 1, "never zero");
        assert_eq!(depth(1_000, 4_000_000, None), 1, "a moment faster than its own time: one");
    }

    /// Test t2931 (ahead-kept D1, kkt-26.11.18 T2): an incomplete moment waits for what it lacked, not
    /// for the lap's end — a stream history short by `k` until the walks computed `k` less what a
    /// composite computes itself, anything else until a walk or a decode moved.
    #[test]
    fn t2931_an_incomplete_moment_waits_for_what_it_lacked() {
        let w = Waits::after(100, 7, 64, 2);
        assert!(!w.ready(100, 7), "nothing moved");
        assert!(!w.ready(161, 9), "61 positions walked of the 62 it lacks beyond a composite's own");
        assert!(w.ready(162, 7), "the walk reached a composite's reach");
        let w = Waits::after(100, 7, 2, 2);
        assert!(!w.ready(100, 7), "nothing moved");
        assert!(w.ready(101, 7), "a walk moved");
        assert!(w.ready(100, 8), "a decode moved");
        let w = Waits::after(100, 7, 0, 0);
        assert!(w.ready(100, 8) && !w.ready(100, 7), "no stream stage: a decode moves it");
    }

    /// The order round a region starts at the moment the start falls in and visits every moment once.
    #[test]
    fn the_order_goes_round_the_region_once() {
        let v: Vec<i64> = order(100, 5, 10, 131).collect();
        assert_eq!(v, vec![130, 140, 100, 110, 120]);
        assert_eq!(order(100, 5, 10, 90).collect::<Vec<_>>(), vec![100, 110, 120, 130, 140]);
        assert_eq!(order(100, 0, 10, 90).count(), 0);
    }
}
