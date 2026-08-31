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
    id: kt_model::ClipId,
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
    /// Окна dissolve из переходов (Р.24, п.137), тики [w0, w1): клип —
    /// incoming (нарастает) / outgoing (затухает). Пара блендится линейно.
    fade_in: Option<(Tick, Tick)>,
    fade_out: Option<(Tick, Tick)>,
    /// «Ручки»: материал за краями тела (тики) — источник продолжается
    /// линейно, за краями материала кадр клампится (политика краёв).
    head_ext: Tick,
    tail_ext: Tick,
}

impl VideoClip {
    /// Активен ли клип в момент t с учётом ручек перехода.
    fn active_at(&self, t: Tick) -> bool {
        self.start.0 - self.head_ext.0 <= t.0 && t.0 < self.end.0 + self.tail_ext.0
    }
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
        // конец использованного куска источника (минус шаг кадра).
        // Правая ручка перехода (t за концом) упирается в кламп source_in —
        // «держим кадр» (политика краёв, п.137)
        let span = c.end.0 - c.start.0;
        let step = grid.frame_step();
        Tick((c.source_in.0 + span - step - (t.0 - c.start.0)).max(c.source_in.0))
    } else {
        // левая ручка (t до начала) может выйти за нулевой кадр источника —
        // кламп-кадр 0
        Tick((c.source_in.0 + (t.0 - c.start.0)).max(0))
    };
    grid.align_floor(src)
}

pub struct VideoRenderer {
    clips: Vec<VideoClip>,
    pub grid: FpsGrid,
    /// Пул RGBA-буферов FX-цепочек (п.2 р.7): без него каждый compose
    /// платил ~6 полноразмерных аллокаций.
    fx_scratch: kt_fx::video::FxScratch,
    /// Переиспользуемый буфер бленда пары перехода (п.137): аллокация
    /// один раз, не на кадр.
    blend: Vec<u8>,
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
                    id: c.id,
                    start: c.start,
                    end: c.start + c.duration,
                    source_in: c.source_in,
                    lane: c.lane,
                    media: c.media,
                    reversed: c.reversed,
                    fx,
                    fx_hash,
                    fade_in: None,
                    fade_out: None,
                    head_ext: Tick::ZERO,
                    tail_ext: Tick::ZERO,
                }
            })
            .collect();
        // окна dissolve из переходов (Р.24): пары находят друг друга по
        // равенству окна на одной дорожке (compose блендит их одним слоем)
        for t in &project.transitions {
            let Some(joint) = t.joint(project) else { continue };
            let (w0, w1) = (joint - t.in_offset, joint + t.out_offset);
            if w1.0 <= w0.0 {
                continue;
            }
            if let Some(a) = clips.iter_mut().find(|c| c.id == t.a) {
                a.fade_out = Some((w0, w1));
                a.tail_ext = Tick((w1.0 - a.end.0).max(0));
            }
            if let Some(b) = clips.iter_mut().find(|c| c.id == t.b) {
                b.fade_in = Some((w0, w1));
                b.head_ext = Tick((b.start.0 - w0.0).max(0));
            }
        }
        // задний план первым (большая lane), передний (lane 0) — последним
        clips.sort_by_key(|c| std::cmp::Reverse((c.lane, c.start.0)));
        VideoRenderer { clips, grid, fx_scratch: Default::default(), blend: Vec::new() }
    }

    /// Кадры, нужные для композиции момента `t` (выравненного вызывающим):
    /// (media, pts источника). Порядок — back-to-front.
    pub fn layers_at(&self, t: Tick) -> Vec<(MediaId, Tick)> {
        self.clips
            .iter()
            .filter(|c| c.active_at(t))
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
    /// Пара клипов в окне перехода блендится линейным dissolve в ОДИН слой
    /// (п.137), затем alpha_over как обычно; недостающая половина пары →
    /// присутствующая с полным весом + complete=false (playback не ждёт,
    /// инвариант №5).
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
        let grid = &self.grid;
        for i in 0..self.clips.len() {
            let clip = &self.clips[i];
            if !clip.active_at(t) {
                continue;
            }
            // outgoing половина пары рисуется на итерации incoming
            if let Some(win) = clip.fade_out
                && t.0 >= win.0.0
                && t.0 < win.1.0
                && find_pair(&self.clips, clip.lane, win, true).is_some()
            {
                continue;
            }
            if let Some(win) = clip.fade_in
                && t.0 >= win.0.0
                && t.0 < win.1.0
                && let Some(pi) = find_pair(&self.clips, clip.lane, win, false)
            {
                let outgoing = &self.clips[pi];
                let fa = clip_layer(outgoing, grid, cache, quality, q_key, scratch, t);
                let fb = clip_layer(clip, grid, cache, quality, q_key, scratch, t);
                let k = (((t.0 - win.0.0) * 4096) / (win.1.0 - win.0.0)).clamp(0, 4096) as u32;
                let fits = |f: &std::sync::Arc<kt_media::VideoFrame>| {
                    (f.width as usize, f.height as usize) == (w, h)
                };
                match (fa, fb) {
                    (Some(a), Some(b)) if fits(&a) && fits(&b) => {
                        let has_alpha =
                            dissolve_into(&mut self.blend, &a, &b, k, w * h);
                        alpha_over(out, &self.blend, has_alpha, w * h);
                    }
                    (Some(f), None) | (None, Some(f)) if fits(&f) => {
                        alpha_over(out, &f.rgba, f.has_alpha, w * h);
                        complete = false;
                    }
                    _ => complete = false,
                }
                continue;
            }
            match clip_layer(clip, grid, cache, quality, q_key, scratch, t) {
                Some(f) => {
                    if (f.width as usize, f.height as usize) == (w, h) {
                        alpha_over(out, &f.rgba, f.has_alpha, w * h);
                    }
                }
                None => complete = false,
            }
        }
        complete
    }
}

/// Партнёр по переходу: клип той же дорожки с ТЕМ ЖЕ окном на противоположной
/// стороне (incoming ищет outgoing и наоборот). Клипов мало, линейный скан.
fn find_pair(
    clips: &[VideoClip],
    lane: u32,
    win: (Tick, Tick),
    incoming: bool,
) -> Option<usize> {
    clips.iter().position(|c| {
        c.lane == lane && (if incoming { c.fade_in } else { c.fade_out }) == Some(win)
    })
}

/// Слой одного клипа для момента t: пре-FX кадр из кэша либо результат
/// FX-цепочки (мемоизированный пост-FX кэшем). None — кадра нет (голод).
/// Логика — бывшее тело цикла compose_from_cache_into, вынесена ради
/// бленда пары перехода (п.137); свежий пост-FX кадр возвращается ЧЕРЕЗ
/// кэш (одна лишняя выборка на холодную цепочку — дешевле клона кадра).
fn clip_layer(
    clip: &VideoClip,
    grid: &FpsGrid,
    cache: &BurstCache,
    quality: Quality,
    q_key: (u32, u32),
    scratch: &mut kt_fx::video::FxScratch,
    t: Tick,
) -> Option<std::sync::Arc<kt_media::VideoFrame>> {
    let src_pts = clip_source_pts(clip, t, grid);
    let key_idx = video_cache_idx(src_pts);
    let sid = SourceId(clip.media.0);
    // пре-FX кадр читается ВСЕГДА (бамп recency: полоски прогретости
    // и prefetch не «остывают» под живым пост-FX кэшем)
    let pre = cache.get_video(sid, Some(quality), key_idx);
    if !kt_fx::video::has_video_fx(&clip.fx) {
        return pre;
    }
    // цепочка видео-FX (п.25): кэш хранит ПРЕ-FX кадры (правда о
    // coverage), пост-FX кадр мемоизируется отдельным kind'ом
    // (п.2 р.7 — иначе цепочка платится на КАЖДЫЙ compose и loop
    // играет 6.8 fps даже прогретым); один код для превью и
    // экспорта (инвариант №7), эффекты детерминированы по
    // (t, fx_hash) → байт-идентичность сохранена.
    if let Some(f) = cache.get_postfx_video(sid, q_key, clip.fx_hash, t.0) {
        return Some(f);
    }
    let frame = pre?;
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
        kt_fx::video::Frame { w: frame.width, h: frame.height, rgba, has_alpha: frame.has_alpha },
        &ctx,
        scratch,
    );
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
    cache.get_postfx_video(sid, q_key, clip.fx_hash, t.0)
}

/// Линейный dissolve пары (п.137): dst = a·(1−k) + b·k, целочисленно
/// (k из 4096 долей, округление к ближайшему). Кадр без альфы считается
/// непрозрачным (A=255). Возвращает has_alpha результата.
fn dissolve_into(
    dst: &mut Vec<u8>,
    a: &kt_media::VideoFrame,
    b: &kt_media::VideoFrame,
    k: u32,
    pixels: usize,
) -> bool {
    let n = pixels * 4;
    dst.clear();
    dst.resize(n, 0);
    let ik = 4096 - k;
    for p in 0..pixels.min(a.rgba.len() / 4).min(b.rgba.len() / 4) {
        let i = p * 4;
        for ch in 0..4 {
            let av = if ch == 3 && !a.has_alpha { 255 } else { a.rgba[i + ch] } as u32;
            let bv = if ch == 3 && !b.has_alpha { 255 } else { b.rgba[i + ch] } as u32;
            dst[i + ch] = ((av * ik + bv * k + 2048) >> 12) as u8;
        }
    }
    a.has_alpha || b.has_alpha
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
