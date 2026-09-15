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
    /// Переход со стороны INCOMING (пара считается в его слое) и
    /// сольный фейд — Р.33 С4. Вид разрешается ПО ВРЕМЕНИ (Р.34 К4:
    /// у параметров вида есть кривые), поэтому здесь лежит не готовый
    /// `TransMix`, а вид с кривыми и начало окна.
    fade_in_trans: Option<TransAnim>,
    fade_out_trans: Option<TransAnim>,
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
    /// Растяжение времени (Slowed+Reverb на ВИДЕО-клипе, Р.37 п.2).
    /// `None` — линейный путь БАЙТ-В-БАЙТ прежний, и на этом стоит
    /// GOLDEN_VIDEO.
    warp: Option<VideoWarp>,
    /// МАКСИМУМ нетто-зума цепочки за всю длительность клипа (Р.46,
    /// п.320): сколько раз цепочке нужен вход крупнее Quality. Считается
    /// один раз при построении рендерера (кривые автоматизации и камеры
    /// сэмплируются по сетке), ступень — по клипу целиком (решение
    /// владельца: один ключ кэша на клип).
    head_zoom: f64,
    /// Нативный размер файла, если известен. `None` — ступень 1: раздувать
    /// декод вслепую нельзя (заглушка отсутствующего файла, синтетика).
    native: Option<(u32, u32)>,
}

// Карта варпа клипа переехала в kt-model (Р.39): конструктор один на
// всех — рендер, превью и правило куска клипа берут её оттуда, поэтому
// разойтись «какой кадр показать» и «где режем» больше не могут.
use kt_model::ClipWarp as VideoWarp;

/// Переход, привязанный к клипу: вид, его кривые и начало окна. Вид
/// разрешается на КАЖДЫЙ момент — иначе анимация параметров не дошла бы
/// до картинки (Р.34 К4).
#[derive(Debug, Clone)]
struct TransAnim {
    kind: kt_model::TransitionKind,
    auto: Vec<kt_model::ParamCurve>,
    w0: Tick,
}

impl TransAnim {
    fn of(t: &kt_model::TransitionEvent, w0: Tick) -> TransAnim {
        TransAnim { kind: t.kind.clone(), auto: t.auto.clone(), w0 }
    }

    fn mix_at(&self, t: Tick) -> crate::TransMix {
        if self.auto.is_empty() {
            return crate::TransMix::of(&self.kind);
        }
        let mut k = self.kind.clone();
        for c in &self.auto {
            if !c.keys.is_empty() {
                k.set_param(
                    &c.param,
                    kt_model::curve_value(&c.keys, c.ease, Tick(t.0 - self.w0.0)),
                );
            }
        }
        crate::TransMix::of(&k)
    }
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
    let span = (c.end.0 - c.start.0).max(1);
    let Some(w) = &c.warp else {
        return kt_model::clip_material_at(
            c.source_in,
            c.start,
            Tick(span),
            c.reversed,
            grid.frame_step(),
            t,
        );
    };
    // Растяжение (Р.37 п.2): материал идёт по интегралу скорости. За телом
    // (ручки перехода) tau уходит из [0,1] — карта продолжает крайним
    // сегментом, ровно как chain_source_window_ext у аудио.
    let tau = (t.0 - c.start.0) as f64 / span as f64;
    let consumed = kt_fx::warp_material_ticks(w.base(), w.prof(), span, tau);
    if c.reversed {
        // ВАРП, ПОТОМ ЗЕРКАЛО — не наоборот. Аудио-путь (`fill_source`)
        // зеркалит УЖЕ потреблённый прямой поток; зеркало до варпа гнало бы
        // кривую скорости задом наперёд и рассинхронило A/V. Якорь —
        // фактически потреблённое `used`, а не длина события.
        Tick((c.source_in.0 + w.used() - grid.frame_step() - consumed).max(c.source_in.0))
    } else {
        Tick((c.source_in.0 + consumed).max(0))
    }
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

/// Кадр(ы) источника для момента `t`: основной и, в режиме «смешивание»,
/// сосед с весом (Р.37 С5). ЕДИНСТВЕННОЕ место, где решается пара —
/// `layers_at` и `layer_src` обязаны звать именно её, иначе префетч греет
/// не то, что читает композит, и это вечный промах кэша.
///
/// `k == 0` (позиция ровно на сетке: целые скорости, rate = 1, кламп у
/// края) пары НЕ даёт — путь остаётся байт-в-байт прежним.
fn clip_frame_keys(c: &VideoClip, t: Tick, grid: &FpsGrid) -> (Tick, Option<(Tick, u32)>) {
    let a = clip_source_pts(c, t, grid);
    if !c.warp.as_ref().is_some_and(|w| w.blend()) {
        return (a, None);
    }
    let m = clip_material_pos(c, t, grid);
    let step = grid.frame_step().max(1);
    let k = (((m.0 - a.0) * 4096) / step).clamp(0, 4096) as u32;
    // сосед — ВПЕРЁД по индексу материала; у reversed материал идёт
    // навстречу, но дробная часть по-прежнему меряется вперёд
    if k == 0 { (a, None) } else { (a, Some((Tick(a.0 + step), k))) }
}

/// Ключ кадра в кэше: idx = pts источника в тиках (выравнен по сетке проекта).
pub fn video_cache_idx(source_pts: Tick) -> i64 {
    source_pts.0
}

/// Ступень декода клипа (Р.46, п.320): во сколько ЦЕЛЫХ раз пре-FX кадр
/// берётся крупнее Quality. Чистая функция от клипа и Quality —
/// детерминирована, от устройства и бюджетов НЕ зависит (иначе экспорт
/// зависел бы от машины).
///
/// Три множителя: сколько просит цепочка (`head_zoom` клипа, тем же
/// квантом, что тапы суперсемплинга), сколько может дать файл (запас
/// натива над Quality, округление ВВЕРХ — решение владельца: остаток
/// дотянет sws при декоде) и потолок промежуточных буферов.
fn clip_decode_scale(clip: &VideoClip, quality: Quality) -> u32 {
    let (qw, qh) = (quality.width.max(2), quality.height.max(2));
    let want = kt_fx::geom::ss_taps(clip.head_zoom);
    if want <= 1 {
        return 1;
    }
    let Some((nw, nh)) = clip.native else { return 1 };
    // запас файла: сколько ЦЕЛЫХ Quality в нативе, с округлением вверх
    // (натив 1920 при Quality 1280 — это ступень 2, а не 1)
    let spare_w = (nw as f64 / qw as f64).ceil() as u32;
    let spare_h = (nh as f64 / qh as f64).ceil() as u32;
    let spare = spare_w.min(spare_h).max(1);
    let cap = (kt_fx::plan::MAX_INTER_DIM / qw.max(qh)).max(1);
    want.min(spare).min(cap)
}

/// Качество декода клипа. ЕДИНСТВЕННОЕ место решения: заказ префетча,
/// чтение композита и полоски прогретости обязаны спрашивать его — иначе
/// греется не то, что читается (t247d по оси размера, t312).
fn clip_decode_quality(clip: &VideoClip, quality: Quality) -> Quality {
    let s = clip_decode_scale(clip, quality);
    if s == 1 {
        return quality;
    }
    Quality { width: quality.width * s, height: quality.height * s }
}

/// Максимум нетто-зума цепочки за длительность клипа (Р.46, п.320).
/// Кривые сэмплируются по ЛИНЕЙНОЙ материальной оси — той же, по которой
/// их читает `layer_src`; камеры — по таймлайну. Точки: равномерная сетка,
/// оба конца и все ключи кривых (экстремум зума почти всегда в ключе).
/// Логическая база — размер ПРОЕКТА: зум почти не зависит от Quality
/// (расхождения целочисленных огрублений сетки мельче кванта ступени), а
/// ступень обязана быть одинаковой на превью и экспорте.
#[allow(clippy::too_many_arguments)]
fn clip_head_zoom(
    fx: &[kt_model::FxInstance],
    auto: &[Vec<kt_model::ParamCurve>],
    start: Tick,
    end: Tick,
    source_in: Tick,
    reversed: bool,
    grid: &FpsGrid,
    cameras: &[kt_model::CameraEvent],
    project_size: (u32, u32),
) -> f64 {
    if !kt_fx::video::has_video_fx(fx) {
        return 1.0;
    }
    let (pw, ph) = (project_size.0.max(2), project_size.1.max(2));
    let has_3d = fx.iter().any(|f| matches!(f, kt_model::FxInstance::Pan3d { .. }));
    let live = auto.iter().any(|c| c.iter().any(|p| !p.keys.is_empty()));
    let span = (end.0 - start.0).max(1);
    let mut probes: Vec<Tick> = Vec::new();
    if live || has_3d || !cameras.is_empty() {
        const GRID: i64 = 32;
        for i in 0..=GRID {
            probes.push(Tick(start.0 + span * i / GRID));
        }
        for curves in auto {
            for c in curves {
                for k in &c.keys {
                    // ключ живёт в МАТЕРИАЛЬНОЙ оси — переводим в таймлайн
                    let t = kt_model::clip_timeline_at(
                        source_in,
                        start,
                        Tick(span),
                        reversed,
                        grid.frame_step(),
                        k.t,
                    );
                    if start.0 <= t.0 && t.0 <= end.0 {
                        probes.push(t);
                    }
                }
            }
        }
        for cam in cameras {
            for t in [cam.start, Tick(cam.start.0 + cam.duration.0)] {
                if start.0 <= t.0 && t.0 <= end.0 {
                    probes.push(t);
                }
            }
        }
    } else {
        // статика: зум одинаков на всей длительности, хватает середины
        probes.push(Tick(start.0 + span / 2));
    }
    let mut best = 1.0f64;
    let mut owned = fx.to_vec();
    for t in probes {
        let mat = kt_model::clip_material_at(
            source_in,
            start,
            Tick(span),
            reversed,
            grid.frame_step(),
            t,
        );
        if live {
            owned.clone_from_slice(fx);
            for (i, f) in owned.iter_mut().enumerate() {
                if let Some(curves) = auto.get(i) {
                    kt_model::apply_fx_auto(f, curves, mat);
                }
            }
        }
        let cam3d = has_3d.then(|| {
            let ev = kt_model::active_camera_in(cameras, t);
            let cam = ev.map(|e| e.params_at(t)).unwrap_or_default();
            // логика считается на размере проекта — world_scale ровно 1
            Cam3d { cam, world_scale: 1.0 }
        });
        let u = (t.0 - start.0) as f64 / span as f64;
        best = best.max(kt_fx::plan::chain_head_zoom(&owned, pw, ph, u, cam3d));
    }
    best
}

impl VideoRenderer {
    pub fn new(project: &Project) -> VideoRenderer {
        VideoRenderer::new_with_natives(project, &std::collections::HashMap::new())
    }

    /// Рендерер, знающий НАТИВНЫЕ размеры источников (Р.46, п.320): без
    /// них ступень декода всегда 1 — раздувать декод, не зная запаса
    /// файла, нельзя. Плеер и экспорт зовут этот конструктор.
    pub fn new_with_natives(
        project: &Project,
        natives: &std::collections::HashMap<MediaId, (u32, u32)>,
    ) -> VideoRenderer {
        VideoRenderer::build(project, natives)
    }

    fn build(
        project: &Project,
        natives: &std::collections::HashMap<MediaId, (u32, u32)>,
    ) -> VideoRenderer {
        // effective_clips: morpher'ы применены здесь, движок их не видит (H1)
        let effective = project.effective_clips();
        let grid = FpsGrid::new(project.settings.fps);
        let mut clips: Vec<VideoClip> = effective
            .iter()
            .filter(|c| c.kind == ClipKind::Video)
            .map(|c| {
                let fx = c.active_fx(); // bypass-слоты отфильтрованы (п.2)
                let auto = c.active_fx_auto();
                let warp = kt_model::clip_warp_of(&fx, &auto, c.source_in, c.duration, c.reversed);
                let fx_hash = fx_chain_hash(
                    &fx,
                    &auto,
                    c.start,
                    c.start + c.duration,
                    c.source_in,
                    c.reversed,
                    grid.frame_step(),
                );
                // ступень декода — свойство клипа (Р.46): максимум зума по
                // всей длительности, считается до перемещения цепочки
                let head_zoom = clip_head_zoom(
                    &fx,
                    &auto,
                    c.start,
                    c.start + c.duration,
                    c.source_in,
                    c.reversed,
                    &grid,
                    &project.cameras,
                    (project.settings.width, project.settings.height),
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
                    fade_in_trans: None,
                    fade_out_trans: None,
                    fade_in_solo: false,
                    fade_out_solo: false,
                    opacity_k: (c.opacity.clamp(0.0, 1.0) * 4096.0).round() as u32,
                    head_ext: Tick::ZERO,
                    tail_ext: Tick::ZERO,
                    warp,
                    head_zoom,
                    native: natives.get(&c.media).copied(),
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
            // морфер-слой мог разрезать цель (Р.35): id остался у куска
            // №0, а стык достался тому куску, чья граница на нём стоит —
            // без этого переход на разрезанном клипе молча исчезал
            if let Some(a) = t.a.and_then(|id| {
                clips.iter_mut().find(|c| kt_model::same_clip(c.id, id) && c.end == joint)
            }) {
                a.fade_out = Some((w0, w1));
                a.fade_out_solo = solo;
                a.fade_out_trans = Some(TransAnim::of(t, w0));
                a.tail_ext = Tick((w1.0 - a.end.0).max(0));
            }
            if let Some(b) = t.b.and_then(|id| {
                clips.iter_mut().find(|c| kt_model::same_clip(c.id, id) && c.start == joint)
            }) {
                b.fade_in = Some((w0, w1));
                b.fade_in_solo = solo;
                b.fade_in_trans = Some(TransAnim::of(t, w0));
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
    /// (media, pts источника, КАЧЕСТВО ДЕКОДА). Порядок — back-to-front.
    ///
    /// Качество — часть заказа (Р.46, п.318), а не свойство сессии: клип с
    /// зумящей цепочкой декодируется крупнее Quality. Вызывающий обязан
    /// греть и читать РОВНО это качество — иначе вечный промах кэша
    /// (t247d), теперь и по оси размера (t312).
    pub fn layers_at(&self, t: Tick, quality: Quality) -> Vec<(MediaId, Tick, Quality)> {
        self.clips
            .iter()
            .filter(|c| c.active_at(t))
            .flat_map(|c| {
                let dq = clip_decode_quality(c, quality);
                // в режиме «смешивание» слой читает ДВА кадра — префетч
                // обязан согреть оба, иначе второй не запросят никогда
                let (a, b) = clip_frame_keys(c, t, &self.grid);
                std::iter::once((c.media, a, dq)).chain(b.map(|(p, _)| (c.media, p, dq)))
            })
            .collect()
    }

    /// Все ключи кадров для диапазона таймлайна (prefetch/экспорт).
    pub fn wanted_frames(
        &self,
        range: kt_time::TickRange,
        quality: Quality,
    ) -> Vec<(MediaId, Tick, Quality)> {
        let mut out = Vec::new();
        let step = self.grid.frame_step();
        let mut t = self.grid.align_floor(range.start);
        while t < range.end {
            out.extend(self.layers_at(t, quality));
            t = Tick(t.0 + step);
        }
        out.sort();
        out.dedup();
        out
    }

    /// Качество декода каждого видео-клипа (Р.46, п.322): полоски
    /// прогретости в UI спрашивают coverage по НЕМУ. Id — эффективного
    /// клипа (морфер-слой мог нарезать исходный на куски с разными
    /// цепочками, а значит и разными ступенями).
    pub fn video_decode_qualities(&self, quality: Quality) -> Vec<(kt_model::ClipId, Quality)> {
        self.clips.iter().map(|c| (c.id, clip_decode_quality(c, quality))).collect()
    }

    /// Максимальное качество декода среди клипов (Р.46, п.318): бюджеты
    /// кэша и лукахеда считают память по НЕМУ, а не по Quality — иначе
    /// чанк экспорта недооценён в S² раз (п.15: «чёрный рендер»).
    pub fn max_decode_quality(&self, quality: Quality) -> Quality {
        self.clips.iter().map(|c| clip_decode_quality(c, quality)).max().unwrap_or(quality)
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
        let ctx =
            PlanCtx { grid: &self.grid, cameras: &self.cameras, project_size: self.project_size };
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
        let ctx =
            PlanCtx { grid: &self.grid, cameras: &self.cameras, project_size: self.project_size };
        let plan = build_plan(&self.clips, &ctx, cache, t, quality, false);
        execute_plan_cpu(&plan, cache, &mut self.fx_scratch, &mut self.blend, out)
    }

    /// Максимум одновременных видео-слоёв (с ручками переходов) — экспорт
    /// делит бюджет чанка на это число (Р.31 п.4).
    pub fn max_concurrent_layers(&self) -> usize {
        let mut evs: Vec<(i64, i32)> = Vec::with_capacity(self.clips.len() * 2);
        for c in &self.clips {
            // смешивающий слой держит ДВА кадра сразу (Р.37 С5). Не учесть
            // это — чанк экспорта вытеснит соседа до композиции, сработает
            // ветка голода, и БАЙТЫ ЭКСПОРТА станут зависеть от тайминга
            // кэша.
            let w = if c.warp.as_ref().is_some_and(|w| w.blend()) { 2 } else { 1 };
            evs.push((c.start.0 - c.head_ext.0, w));
            evs.push((c.end.0 + c.tail_ext.0, -w));
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
        let ctx =
            PlanCtx { grid: &self.grid, cameras: &self.cameras, project_size: self.project_size };
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
    let (src_pts, pair) = clip_frame_keys(clip, t, ctx.grid);
    // Кривые сэмплируются по ЛИНЕЙНОЙ материальной позиции — по той же,
    // которой рисует ось UI и по которой считает звук (Р.37: «БЕЗ учёта
    // замедления-ускорения»). До Р.39 здесь стоял `src_pts` — ВАРП-
    // зависимая позиция кадра, — и на замедленном клипе картинка
    // анимировалась по своему расписанию, а звук и UI по своему.
    //
    // Это разные вопросы, и путать их нельзя: «какой кадр показать» —
    // карта варпа (`clip_frame_keys`), «где мы на оси анимации» — линейка.
    let mat = kt_model::clip_material_at(
        clip.source_in,
        clip.start,
        clip.end - clip.start,
        clip.reversed,
        ctx.grid.frame_step(),
        t,
    );
    let cache_idx = video_cache_idx(src_pts);
    let source = SourceId(clip.media.0);
    // Р.46 (п.318): слой читает кадр СВОЕГО качества декода — того же,
    // которое заказал префетч через layers_at.
    let src_scale = clip_decode_scale(clip, quality);
    let dq = clip_decode_quality(clip, quality);
    let frame = cache.get_video(source, Some(dq), cache_idx);
    // СМЕШИВАНИЕ (Р.37 С5) делается ЗДЕСЬ, в общем коде плана: оба
    // исполнителя получают уже смешанный пре-FX кадр, и держать одну и ту
    // же арифметику синхронной в CPU-ядре и в WGSL не приходится
    // (инвариант №7). Ядро — готовый `dissolve_into`: байт-заморожено,
    // с AVX2-близнецом.
    let (frame, blend) = match pair {
        None => (frame, None),
        Some((pts, k)) => {
            let idx = video_cache_idx(pts);
            let nb = cache.get_video(source, Some(dq), idx);
            (blend_frames(frame, nb, k), Some(crate::video_plan::LayerBlend { cache_idx: idx, k }))
        }
    };
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
            Cam3d { cam, world_scale: quality.width as f64 / ctx.project_size.0.max(1) as f64 }
        });
    debug_assert!(
        src_scale == 1 || kt_fx::video::has_video_fx(&fx),
        "a clip without video fx never gets a decode stage"
    );
    LayerSrc { source, cache_idx, src_scale, frame, fx, fx_hash: clip.fx_hash, u, cam3d, blend }
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
    for (i, clip) in clips.iter().enumerate() {
        if !clip.active_at(t) {
            continue;
        }
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
            // вид разрешается по позиции ВНУТРИ окна: у его параметров
            // могут быть кривые (Р.34 К4)
            let mix = clip.fade_in_trans.as_ref().map(|a| a.mix_at(t)).unwrap_or_default();
            // уровни соседей смешиваются тем же весом, что и картинка
            let op = (outgoing.opacity_k * (4096 - k) + clip.opacity_k * k) / 4096;
            layers.push(PlanLayer::Dissolve {
                a: layer_src(outgoing, ctx, cache, quality, t),
                b: layer_src(clip, ctx, cache, quality, t),
                k,
                alpha_k: op,
                mix,
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
    FramePlan { t, out_w: quality.width, out_h: quality.height, layers, memoize_postfx }
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
fn find_pair(clips: &[VideoClip], lane: u32, win: (Tick, Tick), incoming: bool) -> Option<usize> {
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
    // цепочка видео-FX (п.25): кэш хранит ПРЕ-FX кадры (правда о
    // coverage), пост-FX кадр мемоизируется отдельным kind'ом
    // (п.2 р.7 — иначе цепочка платится на КАЖДЫЙ compose и loop
    // играет 6.8 fps даже прогретым); эффекты детерминированы по
    // (t, fx_hash) → байт-идентичность сохранена. GPU-исполнитель эту
    // мемоизацию не трогает (у него свой ReadbackCache в kt-gpu).
    let has_fx = kt_fx::video::has_video_fx(&src.fx);
    // СИЛА СЛОЯ (Р.38 п.12): слот `LayerMix` смешивает состояние ДО
    // вклада слоя с состоянием после него. Значение уже разрешено
    // автоматизацией при построении плана, а `fx_hash` хэширует всю
    // цепочку — 60 % и 100 % не схлопнутся в один кадр пост-FX кэша.
    let mixes = layer_mixes(&src.fx);
    // Камера — В КЛЮЧ мемоизации (Р.44): CPU считает Pan3d, а камера
    // разрешается ПО-КАДРОВО (анимация, правило активной камеры) — статика
    // fx_hash её не видит, и правка/перенос камеры отдавали бы стейл.
    // Зеркало put_cam3d в plan_key GPU-пути.
    // Ступень декода — тоже В КЛЮЧ (Р.46, п.318): один и тот же клип на
    // одном Quality даёт РАЗНЫЕ пост-FX байты при разном размере входа
    // (натив появился/исчез, сменилась кривая зума) — без неё кэш отдал бы
    // стейл (t313).
    let key_hash = postfx_key_hash(src.fx_hash, &src.cam3d, src.src_scale);
    if has_fx && let Some(f) = cache.get_postfx_video(src.source, q_key, key_hash, t.0) {
        return Some(f);
    }
    if !has_fx {
        // цепочка из одной силы пикселей не трогает: смешивать вход с
        // самим собой нечего
        return src.frame.clone();
    }
    let frame = src.frame.as_ref()?;
    let ctx = kt_fx::video::VideoFxCtx {
        out_w: q_key.0,
        out_h: q_key.1,
        u: src.u,
        burst_index: t.0,
        cam3d: src.cam3d,
        in_scale: src.src_scale,
    };
    let mut rgba = scratch.take(frame.rgba.len());
    rgba.copy_from_slice(&frame.rgba);
    let start =
        kt_fx::video::Frame { w: frame.width, h: frame.height, rgba, has_alpha: frame.has_alpha };
    let f = if mixes.is_empty() {
        // ПРЕЖНИЙ путь, байт-в-байт: на нём стоит GOLDEN_VIDEO
        kt_fx::video::apply_video_chain_pooled(&src.fx, start, &ctx, scratch)
    } else {
        apply_chain_with_mixes(&src.fx, &mixes, start, &ctx, scratch)
    };
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
    cache.insert_postfx_video(src.source, q_key, key_hash, t.0, frame);
    cache.get_postfx_video(src.source, q_key, key_hash, t.0)
}

/// Ключ пост-FX мемоизации: статический хэш цепочки + разрешённая камера
/// момента (Р.44, t66d). None — константное слагаемое: 2D-цепочки
/// по-прежнему различаются только fx_hash.
fn postfx_key_hash(fx_hash: u64, cam3d: &Option<Cam3d>, src_scale: u32) -> u64 {
    use xxhash_rust::xxh3::Xxh3;
    let mut h = Xxh3::new();
    h.update(&fx_hash.to_le_bytes());
    h.update(&src_scale.to_le_bytes());
    match cam3d {
        None => h.update(&[0u8]),
        Some(c) => {
            h.update(&[1u8, c.cam.ortho as u8]);
            for v in [
                c.cam.pos_x,
                c.cam.pos_y,
                c.cam.pos_z,
                c.cam.rot_x,
                c.cam.rot_y,
                c.cam.rot_z,
                c.cam.fov_deg,
                c.world_scale,
            ] {
                h.update(&v.to_bits().to_le_bytes());
            }
            if let Some((x, y, z)) = c.cam.target {
                for v in [x, y, z] {
                    h.update(&v.to_bits().to_le_bytes());
                }
            }
        }
    }
    h.digest()
}

/// Слоты смешения цепочки: (индекс слота, индекс начала вклада слоя,
/// вес 0..4096). Пусто — прежний путь БАЙТ-В-БАЙТ.
fn layer_mixes(fx: &[kt_model::FxInstance]) -> Vec<(usize, usize, u32)> {
    fx.iter()
        .enumerate()
        .filter_map(|(i, f)| match f {
            kt_model::FxInstance::LayerMix { amount, from } => {
                Some((i, (*from as usize).min(i), (amount.clamp(0.0, 1.0) * 4096.0).round() as u32))
            }
            _ => None,
        })
        .collect()
}

/// Цепочка со слотами СИЛЫ (Р.38 п.12): считается посегментно.
///
/// Слой дописывает свои эффекты в хвост цепочки цели, поэтому «сухая»
/// сторона смешения — состояние ДО его первого слота, а не вход всей
/// цепочки: иначе сила слоя откатывала бы и собственные эффекты события.
/// Снимок делается ровно на границе `from`, и на каждый слот силы он
/// ровно один.
fn apply_chain_with_mixes(
    fx: &[kt_model::FxInstance],
    mixes: &[(usize, usize, u32)],
    start: kt_fx::video::Frame,
    ctx: &kt_fx::video::VideoFxCtx,
    scratch: &mut kt_fx::video::FxScratch,
) -> kt_fx::video::Frame {
    let mut frame = start;
    // Ступень — свойство ВХОДНОГО кадра, а не цепочки (Р.46, п.318):
    // первый слайс получает крупный кадр, все последующие работают с уже
    // сведённым в Quality растром (стопы силы всегда на 1×).
    let entry_dims = (frame.w, frame.h);
    let mut snaps: std::collections::HashMap<usize, kt_fx::video::Frame> =
        std::collections::HashMap::new();
    // Границы (снимки сухой стороны и слоты силы) режут цепочку; МЕЖДУ ними
    // она подаётся ЦЕЛЬНЫМИ слайсами — иначе слияние гео-серий (Р.44) не
    // случалось бы у клипов с силой слоя. GPU-зеркало режет там же
    // (Snapshot-точки в chain_ops).
    let mut stops: Vec<usize> = mixes.iter().flat_map(|(at, from, _)| [*at, *from]).collect();
    stops.sort_unstable();
    stops.dedup();
    let mut i = 0usize;
    while i < fx.len() {
        if mixes.iter().any(|(_, from, _)| *from == i) {
            snaps.insert(
                i,
                kt_fx::video::Frame {
                    w: frame.w,
                    h: frame.h,
                    rgba: frame.rgba.clone(),
                    has_alpha: frame.has_alpha,
                },
            );
        }
        match mixes.iter().find(|(at, _, _)| *at == i) {
            Some((_, from, k)) if *k < 4096 => {
                if let Some(dry) = snaps.remove(from) {
                    frame = blend_frames_cpu(dry, frame, *k);
                }
                i += 1;
            }
            // сила 100 % — смешивать нечего, снимок просто выбрасывается
            Some((_, from, _)) => {
                snaps.remove(from);
                i += 1;
            }
            None => {
                let j = stops.iter().copied().find(|s| *s > i).unwrap_or(fx.len());
                let sctx = kt_fx::video::VideoFxCtx {
                    in_scale: if (frame.w, frame.h) == entry_dims { ctx.in_scale } else { 1 },
                    ..*ctx
                };
                frame = kt_fx::video::apply_video_chain_pooled(&fx[i..j], frame, &sctx, scratch);
                i = j;
            }
        }
    }
    frame
}

/// Смешать «сухой» кадр с «мокрым» весом `k` (0..4096).
///
/// Размеры могут не совпасть: Pan/CamMotion ресемплят кадр в размер
/// вывода. Тогда сухая сторона вписывается ТОЙ ЖЕ билинейкой
/// (`kt_fx::video::fit_into`), что и все остальные ресемплы, — её зеркало
/// в kt-gpu считает то же самое, поэтому бэкенды сходятся в допуске.
fn blend_frames_cpu(
    dry: kt_fx::video::Frame,
    wet: kt_fx::video::Frame,
    k: u32,
) -> kt_fx::video::Frame {
    let pixels = (wet.w as usize) * (wet.h as usize);
    let dry = if (dry.w, dry.h) == (wet.w, wet.h) {
        dry
    } else {
        let mut out = vec![0u8; pixels * 4];
        kt_fx::video::fit_into(&dry, wet.w, wet.h, &mut out);
        kt_fx::video::Frame { w: wet.w, h: wet.h, rgba: out, has_alpha: dry.has_alpha }
    };
    let as_vf = |f: &kt_fx::video::Frame| VideoFrame {
        pts: Tick::ZERO,
        width: f.w,
        height: f.h,
        rgba: f.rgba.clone().into_boxed_slice(),
        has_alpha: f.has_alpha,
    };
    let (a, b) = (as_vf(&dry), as_vf(&wet));
    let mut buf = Vec::new();
    dissolve_into(&mut buf, &a, &b, k, pixels);
    kt_fx::video::Frame { w: wet.w, h: wet.h, rgba: buf, has_alpha: wet.has_alpha || dry.has_alpha }
}

/// Сольный фейд (Р.25) в момент t: (вес 0..4096, клип ПОЯВЛЯЕТСЯ, вид).
/// fade_in нарастает 0→1, fade_out затухает 1→0. None — момент вне
/// сольного окна (обычный слой или половина настоящей пары).
fn solo_fade(clip: &VideoClip, t: Tick) -> Option<(u32, bool, crate::TransMix)> {
    let frac =
        |w: (Tick, Tick)| (((t.0 - w.0.0) * 4096) / (w.1.0 - w.0.0).max(1)).clamp(0, 4096) as u32;
    if let Some(w) = clip.fade_in
        && clip.fade_in_solo
        && t.0 >= w.0.0
        && t.0 < w.1.0
    {
        return Some((
            frac(w),
            true,
            clip.fade_in_trans.as_ref().map(|a| a.mix_at(t)).unwrap_or_default(),
        ));
    }
    if let Some(w) = clip.fade_out
        && clip.fade_out_solo
        && t.0 >= w.0.0
        && t.0 < w.1.0
    {
        return Some((
            4096 - frac(w),
            false,
            clip.fade_out_trans.as_ref().map(|a| a.mix_at(t)).unwrap_or_default(),
        ));
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
/// Смешать пару кадров (Р.37 С5). Голод любой стороны — берём имеющуюся
/// с полным весом: то же правило, что у пары `PlanLayer::Dissolve`.
fn blend_frames(
    a: Option<Arc<VideoFrame>>,
    b: Option<Arc<VideoFrame>>,
    k: u32,
) -> Option<Arc<VideoFrame>> {
    let (Some(fa), Some(fb)) = (a.as_ref(), b.as_ref()) else { return a.or(b) };
    if (fa.width, fa.height) != (fb.width, fb.height) {
        return a;
    }
    let pixels = fa.width as usize * fa.height as usize;
    let mut buf = Vec::new();
    let has_alpha = dissolve_into(&mut buf, fa, fb, k, pixels);
    Some(Arc::new(VideoFrame {
        width: fa.width,
        height: fa.height,
        rgba: buf.into(),
        has_alpha,
        pts: fa.pts,
    }))
}

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
