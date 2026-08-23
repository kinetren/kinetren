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
        let inner = kt_ffmpeg::Source::open(path)?;
        let p = inner.probe();
        let info = MediaInfo {
            duration: p.duration,
            has_video: p.has_video,
            has_audio: p.has_audio,
            fps: p.fps,
            width: p.width,
            height: p.height,
        };
        let src_size = (p.width.max(2), p.height.max(2));
        Ok(FfmpegSource { inner, info, src_size })
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

impl MediaSource for FfmpegSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        let mut burst = AudioBurst::silence(idx);
        self.inner.read_audio_burst(idx, &mut burst.data[..])?;
        Ok(burst)
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
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
            has_alpha: false,
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
        self.inner
            .read_video_span(keys, w, h, &mut |k, f| {
                emit(
                    k,
                    VideoFrame {
                        pts: f.pts,
                        width: f.width,
                        height: f.height,
                        rgba: f.rgba.into_boxed_slice(),
                        has_alpha: false,
                    },
                )
            })
            .map_err(MediaError::from)
    }
}

const _: () = assert!(AUDIO_BURST_FRAMES * AUDIO_CHANNELS > 0);
