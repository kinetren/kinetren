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
    /// Ключ пост-FX кэша (п.2 р.7): параметры цепочки + геометрия клипа +
    /// шаг сетки. Любая правка модели пересоздаёт VideoRenderer → хэш
    /// пересчитывается, стейл-кадры не читаются (их ключ не совпадёт).
    fx_hash: u64,
}

/// Хэш цепочки для пост-FX кэша. Вход — Debug-представление параметров
/// (детерминировано, f64 печатается shortest-roundtrip) + геометрия:
/// start/span/source_in/reversed задают отображение t→(src_pts, u),
/// frame_step — сетку (смена fps проекта обязана инвалидировать).
fn fx_chain_hash(
    fx: &[kt_model::FxInstance],
    start: Tick,
    end: Tick,
    source_in: Tick,
    reversed: bool,
    frame_step: i64,
) -> u64 {
    let repr = format!(
        "{fx:?}|{}|{}|{}|{reversed}|{frame_step}",
        start.0,
        end.0 - start.0,
        source_in.0
    );
    xxhash_rust::xxh3::xxh3_64(repr.as_bytes())
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
    /// Пул RGBA-буферов FX-цепочек (п.2 р.7): без него каждый compose
    /// платил ~6 полноразмерных аллокаций.
    fx_scratch: kt_fx::video::FxScratch,
}

/// Ключ кадра в кэше: idx = pts источника в тиках (выравнен по сетке проекта).
pub fn video_cache_idx(source_pts: Tick) -> i64 {
    source_pts.0
}

impl VideoRenderer {
    pub fn new(project: &Project) -> VideoRenderer {
        // effective_clips: morpher'ы применены здесь, движок их не видит (H1)
        let effective = project.effective_clips();
        let grid = FpsGrid::new(project.settings.fps);
        let mut clips: Vec<VideoClip> = effective
            .iter()
            .filter(|c| c.kind == ClipKind::Video)
            .map(|c| {
                let fx = c.active_fx(); // bypass-слоты отфильтрованы (п.2)
                let fx_hash = fx_chain_hash(
                    &fx,
                    c.start,
                    c.start + c.duration,
                    c.source_in,
                    c.reversed,
                    grid.frame_step(),
                );
                VideoClip {
                    start: c.start,
                    end: c.start + c.duration,
                    source_in: c.source_in,
                    lane: c.lane,
                    media: c.media,
                    reversed: c.reversed,
                    fx,
                    fx_hash,
                }
            })
            .collect();
        // задний план первым (большая lane), передний (lane 0) — последним
        clips.sort_by_key(|c| std::cmp::Reverse((c.lane, c.start.0)));
        VideoRenderer { clips, grid, fx_scratch: Default::default() }
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
        &mut self,
        cache: &BurstCache,
        t: Tick,
        quality: Quality,
    ) -> (Vec<u8>, bool) {
        let mut out = Vec::new();
        let complete = self.compose_from_cache_into(cache, t, quality, &mut out);
        (out, complete)
    }

    /// Вариант с переиспользуемым буфером (превью 60 раз/с: аллокация
    /// полного RGBA-кадра на каждый кадр была заметной, п.1 раунда 5).
    pub fn compose_from_cache_into(
        &mut self,
        cache: &BurstCache,
        t: Tick,
        quality: Quality,
        out: &mut Vec<u8>,
    ) -> bool {
        let w = quality.width as usize;
        let h = quality.height as usize;
        out.clear();
        out.resize(w * h * 4, 0);
        let mut complete = true;
        let q_key = (quality.width, quality.height);
        let scratch = &mut self.fx_scratch;
        for clip in self.clips.iter().filter(|c| c.start <= t && t < c.end) {
            let src_pts = clip_source_pts(clip, t, &self.grid);
            let key_idx = video_cache_idx(src_pts);
            let sid = SourceId(clip.media.0);
            // пре-FX кадр читается ВСЕГДА (бамп recency: полоски прогретости
            // и prefetch не «остывают» под живым пост-FX кэшем)
            let pre = cache.get_video(sid, Some(quality), key_idx);
            if !kt_fx::video::has_video_fx(&clip.fx) {
                match pre {
                    Some(frame) => alpha_over(out, &frame.rgba, frame.has_alpha, w * h),
                    None => complete = false,
                }
                continue;
            }
            // цепочка видео-FX (п.25): кэш хранит ПРЕ-FX кадры (правда о
            // coverage), пост-FX кадр мемоизируется отдельным kind'ом
            // (п.2 р.7 — иначе цепочка платится на КАЖДЫЙ compose и loop
            // играет 6.8 fps даже прогретым); один код для превью и
            // экспорта (инвариант №7), эффекты детерминированы по
            // (t, fx_hash) → байт-идентичность сохранена.
            if let Some(f) = cache.get_postfx_video(sid, q_key, clip.fx_hash, t.0) {
                if (f.width as usize, f.height as usize) == (w, h) {
                    alpha_over(out, &f.rgba, f.has_alpha, w * h);
                }
                continue;
            }
            let Some(frame) = pre else {
                complete = false;
                continue;
            };
            let span = (clip.end.0 - clip.start.0).max(1);
            let u = (t.0 - clip.start.0) as f64 / span as f64;
            let ctx = kt_fx::video::VideoFxCtx {
                out_w: quality.width,
                out_h: quality.height,
                u,
                burst_index: t.0,
            };
            let mut rgba = scratch.take(frame.rgba.len());
            rgba.copy_from_slice(&frame.rgba);
            let f = kt_fx::video::apply_video_chain_pooled(
                &clip.fx,
                kt_fx::video::Frame {
                    w: frame.width,
                    h: frame.height,
                    rgba,
                    has_alpha: frame.has_alpha,
                },
                &ctx,
                scratch,
            );
            if (f.w as usize, f.h as usize) == (w, h) {
                alpha_over(out, &f.rgba, f.has_alpha, w * h);
            }
            cache.insert_postfx_video(
                sid,
                q_key,
                clip.fx_hash,
                t.0,
                std::sync::Arc::new(kt_media::VideoFrame {
                    pts: t,
                    width: f.w,
                    height: f.h,
                    rgba: f.rgba.into_boxed_slice(),
                    has_alpha: f.has_alpha,
                }),
            );
        }
        complete
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
