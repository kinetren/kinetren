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
use crate::video_plan::{GpuBackendInfo, GpuExecError, VideoGpuExec};
use crate::worker::{DecodeRequest, DecodeStats, MediaWorker, RequestKind};
use crate::{Priority, source_id};
use kt_cache::{BurstCache, CacheConfig};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick, TickRange};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Кольцо на 4 бурста ≈ 85 мс при 1×. На высоких скоростях запас тает
/// пропорционально (при 8× это ~10 мс — меньше одного цикла сна
/// render-потока), поэтому ёмкость взята с запасом под ENGINE_RATE_MAX
/// (п.4 р.8).
const RING_CAPACITY_FRAMES: usize = AUDIO_BURST_FRAMES * 16;
/// Максимальная скорость воспроизведения (п.4 р.8: «второе дыхание»
/// шаттла до ±8×).
pub const ENGINE_RATE_MAX: f32 = 8.0;
/// Потолок мастер-громкости (+24 dB — верх фейдера UI).
pub const MASTER_GAIN_MAX: f32 = 16.0;
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
    /// Сглаженная задержка (EMA в render-потоке): сырая оценка от
    /// PipeWire/ALSA дрожит на ±кадр между колбэками → ключ кадра превью
    /// прыгал через ячейки сетки и назад — видимые рывки при честном
    /// декоде (п.1 раунда 5). Видео читает ЭТО поле.
    pub latency_smooth: AtomicI64,
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
    /// «Чёрный ящик» воспроизведения (Р.28, п.10): компактное кольцо
    /// событий транспорта и счётчиков — по нему ловят плавающие баги.
    pub flight: Arc<crate::flight::FlightRecorder>,
    pub transport: Arc<Transport>,
    pub shared: Arc<AudioShared>,
    pub meters: Arc<Meters>,
    /// Мастер-громкость (линейная, f32-биты), применяется в render-потоке.
    master_gain: Arc<AtomicU32>,
    /// Loop-регион в кадрах 48k (None = выключен).
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
    /// Часы показа: интерполяция между квантами playhead (см.
    /// display_extrapolation). (последний head, момент, EMA шага).
    video_clock: Mutex<(i64, std::time::Instant, i64)>,
    workers: Workers,
    natives: Natives,
    project: Arc<Mutex<Project>>,
    model_dirty: Arc<AtomicBool>,
    video: Mutex<VideoRenderer>,
    /// GPU-исполнитель видео-композита (Р.29): инжектится сверху
    /// (kt-app), живёт отдельно от VideoRenderer — пересоздание рендерера
    /// при правках модели его не теряет. DeviceLost «отравляет» до конца
    /// сессии → CPU-фолбэк + notice.
    video_gpu: Mutex<VideoGpuState>,
    preview_quality: Arc<Mutex<Quality>>,
    stop: Arc<AtomicBool>,
    render_thread: Option<std::thread::JoinHandle<()>>,
    prefetch_thread: Option<std::thread::JoinHandle<()>>,
    pub underruns: Arc<AtomicU64>,
    pub decode_stats: Arc<DecodeStats>,
    wake: Arc<Wake>,
    /// Версия проекта: prefetch пересобирает рендереры только по смене.
    project_version: Arc<AtomicU64>,
    /// «Содержимое источника изменилось» (Р.53, п.411) — ОТДЕЛЬНО от
    /// версии проекта. Префетч по ней просыпается, но рендереры НЕ
    /// пересобирает: план от параметров генератора не зависит (закон П).
    content_version: Arc<AtomicU64>,
    /// Счётчик поднятых потоков-декодеров (сторож t500/t502).
    worker_spawns: Arc<AtomicU64>,
    /// Значения генераторов, которые СЕЙЧАС стоят в живых источниках
    /// (Р.59, стадия 2). Спрашивать их у самих источников нельзя —
    /// ими эксклюзивно владеет поток воркера (инвариант 5), — а диф
    /// «что изменилось» нужен здесь.
    gen_states: Mutex<HashMap<MediaId, kt_model::GenState>>,
    /// Кэш графа для sample_levels_at (п.1): (версия, renderer, store).
    levels_probe: Mutex<Option<(u64, AudioRenderer, FxStateStore)>>,
    pub loop_stats: Arc<LoopStats>,
}

/// Состояние GPU-исполнителя превью (Р.29).
#[derive(Default)]
struct VideoGpuState {
    exec: Option<Arc<dyn VideoGpuExec>>,
    /// Some(причина) — DeviceLost/Validation: GPU отравлен до конца
    /// сессии, превью идёт CPU-фолбэком, UI показывает notice.
    poisoned: Option<String>,
}

/// Статус видео-бэкенда (для notice и строки перф-панели).
#[derive(Debug, Clone)]
pub enum VideoBackendStatus {
    Cpu,
    Gpu(GpuBackendInfo),
    Poisoned { info: GpuBackendInfo, reason: String },
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
            latency_smooth: AtomicI64::new(0),
            output_latency_frames: AtomicI64::new(0),
        });
        let meters = Arc::new(Meters { peak_l: AtomicU32::new(0), peak_r: AtomicU32::new(0) });
        let master_gain = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let loop_region: Arc<Mutex<Option<(i64, i64)>>> = Arc::new(Mutex::new(None));
        let decode_stats = Arc::new(DecodeStats::default());
        let mut workers = HashMap::new();
        // нативные размеры источников (Р.46, п.321): ступень декода живёт
        // в рендерере, а узнать натив можно только у открытого источника
        let mut nat = crate::video::SourceFacts::default();
        for (media, src) in sources {
            nat.observe(media, src.info());
            workers.insert(
                media,
                MediaWorker::spawn_with_stats(media, src, cache.clone(), decode_stats.clone()),
            );
        }
        let workers: Workers = Arc::new(std::sync::RwLock::new(workers));
        let natives: Natives = Arc::new(std::sync::RwLock::new(nat));
        let video = Mutex::new(VideoRenderer::new_with_facts(&project, &natives.read().unwrap()));
        let video_gpu = Mutex::new(VideoGpuState::default());
        let project = Arc::new(Mutex::new(project));
        let model_dirty = Arc::new(AtomicBool::new(true));
        let wake = Arc::new(Wake { lock: Mutex::new(()), cv: std::sync::Condvar::new() });
        let project_version = Arc::new(AtomicU64::new(0));
        let content_version = Arc::new(AtomicU64::new(0));
        let worker_spawns = Arc::new(AtomicU64::new(0));
        let loop_stats = Arc::new(LoopStats::default());
        let stop = Arc::new(AtomicBool::new(false));
        let underruns = Arc::new(AtomicU64::new(0));
        let flight = Arc::new(crate::flight::FlightRecorder::new());

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
            let flight_r = flight.clone();
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
                        flight: flight_r,
                    })
                })
                .expect("spawn audio render")
        };

        let preview_quality = Arc::new(Mutex::new(Quality { width: 640, height: 360 }));
        let prefetch_thread = {
            let wake_p = wake.clone();
            let stats_p = loop_stats.clone();
            let version_p = project_version.clone();
            let content_p = content_version.clone();
            let cache = cache.clone();
            let transport = transport.clone();
            let workers = workers.clone();
            let natives_p = natives.clone();
            let project = project.clone();
            let stop = stop.clone();
            let quality = preview_quality.clone();
            let region_p = loop_region.clone();
            std::thread::Builder::new()
                .name("prefetch".into())
                .spawn(move || {
                    prefetch_loop(
                        cache, transport, workers, natives_p, project, quality, stop, wake_p,
                        stats_p, version_p, content_p, region_p,
                    )
                })
                .expect("spawn prefetch")
        };

        let player = Player {
            cache,
            natives,
            flight,
            transport,
            shared,
            meters,
            master_gain,
            loop_region,
            video_clock: Mutex::new((0, std::time::Instant::now(), 0)),
            workers,
            project,
            model_dirty,
            video,
            video_gpu,
            preview_quality,
            stop,
            render_thread: Some(render_thread),
            prefetch_thread: Some(prefetch_thread),
            underruns,
            decode_stats,
            wake,
            project_version,
            content_version,
            worker_spawns,
            gen_states: Mutex::new(HashMap::new()),
            loop_stats,
            levels_probe: Mutex::new(None),
        };
        // источники генераторов поднимает САМ плеер: их адреса
        // производны от событий, и вызывающему их взять неоткуда
        let p = player.project.lock().unwrap().clone();
        player.sync_generators(&p);
        (player, consumer)
    }

    pub fn set_project(&self, p: Project) {
        self.project_version.fetch_add(1, Ordering::AcqRel);
        // состав генераторов — функция ПРОЕКТА, и считать её обязан тот,
        // кто проект принимает: адреса производны от событий, и
        // вызывающему их взять неоткуда
        self.sync_generators(&p);
        self.wake_after_project_swap(p);
    }

    fn wake_after_project_swap(&self, p: Project) {
        *self.video.lock().unwrap() =
            VideoRenderer::new_with_facts(&p, &self.natives.read().unwrap());
        *self.project.lock().unwrap() = p;
        self.model_dirty.store(true, Ordering::Release);
        self.wake.notify();
    }

    pub fn project(&self) -> Project {
        self.project.lock().unwrap().clone()
    }

    /// Сколько раз проект подменялся целиком. Каждая подмена — клон всего
    /// проекта и пересборка обоих рендереров, то есть остановка
    /// аудио-render-потока; на большом монтаже это слышно (Р.43, п.296).
    /// Наружу отдаётся ради теста «правка удобства не пересобирает движок».
    pub fn project_version(&self) -> u64 {
        self.project_version.load(Ordering::Acquire)
    }

    /// Скорость воспроизведения (п.4, ползунок как в Vegas).
    /// Смена скорости сбрасывает окно экстраполяции: оно выучено на
    /// шагах ПРЕЖНЕЙ скорости и после входа в слоу-мо ещё ~170 мс
    /// разрешает вчетверо больший забег вперёд — с последующим снапом
    /// назад ровно в момент включения (п.14 р.14).
    pub fn set_playback_rate(&self, rate: f32) {
        if let Ok(mut vc) = self.video_clock.lock() {
            vc.2 = 0;
            vc.1 = std::time::Instant::now();
        }
        // отрицательное = reverse-транспорт (J4 v1.4); 0 = стоп потребления
        // (тишина, playhead стоит). Колбэк использует |rate|, знак читает
        // только render-поток.
        self.shared
            .playback_rate
            .store(rate.clamp(-ENGINE_RATE_MAX, ENGINE_RATE_MAX).to_bits(), Ordering::Release);
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
        self.flight.push(crate::flight::Ev::Play, self.transport.playhead_frames(), 0);
        self.transport.set_playing(true);
        self.wake.notify();
    }

    pub fn pause(&self) {
        self.flight.push(crate::flight::Ev::Pause, self.transport.playhead_frames(), 0);
        self.transport.set_playing(false);
        self.wake.notify();
    }

    /// Мастер-громкость превью (линейная; mute = 0).
    /// Мастер-громкость превью. ПОТОЛОК ОБЯЗАТЕЛЕН (п.7 р.9): сбой в UI
    /// однажды прислал сюда 10^20 (расходящаяся анимация фейдера), и это
    /// умножило целый burst — «по ушам больно». NaN/inf гасим в 1.0.
    pub fn set_master_gain(&self, linear: f32) {
        let g = if linear.is_finite() { linear.clamp(0.0, MASTER_GAIN_MAX) } else { 1.0 };
        self.master_gain.store(g.to_bits(), Ordering::Release);
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
        self.worker_spawns.fetch_add(1, Ordering::AcqRel);
        self.remember_native(media, source.as_ref());
        let w = MediaWorker::spawn_with_stats(
            media,
            source,
            self.cache.clone(),
            self.decode_stats.clone(),
        );
        if let Some(old) = self.workers.write().unwrap().insert(media, w) {
            old.shutdown();
        }
        self.rebuild_video_renderer();
    }

    /// Заменить источник медиа НА ХОДУ (Р.33 п.10: пропавший файл вернулся
    /// на диск — плейсхолдер уступает место настоящему). Порядок важен:
    /// сперва гасим воркер, потом выбиваем его кадры из кэша, и только
    /// после этого поднимаем новый — иначе старый успел бы дописать в кэш
    /// кадр заглушки уже после инвалидации. Транспорт, позиция и кэш
    /// прочих источников не трогаются.
    pub fn replace_media(&self, media: kt_model::MediaId, source: Box<dyn kt_media::MediaSource>) {
        self.worker_spawns.fetch_add(1, Ordering::AcqRel);
        if let Some(old) = self.workers.write().unwrap().remove(&media) {
            old.shutdown();
        }
        // натив нового файла может отличаться от заглушки — ступень декода
        // обязана пересчитаться на следующей пересборке рендерера (Р.46)
        self.remember_native(media, source.as_ref());
        self.cache.invalidate_source(kt_cache::SourceId(media.0));
        let w = MediaWorker::spawn_with_stats(
            media,
            source,
            self.cache.clone(),
            self.decode_stats.clone(),
        );
        self.workers.write().unwrap().insert(media, w);
        // натив мог смениться (заглушка → настоящий файл) — рендерер
        // превью обязан пересчитать ступени, не дожидаясь правки проекта
        self.rebuild_video_renderer();
        // состав источников изменился — префетч обязан проснуться и
        // прогреть кэш заново (в простое он пропускает проходы, пока
        // сигнатура состояния та же, и без бампа версии не заметил бы).
        // Версия СОДЕРЖИМОГО, а не проекта: свою пересборку рендерера
        // подмена уже сделала выше, а плану источник безразличен.
        self.content_version.fetch_add(1, Ordering::AcqRel);
        self.wake.notify();
    }

    /// Перенастроить источник под новый путь БЕЗ пересоздания (Р.53,
    /// п.411): поток декодера жив, рендереры целы, проект не клонируется.
    /// Вызывающий обязан спросить `kt_media::factory::can_retune` — иначе
    /// источник отвергнет путь, и правка молча не применится.
    ///
    /// `false` — воркера нет (медиа не поднято): вызывающий делает
    /// обычную подмену.
    pub fn retune_media(&self, media: kt_model::MediaId, path: &str) -> bool {
        let workers = self.workers.read().unwrap();
        let Some(w) = workers.get(&media) else { return false };
        w.retune(path);
        drop(workers);
        // содержимое источника изменилось: префетч просыпается и греет
        // кэш заново, рендереры НЕ пересобираются
        self.content_version.fetch_add(1, Ordering::AcqRel);
        self.wake.notify();
        true
    }

    /// Привести ИСТОЧНИКИ ГЕНЕРАТОРОВ в соответствие проекту (Р.59,
    /// стадия 2).
    ///
    /// Диф по АДРЕСУ: появившийся генератор получает поток, исчезнувший
    /// его теряет, у оставшегося значения доезжают перенастройкой — поток
    /// жив, кэш чужих источников цел, рендереры не пересобираются.
    /// Пересоздание остаётся ровно на один случай: паспорт источника
    /// изменился (SMPTE со включённым тоном становится ЗВУЧАЩИМ медиа, а
    /// это уже другой набор клипов), и тогда `can_retune_gen` честно
    /// говорит «нет».
    pub fn sync_generators(&self, p: &Project) {
        let wanted = crate::gen_sources(p);
        let want_ids: std::collections::HashSet<MediaId> =
            wanted.iter().map(|(id, _)| *id).collect();
        let stale: Vec<MediaId> = self
            .workers
            .read()
            .unwrap()
            .keys()
            .filter(|m| m.0 & kt_model::GEN_TAG != 0 && !want_ids.contains(m))
            .copied()
            .collect();
        for m in stale {
            self.remove_media(m);
            self.cache.invalidate_source(kt_cache::SourceId(m.0));
        }
        for (addr, state) in wanted {
            let live = self.gen_states.lock().unwrap().get(&addr).cloned();
            match live {
                Some(old) if old == state => {}
                Some(old) if kt_media::factory::can_retune_gen(&old, &state) => {
                    self.update_gen_at(addr, state, Vec::new());
                }
                Some(_) => {
                    self.gen_states.lock().unwrap().insert(addr, state.clone());
                    self.replace_media(addr, kt_media::factory::open_gen(&state));
                }
                None => {
                    self.gen_states.lock().unwrap().insert(addr, state.clone());
                    self.add_media(addr, kt_media::factory::open_gen(&state));
                }
            }
        }
    }

    /// Новые ЗНАЧЕНИЯ генератора одного события — без пересборки
    /// рендереров и без бампа версии проекта (Р.59, стадия 2; тот же
    /// закон П, что у `set_media_path`).
    ///
    /// `project` нужен, чтобы узнать ЛЕНТЫ-ПРЕДКИ события: их
    /// суб-композиты мемоизированы по `content_hash` модели, а модель
    /// правкой значений не двигается — без явной инвалидации лента
    /// продолжала бы отдавать старую картинку.
    pub fn update_gen(
        &self,
        clip: kt_model::ClipId,
        kind: kt_model::GeneratorKind,
        auto: Vec<kt_model::ParamCurve>,
    ) -> bool {
        let addr = crate::gen_source_addr(clip);
        let state = kt_model::GenState { kind, auto };
        let live = self.gen_states.lock().unwrap().get(&addr).cloned();
        match live {
            Some(old) if !kt_media::factory::can_retune_gen(&old, &state) => false,
            Some(_) => {
                // значения хранимого проекта обязаны остаться правдивыми:
                // `Player::project()` читают экспорт и диагностика
                let mut p = self.project.lock().unwrap();
                let put = |cs: &mut Vec<kt_model::ClipEvent>| {
                    for c in cs.iter_mut().filter(|c| c.id == clip) {
                        c.r#gen = Some(state.kind.clone());
                        c.gen_auto = state.auto.clone();
                    }
                };
                put(&mut p.clips);
                for i in 0..p.sequences.len() {
                    let mut cs = std::mem::take(&mut p.sequences[i].clips);
                    put(&mut cs);
                    p.sequences[i].clips = cs;
                }
                let stale: Vec<kt_cache::SourceId> = crate::reel_chain_of_clip(&p, clip)
                    .into_iter()
                    .map(crate::nest_source_id)
                    .collect();
                // ВСЕ адреса, которые несут состояние этого события
                // (Р.59, стадия 3). У экземпляра с переопределениями
                // источник СВОЙ — адрес посолен набором, — и ретюн
                // обязан доехать до каждого: правка кегля в титре
                // шаблона иначе не дошла бы до второго выпуска до
                // первой пересборки плана. Список считает тот же
                // `gen_sources`, что поднимает источники.
                let wanted = crate::gen_sources(&p);
                drop(p);
                self.update_gen_at(addr, state, stale.clone());
                for (a, st) in wanted {
                    if a == addr {
                        continue;
                    }
                    let live = self.gen_states.lock().unwrap().get(&a).cloned();
                    match live {
                        Some(o) if o == st => {}
                        Some(o) if kt_media::factory::can_retune_gen(&o, &st) => {
                            self.update_gen_at(a, st, stale.clone());
                        }
                        Some(_) => {
                            self.gen_states.lock().unwrap().insert(a, st.clone());
                            self.replace_media(a, kt_media::factory::open_gen(&st));
                        }
                        None => {}
                    }
                }
                true
            }
            None => false,
        }
    }

    fn update_gen_at(
        &self,
        addr: MediaId,
        state: kt_model::GenState,
        stale: Vec<kt_cache::SourceId>,
    ) {
        self.gen_states.lock().unwrap().insert(addr, state.clone());
        let workers = self.workers.read().unwrap();
        let Some(w) = workers.get(&addr) else { return };
        w.retune_gen(state, stale);
        drop(workers);
        self.content_version.fetch_add(1, Ordering::AcqRel);
        self.wake.notify();
    }

    /// Попросить кадр у воркера напрямую (Р.53): тот же продуктовый
    /// путь, что у `compose_preview_into`, но без композита — нужен
    /// диагностике и харнесс-тестам, которые проверяют «правка доехала
    /// до картинки».
    pub fn request_video(&self, media: kt_model::MediaId, pts: Tick, q: Quality) {
        let step = self.video.lock().unwrap().grid.frame_step();
        if let Some(w) = self.workers.read().unwrap().get(&media) {
            w.submit(DecodeRequest {
                kind: RequestKind::video_on_grid(pts, Some(q), step),
                prio: Priority::Playback,
                generation: self.transport.generation(),
            });
        }
    }

    pub fn content_version(&self) -> u64 {
        self.content_version.load(Ordering::Acquire)
    }

    /// Сколько раз за жизнь плеера поднимался поток-декодер (Р.53):
    /// сторож того, что правка параметров генератора НЕ пересоздаёт
    /// источник (t500, t502). В продукте не читается.
    pub fn worker_spawns(&self) -> u64 {
        self.worker_spawns.load(Ordering::Acquire)
    }

    /// Правка пути медиа в хранимом проекте БЕЗ бампа версий и пересборки
    /// (Р.53, п.412): `Player::project()` обязан оставаться правдивым,
    /// но плану путь безразличен — рендереры знают только `MediaId`.
    pub fn set_media_path(&self, media: kt_model::MediaId, path: &str) {
        let mut p = self.project.lock().unwrap();
        if let Some(m) = p.media.iter_mut().find(|m| m.id == media) {
            m.path = path.to_string();
        }
    }

    /// Убрать медиа (симметрия RemoveMedia): воркер гасится, кэш других
    /// источников не трогается.
    pub fn remove_media(&self, media: kt_model::MediaId) {
        if let Some(w) = self.workers.write().unwrap().remove(&media) {
            w.shutdown();
        }
        self.natives.write().unwrap().forget(media);
    }

    /// Пересобрать рендерер превью под текущие нативы (Р.46, п.321):
    /// префетч-поток делает это сам по версии проекта, а рендерер плеера
    /// живёт до подмены модели — без этого вернувшийся файл не менял бы
    /// ступень до первой правки таймлайна.
    fn rebuild_video_renderer(&self) {
        let p = self.project.lock().unwrap().clone();
        *self.video.lock().unwrap() =
            VideoRenderer::new_with_facts(&p, &self.natives.read().unwrap());
    }

    /// Запомнить факты источника (Р.46 п.321 — натив, Р.56 п.448 —
    /// процедурность). Источник без видео не получает ни того, ни другого.
    fn remember_native(&self, media: kt_model::MediaId, src: &dyn kt_media::MediaSource) {
        self.natives.write().unwrap().observe(media, src.info());
    }

    /// Качества декода видео-клипов при текущем Quality превью (Р.46,
    /// п.322): полоски прогретости обязаны спрашивать coverage по НИМ, а
    /// не по Quality — иначе прогретый клип показан холодным.
    pub fn video_decode_qualities(&self) -> Vec<(kt_model::ClipId, Quality)> {
        let q = self.preview_quality();
        self.video.lock().unwrap().video_decode_qualities(q)
    }

    /// Прогретость вложений в тиках их содержимого (Р.50, Б6): у ленты
    /// источника нет, и полоска обязана спрашивать её СОДЕРЖИМОЕ.
    pub fn nest_coverage(&self) -> Vec<(kt_model::ClipId, Vec<std::ops::Range<i64>>)> {
        let q = self.preview_quality();
        self.video.lock().unwrap().nest_coverage(&self.cache, q)
    }

    /// Момент, который сейчас СЛЫШНО (для выбора видеокадра превью):
    /// playhead минус задержка вывода. При паузе или без вывода — playhead
    /// как есть; loop-регион учитывается wrap'ом (п.30 — рассинхрон A/V).
    pub fn video_playhead(&self) -> Tick {
        if !self.transport.is_playing() {
            return self.transport.playhead();
        }
        let latency = self.shared.latency_smooth.load(Ordering::Acquire);
        let head = self.transport.playhead_frames();
        let region = *self.loop_region.lock().unwrap();
        let rate = f32::from_bits(self.shared.playback_rate.load(Ordering::Acquire));
        let dir: i64 = if rate < 0.0 { -1 } else { 1 };
        // интерполяция между квантами аудио-часов (display_extrapolation)
        let extra = {
            let mut vc = self.video_clock.lock().unwrap();
            let now = std::time::Instant::now();
            if head != vc.0 {
                let step = (head - vc.0).abs();
                // скачок больше секунды = seek/wrap, EMA не портим
                if step < AUDIO_SAMPLE_RATE as i64 {
                    vc.2 = if vc.2 == 0 { step } else { (vc.2 * 3 + step) / 4 };
                }
                vc.0 = head;
                vc.1 = now;
            }
            let elapsed = (now.duration_since(vc.1).as_secs_f64()
                * AUDIO_SAMPLE_RATE as f64
                * rate.abs() as f64) as i64;
            display_extrapolation(elapsed, vc.2)
        };
        Tick::from_audio_frames(video_display_frame(head + extra * dir, latency, region, dir, rate))
    }

    /// UI: составить кадр превью для момента t (выравнивается по сетке fps).
    /// Недостающие слои заказываются с Priority::Playback; вернётся
    /// (буфер RGBA, размер, все ли слои готовы).
    /// Сетка кадров проекта — ключ кадра превью для UI (тест 39): тик,
    /// выровненный по ней, меняется только на границе видеокадра.
    pub fn video_grid(&self) -> kt_time::FpsGrid {
        self.video.lock().unwrap().grid
    }

    /// Инжекция GPU-исполнителя видео (Р.29). None — принудительный CPU.
    /// Снимает «отравление» (новый движок = новая попытка).
    pub fn set_video_gpu(&self, exec: Option<Arc<dyn VideoGpuExec>>) {
        let mut g = self.video_gpu.lock().unwrap();
        g.exec = exec;
        g.poisoned = None;
    }

    /// Статус видео-бэкенда для notice/перф-панели.
    pub fn video_backend(&self) -> VideoBackendStatus {
        let g = self.video_gpu.lock().unwrap();
        match (&g.exec, &g.poisoned) {
            (Some(e), None) => VideoBackendStatus::Gpu(e.info().clone()),
            (Some(e), Some(r)) => {
                VideoBackendStatus::Poisoned { info: e.info().clone(), reason: r.clone() }
            }
            (None, _) => VideoBackendStatus::Cpu,
        }
    }

    /// Вариант compose_preview с переиспользуемым буфером (превью 60 раз/с:
    /// две аллокации полного RGBA-кадра на каждый кадр — заметная часть
    /// стоимости кадра UI, п.1 раунда 5).
    pub fn compose_preview_into(&self, t: Tick, out: &mut Vec<u8>) -> (Quality, bool) {
        let q = self.preview_quality();
        let mut video = self.video.lock().unwrap();
        let aligned = video.grid.align_floor(t);
        let generation = self.transport.generation();
        let step = video.grid.frame_step();
        // качество декода приходит ИЗ ЗАКАЗА (Р.46, п.318): клип с зумящей
        // цепочкой греется крупнее Quality, и просить надо ровно это
        for (media, src_pts, dq) in video.layers_at(aligned, q) {
            let idx = video_cache_idx(src_pts);
            // contains, не get: проба не должна освежать LRU (п.19)
            if !self.cache.contains_video(source_id(media), Some(dq), idx)
                && let Some(w) = self.workers.read().unwrap().get(&media)
            {
                w.submit(DecodeRequest {
                    kind: RequestKind::video_on_grid(src_pts, Some(dq), step),
                    prio: Priority::Playback,
                    generation,
                });
            }
        }
        // GPU-исполнитель по умолчанию (Р.29); TooLarge — CPU только на
        // этот план, DeviceLost/Validation — «отравить» до конца сессии
        if let Some(exec) = {
            let g = self.video_gpu.lock().unwrap();
            if g.poisoned.is_none() { g.exec.clone() } else { None }
        } {
            let gone = {
                let plan = video.frame_plan(&self.cache, aligned, q);
                match exec.execute_into(&plan, out) {
                    Ok(complete) => return (q, complete),
                    Err(GpuExecError::TooLarge { .. }) => None,
                    Err(e) => Some(format!("{e:?}")),
                }
            };
            if let Some(reason) = gone {
                self.video_gpu.lock().unwrap().poisoned = Some(reason);
            }
        }
        let complete = video.compose_from_cache_into(&self.cache, aligned, q, out);
        (q, complete)
    }

    pub fn compose_preview(&self, t: Tick) -> (Vec<u8>, Quality, bool) {
        let mut out = Vec::new();
        let (q, complete) = self.compose_preview_into(t, &mut out);
        (out, q, complete)
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
    flight: Arc<crate::flight::FlightRecorder>,
}

/// Карта воркеров: RwLock, чтобы add_media/remove_media добавляли декодеры
/// БЕЗ пересоздания плеера (кэш/транспорт/кольцо переживают правки медиа —
/// регрессия п.27: раньше добавление файла пересоздавало Player и сбрасывало
/// весь прогретый кэш). Пишущие захваты — только короткие, из UI-потока.
type Workers = Arc<std::sync::RwLock<HashMap<MediaId, MediaWorker>>>;
/// Факты об открытых источниках (Р.46 п.321 — натив, Р.56 п.448 —
/// процедурность): их знает только открытый источник, а нужны они
/// рендереру на каждой пересборке.
type Natives = Arc<std::sync::RwLock<crate::video::SourceFacts>>;

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
        flight,
    } = ctx;
    let mut store = FxStateStore::new();
    let mut renderer = AudioRenderer::new(&project.lock().unwrap().clone());
    renderer.sync_fx(&mut store);
    let mut fetcher = CacheFetcher::new(cache.clone());
    // локальная позиция рендера (кадры); может опережать playhead на кольцо
    let mut render_head: i64 = transport.playhead_frames();
    let mut last_generation = transport.generation();
    let mut last_dir: i64 = 1;
    // «вооружённость» loop-заворота — семантика транспорта (wrap только
    // изнутри региона, п.132): seek за конец (для реверса — за начало)
    // развооружает до следующего прыжка, и рендер/префетч/транспорт видят
    // effective_region = None (линейная игра того, что под курсором)
    let mut last_region: Option<(i64, i64)> = *loop_region.lock().unwrap();
    let mut loop_armed = loop_arm_after_jump(render_head, last_region, 1);
    let mut last_effective_some = loop_armed && last_region.is_some();
    let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
    // счётчики «чёрного ящика» (п.10): дельты за секунду, а не абсолюты
    let (mut last_ru, mut last_cu, mut last_warm) = (0u64, 0u64, 0u64);
    let mut warmups = 0u64;
    let mut last_snap_ms = 0u64;
    let mut last_head_seen = render_head;
    let mut last_head_ms = 0u64;

    while !stop.load(Ordering::Acquire) {
        if dirty.swap(false, Ordering::AcqRel) {
            renderer = AudioRenderer::new(&project.lock().unwrap().clone());
            renderer.sync_fx(&mut store); // state переживает пересборку (инв. №3)
        }
        let generation = transport.generation();
        let generation_changed = generation != last_generation;
        if generation_changed {
            last_generation = generation;
            store.reset_all(); // pre-roll при следующем чтении
            render_head = transport.playhead_frames();
            shared.consumed_frames.store(0, Ordering::Release);
            flight.push(crate::flight::Ev::Seek, render_head, 0);
        }

        {
            // EMA задержки вывода (см. AudioShared::latency_smooth)
            let raw = shared.output_latency_frames.load(Ordering::Acquire);
            let sm = shared.latency_smooth.load(Ordering::Acquire);
            shared.latency_smooth.store(sm + (raw - sm) / 8, Ordering::Release);
        }
        let region = *loop_region.lock().unwrap();
        // направление транспорта — знак playback_rate (J4 v1.4). Смена
        // знака = мини-seek без бампа generation: сброс FX, кольца и
        // render_head (в кольце лежит материал прежнего направления).
        let rate_now = f32::from_bits(shared.playback_rate.load(Ordering::Acquire));
        let dir: i64 = if rate_now < 0.0 { -1 } else { 1 };
        let dir_changed = dir != last_dir;
        if dir_changed {
            flight.push(crate::flight::Ev::Dir, dir, 0);
            last_dir = dir;
            store.reset_all();
            render_head = transport.playhead_frames();
            shared.consumed_frames.store(0, Ordering::Release);
            shared.flush.store(true, Ordering::Release);
        }
        // пересчёт вооружённости — только на прыжках (seek/направление/
        // регион): при armed-лупе render_head растёт линейно, о
        // принадлежности региону говорит playhead транспорта
        if generation_changed || dir_changed || region != last_region {
            if region != last_region {
                match region {
                    Some((rs, re)) => flight.push(crate::flight::Ev::Region, rs, re),
                    None => flight.push(crate::flight::Ev::Region, -1, 0),
                }
            }
            loop_armed = loop_arm_after_jump(transport.playhead_frames(), region, dir);
            last_region = region;
        }
        // единая правда для рендера, префетча и транспорта: развооружённый
        // loop для всех потребителей выглядит как отсутствие региона
        let effective_region = if loop_armed { region } else { None };
        let effective_some = effective_region.is_some();
        if effective_some != last_effective_some && !generation_changed && !dir_changed {
            // флип Some↔None на лету (границу региона перетащили через
            // playhead, Loop выключили/включили при игре) = мини-seek без
            // бампа generation: кольцо и render_head жили в другой системе
            // координат (armed render_head опережает playhead на круги)
            flight.push(crate::flight::Ev::Reset, transport.playhead_frames(), 0);
            store.reset_all();
            render_head = transport.playhead_frames();
            shared.consumed_frames.store(0, Ordering::Release);
            shared.flush.store(true, Ordering::Release);
        }
        last_effective_some = effective_some;
        // продвигаем playhead на потреблённое колбэком (со знаком
        // направления; wrap при loop работает в обе стороны — тест 43)
        let consumed = shared.consumed_frames.swap(0, Ordering::AcqRel);
        if consumed > 0 {
            let before = transport.playhead_frames();
            transport.advance_frames_wrapped(consumed * dir, effective_region);
            let after = transport.playhead_frames();
            // заворот региона: голова прыгнула ПРОТИВ направления игры
            if (dir > 0 && after < before) || (dir < 0 && after > before) {
                flight.push(crate::flight::Ev::Wrap, after, 0);
            }
        }
        // раз в секунду — снимок дельт счётчиков; в тишине не пишется
        // ничего (правило 3 «чёрного ящика»)
        {
            let now = flight.now_ms();
            let head = transport.playhead_frames();
            if now.saturating_sub(last_snap_ms) >= 1000 {
                last_snap_ms = now;
                let ru = underruns.load(Ordering::Relaxed);
                let cu = shared.callback_underruns.load(Ordering::Relaxed);
                flight.push_snap(crate::flight::Snap {
                    render_underruns: ru - last_ru,
                    callback_underruns: cu - last_cu,
                    warmup_misses: warmups - last_warm,
                    head_frames: head,
                });
                (last_ru, last_cu, last_warm) = (ru, cu, warmups);
            }
            // залипание: играем, а голова стоит (дефект 8.3 — «видны
            // фреймы, звука нет, воспроизведение замирает»)
            if transport.is_playing() {
                if head != last_head_seen {
                    last_head_seen = head;
                    last_head_ms = now;
                } else if now.saturating_sub(last_head_ms) >= 1000 {
                    flight.push(
                        crate::flight::Ev::Stall,
                        now.saturating_sub(last_head_ms) as i64,
                        head,
                    );
                }
            } else {
                last_head_seen = transport.playhead_frames();
                last_head_ms = now;
            }
        }

        if transport.is_playing() {
            let gain = f32::from_bits(master_gain.load(Ordering::Acquire));
            // заказ ближайшего аудио с Playback-приоритетом (идемпотентно);
            // при loop заказываем через wrap-отображение
            match effective_region {
                Some((rs, re))
                    if re > rs && (if dir < 0 { render_head <= re } else { render_head >= rs }) =>
                {
                    let tl = rs + (render_head - rs).rem_euclid(re - rs);
                    // на скорости >1 окно упреждения растягивается: иначе
                    // при 8× 200 мс превращаются в 25 мс реального времени
                    let look =
                        (PLAYBACK_AUDIO_LOOKAHEAD_FRAMES as f32 * rate_now.abs().max(1.0)) as i64;
                    let ahead = look.min(re - rs);
                    if dir < 0 {
                        // реверс: греем ПОЗАДИ головы, заворот в хвост региона
                        let start1 = (tl - ahead).max(rs);
                        prefetch_audio_pass(
                            &renderer,
                            &cache,
                            &workers.read().unwrap(),
                            start1..tl,
                            Priority::Playback,
                            generation,
                            dir < 0,
                        );
                        let rest = ahead - (tl - start1);
                        if rest > 0 {
                            prefetch_audio_pass(
                                &renderer,
                                &cache,
                                &workers.read().unwrap(),
                                re - rest..re,
                                Priority::Playback,
                                generation,
                                dir < 0,
                            );
                        }
                    } else {
                        let end1 = (tl + ahead).min(re);
                        prefetch_audio_pass(
                            &renderer,
                            &cache,
                            &workers.read().unwrap(),
                            tl..end1,
                            Priority::Playback,
                            generation,
                            dir < 0,
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
                                dir < 0,
                            );
                        }
                    }
                }
                _ => {
                    let win = if dir < 0 {
                        (render_head
                            - (PLAYBACK_AUDIO_LOOKAHEAD_FRAMES as f32 * rate_now.abs().max(1.0))
                                as i64)
                            .max(0)..render_head
                    } else {
                        render_head
                            ..render_head
                                + (PLAYBACK_AUDIO_LOOKAHEAD_FRAMES as f32 * rate_now.abs().max(1.0))
                                    as i64
                    };
                    prefetch_audio_pass(
                        &renderer,
                        &cache,
                        &workers.read().unwrap(),
                        win,
                        Priority::Playback,
                        generation,
                        dir < 0,
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
                    let st = match effective_region {
                        // зеркало forward-guard'а: реверс входит в регион
                        // СПРАВА (armed-подход выше re играет линейно), а
                        // уйдя ниже rs, продолжает фолдиться — guard `> rs`
                        // здесь гасил луп через круг (п.133)
                        Some((rs, re)) if re > rs && render_head <= re => {
                            renderer.render_looped_raw(&mut fetcher, rs..re, start, &mut chunk)
                        }
                        _ => renderer.render_frames_raw(&mut fetcher, start, &mut chunk),
                    };
                    reverse_frames(&mut chunk);
                    render_head = start;
                    st
                } else {
                    let st = match effective_region {
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
                warmups += stats.warmup_misses;
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
/// Ключ показанного кадра НЕ ЕДЕТ НАЗАД (п.14 р.14).
///
/// Возвращает (ключ, пропущено ячеек вперёд, был ли откат). Назад ключ
/// уезжает ТОЛЬКО при смене поколения транспорта (seek) или loop-wrap —
/// то есть по построению, а не «если откат меньше двух ячеек». Прежний
/// допуск был задан в ячейках и слеп к скорости: при 0.25× ячейка живёт
/// 8 UI-кадров, любой скачок EMA задержки перепрыгивал допуск и картинка
/// откатывалась на пару кадров назад — ровно то, на что жаловались.
pub fn preview_key_step(
    prev: Option<i64>,
    key: i64,
    step: i64,
    dir: i64,
    reanchor: bool,
) -> (i64, u64, bool) {
    let Some(prev) = prev.filter(|_| !reanchor) else { return (key, 0, false) };
    // «назад» — это против направления воспроизведения: в реверсе ключ
    // обязан УБЫВАТЬ, и убывание откатом не является
    let ahead = (key - prev) * dir.signum();
    if ahead < 0 {
        return (prev, 0, true);
    }
    let skipped = if step > 0 && ahead > step { ((ahead / step) - 1).max(0) as u64 } else { 0 };
    (key, skipped, false)
}

/// Позиция ПОКАЗА: из позиции таймлайна вычитается задержка вывода.
///
/// `latency_frames` — задержка в кадрах WALL-CLOCK (её так меряет
/// cpal-обвязка), а `playhead` — позиция ТАЙМЛАЙНА. За `latency` секунд
/// реального времени таймлайн уходит на `latency·|rate|`, поэтому
/// вычитать надо масштабированную величину (п.14 р.14). Раньше
/// вычиталась полная: при 0.25× это вчетверо больше нужного, а главное —
/// джиттер EMA задержки (сотни кадров) превышал ход показа за UI-кадр
/// (~200 кадров при 0.25×) и разворачивал картинку НАЗАД.
pub fn video_display_frame(
    playhead: i64,
    latency_frames: i64,
    region: Option<(i64, i64)>,
    dir: i64,
    rate: f32,
) -> i64 {
    let scaled = (latency_frames.max(0) as f64 * rate.abs().max(0.0) as f64) as i64;
    let lat = scaled * dir.signum().clamp(-1, 1);
    match region {
        // условие двустороннее, как у advance_frames_wrapped (p == e —
        // внутри): playhead за регионом (seek правее конца, реверс-подход
        // справа) показывается линейно, а не фолдится в регион (п.132)
        Some((rs, re)) if re > rs && playhead >= rs && playhead <= re => {
            let span = re - rs;
            rs + (playhead - rs - lat).rem_euclid(span)
        }
        _ => (playhead - lat).max(0),
    }
}

/// Вооружён ли loop-заворот после прыжка (seek / смена направления или
/// региона): true, если движение из `head` в сторону `dir` может естественно
/// пересечь конец региона ИЗНУТРИ — семантика advance_frames_wrapped
/// (инклюзивность граней: p == e внутри, для реверса p == s внутри).
/// Развооружённый loop (seek за конец при forward, за начало при reverse)
/// играет линейно то, что под курсором, до следующего прыжка (п.132).
pub fn loop_arm_after_jump(head: i64, region: Option<(i64, i64)>, dir: i64) -> bool {
    match region {
        Some((rs, re)) if re > rs => {
            if dir < 0 {
                head >= rs
            } else {
                head <= re
            }
        }
        _ => true,
    }
}

/// Окно упреждения префетча с учётом loop-региона: за концом региона греть
/// нечего — воспроизведение туда не пойдёт, а вытесненная голова региона
/// стоит промахов на каждом круге (prefetch-thrash перф-панели, п.167).
/// Курсор вне региона — линейное окно. Длина не больше круга: при коротком
/// регионе 2 с упреждения означали бы десятки заказов одного и того же.
pub fn lookahead_windows(
    head: i64,
    len: i64,
    region: Option<(i64, i64)>,
) -> Vec<std::ops::Range<i64>> {
    match region {
        Some((rs, re)) if re > rs && head >= rs && head <= re => {
            let span = re - rs;
            let h = rs + (head - rs).rem_euclid(span);
            let want = len.min(span);
            let first = (re - h).min(want);
            let mut v: Vec<std::ops::Range<i64>> = std::iter::once(h..h + first).collect();
            if want > first {
                v.push(rs..rs + (want - first));
            }
            v
        }
        // (не `vec![a..b]` — clippy принимает его за диапазон Vec'а)
        _ => std::iter::once(head..head + len).collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn prefetch_loop(
    cache: Arc<BurstCache>,
    transport: Arc<Transport>,
    workers: Workers,
    natives: Natives,
    project: Arc<Mutex<Project>>,
    preview_quality: Arc<Mutex<Quality>>,
    stop: Arc<AtomicBool>,
    wake: Arc<Wake>,
    loop_stats: Arc<LoopStats>,
    project_version: Arc<AtomicU64>,
    content_version: Arc<AtomicU64>,
    loop_region: Arc<Mutex<Option<(i64, i64)>>>,
) {
    // рендереры кэшируются до смены версии проекта: клонировать Project и
    // пересобирать графы каждые 100 мс было дорого даже в простое (п.1)
    let mut cached: Option<(u64, AudioRenderer, VideoRenderer)> = None;
    // состояние последнего прохода: в простое (пауза, ничего не менялось)
    // проход пропускается — раньше поток молотил 10 раз/с с десятками
    // захватов мьютекса кэша даже при полной тишине (п.1 v1.4). Прогрев
    // после seek/правки/смены качества сохраняется: любое изменение
    // состояния снимает пропуск.
    // (playhead, версия проекта, generation, качество, loop-регион)
    // content_version в состоянии прохода есть (иначе в простое правка
    // параметров генератора не разбудила бы префетч), но в условии
    // ПЕРЕСБОРКИ рендереров его нет: план от параметров не зависит
    // (закон П, Р.53 п.411).
    type PassState = (i64, u64, u64, u64, Quality, Option<(i64, i64)>);
    let mut last_state: Option<PassState> = None;
    while !stop.load(Ordering::Acquire) {
        wake.wait_timeout(std::time::Duration::from_millis(100));
        if stop.load(Ordering::Acquire) {
            return;
        }
        {
            let state = (
                transport.playhead_frames(),
                project_version.load(Ordering::Acquire),
                content_version.load(Ordering::Acquire),
                transport.generation(),
                *preview_quality.lock().unwrap(),
                *loop_region.lock().unwrap(),
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
            let vr = VideoRenderer::new_with_facts(&p, &natives.read().unwrap());
            cached = Some((version, AudioRenderer::new(&p), vr));
        }
        let (_, renderer, video) = cached.as_ref().unwrap();
        let generation = transport.generation();
        let head = transport.playhead_frames();
        let region = *loop_region.lock().unwrap();
        for win in lookahead_windows(head, PREFETCH_AUDIO_LOOKAHEAD_FRAMES, region) {
            prefetch_audio_pass(
                renderer,
                &cache,
                &workers.read().unwrap(),
                win,
                Priority::NearPrefetch,
                generation,
                false,
            );
        }
        // видео-lookahead: 2 c, но не больше половины бюджета кэша (п.19в:
        // 4К-кадры выселяли собственное окно)
        let t0 = transport.playhead();
        let q = *preview_quality.lock().unwrap();
        let step = video.grid.frame_step();
        // бюджет окна — по САМОМУ КРУПНОМУ качеству декода (Р.46, п.318):
        // на Quality окно переливало бы кэш ровно в S^2 раз
        let ahead =
            video_lookahead_ticks(video.max_decode_quality(q), step, cache.max_video_bytes());
        let region_ticks =
            region.map(|(s, e)| (Tick::from_audio_frames(s).0, Tick::from_audio_frames(e).0));
        let ranges: Vec<TickRange> = lookahead_windows(t0.0, ahead, region_ticks)
            .into_iter()
            .map(|r| TickRange::new(Tick(r.start), Tick(r.end)))
            .collect();
        for (media, src_pts, dq) in ranges.iter().flat_map(|r| video.wanted_frames(*r, q)) {
            if !cache.contains_video(source_id(media), Some(dq), video_cache_idx(src_pts))
                && let Some(w) = workers.read().unwrap().get(&media)
            {
                w.submit(DecodeRequest {
                    kind: RequestKind::video_on_grid(src_pts, Some(dq), step),
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
        assert_eq!(video_display_frame(1000, 0, None, 1, 1.0), 1000);
        // вычитание
        assert_eq!(video_display_frame(48_000, 4_800, None, 1, 1.0), 43_200);
        // клэмп в 0 у старта
        assert_eq!(video_display_frame(100, 4_800, None, 1, 1.0), 0);
        // отрицательная задержка не двигает вперёд
        assert_eq!(video_display_frame(1000, -500, None, 1, 1.0), 1000);
        // wrap в loop-регионе: 50_500 − 1_000 → заворот к концу региона
        assert_eq!(video_display_frame(50_500, 1_000, Some((50_000, 60_000)), 1, 1.0), 59_500);
        // внутри региона без пересечения границы
        assert_eq!(video_display_frame(55_000, 1_000, Some((50_000, 60_000)), 1, 1.0), 54_000);
        // до региона — линейно
        assert_eq!(video_display_frame(10_000, 1_000, Some((50_000, 60_000)), 1, 1.0), 9_000);
    }

    /// Тест 44 (J4): при реверсе знак компенсации задержки обратный —
    /// слышимое сейчас ПОЗЖЕ по таймлайну (playhead едет влево).
    #[test]
    fn t44_latency_sign_flips_in_reverse() {
        assert_eq!(video_display_frame(48_000, 4_800, None, -1, 1.0), 52_800);
        assert_eq!(video_display_frame(48_000, 4_800, None, 1, 1.0), 43_200);
        // в loop-регионе wrap в обе стороны
        assert_eq!(video_display_frame(59_500, 1_000, Some((50_000, 60_000)), -1, 1.0), 50_500);
        assert_eq!(
            video_display_frame(1000, 0, None, -1, 1.0),
            1000,
            "нулевая задержка нейтральна"
        );
    }

    /// Тест 134 (п.132, Р.23): условия принадлежности loop'у согласованы с
    /// транспортом — playhead ЗА регионом показывается линейно, а правило
    /// вооружённости зеркально для реверса.
    #[test]
    fn t134_loop_membership_matches_transport() {
        use super::loop_arm_after_jump;
        let reg = Some((50_000, 60_000));
        // правее конца — линейно, не фолдится в регион (сам баг п.132)
        assert_eq!(video_display_frame(65_000, 1_000, reg, 1, 1.0), 64_000);
        assert_eq!(video_display_frame(60_001, 0, reg, 1, 1.0), 60_001);
        // граница включительно, как p <= e у advance_frames_wrapped
        assert_eq!(video_display_frame(60_000, 0, reg, 1, 1.0), 50_000);
        // реверс-подход справа — тоже линейно
        assert_eq!(video_display_frame(62_000, 1_000, reg, -1, 1.0), 63_000);
        // реверс левее начала — линейно (как и было)
        assert_eq!(video_display_frame(48_000, 1_000, reg, -1, 1.0), 49_000);

        // правило вооружённости: forward — до конца включительно
        assert!(loop_arm_after_jump(60_000, reg, 1));
        assert!(!loop_arm_after_jump(60_001, reg, 1));
        assert!(loop_arm_after_jump(10_000, reg, 1), "слева — линейный подход и луп");
        // реверс — зеркало: от начала включительно
        assert!(loop_arm_after_jump(50_000, reg, -1));
        assert!(!loop_arm_after_jump(49_999, reg, -1));
        assert!(loop_arm_after_jump(70_000, reg, -1), "справа — подход и луп");
        // без региона / вырожденный регион — всегда вооружён
        assert!(loop_arm_after_jump(0, None, 1));
        assert!(loop_arm_after_jump(0, Some((5, 5)), 1));
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
        } else {
            // Поток ещё не начался (старт игры, кольцо после flush'а):
            // первого кадра не было, часам не от чего идти, а prev/cur
            // держат материал ДО seek'а — выдавать их нельзя. Это
            // единственный случай, когда время стоит (Р.43, п.294).
            for i in 0..out_frames {
                emit(i, (0.0, 0.0));
            }
            return 0;
        }
    }
    // ВРЕМЯ ИДЁТ ПО ВЫДАННЫМ КАДРАМ, а не по успешно прочитанным (Р.43,
    // п.294). Раньше голодание кольца не увеличивало `consumed`, а
    // playhead двигается ровно на `consumed` (`advance_frames_wrapped`) —
    // значит каждый кадр underrun'а был кадром, на который таймлайн НЕ
    // проехал, и программа физически играла медленнее реального времени.
    // Симптом пользователя: «кстати M3TEOR замедленн почему-то» и «потом
    // там вообще нет ни звука ни видео у основного события». Слышимая
    // тишина от этого не исчезает — она честно считается в
    // `callback_underruns`, — но часы больше не отстают от звуковой карты.
    let mut starved = 0i64;
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
                    // underrun: тянем последний кадр (без щелчка), но кадр
                    // ПРОЗВУЧАЛ — время на него продвигается
                    *cur = *prev;
                    starved += 1;
                }
            }
        }
        let a = *phase as f32;
        emit(i, (prev.0 + (cur.0 - prev.0) * a, prev.1 + (cur.1 - prev.1) * a));
    }
    consumed + starved
}

/// Интерполяция позиции ПОКАЗА между шагами аудио-часов (п.1 раунда 5,
/// тест 56). Playhead двигается квантами аудио-колбэка (PipeWire нередко
/// 2048 сэмплов = 42.7 мс) — без интерполяции видео физически не может
/// показать больше 1000/квант различимых кадров в секунду (наблюдалось
/// ровно 24 при кванте 2048, независимо от качества и бэкенда). Wall-clock
/// здесь ТОЛЬКО сглаживает показ между обновлениями мастер-часов: аудио
/// остаётся мастером (инвариант №6), офлайн-пути это не читают (№11).
/// Экстраполяция клампится размером последнего шага — показ не уходит
/// вперёд дальше, чем реально будет потреблено к следующему обновлению.
pub fn display_extrapolation(elapsed_frames: i64, step_ema: i64) -> i64 {
    elapsed_frames.clamp(0, step_ema.max(0))
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
mod display_clock_tests {
    use super::display_extrapolation;

    /// Тест 56 (п.1 раунда 5): интерполяция показа между квантами
    /// аудио-часов. Симуляция: playhead шагает квантами 2048 (PipeWire
    /// 42.7 мс — потолок 23.4 «сырых» кадров/с), UI сэмплирует 60 Гц;
    /// с интерполяцией последовательность выровненных ключей сетки 60 fps
    /// покрывает ВСЕ ячейки подряд (ноль пропусков) и монотонна.
    #[test]
    fn t56_interpolated_display_hits_every_grid_cell() {
        const QUANTUM: i64 = 2048; // кадров на шаг playhead
        const GRID: i64 = 800; // кадров на ячейку 60 fps (48000/60)
        const UI_DT: i64 = 800; // UI-сэмпл каждые ~16.7 мс в кадрах
        let step_ema = QUANTUM;
        let mut shown = Vec::new();
        let mut prev_key = -1i64;
        for k in 0..600 {
            let wall = k * UI_DT; // «время» в кадрах
            let head = (wall / QUANTUM) * QUANTUM; // квантованный playhead
            let elapsed = wall - head; // с последнего шага
            let display = head + display_extrapolation(elapsed, step_ema);
            let key = display / GRID;
            assert!(key >= prev_key, "монотонность: {prev_key} -> {key}");
            if key != prev_key {
                if prev_key >= 0 {
                    assert_eq!(key, prev_key + 1, "пропуск ячейки: {prev_key} -> {key}");
                }
                shown.push(key);
                prev_key = key;
            }
        }
        // все ячейки за окно наблюдения показаны (60 кадров/с, не 23)
        assert!(shown.len() >= 590, "показано {} ячеек из ~599", shown.len());

        // БЕЗ интерполяции — потолок 1000/42.7 ≈ 23.4 кадра/с (регресс)
        let mut raw_keys = std::collections::BTreeSet::new();
        for k in 0..600 {
            let head = ((k * UI_DT) / QUANTUM) * QUANTUM;
            raw_keys.insert(head / GRID);
        }
        assert!(
            raw_keys.len() < 250,
            "без интерполяции ключей мало ({}) — иначе тест не о том",
            raw_keys.len()
        );
    }
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

        // источник иссяк: не паникуем, тянем последний кадр — но ВРЕМЯ
        // идёт (Р.43, п.294). Прежнее `consumed == 20` утверждало ДЕФЕКТ:
        // на нём playhead замирал, и вся программа играла медленнее
        // реального времени.
        let (out, consumed) = run(1.0, 100, 20);
        assert_eq!(consumed, 102, "часы идут по ВЫДАННЫМ кадрам, а не по прочитанным");
        assert_eq!(out[99].0, 19.0, "после конца — последний кадр");
    }

    /// Тест 284 (Р.43 п.294): голодание кольца НЕ останавливает часы.
    ///
    /// Претензия дословно: «underruns растут… а потом там вообще нет ни
    /// звука ни видео у основного события, только у M3TEOR появляется
    /// следующего, кстати M3TEOR замедленн почему-то».
    ///
    /// Механизм: playhead двигается ровно на `consumed`
    /// (`advance_frames_wrapped`), а `consumed` считал только УСПЕШНО
    /// прочитанные из кольца кадры. Значит каждый кадр callback-underrun'а
    /// — кадр, на который таймлайн не проехал, и всё, что играет после
    /// такого места, отстаёт от реального времени. Замедлялся не M3TEOR:
    /// замедлялась ВСЯ программа.
    #[test]
    fn t284_starved_ring_does_not_stop_the_clock() {
        let t = super::Transport::new();
        t.set_playing(true);
        let out_frames = 512usize;
        let callbacks = 8i64;
        let mut underruns = 0u64;

        // кольцо пусто ПОСЛЕ прайминга: поток начался и оборвался
        let mut left = 2i64;
        let mut next = || {
            if left > 0 {
                left -= 1;
                Some((0.5f32, 0.5f32))
            } else {
                underruns += 1;
                None
            }
        };
        let (mut prev, mut cur, mut primed, mut phase) = ((0.0, 0.0), (0.0, 0.0), false, 0.0);
        let mut total = 0i64;
        for _ in 0..callbacks {
            let mut sink = |_: usize, _: (f32, f32)| {};
            total += varispeed_fill(
                &mut next,
                1.0,
                out_frames,
                &mut prev,
                &mut cur,
                &mut primed,
                &mut phase,
                &mut sink,
            );
        }
        t.advance_frames_wrapped(total, None);

        // +2 — кадры прайминга интерполяции, как в t32
        let want = out_frames as i64 * callbacks + 2;
        assert_eq!(
            t.playhead_frames(),
            want,
            "часы обязаны идти по ВЫДАННЫМ кадрам: устройство сыграло {}, \
             playhead уехал на {} (до Р.43 было 2 — ровно столько, сколько \
             успели прочитать)",
            out_frames as i64 * callbacks,
            t.playhead_frames()
        );
        assert!(underruns > 0, "тишина обязана честно считаться underrun'ами");
    }

    /// Тест 284b (Р.43 п.294): пока ПЕРВОГО кадра не было — часы стоят.
    /// Это единственное исключение: на старте игры и сразу после seek'а
    /// кольцо законно пусто, и продвигать playhead не от чего.
    #[test]
    fn t284b_clock_waits_for_the_first_frame() {
        let mut next = || None;
        let (mut prev, mut cur, mut primed, mut phase) =
            ((7.0f32, 7.0f32), (9.0f32, 9.0f32), false, 0.0);
        let mut out = vec![(1.0f32, 1.0f32); 64];
        let consumed = varispeed_fill(
            &mut next,
            1.0,
            out.len(),
            &mut prev,
            &mut cur,
            &mut primed,
            &mut phase,
            |i, f| out[i] = f,
        );
        assert_eq!(consumed, 0, "потока ещё нет — время не идёт");
        assert!(
            out.iter().all(|&(l, r)| l == 0.0 && r == 0.0),
            "и материал ДО seek'а наружу не выдаётся"
        );
    }

    /// Тест 114 (п.14 р.14): показ на МЕДЛЕННОЙ скорости не откатывается.
    ///
    /// Собираем всю цепочку целиком — шаг мастер-часов по скорости,
    /// экстраполяция, компенсация задержки (с ДЖИТТЕРОМ, как её двигает
    /// EMA), выравнивание по сетке и ратчет ключа. Раньше каждый кусок
    /// проверялся изолированно при 1×, и ошибка единиц (wall-clock
    /// задержка вычиталась из позиции таймлайна без учёта скорости)
    /// пряталась между ними: при 0.25× картинка уезжала на пару кадров
    /// назад — ровно жалоба пользователя.
    #[test]
    fn t114_display_key_never_goes_back_at_slow_rates() {
        let fps_step = 90_000 / 30; // ячейка 30 fps в тиках
        // 0.01 и 0.05 — с Р.57: полоса транспорта теперь доходит до них
        // (прежний её край, 0.25, был тут ЕДИНСТВЕННЫМ нижним случаем)
        for &rate in &[0.01f32, 0.05, 0.25, 0.5, 1.0, 2.0, 8.0] {
            for &dir in &[1i64, -1] {
                let quantum = 2048i64; // кадров устройства на колбэк
                let step = (quantum as f64 * rate as f64) as i64; // ход head
                let mut head = 480_000i64;
                let mut prev_key: Option<i64> = None;
                let mut rollbacks = 0;
                // 200 колбэков, между ними 8 UI-кадров
                for cb in 0..200 {
                    head += step * dir;
                    // джиттер задержки: EMA гуляет на ±квант
                    let latency = 2048 + (cb % 7) * 512;
                    for ui in 0..8 {
                        let elapsed = (quantum as f64 * rate as f64 * (ui as f64 / 8.0)) as i64;
                        let extra = super::display_extrapolation(elapsed, step);
                        let frames = super::video_display_frame(
                            head + extra * dir,
                            latency,
                            None,
                            dir,
                            rate,
                        );
                        let key = kt_time::Tick::from_audio_frames(frames).0.div_euclid(fps_step)
                            * fps_step;
                        let (key, _, rolled) =
                            super::preview_key_step(prev_key, key, fps_step, dir, false);
                        if rolled {
                            rollbacks += 1;
                        }
                        if let Some(p) = prev_key {
                            assert!(
                                (key - p) * dir >= 0,
                                "rate={rate} dir={dir}: ключ показа уехал назад {p} → {key}"
                            );
                        }
                        prev_key = Some(key);
                    }
                }
                // ратчет обязан быть РЕДКОЙ страховкой, а не рабочим
                // режимом: если он срабатывает постоянно — значит часы
                // показа опять дрожат (это и был баг при 0.25×)
                // Числа замерены: со старой (немасштабированной) задержкой
                // при 0.25× ратчет ловил 186/189 откатов из 1600 выборок,
                // с масштабированной — 10/6. При 1× обе формулы дают
                // одинаковые 34/30, то есть правка нейтральна к норме.
                if rate <= 1.0 {
                    assert!(
                        rollbacks < 50,
                        "rate={rate} dir={dir}: {rollbacks} откатов — часы показа дрожат"
                    );
                }
            }
        }
    }

    /// Ратчет: назад только по seek/переанкору, вперёд считает пропуски.
    #[test]
    fn t114_preview_key_step_rules() {
        let step = 3000;
        assert_eq!(super::preview_key_step(None, 9000, step, 1, false), (9000, 0, false));
        // откат запрещён при любом размере — даже на 10 ячеек
        assert_eq!(super::preview_key_step(Some(9000), 6000, step, 1, false), (9000, 0, true));
        assert_eq!(super::preview_key_step(Some(9000), 0, step, 1, false), (9000, 0, true));
        // seek/пауза — переанкор, ключ уходит куда сказано
        assert_eq!(super::preview_key_step(Some(9000), 0, step, 1, true), (0, 0, false));
        // вперёд: пропуск ячеек считается
        assert_eq!(super::preview_key_step(Some(9000), 12_000, step, 1, false), (12_000, 0, false));
        assert_eq!(super::preview_key_step(Some(9000), 21_000, step, 1, false), (21_000, 3, false));
        // РЕВЕРС: убывание — норма, рост — откат
        assert_eq!(super::preview_key_step(Some(9000), 6000, step, -1, false), (6000, 0, false));
        assert_eq!(super::preview_key_step(Some(9000), 12_000, step, -1, false), (9000, 0, true));
    }
}
