//! Синтетические источники для тестов: детерминированные, без FFmpeg.
//! Живут в основном крейте (не под #[cfg(test)]), чтобы downstream-крейты
//! могли использовать их в своих тестах.

use crate::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};

/// Какой сигнал генерирует аудио-часть источника.
#[derive(Debug, Clone, Copy)]
pub enum AudioPattern {
    /// Тишина.
    Silence,
    /// Синус заданной частоты, амплитуда 0.5, одинаковый в обоих каналах.
    Sine { hz: f64 },
    /// Значение сэмпла = его глобальный номер (для sample-exact тестов стыков).
    /// Левый канал = n, правый = -n, масштаб 1e-6 чтобы остаться в разумном диапазоне.
    Ramp,
    /// Постоянное значение (для тестов кроссфейда).
    Dc { value: f32 },
    /// Единичный импульс в сэмпле 0, дальше тишина (для тестов хвостов FX).
    Impulse,
}

/// Разделяемые счётчики обращений: источник живёт в потоке воркера, а тест
/// наблюдает снаружи (тесты 13, 21).
#[derive(Default)]
pub struct SyntheticStats {
    pub audio_reads: std::sync::atomic::AtomicU64,
    pub video_reads: std::sync::atomic::AtomicU64,
    /// Видео-запросы с pts МЕНЬШЕ предыдущего (для реального декодера это
    /// backward seek + пересоздание — тест 21 требует ноль на секвенции).
    pub video_backward_reads: std::sync::atomic::AtomicU64,
    pub last_video_pts: std::sync::atomic::AtomicI64,
    pub audio_backward_reads: std::sync::atomic::AtomicU64,
    pub last_audio_idx: std::sync::atomic::AtomicI64,
}

/// Синтетический источник: детерминированная функция от номера burst'а/pts.
pub struct SyntheticSource {
    info: MediaInfo,
    pattern: AudioPattern,
    /// Счётчик обращений — для регрессионных тестов «ноль декодов» (тест 13).
    pub audio_reads: u64,
    pub video_reads: u64,
    /// Опциональные разделяемые счётчики (тесты через MediaWorker).
    pub stats: Option<std::sync::Arc<SyntheticStats>>,
}

impl SyntheticSource {
    pub fn new(duration: Tick, pattern: AudioPattern) -> SyntheticSource {
        SyntheticSource {
            info: MediaInfo {
                duration,
                has_video: true,
                has_audio: true,
                fps: Some((30, 1)),
                width: 64,
                height: 36,
            },
            pattern,
            audio_reads: 0,
            video_reads: 0,
            stats: None,
        }
    }

    pub fn with_stats(
        duration: Tick,
        pattern: AudioPattern,
        stats: std::sync::Arc<SyntheticStats>,
    ) -> SyntheticSource {
        let mut s = SyntheticSource::new(duration, pattern);
        s.stats = Some(stats);
        s
    }

    /// Значение сэмпла `n` канала `ch` для паттерна — эталон для ассертов в тестах.
    pub fn sample_at(pattern: AudioPattern, n: i64, ch: usize) -> f32 {
        match pattern {
            AudioPattern::Silence => 0.0,
            AudioPattern::Sine { hz } => {
                (0.5 * (2.0 * std::f64::consts::PI * hz * n as f64 / AUDIO_SAMPLE_RATE as f64)
                    .sin()) as f32
            }
            AudioPattern::Ramp => {
                let v = n as f32 * 1e-6;
                if ch == 0 { v } else { -v }
            }
            AudioPattern::Dc { value } => value,
            AudioPattern::Impulse => {
                if n == 0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

impl MediaSource for SyntheticSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        if idx < 0 {
            return Err(MediaError::OutOfRange);
        }
        self.audio_reads += 1;
        if let Some(st) = &self.stats {
            use std::sync::atomic::Ordering::Relaxed;
            st.audio_reads.fetch_add(1, Relaxed);
            let last = st.last_audio_idx.swap(idx, Relaxed);
            if idx < last {
                st.audio_backward_reads.fetch_add(1, Relaxed);
            }
        }
        let mut burst = AudioBurst::silence(idx);
        let base = idx * AUDIO_BURST_FRAMES as i64;
        for f in 0..AUDIO_BURST_FRAMES {
            for ch in 0..AUDIO_CHANNELS {
                burst.data[f * AUDIO_CHANNELS + ch] =
                    Self::sample_at(self.pattern, base + f as i64, ch);
            }
        }
        Ok(burst)
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        self.video_reads += 1;
        if let Some(st) = &self.stats {
            use std::sync::atomic::Ordering::Relaxed;
            st.video_reads.fetch_add(1, Relaxed);
            let last = st.last_video_pts.swap(pts.0, Relaxed);
            if pts.0 < last {
                st.video_backward_reads.fetch_add(1, Relaxed);
            }
        }
        let q = quality.unwrap_or(Quality { width: 64, height: 36 });
        // Кадр однозначно определяется pts: цвет кодирует номер тика.
        let r = (pts.0 & 0xff) as u8;
        let g = ((pts.0 >> 8) & 0xff) as u8;
        let b = ((pts.0 >> 16) & 0xff) as u8;
        let px = [r, g, b, 255u8];
        let rgba: Vec<u8> =
            px.iter().copied().cycle().take((q.width * q.height * 4) as usize).collect();
        Ok(VideoFrame {
            pts,
            width: q.width,
            height: q.height,
            rgba: rgba.into_boxed_slice(),
            has_alpha: false,
        })
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        Some(Tick(pts.0.max(0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_is_globally_continuous_across_bursts() {
        let mut s = SyntheticSource::new(Tick(90_000), AudioPattern::Ramp);
        let b0 = s.read_audio_burst(0).unwrap();
        let b1 = s.read_audio_burst(1).unwrap();
        // последний сэмпл b0 и первый b1 — соседние значения ramp
        let last = b0.data[(AUDIO_BURST_FRAMES - 1) * AUDIO_CHANNELS];
        let first = b1.data[0];
        assert_eq!(first, AUDIO_BURST_FRAMES as f32 * 1e-6);
        assert_eq!(last, (AUDIO_BURST_FRAMES as f32 - 1.0) * 1e-6);
        assert_eq!(s.audio_reads, 2);
    }
}
