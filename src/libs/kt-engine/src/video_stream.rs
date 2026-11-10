//! The STREAM source stage in the plan (datamosh-codec-path, design D1–D6).
//!
//! A clip whose chain holds a `VideoClass::Stream` slot takes as its pre-FX frame what the stage's
//! kernel produces from a stream of the event's source frames (the event before included, the
//! event on another track as a second input). The stage runs here, in the plan's common code, on
//! the CPU — the executor receives the stage's bytes as decoded ones (invariant 7). The order asks exactly
//! what the kernel says it still needs (invariant 4); the bytes do not depend on the memo
//! (invariant 11).

use std::sync::{Arc, OnceLock};

use kt_cache::{BurstCache, SourceId};
use kt_media::{Quality, VideoFrame};
use kt_model::MediaId;
use kt_plugin_api::stream::{mix, PlanCell, Rgba, SecondFrom, Side, StreamInput, StreamKernel, StreamLayout, StreamMemo};
use kt_time::{FpsGrid, Tick};

use super::{clip_base_quality, clip_frame_keys, video_cache_idx, VideoClip};

/// A stop check: who set it and the check.
type Stop = Option<(u32, Box<dyn Fn() -> bool>)>;

thread_local! {
    static STOP: std::cell::RefCell<Stop> = const { std::cell::RefCell::new(None) };
}

/// Run `body` with a stop check (gpu-arbiter, row 147): a stream stage this thread renders inside it
/// stops between history positions once `check` says nobody wants the frame any more (its kernel
/// keeps what it computed in the memo). `who` — a [`kt_time::trace::stop`] value for the trace. The
/// compose thread, the prefetch thread and the reel/poster jobs set one; an export never does.
pub fn stop_when<R>(who: u32, check: impl Fn() -> bool + 'static, body: impl FnOnce() -> R) -> R {
    struct Restore(Stop);
    impl Drop for Restore {
        fn drop(&mut self) {
            let old = self.0.take();
            STOP.with(|s| *s.borrow_mut() = old);
        }
    }
    let _restore = Restore(STOP.with(|s| s.borrow_mut().replace((who, Box::new(check)))));
    body()
}

/// This thread's stop check says stop (a pre-roll gives up between batches).
pub fn stopping() -> bool {
    stop_now().is_some()
}

/// Whether this thread's stop check says stop; `Some(who)` then.
fn stop_now() -> Option<u32> {
    STOP.with(|s| s.borrow().as_ref().and_then(|(who, check)| check().then_some(*who)))
}

/// The preview's ONE memo (design D3): the composer, the prefetch and the picker share it, so
/// "what is still needed" and "what was computed" see the same entries. Each export owns its own.
/// Its bound is `STREAM_MEMO_BASE` plus what the memory ledger grants the stream history
/// (memory-ledger); the player asks by the loop (`stream_memo_bytes`), and a grant taken back (the
/// kept composites asked first, an export claimed the room) drops the entries past it at once.
pub fn stream_memo() -> &'static Arc<StreamMemo> {
    static M: OnceLock<Arc<StreamMemo>> = OnceLock::new();
    M.get_or_init(|| {
        let m = Arc::new(StreamMemo::new(STREAM_MEMO_BASE));
        let weak = Arc::downgrade(&m);
        m.account().on_shrink(move |grant| {
            if let Some(m) = weak.upgrade() {
                m.set_cap(STREAM_MEMO_BASE.saturating_add(usize::try_from(grant).unwrap_or(usize::MAX)));
            }
        });
        m
    })
}

/// The preview memo's bound with no loop to keep: a stretch of playback and its look-ahead.
pub const STREAM_MEMO_BASE: usize = 256 << 20;

/// The preview memo's want for a loop of `loop_positions` stream positions keeping `per_position`
/// bytes each (stream-layout-per-build, found-defects 143). A loop whose history the memo cannot hold
/// recomputes it every lap: at 1080 lines 256 MB held about 20 positions. So the want is what the
/// loop keeps — every position's picture plus an eighth — on top of `base` (the working set of the
/// stage: the encoder's transient state, the look-ahead), and never above a quarter of the memory the
/// machine has `available` when it is sized (measured at run time; unknown — `base`), nor below `base`.
/// The memory ledger then grants it after the kept composites (`size_stream_memo`).
pub fn stream_memo_bytes(base: usize, per_position: usize, loop_positions: Option<usize>, available: Option<u64>) -> usize {
    stream_memo_bytes_walk(base, per_position, loop_positions, 0, available)
}

/// `stream_memo_bytes` with the walk's checkpoint tier (stream-history-on-device, found-defects 394):
/// a walk from the chain's anchor to the loop's end keeps `checkpoints` checkpoints, two pictures each,
/// outside the LRU; without them a re-walk after an eviction or a jump starts at the anchor again.
pub fn stream_memo_bytes_walk(base: usize, per_position: usize, loop_positions: Option<usize>, checkpoints: usize, available: Option<u64>) -> usize {
    let (Some(n), Some(free)) = (loop_positions, available) else { return base };
    let want = per_position.saturating_mul(n + n / 8 + 2 + 2 * checkpoints).saturating_add(base);
    want.min(usize::try_from(free / 4).unwrap_or(usize::MAX)).max(base)
}

/// Size the preview's memo for a loop of `loop_positions` at `per_position` bytes (`None` — no loop):
/// its want by the memory available now, granted by the memory ledger after the kept composites';
/// the bound set, with a `stream_memo_cap` record.
pub fn size_stream_memo(per_position: usize, loop_positions: Option<usize>) -> usize {
    size_stream_memo_walk(per_position, loop_positions, 0)
}

/// `size_stream_memo` with the walk's checkpoints (`stream_memo_bytes_walk`).
pub fn size_stream_memo_walk(per_position: usize, loop_positions: Option<usize>, checkpoints: usize) -> usize {
    let free = memory_available();
    let want = stream_memo_bytes_walk(STREAM_MEMO_BASE, per_position, loop_positions, checkpoints, free);
    let memo = stream_memo();
    let grant = memo.account().want((want - STREAM_MEMO_BASE) as u64);
    let cap = STREAM_MEMO_BASE.saturating_add(usize::try_from(grant).unwrap_or(usize::MAX));
    memo.set_cap(cap);
    kt_time::trace::ev(kt_time::trace::Ev::StreamMemoCap)
        .a(cap as u64)
        .b(loop_positions.unwrap_or(0) as u64)
        .c(free.map_or(0, |f| f >> 20))
        .emit();
    cap
}

/// How the engine learns the memory the process can still take (the host injects it: the probe
/// lives with the device code, `kt_gpu::memory::available`); `None` — unknown. It is the memory
/// ledger's probe (`kt_time::mem::set_probe`).
pub fn set_memory_probe(probe: fn() -> Option<u64>) {
    kt_time::mem::set_probe(probe);
}

/// The memory available now, by the injected probe.
pub fn memory_available() -> Option<u64> {
    kt_time::mem::available()
}

/// How a live preview paces a stream stage (decode-request-storm). The history of a frame can be
/// far larger than the cache — a cut mosh at 1080 lines asks up to 600 frames of 8 MB against a
/// 512 MB budget — and ordering it whole let the decoder run through it front to back while the
/// stage read it only when a composite came: the cache evicted the front, the very frames the
/// stage needed next, and the frame never came (the owner's journal, kkt-26.11.08: 1570 requests
/// and 15–100 frames decoded a second, every one dropped unread, V-fps 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamPacing {
    /// The order names the earliest positions the history lacks, as many as fit these bytes (at
    /// least one): what the stage will read next, never more than the cache keeps until then.
    pub order_bytes: usize,
    /// A composite runs the stage only when its frame lacks at most this many positions (a step
    /// of playback, a skipped frame or two); a longer history is advanced by the prefetch thread
    /// (`VideoRenderer::stream_advance`) and the composite reports the frame incomplete — the UI
    /// thread never computes a history.
    pub compose_steps: usize,
    /// While playing, a frame further short shows the latest the stage holds on its stream (a late
    /// frame of the layer, as a late decode shows its last one) instead of none: when the stage is
    /// slower than real time the playhead never meets its frontier, and the picture stood. Off on a
    /// pause, where the frame itself is waited for. The player sets it per composite.
    pub hold: bool,
}

impl StreamPacing {
    /// The preview's pacing for a cache of `max_video_bytes`: a quarter of it for the history (the
    /// look-ahead takes half, the span being decoded the rest).
    pub fn preview(max_video_bytes: usize) -> StreamPacing {
        StreamPacing { order_bytes: max_video_bytes / 4, compose_steps: 2, hold: false }
    }
}

/// The compiled schedules of one stage, per stream size.
type PlanCells = Vec<(Quality, Arc<PlanCell>)>;

/// A clip's stream stage: the slot's plugin and rows, the event before on the same lane and the
/// events of the second input's lane (clones made when the renderer is built — the renderer is
/// rebuilt on every edit, so they cannot go stale).
#[derive(Clone)]
pub(super) struct StreamStage {
    pub plugin: &'static Arc<dyn kt_plugin_api::Plugin>,
    /// The slot's index in the clip's chain (its curves are applied there, `rows_at`).
    pub slot: usize,
    pub values: Vec<f64>,
    pub before: Option<Box<VideoClip>>,
    pub second: Vec<VideoClip>,
    /// The kernel's compiled schedule per stream size (stream-layout-per-build): made fresh by
    /// `link_stages` on every build, so a neighbour's edit (the event before, the second input) can
    /// never leave a stale one; shared by the composer, the prefetch and the picker of the build.
    pub plans: Arc<std::sync::Mutex<PlanCells>>,
}

impl std::fmt::Debug for StreamStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamStage")
            .field("id", &self.plugin.manifest().id)
            .field("values", &self.values)
            .field("before", &self.before.as_ref().map(|c| c.id))
            .field("second", &self.second.len())
            .finish()
    }
}

impl StreamStage {
    /// The stage of a chain: the first active `Stream` slot whose plugin is registered.
    pub fn of(fx: &[kt_model::FxInstance]) -> Option<StreamStage> {
        fx.iter().enumerate().find_map(|(slot, f)| match f {
            kt_model::FxInstance::Plugin(p) if f.is_stream() => p
                .plugin()
                .map(|plugin| StreamStage {
                    plugin,
                    slot,
                    values: p.values.clone(),
                    before: None,
                    second: vec![],
                    plans: Default::default(),
                }),
            _ => None,
        })
    }

    pub fn kernel(&self) -> &'static dyn StreamKernel {
        self.plugin.stream().expect("a stream slot's plugin has a stream kernel (registry, t1240)")
    }

    /// The cell of the schedule compiled at stream size `q`.
    fn plan_cell(&self, q: Quality) -> Arc<PlanCell> {
        let mut g = self.plans.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, c)) = g.iter().find(|(s, _)| *s == q) {
            return c.clone();
        }
        let c = Arc::new(PlanCell::default());
        g.push((q, c.clone()));
        c
    }
}

/// Fill each stage's event before and second input (after the composition sort).
pub(super) fn link_stages(clips: &mut [VideoClip]) {
    let snapshot: Vec<VideoClip> = clips.to_vec();
    let lite = |c: &VideoClip| {
        let mut c = c.clone();
        c.stream = None;
        c
    };
    for c in clips.iter_mut() {
        let Some(st) = c.stream.as_mut() else { continue };
        // this build's own schedules: the stage may be the last build's, copied (edit-without-rebuild)
        st.plans = Default::default();
        let k = st.kernel();
        if k.before(&st.values) > 0 {
            st.before = snapshot
                .iter()
                .find(|o| o.lane == c.lane && o.end == c.start && o.nest.is_none() && o.id != c.id)
                .map(|o| Box::new(lite(o)));
        }
        let lane = match k.second(&st.values) {
            SecondFrom::None => None,
            SecondFrom::TrackAbove => c.lane.checked_sub(1),
            SecondFrom::TrackBelow => Some(c.lane + 1),
        };
        if let Some(l) = lane {
            st.second = snapshot.iter().filter(|o| o.lane == l && o.nest.is_none()).map(lite).collect();
        }
    }
}

/// The size the stream is ordered and read at: `lines` high, the aspect of the layer's frame.
pub(super) fn stream_quality(c: &VideoClip, st: &StreamStage, quality: Quality) -> Quality {
    let base = clip_base_quality(c, quality);
    let h = st.kernel().lines(&st.values).max(16);
    let w = ((h as u64 * base.width.max(2) as u64 + base.height.max(2) as u64 / 2) / base.height.max(2) as u64) as u32;
    Quality { width: (w.max(16) + 1) & !1, height: h }
}

fn ceil_div(a: i64, b: i64) -> i64 {
    -((-a).div_euclid(b))
}

/// One clip's stream as the kernel sees it (positions = project grid frame indices).
pub(super) struct ClipStream<'a> {
    clip: &'a VideoClip,
    st: &'a StreamStage,
    grid: &'a FpsGrid,
    pub q: Quality,
    step: i64,
    first: i64,
    cut: i64,
    end: i64,
    cache: Option<&'a BurstCache>,
    plan: Arc<PlanCell>,
    /// The kernel asked `stop` and was told to: who stopped it, and before which position.
    stopped: std::cell::Cell<Option<u32>>,
    /// The tick the host shows from (the memo's view, stream-history-on-device D4).
    view: Option<i64>,
}

impl<'a> ClipStream<'a> {
    pub fn new(clip: &'a VideoClip, st: &'a StreamStage, grid: &'a FpsGrid, quality: Quality, cache: Option<&'a BurstCache>) -> Self {
        let step = grid.frame_step().max(1);
        let cut = ceil_div(clip.start.0, step);
        let end = ceil_div(clip.end.0, step);
        // the stream starts in the event before (its last `before` frames), or at the first frame
        // of the event's own handles when it has none
        let first = match &st.before {
            Some(b) => cut - (st.kernel().before(&st.values) as i64).min(cut - ceil_div(b.start.0, step)).max(0),
            None => ceil_div(clip.start.0 - clip.head_ext.0, step).min(cut),
        };
        let q = stream_quality(clip, st, quality);
        ClipStream { clip, st, grid, q, step, first, cut, end, cache, plan: st.plan_cell(q), stopped: Default::default(), view: None }
    }

    /// The host's view, as the memo holds it (a tick): a walk keeps the pictures from there on.
    pub fn with_view(mut self, memo: &StreamMemo) -> Self {
        self.view = memo.view();
        self
    }

    /// The position of a frame time.
    pub fn pos(&self, t: Tick) -> i64 {
        t.0.div_euclid(self.step)
    }

    /// The clip and source pts a position reads on a side.
    pub fn at(&self, side: Side, k: i64) -> Option<(&'a VideoClip, Tick)> {
        let t = Tick(k * self.step);
        let c: &'a VideoClip = match side {
            Side::Main if k < self.cut => self.st.before.as_deref().unwrap_or(self.clip),
            Side::Main => self.clip,
            Side::Second => self.st.second.iter().find(|o| o.start <= t && t < o.end)?,
        };
        Some((c, clip_frame_keys(c, t, self.grid, false).0))
    }

    /// The earliest of `needs` (ascending) that fit `bytes` at the stream's size, at least one.
    pub fn window<'n>(&self, needs: &'n [(Side, i64)], bytes: usize) -> &'n [(Side, i64)] {
        let frame = (self.q.width as usize * self.q.height as usize * 4).max(1);
        &needs[..needs.len().min((bytes / frame).max(1))]
    }

    /// The order of positions: (media, pts, the stream's size).
    pub fn order(&self, needs: &[(Side, i64)], out: &mut Vec<(MediaId, Tick, Quality)>) {
        for &(side, k) in needs {
            if let Some((c, pts)) = self.at(side, k) {
                out.push((c.media, pts, self.q));
            }
        }
    }
}

impl StreamLayout for ClipStream<'_> {
    fn size(&self) -> (u32, u32) {
        (self.q.width, self.q.height)
    }
    fn first(&self) -> i64 {
        self.first
    }
    fn cut(&self) -> i64 {
        self.cut
    }
    fn end(&self) -> i64 {
        self.end
    }
    fn second_start(&self, k: i64) -> Option<i64> {
        let t = Tick(k * self.step);
        let o = self.st.second.iter().find(|o| o.start <= t && t < o.end)?;
        Some(ceil_div(o.start.0, self.step))
    }
    /// The slot's rows at `k` with its curves (the owner, 2026-11-01: "this effect has no curves"):
    /// the curves are sampled at the material position of the frame, as every curve of the chain
    /// is (`layer_src`); a position before the cut reads the event's start.
    fn rows_at(&self, k: i64) -> Option<Vec<f64>> {
        let plan = self.clip.auto_plan.as_ref()?;
        let t = Tick((k * self.step).max(self.clip.start.0));
        let mat = kt_model::clip_material_at(
            self.clip.source_in,
            self.clip.start,
            self.clip.end - self.clip.start,
            self.clip.reversed,
            self.step,
            t,
        );
        // the stage's own slot alone (compiled-curves): a chain clone per position read one slot
        let mut slot = self.clip.fx.get(self.st.slot)?.clone();
        plan.apply_slot(self.st.slot, &mut slot, mat);
        match slot {
            kt_model::FxInstance::Plugin(p) => Some(p.values),
            _ => None,
        }
    }
    fn plan_cell(&self) -> Option<&PlanCell> {
        Some(&self.plan)
    }
    fn shown_from(&self) -> Option<i64> {
        self.view.map(|t| self.pos(Tick(t)))
    }
    fn ident(&self, side: Side, k: i64) -> u64 {
        let Some((c, pts)) = self.at(side, k) else { return 0 };
        let mut h = mix(0x5754_7245_414d, c.media.0);
        h = mix(h, pts.0 as u64);
        h = mix(h, ((self.q.width as u64) << 32) | self.q.height as u64);
        mix(h, c.src_rev).max(1)
    }
}

impl ClipStream<'_> {
    /// The kernel was stopped (and by whom): its `None` is no missing frame.
    pub fn stopped(&self) -> Option<u32> {
        self.stopped.get()
    }

    /// Record a stop the kernel obeyed: the stage gave up position `n` of clip `clip`.
    pub fn trace_stop(&self, n: i64) {
        if let Some(who) = self.stopped.get() {
            kt_time::trace::ev(kt_time::trace::Ev::StreamStop).id(self.clip.id.0).a(n as u64).aux(who).emit();
        }
    }
}

impl StreamInput for ClipStream<'_> {
    fn stop(&self) -> bool {
        match stop_now() {
            Some(who) => {
                self.stopped.set(Some(who));
                true
            }
            None => false,
        }
    }

    fn with_frame(&self, side: Side, k: i64, f: &mut dyn FnMut(Rgba<'_>)) -> bool {
        let Some(cache) = self.cache else { return false };
        let Some((c, pts)) = self.at(side, k) else { return false };
        match cache.get_video(SourceId(c.media.0), Some(self.q), video_cache_idx(pts)) {
            Some(fr) => {
                f(Rgba { w: fr.width, h: fr.height, rgba: &fr.rgba });
                true
            }
            None => false,
        }
    }
}

/// The stage's key for frame time `t` without running it (a probe plan, structured-telemetry): an
/// empty frame of the layer's size and the key the kernel names; `None` — the kernel cannot say.
pub(super) fn key_only(
    clip: &VideoClip,
    st: &StreamStage,
    grid: &FpsGrid,
    quality: Quality,
    dq: Quality,
    t: Tick,
    pts: Tick,
) -> Option<(Option<Arc<VideoFrame>>, u64)> {
    let cs = ClipStream::new(clip, st, grid, quality, None);
    let key = st.kernel().key(&st.values, &cs, cs.pos(t))?;
    let frame = VideoFrame { pts, width: dq.width, height: dq.height, rgba: Box::new([]), has_alpha: false };
    Some((Some(Arc::new(frame)), key))
}

/// Run the stage for frame time `t`: the decoded picture fitted to `dq` (opaque) and its key;
/// `(None, 0)` — a frame of the history is missing (starvation; the order asks for it).
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    clip: &VideoClip,
    st: &StreamStage,
    grid: &FpsGrid,
    cache: &BurstCache,
    memo: &StreamMemo,
    pacing: Option<StreamPacing>,
    held: &std::sync::atomic::AtomicBool,
    quality: Quality,
    dq: Quality,
    t: Tick,
    pts: Tick,
) -> (Option<Arc<VideoFrame>>, u64) {
    use kt_time::trace::{self, stream as so, Ev};
    let cs = ClipStream::new(clip, st, grid, quality, Some(cache)).with_view(memo);
    let k = st.kernel();
    let mut n = cs.pos(t);
    // the stage's run in the trace: the position, what the memo still lacked, and what came out
    let mut span = trace::span(Ev::StreamRun);
    span.id(clip.id.0).a(n as u64).c(n as u64);
    let needs = if pacing.is_some() || trace::on() { k.needs(&st.values, &cs, n, memo).len() } else { 0 };
    span.b(needs as u64);
    if let Some(p) = pacing
        && needs > p.compose_steps
    {
        // the history is the prefetch thread's (`stream_advance`); playing, the layer shows the
        // latest frame the stage holds, a frame it can give without computing anything
        match k.held(&st.values, &cs, n, memo).filter(|&f| p.hold && k.needs(&st.values, &cs, f, memo).is_empty()) {
            Some(f) => {
                n = f;
                held.store(true, std::sync::atomic::Ordering::Relaxed);
                span.aux(so::HELD).c(f as u64);
            }
            None => {
                span.aux(so::SHORT);
                return (None, 0);
            }
        }
    } else {
        span.aux(so::RENDERED);
    }
    let mut buf = Vec::new();
    let Some(out) = k.render(&st.values, &cs, n, memo, &mut buf) else {
        span.aux(if cs.stopped().is_some() { so::STOPPED } else { so::SOURCE_MISSING });
        cs.trace_stop(n);
        return (None, 0);
    };
    drop(span);
    let src = kt_fx::video::Frame { w: out.w, h: out.h, rgba: buf, has_alpha: false };
    let mut rgba = vec![0u8; (dq.width * dq.height * 4) as usize];
    kt_fx::video::fit_into(&src, dq.width, dq.height, &mut rgba);
    let frame = VideoFrame { pts, width: dq.width, height: dq.height, rgba: rgba.into_boxed_slice(), has_alpha: false };
    (Some(Arc::new(frame)), out.key)
}
