//! The final mix of a covered range (cached-range-plays; the owner's choice "a", a change of
//! invariant 2): what the audio render thread rendered — after the effects, the track stages and
//! the nested reels, before the master gain — kept per timeline burst, so that playing the range
//! again hands it out with no decode and no effect run, as a prerender does in VEGAS.
//!
//! Owned by the audio render thread alone, like the FX state it replaces (no lock around it); the
//! bars read what it holds through [`MixCoverage`], which that thread publishes.
//!
//! - **The key is what determines the result**: a burst is held with
//!   [`AudioRenderer::scene_hash`] of its frames — the fingerprints of the events heard there. An
//!   edit drops exactly the bursts whose scene it changed ([`MixStore::retain_valid`]).
//! - **Linear play and the laps of a loop are kept apart** ([`MixCtx`]): a lap after the first
//!   carries the previous lap's ringing at its head, the linear play carries what precedes the
//!   region. Each is replayed only where it was recorded.
//! - **The first recording stays**: a frame already held is not overwritten, so a range keeps one
//!   sound, the one it was first heard with.
//! - **Leaving the store goes on as the continuous play** ([`MixPlay`], kept-mix-settle): before a
//!   chunk the store cannot hand out whole the effects are brought back — through the replayed
//!   frames when they are short, else a reset and a discarded play of their settle length through
//!   the same path (a loop's wrap included) — and a chunk the effects could not warm for is not kept.
//! - Bounded ([`MIX_BUDGET_BYTES`]), least recently heard first; what is evicted leaves the bars.

use crate::audio::{AudioRenderer, RenderStats};
use crate::fxstore::FxStateStore;
use crate::fetch::AudioFetcher;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::{Arc, Mutex};

/// 64 MB: 2.9 minutes of the final mix.
pub const MIX_BUDGET_BYTES: usize = 64 * 1024 * 1024;

const UNIT: i64 = AUDIO_BURST_FRAMES as i64;
const UNIT_SAMPLES: usize = AUDIO_BURST_FRAMES * AUDIO_CHANNELS;

/// Which playing a recording belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MixCtx {
    /// Linear play, and the first lap of a loop (it is linear play from where it began).
    Linear,
    /// The second and later laps of the loop region `[start, end)` in frames.
    Lap(i64, i64),
}

struct Unit {
    /// The frames of the burst it holds, `[lo, hi)` within `0..UNIT`.
    lo: i64,
    hi: i64,
    scene: u64,
    data: Box<[f32]>,
    stamp: u64,
}

/// The kept final mix, per context and timeline burst.
pub struct MixStore {
    units: HashMap<(MixCtx, i64), Unit>,
    /// stamp → key: the least recently heard first.
    order: BTreeMap<u64, (MixCtx, i64)>,
    stamp: u64,
    budget_units: usize,
    /// Something the bars show changed since the last `publish`.
    changed: bool,
    /// Its account in the memory ledger (memory-ledger): the kept mix, its budget as the base.
    account: kt_time::mem::Account,
}

impl Default for MixStore {
    fn default() -> Self {
        MixStore::new(MIX_BUDGET_BYTES)
    }
}

impl MixStore {
    pub fn new(budget_bytes: usize) -> MixStore {
        MixStore {
            units: HashMap::new(),
            order: BTreeMap::new(),
            stamp: 0,
            budget_units: (budget_bytes / (UNIT_SAMPLES * 4)).max(2),
            changed: false,
            account: kt_time::mem::open(kt_time::mem::Tenant::Mix, budget_bytes as u64),
        }
    }

    /// What it holds, to the memory ledger (atomics: the render thread may).
    fn report(&self) {
        self.account.set((self.units.len() * UNIT_SAMPLES * 4) as u64);
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    fn touch(&mut self, key: (MixCtx, i64)) {
        self.stamp += 1;
        if let Some(u) = self.units.get_mut(&key) {
            self.order.remove(&u.stamp);
            u.stamp = self.stamp;
            self.order.insert(self.stamp, key);
        }
    }

    /// Does the store hold the timeline frames `[start, start + n)` of `ctx`, each burst at its
    /// current scene?
    pub fn holds(&self, ctx: MixCtx, start: i64, n: i64, scene: &dyn Fn(i64) -> u64) -> bool {
        let mut f = start;
        while f < start + n {
            let k = f.div_euclid(UNIT);
            let to = ((k + 1) * UNIT).min(start + n);
            let ok = self.units.get(&(ctx, k)).is_some_and(|u| {
                u.scene == scene(k) && u.lo <= f - k * UNIT && u.hi >= to - k * UNIT
            });
            if !ok {
                return false;
            }
            f = to;
        }
        true
    }

    /// Why the store cannot hand the frames out (structured-telemetry): 0 — it can, 1 — a burst was
    /// never kept (or was evicted), 2 — a burst is kept at another scene, 3 — kept only in part.
    pub fn miss_reason(&self, ctx: MixCtx, start: i64, n: i64, scene: &dyn Fn(i64) -> u64) -> u64 {
        let mut f = start;
        while f < start + n {
            let k = f.div_euclid(UNIT);
            let to = ((k + 1) * UNIT).min(start + n);
            match self.units.get(&(ctx, k)) {
                None => return 1,
                Some(u) if u.scene != scene(k) => return 2,
                Some(u) if !(u.lo <= f - k * UNIT && u.hi >= to - k * UNIT) => return 3,
                _ => {}
            }
            f = to;
        }
        0
    }

    /// Copy the frames `[start, start + out/2)` of `ctx` into `out` if every one is held at its
    /// current scene; `false` leaves `out` unspecified.
    pub fn read(&mut self, ctx: MixCtx, start: i64, out: &mut [f32], scene: &dyn Fn(i64) -> u64) -> bool {
        let n = (out.len() / AUDIO_CHANNELS) as i64;
        if !self.holds(ctx, start, n, scene) {
            return false;
        }
        let mut f = start;
        while f < start + n {
            let k = f.div_euclid(UNIT);
            let to = ((k + 1) * UNIT).min(start + n);
            let u = &self.units[&(ctx, k)];
            let (a, b) = ((f - k * UNIT) as usize, (to - k * UNIT) as usize);
            let o = ((f - start) as usize) * AUDIO_CHANNELS;
            out[o..o + (b - a) * AUDIO_CHANNELS]
                .copy_from_slice(&u.data[a * AUDIO_CHANNELS..b * AUDIO_CHANNELS]);
            self.touch((ctx, k));
            f = to;
        }
        true
    }

    /// Keep the frames `[start, start + data/2)` of `ctx`. A frame already held at the same
    /// scene stays as it was; a burst held at another scene is replaced; a piece that would not
    /// join what a burst holds into one run is not kept.
    pub fn write(&mut self, ctx: MixCtx, start: i64, data: &[f32], scene: &dyn Fn(i64) -> u64) {
        let n = (data.len() / AUDIO_CHANNELS) as i64;
        let mut f = start;
        while f < start + n {
            let k = f.div_euclid(UNIT);
            let to = ((k + 1) * UNIT).min(start + n);
            let (a, b) = (f - k * UNIT, to - k * UNIT);
            let src = &data[((f - start) as usize) * AUDIO_CHANNELS..((to - start) as usize) * AUDIO_CHANNELS];
            let sc = scene(k);
            let key = (ctx, k);
            match self.units.get_mut(&key) {
                Some(u) if u.scene == sc => {
                    // joins the run: the new frames outside [lo, hi) are kept
                    if b >= u.lo && a <= u.hi {
                        if a < u.lo {
                            let m = (u.lo - a) as usize * AUDIO_CHANNELS;
                            u.data[a as usize * AUDIO_CHANNELS..u.lo as usize * AUDIO_CHANNELS]
                                .copy_from_slice(&src[..m]);
                            u.lo = a;
                            self.changed = true;
                        }
                        if b > u.hi {
                            let from = (u.hi - a) as usize * AUDIO_CHANNELS;
                            u.data[u.hi as usize * AUDIO_CHANNELS..b as usize * AUDIO_CHANNELS]
                                .copy_from_slice(&src[from..]);
                            u.hi = b;
                            self.changed = true;
                        }
                    }
                }
                _ => {
                    if let Some(old) = self.units.remove(&key) {
                        self.order.remove(&old.stamp);
                    }
                    let mut buf = vec![0f32; UNIT_SAMPLES].into_boxed_slice();
                    buf[a as usize * AUDIO_CHANNELS..b as usize * AUDIO_CHANNELS].copy_from_slice(src);
                    self.stamp += 1;
                    self.units.insert(key, Unit { lo: a, hi: b, scene: sc, data: buf, stamp: self.stamp });
                    self.order.insert(self.stamp, key);
                    self.changed = true;
                    while self.units.len() > self.budget_units {
                        let Some((_, oldest)) = self.order.pop_first() else { break };
                        self.units.remove(&oldest);
                    }
                    self.report();
                }
            }
            f = to;
        }
    }

    /// Drop every burst whose scene the current renderer no longer gives it (after an edit).
    pub fn retain_valid(&mut self, scene: &dyn Fn(i64) -> u64) {
        let before = self.units.len();
        let order = &mut self.order;
        self.units.retain(|(_, k), u| {
            let keep = u.scene == scene(*k);
            if !keep {
                order.remove(&u.stamp);
            }
            keep
        });
        self.changed |= self.units.len() != before;
        self.report();
        let dropped = (before - self.units.len()) as u64;
        kt_time::trace::ev(kt_time::trace::Ev::Invalidate).aux(kt_time::trace::inval::MIX_STORE).b(dropped).emit();
    }

    /// Drop the laps of every loop region but `keep`.
    pub fn retain_laps(&mut self, keep: Option<(i64, i64)>) {
        let before = self.units.len();
        let order = &mut self.order;
        self.units.retain(|(ctx, _), u| {
            let k = match ctx {
                MixCtx::Linear => true,
                MixCtx::Lap(s, e) => keep == Some((*s, *e)),
            };
            if !k {
                order.remove(&u.stamp);
            }
            k
        });
        self.changed |= self.units.len() != before;
        self.report();
    }

    pub fn clear(&mut self) {
        kt_time::trace::ev(kt_time::trace::Ev::Invalidate)
            .aux(kt_time::trace::inval::MIX_STORE)
            .b(self.units.len() as u64)
            .a(1)
            .emit();
        self.changed |= !self.units.is_empty();
        self.units.clear();
        self.order.clear();
        self.report();
    }

    /// The timeline frame runs each context holds, merged and sorted.
    pub fn runs(&self) -> MixRuns {
        let mut by: BTreeMap<MixCtx, Vec<Range<i64>>> = BTreeMap::new();
        for ((ctx, k), u) in &self.units {
            by.entry(*ctx).or_default().push(k * UNIT + u.lo..k * UNIT + u.hi);
        }
        let mut out = MixRuns::default();
        for (ctx, mut v) in by {
            v.sort_by_key(|r| r.start);
            let mut merged: Vec<Range<i64>> = Vec::new();
            for r in v {
                match merged.last_mut() {
                    Some(m) if m.end >= r.start => m.end = m.end.max(r.end),
                    _ => merged.push(r),
                }
            }
            match ctx {
                MixCtx::Linear => out.linear = merged,
                MixCtx::Lap(s, e) => out.laps.push(((s, e), merged)),
            }
        }
        out
    }

    /// Publish the runs if they changed.
    pub fn publish(&mut self, to: &MixCoverage) {
        if std::mem::take(&mut self.changed) {
            *to.0.lock().unwrap() = Arc::new(self.runs());
        }
    }
}

/// The laps the store holds: per loop region `(start, end)`, its runs.
pub type LapRuns = Vec<((i64, i64), Vec<Range<i64>>)>;

/// What the store holds, in timeline frames.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MixRuns {
    pub linear: Vec<Range<i64>>,
    pub laps: LapRuns,
}

impl MixRuns {
    /// What plays without work now: the laps of `region` inside it, the linear runs outside.
    pub fn playing(&self, region: Option<(i64, i64)>) -> Vec<Range<i64>> {
        let Some((rs, re)) = region else { return self.linear.clone() };
        let mut out: Vec<Range<i64>> = Vec::new();
        for r in &self.linear {
            if r.start < rs {
                out.push(r.start..r.end.min(rs));
            }
            if r.end > re {
                out.push(r.start.max(re)..r.end);
            }
        }
        if let Some((_, v)) = self.laps.iter().find(|(reg, _)| *reg == (rs, re)) {
            out.extend(v.iter().map(|r| r.start.max(rs)..r.end.min(re)).filter(|r| !r.is_empty()));
        }
        out.sort_by_key(|r| r.start);
        out
    }

    /// Is all of `[start, end)` held in `ctx`'s runs?
    pub fn covers(&self, ctx: MixCtx, win: Range<i64>) -> bool {
        let runs = match ctx {
            MixCtx::Linear => &self.linear,
            MixCtx::Lap(s, e) => match self.laps.iter().find(|(reg, _)| *reg == (s, e)) {
                Some((_, v)) => v,
                None => return win.is_empty(),
            },
        };
        win.is_empty() || runs.iter().any(|r| r.start <= win.start && r.end >= win.end)
    }
}

/// The store's runs as its thread last published them; the bars and the prefetch read it.
#[derive(Debug, Default, Clone)]
pub struct MixCoverage(pub Arc<Mutex<Arc<MixRuns>>>);

impl MixCoverage {
    pub fn get(&self) -> Arc<MixRuns> {
        self.0.lock().unwrap().clone()
    }
}

/// The pieces of the output stream `[head, head + n)` of the render head: where each lies on the
/// timeline and which recording it belongs to. With an armed loop the head runs on linearly and
/// is folded into the region; the lap it is on decides the context.
pub fn pieces(head: i64, n: i64, region: Option<(i64, i64)>) -> Vec<(MixCtx, i64, i64)> {
    match region {
        Some((rs, re)) if re > rs && head >= rs => {
            let len = re - rs;
            let mut out = Vec::new();
            let mut done = 0;
            while done < n {
                let h = head + done;
                let lap = (h - rs).div_euclid(len);
                let tl = rs + (h - rs).rem_euclid(len);
                let m = (re - tl).min(n - done);
                let ctx = if lap == 0 { MixCtx::Linear } else { MixCtx::Lap(rs, re) };
                out.push((ctx, tl, m));
                done += m;
            }
            out
        }
        _ => vec![(MixCtx::Linear, head, n)],
    }
}

/// The forward render of the player through the store: what it holds is handed out, what it
/// does not is rendered live and kept.
///
/// **A kept chunk is what a continuous play gives** (kept-mix-settle, found-defects 390). The FX
/// state does not run while the store plays, so the live chunk after a replayed run first brings
/// it back: through the replayed frames themselves when they are within the settle length
/// ([`AudioRenderer::settle_frames`]) — the state goes on exactly as if nothing had been replayed —
/// else by a reset and a discarded play of the settle length before the chunk, through the same
/// path, so in a loop lap the warm-up runs through the wrap and rings the previous lap, not the
/// timeline before the region. A reset alone pre-rolled each event from the timeline: a lap kept
/// that way rang the wrong past and was replayed on every lap. What could not warm (a starved
/// source in a pre-roll or a chunk, a track stage after a seek) keeps nothing until the settle
/// length has run.
pub struct MixPlay {
    pub store: MixStore,
    /// Where the FX state stopped: the head of the first chunk handed out from the store since it
    /// last rendered, and the region it was handed out under. `None` — it rendered the previous
    /// chunk, or a seek reset it.
    stale_at: Option<(i64, Option<(i64, i64)>)>,
    /// A chunk whose head is before this is not kept: the effects have not run their settle length
    /// since what they could not warm.
    keep_from: i64,
}

impl Default for MixPlay {
    fn default() -> Self {
        MixPlay { store: MixStore::default(), stale_at: None, keep_from: i64::MIN }
    }
}

/// What one chunk took.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ChunkOutcome {
    pub stats: RenderStats,
    /// Handed out from the store: no decode, no effect ran.
    pub replayed: bool,
    /// Frames rendered and discarded before it to bring the effects back (kept-mix-settle).
    pub warmed: i64,
}

/// The settle length of the chains heard in the output frames `[head, head + n)`: the render head
/// is folded into an armed loop, so it is asked of the timeline frames it plays.
fn settle_at(renderer: &AudioRenderer, region: Option<(i64, i64)>, head: i64, n: i64) -> i64 {
    pieces(head, n, region).iter().map(|&(_, tl, m)| renderer.settle_frames(tl..tl + m)).max().unwrap_or(0)
}

/// One chunk of the forward stream at the render head, as the player renders it: folded into an
/// armed loop from its start on, linear before it.
fn render_chunk(
    renderer: &AudioRenderer,
    fetcher: &mut dyn AudioFetcher,
    fx: &mut FxStateStore,
    region: Option<(i64, i64)>,
    head: i64,
    out: &mut [f32],
) -> RenderStats {
    match region {
        Some((rs, re)) if re > rs && head >= rs => renderer.render_looped_with(fetcher, fx, rs..re, head, out),
        _ => renderer.render_frames_with(fetcher, fx, head, out),
    }
}

impl MixPlay {
    /// One chunk of the forward stream at the render head `head` into `out`: from the store when
    /// it holds all of it at the current scenes, else rendered live (`render_frames_with` or,
    /// under an armed loop, `render_looped_with`) and kept when nothing in it was missing and the
    /// effects have settled.
    pub fn render(
        &mut self,
        renderer: &AudioRenderer,
        fetcher: &mut dyn AudioFetcher,
        fx: &mut FxStateStore,
        region: Option<(i64, i64)>,
        head: i64,
        out: &mut [f32],
    ) -> ChunkOutcome {
        let n = (out.len() / AUDIO_CHANNELS) as i64;
        let parts = pieces(head, n, region);
        let scene = |k: i64| renderer.scene_hash(k * UNIT..(k + 1) * UNIT);
        if parts.iter().all(|&(ctx, tl, m)| self.store.holds(ctx, tl, m, &scene)) {
            let mut done = 0usize;
            for &(ctx, tl, m) in &parts {
                let len = m as usize * AUDIO_CHANNELS;
                self.store.read(ctx, tl, &mut out[done..done + len], &scene);
                done += len;
            }
            if self.stale_at.is_none() {
                self.stale_at = Some((head, region));
            }
            kt_time::trace::ev(kt_time::trace::Ev::MixChunk).a(head as u64).b(n as u64).aux(kt_time::trace::mix::REPLAY).emit();
            return ChunkOutcome { stats: RenderStats::default(), replayed: true, warmed: 0 };
        }
        // why the chunk is live: the first piece the store could not hand out, and its context
        let why = if kt_time::trace::on() {
            parts
                .iter()
                .map(|&(ctx, tl, m)| self.store.miss_reason(ctx, tl, m, &scene) | (u64::from(ctx != MixCtx::Linear) << 8))
                .find(|r| r & 0xff != 0)
                .unwrap_or(0)
        } else {
            0
        };
        let warmed = match self.stale_at.take() {
            Some((at, was)) => self.warm(renderer, fetcher, fx, region, head, n, at, was),
            None => 0,
        };
        let stats = render_chunk(renderer, fetcher, fx, region, head, out);
        let clean = stats.underruns == 0 && stats.warmup_misses == 0;
        if !clean {
            // what the effects took in here is a hole: it rings on for their settle length
            self.keep_from = self.keep_from.max(head + n + settle_at(renderer, region, head, n));
        }
        let keep = clean && head >= self.keep_from;
        if keep {
            let mut done = 0usize;
            for &(ctx, tl, m) in &parts {
                let len = m as usize * AUDIO_CHANNELS;
                self.store.write(ctx, tl, &out[done..done + len], &scene);
                done += len;
            }
        }
        {
            use kt_time::trace::{self, mix};
            trace::ev(trace::Ev::MixChunk)
                .a(head as u64)
                .b(n as u64)
                .c(why)
                .aux(if keep {
                    mix::KEPT
                } else if clean {
                    mix::UNSETTLED
                } else {
                    mix::NOT_CLEAN
                })
                .emit();
        }
        ChunkOutcome { stats, replayed: false, warmed }
    }

    /// Bring the effects back before the live chunk at `head` (`n` frames): they stopped at `at`
    /// under `was`. Returns the frames rendered and discarded.
    #[allow(clippy::too_many_arguments)]
    fn warm(
        &mut self,
        renderer: &AudioRenderer,
        fetcher: &mut dyn AudioFetcher,
        fx: &mut FxStateStore,
        region: Option<(i64, i64)>,
        head: i64,
        n: i64,
        at: i64,
        was: Option<(i64, i64)>,
    ) -> i64 {
        use kt_time::trace::mix;
        let settle = settle_at(renderer, region, head, n);
        let (from, how) = if settle == 0 {
            fx.reset_all();
            (head, mix::WARM_SEEK)
        } else if was == region && at <= head && head - at <= settle {
            // the state stands at `at`: going on from there through the replayed frames is the
            // continuous play itself
            (at, mix::WARM_CONTINUED)
        } else {
            // a reset, then the settle length before the chunk on the player's chunk grid
            fx.reset_all();
            (head - (settle + n - 1) / n * n, mix::WARM_WINDOW)
        };
        let mut scratch = vec![0f32; n as usize * AUDIO_CHANNELS];
        let mut h = from;
        let mut starved = false;
        while h < head {
            let m = (head - h).min(n);
            let st = render_chunk(renderer, fetcher, fx, region, h, &mut scratch[..m as usize * AUDIO_CHANNELS]);
            starved |= st.underruns > 0 || st.warmup_misses > 0;
            h += m;
        }
        if starved {
            // the warm-up read silence where the source was missing: what follows is not settled
            self.keep_from = self.keep_from.max(head + settle);
        }
        kt_time::trace::ev(kt_time::trace::Ev::MixChunk)
            .a(head as u64)
            .b((head - from) as u64)
            .c(how)
            .aux(mix::WARM)
            .emit();
        head - from
    }

    /// The renderer changed (an edit): drop what it would render differently.
    pub fn revalidate(&mut self, renderer: &AudioRenderer) {
        self.store.retain_valid(&|k| renderer.scene_hash(k * UNIT..(k + 1) * UNIT));
    }

    /// A seek (or a change of direction, a loop flip) reset the FX state and moved the render head
    /// to `head`: the state is no longer behind the store, and what plays from here is a play from
    /// a seek. A track stage has no pre-roll, so while its lane's events still ring in it the
    /// chunks are not kept ([`AudioRenderer::cold_after_seek`]).
    pub fn fx_reset(&mut self, renderer: &AudioRenderer, head: i64) {
        self.stale_at = None;
        self.keep_from = head + renderer.cold_after_seek(head);
    }
}
