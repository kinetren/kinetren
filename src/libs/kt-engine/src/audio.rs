//! Аудио-рендер таймлайна: выбор событий, sample-accurate чтение источников,
//! equal-power кроссфейд, gain, суммирование дорожек, loop-wrap.
//!
//! Семантика унаследована от C++ движка (bc_composition.cpp) и закреплена
//! тестами 1–3, 14:
//! - перекрытие ровно 2 клипов на ОДНОЙ дорожке → equal-power кроссфейд
//!   (ранний √(1−t), поздний √t по прогрессу перекрытия);
//! - 3+ клипов на одной дорожке → равные веса 1/N;
//! - разные дорожки суммируются без весов;
//! - gain клипа (dB → линейный) применяется после весов.
//!
//! Всё в домене сэмплов 48 kHz (кадров): границы клипов округляются к сэмплу
//! floor'ом один раз, дальше арифметика точная (инвариант №10 — детерминизм).

use crate::fetch::AudioFetcher;
use kt_media::AudioBurst;
use kt_model::{ClipEvent, ClipKind, Project};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS};
use std::sync::Arc;

/// Клип, подготовленный к аудио-рендеру (границы в кадрах 48 kHz).
#[derive(Debug, Clone)]
struct AudioClip {
    start: i64,
    end: i64,
    source_in: i64,
    lane: u32,
    gain: f32,
    media: kt_model::MediaId,
}

/// Результат рендера непрерывного диапазона.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderStats {
    /// Сколько сэмпло-чтений источника не удалось (подставлена тишина).
    pub underruns: u64,
}

pub struct AudioRenderer {
    clips: Vec<AudioClip>,
}

impl AudioRenderer {
    pub fn new(project: &Project) -> AudioRenderer {
        let mut clips: Vec<AudioClip> = project
            .clips
            .iter()
            .filter(|c| c.kind == ClipKind::Audio)
            .map(|c: &ClipEvent| AudioClip {
                start: c.start.to_audio_frames(),
                end: (c.start + c.duration).to_audio_frames(),
                source_in: c.source_in.to_audio_frames(),
                lane: c.lane,
                gain: db_to_linear(c.gain_db),
                media: c.media,
            })
            .collect();
        // Стабильный порядок — часть детерминизма и правила кроссфейда
        // («ранний» клип определяется по (start, media)).
        clips.sort_by_key(|c| (c.start, c.end, c.lane, c.media));
        AudioRenderer { clips }
    }

    /// Рендер непрерывного диапазона таймлайна [start_frame, start_frame+out/2)
    /// в interleaved stereo `out`. Без loop-логики — wrap делает вызывающий
    /// (`render_looped`), разрезая диапазон по границе региона.
    pub fn render_frames(
        &self,
        fetcher: &mut dyn AudioFetcher,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert_eq!(out.len() % AUDIO_CHANNELS, 0);
        out.fill(0.0);
        let frames = (out.len() / AUDIO_CHANNELS) as i64;
        let range = start_frame..start_frame + frames;
        let mut stats = RenderStats::default();

        let active: Vec<usize> = (0..self.clips.len())
            .filter(|&i| self.clips[i].start < range.end && self.clips[i].end > range.start)
            .collect();

        // Вклад каждого клипа считаем по-бурстово (источника), веса — по-сэмплово.
        for &ci in &active {
            let clip = &self.clips[ci];
            let from = clip.start.max(range.start);
            let to = clip.end.min(range.end);
            let mut f = from;
            let mut cursor: Option<(i64, Arc<AudioBurst>)> = None; // (burst idx, data)
            while f < to {
                let src = clip.source_in + (f - clip.start);
                let bidx = src.div_euclid(AUDIO_BURST_FRAMES as i64);
                let within = src.rem_euclid(AUDIO_BURST_FRAMES as i64);
                // сколько кадров можем взять из этого source-burst'а подряд
                let take = (AUDIO_BURST_FRAMES as i64 - within).min(to - f);
                let burst = match &cursor {
                    Some((i, b)) if *i == bidx => Some(b.clone()),
                    _ => {
                        let b = fetcher.audio_burst(clip.media, bidx);
                        if let Some(b) = &b {
                            cursor = Some((bidx, b.clone()));
                        }
                        b
                    }
                };
                match burst {
                    Some(b) => {
                        for k in 0..take {
                            let w = self.weight_at(ci, f + k) * clip.gain;
                            let oi = ((f + k - range.start) as usize) * AUDIO_CHANNELS;
                            let si = ((within + k) as usize) * AUDIO_CHANNELS;
                            for ch in 0..AUDIO_CHANNELS {
                                out[oi + ch] += b.data[si + ch] * w;
                            }
                        }
                    }
                    None => stats.underruns += take as u64,
                }
                f += take;
            }
        }
        stats
    }

    /// Вес клипа №`ci` в сэмпле `frame` по правилам одной дорожки.
    /// «Ранний» из пары — с меньшим индексом (clips отсортированы по start).
    fn weight_at(&self, ci: usize, frame: i64) -> f32 {
        let lane = self.clips[ci].lane;
        let mut overlapping: [usize; 2] = [usize::MAX; 2];
        let mut n = 0usize;
        for (i, c) in self.clips.iter().enumerate() {
            if c.lane == lane && c.start <= frame && frame < c.end {
                if n < 2 {
                    overlapping[n] = i;
                }
                n += 1;
            }
        }
        match n {
            0 | 1 => 1.0,
            2 => {
                let (a, b) = (&self.clips[overlapping[0]], &self.clips[overlapping[1]]);
                let ov_start = a.start.max(b.start);
                let ov_end = a.end.min(b.end);
                let t = if ov_end > ov_start {
                    (frame - ov_start) as f64 / (ov_end - ov_start) as f64
                } else {
                    0.5
                };
                // equal-power: ранний затухает √(1−t), поздний нарастает √t
                if ci == overlapping[0] {
                    ((1.0 - t).max(0.0)).sqrt() as f32
                } else {
                    (t.max(0.0)).sqrt() as f32
                }
            }
            n => 1.0 / n as f32,
        }
    }

    /// Рендер с loop-регионом: выходной поток длиной `frames` кадров, таймлайн
    /// позиции wrap'ятся по [region) per-sample (граница может не совпадать
    /// с burst'ом). Тест 14: непрерывность через wrap.
    pub fn render_looped(
        &self,
        fetcher: &mut dyn AudioFetcher,
        region: std::ops::Range<i64>,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert!(!region.is_empty());
        let len = region.end - region.start;
        let total = (out.len() / AUDIO_CHANNELS) as i64;
        let mut stats = RenderStats::default();
        let mut done = 0i64;
        while done < total {
            let tl = region.start + (start_frame - region.start + done).rem_euclid(len);
            let chunk = (region.end - tl).min(total - done);
            let sub = &mut out
                [(done as usize) * AUDIO_CHANNELS..((done + chunk) as usize) * AUDIO_CHANNELS];
            let s = self.render_frames(fetcher, tl, sub);
            stats.underruns += s.underruns;
            done += chunk;
        }
        stats
    }

    /// Диапазоны source-burst'ов, нужные для воспроизведения [range) таймлайна
    /// (в кадрах). Основа prefetch'а: wanted − coverage → submit.
    pub fn wanted_audio_bursts(
        &self,
        range: std::ops::Range<i64>,
    ) -> Vec<(kt_model::MediaId, std::ops::Range<i64>)> {
        let mut out = Vec::new();
        for c in &self.clips {
            let from = c.start.max(range.start);
            let to = c.end.min(range.end);
            if from >= to {
                continue;
            }
            let s0 = c.source_in + (from - c.start);
            let s1 = c.source_in + (to - c.start);
            let b0 = s0.div_euclid(AUDIO_BURST_FRAMES as i64);
            let b1 = (s1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1;
            out.push((c.media, b0..b1));
        }
        out
    }
}

pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}
