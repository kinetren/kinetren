//! Доступ к burst'ам источников для render graph.
//!
//! Два режима:
//! - `DirectFetcher` — владеет источниками и декодирует по требованию,
//!   наполняя кэш (офлайн-рендер/экспорт: темп неважен, инвариант №7 —
//!   тот же граф, другой clock);
//! - `CacheFetcher` — ТОЛЬКО кэш (realtime playback: промах = тишина +
//!   underrun-счётчик, playback никогда не блокируется на декоде —
//!   инвариант №5; наполнением кэша занимаются MediaWorker'ы, M3).

use kt_cache::{BurstCache, SourceId};
use kt_media::{AudioBurst, MediaSource};
use kt_model::MediaId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn source_id(media: MediaId) -> SourceId {
    SourceId(media.0)
}

/// Отдаёт аудио-burst источника по (media, idx источника) или None.
pub trait AudioFetcher {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>>;

    /// Кто сейчас читает (Р.55, С4). У ВЛОЖЕНИЯ свой поток суб-микса на
    /// КАЖДЫЙ экземпляр: лента, поставленная в пяти местах, — это пять
    /// разных потоков через её цепочки, и делить между ними один реверб
    /// было бы слышно. Под реверсом и растяжением экземпляры читают ленту
    /// ещё и в РАЗНОМ порядке, то есть дерутся бы за один курсор.
    ///
    /// Обычным фетчерам это безразлично: у файла состояния нет, а адрес
    /// бёрста — чистая функция индекса. Поэтому умолчание пустое, и ни
    /// `DirectFetcher`, ни `CacheFetcher` метод не переопределяют.
    fn begin_clip(&mut self, _id: kt_model::ClipId) {}

    /// КОНЕЦ МАТЕРИАЛА этого медиа в кадрах 48 кГц (Р.63 п.2), если он
    /// существует и известен фетчеру.
    ///
    /// Зачем это фетчеру, а не плану. Длина файла — свойство ИСТОЧНИКА
    /// (закон П), план знает только адрес. Без этого ответа движок не
    /// отличает «бёрст ещё не декодирован» (настоящее голодание) от «за
    /// этим адресом материала нет вовсе» — и честная тишина в хвосте
    /// события, которое длиннее файла, считалась underrun'ами: у
    /// владельца 180 480 кадров на пять секунд (Р.63 п.2).
    ///
    /// `None` — «края нет или он неизвестен»: генератор, заглушка, лента
    /// (её тишину даёт собственный суб-микс), файл без длительности.
    /// Тогда поведение прежнее, до единого кадра.
    fn material_end_frames(&mut self, _media: MediaId) -> Option<i64> {
        None
    }
}

/// Владеет источниками; декодирует на месте, кладёт в кэш (если задан).
pub struct DirectFetcher {
    sources: HashMap<MediaId, Box<dyn MediaSource>>,
    cache: Option<Arc<BurstCache>>,
}

impl DirectFetcher {
    pub fn new(cache: Option<Arc<BurstCache>>) -> DirectFetcher {
        DirectFetcher { sources: HashMap::new(), cache }
    }

    pub fn add_source(&mut self, media: MediaId, source: Box<dyn MediaSource>) {
        self.sources.insert(media, source);
    }

    /// Конец материала по паспорту ОТКРЫТОГО источника (Р.63).
    fn source_end_frames(&self, media: MediaId) -> Option<i64> {
        let d = self.sources.get(&media)?.info().duration;
        (d.0 > 0).then(|| d.to_audio_frames())
    }

    pub fn source_mut(&mut self, media: MediaId) -> Option<&mut Box<dyn MediaSource>> {
        self.sources.get_mut(&media)
    }
}

impl AudioFetcher for DirectFetcher {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>> {
        if let Some(cache) = &self.cache
            && let Some(b) = cache.get_audio(source_id(media), idx)
        {
            return Some(b);
        }
        let src = self.sources.get_mut(&media)?;
        let burst = Arc::new(src.read_audio_burst(idx).ok()?);
        if let Some(cache) = &self.cache {
            cache.insert_audio(source_id(media), burst.clone());
        }
        Some(burst)
    }

    fn material_end_frames(&mut self, media: MediaId) -> Option<i64> {
        self.source_end_frames(media)
    }
}

/// Только кэш. Промах = None; вызывающий подставляет тишину и считает underrun.
pub struct CacheFetcher {
    cache: Arc<BurstCache>,
    pub misses: AtomicU64,
    /// Концы материала в КАДРАХ 48 кГц — от того, кто открыл источники
    /// (Р.63 п.2). Пусто — прежнее поведение до единого кадра.
    ends: std::collections::HashMap<MediaId, i64>,
}

impl CacheFetcher {
    pub fn new(cache: Arc<BurstCache>) -> CacheFetcher {
        CacheFetcher { cache, misses: AtomicU64::new(0), ends: Default::default() }
    }

    /// Тот же фетчер, но знающий длины материала (плеер, экспорт, probe).
    pub fn with_lengths(
        cache: Arc<BurstCache>,
        lengths: &std::collections::HashMap<MediaId, kt_time::Tick>,
    ) -> CacheFetcher {
        let mut f = CacheFetcher::new(cache);
        f.ends = lengths
            .iter()
            .filter(|(_, d)| d.0 > 0)
            .map(|(m, d)| (*m, d.to_audio_frames()))
            .collect();
        f
    }

    pub fn miss_count(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }
}

impl AudioFetcher for CacheFetcher {
    fn material_end_frames(&mut self, media: MediaId) -> Option<i64> {
        self.ends.get(&media).copied()
    }

    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>> {
        let hit = self.cache.get_audio(source_id(media), idx);
        if hit.is_none() {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        hit
    }
}
