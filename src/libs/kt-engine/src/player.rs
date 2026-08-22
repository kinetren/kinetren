//! Player: realtime-ядро воспроизведения.
//!
//! Потоки и владение (docs/architecture.md):
//! - **аудио-render-поток** (здесь) владеет FxStateStore и AudioRenderer,
//!   наполняет SPSC-кольцо; продвигает playhead ровно на потреблённое
//!   аудио-колбэком (инвариант №6);
//! - **cpal-колбэк** (kt-app) только читает кольцо и публикует счётчик
//!   потреблённых кадров — ни аллокаций, ни локов;
//! - **MediaWorker'ы** эксклюзивно владеют декодерами (инвариант №5);
//! - **prefetch-поток** идемпотентно досыпает кэш;
//! - UI-поток читает playhead, композитит видео из кэша, шлёт правки модели.
//!
//! Playback никогда не ждёт декод: промах кэша = тишина + underrun-счётчик,
//! недостающее тут же заказывается с Priority::Playback.

use crate::audio::AudioRenderer;
use crate::fetch::CacheFetcher;
use crate::fxstore::FxStateStore;
use crate::prefetch::prefetch_audio_pass;
use crate::transport::Transport;
use crate::video::{VideoRenderer, video_cache_idx};
use crate::worker::{DecodeRequest, MediaWorker, RequestKind};
use crate::{Priority, source_id};
use kt_cache::{BurstCache, CacheConfig};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick, TickRange};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const RING_CAPACITY_FRAMES: usize = AUDIO_BURST_FRAMES * 4;
const PLAYBACK_AUDIO_LOOKAHEAD_FRAMES: i64 = AUDIO_SAMPLE_RATE as i64 / 5; // 200 мс
const PREFETCH_AUDIO_LOOKAHEAD_FRAMES: i64 = 2 * AUDIO_SAMPLE_RATE as i64; // 2 c
const PREFETCH_VIDEO_LOOKAHEAD_TICKS: i64 = 2 * kt_time::TICK_HZ;

/// Общая с аудио-колбэком часть: кольцо + счётчик потребления + flush.
pub struct AudioShared {
    /// Кадров реально потреблено колбэком (для продвижения playhead).
    pub consumed_frames: AtomicI64,
    /// Просьба колбэку выбросить всё из кольца (после seek).
    pub flush: AtomicBool,
    /// Underrun'ы колбэка (кольцо было пусто во время playing).
    pub callback_underruns: AtomicU64,
}

pub struct Player {
    pub cache: Arc<BurstCache>,
    pub transport: Arc<Transport>,
    pub shared: Arc<AudioShared>,
    workers: Arc<HashMap<MediaId, MediaWorker>>,
    project: Arc<Mutex<Project>>,
    model_dirty: Arc<AtomicBool>,
    video: Mutex<VideoRenderer>,
    preview_quality: Arc<Mutex<Quality>>,
    stop: Arc<AtomicBool>,
    render_thread: Option<std::thread::JoinHandle<()>>,
    prefetch_thread: Option<std::thread::JoinHandle<()>>,
    pub underruns: Arc<AtomicU64>,
}

impl Player {
    /// `sources` — открытые источники по media id (владение уходит воркерам).
    pub fn new(
        project: Project,
        sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    ) -> (Player, rtrb::Consumer<f32>) {
        let cache = Arc::new(BurstCache::new(CacheConfig::default()));
        let transport = Arc::new(Transport::new());
        let shared = Arc::new(AudioShared {
            consumed_frames: AtomicI64::new(0),
            flush: AtomicBool::new(false),
            callback_underruns: AtomicU64::new(0),
        });
        let mut workers = HashMap::new();
        for (media, src) in sources {
            workers.insert(media, MediaWorker::spawn(media, src, cache.clone()));
        }
        let workers = Arc::new(workers);
        let video = Mutex::new(VideoRenderer::new(&project));
        let project = Arc::new(Mutex::new(project));
        let model_dirty = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let underruns = Arc::new(AtomicU64::new(0));

        let (producer, consumer) =
            rtrb::RingBuffer::<f32>::new(RING_CAPACITY_FRAMES * AUDIO_CHANNELS);

        let render_thread = {
            let cache = cache.clone();
            let transport = transport.clone();
            let shared = shared.clone();
            let workers = workers.clone();
            let project = project.clone();
            let dirty = model_dirty.clone();
            let stop = stop.clone();
            let underruns = underruns.clone();
            std::thread::Builder::new()
                .name("audio-render".into())
                .spawn(move || {
                    audio_render_loop(
                        producer, cache, transport, shared, workers, project, dirty, stop,
                        underruns,
                    )
                })
                .expect("spawn audio render")
        };

        let preview_quality = Arc::new(Mutex::new(Quality { width: 640, height: 360 }));
        let prefetch_thread = {
            let cache = cache.clone();
            let transport = transport.clone();
            let workers = workers.clone();
            let project = project.clone();
            let stop = stop.clone();
            let quality = preview_quality.clone();
            std::thread::Builder::new()
                .name("prefetch".into())
                .spawn(move || prefetch_loop(cache, transport, workers, project, quality, stop))
                .expect("spawn prefetch")
        };

        let player = Player {
            cache,
            transport,
            shared,
            workers,
            project,
            model_dirty,
            video,
            preview_quality,
            stop,
            render_thread: Some(render_thread),
            prefetch_thread: Some(prefetch_thread),
            underruns,
        };
        (player, consumer)
    }

    pub fn set_project(&self, p: Project) {
        *self.video.lock().unwrap() = VideoRenderer::new(&p);
        *self.project.lock().unwrap() = p;
        self.model_dirty.store(true, Ordering::Release);
    }

    pub fn project(&self) -> Project {
        self.project.lock().unwrap().clone()
    }

    pub fn set_preview_quality(&self, q: Quality) {
        *self.preview_quality.lock().unwrap() = q;
    }

    pub fn preview_quality(&self) -> Quality {
        *self.preview_quality.lock().unwrap()
    }

    pub fn play(&self) {
        self.transport.set_playing(true);
    }

    pub fn pause(&self) {
        self.transport.set_playing(false);
    }

    pub fn seek(&self, t: Tick) {
        self.transport.seek(t);
        // воркеры дропают запросы старых поколений
        let generation = self.transport.generation();
        for w in self.workers.values() {
            w.cancel_generations_below(generation);
        }
        self.shared.flush.store(true, Ordering::Release);
    }

    /// UI: составить кадр превью для момента t (выравнивается по сетке fps).
    /// Недостающие слои заказываются с Priority::Playback; вернётся
    /// (буфер RGBA, размер, все ли слои готовы).
    pub fn compose_preview(&self, t: Tick) -> (Vec<u8>, Quality, bool) {
        let q = self.preview_quality();
        let video = self.video.lock().unwrap();
        let aligned = video.grid.align_floor(t);
        let generation = self.transport.generation();
        for (media, src_pts) in video.layers_at(aligned) {
            let idx = video_cache_idx(src_pts);
            if self.cache.get_video(source_id(media), Some(q), idx).is_none()
                && let Some(w) = self.workers.get(&media)
            {
                w.submit(DecodeRequest {
                    kind: RequestKind::video(src_pts, Some(q)),
                    prio: Priority::Playback,
                    generation,
                });
            }
        }
        let (buf, complete) = video.compose_from_cache(&self.cache, aligned, q);
        (buf, q, complete)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.render_thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self.prefetch_thread.take() {
            let _ = t.join();
        }
        // воркеры остановим, забрав их из Arc: render/prefetch потоки уже
        // завершены, мы — последний владелец
        if let Some(map) = Arc::get_mut(&mut self.workers) {
            for (_, w) in map.drain() {
                w.shutdown();
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audio_render_loop(
    mut producer: rtrb::Producer<f32>,
    cache: Arc<BurstCache>,
    transport: Arc<Transport>,
    shared: Arc<AudioShared>,
    workers: Arc<HashMap<MediaId, MediaWorker>>,
    project: Arc<Mutex<Project>>,
    dirty: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
) {
    let mut store = FxStateStore::new();
    let mut renderer = AudioRenderer::new(&project.lock().unwrap().clone());
    renderer.sync_fx(&mut store);
    let mut fetcher = CacheFetcher::new(cache.clone());
    // локальная позиция рендера (кадры); может опережать playhead на кольцо
    let mut render_head: i64 = transport.playhead_frames();
    let mut last_generation = transport.generation();
    let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS];

    while !stop.load(Ordering::Acquire) {
        if dirty.swap(false, Ordering::AcqRel) {
            renderer = AudioRenderer::new(&project.lock().unwrap().clone());
            renderer.sync_fx(&mut store); // state переживает пересборку (инв. №3)
        }
        let generation = transport.generation();
        if generation != last_generation {
            last_generation = generation;
            store.reset_all(); // pre-roll при следующем чтении
            render_head = transport.playhead_frames();
            shared.consumed_frames.store(0, Ordering::Release);
        }

        // продвигаем playhead на потреблённое колбэком
        let consumed = shared.consumed_frames.swap(0, Ordering::AcqRel);
        if consumed > 0 {
            transport.advance_frames(consumed);
        }

        if transport.is_playing() {
            // заказ ближайшего аудио с Playback-приоритетом (идемпотентно)
            prefetch_audio_pass(
                &renderer,
                &cache,
                &workers,
                render_head..render_head + PLAYBACK_AUDIO_LOOKAHEAD_FRAMES,
                Priority::Playback,
                generation,
            );
            // наполняем кольцо
            while producer.slots() >= chunk.len() {
                let stats =
                    renderer.render_frames_with(&mut fetcher, &mut store, render_head, &mut chunk);
                if stats.underruns > 0 {
                    underruns.fetch_add(stats.underruns, Ordering::Relaxed);
                }
                for &s in chunk.iter() {
                    let _ = producer.push(s);
                }
                render_head += AUDIO_BURST_FRAMES as i64;
            }
        }
        // render_head сбрасывается ТОЛЬКО по seek (generation): при паузе
        // кольцо сохраняет уже отрендеренный хвост, resume продолжает с него.
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

fn prefetch_loop(
    cache: Arc<BurstCache>,
    transport: Arc<Transport>,
    workers: Arc<HashMap<MediaId, MediaWorker>>,
    project: Arc<Mutex<Project>>,
    preview_quality: Arc<Mutex<Quality>>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Acquire) {
        std::thread::sleep(std::time::Duration::from_millis(100));
        // модель могла смениться — пересобираем дёшево каждый цикл
        let p = project.lock().unwrap().clone();
        let renderer = AudioRenderer::new(&p);
        let video = VideoRenderer::new(&p);
        let generation = transport.generation();
        let head = transport.playhead_frames();
        prefetch_audio_pass(
            &renderer,
            &cache,
            &workers,
            head..head + PREFETCH_AUDIO_LOOKAHEAD_FRAMES,
            Priority::NearPrefetch,
            generation,
        );
        // видео-lookahead: кадры ближайших 2 c в превью-качестве
        let t0 = transport.playhead();
        let range = TickRange::new(t0, Tick(t0.0 + PREFETCH_VIDEO_LOOKAHEAD_TICKS));
        let q = *preview_quality.lock().unwrap();
        for (media, src_pts) in video.wanted_frames(range) {
            if cache.get_video(source_id(media), Some(q), video_cache_idx(src_pts)).is_none()
                && let Some(w) = workers.get(&media)
            {
                w.submit(DecodeRequest {
                    kind: RequestKind::video(src_pts, Some(q)),
                    prio: Priority::NearPrefetch,
                    generation,
                });
            }
        }
    }
}
