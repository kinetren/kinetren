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
    // the room, not the bare available: the memo's own growth is memory it may keep (class-walk-revision)
    let free = kt_time::mem::room();
    let want = stream_memo_bytes_walk(STREAM_MEMO_BASE, per_position, loop_positions, checkpoints, free);
    size_to(want, loop_positions.unwrap_or(0), free)
}

/// What the stream stages over a loop keep (`VideoRenderer::stream_want`, stream-memo-budget): each
/// stage's positions in the region at its own picture size, reels included.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamWant {
    /// The positions the stages show in the region, summed over the stages.
    pub positions: usize,
    /// Their pictures: each stage's positions × its picture (half its RGBA frame).
    pub lap_bytes: usize,
    /// The largest picture of a stage.
    pub per_position: usize,
    /// The checkpoint tiers of the stages' walks to their last position in the region, two pictures each.
    pub checkpoint_bytes: usize,
}

/// The preview memo's want for a loop the stages keep `want` of (stream-memo-budget, found-defects
/// 518): the loop's pictures plus an eighth, the two the next frame reads and the checkpoint tiers, on
/// top of `base`, within a quarter of the memory `available`, never below `base`. `None` — no loop.
pub fn stream_memo_want(base: usize, want: Option<&StreamWant>, available: Option<u64>) -> usize {
    let (Some(w), Some(free)) = (want, available) else { return base };
    let bytes = w.lap_bytes + w.lap_bytes / 8 + 2 * w.per_position + w.checkpoint_bytes;
    bytes.saturating_add(base).min(usize::try_from(free / 4).unwrap_or(usize::MAX)).max(base)
}

/// Size the preview's memo for the loop the stages keep `want` of (`None` — no loop), as
/// `size_stream_memo` does.
pub fn size_stream_memo_want(want: Option<&StreamWant>) -> usize {
    let free = kt_time::mem::room();
    size_to(stream_memo_want(STREAM_MEMO_BASE, want, free), want.map_or(0, |w| w.positions), free)
}

/// Ask the ledger for `want`, set the preview memo's bound to what it grants, and record it.
fn size_to(want: usize, loop_positions: usize, free: Option<u64>) -> usize {
    let memo = stream_memo();
    let grant = memo.account().want((want - STREAM_MEMO_BASE) as u64);
    let cap = STREAM_MEMO_BASE.saturating_add(usize::try_from(grant).unwrap_or(usize::MAX));
    memo.set_cap(cap);
    kt_time::trace::ev(kt_time::trace::Ev::StreamMemoCap)
        .a(cap as u64)
        .b(loop_positions as u64)
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
    /// (`VideoRenderer::stream_advance`) and the composite reports the frame incomplete, playing or
    /// paused — the UI thread never computes a history, and no picture of another position stands
    /// in for the frame (catch-up-shows-loading: the stages after the stream ran over it).
    pub compose_steps: usize,
}

impl StreamPacing {
    /// The preview's pacing for a cache of `max_video_bytes`: a quarter of it for the history (the
    /// look-ahead takes half, the span being decoded the rest).
    pub fn preview(max_video_bytes: usize) -> StreamPacing {
        StreamPacing { order_bytes: max_video_bytes / 4, compose_steps: 2 }
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
    /// The slots IN FRONT of the stage that act on the picture (stream-input-order, row 484): run on
    /// every frame of the main stream before the kernel reads it, in the order the person built.
    /// Indices into the clip's materialized chain; the leading Geometry node is not one (it places
    /// the event's picture on the canvas — it stays after the stage, with `lead`).
    pub front: Vec<usize>,
    /// What determines the front's output besides the frame: its slots and their curves hashed;
    /// an edit of a slot in front changes the stream's addresses, so the history is computed again
    /// (an edit of a slot after the stage does not touch it).
    pub front_key: u64,
    /// A slot in front has curves: its output depends on the position too.
    pub front_curved: bool,
    /// The picture slots each `second` event runs before the kernel reads it (chain-everywhere,
    /// found-defects 540): that event's chain in front of its own stream slot, if any, without its
    /// leading Geometry node — the other track's picture as its chain made it, never as decoded.
    pub second_front: Vec<Vec<usize>>,
    /// The cameras of the composite the stage stands in and the project's size: a Pan3d or a
    /// camera-driven slot among the inputs' slots resolves its camera as the layer does (row 541).
    pub cams: Arc<[kt_model::CameraEvent]>,
    pub project_size: (u32, u32),
    /// Per `second` event: the cameras' hash when its input slots read a camera, else 0.
    pub second_cam: Vec<u64>,
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
    /// The stage of a chain: the first active `Stream` slot whose plugin is registered; `auto` —
    /// the chain's raw curves (they name the front's key).
    pub fn of(fx: &[kt_model::FxInstance], auto: &[Vec<kt_model::ParamCurve>]) -> Option<StreamStage> {
        let mut st = fx.iter().enumerate().find_map(|(slot, f)| match f {
            kt_model::FxInstance::Plugin(p) if f.is_stream() => p.plugin().map(|plugin| StreamStage {
                plugin,
                slot,
                values: p.values.clone(),
                before: None,
                second: vec![],
                front: vec![],
                front_key: 0,
                front_curved: false,
                second_front: vec![],
                second_cam: vec![],
                cams: Arc::from(Vec::new()),
                project_size: (1, 1),
                plans: Default::default(),
            }),
            _ => None,
        })?;
        st.front = front_slots(&fx[..st.slot]);
        if !st.front.is_empty() {
            let curves: Vec<_> = st.front.iter().map(|&i| auto.get(i).map(|c| &c[..]).unwrap_or(&[])).collect();
            let slots: Vec<_> = st.front.iter().map(|&i| &fx[i]).collect();
            st.front_curved = curves.iter().any(|c| !c.is_empty());
            st.front_key = xxhash_rust::xxh3::xxh3_64(format!("{slots:?}|{curves:?}").as_bytes()).max(1);
        }
        Some(st)
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

/// The slots of `head` (the chain before a stream slot) that change the picture: not audio, not a
/// layer strength, not an identity geometry, not the leading Geometry node (stream-input-order).
pub fn front_slots(head: &[kt_model::FxInstance]) -> Vec<usize> {
    (0..head.len())
        .filter(|&i| !head[i].is_lead_geometry() && kt_fx::video::has_video_fx(&head[i..=i]))
        .collect()
}

/// The chain the layer runs AFTER its stream stage: the slots in front of it ran on the stream's
/// input (`front`), so here each is the identity geometry (transparent to the planner; the
/// indices masks and layer strengths name stay as they are).
pub(super) fn after_chain(fx: &mut [kt_model::FxInstance], front: &[usize]) {
    for &i in front {
        if let Some(f) = fx.get_mut(i) {
            *f = identity_slot();
        }
    }
}

fn identity_slot() -> kt_model::FxInstance {
    kt_model::FxInstance::Geometry {
        lead: false,
        crop_l: 0.0,
        crop_t: 0.0,
        crop_r: 0.0,
        crop_b: 0.0,
        pos_x: 0.0,
        pos_y: 0.0,
        zoom: 1.0,
        rot: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
    }
}

/// How the engine opens the executor of the slots in front of a stream stage (the host injects it,
/// as `kt_plugin_api::compute::install`: kt-engine knows only the trait). `None` — no device.
pub type OpenFrontExec = fn() -> Option<Arc<dyn crate::video_plan::VideoGpuExec>>;

static FRONT_OPEN: OnceLock<OpenFrontExec> = OnceLock::new();
static FRONT_EXEC: OnceLock<Option<Arc<dyn crate::video_plan::VideoGpuExec>>> = OnceLock::new();

/// Name how the process opens the front's executor, once; it opens at the first stream frame that
/// has a slot in front (a project with none never opens it).
pub fn install_stream_front(open: OpenFrontExec) {
    let _ = FRONT_OPEN.set(open);
}

/// Compile what the project reaches on the front's device, on a thread of its own (opening the
/// device takes up to seconds): the first walk of a history then compiles nothing (pipeline-prewarm).
pub fn prewarm_stream_front(ids: Vec<String>, runs: Vec<Vec<String>>) {
    if FRONT_OPEN.get().is_none() {
        return;
    }
    let _ = std::thread::Builder::new().name("front-prewarm".into()).spawn(move || {
        if let Some(e) = front_exec() {
            e.prewarm(&ids);
            e.prewarm_runs(&runs);
        }
    });
}

fn front_exec() -> Option<&'static Arc<dyn crate::video_plan::VideoGpuExec>> {
    let open = FRONT_OPEN.get()?;
    FRONT_EXEC.get_or_init(open).as_ref()
}

/// The front's outputs by stream address (`ClipStream::ident`, which names the front): a re-walk
/// after the codec memo dropped a position, or the prefetch and the composer reading the same
/// position, runs the slots once. A [`StreamMemo`] (loop-keeps-its-lap, found-defects 542): its bytes
/// are in the memory ledger as stream history, and told of the preview's loop with the stream memo
/// (`VideoRenderer::stream_keep_loop`) it keeps a part of a loop it cannot hold instead of an LRU's
/// nothing. Bounded; the bytes do not depend on it (invariant 11).
pub fn front_memo() -> &'static StreamMemo {
    static M: OnceLock<StreamMemo> = OnceLock::new();
    M.get_or_init(|| StreamMemo::new(FRONT_MEMO_BYTES))
}

/// What the slots in front made at a position, as the front memo keeps it.
struct FrontOut(u32, u32, Arc<[u8]>);

const FRONT_MEMO_BYTES: usize = 64 << 20;

/// How many times the slots in front ran (tests and `kt-cli stream-cost`).
pub fn front_runs() -> u64 {
    FRONT_RUNS.load(std::sync::atomic::Ordering::Relaxed)
}

static FRONT_RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn front_memo_get(id: u64) -> Option<(u32, u32, Arc<[u8]>)> {
    front_memo().get::<FrontOut>(id).map(|o| (o.0, o.1, o.2.clone()))
}

fn front_memo_put(id: u64, w: u32, h: u32, rgba: Arc<[u8]>) {
    if !front_memo().contains(id) {
        let bytes = rgba.len();
        front_memo().put(id, Arc::new(FrontOut(w, h, rgba)), bytes);
    }
}

/// The slots of an event's chain that make its picture as a stream's input reads it: the slots in
/// front of its own stream slot (the whole chain without one), not the leading Geometry node.
fn input_slots(fx: &[kt_model::FxInstance]) -> Vec<usize> {
    let end = fx.iter().position(|f| f.is_stream()).unwrap_or(fx.len());
    front_slots(&fx[..end])
}

/// Fill each stage's event before and second input (after the composition sort); `cams` and
/// `project_size` are the composite's, as `layer_src` resolves a camera with them.
pub(super) fn link_stages(clips: &mut [VideoClip], cams: &[kt_model::CameraEvent], project_size: (u32, u32)) {
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
        // the masks of the slots in front cut them in the front run (found-defects 541): an edit of
        // one changes what the stream reads, and an animated one changes it per position
        if let Some(m) = &c.masks {
            let front_masks: Vec<_> = m.slots.iter().filter(|(from, ..)| st.front.contains(from)).collect();
            if !front_masks.is_empty() {
                st.front_key = mix(st.front_key, xxhash_rust::xxh3::xxh3_64(format!("{front_masks:?}").as_bytes()));
                st.front_curved = true;
            }
        }
        // curves in front are sampled by the event's placement: it joins the front's key
        if st.front_curved {
            let place = format!("{}|{}|{}|{}", c.start.0, c.end.0, c.source_in.0, c.reversed);
            st.front_key = mix(st.front_key, xxhash_rust::xxh3::xxh3_64(place.as_bytes()));
        }
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
        st.second_front = st.second.iter().map(|o| input_slots(&o.fx)).collect();
        st.cams = Arc::from(cams.to_vec());
        st.project_size = project_size;
        // a camera-driven slot among the inputs' reads the cameras at each position (row 541)
        let cams_key = xxhash_rust::xxh3::xxh3_64(format!("{cams:?}").as_bytes());
        if st.front.iter().any(|&i| c.fx[i].uses_camera()) {
            st.front_key = mix(st.front_key, cams_key);
            st.front_curved = true;
        }
        st.second_cam = st
            .second
            .iter()
            .zip(&st.second_front)
            .map(|(o, sl)| if sl.iter().any(|&i| o.fx[i].uses_camera()) { cams_key } else { 0 })
            .collect();
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
        h = mix(h, c.src_rev);
        // the slots in front feed the main stream: they are part of what the kernel reads
        if side == Side::Main && !self.st.front.is_empty() {
            h = mix(h, self.st.front_key);
            if self.st.front_curved {
                h = mix(h, k as u64);
            }
        }
        // the second input is read through its event's chain (found-defects 540): its chain, curves
        // and placement (`fx_hash`) name what the kernel reads, and the position when curved
        if side == Side::Second
            && let Some(i) = self.second_index(k)
            && !self.st.second_front[i].is_empty()
        {
            h = mix(h, c.fx_hash);
            if self.st.second_cam[i] != 0 {
                h = mix(h, self.st.second_cam[i]);
            }
            if c.auto_plan.is_some() || c.masks.is_some() || self.st.second_cam[i] != 0 {
                h = mix(h, k as u64);
            }
        }
        h.max(1)
    }
}

impl ClipStream<'_> {
    /// The index in `second` of the event the second input reads at position `k`.
    fn second_index(&self, k: i64) -> Option<usize> {
        let t = Tick(k * self.step);
        self.st.second.iter().position(|o| o.start <= t && t < o.end)
    }

    /// Run `slots` of event `clip`'s chain over `fr`, the frame read at position `k`, as its layer runs
    /// them: their curves at the position's material time (clamped into the event), its `u`, the
    /// masks those slots carry and the camera they resolve (chain-everywhere, found-defects 541), by
    /// the injected executor at the stream's size. The output is kept by `id` (the stream address,
    /// which names the slots). `None` — no device, or the executor refused the plan: the frame is
    /// missing, never the decoded picture standing in for it.
    fn run_slots(&self, clip: &VideoClip, slots: &[usize], id: u64, k: i64, fr: Arc<VideoFrame>) -> Option<(u32, u32, Arc<[u8]>)> {
        if let Some(hit) = front_memo_get(id) {
            return Some(hit);
        }
        let exec = front_exec()?;
        let t = Tick((k * self.step).clamp(clip.start.0, (clip.end.0 - 1).max(clip.start.0)));
        let mat = kt_model::clip_material_at(clip.source_in, clip.start, clip.end - clip.start, clip.reversed, self.step, t);
        let fx: Vec<kt_model::FxInstance> = slots
            .iter()
            .filter_map(|&i| {
                let mut s = clip.fx.get(i)?.clone();
                if let Some(plan) = &clip.auto_plan {
                    plan.apply_slot(i, &mut s, mat);
                }
                Some(s)
            })
            .collect();
        let u = (t.0 - clip.start.0) as f64 / (clip.end.0 - clip.start.0).max(1) as f64;
        let (w, h) = (fr.width, fr.height);
        let (pw, ph) = self.st.project_size;
        let cam3d = super::resolve_cam3d(&fx, &self.st.cams, t, w, pw);
        let masks = clip.masks.as_ref().map(|m| m.ops_for(slots, mat, ph.max(1) as f64)).unwrap_or_default();
        let plan = crate::video_plan::chain_plan_with(fr, &fx, id, u.clamp(0.0, 1.0), t, cam3d, masks);
        let mut out = Vec::new();
        FRONT_RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        match exec.execute_once(&plan, &mut out) {
            Ok(true) if out.len() == (w * h * 4) as usize => {
                let rgba: Arc<[u8]> = out.into();
                front_memo_put(id, w, h, rgba.clone());
                Some((w, h, rgba))
            }
            _ => None,
        }
    }

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
        let Some(fr) = cache.get_video(SourceId(c.media.0), Some(self.q), video_cache_idx(pts)) else { return false };
        // the main stream through the slots in front of the stage (stream-input-order), the event
        // before's frames included (its D2); the second input through its own event's chain (row 540)
        let slots: &[usize] = match side {
            Side::Main => &self.st.front,
            Side::Second => self.second_index(k).map(|i| &self.st.second_front[i][..]).unwrap_or(&[]),
        };
        if !slots.is_empty() {
            let owner = if side == Side::Main { self.clip } else { c };
            let Some((w, h, rgba)) = self.run_slots(owner, slots, self.ident(side, k), k, fr) else { return false };
            f(Rgba { w, h, rgba: &rgba });
            return true;
        }
        f(Rgba { w: fr.width, h: fr.height, rgba: &fr.rgba });
        true
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
/// `(None, 0)` — a frame of the history is missing (starvation; the order asks for it), or, paced, the
/// history lacks more positions than a composite computes: `short` then keeps the most positions a
/// stage of the plan still lacks (the preview writes them under Loading).
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    clip: &VideoClip,
    st: &StreamStage,
    grid: &FpsGrid,
    cache: &BurstCache,
    memo: &StreamMemo,
    view: bool,
    pacing: Option<StreamPacing>,
    quality: Quality,
    dq: Quality,
    t: Tick,
    pts: Tick,
) -> (Option<Arc<VideoFrame>>, u64) {
    use kt_time::trace::{self, stream as so, Ev};
    let cs = ClipStream::new(clip, st, grid, quality, Some(cache));
    let cs = if view { cs.with_view(memo) } else { cs };
    let k = st.kernel();
    let n = cs.pos(t);
    // the stage's run in the trace: the position, what the memo still lacked, and what came out
    let mut span = trace::span(Ev::StreamRun);
    span.id(clip.id.0).a(n as u64).c(n as u64);
    let needs = if pacing.is_some() || trace::on() { k.needs(&st.values, &cs, n, memo).len() } else { 0 };
    span.b(needs as u64);
    if let Some(p) = pacing
        && needs > p.compose_steps
    {
        // the history is the prefetch thread's (`stream_advance`); the frame is incomplete, playing or
        // paused: a picture of another position would have the later stages (the event's VHS Tape, the
        // track's and the reel's stages) run over it at this moment (kkt-26.11.10b K5, found-defects 403)
        crate::video::note_stream_short(needs as u64);
        span.aux(so::SHORT);
        return (None, 0);
    }
    span.aux(so::RENDERED);
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
