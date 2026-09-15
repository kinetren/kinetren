//! Экспорт. Инварианты №7 и №9:
//! - путь рендеринга ОДИН: те же AudioRenderer/VideoRenderer, что и превью
//!   (другой clock — «по готовности», другое Quality — разрешение проекта);
//! - ноль временных файлов: rawvideo(RGBA) → stdin ffmpeg, аудио f32le →
//!   named pipe; ffmpeg муксит на лету, backpressure через пайпы.
//!   (Старый движок писал гигабайты rgb24+s16le в tmpfs — дефект o.)
//!
//! `render_raw_*` отдают сырые потоки колбэкам — на этом стоят golden-тесты
//! (тест 11: хэши потоков + «превью == экспорт» по построению).

use crate::audio::AudioRenderer;
use crate::fetch::DirectFetcher;
use crate::fxstore::FxStateStore;
use crate::source_id;
use crate::video::{VideoRenderer, video_cache_idx};
use crate::video_plan::VideoGpuExec;
use kt_cache::{BurstCache, CacheConfig};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Прогресс экспорта (ETA/скорость считает UI — wall-clock не наш, инв. №10).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExportProgress {
    pub audio_frames: u64,
    pub total_audio_frames: u64,
    pub video_frames: u64,
    pub total_video_frames: u64,
    /// Текущая фаза (п.17: видно, чем занят экспорт).
    pub phase: ExportPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportPhase {
    #[default]
    Video,
    Audio,
    Finalize,
    Done,
}

#[derive(Debug, Clone, Default)]
pub struct ExportStats {
    pub video_frames: u64,
    pub audio_frames: u64,
    pub underruns: u64,
    /// Какой видео-энкодер выбран (для UI); пусто для render_raw_*.
    pub encoder: String,
    /// Слои видео, не декодированные к моменту кадра (в экспорте это ошибка
    /// источника, не тайминга — декод блокирующий).
    pub missing_video_layers: u64,
    /// Громкость СМИКШИРОВАННОГО материала (Р.49, п.371). `None` — аудио не
    /// считалось (видео-проход) или материал целиком ниже абсолютного гейта.
    ///
    /// Меряется на проходе, который И ТАК ИДЁТ: метр кормится готовым
    /// буфером чанка — ни одного нового декода и ни одного нового цикла.
    ///
    /// БЕЗ мастер-гейна — и это не упущение: мастер-гейн живёт только в
    /// `Player`, его нет ни в `Project`, ни в .otio, экспорт его не
    /// применяет, а ставит его фейдер панели метров. Это громкость
    /// КОНТРОЛЯ, а не материала. Следствие: число в превью и число файла
    /// совпадают по построению.
    pub loudness: Option<kt_fx::audio::loudness::Loudness>,
}

/// Диапазон экспорта: [0, конец последнего клипа].
/// Бюджет видео-кэша экспорта.
pub const EXPORT_VIDEO_CACHE_BYTES: usize = 192 * 1024 * 1024;

/// Сколько кадров декодировать одной пачкой, чтобы они ГАРАНТИРОВАННО
/// дожили в кэше до композиции (р.15 п.1). Пачка занимает не больше
/// половины бюджета; `layers` — максимум ОДНОВРЕМЕННЫХ видео-слоёв
/// (Р.31 п.4: каждый момент чанка держит в кэше по кадру НА СЛОЙ —
/// пачка, посчитанная на один слой, вытесняла пре-кадры соседних дорожек
/// до композиции, и рендер терял слои).
pub fn export_chunk_frames(budget_bytes: usize, width: u32, height: u32, layers: usize) -> i64 {
    let frame = (width.max(1) as usize) * (height.max(1) as usize) * 4 * layers.max(1);
    ((budget_bytes / 2) / frame.max(1)).clamp(1, 120) as i64
}

pub fn project_range(project: &Project) -> Tick {
    let material = project.clips.iter().map(|c| c.start + c.duration).max().unwrap_or(Tick::ZERO);
    // монтажная длина чужого файла (хвостовой Gap) считается концом: рендер
    // доходит до неё чёрным/тишиной, как у эталона и melt (Р.25)
    project.timeline_end.map_or(material, |e| e.max(material))
}

/// Аудио-поток экспорта: interleaved f32 48k/2ch чанками по burst'у.
/// Тот же AudioRenderer, что в плеере; DirectFetcher = «clock по готовности».
pub fn render_raw_audio(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    emit: impl FnMut(&[f32]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    render_raw_audio_range(project, sources, None, &AtomicBool::new(false).into(), |_| {}, emit)
}

/// Полный вариант: диапазон, отмена, прогресс.
pub fn render_raw_audio_range(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    range: Option<(Tick, Tick)>,
    cancel: &Arc<AtomicBool>,
    mut progress: impl FnMut(u64),
    mut emit: impl FnMut(&[f32]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    let mut stats = ExportStats::default();
    let mut fetcher = DirectFetcher::new(None);
    for (id, s) in sources {
        fetcher.add_source(id, s);
    }
    let renderer = AudioRenderer::new(project);
    let mut store = FxStateStore::new();
    renderer.sync_fx(&mut store);
    let (start_frame, end_frame) = match range {
        Some((a, b)) => (a.to_audio_frames(), b.to_audio_frames()),
        None => (0, project_range(project).to_audio_frames()),
    };
    let mut meter = kt_fx::audio::loudness::LoudnessMeter::new();
    let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
    let mut head = start_frame;
    while head < end_frame {
        if cancel.load(Ordering::Acquire) {
            return Err(std::io::Error::other("cancelled"));
        }
        progress(stats.audio_frames);
        let n = ((end_frame - head) as usize).min(AUDIO_BURST_FRAMES);
        let sub = &mut chunk[..n * AUDIO_CHANNELS];
        let s = renderer.render_frames_with(&mut fetcher, &mut store, head, sub);
        stats.underruns += s.underruns;
        meter.feed(sub);
        emit(sub)?;
        stats.audio_frames += n as u64;
        head += n as i64;
    }
    stats.loudness = Some(meter.finish());
    Ok(stats)
}

/// Бэкенды рендера экспорта (Р.29). Тестовые/старые вызовы идут без них
/// (CPU — байт-эталон, golden-тесты нетронуты); продукт (kt-app/kt-cli)
/// передаёт GPU-исполнитель явно.
#[derive(Default, Clone)]
pub struct ExportBackends {
    pub video_gpu: Option<Arc<dyn VideoGpuExec>>,
}

/// Видео-поток экспорта: RGBA-кадры по сетке fps проекта. Композит —
/// ТОТ ЖЕ план кадра (`VideoRenderer::frame_plan`), что у превью;
/// исполнитель — CPU (этот вариант) или GPU (`_with` c backends).
pub fn render_raw_video(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    render_raw_video_range(
        project,
        sources,
        quality,
        None,
        &AtomicBool::new(false).into(),
        |_| {},
        emit,
    )
}

/// Полный вариант: диапазон, отмена, прогресс. CPU-исполнитель.
pub fn render_raw_video_range(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    cancel: &Arc<AtomicBool>,
    progress: impl FnMut(u64),
    emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    render_raw_video_range_with(
        project,
        sources,
        quality,
        range,
        &ExportBackends::default(),
        cancel,
        progress,
        emit,
    )
}

/// Вариант с выбором бэкенда. Ошибка GPU ПОСРЕДИ экспорта — ошибка
/// экспорта, а не молчаливый переход на CPU: начатый файл обязан быть
/// отрендерен одним бэкендом (ред. инварианта №7, Р.29).
#[allow(clippy::too_many_arguments)]
pub fn render_raw_video_range_with(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    backends: &ExportBackends,
    cancel: &Arc<AtomicBool>,
    mut progress: impl FnMut(u64),
    mut emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    let mut stats = ExportStats::default();
    let mut src: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources.into_iter().collect();
    // нативные размеры (Р.46, п.321): ступень декода считается по запасу
    // файла над Quality экспорта
    let natives: std::collections::HashMap<MediaId, (u32, u32)> = src
        .iter()
        .filter(|(_, s)| s.info().has_video)
        .map(|(m, s)| (*m, (s.info().width, s.info().height)))
        .collect();
    // Кэш на пару кадров: композит читает из кэша (тот же код, что превью),
    // экспорт лишь наполняет его блокирующим декодом.
    let cache = BurstCache::new(CacheConfig {
        max_audio_bytes: 1,
        max_video_bytes: EXPORT_VIDEO_CACHE_BYTES,
    });
    let mut video = VideoRenderer::new_with_natives(project, &natives);
    let (mut t, end) = match range {
        Some((a, b)) => (video.grid.align_floor(a), b),
        None => (Tick(0), project_range(project)),
    };
    let step = video.grid.frame_step();
    // Кадры берём ПАЧКАМИ через read_video_span — тем же путём, что
    // превью (п.14 р.14). Раньше здесь был по-кадровый read_video_frame:
    // при fps проекта > fps источника он на каждом втором кадре уходил в
    // seek+пересоздание декодера, а семантика выборки расходилась со
    // span'ом — отсюда джаддер в готовом файле и нарушение инварианта №7.
    // Размер чанка считается от ПАМЯТИ, а не «120 и всё» (р.15 п.1):
    // 120 кадров 1080p — это 995 МБ, они не влезали в кэш экспорта, LRU
    // выбрасывал их ДО композиции, и рендер выходил чёрным с редкими
    // уцелевшими кадрами. На 320×180 (где я это «проверил») чанк влезал —
    // отсюда и пропущенная регрессия.
    // Считается по САМОМУ КРУПНОМУ качеству декода (Р.46, п.318): клип,
    // который берётся крупнее Quality, занимает в кэше в S^2 раз больше —
    // на Quality чанк был бы недооценён и вернул бы ту же регрессию.
    let dq_max = video.max_decode_quality(quality);
    let chunk_frames = export_chunk_frames(
        EXPORT_VIDEO_CACHE_BYTES,
        dq_max.width,
        dq_max.height,
        video.max_concurrent_layers(),
    );
    let mut frame_buf = Vec::new();
    while t < end {
        if cancel.load(Ordering::Acquire) {
            return Err(std::io::Error::other("cancelled"));
        }
        let chunk_end = Tick((t.0 + chunk_frames * step).min(end.0));
        // ключи чанка по каждому источнику: строго возрастают, дубли сняты
        // ключ группы — (медиа, КАЧЕСТВО ДЕКОДА): спан декодера не смешивает
        // размеры (kt-media), и заказ приходит уже с качеством (Р.46)
        let mut per_src: std::collections::BTreeMap<(MediaId, Quality), Vec<Tick>> =
            Default::default();
        for (media, src_pts, dq) in
            video.wanted_frames(kt_time::TickRange::new(t, chunk_end), quality)
        {
            per_src.entry((media, dq)).or_default().push(src_pts);
        }
        for ((media, dq), keys) in per_src {
            let Some(s) = src.get_mut(&media) else {
                stats.missing_video_layers += keys.len() as u64;
                continue;
            };
            let sid = source_id(media);
            let missing: Vec<Tick> = keys
                .into_iter()
                .filter(|k| cache.get_video(sid, Some(dq), video_cache_idx(*k)).is_none())
                .collect();
            if missing.is_empty() {
                continue;
            }
            let ok = s
                .read_video_span(&missing, Some(dq), &mut |k, frame| {
                    cache.insert_video(sid, Some(dq), video_cache_idx(k), frame.into());
                })
                .is_ok();
            if !ok {
                stats.missing_video_layers += 1;
            }
        }
        while t < chunk_end {
            if cancel.load(Ordering::Acquire) {
                return Err(std::io::Error::other("cancelled"));
            }
            progress(stats.video_frames);
            let complete = match &backends.video_gpu {
                Some(gpu) => {
                    let plan = video.frame_plan(&cache, t, quality);
                    gpu.execute_into(&plan, &mut frame_buf)
                        .map_err(|e| std::io::Error::other(format!("gpu render: {e:?}")))?
                }
                None => video.compose_export_into(&cache, t, quality, &mut frame_buf),
            };
            if !complete {
                stats.missing_video_layers += 1;
            }
            emit(&frame_buf)?;
            stats.video_frames += 1;
            t = Tick(t.0 + step);
        }
    }
    Ok(stats)
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg exited with {0}")]
    FfmpegFailed(std::process::ExitStatus),
    #[error("audio thread panicked")]
    AudioThread,
    #[error("export cancelled")]
    Cancelled,
}
// Отказ GpuRequired (Р.29) снят в Р.44: Pan3d считается и на CPU
// (обратная гомография в слитом гео-проходе) — экспорт без GPU легален,
// просто медленнее. Тест 301.

/// Потоковый экспорт в mp4 (h264+aac). `audio_sources`/`video_sources` —
/// независимые наборы (пишутся из двух потоков в два пайпа параллельно,
/// иначе дедлок на backpressure).
pub fn export_mp4(
    project: &Project,
    audio_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    video_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    out_path: &str,
) -> Result<ExportStats, ExportError> {
    export_mp4_with(
        project,
        audio_sources,
        video_sources,
        out_path,
        None,
        &ExportOptions::default(),
        &AtomicBool::new(false).into(),
        &AtomicBool::new(false).into(),
        |_| {},
    )
}

/// Полный экспорт: диапазон (регион), отмена (kill ffmpeg + удаление
/// частичного файла), прогресс-колбэк (зовётся из обоих потоков).
/// Настройки экспорта (п.5): выбор кодека, качество, битрейт, разрешение.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// CRF/CQ/QP (23 ≈ визуально прозрачно). Игнорируется при video_bitrate.
    pub quality: u32,
    /// Целевой битрейт видео, бит/с (None = режим качества).
    pub video_bitrate: Option<u32>,
    pub audio_bitrate: u32,
    /// None = автовыбор с hw-приоритетом; Some = только этот энкодер
    /// (не открылся — явная ошибка, без фолбэка).
    pub force_encoder: Option<String>,
    /// Переопределение разрешения (None = из настроек проекта).
    pub resolution: Option<(u32, u32)>,
}

impl Default for ExportOptions {
    fn default() -> ExportOptions {
        ExportOptions {
            quality: 23,
            video_bitrate: None,
            audio_bitrate: 192_000,
            force_encoder: None,
            resolution: None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn export_mp4_with(
    project: &Project,
    audio_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    video_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    out_path: &str,
    range: Option<(Tick, Tick)>,
    opts: &ExportOptions,
    cancel: &Arc<AtomicBool>,
    pause: &Arc<AtomicBool>,
    progress: impl Fn(ExportProgress) + Send + Sync + 'static,
) -> Result<ExportStats, ExportError> {
    export_mp4_with_backends(
        project,
        audio_sources,
        video_sources,
        out_path,
        range,
        opts,
        &ExportBackends::default(),
        cancel,
        pause,
        progress,
    )
}

/// Продуктовый вариант (kt-app/kt-cli): бэкенды рендера передаются явно —
/// GPU по умолчанию, CPU когда движка нет (Р.29).
#[allow(clippy::too_many_arguments)]
pub fn export_mp4_with_backends(
    project: &Project,
    audio_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    video_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    out_path: &str,
    range: Option<(Tick, Tick)>,
    opts: &ExportOptions,
    backends: &ExportBackends,
    cancel: &Arc<AtomicBool>,
    pause: &Arc<AtomicBool>,
    progress: impl Fn(ExportProgress) + Send + Sync + 'static,
) -> Result<ExportStats, ExportError> {
    // пауза (порт v1 waitWhilePaused): кооперативный wait 40 мс,
    // отменяемый; проверяется на каждом кадре/чанке через progress-обёртки
    let wait_paused = {
        let pause = pause.clone();
        let cancel = cancel.clone();
        move || {
            while pause.load(Ordering::Acquire) && !cancel.load(Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
        }
    };
    let (ow, oh) = opts.resolution.unwrap_or((project.settings.width, project.settings.height));
    let quality = Quality { width: ow.max(2) & !1, height: oh.max(2) & !1 };
    let fps = project.settings.fps;
    let (r0, r1) = match range {
        Some((a, b)) => (a, b),
        None => (Tick(0), project_range(project)),
    };
    let total_audio = (r1.to_audio_frames() - r0.to_audio_frames()).max(0) as u64;
    let total_video = ((r1.0 - r0.0) / (kt_time::TICK_HZ / fps as i64)).max(0) as u64;

    // Внутрипроцессный муксер: кодеки берутся из СИСТЕМНОГО FFmpeg,
    // поэтому доступны его аппаратные энкодеры. Внешний бинарник ffmpeg
    // и mkfifo больше не нужны (инвариант №9 усилен: не только без
    // временных файлов, но и без внешних процессов).
    let mut muxer = kt_media::muxer::Muxer::open(&kt_media::muxer::MuxConfig {
        path: out_path.to_string(),
        width: quality.width,
        height: quality.height,
        fps,
        quality: opts.quality,
        video_bitrate: opts.video_bitrate,
        audio_bitrate: opts.audio_bitrate,
        force_encoder: opts.force_encoder.clone(),
    })
    .map_err(|e| ExportError::Io(std::io::Error::other(e)))?;

    // Аудио и видео рендерятся последовательно в один муксер (он
    // сам перемежает пакеты через av_interleaved_write_frame).
    let a_done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let v_done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let progress = Arc::new(progress);
    let report = {
        let progress = progress.clone();
        let a_done = a_done.clone();
        let v_done = v_done.clone();
        move |phase: ExportPhase| {
            progress(ExportProgress {
                audio_frames: a_done.load(Ordering::Relaxed),
                total_audio_frames: total_audio,
                video_frames: v_done.load(Ordering::Relaxed),
                total_video_frames: total_video,
                phase,
            })
        }
    };

    // 1) видео (ошибка/отмена внутри фаз — удалить частичный файл, п.17)
    let cleanup_on_err = |e: ExportError| {
        let _ = std::fs::remove_file(out_path);
        e
    };
    let vstats = {
        let v_done = v_done.clone();
        let report = report.clone();
        render_raw_video_range_with(
            project,
            video_sources,
            quality,
            range,
            backends,
            cancel,
            |done| {
                wait_paused();
                v_done.store(done, Ordering::Relaxed);
                if done % 10 == 0 {
                    report(ExportPhase::Video);
                }
            },
            |rgba| muxer.write_video(rgba).map_err(std::io::Error::other),
        )
        .map_err(|e| cleanup_on_err(e.into()))?
    };
    // 2) аудио
    let astats = {
        let a_done = a_done.clone();
        let report = report.clone();
        render_raw_audio_range(
            project,
            audio_sources,
            range,
            cancel,
            |done| {
                wait_paused();
                a_done.store(done, Ordering::Relaxed);
                if done % 48_000 < 1024 {
                    report(ExportPhase::Audio);
                }
            },
            |chunk| muxer.write_audio(chunk).map_err(std::io::Error::other),
        )
        .map_err(|e| cleanup_on_err(e.into()))?
    };

    if cancel.load(Ordering::Acquire) {
        drop(muxer);
        let _ = std::fs::remove_file(out_path);
        return Err(ExportError::Cancelled);
    }
    report(ExportPhase::Finalize);
    let encoder = muxer.encoder_name().to_string();
    muxer.finish().map_err(|e| ExportError::Io(std::io::Error::other(e)))?;
    report(ExportPhase::Done);
    Ok(ExportStats {
        video_frames: vstats.video_frames,
        audio_frames: astats.audio_frames,
        underruns: astats.underruns,
        missing_video_layers: vstats.missing_video_layers,
        encoder,
        // громкость посчитана на аудио-проходе — тем же метром, тем же
        // буфером и без мастер-гейна (п.371)
        loudness: astats.loudness,
    })
}
