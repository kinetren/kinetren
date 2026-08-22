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
}

/// Только кэш. Промах = None; вызывающий подставляет тишину и считает underrun.
pub struct CacheFetcher {
    cache: Arc<BurstCache>,
    pub misses: AtomicU64,
}

impl CacheFetcher {
    pub fn new(cache: Arc<BurstCache>) -> CacheFetcher {
        CacheFetcher { cache, misses: AtomicU64::new(0) }
    }

    pub fn miss_count(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }
}

impl AudioFetcher for CacheFetcher {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>> {
        let hit = self.cache.get_audio(source_id(media), idx);
        if hit.is_none() {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        hit
    }
}
