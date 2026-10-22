//! Access to the source bursts for the render graph.
//!
//! Two modes:
//! - `DirectFetcher` — owns the sources and decodes on demand,
//!   filling the cache (offline render/export: pace does not matter, invariant 7 —
//!   the same graph, another clock);
//! - `CacheFetcher` — CACHE ONLY (realtime playback: a miss = silence +
//!   underrun counter, playback never blocks on decode —
//!   invariant 5; filling the cache is done by MediaWorkers, M3).

use kt_cache::{BurstCache, SourceId};
use kt_media::{AudioBurst, MediaSource};
use kt_model::MediaId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn source_id(media: MediaId) -> SourceId {
    SourceId(media.0)
}

/// Returns the source's audio burst by (media, source idx) or None.
pub trait AudioFetcher {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>>;

    /// Who is reading now (R.55, C4). An ATTACHMENT has its own sub-mix stream for
    /// EACH instance: a reel placed in five places is five
    /// different streams through its chains, and sharing one reverb between them
    /// would be audible. Under reverse and stretching the instances read the reel
    /// also in a DIFFERENT order, that is, they would fight over one cursor.
    ///
    /// For regular fetchers this does not matter: a file has no state, and the address of a
    /// burst is a pure function of the index. Therefore the default is empty, and neither
    /// `DirectFetcher` nor `CacheFetcher` override the method.
    fn begin_clip(&mut self, _id: kt_model::ClipId) {}

    /// END OF MATERIAL of this media in 48 kHz frames (R.63 item 2), if it
    /// exists and is known to the fetcher.
    ///
    /// Why it is the fetcher's and not the plan's. The file length is a property of the SOURCE
    /// (law P), the plan knows only the address. Without this answer the engine does not
    /// distinguish “the burst is not yet decoded” (real starvation) from “there is no material
    /// at this address at all” — and honest silence at the tail of an event longer than the file
    /// was counted as underruns: the owner had 180 480 frames for five seconds
    /// (R.63 item 2).
    ///
    /// `None` — “there is no edge or it is unknown”: a generator, a stub, a reel
    /// (its silence is given by its own sub-mix), a file without a duration.
    /// Then the behaviour is as before, down to a single frame.
    fn material_end_frames(&mut self, _media: MediaId) -> Option<i64> {
        None
    }
}

/// Owns the sources; decodes in place, puts into the cache (if set).
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

    /// End of material from the passport of the OPEN source (R.63).
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

/// Cache only. Miss = None; the caller substitutes silence and counts an underrun.
pub struct CacheFetcher {
    cache: Arc<BurstCache>,
    pub misses: AtomicU64,
    /// Material ends in FRAMES at 48 kHz — from whoever opened the sources
    /// (R.63 item 2). Empty — the previous behaviour, down to a single frame.
    ends: std::collections::HashMap<MediaId, i64>,
}

impl CacheFetcher {
    pub fn new(cache: Arc<BurstCache>) -> CacheFetcher {
        CacheFetcher { cache, misses: AtomicU64::new(0), ends: Default::default() }
    }

    /// The same fetcher, but knowing the material lengths (player, export, probe).
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
