//! The loop fill (scheduler-followups D5, found-defects 251): while a loop plays, the moments of
//! it the display's executor does not keep are composed below the display on executors of their
//! own, and kept on the display's executor as a slow-chain composite is. On a display slower than
//! the project's frames each lap showed a different subset and only shown moments were kept, so a
//! covered loop composed "for the first time" lap after lap.
//!
//! The rules: only a moment whose every layer is decoded (the fill asks no decode and withdraws
//! no request of the frame on screen); ahead of the playhead by twice its own frame time (the
//! moment at the head is the compose thread's); a moment that comes out incomplete (a stream stage
//! short of its frame included) or not kept by the display's memo is skipped for the lap; nothing
//! runs while nothing plays. The class is `near`: on every ring it is admitted behind the frame on
//! screen. `KT_LOOP_FILL=off` turns it off (an A/B on one binary).
//!
//! How many moments at once (cpu-idle-work, kkt-26.11.15 item 4): a moment is a chain of round
//! trips — its pieces admitted one after another, then the readback — and on the owner's iGPU one
//! took ~170 ms of wall for ~20 ms of device time while the processor and the card idled. So on a
//! hardware device the fill runs [`width`] moments at once, each on an executor of its own (a
//! sibling: same device and ring, its own lock), and no moment waits for the display to end a
//! composite of its own (the ring puts the frame on screen first). On a software rasterizer the
//! fill and the display share the processor: one moment at a time, none while the display composes
//! (it takes the time the display leaves). A moment that comes out incomplete narrows the fill to
//! one for the rest of the lap: parallel moments must not take the device from the history walk
//! the display needs. `KT_LOOP_FILL_WIDTH=N` sets the width (an A/B on one binary).

use crate::player::PreviewParts;
use crate::video_plan::VideoGpuExec;
use kt_media::Quality;
use kt_time::sched::{Class, Ctx, Lane, LaneRef, Pump};
use kt_time::Tick;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

/// `KT_LOOP_FILL=off`: the fill never runs.
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("KT_LOOP_FILL").as_deref() != Ok("off"))
}

/// Lower-class pieces a ring admits uncompleted in front of the display (kt-gpu's
/// `arbiter::AHEAD_PIECES`, which kt-engine cannot name): more fill moments at once only wait in its
/// admission. They change together.
pub const RING_LOWER_PIECES: usize = 3;

/// The lanes' names, one per moment in flight; the width never passes their count.
const LANES: [&str; 8] =
    ["loop-fill", "loop-fill-1", "loop-fill-2", "loop-fill-3", "loop-fill-4", "loop-fill-5", "loop-fill-6", "loop-fill-7"];

/// `KT_LOOP_FILL_WIDTH=N`, read once.
fn width_override() -> Option<usize> {
    static W: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
    *W.get_or_init(|| std::env::var("KT_LOOP_FILL_WIDTH").ok().and_then(|v| v.trim().parse().ok()))
}

/// How many moments the fill composes at once: a software rasterizer one (the display shares the
/// processor); a hardware device what the scheduler's pool of `workers` leaves beside one worker for
/// a person's answer, at most the ring's lower pieces in flight; `forced` (the variable) over both.
/// The pool is sized from the machine's cores at run time (`sched::workers`).
pub fn width(software: bool, workers: usize, forced: Option<usize>) -> usize {
    if let Some(n) = forced {
        return n.clamp(1, LANES.len());
    }
    if software { 1 } else { workers.saturating_sub(1).clamp(1, RING_LOWER_PIECES) }
}

/// The state a lap's skips belong to: versions, quality, region, build.
type LapKey = ((u64, u64), Quality, (i64, i64), u64);

/// What the fill keeps between its moments.
#[derive(Default)]
struct St {
    key: Option<LapKey>,
    /// Moments skipped this lap.
    skipped: BTreeSet<i64>,
    /// Moments a lane composes now: no other lane takes them.
    claimed: BTreeSet<i64>,
    /// A moment came out incomplete this lap: one lane only until the lap moves.
    narrow: bool,
    /// The playhead it last saw: falling back is a new lap.
    head: i64,
    /// Its last composite's time (the lead ahead of the playhead).
    last_ns: u64,
    /// Composites kept, for tests and the bench.
    kept: u64,
}

/// The player's loop fill: its executors (one per lane), its state and its lanes.
#[derive(Default)]
pub(crate) struct LoopFill {
    execs: Mutex<Vec<Arc<dyn VideoGpuExec>>>,
    st: Mutex<St>,
    lanes: Mutex<Vec<LaneRef>>,
}

impl LoopFill {
    /// The service on the scheduler, over the player's preview parts and loop region: one lane per
    /// moment it may compose at once (a lane past the width stays idle and costs nothing).
    pub(crate) fn start(parts: PreviewParts, region: Arc<Mutex<Option<(i64, i64)>>>) -> Arc<LoopFill> {
        let fill = Arc::new(LoopFill::default());
        let lanes = LANES
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let (f, parts, region) = (fill.clone(), parts.clone(), region.clone());
                Lane::service(name, Class::Near, move |ctx| pump(&f, i, &parts, &region, ctx))
            })
            .collect();
        *fill.lanes.lock().unwrap() = lanes;
        fill
    }

    /// The fill's executor, and as many siblings of it as the width asks (made here, on the
    /// caller's thread: an engine's frame state is created on its device).
    pub(crate) fn set_exec(&self, exec: Option<Arc<dyn VideoGpuExec>>) {
        let mut execs = Vec::new();
        if let Some(e) = exec {
            let n = width(e.info().software, kt_time::sched::workers(), width_override());
            execs.push(e.clone());
            while execs.len() < n {
                match e.sibling_exec() {
                    Some(s) => execs.push(s),
                    None => break,
                }
            }
        }
        // the old executors drop outside the lock
        let old = std::mem::replace(&mut *self.execs.lock().unwrap(), execs);
        drop(old);
    }

    /// A pass of the player's prefetch thread while a loop plays: look for work.
    pub(crate) fn kick(&self) {
        if enabled() && !self.execs.lock().unwrap().is_empty() {
            self.kick_lane(0);
        }
    }

    /// Kick lane `i` when it is within the width.
    fn kick_lane(&self, i: usize) {
        if i < self.execs.lock().unwrap().len()
            && let Some(l) = self.lanes.lock().unwrap().get(i)
        {
            l.kick();
        }
    }

    /// Composites the fill kept so far.
    pub(crate) fn kept(&self) -> u64 {
        self.st.lock().unwrap().kept
    }

    /// Moments it composes at once on its executor (tests, the bench).
    pub(crate) fn width(&self) -> usize {
        self.execs.lock().unwrap().len()
    }

    /// The player drops: the lanes close (their pumps hold this fill, so the cycle ends here).
    pub(crate) fn close(&self) {
        let lanes = std::mem::take(&mut *self.lanes.lock().unwrap());
        drop(lanes);
    }
}

/// The display composes a frame now. On a software rasterizer, where the fill and the display share
/// the processor, the fill takes only the time the display replays kept moments.
fn display_busy(parts: &PreviewParts) -> bool {
    parts.composing.load(std::sync::atomic::Ordering::Acquire) > 0
}

/// Releases a lane's claim on its moment however the composite ends.
struct Claim<'a>(&'a LoopFill, i64);

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        self.0.st.lock().unwrap().claimed.remove(&self.1);
    }
}

/// One moment of lane `lane`, or nothing to do.
fn pump(f: &LoopFill, lane: usize, parts: &PreviewParts, region: &Mutex<Option<(i64, i64)>>, ctx: &Ctx) -> Pump {
    if !enabled() || ctx.cancelled() || !parts.transport.is_playing() {
        return Pump::Idle;
    }
    let Some(exec) = f.execs.lock().unwrap().get(lane).cloned() else { return Pump::Idle };
    let gated = exec.info().software;
    if gated && display_busy(parts) {
        return Pump::Idle;
    }
    let Some((s, e)) = *region.lock().unwrap() else { return Pump::Idle };
    let Some(display) = parts.display_exec() else { return Pump::Idle };
    // a build in flight: its version's moments are the ones to make, and the prefetch kicks again
    let built = parts.builds.latest();
    if built.seq < parts.builds.want_seq() {
        return Pump::Idle;
    }
    let video = built.video.clone();
    let q = parts.quality();
    let versions = parts.versions();
    let step = video.grid.frame_step().max(1);
    let (ts, te) = (Tick::from_audio_frames(s), Tick::from_audio_frames(e));
    let mut first = video.grid.align_floor(ts);
    if first < ts {
        first = Tick(first.0 + step);
    }
    if te.0 <= first.0 {
        return Pump::Idle;
    }
    let n = ((te.0 - first.0 + step - 1) / step) as usize;
    let head = parts.transport.playhead();
    let lead = {
        let mut st = f.st.lock().unwrap();
        let key = (versions, q, (s, e), built.seq);
        if st.key != Some(key) || head.0 < st.head {
            st.key = Some(key);
            st.skipped.clear();
            st.narrow = false;
        }
        st.head = head.0;
        if lane > 0 && st.narrow {
            return Pump::Idle;
        }
        // twice its own frame time ahead, at least a frame: the moment at the head is the display's
        let lead = (st.last_ns as i128 * 2 * kt_time::TICK_HZ as i128 / 1_000_000_000) as i64;
        lead.max(step)
    };
    let kept: BTreeSet<i64> = parts
        .shown
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, (_, p, c))| (*p, *c) == versions)
        .map(|(t, _)| *t)
        .collect();
    let start = (head.0 + lead - first.0).max(0) / step;
    for i in 0..n {
        let t = Tick(first.0 + ((start as usize + i) % n) as i64 * step);
        if kept.contains(&t.0) {
            continue;
        }
        if ctx.cancelled() || !parts.transport.is_playing() || (gated && display_busy(parts)) {
            return Pump::Idle;
        }
        // take the moment, or leave it to the lane that has it
        let _claim = {
            let mut st = f.st.lock().unwrap();
            if st.skipped.contains(&t.0) || st.claimed.contains(&t.0) {
                continue;
            }
            if lane > 0 && st.narrow {
                return Pump::Idle;
            }
            st.claimed.insert(t.0);
            Claim(f, t.0)
        };
        // the display may hold it with the bars' map behind: noted, no composite
        if let Some(k) = display.present_key(&video.probe_plan(&parts.cache, t, q))
            && display.holds(k)
        {
            parts.note_kept(t, k);
            continue;
        }
        // the next lane looks for a moment of its own while this one composes
        f.kick_lane(lane + 1);
        let id = kt_time::trace::next_id();
        let _cause = kt_time::trace::with_cause(id);
        let mut span = kt_time::trace::span(kt_time::trace::Ev::Job);
        span.aux(kt_time::trace::job::LOOP_FILL).id(id).a(t.0 as u64).c(lane as u64);
        let t0 = std::time::Instant::now();
        let mut buf = Vec::new();
        let Some((complete, key)) = parts.compose_fill(&exec, t, &mut buf) else {
            // not warm: the look-ahead brings it, the next pass looks again
            span.cancel();
            continue;
        };
        let ok = complete && key.is_some_and(|k| display.holds(k));
        span.b(if ok { 0 } else if !complete { 1 } else { 3 });
        drop(span);
        let mut st = f.st.lock().unwrap();
        st.last_ns = t0.elapsed().as_nanos() as u64;
        if ok {
            st.kept += 1;
        } else {
            st.skipped.insert(t.0);
            st.narrow |= !complete;
        }
        return Pump::More;
    }
    Pump::Idle
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test t2861: the width comes from the machine's pool and the device kind; the variable wins.
    #[test]
    fn t2861_the_width_comes_from_the_machine_and_the_device() {
        for workers in 3..=6 {
            assert_eq!(width(true, workers, None), 1, "a software device shares the processor with the display");
            let w = width(false, workers, None);
            assert_eq!(w, (workers - 1).min(RING_LOWER_PIECES), "a card: the pool less the answer's worker, within the ring's pieces");
            assert!(w >= 2, "a card always runs more than one moment at once");
        }
        assert_eq!(width(false, 1, None), 1, "a pool of one keeps one moment");
        assert_eq!(width(true, 6, Some(3)), 3, "the variable wins on a software device");
        assert_eq!(width(false, 6, Some(1)), 1, "the variable wins on a card");
        assert_eq!(width(false, 6, Some(0)), 1, "never zero");
        assert_eq!(width(false, 6, Some(99)), LANES.len(), "never past the lanes");
        // the pool's own rule, as this machine gives it
        let w = width(false, kt_time::sched::workers(), None);
        assert!((2..=RING_LOWER_PIECES).contains(&w), "this machine's card width {w}");
    }
}
