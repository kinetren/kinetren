//! The burst cache. The single truth about “warmth” (invariant 4):
//! coverage for the UI is a query to the cache itself, with no separate bookkeeping
//! (the old ms-buckets — defect g of v1 — lied in four
//! ways; here there is structurally nothing to lie with: the interval set is updated in the same
//! operations as the cache itself).
//!
//! Key: (source_id, kind{Audio | Video{quality}}, idx). Quality is part of the key:
//! entries of different qualities are not interchangeable.

use kt_media::{AudioBurst, Quality, VideoFrame};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BurstKind {
    Audio,
    /// None = the full resolution of the source.
    Video {
        quality: Option<(u32, u32)>,
    },
}

impl BurstKind {
    pub fn video(q: Option<Quality>) -> BurstKind {
        BurstKind::Video { quality: q.map(|q| (q.width, q.height)) }
    }

    /// The size the trace packs: audio `(0, 0)`, a native-size frame `(0, 1)`.
    pub fn trace_size(self) -> (u32, u32) {
        match self {
            BurstKind::Audio => (0, 0),
            BurstKind::Video { quality } => quality.unwrap_or((0, 1)),
        }
    }
}

/// The trace's id of a cache key (structured-telemetry): the decode request, the decoded entry, the
/// eviction and the frame waiting for it name the same number.
pub fn trace_id(source: SourceId, kind: BurstKind, idx: i64) -> u64 {
    let (w, h) = kind.trace_size();
    kt_time::trace::key_id(source.0, w, h, idx)
}

impl BurstKey {
    pub fn trace_id(&self) -> u64 {
        trace_id(self.source, self.kind, self.idx)
    }
}

/// An eviction in the trace: the key's parts, read or not, and the insert that pushed it out.
fn trace_evict(key: &BurstKey, had_hit: bool, wanted: bool, by: u64) {
    use kt_time::trace::{self, evict, Ev};
    if !trace::on() {
        return;
    }
    let (w, h) = key.kind.trace_size();
    let mut flags = 0;
    if !had_hit {
        flags |= evict::UNREAD;
        if wanted {
            flags |= evict::WANTED;
        }
    }
    if key.kind == BurstKind::Audio {
        flags |= evict::AUDIO;
    }
    trace::ev(Ev::CacheEvict)
        .id(key.trace_id())
        .a(key.source.0)
        .b(key.idx as u64)
        .c(trace::size(w, h))
        .aux(flags)
        .cause(by)
        .emit();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BurstKey {
    pub source: SourceId,
    pub kind: BurstKind,
    pub idx: i64,
}

#[derive(Clone)]
pub enum Payload {
    Audio(Arc<AudioBurst>),
    Video(Arc<VideoFrame>),
}

impl Payload {
    fn bytes(&self) -> usize {
        match self {
            Payload::Audio(b) => std::mem::size_of_val(&*b.data),
            Payload::Video(f) => f.rgba.len(),
        }
    }
    fn is_audio(&self) -> bool {
        matches!(self, Payload::Audio(_))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CacheConfig {
    pub max_audio_bytes: usize,
    pub max_video_bytes: usize,
}

impl Default for CacheConfig {
    fn default() -> CacheConfig {
        CacheConfig { max_audio_bytes: 64 << 20, max_video_bytes: 512 << 20 }
    }
}

/// The next content serial number of the process (`Entry::serial`).
static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

struct Entry {
    payload: Payload,
    /// The serial number of the CONTENT (R.53, item 409): monotonic, never
    /// reused. By it consumers (the texture upload into kt-gpu,
    /// the composite memo) tell apart “the same address, other bytes” — the addresses
    /// `(source, time, size)` are too few for this: a generator with new
    /// parameters and a file that came back in place of a placeholder give the SAME address
    /// and a different picture (t496, t211). Unique across every cache of the process.
    serial: u64,
    recency: u64,
    pinned: bool,
    /// How many users still need this entry (`hold_video`): not evicted while above zero.
    holds: u32,
    /// Was there at least one get after the insertion — eviction of an unread entry
    /// is prefetch-thrash (the performance panel: “no-read drops”, port of v1 hadHit).
    had_hit: bool,
    /// The last prefetch pass (`refresh`) that still wanted this entry — or the one current at
    /// its insertion. Evicted unread while it equals the current pass, the entry is thrash;
    /// older, the prefetch had stopped asking for it (a seek left it, the picture skipped its
    /// frame), and the drop is "abandoned" (video-prefetch-thrash).
    wanted: u64,
    /// The width of an entry’s coverage in idx units (audio = 1 burst; video = the step of
    /// the frame grid in ticks — coverage is merged into honest bars, item 20).
    span: i64,
}

/// A set of non-overlapping half-open ranges [start, end) by idx.
#[derive(Default, Debug, Clone)]
struct IntervalSet {
    runs: BTreeMap<i64, i64>, // start -> end
}

impl IntervalSet {
    fn insert_range(&mut self, idx: i64, idx_end: i64) {
        if idx_end <= idx {
            return;
        }
        let (mut start, mut end) = (idx, idx_end);
        // the neighbour on the left (may contain or adjoin)
        if let Some((&s, &e)) = self.runs.range(..=idx).next_back()
            && e >= start
        {
            if e >= end {
                return; // already covered
            }
            start = s;
            end = end.max(e);
            self.runs.remove(&s);
        }
        // all runs starting inside/on the boundary of the new range
        let overlapped: Vec<i64> = self.runs.range(start..=end).map(|(&s, _)| s).collect();
        for s in overlapped {
            let e = self.runs.remove(&s).unwrap();
            end = end.max(e);
        }
        self.runs.insert(start, end);
    }

    fn remove_range(&mut self, idx: i64, idx_end: i64) {
        if idx_end <= idx {
            return;
        }
        // the run on the left covering the start
        if let Some((&s, &e)) = self.runs.range(..=idx).next_back()
            && idx < e
        {
            self.runs.remove(&s);
            if s < idx {
                self.runs.insert(s, idx);
            }
            if idx_end < e {
                self.runs.insert(idx_end, e);
            }
        }
        // runs starting inside the range being cut out
        let inside: Vec<i64> = self.runs.range(idx..idx_end).map(|(&s, _)| s).collect();
        for s in inside {
            let e = self.runs.remove(&s).unwrap();
            if idx_end < e {
                self.runs.insert(idx_end, e);
            }
        }
    }

    fn query(&self, range: std::ops::Range<i64>) -> Vec<std::ops::Range<i64>> {
        let mut out = Vec::new();
        // the first run may start before range.start
        let iter = self
            .runs
            .range(..range.end)
            .rev()
            .take_while(|&(_, &e)| e > range.start)
            .collect::<Vec<_>>();
        for (&s, &e) in iter.into_iter().rev() {
            let a = s.max(range.start);
            let b = e.min(range.end);
            if a < b {
                out.push(a..b);
            }
        }
        out
    }
}

struct Inner {
    map: HashMap<BurstKey, Entry>,
    // recency -> key; the uniqueness of recency is guaranteed by a monotonic counter
    lru: BTreeMap<u64, BurstKey>,
    /// The keys of each source (R.53, item 409): `invalidate_source` otherwise
    /// scans the WHOLE cache, and it is now called on every edit of
    /// generator parameters. It is kept in sync with `map`.
    by_source: HashMap<SourceId, HashSet<BurstKey>>,
    coverage: HashMap<(SourceId, BurstKind), IntervalSet>,
    audio_bytes: usize,
    video_bytes: usize,
    /// The bytes of the entries with at least one hold.
    held_bytes: usize,
    recency_counter: u64,
    /// The number of the last prefetch pass, one per `refresh` call (video-prefetch-thrash).
    want_pass: u64,
    /// The serials of evicted entries (structured-telemetry): an entry decoded again from an
    /// unchanged source holds the same bytes (invariant 11), so it takes the serial it had, and
    /// what is keyed by the serial — the executor's kept composites — survives the eviction.
    /// `invalidate_source` forgets a source's (new bytes at the old address get a new serial,
    /// item 409). Bounded, oldest first.
    past: HashMap<BurstKey, u64>,
    past_order: std::collections::VecDeque<BurstKey>,
}

/// Evicted serials remembered (`Inner::past`): 24 bytes each.
const PAST_SERIALS: usize = 1 << 16;

impl Inner {
    fn remember(&mut self, key: BurstKey, serial: u64) {
        if self.past.insert(key, serial).is_none() {
            self.past_order.push_back(key);
            while self.past_order.len() > PAST_SERIALS {
                if let Some(k) = self.past_order.pop_front() {
                    self.past.remove(&k);
                }
            }
        }
    }
}

/// Counters for the performance panel (cumulative; the UI computes the deltas).
#[derive(Debug, Default, Clone, Copy)]
pub struct CacheStats {
    pub audio_hits: u64,
    pub audio_misses: u64,
    pub video_hits: u64,
    pub video_misses: u64,
    pub evictions: u64,
    /// Entries evicted/overwritten BEFORE the first read (prefetch-thrash).
    pub unused_evictions: u64,
    /// How many keys `invalidate_source` has viewed (item 409): a guard that
    /// it goes by the source index, and not by the whole cache.
    pub invalidate_scan_steps: u64,
    /// Entries evicted or overwritten unread AFTER the prefetch had stopped asking for them
    /// (a seek left the window, the picture skipped the frame): waste, not thrash — they are
    /// NOT in `unused_evictions` (video-prefetch-thrash).
    pub abandoned_evictions: u64,
}

#[derive(Default)]
struct StatCounters {
    audio_hits: AtomicU64,
    audio_misses: AtomicU64,
    video_hits: AtomicU64,
    video_misses: AtomicU64,
    evictions: AtomicU64,
    unused_evictions: AtomicU64,
    invalidate_scan_steps: AtomicU64,
    abandoned_evictions: AtomicU64,
}

/// A thread-safe burst cache with LRU over separate byte budgets for
/// audio/video, a pin (loop region) and coverage queries.
pub struct BurstCache {
    config: CacheConfig,
    inner: Mutex<Inner>,
    /// Changes on insert/evict/invalidate — the UI redraws the bars
    /// only when it changes.
    epoch: AtomicU64,
    stats: StatCounters,
    /// What the next insertion calls (`on_next_insert`); the flag keeps an insertion with
    /// nobody waiting to one atomic read.
    next_insert: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    next_armed: std::sync::atomic::AtomicBool,
    /// Its accounts in the process's memory ledger (memory-ledger): decoded frames, video and audio,
    /// the configured budgets as their bases.
    accounts: [kt_time::mem::Account; 2],
    /// What this cache asked above its base; 0 — it never asked (an export's, a reel strip's cache),
    /// and its cap is its base whatever the kind's grant.
    video_want: AtomicU64,
    /// The shrink hook is set at the first want (it needs the cache's `Arc`).
    hooked: AtomicBool,
}

impl BurstCache {
    pub fn new(config: CacheConfig) -> BurstCache {
        BurstCache::with_ledger(config, kt_time::mem::ledger())
    }

    /// A cache reporting to `ledger` (tests make their own; the program has one).
    pub fn with_ledger(config: CacheConfig, ledger: &'static kt_time::mem::Ledger) -> BurstCache {
        BurstCache {
            config,
            inner: Mutex::new(Inner {
                map: HashMap::new(),
                lru: BTreeMap::new(),
                by_source: HashMap::new(),
                coverage: HashMap::new(),
                audio_bytes: 0,
                video_bytes: 0,
                held_bytes: 0,
                recency_counter: 0,
                want_pass: 0,
                past: HashMap::new(),
                past_order: std::collections::VecDeque::new(),
            }),
            epoch: AtomicU64::new(0),
            stats: StatCounters::default(),
            next_insert: Mutex::new(None),
            next_armed: std::sync::atomic::AtomicBool::new(false),
            accounts: [
                ledger.open(kt_time::mem::Tenant::Frames, config.max_video_bytes as u64),
                ledger.open(kt_time::mem::Tenant::Frames, config.max_audio_bytes as u64),
            ],
            video_want: AtomicU64::new(0),
            hooked: AtomicBool::new(false),
        }
    }

    /// Ask the memory ledger for a video bound of `total` bytes (frames-cache-growth): what the
    /// preview's look-ahead and loop need at the current quality. Below the base it asks nothing.
    /// The cap becomes the base plus the grant; a lower cap evicts at once, here for this cache's
    /// own want and through the ledger's shrink hook when a claim or a higher tenant takes the
    /// growth back. Holds and pins stay stronger than any cap. The new cap.
    ///
    /// One cache of the process asks — the player's: the ledger keeps one want per kind.
    pub fn want_video(self: &Arc<Self>, total: usize) -> usize {
        if !self.hooked.swap(true, Ordering::AcqRel) {
            let weak = Arc::downgrade(self);
            self.accounts[0].on_shrink(move |_| {
                if let Some(c) = weak.upgrade() {
                    c.fit_cap();
                }
            });
        }
        let extra = total.saturating_sub(self.config.max_video_bytes) as u64;
        self.video_want.store(extra, Ordering::Relaxed);
        self.accounts[0].want(extra);
        self.fit_cap();
        self.max_video_bytes()
    }

    /// Evict what the cap no longer lets it hold (a grant taken back, a lower want of its own).
    fn fit_cap(&self) {
        let mut inner = self.inner.lock().unwrap();
        if inner.video_bytes <= self.max_video_bytes() {
            return;
        }
        let before = inner.map.len();
        self.evict_over_budget(&mut inner, 0);
        self.report(&inner);
        if inner.map.len() != before {
            self.epoch.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// The video bound as a reader that outlives a call (the stream pacing of a shared renderer
    /// build): it reads the cap as the ledger moves it, and the base once the cache is gone.
    pub fn video_cap_reader(self: &Arc<Self>) -> Arc<dyn Fn() -> usize + Send + Sync> {
        let weak = Arc::downgrade(self);
        let base = self.config.max_video_bytes;
        Arc::new(move || weak.upgrade().map_or(base, |c| c.max_video_bytes()))
    }

    /// The configured video budget: the base the cap never goes below.
    pub fn video_base(&self) -> usize {
        self.config.max_video_bytes
    }

    /// What it holds, to the memory ledger.
    fn report(&self, inner: &Inner) {
        self.accounts[0].set(inner.video_bytes as u64);
        self.accounts[1].set(inner.audio_bytes as u64);
    }

    /// Call `f` after the next insertion, on the inserting thread: how a paused preview waits for
    /// a frame without asking on a clock (idle-cpu). A later call replaces an earlier one.
    pub fn on_next_insert(&self, f: Box<dyn FnOnce() + Send>) {
        *self.next_insert.lock().unwrap() = Some(f);
        self.next_armed.store(true, Ordering::Release);
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            audio_hits: self.stats.audio_hits.load(Ordering::Relaxed),
            audio_misses: self.stats.audio_misses.load(Ordering::Relaxed),
            video_hits: self.stats.video_hits.load(Ordering::Relaxed),
            video_misses: self.stats.video_misses.load(Ordering::Relaxed),
            evictions: self.stats.evictions.load(Ordering::Relaxed),
            unused_evictions: self.stats.unused_evictions.load(Ordering::Relaxed),
            invalidate_scan_steps: self.stats.invalidate_scan_steps.load(Ordering::Relaxed),
            abandoned_evictions: self.stats.abandoned_evictions.load(Ordering::Relaxed),
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    /// A presence probe WITHOUT a recency bump and without statistics: for the planner and
    /// prefetch (item 19: get-probes in ascending order inverted the LRU — the frames
    /// nearest the playhead became “the oldest” and were evicted
    /// first).
    pub fn contains(&self, key: BurstKey) -> bool {
        self.inner.lock().unwrap().map.contains_key(&key)
    }

    pub fn contains_audio(&self, source: SourceId, idx: i64) -> bool {
        self.contains(BurstKey { source, kind: BurstKind::Audio, idx })
    }

    pub fn contains_video(&self, source: SourceId, quality: Option<Quality>, idx: i64) -> bool {
        self.contains(BurstKey { source, kind: BurstKind::video(quality), idx })
    }

    /// Mark the entries that are STILL WANTED as recent, in the given order: the last key
    /// ends up the most recent (video-prefetch-thrash). The prefetch passes its look-ahead
    /// window from the furthest frame to the nearest, so the eviction takes what was read
    /// or is no longer wanted first, and among the window the frame furthest ahead —
    /// never one the playhead reaches sooner. Unlike a read it does not set `had_hit`,
    /// counts no hit or miss and does not move the epoch; absent keys are skipped.
    /// (A plain `contains` probe must not do this: in ascending order it inverted the LRU,
    /// item 19.)
    pub fn refresh(&self, keys: impl IntoIterator<Item = BurstKey>) {
        let mut inner = self.inner.lock().unwrap();
        inner.want_pass += 1;
        let pass = inner.want_pass;
        for key in keys {
            inner.recency_counter += 1;
            let counter = inner.recency_counter;
            let Some(entry) = inner.map.get_mut(&key) else { continue };
            entry.wanted = pass;
            let old = entry.recency;
            entry.recency = counter;
            inner.lru.remove(&old);
            inner.lru.insert(counter, key);
        }
    }

    /// The video bound now: the base plus the ledger's grant (frames-cache-growth). The look-ahead
    /// takes half of it, the decoder's span room half, the stream pacing a quarter.
    pub fn max_video_bytes(&self) -> usize {
        // read at use: a grant given back after a claim reaches the cache with no hook (the ledger
        // tells only a shrink); this cache takes at most its own want of the kind's grant
        let grant = self.accounts[0].cap() - self.accounts[0].base();
        let own = grant.min(self.video_want.load(Ordering::Relaxed));
        self.config.max_video_bytes.saturating_add(usize::try_from(own).unwrap_or(usize::MAX))
    }

    pub fn get_audio(&self, source: SourceId, idx: i64) -> Option<Arc<AudioBurst>> {
        match self.get(BurstKey { source, kind: BurstKind::Audio, idx })? {
            Payload::Audio(b) => Some(b),
            Payload::Video(_) => unreachable!("audio key holds video payload"),
        }
    }

    pub fn get_video(
        &self,
        source: SourceId,
        quality: Option<Quality>,
        idx: i64,
    ) -> Option<Arc<VideoFrame>> {
        self.get_video_stamped(source, quality, idx).map(|(f, _)| f)
    }

    /// A frame together with the SERIAL NUMBER of the content (R.53, item 409). The number
    /// travels into the plan (`LayerSrc.content`) and further into the GPU keys: the address
    /// `(source, time, size)` is the same for the old and the new
    /// frame of a generator, while the bytes differ.
    pub fn get_video_stamped(
        &self,
        source: SourceId,
        quality: Option<Quality>,
        idx: i64,
    ) -> Option<(Arc<VideoFrame>, u64)> {
        let key = BurstKey { source, kind: BurstKind::video(quality), idx };
        match self.get_stamped(key)? {
            (Payload::Video(f), serial) => Some((f, serial)),
            (Payload::Audio(_), _) => unreachable!("video key holds audio payload"),
        }
    }

    pub fn get(&self, key: BurstKey) -> Option<Payload> {
        self.get_stamped(key).map(|(p, _)| p)
    }

    /// `get` + the serial number of the entry’s content (item 409).
    pub fn get_stamped(&self, key: BurstKey) -> Option<(Payload, u64)> {
        let mut inner = self.inner.lock().unwrap();
        inner.recency_counter += 1;
        let counter = inner.recency_counter;
        let is_audio = key.kind == BurstKind::Audio;
        let Some(entry) = inner.map.get_mut(&key) else {
            match key.kind {
                BurstKind::Audio => self.stats.audio_misses.fetch_add(1, Ordering::Relaxed),
                _ => self.stats.video_misses.fetch_add(1, Ordering::Relaxed),
            };
            return None;
        };
        match is_audio {
            true => self.stats.audio_hits.fetch_add(1, Ordering::Relaxed),
            false => self.stats.video_hits.fetch_add(1, Ordering::Relaxed),
        };
        entry.had_hit = true;
        let old = entry.recency;
        entry.recency = counter;
        let payload = entry.payload.clone();
        let serial = entry.serial;
        inner.lru.remove(&old);
        inner.lru.insert(counter, key);
        Some((payload, serial))
    }

    pub fn insert_audio(&self, source: SourceId, burst: Arc<AudioBurst>) {
        let idx = burst.idx;
        self.insert(BurstKey { source, kind: BurstKind::Audio, idx }, Payload::Audio(burst));
    }

    pub fn insert_video(
        &self,
        source: SourceId,
        quality: Option<Quality>,
        idx: i64,
        frame: Arc<VideoFrame>,
    ) {
        self.insert(
            BurstKey { source, kind: BurstKind::video(quality), idx },
            Payload::Video(frame),
        );
    }

    /// Video with a coverage span (the step of the frame grid, ticks): coverage
    /// is merged into honest bars instead of 1-tick points (item 20).
    pub fn insert_video_span(
        &self,
        source: SourceId,
        quality: Option<Quality>,
        idx: i64,
        span: i64,
        frame: Arc<VideoFrame>,
    ) {
        self.insert_spanned(
            BurstKey { source, kind: BurstKind::video(quality), idx },
            Payload::Video(frame),
            span.max(1),
        );
    }

    pub fn insert(&self, key: BurstKey, payload: Payload) {
        self.insert_spanned(key, payload, 1);
    }

    fn insert_spanned(&self, key: BurstKey, payload: Payload, span: i64) {
        self.insert_with_holds(key, payload, span, 0);
    }

    /// `holds_added` is taken in the same step as the insert: eviction cannot pass between them.
    fn insert_with_holds(&self, key: BurstKey, payload: Payload, span: i64, holds_added: u32) {
        let mut inner = self.inner.lock().unwrap();
        let bytes = payload.bytes();
        let is_audio = payload.is_audio();
        debug_assert_eq!(is_audio, key.kind == BurstKind::Audio);

        // an overwrite keeps the holds of the entry it replaces; its bytes are new (a new serial).
        // An entry evicted and decoded again takes the serial it had (`Inner::past`)
        let mut holds = holds_added;
        let mut kept_serial = inner.past.remove(&key);
        if let Some(old) = inner.map.remove(&key) {
            kept_serial = None;
            holds += old.holds;
            if old.holds > 0 {
                inner.held_bytes -= old.payload.bytes();
            }
            if !old.had_hit {
                // overwriting an unread entry is also thrash (v1)
                self.count_unread_drop(&old, inner.want_pass);
                trace_evict(&key, false, old.wanted == inner.want_pass, key.trace_id());
            }
            inner.lru.remove(&old.recency);
            let ob = old.payload.bytes();
            if old.payload.is_audio() {
                inner.audio_bytes -= ob;
            } else {
                inner.video_bytes -= ob;
            }
            // the old coverage is removed with ITS span (it could have differed)
            if let Some(set) = inner.coverage.get_mut(&(key.source, key.kind)) {
                set.remove_range(key.idx, key.idx + old.span);
            }
        }

        inner.recency_counter += 1;
        let recency = inner.recency_counter;
        let wanted = inner.want_pass;
        // the serial number of the content: never reused, IN THE WHOLE PROCESS, therefore “the
        // same address — other bytes” is always distinguishable (item 409). A GPU engine keys its
        // textures and frames by it alone and outlives any one cache, so a per-cache count gave a
        // new cache the old one's pictures (t1457, t1574, t1607).
        let serial = kept_serial.unwrap_or_else(|| NEXT_SERIAL.fetch_add(1, Ordering::Relaxed));
        inner.map.insert(
            key,
            Entry { payload, serial, recency, pinned: false, holds, had_hit: false, wanted, span },
        );
        if holds > 0 {
            inner.held_bytes += bytes;
        }
        inner.lru.insert(recency, key);
        inner.by_source.entry(key.source).or_default().insert(key);
        if is_audio {
            inner.audio_bytes += bytes;
        } else {
            inner.video_bytes += bytes;
        }
        inner
            .coverage
            .entry((key.source, key.kind))
            .or_default()
            .insert_range(key.idx, key.idx + span);

        self.evict_over_budget(&mut inner, key.trace_id());
        self.report(&inner);
        self.epoch.fetch_add(1, Ordering::AcqRel);
        drop(inner);
        if self.next_armed.load(Ordering::Acquire) && self.next_armed.swap(false, Ordering::AcqRel) {
            let f = self.next_insert.lock().unwrap().take();
            if let Some(f) = f {
                f();
            }
        }
    }

    /// An entry dropped before its first read: thrash while the latest prefetch pass still
    /// wanted it (or it is audio, or nothing ever refreshed — export, the tests of t37), otherwise
    /// abandoned.
    fn count_unread_drop(&self, entry: &Entry, want_pass: u64) {
        // only the video look-ahead is refreshed: an audio burst keeps the old rule
        if entry.wanted == want_pass || entry.payload.is_audio() {
            self.stats.unused_evictions.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.abandoned_evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn evict_over_budget(&self, inner: &mut Inner, by: u64) {
        // Go from the oldest; pinned and held ones are skipped. If everything is — the budget
        // may be exceeded, and this is deliberate (a pin is stronger than the budget).
        let mut cursor = 0u64;
        let video_cap = self.max_video_bytes();
        while inner.audio_bytes > self.config.max_audio_bytes || inner.video_bytes > video_cap {
            let Some((&recency, &key)) = inner.lru.range(cursor..).next() else {
                break;
            };
            cursor = recency + 1;
            let entry = inner.map.get(&key).expect("lru/map sync");
            if entry.pinned || entry.holds > 0 {
                continue;
            }
            let over_audio = inner.audio_bytes > self.config.max_audio_bytes;
            if entry.payload.is_audio() != over_audio {
                continue; // evict from the pool that has been exceeded
            }
            let entry = inner.map.remove(&key).unwrap();
            inner.lru.remove(&recency);
            if let Some(keys) = inner.by_source.get_mut(&key.source) {
                keys.remove(&key);
                if keys.is_empty() {
                    inner.by_source.remove(&key.source);
                }
            }
            let b = entry.payload.bytes();
            if entry.payload.is_audio() {
                inner.audio_bytes -= b;
            } else {
                inner.video_bytes -= b;
            }
            if let Some(set) = inner.coverage.get_mut(&(key.source, key.kind)) {
                set.remove_range(key.idx, key.idx + entry.span);
            }
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
            inner.remember(key, entry.serial);
            trace_evict(&key, entry.had_hit, entry.wanted == inner.want_pass, by);
            if !entry.had_hit {
                self.count_unread_drop(&entry, inner.want_pass);
            }
        }
    }

    /// The idx ranges actually lying in the cache, intersected with [range).
    /// These ARE the “warmth” bars for the UI.
    pub fn coverage(
        &self,
        source: SourceId,
        kind: BurstKind,
        range: std::ops::Range<i64>,
    ) -> Vec<std::ops::Range<i64>> {
        let inner = self.inner.lock().unwrap();
        inner.coverage.get(&(source, kind)).map(|s| s.query(range)).unwrap_or_default()
    }

    /// Pin the entries of the range (the head of the loop region): pinned ones are not evicted.
    /// A pin acts only on entries that already exist; call it after prefetch.
    pub fn pin_range(&self, source: SourceId, kind: BurstKind, range: std::ops::Range<i64>) {
        let mut inner = self.inner.lock().unwrap();
        for idx in range {
            if let Some(e) = inner.map.get_mut(&BurstKey { source, kind, idx }) {
                e.pinned = true;
            }
        }
    }

    pub fn unpin_all(&self) {
        let mut inner = self.inner.lock().unwrap();
        for e in inner.map.values_mut() {
            e.pinned = false;
        }
    }

    /// Hold a video entry if it is here (export-pipeline-determinism D1): a held entry is never
    /// evicted, and each hold needs its own `release_video`. The check and the hold are one step.
    /// For a job that reads ahead of its use: LRU's order is not the order of need, since a read
    /// refreshes entries that will never be needed again.
    pub fn hold_video(&self, source: SourceId, quality: Option<Quality>, idx: i64) -> bool {
        let key = BurstKey { source, kind: BurstKind::video(quality), idx };
        let mut inner = self.inner.lock().unwrap();
        let Some(entry) = inner.map.get_mut(&key) else { return false };
        entry.holds += 1;
        if entry.holds == 1 {
            let bytes = entry.payload.bytes();
            inner.held_bytes += bytes;
        }
        true
    }

    /// `insert_video` that takes a hold in the same step.
    pub fn insert_video_held(&self, source: SourceId, quality: Option<Quality>, idx: i64, frame: Arc<VideoFrame>) {
        self.insert_with_holds(BurstKey { source, kind: BurstKind::video(quality), idx }, Payload::Video(frame), 1, 1);
    }

    /// Give back one hold of each key; the entries go back to the LRU, where they already are.
    pub fn release_video(&self, keys: impl IntoIterator<Item = BurstKey>) {
        let mut inner = self.inner.lock().unwrap();
        for key in keys {
            let Some(entry) = inner.map.get_mut(&key) else { continue };
            debug_assert!(entry.holds > 0, "a release without a hold");
            entry.holds = entry.holds.saturating_sub(1);
            if entry.holds == 0 {
                let bytes = entry.payload.bytes();
                inner.held_bytes -= bytes;
            }
        }
        self.evict_over_budget(&mut inner, 0);
        self.report(&inner);
    }

    /// The bytes of the entries held at least once: each counted once, however many hold it.
    pub fn held_video_bytes(&self) -> usize {
        self.inner.lock().unwrap().held_bytes
    }

    /// Drop all entries of the source (the file changed on disk; the parameters
    /// of the generator were reconfigured — R.53, item 411). It goes by the index
    /// `by_source`, and not by the whole cache: it is called on every edit.
    pub fn invalidate_source(&self, source: SourceId) {
        let mut inner = self.inner.lock().unwrap();
        let keys: Vec<BurstKey> =
            inner.by_source.remove(&source).map(|k| k.into_iter().collect()).unwrap_or_default();
        self.stats.invalidate_scan_steps.fetch_add(keys.len() as u64, Ordering::Relaxed);
        kt_time::trace::ev(kt_time::trace::Ev::CacheInvalidate).a(source.0).b(keys.len() as u64).emit();
        inner.past.retain(|k, _| k.source != source);
        inner.past_order.retain(|k| k.source != source);
        for key in keys {
            let entry = inner.map.remove(&key).unwrap();
            inner.lru.remove(&entry.recency);
            let b = entry.payload.bytes();
            if entry.holds > 0 {
                inner.held_bytes -= b;
            }
            if entry.payload.is_audio() {
                inner.audio_bytes -= b;
            } else {
                inner.video_bytes -= b;
            }
        }
        inner.coverage.retain(|(s, _), _| *s != source);
        self.report(&inner);
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    pub fn audio_bytes(&self) -> usize {
        self.inner.lock().unwrap().audio_bytes
    }
    pub fn video_bytes(&self) -> usize {
        self.inner.lock().unwrap().video_bytes
    }
}

impl Drop for BurstCache {
    /// A cache that asked gives its want back: the next player's cache asks its own.
    /// A want another cache has asked since is left standing.
    fn drop(&mut self) {
        let own = self.video_want.load(Ordering::Relaxed);
        if own > 0 && self.accounts[0].wanted() == own {
            self.accounts[0].want(0);
        }
    }
}

#[cfg(test)]
mod interval_tests {
    use super::IntervalSet;

    #[test]
    fn insert_merges_and_remove_splits() {
        let mut s = IntervalSet::default();
        for i in [1, 3, 2, 7, 8, 0] {
            s.insert_range(i, i + 1);
        }
        assert_eq!(s.query(-10..100), vec![0..4, 7..9]);
        s.remove_range(2, 2 + 1);
        assert_eq!(s.query(-10..100), vec![0..2, 3..4, 7..9]);
        s.remove_range(0, 1);
        s.remove_range(3, 3 + 1);
        assert_eq!(s.query(-10..100), vec![1..2, 7..9]);
        // repeated operations are idempotent
        s.remove_range(0, 1);
        s.insert_range(7, 7 + 1);
        assert_eq!(s.query(-10..100), vec![1..2, 7..9]);
        // the query is clipped by the range
        assert_eq!(s.query(8..100), vec![8..9]);
    }
}

#[cfg(test)]
mod v12_tests {
    use super::*;
    use kt_time::Tick;

    fn frame(bytes: usize) -> Arc<VideoFrame> {
        // w*h*4 == bytes
        let w = 4u32;
        let h = (bytes / 16) as u32;
        Arc::new(VideoFrame {
            pts: Tick(0),
            width: w,
            height: h,
            rgba: vec![0u8; bytes].into_boxed_slice(),
            has_alpha: false,
        })
    }

    /// Test 27 (cache part, item 19): a contains-probe touches neither the LRU nor the epoch.
    #[test]
    fn t27_contains_probe_does_not_touch_lru_or_epoch() {
        let bytes = 1000usize;
        let cache =
            BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: bytes * 3 });
        let src = SourceId(1);
        for idx in [0i64, 1, 2] {
            cache.insert_video(src, None, idx, frame(bytes));
        }
        // an honest read of 0 — the freshest
        let _ = cache.get_video(src, None, 0);
        let epoch_before = cache.epoch();
        // probes 1 and 2 must NOT refresh
        assert!(cache.contains_video(src, None, 1));
        assert!(cache.contains_video(src, None, 2));
        assert_eq!(cache.epoch(), epoch_before, "contains does not change epoch");
        // insert 3 evicts 1 (the oldest by the REAL recency)
        cache.insert_video(src, None, 3, frame(bytes));
        assert!(!cache.contains_video(src, None, 1), "evicted must be idx 1");
        assert!(cache.contains_video(src, None, 0), "honestly read 0 alive");
    }

    /// Test 28 (item 20): video coverage with a span is merged into bars and
    /// honestly gets holes on eviction.
    #[test]
    fn t28_video_coverage_merges_and_eviction_splits() {
        let bytes = 1000usize;
        let cache =
            BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: bytes * 3 });
        let src = SourceId(1);
        let step = 3000i64;
        for idx in [0i64, 3000, 6000] {
            cache.insert_video_span(src, None, idx, step, frame(bytes));
        }
        let cov = cache.coverage(src, BurstKind::Video { quality: None }, 0..9000);
        assert_eq!(cov, vec![0..9000], "bands must merge into one");

        // we bump 0 and 6000 with honest reads, insert 9000 → 3000 is evicted
        let _ = cache.get_video(src, None, 0);
        let _ = cache.get_video(src, None, 6000);
        cache.insert_video_span(src, None, 9000, step, frame(bytes));
        let cov = cache.coverage(src, BurstKind::Video { quality: None }, 0..12000);
        assert_eq!(
            cov,
            vec![0..3000, 6000..12000],
            "eviction must remove exactly its own span (hole at 3000..6000)"
        );
    }

    /// insert_range/remove_range: edge cases of merging/cutting.
    #[test]
    fn t28_interval_set_range_ops() {
        let mut s = IntervalSet::default();
        s.insert_range(0, 10);
        s.insert_range(20, 30);
        s.insert_range(10, 20); // bridge
        assert_eq!(s.query(0..40), vec![0..30]);
        s.remove_range(5, 25);
        assert_eq!(s.query(0..40), vec![0..5, 25..30]);
        s.insert_range(4, 26); // cover both + the hole
        assert_eq!(s.query(0..40), vec![0..30]);
        s.remove_range(0, 30);
        assert!(s.query(0..40).is_empty());
    }
}

#[cfg(test)]
mod postfx_tests {
    use super::*;

    fn vframe(bytes: usize) -> Arc<VideoFrame> {
        Arc::new(VideoFrame {
            pts: kt_time::Tick(0),
            width: 2,
            height: 2,
            rgba: vec![7u8; bytes].into_boxed_slice(),
            has_alpha: false,
        })
    }

}

#[cfg(test)]
mod perf_counter_tests {
    use super::*;
    use kt_time::Tick;

    fn frame(bytes: usize) -> Arc<VideoFrame> {
        Arc::new(VideoFrame {
            pts: Tick(0),
            width: 4,
            height: (bytes / 16) as u32,
            rgba: vec![0u8; bytes].into_boxed_slice(),
            has_alpha: false,
        })
    }

    /// Test 37 (performance panel): no-read drops — entries evicted OR
    /// overwritten before the first read — are counted
    /// by a separate counter (prefetch-thrash, port of hadHit v1);
    /// read entries do not get into it.
    #[test]
    fn t37_unused_evictions_counted_separately() {
        let bytes = 1000usize;
        let cache =
            BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: bytes * 2 });
        let src = SourceId(1);

        // 0 and 1 fit; 0 is read (had_hit), 1 — not
        cache.insert_video(src, None, 0, frame(bytes));
        cache.insert_video(src, None, 1, frame(bytes));
        let _ = cache.get_video(src, None, 0);
        assert_eq!(cache.stats().unused_evictions, 0);

        // insert 2 evicts the oldest UNREAD one (1) → thrash +1
        cache.insert_video(src, None, 2, frame(bytes));
        let s = cache.stats();
        assert_eq!(s.evictions, 1, "budget for 2 frames");
        assert_eq!(s.unused_evictions, 1, "evicted unread");

        // insert 3 evicts the read one (0) → thrash does NOT grow
        cache.insert_video(src, None, 3, frame(bytes));
        let s = cache.stats();
        assert_eq!(s.evictions, 2);
        assert_eq!(s.unused_evictions, 1, "read entry is not thrash");

        // overwriting the unread key 3 with the same key → thrash +1 (v1:
        // “dropped by LRU or overwritten before any read”)
        cache.insert_video(src, None, 3, frame(bytes));
        assert_eq!(cache.stats().unused_evictions, 2, "overwrite before read — thrash");
    }

    /// Test 1417 (video-prefetch-thrash): `refresh` keeps the look-ahead window ahead of
    /// what was read. Plain LRU ranks a prefetched frame by its INSERTION, so the nearest
    /// frame of the window — decoded first — is the oldest and goes first; refreshed from
    /// the furthest to the nearest, the window outlives the read frame, and among the
    /// window the furthest frame goes first. A refresh is not a read: no `had_hit`, no
    /// hit counted, the epoch stays.
    #[test]
    fn t1417_refresh_keeps_the_window_ahead_of_the_read() {
        let bytes = 1000usize;
        let cache =
            BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: bytes * 3 });
        let src = SourceId(1);
        // the window 0..3 is decoded in order, the playhead reads 0
        for i in 0..3 {
            cache.insert_video(src, None, i, frame(bytes));
        }
        let _ = cache.get_video(src, None, 0);
        let (stats, epoch) = (cache.stats(), cache.epoch());
        // the next pass wants 1..4: refreshed far → near, then 3 arrives
        let key = |idx| BurstKey { source: src, kind: BurstKind::video(None), idx };
        cache.refresh([2, 1].map(key));
        cache.refresh([key(99)]); // an absent key is skipped
        assert_eq!(cache.epoch(), epoch, "a refresh does not move the epoch");
        assert_eq!(cache.stats().video_hits, stats.video_hits, "a refresh is not a read");
        cache.insert_video(src, None, 3, frame(bytes));
        assert!(!cache.contains_video(src, None, 0), "the frame already shown goes");
        assert!(cache.contains_video(src, None, 1) && cache.contains_video(src, None, 2));
        assert_eq!(cache.stats().unused_evictions, 0, "nothing unread was dropped");
        // without the refresh the same sequence dropped the window's nearest frame
        let lru =
            BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: bytes * 3 });
        for i in 0..3 {
            lru.insert_video(src, None, i, frame(bytes));
        }
        let _ = lru.get_video(src, None, 0);
        lru.insert_video(src, None, 3, frame(bytes));
        assert!(!lru.contains_video(src, None, 1), "plain LRU: the nearest unread frame goes");
        // among the window the furthest goes first; a refreshed entry is still unread
        cache.refresh([3, 2, 1].map(key));
        cache.insert_video(src, None, 4, frame(bytes));
        assert!(!cache.contains_video(src, None, 3), "the furthest of the window goes first");
        assert_eq!(cache.stats().unused_evictions, 1, "a refresh does not count as a read");
    }

    /// Test 1417b (video-prefetch-thrash): an entry dropped unread is THRASH only while the
    /// latest prefetch pass still wanted it. One the prefetch had stopped asking for — the
    /// window a seek left, a frame the picture skipped — is counted as abandoned, and the
    /// performance panel's "prefetch thrash" no longer reports it. Audio is never refreshed
    /// and keeps the old rule; so does a cache nobody refreshes (t37).
    #[test]
    fn t1417b_abandoned_is_not_thrash() {
        let bytes = 1000usize;
        let cache =
            BurstCache::new(CacheConfig { max_audio_bytes: 8192, max_video_bytes: bytes * 2 });
        let src = SourceId(1);
        let key = |idx| BurstKey { source: src, kind: BurstKind::video(None), idx };
        // a window 0..2 is decoded and wanted; then a seek: the next pass wants 10..12
        cache.insert_video(src, None, 0, frame(bytes));
        cache.insert_video(src, None, 1, frame(bytes));
        cache.refresh([1, 0].map(key));
        cache.refresh([11, 10].map(key));
        cache.insert_video(src, None, 10, frame(bytes));
        cache.insert_video(src, None, 11, frame(bytes));
        let s = cache.stats();
        assert_eq!(s.evictions, 2, "the left window went");
        assert_eq!(s.unused_evictions, 0, "a window the seek left is not thrash");
        assert_eq!(s.abandoned_evictions, 2);
        // the window still wanted IS thrash when it is dropped unread
        cache.refresh([11, 10].map(key));
        cache.insert_video(src, None, 12, frame(bytes));
        let s = cache.stats();
        assert_eq!((s.unused_evictions, s.abandoned_evictions), (1, 2), "wanted and dropped");
        // so is an entry inserted after the pass that asked for it
        cache.insert_video(src, None, 13, frame(bytes));
        assert_eq!(cache.stats().unused_evictions, 2, "inserted since the pass: still wanted");
        // audio keeps the old rule whatever the passes
        cache.insert_audio(src, Arc::new(kt_media::AudioBurst::silence(0)));
        cache.refresh([key(13)]);
        cache.insert_audio(src, Arc::new(kt_media::AudioBurst::silence(1)));
        let s = cache.stats();
        assert_eq!((s.unused_evictions, s.abandoned_evictions), (3, 2), "audio unread is thrash");
    }

    /// Test 497 (R.53, item 409): `invalidate_source` goes by the index of ITS
    /// source, and not by the whole cache. It is now called on every edit of the
    /// generator’s parameters, and a linear pass over other entries would be a price
    /// growing with the warmth of the edit.
    #[test]
    fn t497_invalidate_source_scans_only_its_own() {
        let cache =
            BurstCache::new(CacheConfig { max_audio_bytes: 1 << 24, max_video_bytes: 1 << 24 });
        let mine = SourceId(7);
        for i in 0..300 {
            cache.insert_video(SourceId(100 + i as u64), None, i, frame(64));
        }
        for i in 0..3 {
            cache.insert_video(mine, None, i, frame(64));
        }
        assert_eq!(cache.stats().invalidate_scan_steps, 0);

        cache.invalidate_source(mine);
        let s = cache.stats();
        assert!(
            s.invalidate_scan_steps <= 8,
            "cleanup swept other entries: {} steps",
            s.invalidate_scan_steps
        );
        for i in 0..3 {
            assert!(cache.get_video(mine, None, i).is_none(), "own entries must go");
        }
        for i in 0..300 {
            assert!(
                cache.get_video(SourceId(100 + i as u64), None, i).is_some(),
                "foreign entry {i} was harmed"
            );
        }
    }

    /// t1574 (compute-kernels-render): the serial numbers of two caches differ, so a frame of one
    /// cannot pass for a frame of the other in an engine that serves both.
    #[test]
    fn t1574_two_caches_share_no_serial() {
        let (a, b) = (BurstCache::new(CacheConfig::default()), BurstCache::new(CacheConfig::default()));
        let f = || Arc::new(VideoFrame { pts: kt_time::Tick(0), width: 1, height: 1, rgba: vec![0; 4].into_boxed_slice(), has_alpha: false });
        let id = SourceId(1);
        a.insert_video(id, None, 0, f());
        b.insert_video(id, None, 0, f());
        assert_ne!(a.get_video_stamped(id, None, 0).unwrap().1, b.get_video_stamped(id, None, 0).unwrap().1);
    }

    /// Test 496a (R.53, item 409): the serial number of the content. The SAME ADDRESS
    /// with other bytes must receive another number — by it kt-gpu
    /// tells “a new frame of the generator” from “the same frame” (otherwise on the
    /// GPU path a color edit is not visible: t496 in kt-gpu).
    #[test]
    fn t496a_content_serial_changes_with_bytes() {
        let cache =
            BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: 1 << 20 });
        let src = SourceId(1);
        cache.insert_video(src, None, 0, frame(64));
        let (_, first) = cache.get_video_stamped(src, None, 0).expect("frame is stored");
        let (_, again) = cache.get_video_stamped(src, None, 0).expect("frame is stored");
        assert_eq!(first, again, "reading does not change the content number");

        // an overwrite of the same address with other bytes — the number must become different
        cache.insert_video(src, None, 0, frame(64));
        let (_, second) = cache.get_video_stamped(src, None, 0).expect("frame is stored");
        assert_ne!(first, second, "same address, different bytes — the number must differ");
        assert!(second > first, "numbers are monotonic and not reused");
    }

    /// t1607: a content serial means one content in the whole process, not in one cache. A GPU
    /// engine keys its textures by the serial alone: with a count per cache, a new cache on a used
    /// engine got the old cache's pictures (t1457 in kt-gpu).
    #[test]
    fn t1607_two_caches_never_share_a_content_serial() {
        let config = CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: 1 << 20 };
        let (a, b) = (BurstCache::new(config), BurstCache::new(config));
        let src = SourceId(1);
        let mut seen = HashSet::new();
        for idx in 0..50 {
            a.insert_video(src, None, idx, frame(8));
            b.insert_video(src, None, idx, frame(8));
            for cache in [&a, &b] {
                let (_, serial) = cache.get_video_stamped(src, None, idx).expect("just stored");
                assert!(seen.insert(serial), "the serial {serial} names two contents");
            }
        }
    }

    /// t1600 (export-pipeline-determinism): a held entry outlives any pressure, however fresh the
    /// entries read around it, and goes when the last hold is released.
    #[test]
    fn t1600_a_held_entry_outlives_the_pressure_and_goes_when_released() {
        let bytes = 64usize;
        let cache =
            BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: bytes * 3 });
        let src = SourceId(1);
        let key = |idx| BurstKey { source: src, kind: BurstKind::video(None), idx };
        assert!(!cache.hold_video(src, None, 0), "an absent key answers “not present”");
        assert_eq!(cache.held_video_bytes(), 0, "and holds nothing");
        cache.release_video([key(0)]);

        cache.insert_video_held(src, None, 0, frame(bytes));
        assert!(cache.hold_video(src, None, 0), "a second user of the same frame");
        assert_eq!(cache.held_video_bytes(), bytes, "counted once, however many hold it");
        // the pressure of an export: fresh inserts, and reads that refresh what is never needed again
        for idx in 1..20 {
            cache.insert_video(src, None, idx, frame(bytes));
            assert!(cache.get_video(src, None, idx).is_some());
        }
        assert!(cache.contains_video(src, None, 0), "evicted while held");
        assert!(!cache.contains_video(src, None, 1), "the budget still governs the rest");
        assert_eq!(cache.video_bytes(), bytes * 3, "held entries are inside the budget's count");

        cache.insert_video(src, None, 0, frame(bytes));
        assert_eq!(cache.held_video_bytes(), bytes, "an overwrite keeps the holds");
        cache.release_video([key(0)]);
        cache.insert_video(src, None, 30, frame(bytes));
        assert!(cache.contains_video(src, None, 0), "one hold is left");
        assert_eq!(cache.held_video_bytes(), bytes);
        cache.release_video([key(0)]);
        assert_eq!(cache.held_video_bytes(), 0, "the last hold gone");
        for idx in 31..34 {
            cache.insert_video(src, None, idx, frame(bytes));
        }
        assert!(!cache.contains_video(src, None, 0), "released, it is an entry like any other");

        // a hold over the budget: the budget gives way, and the release collects it
        for idx in 40..46 {
            cache.insert_video_held(src, None, idx, frame(bytes));
        }
        assert_eq!(cache.video_bytes(), bytes * 6, "a hold is stronger than the budget");
        cache.release_video((40..46).map(key));
        assert_eq!(cache.video_bytes(), bytes * 3, "the release brings it back under the budget");
    }

    /// Test t1836 (idle-cpu): a waiter hears the next insertion, once. A paused preview missing a
    /// frame waits on this instead of a clock.
    #[test]
    fn t1836_the_next_insert_is_heard_once() {
        use std::sync::atomic::AtomicU32;
        let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: 1 << 20 });
        let heard = Arc::new(AtomicU32::new(0));
        let h = heard.clone();
        cache.on_next_insert(Box::new(move || {
            h.fetch_add(1, Ordering::Relaxed);
        }));
        assert_eq!(heard.load(Ordering::Relaxed), 0, "nothing arrived yet");
        let burst = || Arc::new(AudioBurst::silence(0));
        cache.insert_audio(SourceId(1), burst());
        cache.insert_audio(SourceId(1), burst());
        assert_eq!(heard.load(Ordering::Relaxed), 1, "one waiter, one call");
    }
}

#[cfg(test)]
mod serial_tests {
    use super::*;

    fn frame() -> Arc<VideoFrame> {
        Arc::new(VideoFrame { pts: kt_time::Tick(0), width: 4, height: 4, rgba: vec![1u8; 64].into_boxed_slice(), has_alpha: false })
    }

    /// Test t2152 (structured-telemetry): an entry evicted and decoded again from an unchanged source
    /// takes back its content serial (what the executor's kept composites are keyed by); after the
    /// source is invalidated the same address gets a new one, and an overwrite of a present entry
    /// is new bytes with a new serial.
    #[test]
    fn t2152_a_redecoded_entry_keeps_its_serial() {
        let c = BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: 64 * 2 });
        let (s, q) = (SourceId(5), Some(Quality { width: 4, height: 4 }));
        c.insert_video(s, q, 0, frame());
        let first = c.get_video_stamped(s, q, 0).unwrap().1;
        c.insert_video(s, q, 1, frame());
        c.insert_video(s, q, 2, frame());
        assert!(!c.contains_video(s, q, 0), "scene guard: the budget evicted the first entry");
        c.insert_video(s, q, 0, frame());
        assert_eq!(c.get_video_stamped(s, q, 0).unwrap().1, first, "decoded again: the same serial");
        c.insert_video(s, q, 0, frame());
        let over = c.get_video_stamped(s, q, 0).unwrap().1;
        assert_ne!(over, first, "an overwrite is new bytes");
        c.insert_video(s, q, 1, frame());
        c.insert_video(s, q, 2, frame());
        c.invalidate_source(s);
        c.insert_video(s, q, 0, frame());
        let after = c.get_video_stamped(s, q, 0).unwrap().1;
        assert!(after != first && after != over, "after the source changed the address is new bytes");
    }
}

#[cfg(test)]
mod growth_tests {
    use super::*;
    use kt_time::Tick;

    fn frame(bytes: usize) -> Arc<VideoFrame> {
        Arc::new(VideoFrame {
            pts: Tick(0),
            width: 4,
            height: (bytes / 16) as u32,
            rgba: vec![0u8; bytes].into_boxed_slice(),
            has_alpha: false,
        })
    }

    fn ledger_with(available: u64) -> &'static kt_time::mem::Ledger {
        let l: &'static kt_time::mem::Ledger = Box::leak(Box::new(kt_time::mem::Ledger::new()));
        l.set_probe(move || Some(available));
        l
    }

    /// t2390 (frames-cache-growth, found-defects 396): with memory to spare the player's cache grows
    /// past its base for a loop whose frames exceed it, and a lap evicts nothing; a claim (an export)
    /// takes the growth back at once — evicting down to the base, holds kept — and its end gives it
    /// back; a lower want (a smaller preview quality) shrinks the cap in the asking call.
    #[test]
    fn t2390_the_cache_grows_for_its_loop_and_gives_back_to_a_claim() {
        let bytes = 1usize << 20;
        let base = 16 * bytes;
        let l = ledger_with(1 << 30); // 512 MB of growth
        let cache = Arc::new(BurstCache::with_ledger(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: base }, l));
        let src = SourceId(1);
        let lap = |c: &BurstCache| {
            for idx in 0..40 {
                if c.get_video(src, None, idx).is_none() {
                    c.insert_video(src, None, idx, frame(bytes));
                }
            }
        };
        // the base alone: a loop of 40 frames over 16 evicts on every lap
        lap(&cache);
        let before = cache.stats().evictions;
        lap(&cache);
        assert!(cache.stats().evictions - before >= 24, "the base cannot hold the loop: the test's premise");

        let want = 40 * bytes + 40 * bytes / 8;
        let cap = cache.want_video(want);
        assert_eq!(cap, want, "granted in full: the memory is there");
        assert_eq!(cache.max_video_bytes(), want, "the readers of the budget read the cap");
        assert_eq!(cache.video_cap_reader()(), want);
        lap(&cache);
        let settled = cache.stats().evictions;
        lap(&cache);
        lap(&cache);
        assert_eq!(cache.stats().evictions, settled, "a lap of a loop that fits evicts nothing");

        // an export's claim: the growth goes at once, evicted down to the base; a hold stays
        assert!(cache.hold_video(src, None, 39));
        let claim = l.claim();
        assert_eq!(cache.max_video_bytes(), base, "the claim took the growth back");
        assert!(cache.video_bytes() <= base, "evicted at once, not at the next insert: {}", cache.video_bytes());
        assert!(cache.contains_video(src, None, 39), "a hold is stronger than any cap");
        drop(claim);
        assert_eq!(cache.max_video_bytes(), want, "the claim gone, the grant is back");
        cache.release_video([BurstKey { source: src, kind: BurstKind::video(None), idx: 39 }]);

        // a smaller want (a lower preview quality): the cap and the bytes follow in the call
        lap(&cache);
        assert_eq!(cache.want_video(20 * bytes), 20 * bytes);
        assert!(cache.video_bytes() <= 20 * bytes, "the asker evicts itself: no hook tells it");
        // below the base nothing is asked, and the base is never cut
        assert_eq!(cache.want_video(bytes), base);
    }

    /// t2391 (frames-cache-growth): a cache that never asks keeps its base whatever the kind's grant
    /// (an export's, a reel strip's, a poster's cache beside the player's).
    #[test]
    fn t2391_a_cache_that_never_asks_stays_at_its_base() {
        let l = ledger_with(1 << 30);
        let player = Arc::new(BurstCache::with_ledger(CacheConfig { max_audio_bytes: 1, max_video_bytes: 1 << 20 }, l));
        let job = BurstCache::with_ledger(CacheConfig { max_audio_bytes: 1, max_video_bytes: 1 << 20 }, l);
        assert_eq!(player.want_video(64 << 20), 64 << 20);
        assert_eq!(job.max_video_bytes(), 1 << 20, "the kind's grant is the asker's alone");
        for idx in 0..4 {
            job.insert_video(SourceId(2), None, idx, frame(1 << 20));
        }
        assert_eq!(job.video_bytes(), 1 << 20, "it evicts against its base");
    }

    /// t2392 (frames-cache-growth): a dropped cache gives its want back, unless another cache has
    /// asked since (a project reopened: the new player's cache asks before the old one goes); unknown
    /// memory grants nothing.
    #[test]
    fn t2392_a_dropped_cache_gives_its_want_back() {
        let l = ledger_with(1 << 30);
        let cfg = CacheConfig { max_audio_bytes: 1, max_video_bytes: 1 << 20 };
        let old = Arc::new(BurstCache::with_ledger(cfg, l));
        old.want_video(64 << 20);
        drop(old);
        assert_eq!(l.wanted(kt_time::mem::Tenant::Frames), 0, "the want went with the cache");
        let old = Arc::new(BurstCache::with_ledger(cfg, l));
        old.want_video(64 << 20);
        let new = Arc::new(BurstCache::with_ledger(cfg, l));
        new.want_video(32 << 20);
        drop(old);
        assert_eq!(l.wanted(kt_time::mem::Tenant::Frames), 31 << 20, "the newer want stands");
        assert_eq!(new.max_video_bytes(), 32 << 20);

        let blind: &'static kt_time::mem::Ledger = Box::leak(Box::new(kt_time::mem::Ledger::new()));
        let c = Arc::new(BurstCache::with_ledger(cfg, blind));
        assert_eq!(c.want_video(64 << 20), 1 << 20, "unknown memory: the base");
    }
}
