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
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// Post-FX composite layer of the clip (memoization, item 2 round 7): the idx of the key is
    /// a TIMELINE tick on the frame grid (vhs/noise depend on t, pan/cam — on the
    /// position in the clip; both are functions of (idx, fx_hash)). fx_hash
    /// includes the parameters of the chain and the geometry of the clip. It does NOT get into the coverage bars,
    /// prefetch and pin: all these queries are kind-exact (invariant
    /// 4 remains about PRE-FX data; see kt-cache/CLAUDE.md).
    PostFxVideo {
        quality: (u32, u32),
        fx_hash: u64,
    },
}

impl BurstKind {
    pub fn video(q: Option<Quality>) -> BurstKind {
        BurstKind::Video { quality: q.map(|q| (q.width, q.height)) }
    }
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

struct Entry {
    payload: Payload,
    /// The serial number of the CONTENT (R.53, item 409): monotonic, never
    /// reused. By it consumers (the texture upload into kt-gpu,
    /// the composite memo) tell apart “the same address, other bytes” — the addresses
    /// `(source, time, size)` are too few for this: a generator with new
    /// parameters and a file that came back in place of a placeholder give the SAME address
    /// and a different picture (t496, t211).
    serial: u64,
    recency: u64,
    pinned: bool,
    /// Was there at least one get after the insertion — eviction of an unread entry
    /// is prefetch-thrash (the performance panel: “no-read drops”, port of v1 hadHit).
    had_hit: bool,
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
    recency_counter: u64,
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
}

impl BurstCache {
    pub fn new(config: CacheConfig) -> BurstCache {
        BurstCache {
            config,
            inner: Mutex::new(Inner {
                map: HashMap::new(),
                lru: BTreeMap::new(),
                by_source: HashMap::new(),
                coverage: HashMap::new(),
                audio_bytes: 0,
                video_bytes: 0,
                recency_counter: 0,
            }),
            epoch: AtomicU64::new(0),
            stats: StatCounters::default(),
        }
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

    /// The video budget (for the budget-aware lookahead of prefetch).
    pub fn max_video_bytes(&self) -> usize {
        self.config.max_video_bytes
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
            match is_audio {
                true => self.stats.audio_misses.fetch_add(1, Ordering::Relaxed),
                false => self.stats.video_misses.fetch_add(1, Ordering::Relaxed),
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

    /// A post-FX frame of the clip (item 2 round 7): idx = the timeline tick on the grid.
    pub fn get_postfx_video(
        &self,
        source: SourceId,
        quality: (u32, u32),
        fx_hash: u64,
        idx: i64,
    ) -> Option<Arc<VideoFrame>> {
        match self.get(BurstKey {
            source,
            kind: BurstKind::PostFxVideo { quality, fx_hash },
            idx,
        })? {
            Payload::Video(f) => Some(f),
            Payload::Audio(_) => unreachable!("postfx key holds audio payload"),
        }
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

    pub fn insert_postfx_video(
        &self,
        source: SourceId,
        quality: (u32, u32),
        fx_hash: u64,
        idx: i64,
        frame: Arc<VideoFrame>,
    ) {
        self.insert(
            BurstKey { source, kind: BurstKind::PostFxVideo { quality, fx_hash }, idx },
            Payload::Video(frame),
        );
    }

    pub fn insert(&self, key: BurstKey, payload: Payload) {
        self.insert_spanned(key, payload, 1);
    }

    fn insert_spanned(&self, key: BurstKey, payload: Payload, span: i64) {
        let mut inner = self.inner.lock().unwrap();
        let bytes = payload.bytes();
        let is_audio = payload.is_audio();
        debug_assert_eq!(is_audio, key.kind == BurstKind::Audio);

        if let Some(old) = inner.map.remove(&key) {
            if !old.had_hit {
                // overwriting an unread entry is also thrash (v1)
                self.stats.unused_evictions.fetch_add(1, Ordering::Relaxed);
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
        // the serial number of the content is taken from the same monotonic
        // counter: it is not reused, therefore “the same address — other
        // bytes” is always distinguishable (item 409)
        inner.recency_counter += 1;
        let serial = inner.recency_counter;
        inner
            .map
            .insert(key, Entry { payload, serial, recency, pinned: false, had_hit: false, span });
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

        self.evict_over_budget(&mut inner);
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    fn evict_over_budget(&self, inner: &mut Inner) {
        // Go from the oldest; pinned ones are skipped. If everything is pinned — the budget
        // may be exceeded, and this is deliberate (a pin is stronger than the budget).
        let mut cursor = 0u64;
        while inner.audio_bytes > self.config.max_audio_bytes
            || inner.video_bytes > self.config.max_video_bytes
        {
            let Some((&recency, &key)) = inner.lru.range(cursor..).next() else {
                break;
            };
            cursor = recency + 1;
            let entry = inner.map.get(&key).expect("lru/map sync");
            if entry.pinned {
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
            if !entry.had_hit {
                self.stats.unused_evictions.fetch_add(1, Ordering::Relaxed);
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

    /// Drop all entries of the source (the file changed on disk; the parameters
    /// of the generator were reconfigured — R.53, item 411). It goes by the index
    /// `by_source`, and not by the whole cache: it is called on every edit.
    pub fn invalidate_source(&self, source: SourceId) {
        let mut inner = self.inner.lock().unwrap();
        let keys: Vec<BurstKey> =
            inner.by_source.remove(&source).map(|k| k.into_iter().collect()).unwrap_or_default();
        self.stats.invalidate_scan_steps.fetch_add(keys.len() as u64, Ordering::Relaxed);
        for key in keys {
            let entry = inner.map.remove(&key).unwrap();
            inner.lru.remove(&entry.recency);
            let b = entry.payload.bytes();
            if entry.payload.is_audio() {
                inner.audio_bytes -= b;
            } else {
                inner.video_bytes -= b;
            }
        }
        inner.coverage.retain(|(s, _), _| *s != source);
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    pub fn audio_bytes(&self) -> usize {
        self.inner.lock().unwrap().audio_bytes
    }
    pub fn video_bytes(&self) -> usize {
        self.inner.lock().unwrap().video_bytes
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

    /// Test 65 (item 2 round 7): the post-FX kind is invisible to coverage (invariant 4 —
    /// the warmth bars are built ONLY from pre-FX entries), but it takes part
    /// in the video budget/LRU and is knocked out by invalidate_source.
    #[test]
    fn t65_postfx_kind_invisible_to_coverage() {
        let cache = BurstCache::new(CacheConfig::default());
        let src = SourceId(1);
        let q = Quality { width: 2, height: 2 };
        cache.insert_video_span(src, Some(q), 0, 3000, vframe(16));
        let before = cache.coverage(src, BurstKind::video(Some(q)), 0..10_000);
        let bytes_before = cache.video_bytes();

        cache.insert_postfx_video(src, (2, 2), 0xDEAD, 0, vframe(16));
        cache.insert_postfx_video(src, (2, 2), 0xDEAD, 3000, vframe(16));
        // the pre-FX coverage did not change
        assert_eq!(cache.coverage(src, BurstKind::video(Some(q)), 0..10_000), before);
        assert!(cache.coverage(src, BurstKind::Audio, 0..10_000).is_empty());
        // the bytes are counted in the video pool
        assert_eq!(cache.video_bytes(), bytes_before + 32);
        // a hit by the exact key; another fx_hash/quality — a miss
        assert!(cache.get_postfx_video(src, (2, 2), 0xDEAD, 0).is_some());
        assert!(cache.get_postfx_video(src, (2, 2), 0xBEEF, 0).is_none());
        assert!(cache.get_postfx_video(src, (4, 4), 0xDEAD, 0).is_none());
        // invalidate_source knocks out post-FX entries as well
        cache.invalidate_source(src);
        assert!(cache.get_postfx_video(src, (2, 2), 0xDEAD, 0).is_none());
        assert_eq!(cache.video_bytes(), 0);
    }

    /// Post-FX entries are displaced by LRU when the video budget overflows,
    /// without punching a hole in the pre-FX bar (it has its own kind).
    #[test]
    fn t65b_postfx_evicts_under_budget() {
        let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: 64 });
        let src = SourceId(1);
        let q = Quality { width: 2, height: 2 };
        cache.insert_video_span(src, Some(q), 0, 3000, vframe(32));
        let pre = cache.coverage(src, BurstKind::video(Some(q)), 0..10_000);
        for i in 0..8 {
            cache.insert_postfx_video(src, (2, 2), 1, i * 3000, vframe(32));
            // a pre-FX entry is hot (as in live compose: a get bumps the
            // recency) — only post-FX entries should be displaced
            assert!(cache.get_video(src, Some(q), 0).is_some());
        }
        assert!(cache.video_bytes() <= 64 + 32, "budget holds");
        assert_eq!(
            cache.coverage(src, BurstKind::video(Some(q)), 0..10_000),
            pre,
            "pre-FX band is not punched by post-FX eviction"
        );
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
}
