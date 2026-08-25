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
    /// Скорость воспроизведения (f32-биты; 1.0 = норма). Колбэк читает
    /// кольцо с этим шагом (линейная интерполяция) — питч плывёт как в
    /// Vegas (varispeed, п.4); playhead сам едет быстрее/медленнее,
    /// т.к. consumed считает кадры ИСТОЧНИКА.
    pub playback_rate: AtomicU32,
    /// Оценка задержки вывода в кадрах 48k (cpal timestamp playback−callback):
    /// playhead двигается в момент ЗАБОРА кадров колбэком, а слышны они на
    /// эту задержку позже — видео для «слышимого сейчас» момента (п.30).
    pub output_latency_frames: AtomicI64,
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

/// Побудка фоновых потоков: в паузе они паркуются на condvar вместо
/// busy-цикла (п.1: 500 пробуждений/с audio-render + клон Project каждые
/// 100 мс prefetch грели систему даже с пустым проектом).
pub struct Wake {
    lock: Mutex<()>,
    cv: std::sync::Condvar,
}

impl Wake {
    fn notify(&self) {
        let _g = self.lock.lock().unwrap();
        self.cv.notify_all();
    }

    fn wait_timeout(&self, d: std::time::Duration) {
        let g = self.lock.lock().unwrap();
        let _ = self.cv.wait_timeout(g, d).unwrap();
    }
}

/// Диагностика фоновых циклов (тест 20: тишина в простое).
#[derive(Default)]
pub struct LoopStats {
    pub render_passes: AtomicU64,
    pub prefetch_passes: AtomicU64,
    pub prefetch_rebuilds: AtomicU64,
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
    workers: Workers,
    project: Arc<Mutex<Project>>,
    model_dirty: Arc<AtomicBool>,
    video: Mutex<VideoRenderer>,
    preview_quality: Arc<Mutex<Quality>>,
    stop: Arc<AtomicBool>,
    render_thread: Option<std::thread::JoinHandle<()>>,
    prefetch_thread: Option<std::thread::JoinHandle<()>>,
    pub underruns: Arc<AtomicU64>,
    pub decode_stats: Arc<DecodeStats>,
    wake: Arc<Wake>,
    /// Версия проекта: prefetch пересобирает рендереры только по смене.
    project_version: Arc<AtomicU64>,
    /// Кэш графа для sample_levels_at (п.1): (версия, renderer, store).
    levels_probe: Mutex<Option<(u64, AudioRenderer, FxStateStore)>>,
    pub loop_stats: Arc<LoopStats>,
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
            playback_rate: AtomicU32::new(1.0f32.to_bits()),
            output_latency_frames: AtomicI64::new(0),
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
        let workers: Workers = Arc::new(std::sync::RwLock::new(workers));
        let video = Mutex::new(VideoRenderer::new(&project));
        let project = Arc::new(Mutex::new(project));
        let model_dirty = Arc::new(AtomicBool::new(true));
        let wake = Arc::new(Wake { lock: Mutex::new(()), cv: std::sync::Condvar::new() });
        let project_version = Arc::new(AtomicU64::new(0));
        let loop_stats = Arc::new(LoopStats::default());
        let stop = Arc::new(AtomicBool::new(false));
        let underruns = Arc::new(AtomicU64::new(0));

        let (producer, consumer) =
            rtrb::RingBuffer::<f32>::new(RING_CAPACITY_FRAMES * AUDIO_CHANNELS);

        let render_thread = {
            let wake_r = wake.clone();
            let stats_r = loop_stats.clone();
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
                        wake: wake_r,
                        loop_stats: stats_r,
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
            let wake_p = wake.clone();
            let stats_p = loop_stats.clone();
            let version_p = project_version.clone();
            let cache = cache.clone();
            let transport = transport.clone();
            let workers = workers.clone();
            let project = project.clone();
            let stop = stop.clone();
            let quality = preview_quality.clone();
            std::thread::Builder::new()
                .name("prefetch".into())
                .spawn(move || {
                    prefetch_loop(
                        cache, transport, workers, project, quality, stop, wake_p, stats_p,
                        version_p,
                    )
                })
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
            wake,
            project_version,
            loop_stats,
            levels_probe: Mutex::new(None),
        };
        (player, consumer)
    }

    pub fn set_project(&self, p: Project) {
        self.project_version.fetch_add(1, Ordering::AcqRel);
        self.wake_after_project_swap(p);
    }

    fn wake_after_project_swap(&self, p: Project) {
        *self.video.lock().unwrap() = VideoRenderer::new(&p);
        *self.project.lock().unwrap() = p;
        self.model_dirty.store(true, Ordering::Release);
        self.wake.notify();
    }

    pub fn project(&self) -> Project {
        self.project.lock().unwrap().clone()
    }

    /// Скорость воспроизведения (п.4, ползунок как в Vegas).
    pub fn set_playback_rate(&self, rate: f32) {
        // отрицательное = reverse-транспорт (J4 v1.4); 0 = стоп потребления
        // (тишина, playhead стоит). Колбэк использует |rate|, знак читает
        // только render-поток.
        self.shared.playback_rate.store(rate.clamp(-2.0, 2.0).to_bits(), Ordering::Release);
    }

    pub fn playback_rate(&self) -> f32 {
        f32::from_bits(self.shared.playback_rate.load(Ordering::Acquire))
    }

    pub fn set_preview_quality(&self, q: Quality) {
        *self.preview_quality.lock().unwrap() = q;
    }

    pub fn preview_quality(&self) -> Quality {
        *self.preview_quality.lock().unwrap()
    }

    pub fn play(&self) {
        self.transport.set_playing(true);
        self.wake.notify();
    }

    pub fn pause(&self) {
        self.transport.set_playing(false);
        self.wake.notify();
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
        self.wake.notify();
    }

    /// Пиковые уровни на паузе: один burst с позиции t через одноразовое
    /// FX-состояние и кэш (промах = тишина — для метра этого достаточно).
    pub fn sample_levels_at(&self, t: Tick) -> (f32, f32) {
        // Кэш пробы (п.1): раньше КАЖДЫЙ вызов (8 раз/с на паузе) клонировал
        // весь Project и пересоздавал все FX. Теперь граф живёт до смены
        // версии проекта; state сбрасывается перед замером (детерминизм).
        let version = self.project_version.load(Ordering::Acquire);
        let mut probe = self.levels_probe.lock().unwrap();
        let rebuild = probe.as_ref().map(|(v, _, _)| *v != version).unwrap_or(true);
        if rebuild {
            let renderer = AudioRenderer::new(&self.project.lock().unwrap().clone());
            let mut store = FxStateStore::new();
            renderer.sync_fx(&mut store);
            *probe = Some((version, renderer, store));
        }
        let (_, renderer, store) = probe.as_mut().unwrap();
        store.reset_all();
        let mut fetcher = CacheFetcher::new(self.cache.clone());
        let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
        renderer.render_frames_with(&mut fetcher, store, t.to_audio_frames(), &mut chunk);
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
        for w in self.workers.read().unwrap().values() {
            w.cancel_generations_below(generation);
        }
        self.shared.flush.store(true, Ordering::Release);
        self.wake.notify();
    }

    /// Добавить медиа-источник БЕЗ пересоздания плеера: новый MediaWorker
    /// на общий кэш; транспорт, кольцо, FX-state и прогретый кэш нетронуты
    /// (регрессия п.27: раньше добавление файла сбрасывало весь кэш).
    pub fn add_media(&self, media: kt_model::MediaId, source: Box<dyn kt_media::MediaSource>) {
        let w = MediaWorker::spawn_with_stats(
            media,
            source,
            self.cache.clone(),
            self.decode_stats.clone(),
        );
        if let Some(old) = self.workers.write().unwrap().insert(media, w) {
            old.shutdown();
        }
    }

    /// Убрать медиа (симметрия RemoveMedia): воркер гасится, кэш других
    /// источников не трогается.
    pub fn remove_media(&self, media: kt_model::MediaId) {
        if let Some(w) = self.workers.write().unwrap().remove(&media) {
            w.shutdown();
        }
    }

    /// Момент, который сейчас СЛЫШНО (для выбора видеокадра превью):
    /// playhead минус задержка вывода. При паузе или без вывода — playhead
    /// как есть; loop-регион учитывается wrap'ом (п.30 — рассинхрон A/V).
    pub fn video_playhead(&self) -> Tick {
        if !self.transport.is_playing() {
            return self.transport.playhead();
        }
        let latency = self.shared.output_latency_frames.load(Ordering::Acquire);
        let head = self.transport.playhead_frames();
        let region = *self.loop_region.lock().unwrap();
        let dir: i64 =
            if f32::from_bits(self.shared.playback_rate.load(Ordering::Acquire)) < 0.0 {
                -1
            } else {
                1
            };
        Tick::from_audio_frames(video_display_frame(head, latency, region, dir))
    }

    /// UI: составить кадр превью для момента t (выравнивается по сетке fps).
    /// Недостающие слои заказываются с Priority::Playback; вернётся
    /// (буфер RGBA, размер, все ли слои готовы).
    /// Сетка кадров проекта — ключ кадра превью для UI (тест 39): тик,
    /// выровненный по ней, меняется только на границе видеокадра.
    pub fn video_grid(&self) -> kt_time::FpsGrid {
        self.video.lock().unwrap().grid
    }

    pub fn compose_preview(&self, t: Tick) -> (Vec<u8>, Quality, bool) {
        let q = self.preview_quality();
        let video = self.video.lock().unwrap();
        let aligned = video.grid.align_floor(t);
        let generation = self.transport.generation();
        let step = video.grid.frame_step();
        for (media, src_pts) in video.layers_at(aligned) {
            let idx = video_cache_idx(src_pts);
            // contains, не get: проба не должна освежать LRU (п.19)
            if !self.cache.contains_video(source_id(media), Some(q), idx)
                && let Some(w) = self.workers.read().unwrap().get(&media)
            {
                w.submit(DecodeRequest {
                    kind: RequestKind::video_on_grid(src_pts, Some(q), step),
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
        self.wake.notify();
        if let Some(t) = self.render_thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self.prefetch_thread.take() {
            let _ = t.join();
        }
        // воркеры остановим, забрав их из Arc: render/prefetch потоки уже
        // завершены, мы — последний владелец
        if let Some(lock) = Arc::get_mut(&mut self.workers) {
            for (_, w) in lock.get_mut().unwrap().drain() {
                w.shutdown();
            }
        }
    }
}

struct RenderCtx {
    producer: rtrb::Producer<f32>,
    cache: Arc<BurstCache>,
    transport: Arc<Transport>,
    wake: Arc<Wake>,
    loop_stats: Arc<LoopStats>,
    shared: Arc<AudioShared>,
    workers: Workers,
    project: Arc<Mutex<Project>>,
    dirty: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    meters: Arc<Meters>,
    master_gain: Arc<AtomicU32>,
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
}

/// Карта воркеров: RwLock, чтобы add_media/remove_media добавляли декодеры
/// БЕЗ пересоздания плеера (кэш/транспорт/кольцо переживают правки медиа —
/// регрессия п.27: раньше добавление файла пересоздавало Player и сбрасывало
/// весь прогретый кэш). Пишущие захваты — только короткие, из UI-потока.
type Workers = Arc<std::sync::RwLock<HashMap<MediaId, MediaWorker>>>;

fn audio_render_loop(ctx: RenderCtx) {
    let RenderCtx {
        mut producer,
        cache,
        transport,
        wake,
        loop_stats,
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
    let mut last_dir: i64 = 1;
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
        // направление транспорта — знак playback_rate (J4 v1.4). Смена
        // знака = мини-seek без бампа generation: сброс FX, кольца и
        // render_head (в кольце лежит материал прежнего направления).
        let rate_now = f32::from_bits(shared.playback_rate.load(Ordering::Acquire));
        let dir: i64 = if rate_now < 0.0 { -1 } else { 1 };
        if dir != last_dir {
            last_dir = dir;
            store.reset_all();
            render_head = transport.playhead_frames();
            shared.consumed_frames.store(0, Ordering::Release);
            shared.flush.store(true, Ordering::Release);
        }
        // продвигаем playhead на потреблённое колбэком (со знаком
        // направления; wrap при loop работает в обе стороны — тест 43)
        let consumed = shared.consumed_frames.swap(0, Ordering::AcqRel);
        if consumed > 0 {
            transport.advance_frames_wrapped(consumed * dir, region);
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
                        &workers.read().unwrap(),
                        tl..end1,
                        Priority::Playback,
                        generation,
                    );
                    let rest = ahead - (end1 - tl);
                    if rest > 0 {
                        prefetch_audio_pass(
                            &renderer,
                            &cache,
                            &workers.read().unwrap(),
                            rs..rs + rest,
                            Priority::Playback,
                            generation,
                        );
                    }
                }
                _ => {
                    let win = if dir < 0 {
                        (render_head - PLAYBACK_AUDIO_LOOKAHEAD_FRAMES).max(0)..render_head
                    } else {
                        render_head..render_head + PLAYBACK_AUDIO_LOOKAHEAD_FRAMES
                    };
                    prefetch_audio_pass(
                        &renderer,
                        &cache,
                        &workers.read().unwrap(),
                        win,
                        Priority::Playback,
                        generation,
                    );
                }
            }
            // наполняем кольцо: render_head линеен, loop-маппинг внутри
            // render_looped_with (wrap НЕ бампает generation — FX живут)
            while producer.slots() >= chunk.len() {
                let stats = if dir < 0 {
                    // reverse (J4 v1.4): блок ПЕРЕД render_head, материал
                    // разворачивается покадрово перед кольцом; FX — dry
                    // (см. render_frames_raw), loop-wrap через rem_euclid
                    let start = render_head - AUDIO_BURST_FRAMES as i64;
                    let st = match region {
                        Some((rs, re)) if re > rs && render_head > rs => {
                            renderer.render_looped_raw(&mut fetcher, rs..re, start, &mut chunk)
                        }
                        _ => renderer.render_frames_raw(&mut fetcher, start, &mut chunk),
                    };
                    reverse_frames(&mut chunk);
                    render_head = start;
                    st
                } else {
                    let st = match region {
                        Some((rs, re)) if re > rs && render_head >= rs => renderer
                            .render_looped_with(
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
                    render_head += AUDIO_BURST_FRAMES as i64;
                    st
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
            }
        }
        // render_head сбрасывается ТОЛЬКО по seek (generation): при паузе
        // кольцо сохраняет уже отрендеренный хвост, resume продолжает с него.
        loop_stats.render_passes.fetch_add(1, Ordering::Relaxed);
        if transport.is_playing() {
            // 8 мс: кольцо держит ~85 мс (4 burst'а), колбэк ест ~21 мс за
            // раз — запас четырёхкратный; 2 мс были 500 пробуждениями/с и
            // грели CPU на слабых машинах (п.1 раунда 5)
            std::thread::sleep(std::time::Duration::from_millis(8));
        } else {
            // пауза: паркуемся до play/seek/правки (страховочный таймаут —
            // добрать «хвостовой» consumed после остановки колбэка)
            wake.wait_timeout(std::time::Duration::from_millis(250));
        }
    }
}

/// Видео-lookahead префетча: мин(2 с, половина видео-бюджета) — иначе
/// prefetch-вставки высоких качеств выселяют собственное окно (п.19в).
fn video_lookahead_ticks(q: Quality, frame_step: i64, max_video_bytes: usize) -> i64 {
    let frame_bytes = (q.width as usize * q.height as usize * 4).max(1);
    let frames = ((max_video_bytes / 2) / frame_bytes).max(2) as i64;
    (frames * frame_step).min(PREFETCH_VIDEO_LOOKAHEAD_TICKS)
}

/// Кадр (48k) для отображения при данной задержке вывода. Чистая функция —
/// тесты 22-sync и 44. Внутри loop-региона вычитание wrap'ится по региону.
/// `dir` — направление транспорта (J4): при реверсе слышимое сейчас
/// находится ПОЗЖЕ по таймлайну (playhead едет влево), поэтому знак
/// компенсации задержки обратный.
pub fn video_display_frame(
    playhead: i64,
    latency_frames: i64,
    region: Option<(i64, i64)>,
    dir: i64,
) -> i64 {
    let lat = latency_frames.max(0) * dir.signum().clamp(-1, 1);
    match region {
        Some((rs, re)) if re > rs && playhead >= rs => {
            let span = re - rs;
            rs + (playhead - rs - lat).rem_euclid(span)
        }
        _ => (playhead - lat).max(0),
    }
}

#[allow(clippy::too_many_arguments)]
fn prefetch_loop(
    cache: Arc<BurstCache>,
    transport: Arc<Transport>,
    workers: Workers,
    project: Arc<Mutex<Project>>,
    preview_quality: Arc<Mutex<Quality>>,
    stop: Arc<AtomicBool>,
    wake: Arc<Wake>,
    loop_stats: Arc<LoopStats>,
    project_version: Arc<AtomicU64>,
) {
    // рендереры кэшируются до смены версии проекта: клонировать Project и
    // пересобирать графы каждые 100 мс было дорого даже в простое (п.1)
    let mut cached: Option<(u64, AudioRenderer, VideoRenderer)> = None;
    // состояние последнего прохода: в простое (пауза, ничего не менялось)
    // проход пропускается — раньше поток молотил 10 раз/с с десятками
    // захватов мьютекса кэша даже при полной тишине (п.1 v1.4). Прогрев
    // после seek/правки/смены качества сохраняется: любое изменение
    // состояния снимает пропуск.
    let mut last_state: Option<(i64, u64, u64, Quality)> = None;
    while !stop.load(Ordering::Acquire) {
        wake.wait_timeout(std::time::Duration::from_millis(100));
        if stop.load(Ordering::Acquire) {
            return;
        }
        {
            let state = (
                transport.playhead_frames(),
                project_version.load(Ordering::Acquire),
                transport.generation(),
                *preview_quality.lock().unwrap(),
            );
            if !transport.is_playing() && last_state == Some(state) {
                continue;
            }
            last_state = Some(state);
        }
        loop_stats.prefetch_passes.fetch_add(1, Ordering::Relaxed);
        let version = project_version.load(Ordering::Acquire);
        if cached.as_ref().map(|(v, _, _)| *v) != Some(version) {
            let p = project.lock().unwrap().clone();
            loop_stats.prefetch_rebuilds.fetch_add(1, Ordering::Relaxed);
            cached = Some((version, AudioRenderer::new(&p), VideoRenderer::new(&p)));
        }
        let (_, renderer, video) = cached.as_ref().unwrap();
        let generation = transport.generation();
        let head = transport.playhead_frames();
        prefetch_audio_pass(
            renderer,
            &cache,
            &workers.read().unwrap(),
            head..head + PREFETCH_AUDIO_LOOKAHEAD_FRAMES,
            Priority::NearPrefetch,
            generation,
        );
        // видео-lookahead: 2 c, но не больше половины бюджета кэша (п.19в:
        // 4К-кадры выселяли собственное окно)
        let t0 = transport.playhead();
        let q = *preview_quality.lock().unwrap();
        let step = video.grid.frame_step();
        let ahead = video_lookahead_ticks(q, step, cache.max_video_bytes());
        let range = TickRange::new(t0, Tick(t0.0 + ahead));
        for (media, src_pts) in video.wanted_frames(range) {
            if !cache.contains_video(source_id(media), Some(q), video_cache_idx(src_pts))
                && let Some(w) = workers.read().unwrap().get(&media)
            {
                w.submit(DecodeRequest {
                    kind: RequestKind::video_on_grid(src_pts, Some(q), step),
                    prio: Priority::NearPrefetch,
                    generation,
                });
            }
        }
    }
}

#[cfg(test)]
mod sync_tests {
    use super::video_display_frame;

    /// Тест 22-sync (п.30): компенсация задержки вывода при выборе видеокадра.
    #[test]
    fn t22_video_display_frame_compensates_latency() {
        // без задержки — тождество
        assert_eq!(video_display_frame(1000, 0, None, 1), 1000);
        // вычитание
        assert_eq!(video_display_frame(48_000, 4_800, None, 1), 43_200);
        // клэмп в 0 у старта
        assert_eq!(video_display_frame(100, 4_800, None, 1), 0);
        // отрицательная задержка не двигает вперёд
        assert_eq!(video_display_frame(1000, -500, None, 1), 1000);
        // wrap в loop-регионе: 50_500 − 1_000 → заворот к концу региона
        assert_eq!(video_display_frame(50_500, 1_000, Some((50_000, 60_000)), 1), 59_500);
        // внутри региона без пересечения границы
        assert_eq!(video_display_frame(55_000, 1_000, Some((50_000, 60_000)), 1), 54_000);
        // до региона — линейно
        assert_eq!(video_display_frame(10_000, 1_000, Some((50_000, 60_000)), 1), 9_000);
    }

    /// Тест 44 (J4): при реверсе знак компенсации задержки обратный —
    /// слышимое сейчас ПОЗЖЕ по таймлайну (playhead едет влево).
    #[test]
    fn t44_latency_sign_flips_in_reverse() {
        assert_eq!(video_display_frame(48_000, 4_800, None, -1), 52_800);
        assert_eq!(video_display_frame(48_000, 4_800, None, 1), 43_200);
        // в loop-регионе wrap в обе стороны
        assert_eq!(video_display_frame(59_500, 1_000, Some((50_000, 60_000)), -1), 50_500);
        assert_eq!(video_display_frame(1000, 0, None, -1), 1000, "нулевая задержка нейтральна");
    }
}

/// Varispeed-ядро колбэка (п.4): читает кадры источника через `next_frame`
/// с дробным шагом `step` (кадров источника на выходной кадр) и ЛИНЕЙНОЙ
/// интерполяцией. `phase` — дробное положение между prev и cur (переживает
/// колбэки). Возвращает число потреблённых кадров источника — оно двигает
/// playhead, поэтому скорость воспроизведения масштабируется сама.
/// Чистая функция — тест 32.
#[allow(clippy::too_many_arguments)]
pub fn varispeed_fill(
    next_frame: &mut dyn FnMut() -> Option<(f32, f32)>,
    step: f64,
    out_frames: usize,
    prev: &mut (f32, f32),
    cur: &mut (f32, f32),
    primed: &mut bool,
    phase: &mut f64,
    mut emit: impl FnMut(usize, (f32, f32)),
) -> i64 {
    let mut consumed = 0i64;
    // защита (тест 41): неположительный шаг НЕ крутит while phase>=1.0 —
    // раньше phase уходил в −∞ и интерполяция prev+(cur−prev)·a линейно
    // РАСХОДИЛАСЬ (опасно громко). step<=0 = тишина, ноль потребления.
    if step <= 0.0 || step.is_nan() {
        for i in 0..out_frames {
            emit(i, (0.0, 0.0));
        }
        return 0;
    }
    if !*primed {
        // прайминг: два кадра, чтобы интерполяция prev..cur была честной
        if let Some(f) = next_frame() {
            *prev = f;
            consumed += 1;
            *cur = next_frame().inspect(|_| consumed += 1).unwrap_or(f);
            *primed = true;
            *phase = 0.0;
        }
    }
    for i in 0..out_frames {
        *phase += step;
        while *phase >= 1.0 {
            *phase -= 1.0;
            *prev = *cur;
            match next_frame() {
                Some(f) => {
                    *cur = f;
                    consumed += 1;
                }
                None => {
                    // underrun: тянем последний кадр (без щелчка)
                    *cur = *prev;
                }
            }
        }
        let a = *phase as f32;
        emit(i, (prev.0 + (cur.0 - prev.0) * a, prev.1 + (cur.1 - prev.1) * a));
    }
    consumed
}

/// Покадровый разворот interleaved-стерео буфера: L/R остаются парой
/// (реверс-транспорт, тест 42).
pub fn reverse_frames(buf: &mut [f32]) {
    let n = buf.len() / AUDIO_CHANNELS;
    for i in 0..n / 2 {
        let j = n - 1 - i;
        for ch in 0..AUDIO_CHANNELS {
            buf.swap(i * AUDIO_CHANNELS + ch, j * AUDIO_CHANNELS + ch);
        }
    }
}

/// Шаг чтения кольца для cpal-колбэка (тест 45): всегда |rate| — колбэк
/// не знает про направление (материал в кольце уже развёрнут render-
/// потоком, инвариант №6 цел); None = |rate|≈0 → тишина, ноль потребления.
pub fn callback_step(device_step: f64, rate: f32) -> Option<f64> {
    let r = (rate as f64).abs();
    if r < 1e-3 { None } else { Some(device_step * r) }
}

#[cfg(test)]
mod varispeed_tests {
    use super::{callback_step, varispeed_fill};

    /// Тест 41 (J4): неположительный шаг безопасен — тишина, ноль
    /// потребления, фаза не уезжает (раньше step<0 давал расходящуюся
    /// экстраполяцию — опасно громко).
    #[test]
    fn t41_nonpositive_step_is_safe_silence() {
        for step in [-1.0f64, -0.25, 0.0] {
            let mut frames = (0..64).map(|i| (i as f32, -(i as f32)));
            let mut next = || frames.next();
            let (mut prev, mut cur, mut primed, mut phase) =
                ((1.0f32, 1.0f32), (2.0f32, 2.0f32), true, 0.25f64);
            let mut out = vec![(9.0f32, 9.0f32); 32];
            let consumed = varispeed_fill(
                &mut next,
                step,
                out.len(),
                &mut prev,
                &mut cur,
                &mut primed,
                &mut phase,
                |i, f| out[i] = f,
            );
            assert_eq!(consumed, 0, "step={step}: ничего не потреблено");
            assert!(out.iter().all(|&(l, r)| l == 0.0 && r == 0.0), "step={step}: тишина");
            assert_eq!(phase, 0.25, "step={step}: фаза не тронута");
        }
    }

    /// Тест 45 (J4): шаг колбэка — всегда |rate| (направление применяет
    /// render-поток); |rate|≈0 → None (тишина). Стерео-парность закрыта
    /// конструктивно: колбэк не забирает полкадра (slots >= 2).
    #[test]
    fn t45_callback_step_uses_abs_rate() {
        assert_eq!(callback_step(1.0, 1.5), Some(1.5));
        assert_eq!(callback_step(1.0, -1.5), Some(1.5), "реверс = тот же шаг по модулю");
        assert_eq!(callback_step(0.5, -2.0), Some(1.0));
        assert_eq!(callback_step(1.0, 0.0), None, "rate 0 — тишина");
        assert_eq!(callback_step(1.0, -0.0004), None);
    }

    fn run(rate: f64, out_frames: usize, src_len: usize) -> (Vec<(f32, f32)>, i64) {
        // источник-рампа: кадр n = (n, -n)
        let mut n = 0usize;
        let mut next = move || {
            if n < src_len {
                let v = n as f32;
                n += 1;
                Some((v, -v))
            } else {
                None
            }
        };
        let mut out = vec![(0.0, 0.0); out_frames];
        let (mut prev, mut cur, mut primed, mut phase) = ((0.0, 0.0), (0.0, 0.0), false, 0.0);
        let consumed = varispeed_fill(
            &mut next,
            rate,
            out_frames,
            &mut prev,
            &mut cur,
            &mut primed,
            &mut phase,
            |i, f| out[i] = f,
        );
        (out, consumed)
    }

    /// Тест 32 (п.4): потребление = out×rate (±1 кадр прайминга),
    /// линейная интерполяция exact на рампе, rate=1 — тождество.
    #[test]
    fn t32_varispeed_consumption_and_interpolation() {
        // rate=1: тождество
        let (out, consumed) = run(1.0, 100, 1000);
        assert_eq!(consumed, 102, "прайминг (2) + 100 шагов");
        for (i, f) in out.iter().enumerate() {
            assert_eq!(f.0, (i + 1) as f32, "rate=1 == кадр за кадром");
            assert_eq!(f.1, -((i + 1) as f32));
        }

        // rate=0.5: половинное потребление, интерполяция между кадрами exact
        let (out, consumed) = run(0.5, 100, 1000);
        assert!((consumed - 52).abs() <= 1, "0.5×100 + прайминг, вышло {consumed}");
        // рампа: значение выхода растёт ровно на 0.5 за кадр
        for w in out.windows(2) {
            let d = w[1].0 - w[0].0;
            assert!((d - 0.5).abs() < 1e-4, "линейная интерполяция рампы: шаг {d}");
        }

        // rate=2: двойное потребление
        let (_, consumed) = run(2.0, 100, 1000);
        assert!((consumed - 202).abs() <= 1, "2×100 + прайминг, вышло {consumed}");

        // источник иссяк: не паникуем, тянем последний кадр
        let (out, consumed) = run(1.0, 100, 20);
        assert_eq!(consumed, 20);
        assert_eq!(out[99].0, 19.0, "после конца — последний кадр");
    }
}
