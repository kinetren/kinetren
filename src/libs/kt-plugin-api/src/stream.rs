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
    /// The ADDRESS of the source frame at `k`, hashed: the media, the source pts, the size and the
    /// source's revision. 0 — there is no frame at `k` on this side. A memo key is built from
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
    /// question (`needs`, `held`, `key`, `render`) reads it. `None` — no cell: the kernel compiles per
    /// call, to the same values.
    fn plan_cell(&self) -> Option<&PlanCell> {
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
    /// The latest position at or before `n` on `n`'s own stream whose output the memo holds — the
    /// picture a live preview shows while the stage is still computing its way to `n` (a late
    /// frame, as a late decode shows its last one); `None` — none, or the kernel does not say.
    fn held(&self, _p: &[f64], _lay: &dyn StreamLayout, _n: i64, _memo: &StreamMemo) -> Option<i64> {
        None
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
    /// advance, the export's pre-roll: stream-layout-per-build — they threw the picture away);
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
type Entry = (Arc<dyn Any + Send + Sync>, usize, u64);

/// The host-owned memo of a stream kernel (design D3): a bounded LRU of opaque entries keyed by a
/// `u64` the kernel derives from what determines the entry. Speed only — never another byte.
pub struct StreamMemo {
    /// The bound; the host may change it while the memo is used (stream-layout-per-build: the
    /// preview sizes it by its loop and the machine's memory).
    cap: std::sync::atomic::AtomicUsize,
    inner: Mutex<MemoInner>,
}

#[derive(Default)]
struct MemoInner {
    map: HashMap<u64, Entry>,
    bytes: usize,
    clock: u64,
}

impl StreamMemo {
    /// A memo holding at most `cap_bytes` (an entry larger than that is not kept).
    pub fn new(cap_bytes: usize) -> StreamMemo {
        StreamMemo { cap: std::sync::atomic::AtomicUsize::new(cap_bytes), inner: Mutex::new(MemoInner::default()) }
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
        let e = g.map.get_mut(&key)?;
        e.2 = now;
        e.0.clone().downcast::<T>().ok()
    }

    /// Whether `key` is held (no recency bump: a question, not a use).
    pub fn contains(&self, key: u64) -> bool {
        self.lock().map.contains_key(&key)
    }

    /// Keep `v` under `key`, evicting the least recently used entries past the bound.
    pub fn put<T: Any + Send + Sync>(&self, key: u64, v: Arc<T>, bytes: usize) {
        let cap = self.bytes_cap();
        if bytes > cap {
            return;
        }
        let mut g = self.lock();
        g.clock += 1;
        let now = g.clock;
        if let Some(old) = g.map.insert(key, (v, bytes, now)) {
            g.bytes -= old.1;
        }
        g.bytes += bytes;
        Self::evict_to(&mut g, cap, key);
    }

    /// Drop the least recently used entries until `cap` holds the rest; `cause` names the key that
    /// made room necessary (0 — the bound shrank).
    fn evict_to(g: &mut MemoInner, cap: usize, cause: u64) {
        let key = cause;
        while g.bytes > cap {
            let Some((&k, _)) = g.map.iter().min_by_key(|(_, e)| e.2) else { break };
            if let Some(e) = g.map.remove(&k) {
                g.bytes -= e.1;
                // what the history lost, and to what (structured-telemetry)
                kt_time::trace::ev(kt_time::trace::Ev::StreamMemoEvict).id(k).a(e.1 as u64).cause(key).emit();
            }
        }
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
