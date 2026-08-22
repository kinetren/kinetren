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
use kt_cache::{BurstCache, CacheConfig};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Прогресс экспорта (ETA/скорость считает UI — wall-clock не наш, инв. №10).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExportProgress {
    pub audio_frames: u64,
    pub total_audio_frames: u64,
    pub video_frames: u64,
    pub total_video_frames: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExportStats {
    pub video_frames: u64,
    pub audio_frames: u64,
    pub underruns: u64,
    /// Слои видео, не декодированные к моменту кадра (в экспорте это ошибка
    /// источника, не тайминга — декод блокирующий).
    pub missing_video_layers: u64,
}

/// Диапазон экспорта: [0, конец последнего клипа].
pub fn project_range(project: &Project) -> Tick {
    project.clips.iter().map(|c| c.start + c.duration).max().unwrap_or(Tick::ZERO)
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
        emit(sub)?;
        stats.audio_frames += n as u64;
        head += n as i64;
    }
    Ok(stats)
}

/// Видео-поток экспорта: RGBA-кадры по сетке fps проекта. Композит —
/// ТОТ ЖЕ `VideoRenderer::compose_from_cache`, что рисует превью.
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

/// Полный вариант: диапазон, отмена, прогресс.
pub fn render_raw_video_range(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    cancel: &Arc<AtomicBool>,
    mut progress: impl FnMut(u64),
    mut emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    let mut stats = ExportStats::default();
    let mut src: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources.into_iter().collect();
    // Кэш на пару кадров: композит читает из кэша (тот же код, что превью),
    // экспорт лишь наполняет его блокирующим декодом.
    let cache =
        BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: 64 * 1024 * 1024 });
    let video = VideoRenderer::new(project);
    let (mut t, end) = match range {
        Some((a, b)) => (video.grid.align_floor(a), b),
        None => (Tick(0), project_range(project)),
    };
    let step = video.grid.frame_step();
    while t < end {
        if cancel.load(Ordering::Acquire) {
            return Err(std::io::Error::other("cancelled"));
        }
        progress(stats.video_frames);
        for (media, src_pts) in video.layers_at(t) {
            let idx = video_cache_idx(src_pts);
            if cache.get_video(source_id(media), Some(quality), idx).is_none() {
                match src.get_mut(&media).map(|s| s.read_video_frame(src_pts, Some(quality))) {
                    Some(Ok(frame)) => {
                        cache.insert_video(source_id(media), Some(quality), idx, frame.into())
                    }
                    _ => stats.missing_video_layers += 1,
                }
            }
        }
        let (rgba, complete) = video.compose_from_cache(&cache, t, quality);
        if !complete {
            stats.missing_video_layers += 1;
        }
        emit(&rgba)?;
        stats.video_frames += 1;
        t = Tick(t.0 + step);
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
        &AtomicBool::new(false).into(),
        |_| {},
    )
}

/// Полный экспорт: диапазон (регион), отмена (kill ffmpeg + удаление
/// частичного файла), прогресс-колбэк (зовётся из обоих потоков).
pub fn export_mp4_with(
    project: &Project,
    audio_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    video_sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    out_path: &str,
    range: Option<(Tick, Tick)>,
    cancel: &Arc<AtomicBool>,
    progress: impl Fn(ExportProgress) + Send + Sync + 'static,
) -> Result<ExportStats, ExportError> {
    let quality = Quality {
        width: project.settings.width.max(2) & !1,
        height: project.settings.height.max(2) & !1,
    };
    let fps = project.settings.fps;

    // fifo — крошечный спец-файл (не данные!), единственное, что касается ФС
    let fifo_dir = std::env::temp_dir().join(format!("kt-export-{}", std::process::id()));
    std::fs::create_dir_all(&fifo_dir)?;
    let fifo = fifo_dir.join("audio.f32le");
    let _ = std::fs::remove_file(&fifo);
    let status = std::process::Command::new("mkfifo").arg(&fifo).status()?;
    if !status.success() {
        return Err(ExportError::Io(std::io::Error::other("mkfifo failed")));
    }

    let mut child = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        // видео: rawvideo RGBA из stdin
        .args(["-f", "rawvideo", "-pix_fmt", "rgba"])
        .args(["-s", &format!("{}x{}", quality.width, quality.height)])
        .args(["-r", &fps.to_string()])
        .args(["-i", "pipe:0"])
        // аудио: f32le из fifo
        .args(["-f", "f32le", "-ar", "48000", "-ac", "2"])
        .args(["-i", &fifo.to_string_lossy()])
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-preset", "veryfast", "-crf", "23"])
        .args(["-c:a", "aac", "-b:a", "192k"])
        .args(["-movflags", "+faststart", "-shortest"])
        .arg(out_path)
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    // тоталы для прогресса
    let (r0, r1) = match range {
        Some((a, b)) => (a, b),
        None => (Tick(0), project_range(project)),
    };
    let total_audio = (r1.to_audio_frames() - r0.to_audio_frames()).max(0) as u64;
    let total_video = ((r1.0 - r0.0) / (kt_time::TICK_HZ / fps as i64)).max(0) as u64;
    let progress = Arc::new(progress);
    let a_done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let v_done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let report = {
        let progress = progress.clone();
        let a_done = a_done.clone();
        let v_done = v_done.clone();
        move || {
            progress(ExportProgress {
                audio_frames: a_done.load(Ordering::Relaxed),
                total_audio_frames: total_audio,
                video_frames: v_done.load(Ordering::Relaxed),
                total_video_frames: total_video,
            })
        }
    };

    // аудио — в своём потоке (fifo блокируется, пока ffmpeg его не откроет)
    let audio_project = project.clone();
    let fifo_path = fifo.clone();
    let a_cancel = cancel.clone();
    let audio_thread = {
        let a_done = a_done.clone();
        let report = report.clone();
        std::thread::Builder::new()
            .name("export-audio".into())
            .spawn(move || -> std::io::Result<ExportStats> {
                let mut w = std::io::BufWriter::new(std::fs::File::create(&fifo_path)?);
                let stats = render_raw_audio_range(
                    &audio_project,
                    audio_sources,
                    range,
                    &a_cancel,
                    |done| {
                        a_done.store(done, Ordering::Relaxed);
                        if done % 48_000 < 1024 {
                            report();
                        }
                    },
                    |chunk| {
                        let bytes: &[u8] = unsafe {
                            std::slice::from_raw_parts(chunk.as_ptr() as *const u8, chunk.len() * 4)
                        };
                        w.write_all(bytes)
                    },
                )?;
                w.flush()?;
                Ok(stats)
            })
            .expect("spawn export audio")
    };

    // видео — в текущем потоке, в stdin ffmpeg
    let mut stdin = child.stdin.take().expect("child stdin");
    let vres = {
        let mut w = std::io::BufWriter::new(&mut stdin);
        let v_done = v_done.clone();
        let report = report.clone();
        let r = render_raw_video_range(
            project,
            video_sources,
            quality,
            range,
            cancel,
            |done| {
                v_done.store(done, Ordering::Relaxed);
                if done % 10 == 0 {
                    report();
                }
            },
            |rgba| w.write_all(rgba),
        );
        let _ = w.flush();
        r
    };
    drop(stdin); // EOF видео

    let ares = audio_thread.join().map_err(|_| ExportError::AudioThread)?;
    if cancel.load(Ordering::Acquire) {
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(out_path);
        let _ = std::fs::remove_file(&fifo);
        let _ = std::fs::remove_dir(&fifo_dir);
        return Err(ExportError::Cancelled);
    }
    let vstats = vres?;
    let astats = ares?;
    let status = child.wait()?;
    let _ = std::fs::remove_file(&fifo);
    let _ = std::fs::remove_dir(&fifo_dir);
    if !status.success() {
        return Err(ExportError::FfmpegFailed(status));
    }
    report();
    Ok(ExportStats {
        video_frames: vstats.video_frames,
        audio_frames: astats.audio_frames,
        underruns: astats.underruns,
        missing_video_layers: vstats.missing_video_layers,
    })
}
