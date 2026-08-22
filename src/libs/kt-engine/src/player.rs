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
use crate::worker::{DecodeRequest, DecodeStats, MediaWorker, RequestKind};
use crate::{Priority, source_id};
use kt_cache::{BurstCache, CacheConfig};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick, TickRange};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
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

/// Пиковые уровни того, что уходит в кольцо (f32-биты; UI делает dB,
/// спад и peak-hold). fetch_max корректен: битовый паттерн неотрицательных
/// f32 монотонен.
pub struct Meters {
    pub peak_l: AtomicU32,
    pub peak_r: AtomicU32,
}

impl Meters {
    fn fold(&self, chunk: &[f32]) {
        let mut pl = 0f32;
        let mut pr = 0f32;
        for fr in chunk.chunks_exact(AUDIO_CHANNELS) {
            pl = pl.max(fr[0].abs());
            pr = pr.max(fr[1].abs());
        }
        self.peak_l.fetch_max(pl.to_bits(), Ordering::AcqRel);
        self.peak_r.fetch_max(pr.to_bits(), Ordering::AcqRel);
    }

    /// Прочитать и сбросить (swap-to-zero).
    pub fn take(&self) -> (f32, f32) {
        (
            f32::from_bits(self.peak_l.swap(0, Ordering::AcqRel)),
            f32::from_bits(self.peak_r.swap(0, Ordering::AcqRel)),
        )
    }
}

pub struct Player {
    pub cache: Arc<BurstCache>,
    pub transport: Arc<Transport>,
    pub shared: Arc<AudioShared>,
    pub meters: Arc<Meters>,
    /// Мастер-громкость (линейная, f32-биты), применяется в render-потоке.
    master_gain: Arc<AtomicU32>,
    /// Loop-регион в кадрах 48k (None = выключен).
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
    workers: Arc<HashMap<MediaId, MediaWorker>>,
    project: Arc<Mutex<Project>>,
    model_dirty: Arc<AtomicBool>,
    video: Mutex<VideoRenderer>,
    preview_quality: Arc<Mutex<Quality>>,
    stop: Arc<AtomicBool>,
    render_thread: Option<std::thread::JoinHandle<()>>,
    prefetch_thread: Option<std::thread::JoinHandle<()>>,
    pub underruns: Arc<AtomicU64>,
    pub decode_stats: Arc<DecodeStats>,
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
        let meters = Arc::new(Meters { peak_l: AtomicU32::new(0), peak_r: AtomicU32::new(0) });
        let master_gain = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let loop_region: Arc<Mutex<Option<(i64, i64)>>> = Arc::new(Mutex::new(None));
        let decode_stats = Arc::new(DecodeStats::default());
        let mut workers = HashMap::new();
        for (media, src) in sources {
            workers.insert(
                media,
                MediaWorker::spawn_with_stats(media, src, cache.clone(), decode_stats.clone()),
            );
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
            let meters = meters.clone();
            let master_gain = master_gain.clone();
            let loop_region = loop_region.clone();
            std::thread::Builder::new()
                .name("audio-render".into())
                .spawn(move || {
                    audio_render_loop(RenderCtx {
                        producer,
                        cache,
                        transport,
                        shared,
                        workers,
                        project,
                        dirty,
                        stop,
                        underruns,
                        meters,
                        master_gain,
                        loop_region,
                    })
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
            meters,
            master_gain,
            loop_region,
            workers,
            project,
            model_dirty,
            video,
            preview_quality,
            stop,
            render_thread: Some(render_thread),
            prefetch_thread: Some(prefetch_thread),
            underruns,
            decode_stats,
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

    /// Мастер-громкость превью (линейная; mute = 0).
    pub fn set_master_gain(&self, linear: f32) {
        self.master_gain.store(linear.max(0.0).to_bits(), Ordering::Release);
    }

    pub fn master_gain(&self) -> f32 {
        f32::from_bits(self.master_gain.load(Ordering::Acquire))
    }

    /// Loop-регион (None = выключен). Голова региона пинится в кэше,
    /// чтобы wrap не начинался с холода (тест 14).
    pub fn set_loop_region(&self, region: Option<TickRange>) {
        let frames = region.map(|r| (r.start.to_audio_frames(), r.end.to_audio_frames()));
        *self.loop_region.lock().unwrap() = frames;
        self.cache.unpin_all();
        if let Some((s, _)) = frames {
            let renderer = AudioRenderer::new(&self.project.lock().unwrap().clone());
            for (media, bursts) in
                renderer.wanted_audio_bursts(s..s + kt_time::AUDIO_SAMPLE_RATE as i64 / 2)
            {
                self.cache.pin_range(source_id(media), kt_cache::BurstKind::Audio, bursts);
            }
        }
    }

    /// Пиковые уровни на паузе: один burst с позиции t через одноразовое
    /// FX-состояние и кэш (промах = тишина — для метра этого достаточно).
    pub fn sample_levels_at(&self, t: Tick) -> (f32, f32) {
        let renderer = AudioRenderer::new(&self.project.lock().unwrap().clone());
        let mut store = FxStateStore::new();
        renderer.sync_fx(&mut store);
        let mut fetcher = CacheFetcher::new(self.cache.clone());
        let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
        renderer.render_frames_with(&mut fetcher, &mut store, t.to_audio_frames(), &mut chunk);
        let gain = self.master_gain();
        let mut pl = 0f32;
        let mut pr = 0f32;
        for fr in chunk.chunks_exact(AUDIO_CHANNELS) {
            pl = pl.max((fr[0] * gain).abs());
            pr = pr.max((fr[1] * gain).abs());
        }
        (pl, pr)
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

struct RenderCtx {
    producer: rtrb::Producer<f32>,
    cache: Arc<BurstCache>,
    transport: Arc<Transport>,
    shared: Arc<AudioShared>,
    workers: Arc<HashMap<MediaId, MediaWorker>>,
    project: Arc<Mutex<Project>>,
    dirty: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    meters: Arc<Meters>,
    master_gain: Arc<AtomicU32>,
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
}

fn audio_render_loop(ctx: RenderCtx) {
    let RenderCtx {
        mut producer,
        cache,
        transport,
        shared,
        workers,
        project,
        dirty,
        stop,
        underruns,
        meters,
        master_gain,
        loop_region,
    } = ctx;
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

        let region = *loop_region.lock().unwrap();
        // продвигаем playhead на потреблённое колбэком (с wrap при loop)
        let consumed = shared.consumed_frames.swap(0, Ordering::AcqRel);
        if consumed > 0 {
            transport.advance_frames_wrapped(consumed, region);
        }

        if transport.is_playing() {
            let gain = f32::from_bits(master_gain.load(Ordering::Acquire));
            // заказ ближайшего аудио с Playback-приоритетом (идемпотентно);
            // при loop заказываем через wrap-отображение
            match region {
                Some((rs, re)) if re > rs && render_head >= rs => {
                    let tl = rs + (render_head - rs).rem_euclid(re - rs);
                    let ahead = PLAYBACK_AUDIO_LOOKAHEAD_FRAMES.min(re - rs);
                    let end1 = (tl + ahead).min(re);
                    prefetch_audio_pass(
                        &renderer,
                        &cache,
                        &workers,
                        tl..end1,
                        Priority::Playback,
                        generation,
                    );
                    let rest = ahead - (end1 - tl);
                    if rest > 0 {
                        prefetch_audio_pass(
                            &renderer,
                            &cache,
                            &workers,
                            rs..rs + rest,
                            Priority::Playback,
                            generation,
                        );
                    }
                }
                _ => {
                    prefetch_audio_pass(
                        &renderer,
                        &cache,
                        &workers,
                        render_head..render_head + PLAYBACK_AUDIO_LOOKAHEAD_FRAMES,
                        Priority::Playback,
                        generation,
                    );
                }
            }
            // наполняем кольцо: render_head линеен, loop-маппинг внутри
            // render_looped_with (wrap НЕ бампает generation — FX живут)
            while producer.slots() >= chunk.len() {
                let stats = match region {
                    Some((rs, re)) if re > rs && render_head >= rs => renderer.render_looped_with(
                        &mut fetcher,
                        &mut store,
                        rs..re,
                        render_head,
                        &mut chunk,
                    ),
                    _ => renderer.render_frames_with(
                        &mut fetcher,
                        &mut store,
                        render_head,
                        &mut chunk,
                    ),
                };
                if stats.underruns > 0 {
                    underruns.fetch_add(stats.underruns, Ordering::Relaxed);
                }
                if gain != 1.0 {
                    for s in chunk.iter_mut() {
                        *s *= gain;
                    }
                }
                meters.fold(&chunk);
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
