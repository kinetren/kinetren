//! The STREAM class (datamosh-codec-path, D2): a source stage whose output at frame N is a pure
//! function of its rows and of the source frames of a bounded HISTORY it names for N.
//!
//! It is not video state (the registry still refuses `state`, `tail` and `lookahead` for video):
//! the kernel keeps nothing between calls. What makes it affordable is the host-owned
//! [`StreamMemo`] — a bounded cache keyed by what determines the result, which the kernel reads
//! and fills. The bytes never depend on what the memo holds (invariant 11).
//!
//! Positions are the project grid's frame indices. The host maps a position to a source frame
//! through the stream's layout: before the cut — the event before; from the cut — the event
//! itself; on the second side — the event on the other track at that moment.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Which input a stream position reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    /// The event's own stream (the event before included, when the stream starts there).
    Main,
    /// The second input: the event active at the same moment on another track.
    Second,
}

/// Where the second input comes from (a choice row the kernel declares).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondFrom {
    None,
    /// The track drawn above the event's own.
    TrackAbove,
    /// The track drawn below it.
    TrackBelow,
}

/// The stream's layout as the host resolved it for one event. Reads no frame.
pub trait StreamLayout {
    /// The size the host orders the stream's frames at (`lines`, the layer's aspect).
    fn size(&self) -> (u32, u32);
    /// The first position of the main stream (the cut, or earlier when it starts in the event before).
    fn first(&self) -> i64;
    /// The event's own first position — the cut.
    fn cut(&self) -> i64;
    /// One past the event's last position.
    fn end(&self) -> i64;
    /// The first position of the second input's event active at `k`; `None` — no event there.
    fn second_start(&self, k: i64) -> Option<i64>;
    /// The ADDRESS of the source frame at `k`, hashed: the media, the source pts, the size, the
    /// source's revision and whatever the host runs on it before the kernel reads it (the chain's
    /// slots in front of the stage, stream-input-order). 0 — there is no frame at `k` on this side. A memo key is built from
    /// addresses alone, so "what is still needed" is answered without reading a frame.
    fn ident(&self, side: Side, k: i64) -> u64;
    /// The slot's rows AT position `k`, its curves applied (positions before the cut read the
    /// event's start); `None` — the rows have no curves and the static values hold everywhere.
    /// A kernel reads every per-frame quantity here, so frame N stays a pure function of the curves.
    fn rows_at(&self, _k: i64) -> Option<Vec<f64>> {
        None
    }
    /// The identity of the REGION OF INTEREST at `k` (0 — no region: the whole frame). The bridge
    /// for a mask or a detector (faces, people): the host names WHERE the effect acts, the kernel
    /// decides what that means (the datamosh shows its broken stream inside, the intact one outside).
    fn region_ident(&self, _k: i64) -> u64 {
        0
    }
    /// Where the kernel keeps what it COMPILED from this layout (stream-layout-per-build): its
    /// schedule, the rows per position, the content keys. The host gives one cell per stage, build
    /// and stream size, and drops it with the build, so the kernel compiles once and every later
    /// question (`needs`, `key`, `render`) reads it. `None` — no cell: the kernel compiles per
    /// call, to the same values.
    fn plan_cell(&self) -> Option<&PlanCell> {
        None
    }
    /// The earliest position the host will show from now on (stream-history-on-device D4): the loop
    /// region's start while the playhead is in it, else the frame on screen. A history walk keeps the
    /// pictures from here on and only sparse checkpoints before it; `None` — the host shows every
    /// position (an export): keep every picture.
    fn shown_from(&self) -> Option<i64> {
        None
    }
}

/// A host-owned slot for a kernel's compiled schedule of one layout (stream-layout-per-build). Opaque
/// to the host; filled once, by the first question. Speed only: what it holds is a pure function of
/// the rows and the layout.
#[derive(Default)]
pub struct PlanCell(OnceLock<Arc<dyn Any + Send + Sync>>);

impl PlanCell {
    /// The compiled value, built by `f` at the first call; `None` — the cell holds another type.
    pub fn get_or_init<T: Any + Send + Sync>(&self, f: impl FnOnce() -> T) -> Option<Arc<T>> {
        self.0.get_or_init(|| Arc::new(f())).clone().downcast::<T>().ok()
    }
}

/// Run `f`, a kernel compiling positions `from .. to` of its schedule, as a record of the structured
/// trace (`stream_plan`); `f` returns the positions the schedule holds after it.
pub fn compiling(from: i64, to: i64, f: impl FnOnce() -> usize) {
    let mut span = kt_time::trace::span(kt_time::trace::Ev::StreamPlan);
    span.a(from as u64).b(to as u64);
    let held = f();
    span.c(held as u64);
}

/// A batch of a history walk was submitted and came back (stream-history-on-device): the trace's
/// `stream_walk` record — its first position, its positions, the pictures it brought back and the
/// checkpoints it kept.
pub fn walked(first: i64, positions: usize, reads: u64, checkpoints: u32) {
    WALKED.fetch_add(positions as u64, std::sync::atomic::Ordering::Relaxed);
    kt_time::trace::ev(kt_time::trace::Ev::StreamWalk).a(first as u64).b(positions as u64).c(reads).aux(checkpoints).emit();
}

static WALKED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// History positions every walk of the process computed so far: the host's sign of progress
/// (gpu-throughput D3 — a look-ahead that computed nothing over several passes stops retrying).
pub fn positions_walked() -> u64 {
    WALKED.load(std::sync::atomic::Ordering::Relaxed)
}

impl std::fmt::Debug for PlanCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.get().is_some() { "PlanCell(filled)" } else { "PlanCell(empty)" })
    }
}

/// One RGBA frame at the stream's size.
pub struct Rgba<'a> {
    pub w: u32,
    pub h: u32,
    pub rgba: &'a [u8],
}

/// The layout plus the frames themselves.
pub trait StreamInput: StreamLayout {
    /// Calls `f` with the frame at `k` when the host has it; `false` — it is missing (starvation).
    fn with_frame(&self, side: Side, k: i64, f: &mut dyn FnMut(Rgba<'_>)) -> bool;
    /// The region of interest at `k` as a coverage plane at the stream's size (0 — outside, 255 —
    /// inside), when `region_ident(k) != 0`; `false` — no region (the whole frame). No host part
    /// gives one yet: masks and object detection will, through this one door.
    fn with_region(&self, _k: i64, _f: &mut dyn FnMut(&[u8])) -> bool {
        false
    }
    /// Nobody wants the frame being rendered any more (gpu-arbiter, row 147): the preview moved to
    /// another moment, an edit replaced the state, the playhead left a look-ahead, a job was
    /// cancelled. A kernel asks it before each history position it computes and, on `true`, returns
    /// `None` with what it computed kept in the memo — a later render resumes there and gives the
    /// same bytes. An export never stops.
    fn stop(&self) -> bool {
        false
    }
}

/// What a render produced: the size of `out` and the identity of the bytes (the host puts it into
/// the layer's content key, so no cache returns another stream's picture).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamOut {
    pub w: u32,
    pub h: u32,
    pub key: u64,
}

/// A stream kernel (`VideoClass::Stream`).
pub trait StreamKernel: Send + Sync {
    /// The encoded height: the host orders the stream at `(even(lines × aspect), lines)`.
    fn lines(&self, p: &[f64]) -> u32;
    /// How many frames of the event before the stream starts in (0 — it starts at the cut).
    fn before(&self, p: &[f64]) -> u32;
    /// The second input.
    fn second(&self, p: &[f64]) -> SecondFrom;
    /// The positions whose frames are still needed to produce frame `n`, given the memo; empty —
    /// the memo already holds it. Ascending, without duplicates.
    fn needs(&self, p: &[f64], lay: &dyn StreamLayout, n: i64, memo: &StreamMemo) -> Vec<(Side, i64)>;
    /// How many checkpoints a walk of frame `n`'s history from its anchor keeps in the memo's pinned
    /// tier, each about two of the stage's pictures (stream-history-on-device: the host sizes the
    /// memo by the walk, not only by its loop). 0 — the kernel keeps none.
    fn checkpoints(&self, _p: &[f64], _lay: &dyn StreamLayout, _n: i64) -> usize {
        0
    }
    /// The key `render` would return for frame `n` (structured-telemetry), from the rows and the
    /// addresses alone: nothing computed, the memo not read. The host finds a kept composite of the
    /// frame by it without running the stage, so a covered loop never waits for a history the memo
    /// has dropped. `None` — the kernel cannot say; the host runs the stage. Must equal the key of a
    /// `render(.., n, ..)` that returns, or a kept composite is shown for another picture.
    fn key(&self, _p: &[f64], _lay: &dyn StreamLayout, _n: i64) -> Option<u64> {
        None
    }
    /// Advance the history to frame `n` in the memo without producing its picture (the prefetch's
    /// advance, the export's pre-roll: stream-layout-per-build — they converted a picture and threw it
    /// away);
    /// `false` — a frame of the history is missing. By default `render` into a scratch buffer.
    fn advance(&self, p: &[f64], input: &dyn StreamInput, n: i64, memo: &StreamMemo) -> bool {
        self.render(p, input, n, memo, &mut Vec::new()).is_some()
    }
    /// Produce frame `n` as RGBA (opaque) into `out`; `None` — a frame of the history is missing.
    /// Everything computed before the gap stays in the memo (progress is monotonic).
    fn render(
        &self,
        p: &[f64],
        input: &dyn StreamInput,
        n: i64,
        memo: &StreamMemo,
        out: &mut Vec<u8>,
    ) -> Option<StreamOut>;
}

/// One memo entry: opaque to the host, sized for the bound.
struct Entry {
    v: Arc<dyn Any + Send + Sync>,
    bytes: usize,
    /// Its last use.
    used: u64,
    transient: bool,
    /// A checkpoint of a history walk and its rank (stream-history-on-device D5): outside the LRU.
    pinned: Option<u32>,
    /// The lap of its last use (`StreamMemo::lap`).
    lap: u64,
    /// Admitted while the memo was full of the loop's own laps (stream-memo-budget): it lives in the
    /// window past the part of the loop the memo keeps, and goes first.
    probation: bool,
}

/// The newest transient entries the eviction spares like any other: what the next step still reads
/// (the clean encoder's state of the last frame, on each side).
const TRANSIENT_SPARED: usize = 4;

/// The share of the bound the window past a kept part of the loop takes (stream-memo-budget): what the
/// look-ahead walks and the composite then reads, a quarter as the look-ahead's reach.
const WINDOW_DIV: usize = 4;

/// The host-owned memo of a stream kernel (design D3): a bounded LRU of opaque entries keyed by a
/// `u64` the kernel derives from what determines the entry. Speed only — never another byte.
///
/// Told of a loop (`keep_loop`) it cannot hold, it keeps a part (stream-memo-budget, found-defects
/// 517, as the present memo does, memory-ledger D5): an entry used in this lap or the last is not
/// evicted for a new one; the new one goes to a window of a quarter of the bound past the kept part,
/// oldest first. An LRU over a cycle longer than itself evicts exactly what the next lap reads first.
pub struct StreamMemo {
    /// The bound; the host may change it while the memo is used (stream-layout-per-build: the
    /// preview sizes it by its loop and the machine's memory).
    cap: std::sync::atomic::AtomicUsize,
    inner: Mutex<MemoInner>,
    /// Its account in the process's memory ledger (memory-ledger): stream history, its first bound
    /// as the base.
    account: kt_time::mem::Account,
    /// The host's view (`set_view`), in the host's own time unit; `i64::MIN` — none.
    view: std::sync::atomic::AtomicI64,
}

#[derive(Default)]
struct MemoInner {
    map: HashMap<u64, Entry>,
    bytes: usize,
    /// Of `bytes`, the pinned tier's.
    pinned: usize,
    /// Of `bytes`, the window's (`Entry::probation`).
    probation: usize,
    clock: u64,
    /// A loop is kept (`keep_loop`).
    looping: bool,
    /// The laps the host marked (`lap`).
    lap: u64,
}

impl StreamMemo {
    /// A memo holding at most `cap_bytes` (an entry larger than that is not kept).
    pub fn new(cap_bytes: usize) -> StreamMemo {
        StreamMemo {
            cap: std::sync::atomic::AtomicUsize::new(cap_bytes),
            inner: Mutex::new(MemoInner::default()),
            account: kt_time::mem::open(kt_time::mem::Tenant::History, cap_bytes as u64),
            view: std::sync::atomic::AtomicI64::new(i64::MIN),
        }
    }

    /// The host's view of what it shows from now on, in its own time unit (the preview's player sets
    /// it; an export never does): the host's `StreamLayout::shown_from` reads it. `None` clears it.
    pub fn set_view(&self, at: Option<i64>) {
        self.view.store(at.unwrap_or(i64::MIN), std::sync::atomic::Ordering::Relaxed);
    }

    pub fn view(&self) -> Option<i64> {
        let v = self.view.load(std::sync::atomic::Ordering::Relaxed);
        (v != i64::MIN).then_some(v)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, MemoInner> {
        // an entry is immutable once in; a panic elsewhere cannot leave it half-written
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The entry under `key`, as the kernel's type; bumps its recency.
    pub fn get<T: Any + Send + Sync>(&self, key: u64) -> Option<Arc<T>> {
        let mut g = self.lock();
        g.clock += 1;
        let now = g.clock;
        let lap = g.lap;
        let e = g.map.get_mut(&key)?;
        e.used = now;
        e.lap = lap;
        e.v.clone().downcast::<T>().ok()
    }

    /// Whether `key` is held (no recency bump: a question, not a use).
    pub fn contains(&self, key: u64) -> bool {
        self.lock().map.contains_key(&key)
    }

    /// Keep `v` under `key`, evicting the least recently used entries past the bound.
    pub fn put<T: Any + Send + Sync>(&self, key: u64, v: Arc<T>, bytes: usize) {
        self.put_entry(key, v, bytes, false, None);
    }

    /// `put` an entry only the next steps read (stream-layout-per-build: the clean encoder's state of
    /// a frame, which the encoder of the next frame reads, against the decoded picture a loop shows
    /// again): past the newest few, such entries go before any other, so a memo sized for a loop's
    /// pictures keeps them.
    pub fn put_transient<T: Any + Send + Sync>(&self, key: u64, v: Arc<T>, bytes: usize) {
        self.put_entry(key, v, bytes, true, None);
    }

    /// `put` a checkpoint of a history walk (stream-history-on-device D5): the LRU never drops it; the
    /// pinned tier holds at most half the bound and, past it, drops the lowest `rank` first (the
    /// oldest among equals), so checkpoints thin out evenly when a kernel ranks them by how many times
    /// their position divides by two.
    pub fn put_pinned<T: Any + Send + Sync>(&self, key: u64, v: Arc<T>, bytes: usize, rank: u32) {
        self.put_entry(key, v, bytes, false, Some(rank));
    }

    fn put_entry<T: Any + Send + Sync>(&self, key: u64, v: Arc<T>, bytes: usize, transient: bool, pinned: Option<u32>) {
        let cap = self.bytes_cap();
        if bytes > cap || pinned.is_some() && bytes > cap / 2 {
            return;
        }
        let mut g = self.lock();
        g.clock += 1;
        let now = g.clock;
        let lap = g.lap;
        if let Some(old) = g.map.remove(&key) {
            Self::uncount(&mut g, &old);
        }
        // the memo full of the loop's own laps: the new one goes to the window past the kept part
        let probation = g.looping && !transient && pinned.is_none() && g.bytes + bytes > cap && Self::stale_victim(&g).is_none();
        let e = Entry { v, bytes, used: now, transient, pinned, lap, probation };
        Self::count(&mut g, &e);
        g.map.insert(key, e);
        Self::evict_to(&mut g, cap, key);
        self.account.set(g.bytes as u64);
    }

    fn count(g: &mut MemoInner, e: &Entry) {
        g.bytes += e.bytes;
        if e.pinned.is_some() {
            g.pinned += e.bytes;
        }
        if e.probation {
            g.probation += e.bytes;
        }
    }

    fn uncount(g: &mut MemoInner, e: &Entry) {
        g.bytes -= e.bytes;
        if e.pinned.is_some() {
            g.pinned -= e.bytes;
        }
        if e.probation {
            g.probation -= e.bytes;
        }
    }

    /// The least recently used kept entry no lap of the loop used lately (neither this lap nor the
    /// last): what a loop it cannot hold may give up.
    fn stale_victim(g: &MemoInner) -> Option<u64> {
        g.map
            .iter()
            .filter(|(_, e)| e.pinned.is_none() && !e.transient && !e.probation && e.lap + 1 < g.lap)
            .min_by_key(|(_, e)| e.used)
            .map(|(k, _)| *k)
    }

    fn remove(g: &mut MemoInner, k: u64, cause: u64) {
        if let Some(e) = g.map.remove(&k) {
            Self::uncount(g, &e);
            // what the history lost, and to what (structured-telemetry)
            kt_time::trace::ev(kt_time::trace::Ev::StreamMemoEvict).id(k).a(e.bytes as u64).cause(cause).emit();
        }
    }

    /// Drop entries until `cap` holds the rest; `cause` names the key that made room necessary (0 —
    /// the bound shrank). The pinned tier first keeps under half the bound (lowest rank, then oldest);
    /// then the oldest transient past the newest few, else the least recently used unpinned entry;
    /// pinned entries go last. While a loop is kept, the unpinned victim is, in this order: the
    /// window's oldest past its quarter of the bound, a kept entry no recent lap used, the kept part's
    /// least recent (the window grows into it up to its quarter), the window's oldest.
    fn evict_to(g: &mut MemoInner, cap: usize, cause: u64) {
        let pinned_victim = |g: &MemoInner| g.map.iter().filter_map(|(k, e)| e.pinned.map(|r| ((r, e.used), *k))).min().map(|x| x.1);
        while g.pinned > cap / 2 {
            let Some(k) = pinned_victim(g) else { break };
            Self::remove(g, k, cause);
        }
        while g.bytes > cap {
            let mut tr: Vec<(u64, u64)> = g.map.iter().filter(|(_, e)| e.transient).map(|(k, e)| (e.used, *k)).collect();
            let victim = if tr.len() > TRANSIENT_SPARED {
                tr.select_nth_unstable(0);
                Some(tr[0].1)
            } else if g.looping {
                Self::loop_victim(g, cap, cause).or_else(|| pinned_victim(g))
            } else {
                g.map.iter().filter(|(_, e)| e.pinned.is_none()).min_by_key(|(_, e)| e.used).map(|(k, _)| *k).or_else(|| pinned_victim(g))
            };
            let Some(k) = victim else { break };
            Self::remove(g, k, cause);
        }
    }

    /// The unpinned victim while a loop is kept (`evict_to`).
    fn loop_victim(g: &MemoInner, cap: usize, cause: u64) -> Option<u64> {
        let oldest = |f: &dyn Fn(&Entry) -> bool| g.map.iter().filter(|(k, e)| **k != cause && e.pinned.is_none() && f(e)).min_by_key(|(_, e)| e.used).map(|(k, _)| *k);
        let window = || oldest(&|e: &Entry| e.probation);
        (if g.probation > cap / WINDOW_DIV { window() } else { None })
            .or_else(|| Self::stale_victim(g).filter(|k| *k != cause))
            .or_else(|| oldest(&|e: &Entry| !e.probation))
            .or_else(window)
            .or_else(|| g.map.get(&cause).filter(|e| e.pinned.is_none()).map(|_| cause))
    }

    /// Keep the loop the host plays (`true`) or none (stream-memo-budget): a loop it cannot hold
    /// keeps a part, lap after lap. A new loop starts a fresh count — what an earlier one kept is stale.
    pub fn keep_loop(&self, on: bool) {
        let mut g = self.lock();
        if on {
            g.lap += 2;
        } else {
            for e in g.map.values_mut() {
                e.probation = false;
            }
            g.probation = 0;
        }
        g.looping = on;
    }

    /// The host's loop came round (the playhead went back to its start): a new lap.
    pub fn lap(&self) {
        self.lock().lap += 1;
    }

    /// Drop every entry but the pinned tier (tests: an LRU that lost the history).
    pub fn drop_unpinned(&self) {
        let mut g = self.lock();
        let ks: Vec<u64> = g.map.iter().filter(|(_, e)| e.pinned.is_none()).map(|(k, _)| *k).collect();
        for k in ks {
            Self::remove(&mut g, k, 0);
        }
        self.account.set(g.bytes as u64);
    }

    /// Bytes of the pinned tier (checkpoints).
    pub fn pinned_bytes(&self) -> usize {
        self.lock().pinned
    }

    /// The bound.
    pub fn bytes_cap(&self) -> usize {
        self.cap.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// A new bound; a smaller one drops the least recently used entries now.
    pub fn set_cap(&self, cap_bytes: usize) {
        self.cap.store(cap_bytes, std::sync::atomic::Ordering::Relaxed);
        let mut g = self.lock();
        Self::evict_to(&mut g, cap_bytes, 0);
        self.account.set(g.bytes as u64);
    }

    /// Its account in the memory ledger: the base is its first bound.
    pub fn account(&self) -> &kt_time::mem::Account {
        &self.account
    }

    /// Bytes held.
    pub fn bytes(&self) -> usize {
        self.lock().bytes
    }

    /// Drop everything (tests: a cold memo).
    pub fn clear(&self) {
        let mut g = self.lock();
        g.map.clear();
        g.bytes = 0;
        g.pinned = 0;
        g.probation = 0;
        self.account.set(0);
    }
}

/// The key mixer every stream kernel and host hashes addresses with (splitmix64's finalizer over
/// `h ^ v`): one text, no dependency.
pub fn mix(h: u64, v: u64) -> u64 {
    let mut z = (h ^ v).wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test t2354 (stream-history-on-device D5): a pinned tier over half the bound thins evenly — the
    /// lowest rank goes first, the oldest among equals — and the LRU never drops a pinned entry.
    #[test]
    fn t2354_the_pinned_tier_thins_evenly() {
        const N: i64 = 16;
        let memo = StreamMemo::new(1 << 20);
        let rank = |k: i64| ((k / N).max(1) as u64).trailing_zeros().min(31);
        // 33 checkpoints of 10 KB: 330 KB, under half a 1 MB bound
        for i in 0..=32 {
            let k = i * N;
            memo.put_pinned(k as u64 + 1, Arc::new(k), 10_000, if k == 0 { 31 } else { rank(k) });
        }
        assert_eq!(memo.pinned_bytes(), 330_000);
        // the LRU fills past the bound: only unpinned entries go
        for i in 0..200u64 {
            memo.put(10_000 + i, Arc::new(i), 10_000);
        }
        assert_eq!(memo.pinned_bytes(), 330_000, "the LRU dropped a checkpoint");
        // half the bound for the tier: 170 KB → the odd multiples of N go, oldest first
        memo.set_cap(340_000);
        let held: Vec<i64> = (0..=32).map(|i| i * N).filter(|&k| memo.contains(k as u64 + 1)).collect();
        assert!(memo.pinned_bytes() <= 170_000);
        assert!(held.iter().all(|k| (k / N) % 2 == 0), "an odd multiple outlived an even one: {held:?}");
        assert!(held.contains(&(32 * N)) && held.contains(&0), "the newest and the anchor stay: {held:?}");
        assert!(held.len() >= 16, "it thinned more than half: {held:?}");
    }

    /// Laps of a loop of `n` positions of 10 KB over `memo`: per lap, the positions found held. Each
    /// position computed is put and read back at once, as the look-ahead's walk and the composite do.
    fn laps(memo: &StreamMemo, first: u64, n: u64, laps: usize, tell: bool) -> Vec<usize> {
        memo.keep_loop(tell);
        (0..laps)
            .map(|_| {
                let mut hits = 0;
                for k in first..first + n {
                    if memo.get::<u64>(k).is_some() {
                        hits += 1;
                    } else {
                        memo.put(k, Arc::new(k), 10_000);
                        assert!(memo.get::<u64>(k).is_some(), "a position just computed is not there for its composite");
                    }
                }
                memo.lap();
                hits
            })
            .collect()
    }

    /// Test t2490 (stream-memo-budget, found-defects 517): a loop three times the memo keeps a part of
    /// it lap after lap — what it held stays and the rest passes through a window of a quarter of the
    /// bound; without the loop told it is the LRU, which keeps nothing of a cycle longer than itself.
    /// A loop it holds whole is held whole either way.
    #[test]
    fn t2490_a_loop_longer_than_the_memo_keeps_a_part() {
        let lru = laps(&StreamMemo::new(100_000), 1, 30, 3, false);
        assert_eq!(lru, vec![0, 0, 0], "the LRU kept a part of a loop longer than itself");
        let kept = laps(&StreamMemo::new(100_000), 1, 30, 4, true);
        assert_eq!(kept[0], 0);
        assert!(kept[1..].iter().all(|&h| h >= 7), "a lap found less than the kept part: {kept:?}");
        assert_eq!(kept[2], kept[3], "the kept part moved between laps: {kept:?}");
        let whole = laps(&StreamMemo::new(400_000), 1, 30, 3, true);
        assert_eq!(whole, vec![0, 30, 30], "a loop the memo holds");
        // a new loop: what the old one kept goes for it, it is no longer protected
        let memo = StreamMemo::new(100_000);
        laps(&memo, 1, 30, 3, true);
        let other = laps(&memo, 1000, 30, 3, true);
        assert!(other[1..].iter().all(|&h| h >= 7), "the new loop kept nothing: {other:?}");
    }
}
