//! Видео-путь: выбор активных клипов, ключи кадров, CPU-композит.
//!
//! Инвариант №7: этот же композит питает и превью, и экспорт — отличаются
//! только clock и Quality. Слои запрашиваются в ОДНОМ качестве и
//! складываются alpha-over по z (lane: меньше = выше, как в старом движке,
//! где меньший индекс дорожки был передним планом).

use kt_cache::{BurstCache, SourceId};
use kt_media::Quality;
use kt_model::{ClipKind, MediaId, Project};
use kt_time::{FpsGrid, Tick};

#[derive(Debug, Clone)]
struct VideoClip {
    start: Tick,
    end: Tick,
    source_in: Tick,
    lane: u32,
    media: MediaId,
    reversed: bool,
    fx: Vec<kt_model::FxInstance>,
}

/// pts источника для момента таймлайна `t` внутри клипа: прямой или
/// зеркальный (п.16 реверс). Чистая функция — тест 24.
fn clip_source_pts(c: &VideoClip, t: Tick, grid: &FpsGrid) -> Tick {
    let src = if c.reversed {
        // зеркало: последний кадр клипа показывает source_in, первый —
        // конец использованного куска источника (минус шаг кадра)
        let span = c.end.0 - c.start.0;
        let step = grid.frame_step();
        Tick((c.source_in.0 + span - step - (t.0 - c.start.0)).max(c.source_in.0))
    } else {
        Tick(c.source_in.0 + (t.0 - c.start.0))
    };
    grid.align_floor(src)
}

pub struct VideoRenderer {
    clips: Vec<VideoClip>,
    pub grid: FpsGrid,
}

/// Ключ кадра в кэше: idx = pts источника в тиках (выравнен по сетке проекта).
pub fn video_cache_idx(source_pts: Tick) -> i64 {
    source_pts.0
}

impl VideoRenderer {
    pub fn new(project: &Project) -> VideoRenderer {
        // effective_clips: morpher'ы применены здесь, движок их не видит (H1)
        let effective = project.effective_clips();
        let mut clips: Vec<VideoClip> = effective
            .iter()
            .filter(|c| c.kind == ClipKind::Video)
            .map(|c| VideoClip {
                start: c.start,
                end: c.start + c.duration,
                source_in: c.source_in,
                lane: c.lane,
                media: c.media,
                reversed: c.reversed,
                fx: c.active_fx(), // bypass-слоты отфильтрованы (п.2)
            })
            .collect();
        // задний план первым (большая lane), передний (lane 0) — последним
        clips.sort_by_key(|c| std::cmp::Reverse((c.lane, c.start.0)));
        VideoRenderer { clips, grid: FpsGrid::new(project.settings.fps) }
    }

    /// Кадры, нужные для композиции момента `t` (выравненного вызывающим):
    /// (media, pts источника). Порядок — back-to-front.
    pub fn layers_at(&self, t: Tick) -> Vec<(MediaId, Tick)> {
        self.clips
            .iter()
            .filter(|c| c.start <= t && t < c.end)
            .map(|c| (c.media, clip_source_pts(c, t, &self.grid)))
            .collect()
    }

    /// Все ключи кадров для диапазона таймлайна (prefetch/экспорт).
    pub fn wanted_frames(&self, range: kt_time::TickRange) -> Vec<(MediaId, Tick)> {
        let mut out = Vec::new();
        let step = self.grid.frame_step();
        let mut t = self.grid.align_floor(range.start);
        while t < range.end {
            out.extend(self.layers_at(t));
            t = Tick(t.0 + step);
        }
        out.sort();
        out.dedup();
        out
    }

    /// Композит момента `t` из кэша в RGBA-буфер `quality`-размера.
    /// Возвращает (буфер, все ли слои были в кэше). Отсутствующий слой
    /// пропускается (playback не ждёт — инвариант №5).
    pub fn compose_from_cache(
        &self,
        cache: &BurstCache,
        t: Tick,
        quality: Quality,
    ) -> (Vec<u8>, bool) {
        let w = quality.width as usize;
        let h = quality.height as usize;
        let mut out = vec![0u8; w * h * 4];
        let mut complete = true;
        for clip in self.clips.iter().filter(|c| c.start <= t && t < c.end) {
            let src_pts = clip_source_pts(clip, t, &self.grid);
            let key_idx = video_cache_idx(src_pts);
            match cache.get_video(SourceId(clip.media.0), Some(quality), key_idx) {
                Some(frame) => {
                    if kt_fx::video::has_video_fx(&clip.fx) {
                        // цепочка видео-FX (п.25): кэш хранит ПРЕ-FX кадры
                        // (инвариант кэша), эффекты — на композите; один код
                        // для превью и экспорта (инвариант №7)
                        let span = (clip.end.0 - clip.start.0).max(1);
                        let u = (t.0 - clip.start.0) as f64 / span as f64;
                        let ctx = kt_fx::video::VideoFxCtx {
                            out_w: quality.width,
                            out_h: quality.height,
                            u,
                            burst_index: t.0,
                        };
                        let f = kt_fx::video::apply_video_chain(
                            &clip.fx,
                            kt_fx::video::Frame {
                                w: frame.width,
                                h: frame.height,
                                rgba: frame.rgba.to_vec(),
                                has_alpha: frame.has_alpha,
                            },
                            &ctx,
                        );
                        if (f.w as usize, f.h as usize) == (w, h) {
                            alpha_over(&mut out, &f.rgba, f.has_alpha, w * h);
                        }
                    } else {
                        alpha_over(&mut out, &frame.rgba, frame.has_alpha, w * h);
                    }
                }
                None => complete = false,
            }
        }
        (out, complete)
    }
}

/// dst = src OVER dst (straight alpha). Непрозрачный src — просто копия.
fn alpha_over(dst: &mut [u8], src: &[u8], has_alpha: bool, pixels: usize) {
    let n = pixels * 4;
    if !has_alpha {
        let n = n.min(src.len()).min(dst.len());
        dst[..n].copy_from_slice(&src[..n]);
        return;
    }
    for p in 0..pixels.min(src.len() / 4).min(dst.len() / 4) {
        let i = p * 4;
        let sa = src[i + 3] as u32;
        if sa == 255 {
            dst[i..i + 4].copy_from_slice(&src[i..i + 4]);
            continue;
        }
        if sa == 0 {
            continue;
        }
        let da = dst[i + 3] as u32;
        let inv = 255 - sa;
        for ch in 0..3 {
            let s = src[i + ch] as u32;
            let d = dst[i + ch] as u32;
            dst[i + ch] = ((s * sa + d * inv * da / 255) / 255).min(255) as u8;
        }
        dst[i + 3] = (sa + da * inv / 255).min(255) as u8;
    }
}
