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
}

/// Набор непересекающихся полуоткрытых диапазонов [start, end) по idx.
#[derive(Default, Debug, Clone)]
struct IntervalSet {
    runs: BTreeMap<i64, i64>, // start -> end
}

impl IntervalSet {
    fn insert(&mut self, idx: i64) {
        let (mut start, mut end) = (idx, idx + 1);
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
        // сосед справа (примыкание end == next start)
        if let Some((&s, &e)) = self.runs.range(end..=end).next() {
            self.runs.remove(&s);
            end = e;
        }
        self.runs.insert(start, end);
    }

    fn remove(&mut self, idx: i64) {
        if let Some((&s, &e)) = self.runs.range(..=idx).next_back()
            && idx < e
        {
            self.runs.remove(&s);
            if s < idx {
                self.runs.insert(s, idx);
            }
            if idx + 1 < e {
                self.runs.insert(idx + 1, e);
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

/// Потокобезопасный кэш burst'ов с LRU по раздельным байтовым бюджетам
/// аудио/видео, pin'ом (loop-регион) и coverage-запросами.
pub struct BurstCache {
    config: CacheConfig,
    inner: Mutex<Inner>,
    /// Меняется при insert/evict/invalidate — UI перерисовывает полоски
    /// только при изменении.
    epoch: AtomicU64,
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
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
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
        let entry = inner.map.get_mut(&key)?;
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

    pub fn insert(&self, key: BurstKey, payload: Payload) {
        let mut inner = self.inner.lock().unwrap();
        let bytes = payload.bytes();
        let is_audio = payload.is_audio();
        debug_assert_eq!(is_audio, key.kind == BurstKind::Audio);

        if let Some(old) = inner.map.remove(&key) {
            inner.lru.remove(&old.recency);
            let ob = old.payload.bytes();
            if old.payload.is_audio() {
                inner.audio_bytes -= ob;
            } else {
                inner.video_bytes -= ob;
            }
        }

        inner.recency_counter += 1;
        let recency = inner.recency_counter;
        inner.map.insert(key, Entry { payload, recency, pinned: false });
        inner.lru.insert(recency, key);
        if is_audio {
            inner.audio_bytes += bytes;
        } else {
            inner.video_bytes += bytes;
        }
        inner.coverage.entry((key.source, key.kind)).or_default().insert(key.idx);

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
                set.remove(key.idx);
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
            s.insert(i);
        }
        assert_eq!(s.query(-10..100), vec![0..4, 7..9]);
        s.remove(2);
        assert_eq!(s.query(-10..100), vec![0..2, 3..4, 7..9]);
        s.remove(0);
        s.remove(3);
        assert_eq!(s.query(-10..100), vec![1..2, 7..9]);
        // повторные операции идемпотентны
        s.remove(0);
        s.insert(7);
        assert_eq!(s.query(-10..100), vec![1..2, 7..9]);
        // запрос обрезается диапазоном
        assert_eq!(s.query(8..100), vec![8..9]);
    }
}
