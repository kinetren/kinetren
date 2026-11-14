//! The loop fill (scheduler-followups D5, found-defects 251): while a loop plays, the moments of
//! it the display's executor does not keep are composed below the display, one at a time, on an
//! executor of their own, and kept on the display's executor as a slow-chain composite is. On a
//! display slower than the project's frames each lap showed a different subset and only shown
//! moments were kept, so a covered loop composed "for the first time" lap after lap.
//!
//! The rules: only a moment whose every layer is decoded (the fill asks no decode and withdraws
//! no request of the frame on screen); ahead of the playhead by twice its own frame time (the
//! moment at the head is the compose thread's); never while the display composes a frame of its own
//! (it takes the time the display leaves); a moment that comes out incomplete (a stream stage short of its frame included) or not
//! kept by the display's memo is skipped for the lap; nothing runs while nothing plays. The class
//! is `near`: on every ring it is admitted behind the frame on screen. `KT_LOOP_FILL=off` turns it
//! off (an A/B on one binary).

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

/// The state a lap's skips belong to: versions, quality, region, build.
type LapKey = ((u64, u64), Quality, (i64, i64), u64);

/// What the fill keeps between its moments.
#[derive(Default)]
struct St {
    key: Option<LapKey>,
    /// Moments skipped this lap.
    skipped: BTreeSet<i64>,
    /// The playhead it last saw: falling back is a new lap.
    head: i64,
    /// Its last composite's time (the lead ahead of the playhead).
    last_ns: u64,
    /// Composites kept, for tests and the bench.
    kept: u64,
}

/// The player's loop fill: its executor, its state and its lane.
#[derive(Default)]
pub(crate) struct LoopFill {
    exec: Mutex<Option<Arc<dyn VideoGpuExec>>>,
    st: Mutex<St>,
    lane: Mutex<Option<LaneRef>>,
}

impl LoopFill {
    /// The service on the scheduler, over the player's preview parts and loop region.
    pub(crate) fn start(parts: PreviewParts, region: Arc<Mutex<Option<(i64, i64)>>>) -> Arc<LoopFill> {
        let fill = Arc::new(LoopFill::default());
        let f = fill.clone();
        let lane = Lane::service("loop-fill", Class::Near, move |ctx| pump(&f, &parts, &region, ctx));
        *fill.lane.lock().unwrap() = Some(lane);
        fill
    }

    pub(crate) fn set_exec(&self, exec: Option<Arc<dyn VideoGpuExec>>) {
        *self.exec.lock().unwrap() = exec;
    }

    /// A pass of the player's prefetch thread while a loop plays: look for work.
    pub(crate) fn kick(&self) {
        if enabled()
            && self.exec.lock().unwrap().is_some()
            && let Some(l) = &*self.lane.lock().unwrap()
        {
            l.kick();
        }
    }

    /// Composites the fill kept so far.
    pub(crate) fn kept(&self) -> u64 {
        self.st.lock().unwrap().kept
    }

    /// The player drops: the lane closes (its pump holds this fill, so the cycle ends here).
    pub(crate) fn close(&self) {
        drop(self.lane.lock().unwrap().take());
    }
}

/// The display composes a frame now: the fill takes only the time the display does not use. On a
/// card that is the device's time between frames; on a software rasterizer, where the fill and the
/// display share the processor, it is the time the display replays kept moments.
fn display_busy(parts: &PreviewParts) -> bool {
    parts.composing.load(std::sync::atomic::Ordering::Acquire) > 0
}

/// One moment, or nothing to do.
fn pump(f: &LoopFill, parts: &PreviewParts, region: &Mutex<Option<(i64, i64)>>, ctx: &Ctx) -> Pump {
    if !enabled() || ctx.cancelled() || !parts.transport.is_playing() || display_busy(parts) {
        return Pump::Idle;
    }
    let Some(exec) = f.exec.lock().unwrap().clone() else { return Pump::Idle };
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
    let (lead, todo) = {
        let mut st = f.st.lock().unwrap();
        let key = (versions, q, (s, e), built.seq);
        if st.key != Some(key) || head.0 < st.head {
            st.key = Some(key);
            st.skipped.clear();
        }
        st.head = head.0;
        // twice its own frame time ahead, at least a frame: the moment at the head is the display's
        let lead = (st.last_ns as i128 * 2 * kt_time::TICK_HZ as i128 / 1_000_000_000) as i64;
        (lead.max(step), st.skipped.clone())
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
        if kept.contains(&t.0) || todo.contains(&t.0) {
            continue;
        }
        if ctx.cancelled() || !parts.transport.is_playing() || display_busy(parts) {
            return Pump::Idle;
        }
        // the display may hold it with the bars' map behind: noted, no composite
        if let Some(k) = display.present_key(&video.probe_plan(&parts.cache, t, q))
            && display.holds(k)
        {
            parts.note_kept(t, k);
            continue;
        }
        let id = kt_time::trace::next_id();
        let _cause = kt_time::trace::with_cause(id);
        let mut span = kt_time::trace::span(kt_time::trace::Ev::Job);
        span.aux(kt_time::trace::job::LOOP_FILL).id(id).a(t.0 as u64);
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
        }
        return Pump::More;
    }
    Pump::Idle
}
