//! Видео-путь: выбор активных клипов, ключи кадров, построение плана
//! кадра (video_plan) и CPU-исполнитель композиции.
//!
//! Инвариант №7 (ред. Р.29): код композиции ОДИН — build_plan решает слои,
//! порядок, веса переходов и уровни для превью и экспорта; бэкенд
//! (CPU здесь / GPU в kt-gpu) — параметр исполнения, не второй путь.
//! CPU-исполнитель — байт-эталон (инвариант №11) и фолбэк. Слои
//! запрашиваются в ОДНОМ качестве и складываются alpha-over по z
//! (lane: меньше = выше, как в старом движке, где меньший индекс дорожки
//! был передним планом).

use std::sync::Arc;

use kt_cache::{BurstCache, SourceId};
use kt_media::{Quality, VideoFrame};
use kt_model::{ClipKind, MediaId, Project};
use kt_time::{FpsGrid, Tick};

use crate::video_plan::{Cam3d, FramePlan, LayerSrc, PlanLayer};

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
    /// Кривые автоматизации по слотам (Р.30), выровнены с fx (bypass уже
    /// отфильтрован вместе — active_fx_auto).
    auto: Vec<Vec<kt_model::ParamCurve>>,
    /// Ключ пост-FX кэша (п.2 р.7): параметры цепочки + геометрия клипа +
    /// шаг сетки. Любая правка модели пересоздаёт VideoRenderer → хэш
    /// пересчитывается, стейл-кадры не читаются (их ключ не совпадёт).
    fx_hash: u64,
    /// Окна dissolve из переходов (Р.24, п.137), тики [w0, w1): клип —
    /// incoming (нарастает) / outgoing (затухает). Пара блендится линейно.
    fade_in: Option<(Tick, Tick)>,
    fade_out: Option<(Tick, Tick)>,
    /// Вид перехода со стороны INCOMING (пара считается в его слое) и
    /// вид сольного фейда — Р.33 С4. Кроссфейд = прежний путь.
    fade_in_mix: crate::TransMix,
    fade_out_mix: crate::TransMix,
    /// Фейд (Р.25): напротив окна пустота (край дорожки/Gap), пары нет и
    /// ЖДАТЬ её не надо — слой уходит в прозрачность (под ним просвечивает
    /// нижняя дорожка, а её нет — чёрное).
    fade_in_solo: bool,
    fade_out_solo: bool,
    /// Непрозрачность слоя 0..4096 (Р.28, п.12: «уровень» события как в
    /// Vegas). Множится на альфу при композиции — под полупрозрачным
    /// клипом просвечивает нижняя дорожка.
    opacity_k: u32,
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
    auto: &[Vec<kt_model::ParamCurve>],
    start: Tick,
    end: Tick,
    source_in: Tick,
    reversed: bool,
    frame_step: i64,
) -> u64 {
    // кривые автоматизации — часть ключа (Р.30): правка кейфрейма обязана
    // инвалидировать пост-FX кадры, пережившие пересоздание рендерера
    let repr = format!(
        "{fx:?}|{auto:?}|{}|{}|{}|{reversed}|{frame_step}",
        start.0,
        end.0 - start.0,
        source_in.0
    );
    xxhash_rust::xxh3::xxh3_64(repr.as_bytes())
}

/// pts источника для момента таймлайна `t` внутри клипа: прямой или
/// зеркальный (п.16 реверс). Чистая функция — тест 24.
fn clip_source_pts(c: &VideoClip, t: Tick, grid: &FpsGrid) -> Tick {
    grid.align_floor(clip_material_pos(c, t, grid))
}

/// Материальная позиция момента `t` БЕЗ выравнивания по сетке — в ней
/// живут ключи автоматизации (Р.33). Правило одно на движок, модель и UI
/// (`kt_model::clip_material_at`): зеркало реверса, кламп краёв — там же.
fn clip_material_pos(c: &VideoClip, t: Tick, grid: &FpsGrid) -> Tick {
    kt_model::clip_material_at(
        c.source_in,
        c.start,
        Tick(c.end.0 - c.start.0),
        c.reversed,
        grid.frame_step(),
        t,
    )
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
    /// Снапшот камер проекта (Р.29): активная/прибитая камера Pan3d
    /// разрешается при построении плана тем же правилом, что в модели.
    cameras: Vec<kt_model::CameraEvent>,
    /// Размер проекта в px — мировая система координат 3D-эффектов.
    project_size: (u32, u32),
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
                let auto = c.active_fx_auto();
                let fx_hash = fx_chain_hash(
                    &fx,
                    &auto,
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
                    auto,
                    fx_hash,
                    fade_in: None,
                    fade_out: None,
                    fade_in_mix: Default::default(),
                    fade_out_mix: Default::default(),
                    fade_in_solo: false,
                    fade_out_solo: false,
                    opacity_k: (c.opacity.clamp(0.0, 1.0) * 4096.0).round() as u32,
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
            let solo = t.is_fade();
            if let Some(a) = t.a.and_then(|id| clips.iter_mut().find(|c| c.id == id)) {
                a.fade_out = Some((w0, w1));
                a.fade_out_solo = solo;
                a.fade_out_mix = crate::TransMix::of(&t.kind);
                a.tail_ext = Tick((w1.0 - a.end.0).max(0));
            }
            if let Some(b) = t.b.and_then(|id| clips.iter_mut().find(|c| c.id == id)) {
                b.fade_in = Some((w0, w1));
                b.fade_in_solo = solo;
                b.fade_in_mix = crate::TransMix::of(&t.kind);
                b.head_ext = Tick((b.start.0 - w0.0).max(0));
            }
        }
        // Порядок композиции: задний план первым (большая lane), передний
        // (lane 0) — последним. Внутри ОДНОЙ дорожки при наезде сверху тот,
        // кто начался ПОЗЖЕ, — правило Vegas (решение пользователя после
        // Р.28; до этого работало правило v1 «сверху тот, у кого start
        // меньше», и полоса таймлайна с ним расходилась). Тест 162.
        clips.sort_by_key(|c| (std::cmp::Reverse(c.lane), c.start.0));
        VideoRenderer {
            clips,
            grid,
            fx_scratch: Default::default(),
            blend: Vec::new(),
            cameras: project.cameras.clone(),
            project_size: (project.settings.width, project.settings.height),
        }
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
    /// Строит план кадра и исполняет его CPU-исполнителем (Р.29).
    pub fn compose_from_cache_into(
        &mut self,
        cache: &BurstCache,
        t: Tick,
        quality: Quality,
        out: &mut Vec<u8>,
    ) -> bool {
        let ctx = PlanCtx {
            grid: &self.grid,
            cameras: &self.cameras,
            project_size: self.project_size,
        };
        let plan = build_plan(&self.clips, &ctx, cache, t, quality, true);
        execute_plan_cpu(&plan, cache, &mut self.fx_scratch, &mut self.blend, out)
    }

    /// Экспортный вариант: без пост-FX мемоизации (Р.31 п.4 — см.
    /// FramePlan::memoize_postfx). Байты кадра идентичны compose_from_cache.
    pub fn compose_export_into(
        &mut self,
        cache: &BurstCache,
        t: Tick,
        quality: Quality,
        out: &mut Vec<u8>,
    ) -> bool {
        let ctx = PlanCtx {
            grid: &self.grid,
            cameras: &self.cameras,
            project_size: self.project_size,
        };
        let plan = build_plan(&self.clips, &ctx, cache, t, quality, false);
        execute_plan_cpu(&plan, cache, &mut self.fx_scratch, &mut self.blend, out)
    }

    /// Максимум одновременных видео-слоёв (с ручками переходов) — экспорт
    /// делит бюджет чанка на это число (Р.31 п.4).
    pub fn max_concurrent_layers(&self) -> usize {
        let mut evs: Vec<(i64, i32)> = Vec::with_capacity(self.clips.len() * 2);
        for c in &self.clips {
            evs.push((c.start.0 - c.head_ext.0, 1));
            evs.push((c.end.0 + c.tail_ext.0, -1));
        }
        evs.sort();
        let (mut cur, mut max) = (0i32, 0i32);
        for (_, d) in evs {
            cur += d;
            max = max.max(cur);
        }
        max.max(1) as usize
    }

    /// План кадра для внешнего исполнителя (GPU в kt-gpu). Пре-FX кадры
    /// уже прочитаны из кэша (recency бампнут), структура слоёв решена.
    pub fn frame_plan(&self, cache: &BurstCache, t: Tick, quality: Quality) -> FramePlan<'_> {
        let ctx = PlanCtx {
            grid: &self.grid,
            cameras: &self.cameras,
            project_size: self.project_size,
        };
        build_plan(&self.clips, &ctx, cache, t, quality, true)
    }
}

/// Контекст построения плана: сетка, камеры и мировой размер проекта.
struct PlanCtx<'a> {
    grid: &'a FpsGrid,
    cameras: &'a [kt_model::CameraEvent],
    project_size: (u32, u32),
}

/// Источник слоя: pts, пре-FX кадр (читается ВСЕГДА — бамп recency, чтобы
/// полоски прогретости и prefetch не «остывали» под живым пост-FX кэшем),
/// цепочка и прогресс клипа.
fn layer_src<'a>(
    clip: &'a VideoClip,
    ctx: &PlanCtx<'_>,
    cache: &BurstCache,
    quality: Quality,
    t: Tick,
) -> LayerSrc<'a> {
    let src_pts = clip_source_pts(clip, t, ctx.grid);
    // кривые сэмплируются по ВЫРОВНЕННОЙ материальной позиции — по тому
    // самому кадру источника, который сейчас на экране (Р.33)
    let mat = src_pts;
    let cache_idx = video_cache_idx(src_pts);
    let source = SourceId(clip.media.0);
    let frame = cache.get_video(source, Some(quality), cache_idx);
    let span = (clip.end.0 - clip.start.0).max(1);
    let u = (t.0 - clip.start.0) as f64 / span as f64;
    // автоматизация (Р.30): кривые разрешаются ЗДЕСЬ, один раз на кадр —
    // CPU и GPU получают уже вычисленные параметры (клон цепочки только
    // при живых кривых)
    let fx: std::borrow::Cow<'_, [kt_model::FxInstance]> =
        if clip.auto.iter().any(|c| c.iter().any(|p| !p.keys.is_empty())) {
            let mut owned = clip.fx.clone();
            for (i, f) in owned.iter_mut().enumerate() {
                if let Some(curves) = clip.auto.get(i) {
                    kt_model::apply_fx_auto(f, curves, mat);
                }
            }
            std::borrow::Cow::Owned(owned)
        } else {
            std::borrow::Cow::Borrowed(&clip.fx[..])
        };
    // 3D-эффект → разрешаем камеру: прибитая по id, иначе активная по t,
    // иначе дефолтная «50 мм» (тождество для Pan3d с дефолтами)
    let cam3d = clip
        .fx
        .iter()
        .find_map(|f| match f {
            kt_model::FxInstance::Pan3d { camera, .. } => Some(*camera),
            _ => None,
        })
        .map(|pinned| {
            let ev = pinned
                .and_then(|id| ctx.cameras.iter().find(|c| c.id == id))
                .or_else(|| kt_model::active_camera_in(ctx.cameras, t));
            let cam = ev.map(|e| e.params_at(t)).unwrap_or_default();
            Cam3d {
                cam,
                world_scale: quality.width as f64 / ctx.project_size.0.max(1) as f64,
            }
        });
    LayerSrc { source, cache_idx, frame, fx, fx_hash: clip.fx_hash, u, cam3d }
}

/// Построение плана кадра: какие слои, в каких ролях и с какими весами.
/// Пара клипов в окне перехода блендится линейным dissolve в ОДИН слой
/// (п.137); фейд у края/Gap (Р.25) — соло-слой, уходящий в прозрачность.
fn build_plan<'a>(
    clips: &'a [VideoClip],
    ctx: &PlanCtx<'_>,
    cache: &BurstCache,
    t: Tick,
    quality: Quality,
    memoize_postfx: bool,
) -> FramePlan<'a> {
    let mut layers = Vec::new();
    let mut has_3d = false;
    for (i, clip) in clips.iter().enumerate() {
        if !clip.active_at(t) {
            continue;
        }
        has_3d |= clip.fx.iter().any(|f| f.requires_gpu());
        // фейд у края/Gap: пары нет по модели, слой всегда идёт через
        // масштабирование альфы (scale_alpha)
        if let Some((w, incoming, mix)) = solo_fade(clip, t) {
            // кроссфейд — прежний путь (вес прямо в альфе, байты те же);
            // любой другой вид форму даёт ядром, а уровень остаётся
            // уровнем события
            let (alpha_k, fade) = if mix.is_crossfade() {
                (w * clip.opacity_k / 4096, None)
            } else {
                let k = if incoming { w } else { 4096 - w };
                (clip.opacity_k, Some(crate::video_plan::SoloFade { k, mix, incoming }))
            };
            layers.push(PlanLayer::One {
                src: layer_src(clip, ctx, cache, quality, t),
                alpha_k,
                scale_alpha: true,
                fade,
            });
            continue;
        }
        // outgoing половина пары рисуется в слое incoming
        if let Some(win) = clip.fade_out
            && t.0 >= win.0.0
            && t.0 < win.1.0
            && find_pair(clips, clip.lane, win, true).is_some()
        {
            continue;
        }
        if let Some(win) = clip.fade_in
            && t.0 >= win.0.0
            && t.0 < win.1.0
            && let Some(pi) = find_pair(clips, clip.lane, win, false)
        {
            let outgoing = &clips[pi];
            debug_assert_ne!(pi, i);
            let k = (((t.0 - win.0.0) * 4096) / (win.1.0 - win.0.0)).clamp(0, 4096) as u32;
            // уровни соседей смешиваются тем же весом, что и картинка
            let op = (outgoing.opacity_k * (4096 - k) + clip.opacity_k * k) / 4096;
            layers.push(PlanLayer::Dissolve {
                a: layer_src(outgoing, ctx, cache, quality, t),
                b: layer_src(clip, ctx, cache, quality, t),
                k,
                alpha_k: op,
                mix: clip.fade_in_mix,
            });
            continue;
        }
        layers.push(PlanLayer::One {
            src: layer_src(clip, ctx, cache, quality, t),
            alpha_k: clip.opacity_k,
            scale_alpha: clip.opacity_k < 4096,
            fade: None,
        });
    }
    FramePlan { t, out_w: quality.width, out_h: quality.height, layers, has_3d, memoize_postfx }
}

/// CPU-исполнение плана без держателя пулов (parity-тесты kt-gpu:
/// GPU-выход сверяется с этим эталоном; пулы одноразовые).
pub fn compose_plan_cpu(plan: &FramePlan<'_>, cache: &BurstCache, out: &mut Vec<u8>) -> bool {
    let mut scratch = kt_fx::video::FxScratch::default();
    let mut blend = Vec::new();
    execute_plan_cpu(plan, cache, &mut scratch, &mut blend, out)
}

/// CPU-исполнитель плана: недостающая половина пары → присутствующая с
/// полным весом + complete=false (playback не ждёт, инвариант №5); слой
/// чужого размера молча пропускается (контракт «декодер отдаёт Quality»).
fn execute_plan_cpu(
    plan: &FramePlan<'_>,
    cache: &BurstCache,
    scratch: &mut kt_fx::video::FxScratch,
    blend: &mut Vec<u8>,
    out: &mut Vec<u8>,
) -> bool {
    let w = plan.out_w as usize;
    let h = plan.out_h as usize;
    out.clear();
    out.resize(w * h * 4, 0);
    let mut complete = true;
    let q_key = (plan.out_w, plan.out_h);
    let fits = |f: &Arc<VideoFrame>| (f.width as usize, f.height as usize) == (w, h);
    for layer in &plan.layers {
        match layer {
            PlanLayer::One { src, alpha_k, scale_alpha, fade } => {
                match layer_frame_cpu(src, cache, q_key, scratch, plan.t, plan.memoize_postfx) {
                    Some(f) if fits(&f) => {
                        if let Some(sf) = fade {
                            // сольный фейд с видом: вторая сторона — пустота
                            let (fa, fb) =
                                if sf.incoming { (None, Some(&*f)) } else { (Some(&*f), None) };
                            crate::transitions::mix_into(blend, fa, fb, sf.k, sf.mix, w, h);
                            if *alpha_k < 4096 {
                                scale_alpha_inplace(blend, *alpha_k, w * h);
                            }
                            alpha_over(out, blend, true, w * h);
                        } else if *scale_alpha {
                            scale_alpha_into(blend, &f, *alpha_k, w * h);
                            alpha_over(out, blend, true, w * h);
                        } else {
                            alpha_over(out, &f.rgba, f.has_alpha, w * h);
                        }
                    }
                    Some(_) => {}
                    None => complete = false,
                }
            }
            PlanLayer::Dissolve { a, b, k, alpha_k, mix } => {
                let fa = layer_frame_cpu(a, cache, q_key, scratch, plan.t, plan.memoize_postfx);
                let fb = layer_frame_cpu(b, cache, q_key, scratch, plan.t, plan.memoize_postfx);
                match (fa, fb) {
                    (Some(fa), Some(fb)) if fits(&fa) && fits(&fb) => {
                        let mut has_alpha = if mix.is_crossfade() {
                            dissolve_into(blend, &fa, &fb, *k, w * h)
                        } else {
                            crate::transitions::mix_into(
                                blend,
                                Some(&fa),
                                Some(&fb),
                                *k,
                                *mix,
                                w,
                                h,
                            )
                        };
                        if *alpha_k < 4096 {
                            scale_alpha_inplace(blend, *alpha_k, w * h);
                            has_alpha = true;
                        }
                        alpha_over(out, blend, has_alpha, w * h);
                    }
                    (Some(f), None) | (None, Some(f)) if fits(&f) => {
                        if *alpha_k < 4096 {
                            scale_alpha_into(blend, &f, *alpha_k, w * h);
                            alpha_over(out, blend, true, w * h);
                        } else {
                            alpha_over(out, &f.rgba, f.has_alpha, w * h);
                        }
                        complete = false;
                    }
                    _ => complete = false,
                }
            }
        }
    }
    complete
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

/// Кадр слоя для CPU-исполнителя: пре-FX кадр из плана либо результат
/// FX-цепочки (мемоизированный пост-FX кэшем). None — кадра нет (голод).
/// Свежий пост-FX кадр возвращается ЧЕРЕЗ кэш (одна лишняя выборка на
/// холодную цепочку — дешевле клона кадра).
fn layer_frame_cpu(
    src: &LayerSrc<'_>,
    cache: &BurstCache,
    q_key: (u32, u32),
    scratch: &mut kt_fx::video::FxScratch,
    t: Tick,
    memoize: bool,
) -> Option<Arc<VideoFrame>> {
    if !kt_fx::video::has_video_fx(&src.fx) {
        return src.frame.clone();
    }
    // цепочка видео-FX (п.25): кэш хранит ПРЕ-FX кадры (правда о
    // coverage), пост-FX кадр мемоизируется отдельным kind'ом
    // (п.2 р.7 — иначе цепочка платится на КАЖДЫЙ compose и loop
    // играет 6.8 fps даже прогретым); эффекты детерминированы по
    // (t, fx_hash) → байт-идентичность сохранена. GPU-исполнитель эту
    // мемоизацию не трогает (у него свой ReadbackCache в kt-gpu).
    if let Some(f) = cache.get_postfx_video(src.source, q_key, src.fx_hash, t.0) {
        return Some(f);
    }
    let frame = src.frame.as_ref()?;
    let ctx = kt_fx::video::VideoFxCtx {
        out_w: q_key.0,
        out_h: q_key.1,
        u: src.u,
        burst_index: t.0,
    };
    let mut rgba = scratch.take(frame.rgba.len());
    rgba.copy_from_slice(&frame.rgba);
    let f = kt_fx::video::apply_video_chain_pooled(
        &src.fx,
        kt_fx::video::Frame { w: frame.width, h: frame.height, rgba, has_alpha: frame.has_alpha },
        &ctx,
        scratch,
    );
    let frame = Arc::new(VideoFrame {
        pts: t,
        width: f.w,
        height: f.h,
        rgba: f.rgba.into_boxed_slice(),
        has_alpha: f.has_alpha,
    });
    if !memoize {
        // экспорт (Р.31 п.4): кадр не переживёт этот t — не выжираем кэш
        return Some(frame);
    }
    cache.insert_postfx_video(src.source, q_key, src.fx_hash, t.0, frame);
    cache.get_postfx_video(src.source, q_key, src.fx_hash, t.0)
}

/// Сольный фейд (Р.25) в момент t: (вес 0..4096, клип ПОЯВЛЯЕТСЯ, вид).
/// fade_in нарастает 0→1, fade_out затухает 1→0. None — момент вне
/// сольного окна (обычный слой или половина настоящей пары).
fn solo_fade(clip: &VideoClip, t: Tick) -> Option<(u32, bool, crate::TransMix)> {
    let frac = |w: (Tick, Tick)| {
        (((t.0 - w.0.0) * 4096) / (w.1.0 - w.0.0).max(1)).clamp(0, 4096) as u32
    };
    if let Some(w) = clip.fade_in
        && clip.fade_in_solo
        && t.0 >= w.0.0
        && t.0 < w.1.0
    {
        return Some((frac(w), true, clip.fade_in_mix));
    }
    if let Some(w) = clip.fade_out
        && clip.fade_out_solo
        && t.0 >= w.0.0
        && t.0 < w.1.0
    {
        return Some((4096 - frac(w), false, clip.fade_out_mix));
    }
    None
}

/// Альфа буфера, умноженная на k/4096 НА МЕСТЕ (уровень поверх готового
/// блендa пары — см. compose_from_cache_into).
fn scale_alpha_inplace(buf: &mut [u8], k: u32, pixels: usize) {
    for p in 0..pixels {
        let a = buf[p * 4 + 3] as u32;
        buf[p * 4 + 3] = ((a * k) / 4096) as u8;
    }
}

/// Копия кадра с альфой, умноженной на k/4096 (кадр без альфы считается
/// непрозрачным). Дальше идёт обычный alpha_over — под фейдом просвечивает
/// нижняя дорожка, а если её нет — чёрный фон композиции.
fn scale_alpha_into(dst: &mut Vec<u8>, f: &kt_media::VideoFrame, k: u32, pixels: usize) {
    let n = pixels * 4;
    dst.clear();
    dst.resize(n, 0);
    let pixels_avail = pixels.min(f.rgba.len() / 4);
    let mut start = 0usize;
    #[cfg(target_arch = "x86_64")]
    if kt_fx::simd::simd_level() == kt_fx::simd::SimdLevel::Avx2 && pixels_avail >= 8 {
        // SAFETY: уровень проверен детектом; обе длины >= pixels_avail*4
        unsafe {
            crate::video_simd::scale_alpha_avx2(dst, &f.rgba, f.has_alpha, k, pixels_avail);
        }
        start = pixels_avail & !7; // хвост — скаляром ниже
    }
    for p in start..pixels_avail {
        let i = p * 4;
        dst[i..i + 3].copy_from_slice(&f.rgba[i..i + 3]);
        let a = if f.has_alpha { f.rgba[i + 3] as u32 } else { 255 };
        dst[i + 3] = ((a * k + 2048) >> 12) as u8;
    }
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
    let pixels_avail = pixels.min(a.rgba.len() / 4).min(b.rgba.len() / 4);
    let mut start = 0usize;
    #[cfg(target_arch = "x86_64")]
    if kt_fx::simd::simd_level() == kt_fx::simd::SimdLevel::Avx2 && pixels_avail >= 4 {
        // SAFETY: уровень проверен детектом; длины >= pixels_avail*4
        unsafe {
            crate::video_simd::dissolve_avx2(
                dst,
                &a.rgba,
                &b.rgba,
                a.has_alpha,
                b.has_alpha,
                k,
                pixels_avail,
            );
        }
        start = pixels_avail & !3;
    }
    for p in start..pixels_avail {
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
    let pixels_avail = pixels.min(src.len() / 4).min(dst.len() / 4);
    let mut start = 0usize;
    #[cfg(target_arch = "x86_64")]
    if kt_fx::simd::simd_level() == kt_fx::simd::SimdLevel::Avx2 && pixels_avail >= 8 {
        // SAFETY: уровень проверен детектом; длины >= pixels_avail*4
        unsafe { crate::video_simd::alpha_over_avx2(dst, src, pixels_avail) };
        start = pixels_avail & !7;
    }
    for p in start..pixels_avail {
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
