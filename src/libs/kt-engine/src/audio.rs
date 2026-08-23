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
    id: kt_model::ClipId,
    start: i64,
    end: i64,
    /// Конец с учётом хвоста FX (реверб дозванивает за границей клипа —
    /// инвариант «tail», дефект f старого движка).
    ext_end: i64,
    source_in: i64,
    lane: u32,
    gain: f32,
    media: kt_model::MediaId,
    fx: Vec<kt_model::FxInstance>,
    reversed: bool,
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
            .map(|c: &ClipEvent| {
                let start = c.start.to_audio_frames();
                let end = (c.start + c.duration).to_audio_frames();
                let tail = kt_fx::chain_tail(&c.fx).to_audio_frames();
                AudioClip {
                    id: c.id,
                    start,
                    end,
                    ext_end: end + tail,
                    source_in: c.source_in.to_audio_frames(),
                    lane: c.lane,
                    gain: db_to_linear(c.gain_db),
                    media: c.media,
                    fx: c.fx.clone(),
                    reversed: c.reversed,
                }
            })
            .collect();
        // Стабильный порядок — часть детерминизма и правила кроссфейда
        // («ранний» клип определяется по (start, media)).
        clips.sort_by_key(|c| (c.start, c.end, c.lane, c.media));
        AudioRenderer { clips }
    }

    /// Синхронизировать FX-состояние с этим графом (вызывается владельцем
    /// стора — аудио-render-потоком — после каждой пересборки графа).
    pub fn sync_fx(&self, store: &mut crate::fxstore::FxStateStore) {
        for c in &self.clips {
            store.sync_clip(c.id, &c.fx, c.source_in, c.end - c.start);
        }
        let alive: std::collections::HashSet<_> = self.clips.iter().map(|c| c.id).collect();
        store.retain_clips(&|id| alive.contains(&id));
    }

    /// Рендер без внешнего FX-состояния (для клипов без FX эквивалентно
    /// `render_frames_with`; FX-клипы получат одноразовый store — годится
    /// для одноразовых офлайн-вызовов, но не для потокового плеера).
    pub fn render_frames(
        &self,
        fetcher: &mut dyn AudioFetcher,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        let mut store = crate::fxstore::FxStateStore::new();
        self.sync_fx(&mut store);
        self.render_frames_with(fetcher, &mut store, start_frame, out)
    }

    /// Рендер непрерывного диапазона таймлайна [start_frame, start_frame+out/2)
    /// в interleaved stereo `out`. Без loop-логики — wrap делает вызывающий
    /// (`render_looped`), разрезая диапазон по границе региона.
    /// `store` — долгоживущее FX-состояние аудио-render-потока.
    pub fn render_frames_with(
        &self,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert_eq!(out.len() % AUDIO_CHANNELS, 0);
        out.fill(0.0);
        let frames = (out.len() / AUDIO_CHANNELS) as i64;
        let range = start_frame..start_frame + frames;
        let mut stats = RenderStats::default();

        let active: Vec<usize> = (0..self.clips.len())
            .filter(|&i| self.clips[i].start < range.end && self.clips[i].ext_end > range.start)
            .collect();

        // Вклад каждого клипа считаем по-бурстово (источника), веса — по-сэмплово.
        for &ci in &active {
            let clip = &self.clips[ci];
            if !clip.fx.is_empty() {
                self.render_fx_clip(ci, fetcher, store, &range, out, &mut stats);
                continue;
            }
            let from = clip.start.max(range.start);
            let to = clip.end.min(range.end);
            if from >= to {
                continue;
            }
            let mut f = from;
            let mut cursor: Option<(i64, Arc<AudioBurst>)> = None; // (burst idx, data)
            let clip_len = clip.end - clip.start;
            while f < to {
                // реверс: локальный кадр k читает источник с конца (п.16)
                let src = if clip.reversed {
                    clip.source_in + (clip_len - 1 - (f - clip.start))
                } else {
                    clip.source_in + (f - clip.start)
                };
                let bidx = src.div_euclid(AUDIO_BURST_FRAMES as i64);
                let within = src.rem_euclid(AUDIO_BURST_FRAMES as i64);
                // сколько кадров можем взять из этого source-burst'а подряд:
                // вперёд — до конца бёрста, при реверсе — вниз до его начала
                let take = if clip.reversed {
                    (within + 1).min(to - f)
                } else {
                    (AUDIO_BURST_FRAMES as i64 - within).min(to - f)
                };
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
                            let sk = if clip.reversed { within - k } else { within + k };
                            let si = (sk as usize) * AUDIO_CHANNELS;
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

    /// FX-клип: клип-локальный поток через ClipProcessor (варп+реверб+gain
    /// цепочки), затем веса кроссфейда (в пределах реального клипа) и gain
    /// клипа. В зоне хвоста (за end) вес 1.0 — реверб дозванивает под
    /// соседями без взвешивания.
    fn render_fx_clip(
        &self,
        ci: usize,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        range: &std::ops::Range<i64>,
        out: &mut [f32],
        stats: &mut RenderStats,
    ) {
        let clip = &self.clips[ci];
        let from = clip.start.max(range.start);
        let to = clip.ext_end.min(range.end);
        if from >= to {
            return;
        }
        // окно источника клипа: за его пределами — законная тишина.
        // Единый расчёт с wanted_audio_bursts (chain_source_window); dry за
        // концом клипа глушит сам ClipProcessor (output-гейт), поэтому запас
        // окна безопасен.
        let clip_len = clip.end - clip.start;
        let src_window_end =
            clip.source_in + kt_fx::chain_source_window(&clip.fx, clip_len, 0..clip_len).end;
        let media = clip.media;
        let mut pull_underruns = 0u64;
        let Some(proc) = store.get_mut(clip.id) else { return };
        let local = from - clip.start;
        let count = (to - from) as usize;
        let mut buf = vec![0f32; count * AUDIO_CHANNELS];
        {
            let reversed = clip.reversed;
            let win = (clip.source_in, src_window_end);
            let mut pull = |src_frame: i64, dst: &mut [f32]| -> bool {
                fill_source(fetcher, media, src_frame, win, reversed, dst, &mut pull_underruns)
            };
            proc.read(local, &mut buf, &mut pull);
        }
        stats.underruns += pull_underruns;
        for k in 0..count as i64 {
            let f = from + k;
            let w = if f < clip.end { self.weight_at(ci, f) } else { 1.0 } * clip.gain;
            let oi = ((f - range.start) as usize) * AUDIO_CHANNELS;
            let bi = (k as usize) * AUDIO_CHANNELS;
            for ch in 0..AUDIO_CHANNELS {
                out[oi + ch] += buf[bi + ch] * w;
            }
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
        let mut store = crate::fxstore::FxStateStore::new();
        self.sync_fx(&mut store);
        self.render_looped_with(fetcher, &mut store, region, start_frame, out)
    }

    pub fn render_looped_with(
        &self,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
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
            let s = self.render_frames_with(fetcher, store, tl, sub);
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
            let (s0, s1) = if c.fx.is_empty() {
                if c.reversed {
                    // зеркало: конец таймлайн-диапазона читает НАЧАЛО куска
                    let len = c.end - c.start;
                    (c.source_in + (len - (to - c.start)), c.source_in + (len - (from - c.start)))
                } else {
                    // без FX: источник течёт 1:1 с таймлайном
                    (c.source_in + (from - c.start), c.source_in + (to - c.start))
                }
            } else if c.reversed {
                // FX+реверс: окно потребления зеркалится внутри ПОЛНОГО окна
                let len = c.end - c.start;
                let total = kt_fx::chain_source_window(&c.fx, len, 0..len).end;
                let w = kt_fx::chain_source_window(&c.fx, len, (from - c.start)..(to - c.start));
                (c.source_in + (total - w.end).max(0), c.source_in + (total - w.start).max(1))
            } else {
                // с FX: варп потребляет источник по rate_integral, а не
                // линейно; wanted обязан покрывать РЕАЛЬНОЕ потребление
                // (регрессия п.13: линейный маппинг грел не те бёрсты →
                // глубже в клипе вечный промах кэша → тишина)
                let len = c.end - c.start;
                let w = kt_fx::chain_source_window(&c.fx, len, (from - c.start)..(to - c.start));
                (c.source_in + w.start, c.source_in + w.end.max(w.start + 1))
            };
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

/// Заполнить `dst` кадрами источника [src_frame, +len): читает burst'ы через
/// фетчер; кадры вне [0, window_end) — тишина БЕЗ underrun'а (хвост/пре-ролл),
/// промах внутри окна — тишина с underrun'ом. Возвращает true (буфер всегда
/// валиден).
fn fill_source(
    fetcher: &mut dyn AudioFetcher,
    media: kt_model::MediaId,
    src_frame: i64,
    window: (i64, i64), // (start=source_in, end) — окно потребления клипа
    reversed: bool,
    dst: &mut [f32],
    underruns: &mut u64,
) -> bool {
    dst.fill(0.0);
    let (win_start, window_end) = window;
    let frames = (dst.len() / AUDIO_CHANNELS) as i64;
    let mut f = 0i64;
    while f < frames {
        // Реверс (п.16): виртуальный форвард-поток [win_start, window_end)
        // читает РЕАЛЬНЫЙ источник зеркально внутри окна; FX-цепочка (варп,
        // реверб) видит обычный форвард-поток и не знает о реверсе.
        let virt = src_frame + f;
        if virt >= window_end {
            break; // дальше только тишина
        }
        let src = if reversed { win_start + (window_end - 1 - virt) } else { virt };
        if src < 0 || virt < 0 {
            f += 1;
            continue;
        }
        let bidx = src.div_euclid(AUDIO_BURST_FRAMES as i64);
        let within = src.rem_euclid(AUDIO_BURST_FRAMES as i64);
        let take = if reversed {
            (within + 1).min(frames - f).min(window_end - virt)
        } else {
            (AUDIO_BURST_FRAMES as i64 - within).min(frames - f).min(window_end - virt)
        };
        match fetcher.audio_burst(media, bidx) {
            Some(b) => {
                let d0 = (f as usize) * AUDIO_CHANNELS;
                if reversed {
                    for k in 0..take as usize {
                        let si = (within as usize - k) * AUDIO_CHANNELS;
                        for ch in 0..AUDIO_CHANNELS {
                            dst[d0 + k * AUDIO_CHANNELS + ch] = b.data[si + ch];
                        }
                    }
                } else {
                    let s0 = (within as usize) * AUDIO_CHANNELS;
                    let n = (take as usize) * AUDIO_CHANNELS;
                    dst[d0..d0 + n].copy_from_slice(&b.data[s0..s0 + n]);
                }
            }
            None => *underruns += 1,
        }
        f += take;
    }
    true
}
