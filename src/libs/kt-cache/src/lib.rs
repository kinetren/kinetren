//! Кэш burst'ов. Единственная правда о «прогретости» (инвариант №4):
//! coverage для UI — это запрос к самому кэшу, никакой отдельной бухгалтерии
//! (старые ms-бакеты — дефект g в docs/porting-notes.md — врали четырьмя
//! способами; здесь врать структурно нечему: interval-set обновляется в тех же
//! операциях, что и сам кэш).
//!
//! Ключ: (source_id, kind{Audio | Video{quality}}, idx). Quality — часть ключа:
//! записи разных качеств не взаимозаменяемы.

use kt_media::{AudioBurst, Quality, VideoFrame};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BurstKind {
    Audio,
    /// None = полное разрешение источника.
    Video {
        quality: Option<(u32, u32)>,
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
    recency: u64,
    pinned: bool,
    /// Был ли хоть один get после вставки — eviction непрочитанной записи
    /// это prefetch-thrash (перф-панель: «no-read drops», порт v1 hadHit).
    had_hit: bool,
    /// Ширина покрытия записи в idx-единицах (аудио = 1 burst; видео = шаг
    /// сетки кадров в тиках — coverage сливается в честные полосы, п.20).
    span: i64,
}

/// Набор непересекающихся полуоткрытых диапазонов [start, end) по idx.
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
        // сосед слева (может содержать или примыкать)
        if let Some((&s, &e)) = self.runs.range(..=idx).next_back()
            && e >= start
        {
            if e >= end {
                return; // уже покрыт
            }
            start = s;
            end = end.max(e);
            self.runs.remove(&s);
        }
        // все runs, начинающиеся внутри/на границе нового диапазона
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
        // run слева, накрывающий начало
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
        // runs, начинающиеся внутри вырезаемого диапазона
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
        // первый run может начинаться до range.start
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
    // recency -> key; уникальность recency гарантируется монотонным счётчиком
    lru: BTreeMap<u64, BurstKey>,
    coverage: HashMap<(SourceId, BurstKind), IntervalSet>,
    audio_bytes: usize,
    video_bytes: usize,
    recency_counter: u64,
}

/// Счётчики для перф-панели (кумулятивные; UI считает дельты).
#[derive(Debug, Default, Clone, Copy)]
pub struct CacheStats {
    pub audio_hits: u64,
    pub audio_misses: u64,
    pub video_hits: u64,
    pub video_misses: u64,
    pub evictions: u64,
    /// Записи, выселенные/перезаписанные ДО первого чтения (prefetch-thrash).
    pub unused_evictions: u64,
}

#[derive(Default)]
struct StatCounters {
    audio_hits: AtomicU64,
    audio_misses: AtomicU64,
    video_hits: AtomicU64,
    video_misses: AtomicU64,
    evictions: AtomicU64,
    unused_evictions: AtomicU64,
}

/// Потокобезопасный кэш burst'ов с LRU по раздельным байтовым бюджетам
/// аудио/видео, pin'ом (loop-регион) и coverage-запросами.
pub struct BurstCache {
    config: CacheConfig,
    inner: Mutex<Inner>,
    /// Меняется при insert/evict/invalidate — UI перерисовывает полоски
    /// только при изменении.
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
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    /// Проба наличия БЕЗ бампа recency и статистики: для планировщика и
    /// префетча (п.19: get-пробы в возрастающем порядке инвертировали LRU —
    /// ближние к playhead кадры становились «самыми старыми» и выселялись
    /// первыми).
    pub fn contains(&self, key: BurstKey) -> bool {
        self.inner.lock().unwrap().map.contains_key(&key)
    }

    pub fn contains_audio(&self, source: SourceId, idx: i64) -> bool {
        self.contains(BurstKey { source, kind: BurstKind::Audio, idx })
    }

    pub fn contains_video(&self, source: SourceId, quality: Option<Quality>, idx: i64) -> bool {
        self.contains(BurstKey { source, kind: BurstKind::video(quality), idx })
    }

    /// Видео-бюджет (для бюджетно-осознанного lookahead префетча).
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
        match self.get(BurstKey { source, kind: BurstKind::video(quality), idx })? {
            Payload::Video(f) => Some(f),
            Payload::Audio(_) => unreachable!("video key holds audio payload"),
        }
    }

    pub fn get(&self, key: BurstKey) -> Option<Payload> {
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
        inner.lru.remove(&old);
        inner.lru.insert(counter, key);
        Some(payload)
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

    /// Видео со span'ом покрытия (шаг сетки кадров, тики): coverage
    /// сливается в честные полосы вместо 1-тиковых точек (п.20).
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
        let mut inner = self.inner.lock().unwrap();
        let bytes = payload.bytes();
        let is_audio = payload.is_audio();
        debug_assert_eq!(is_audio, key.kind == BurstKind::Audio);

        if let Some(old) = inner.map.remove(&key) {
            if !old.had_hit {
                // перезапись непрочитанной записи — тоже thrash (v1)
                self.stats.unused_evictions.fetch_add(1, Ordering::Relaxed);
            }
            inner.lru.remove(&old.recency);
            let ob = old.payload.bytes();
            if old.payload.is_audio() {
                inner.audio_bytes -= ob;
            } else {
                inner.video_bytes -= ob;
            }
            // старое покрытие снимаем со СВОИМ span'ом (мог отличаться)
            if let Some(set) = inner.coverage.get_mut(&(key.source, key.kind)) {
                set.remove_range(key.idx, key.idx + old.span);
            }
        }

        inner.recency_counter += 1;
        let recency = inner.recency_counter;
        inner.map.insert(key, Entry { payload, recency, pinned: false, had_hit: false, span });
        inner.lru.insert(recency, key);
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
        // Идём от самых старых; pinned пропускаем. Если всё pinned — бюджет
        // может быть превышен, это осознанно (pin сильнее бюджета).
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
                continue; // выселяем из того пула, который превышен
            }
            let entry = inner.map.remove(&key).unwrap();
            inner.lru.remove(&recency);
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

    /// Диапазоны idx, реально лежащие в кэше, пересечённые с [range).
    /// Это И ЕСТЬ полоски «прогретости» для UI.
    pub fn coverage(
        &self,
        source: SourceId,
        kind: BurstKind,
        range: std::ops::Range<i64>,
    ) -> Vec<std::ops::Range<i64>> {
        let inner = self.inner.lock().unwrap();
        inner.coverage.get(&(source, kind)).map(|s| s.query(range)).unwrap_or_default()
    }

    /// Закрепить записи диапазона (голова loop-региона): pinned не выселяются.
    /// Pin действует только на уже существующие записи; вызывать после prefetch.
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

    /// Сбросить все записи источника (файл изменился на диске).
    pub fn invalidate_source(&self, source: SourceId) {
        let mut inner = self.inner.lock().unwrap();
        let keys: Vec<BurstKey> =
            inner.map.keys().filter(|k| k.source == source).copied().collect();
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
        // повторные операции идемпотентны
        s.remove_range(0, 1);
        s.insert_range(7, 7 + 1);
        assert_eq!(s.query(-10..100), vec![1..2, 7..9]);
        // запрос обрезается диапазоном
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

    /// Тест 27 (кэш-часть, п.19): contains-проба не трогает ни LRU, ни epoch.
    #[test]
    fn t27_contains_probe_does_not_touch_lru_or_epoch() {
        let bytes = 1000usize;
        let cache =
            BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: bytes * 3 });
        let src = SourceId(1);
        for idx in [0i64, 1, 2] {
            cache.insert_video(src, None, idx, frame(bytes));
        }
        // честное чтение 0 — самый свежий
        let _ = cache.get_video(src, None, 0);
        let epoch_before = cache.epoch();
        // пробы 1 и 2 НЕ должны освежать
        assert!(cache.contains_video(src, None, 1));
        assert!(cache.contains_video(src, None, 2));
        assert_eq!(cache.epoch(), epoch_before, "contains не меняет epoch");
        // вставка 3 выселяет 1 (старейший по НАСТОЯЩЕЙ recency)
        cache.insert_video(src, None, 3, frame(bytes));
        assert!(!cache.contains_video(src, None, 1), "выселен должен быть idx 1");
        assert!(cache.contains_video(src, None, 0), "честно прочитанный 0 жив");
    }

    /// Тест 28 (п.20): видео-coverage со span'ом сливается в полосы и
    /// честно дырявится при eviction.
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
        assert_eq!(cov, vec![0..9000], "полосы должны слиться в одну");

        // бампим 0 и 6000 честным чтением, вставляем 9000 → выселяется 3000
        let _ = cache.get_video(src, None, 0);
        let _ = cache.get_video(src, None, 6000);
        cache.insert_video_span(src, None, 9000, step, frame(bytes));
        let cov = cache.coverage(src, BurstKind::Video { quality: None }, 0..12000);
        assert_eq!(
            cov,
            vec![0..3000, 6000..12000],
            "eviction обязан выбить ровно свой span (дыра на 3000..6000)"
        );
    }

    /// insert_range/remove_range: граничные случаи слияния/разрезания.
    #[test]
    fn t28_interval_set_range_ops() {
        let mut s = IntervalSet::default();
        s.insert_range(0, 10);
        s.insert_range(20, 30);
        s.insert_range(10, 20); // мост
        assert_eq!(s.query(0..40), vec![0..30]);
        s.remove_range(5, 25);
        assert_eq!(s.query(0..40), vec![0..5, 25..30]);
        s.insert_range(4, 26); // накрыть обе + дыру
        assert_eq!(s.query(0..40), vec![0..30]);
        s.remove_range(0, 30);
        assert!(s.query(0..40).is_empty());
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

    /// Тест 37 (перф-панель): no-read drops — записи, выселенные ИЛИ
    /// перезаписанные до первого чтения, считаются отдельным счётчиком
    /// (prefetch-thrash, порт hadHit v1); прочитанные записи в него
    /// не попадают.
    #[test]
    fn t37_unused_evictions_counted_separately() {
        let bytes = 1000usize;
        let cache =
            BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: bytes * 2 });
        let src = SourceId(1);

        // 0 и 1 влезают; 0 читаем (had_hit), 1 — нет
        cache.insert_video(src, None, 0, frame(bytes));
        cache.insert_video(src, None, 1, frame(bytes));
        let _ = cache.get_video(src, None, 0);
        assert_eq!(cache.stats().unused_evictions, 0);

        // вставка 2 выселяет самого старого НЕчитанного (1) → thrash +1
        cache.insert_video(src, None, 2, frame(bytes));
        let s = cache.stats();
        assert_eq!(s.evictions, 1, "бюджет на 2 кадра");
        assert_eq!(s.unused_evictions, 1, "выселен непрочитанный");

        // вставка 3 выселяет прочитанного (0) → thrash НЕ растёт
        cache.insert_video(src, None, 3, frame(bytes));
        let s = cache.stats();
        assert_eq!(s.evictions, 2);
        assert_eq!(s.unused_evictions, 1, "прочитанная запись — не thrash");

        // перезапись непрочитанного ключа 3 тем же ключом → thrash +1 (v1:
        // «dropped by LRU or overwritten before any read»)
        cache.insert_video(src, None, 3, frame(bytes));
        assert_eq!(cache.stats().unused_evictions, 2, "overwrite до чтения — thrash");
    }
}
