//! Мост к системному FFmpeg через kt-ffmpeg (рантайм-загрузка, dlopen).
//!
//! Здесь только адаптация к трейту `MediaSource`; вся работа с libav*
//! (включая выбор мажорной версии 6/7/8) — в крейте kt-ffmpeg.
//! ИНВАРИАНТ: бинарь не линкуется с libav*, поэтому один и тот же
//! исполняемый файл работает на системах с разными версиями FFmpeg.

use crate::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

pub struct FfmpegSource {
    inner: kt_ffmpeg::Source,
    info: MediaInfo,
    /// Исходный размер кадра — используется при quality = None.
    src_size: (u32, u32),
    /// У формата потока есть альфа (Р.67): кадр уходит в композит с ней.
    has_alpha: bool,
}

impl From<kt_ffmpeg::FfError> for MediaError {
    fn from(e: kt_ffmpeg::FfError) -> MediaError {
        match e {
            kt_ffmpeg::FfError::OutOfRange => MediaError::OutOfRange,
            other => MediaError::Decode(other.to_string()),
        }
    }
}

impl FfmpegSource {
    pub fn open(path: &str) -> Result<FfmpegSource, MediaError> {
        Self::from_inner(kt_ffmpeg::Source::open(path)?)
    }

    /// Открытие через sidecar индекс-БД: пакетный индекс (pts аудио-пакетов
    /// и видео-кейфреймов) берётся из БД, если файл не менялся — полный
    /// скан-проход по файлу пропадает (быстрое открытие больших файлов).
    /// После свежего скана индекс сохраняется в БД. Любая ошибка БД — это
    /// ошибка КЭША: молча откатываемся к обычному open со сканом.
    pub fn open_indexed(
        path: &str,
        db: Option<&mut crate::index::MediaIndexDb>,
    ) -> Result<FfmpegSource, MediaError> {
        let Some(db) = db else { return Self::open(path) };
        let Ok((mid, status)) = db.ensure_media(std::path::Path::new(path)) else {
            return Self::open(path);
        };
        if status == crate::index::MediaEntryStatus::Reused
            && let (Ok(Some(audio)), Ok(Some(video))) =
                (db.packet_index(mid, "audio"), db.packet_index(mid, "video"))
        {
            let prebuilt = kt_ffmpeg::PrebuiltIndex { audio_pts: audio, video_keyframes: video };
            return Self::from_inner(kt_ffmpeg::Source::open_with_index(path, Some(prebuilt))?);
        }
        let src = Self::open(path)?;
        let (audio, video) = src.inner.packet_index();
        let _ = db.put_packet_index(mid, "audio", audio);
        let _ = db.put_packet_index(mid, "video", video);
        Ok(src)
    }

    /// Индекс пришёл из БД (скан не выполнялся) — диагностика/тест 34.
    pub fn index_was_prebuilt(&self) -> bool {
        self.inner.index_was_prebuilt()
    }

    /// (pts аудио-пакетов, pts видео-кейфреймов) — сверка в тесте 34.
    pub fn packet_index(&self) -> (&[i64], &[i64]) {
        self.inner.packet_index()
    }

    fn from_inner(inner: kt_ffmpeg::Source) -> Result<FfmpegSource, MediaError> {
        let p = inner.probe();
        let info = MediaInfo {
            duration: p.duration,
            has_video: p.has_video,
            has_audio: p.has_audio,
            fps: p.fps,
            width: p.width,
            height: p.height,
            sar: p.sar,
            still: p.still,
        };
        let src_size = (p.width.max(2), p.height.max(2));
        let has_alpha = p.has_alpha;
        Ok(FfmpegSource { inner, info, src_size, has_alpha })
    }
}

impl FfmpegSource {
    /// (backward_reopens, frames_decoded) — диагностика теста 21.
    pub fn decode_debug_stats(&self) -> (u64, u64) {
        self.inner.video_debug_stats()
    }
}

/// Версия загруженного FFmpeg (для перф-панели/диагностики).
pub fn backend_info() -> Option<kt_ffmpeg::BackendInfo> {
    kt_ffmpeg::backend_info()
}

/// Причина, по которой FFmpeg не загрузился (для сообщения пользователю).
pub fn backend_error() -> Option<String> {
    kt_ffmpeg::backend_error()
}

/// Кумулятивные (opens, packet_reads) FFmpeg-слоя — перф-панель
/// дифференцирует по времени (порт avcpp-метрик v1).
pub fn io_stats() -> (u64, u64) {
    kt_ffmpeg::iostats::snapshot()
}

impl MediaSource for FfmpegSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        let mut burst = AudioBurst::silence(idx);
        // У КАРТИНКИ звука нет по природе, и это не ошибка чтения (Р.67):
        // событие со стиллом обязано играть тишину, как заглушка, а не
        // валить микс отказом «вне диапазона».
        if self.info.still {
            return Ok(burst);
        }
        self.inner.read_audio_burst(idx, &mut burst.data[..])?;
        Ok(burst)
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        // У КАРТИНКИ кадр один и тот же на любой тик (Р.67): просить у
        // демуксера позицию, которой в файле нет, — верный способ получить
        // «кадра нет» и вечное «Loading…» над событием.
        let pts = if self.info.still { Tick::ZERO } else { pts };
        let (w, h) = match quality {
            Some(q) => (q.width, q.height),
            None => self.src_size, // исходное разрешение источника
        };
        let f = self.inner.read_video_frame(pts, w, h)?;
        Ok(VideoFrame {
            pts: f.pts,
            width: f.width,
            height: f.height,
            rgba: f.rgba.into_boxed_slice(),
            // ПРАВДА об альфе (Р.67): её знает формат потока, и композит
            // умеет её смешивать — `false` гвоздём выбрасывал прозрачность
            has_alpha: self.has_alpha,
        })
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        self.inner.keyframe_before(pts)
    }

    fn read_video_span(
        &mut self,
        keys: &[Tick],
        quality: Option<Quality>,
        emit: &mut dyn FnMut(Tick, VideoFrame),
    ) -> Result<(), MediaError> {
        let (w, h) = match quality {
            Some(q) => (q.width, q.height),
            None => self.src_size,
        };
        let has_alpha = self.has_alpha;
        self.inner
            .read_video_span(keys, w, h, &mut |k, f| {
                emit(
                    k,
                    VideoFrame {
                        pts: f.pts,
                        width: f.width,
                        height: f.height,
                        rgba: f.rgba.into_boxed_slice(),
                        has_alpha,
                    },
                )
            })
            .map_err(MediaError::from)
    }
}

const _: () = assert!(AUDIO_BURST_FRAMES * AUDIO_CHANNELS > 0);
