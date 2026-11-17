//! The render-ahead producer (render-ahead, `docs/architecture-review-26.11.17.md` R2; the owner,
//! kkt-26.11.17 S4). While the transport plays, the display only presents composites its executor keeps
//! (`compose_queue`); this one producer composes the moments ahead of the frame on screen and keeps them
//! there. It replaced the loop fill (its lanes, width, claims, narrow mode and display gate).
//!
//! - **Order** (D2): from the frame on screen plus the measured lead (the wall a moment takes, at least a
//!   frame) forward — inside a loop region round the whole region, outside one until the first moment
//!   whose layers are not all decoded (the look-ahead's window, sized by the memory ledger, is the
//!   horizon) or the first moment with no layer (a gap or the end: the display clears those itself).
//!   Moments the display holds are noted for the bars; a moment that came out incomplete (a
//!   stream stage short of its frame included) or that the memo refused is skipped for the lap. It asks
//!   no decode and withdraws no request. Nothing runs while nothing plays.
//! - **Where** (D3): on a card, on the display's own device (a sibling engine: its own lock), the
//!   composite handed to the display's memo on the device with no readback and no wait; on a software
//!   rasterizer, on the offline device's side engine, by bytes.
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
use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// `KT_RENDER_AHEAD=off`: no producer; the display composes on demand while playing.
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("KT_RENDER_AHEAD").as_deref() != Ok("off"))
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

/// A moment handed to the device and not yet finished.
struct Flight {
    done: Arc<AtomicBool>,
    /// When its plan began: its wall ends when the flag is seen.
    at: Instant,
}

#[derive(Default)]
struct St {
    key: Option<LapKey>,
    /// Moments skipped this lap.
    skipped: BTreeSet<i64>,
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
    /// The offline device's side engine (kt-app, the bench, tests): a software display's producer.
    side: Option<Arc<dyn VideoGpuExec>>,
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

    /// The executor to compose on for `display`: on a card the display's sibling (made once per
    /// display, on the caller's thread), on a software rasterizer the side engine; `None` — none.
    pub(crate) fn exec_for(&self, display: &Arc<dyn VideoGpuExec>) -> Option<Arc<dyn VideoGpuExec>> {
        let id = Arc::as_ptr(display) as *const () as usize;
        let mut e = self.execs.lock().unwrap();
        if e.display != Some(id) {
            let made = if display.info().software { e.side.clone() } else { display.sibling_exec().or_else(|| e.side.clone()) };
            let old = std::mem::replace(&mut e.exec, made);
            e.display = Some(id);
            drop(e);
            // the old executor drops outside the lock
            drop(old);
            e = self.execs.lock().unwrap();
        }
        e.exec.clone()
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

    /// The producer skipped the moment `t` this lap (incomplete, or refused by the display's memo).
    pub(crate) fn skipped(&self, t: i64) -> bool {
        self.st.lock().unwrap().skipped.contains(&t)
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
    for t in moments {
        if ctx.cancelled() || !parts.transport.is_playing() {
            return Pump::Idle;
        }
        // linear play: a moment with no layer (a gap, the end) ends the walk; the display composes an
        // empty moment itself (a clear)
        if linear && video.layers_at(Tick(t), q).is_empty() {
            return Pump::Idle;
        }
        if kept.contains(&t) || ra.st.lock().unwrap().skipped.contains(&t) {
            continue;
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
        let in_flight = ra.st.lock().unwrap().flights.len();
        span.aux(kt_time::trace::job::RENDER_AHEAD).id(id).a(t as u64).c(in_flight as u64);
        let at = Instant::now();
        let out = parts.compose_ahead(&exec, &display, &video, Tick(t), q, &mut buf);
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
                drop(st);
                parts.note_kept(Tick(t), k);
                let mut st = ra.st.lock().unwrap();
                match done {
                    Some(done) => st.flights.push_back(Flight { done, at }),
                    None => st.lat_ns = ema(st.lat_ns, own),
                }
                st.deepest = st.deepest.max(st.flights.len());
            }
            Ok(a) => {
                span.b(if a.complete { 3 } else { 1 });
                st.skipped.insert(t);
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

    /// The order round a region starts at the moment the start falls in and visits every moment once.
    #[test]
    fn the_order_goes_round_the_region_once() {
        let v: Vec<i64> = order(100, 5, 10, 131).collect();
        assert_eq!(v, vec![130, 140, 100, 110, 120]);
        assert_eq!(order(100, 5, 10, 90).collect::<Vec<_>>(), vec![100, 110, 120, 130, 140]);
        assert_eq!(order(100, 0, 10, 90).count(), 0);
    }
}
