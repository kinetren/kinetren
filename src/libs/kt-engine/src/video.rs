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
    /// Дорожка ЗАГЛУШЕНА (mute либо не в соло-множестве, Р.62): слой в
    /// композит не идёт, а ЗАКАЗ ПРЕФЕТЧА остаётся — снятие mute иначе
    /// давало бы чёрный кадр до прогрева (инвариант 4 про заказ == чтение
    /// сохраняется: греем то же, что прочтём, когда дорожку вернут).
    silent: bool,
    id: kt_model::ClipId,
    start: Tick,
    end: Tick,
    source_in: Tick,
    lane: u32,
    media: MediaId,
    reversed: bool,
    fx: Vec<kt_model::FxInstance>,
    /// Компилированная автоматизация цепочки (Р.30; компиляция — Р.59,
    /// п.467). `None` — живых кривых нет, и тогда цепочка слоя
    /// ЗАИМСТВУЕТСЯ: клип без кривых не платит ни аллокации, ни одного
    /// сравнения строк. Хэш пост-FX по-прежнему считается от СЫРЫХ
    /// кривых, поэтому ключи кэша не сдвинулись.
    auto_plan: Option<kt_model::AutoPlan>,
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
    /// Контекст ведущего узла «Геометрия» (Р.62): вид источника (натив ×
    /// SAR) и разрешённое правило приведения. У ленты вид неизвестен
    /// намеренно: суб-композит собирается в размере холста, рамка = холст.
    lead: Option<kt_fx::conform::LeadCtx>,
    /// Кроп ведущего узла (доли источника) — входит в размер декода: рамка
    /// растягивается обратно на весь кадр, декодер отдаёт кадр целиком.
    crop: [f64; 4],
    /// Вложенная лента (Р.50, волна Б): вместо кадра из кэша слой получает
    /// СУБ-КОМПОЗИТ — и цепочка, навешенная на это событие, применяется к
    /// СОБРАННОЙ картинке, а не к каждому куску. Ради этого всё и затевалось.
    /// Ключ — ПАРА (лента, редакция ЭКЗЕМПЛЯРА): с Р.59 у двух
    /// экземпляров одной ленты могут быть разные переопределения, и это
    /// разное содержимое. Пустой набор даёт `content == content_hash`,
    /// поэтому экземпляры без переопределений по-прежнему делят ОДИН
    /// суб-рендерер (t442/t443 живы).
    nest: Option<NestKey>,
    /// Хэш редакции ленты: правка ВНУТРИ не трогает ни одного поля
    /// экземпляра, поэтому без него мемо отдавало бы стейл. С Р.59 —
    /// функция ПАРЫ (лента, набор переопределений).
    content: u64,
    /// Ступень суб-композита — МАКСИМУМ по всем экземплярам этой ленты в
    /// дереве (Р.50, Б5). Свойство ЛЕНТЫ, а не экземпляра: разойдись они —
    /// у внутреннего клипа два ключа кэша и два заказа.
    nest_scale: u32,
    /// СВОЙ fps ленты (Р.62): внутреннее время экземпляра квантуется к её
    /// сетке (кадр держится). `None` — непрерывно, как Р.56.
    nest_fps: Option<u32>,
    /// ПРОЦЕДУРНЫЙ источник (Р.56, п.448): у медиа нет своей сетки кадров,
    /// кадр — чистая функция тика (генераторы, титры, заглушка). Такому
    /// клипу пол по сетке не ставится: под растяжением он обязан двигаться
    /// НЕПРЕРЫВНО, иначе замедленный титр дёргается покадрово.
    procedural: bool,
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
    /// Инверсия перехода (Р.51, п.398). Исполняется ПЛАНОМ, не ядром:
    /// `apply_invert` меняет стороны местами и обращает прогресс.
    invert: bool,
}

impl TransAnim {
    fn of(t: &kt_model::TransitionEvent, w0: Tick) -> TransAnim {
        TransAnim { kind: t.kind.clone(), auto: t.auto.clone(), w0, invert: t.invert }
    }

    fn mix_at(&self, t: Tick) -> crate::TransMix {
        if self.auto.is_empty() {
            return crate::TransMix::of(&self.kind);
        }
        let mut k = self.kind.clone();
        for c in &self.auto {
            if !c.keys.is_empty() {
                k.set_param(&c.param, kt_model::curve_value(&c.keys, Tick(t.0 - self.w0.0)));
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

/// Хэш СОДЕРЖИМОГО вложенной ленты, рекурсивный (Р.50, волна Б).
///
/// Сегодняшнее обещание «правка модели пересоздаёт рендерер, стейл-кадры не
/// читаются» для вложения НЕПРАВДА: правка ВНУТРИ ленты не трогает ни одного
/// поля клипа-экземпляра, и его `fx_hash` остаётся прежним. Этот хэш
/// вплетается туда и закрывает дыру. Разделение получается чистым:
/// `source` различает ЛЕНТЫ, `fx_hash` — ЭКЗЕМПЛЯРЫ, `content_hash` —
/// РЕДАКЦИИ.
///
/// Считается Debug-представлением — тем же приёмом, что `fx_chain_hash`:
/// f64 печатается shortest-roundtrip, значит детерминированно, а новое поле
/// модели попадает в хэш САМО.
/// Адрес суб-рендерера: ЛЕНТА плюс РЕДАКЦИЯ экземпляра (Р.59, стадия 3).
pub type NestKey = (MediaId, u64);

/// Редакция ленты, какой её видит ЭТОТ экземпляр (Р.59, стадия 3).
///
/// Пустой набор переопределений обязан давать РОВНО `content_hash`:
/// иначе сдвинулись бы все ключи мемоизации, sidecar-превью и GPU-планы
/// у проектов, которые публикацией не пользуются вовсе.
pub fn instance_content(project: &Project, seq: MediaId, overrides: &[kt_model::Override]) -> u64 {
    let base = content_hash(project, seq);
    if overrides.is_empty() {
        return base;
    }
    // Тем же приёмом, что и сам `content_hash`: Debug-представление
    // детерминировано (f64 печатается shortest-roundtrip), и новое поле
    // переопределения попадает в хэш САМО.
    xxhash_rust::xxh3::xxh3_64(
        &[base.to_le_bytes().as_slice(), format!("{overrides:?}").as_bytes()].concat(),
    )
}

pub fn content_hash(project: &Project, seq: MediaId) -> u64 {
    use xxhash_rust::xxh3::Xxh3;
    let mut h = Xxh3::new();
    hash_reel(project, seq, 0, &mut h);
    h.digest()
}

fn hash_reel(project: &Project, seq: MediaId, depth: usize, h: &mut xxhash_rust::xxh3::Xxh3) {
    let Some(s) = project.sequence(seq) else { return };
    if depth > kt_model::MAX_NEST_DEPTH {
        return;
    }
    h.update(&[s.overlap_layers as u8]);
    // ДОРОЖКИ ленты (Р.62) — редакция целиком: порядок строк, выключатели,
    // уровень, режим наложения и цепочка дорожки меняют картинку и звук
    h.update(format!("{:?}", s.tracks).as_bytes());
    h.update(format!("{:?}", s.clips).as_bytes());
    h.update(format!("{:?}", s.transitions).as_bytes());
    h.update(format!("{:?}", s.morphers).as_bytes());
    h.update(format!("{:?}", s.cameras).as_bytes());
    // объявленный набор ручек — тоже СОДЕРЖИМОЕ ленты: сужение предела
    // меняет то, что рисуется у экземпляра с переопределением
    h.update(format!("{:?}", s.published).as_bytes());
    // свои растр и fps (Р.62) — редакция: другой холст или другая сетка —
    // другой кадр, ключи мемо и превью ленты обязаны это видеть
    h.update(format!("{:?}{:?}", s.raster, s.fps).as_bytes());
    for c in &s.clips {
        if project.sequence(c.media).is_some() {
            hash_reel(project, c.media, depth + 1, h);
        }
    }
}

/// Хэш цепочки для пост-FX кэша. Вход — Debug-представление параметров
/// (детерминировано, f64 печатается shortest-roundtrip) + геометрия:
/// start/span/source_in/reversed задают отображение t→(src_pts, u),
/// frame_step — сетку (смена fps проекта обязана инвалидировать).
#[allow(clippy::too_many_arguments)]
fn fx_chain_hash(
    fx: &[kt_model::FxInstance],
    auto: &[Vec<kt_model::ParamCurve>],
    start: Tick,
    end: Tick,
    source_in: Tick,
    reversed: bool,
    frame_step: i64,
    content: u64,
) -> u64 {
    // кривые автоматизации — часть ключа (Р.30): правка кейфрейма обязана
    // инвалидировать пост-FX кадры, пережившие пересоздание рендерера.
    // `content` (Р.50) — редакция ВЛОЖЕННОЙ ленты: правка внутри не
    // трогает ни одного поля экземпляра, и без неё пост-FX кадр экземпляра
    // остался бы стейлом. У обычного клипа это ноль, и ключ прежний.
    let repr = format!(
        "{fx:?}|{auto:?}|{}|{}|{}|{reversed}|{frame_step}|{content}",
        start.0,
        end.0 - start.0,
        source_in.0
    );
    xxhash_rust::xxh3::xxh3_64(repr.as_bytes())
}

/// pts источника для момента таймлайна `t` внутри клипа: прямой или
/// зеркальный (п.16 реверс). Чистая функция — тест 24.
///
/// **Пол по сетке ставит ЛИСТ, и только ФАЙЛОВЫЙ** (Р.56, п.448). Файл
/// хранит кадры на своей сетке: просить между ними бессмысленно, декодер
/// всё равно отдаст ближайший, а два ключа на один кадр — двойной расход
/// кэша. У процедурного источника (генератор, титр, заглушка) кадра «между»
/// не существует — он считается на любой тик, и пол там только дробит
/// движение на ступеньки. У ЛЕНТЫ пола нет вовсе: её внутреннее время —
/// время монтажа, а не индекс кадра (`clip_inner_tick`).
fn clip_source_pts(c: &VideoClip, t: Tick, grid: &FpsGrid) -> Tick {
    let m = clip_material_pos(c, t, grid);
    if c.procedural { m } else { grid.align_floor(m) }
}

/// НЕПРЕРЫВНОЕ внутреннее время вложения (Р.56, п.448): момент ленты, в
/// который играет экземпляр. Пола здесь нет по построению — у ленты нет
/// декодированных кадров, округлять нечего, а округление ЛОМАЕТ: при
/// rate 0.5 два соседних выходных кадра получали один и тот же `inner_t`,
/// мемо суб-композита отдавало ту же запись, и всё процедурное внутри
/// (титры, анимация, автоматизация, камеры) шло ступеньками по два кадра.
fn clip_inner_tick(c: &VideoClip, t: Tick, grid: &FpsGrid) -> Tick {
    let m = clip_material_pos(c, t, grid);
    match c.nest_fps {
        // СВОЙ fps ленты (Р.62): её время квантуется к её сетке — кадр
        // держится до следующего, без смешения; это намеренные ступеньки
        Some(f) if f > 0 => {
            let step = kt_time::TICK_HZ / f as i64;
            Tick(m.0.div_euclid(step) * step)
        }
        _ => m,
    }
}

/// Смешивает ли этот экземпляр кадры (Р.56, п.448). Наследуется ВНИЗ: на
/// самой ленте смешивать нечего (суб-композит — функция непрерывного
/// времени, он и так точен), а вот её ЛИСТЬЯ обязаны смешать свои пары,
/// иначе футаж внутри замедленной композиции остаётся ступенчатым.
fn nest_blend(c: &VideoClip, inherited: bool) -> bool {
    inherited || c.warp.as_ref().is_some_and(|w| w.blend())
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
        // ПРОЛЁТ позиции (Р.57), а не «сколько съедено»: со знаковой
        // скоростью кривая может уйти назад и вернуться, съев вдвое
        // больше, чем прошла. На положительных кривых пролёт равен
        // (0, used), и выражение сворачивается в прежнее.
        let (smin, smax) = w.span();
        // два клампа, и оба нужны: нижний край ПРОЛЁТА (дальше зеркало не
        // ходит) и НАЧАЛО ФАЙЛА — со знаковой скоростью пролёт уезжает за
        // точку входа и может уйти левее нуля (Р.57). Кадра там нет ни у
        // одного источника, держим крайний.
        Tick(
            (c.source_in.0 + smin + smax - grid.frame_step() - consumed)
                .max(c.source_in.0 + smin)
                .max(0),
        )
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
    /// Рендереры вложенных лент (Р.50). Дерево строится ОДИН раз вместе с
    /// корнем: правка модели пересоздаёт рендерер целиком, и стейл-узлов
    /// не бывает по построению.
    nests: std::collections::HashMap<NestKey, VideoRenderer>,
}

/// Кадр(ы) источника для момента `t`: основной и, в режиме «смешивание»,
/// сосед с весом (Р.37 С5). ЕДИНСТВЕННОЕ место, где решается пара —
/// `layers_at` и `layer_src` обязаны звать именно её, иначе префетч греет
/// не то, что читает композит, и это вечный промах кэша.
///
/// `k == 0` (позиция ровно на сетке: целые скорости, rate = 1, кламп у
/// края) пары НЕ даёт — путь остаётся байт-в-байт прежним.
///
/// `blend_footage` — смешивание, УНАСЛЕДОВАННОЕ от вложения сверху (Р.56,
/// п.448): режим стоит на экземпляре ленты, а смешивают её листья.
/// У самой ленты пары не бывает никогда: два суб-композита на кадр вдвое
/// дороже на обоих бэкендах и ничего не добавляют — её время и так
/// непрерывно.
fn clip_frame_keys(
    c: &VideoClip,
    t: Tick,
    grid: &FpsGrid,
    blend_footage: bool,
) -> (Tick, Option<(Tick, u32)>) {
    if c.nest.is_some() {
        return (clip_inner_tick(c, t, grid), None);
    }
    let a = clip_source_pts(c, t, grid);
    if !(blend_footage || c.warp.as_ref().is_some_and(|w| w.blend())) {
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
    // база — размер приведения (Р.62): рамка правила на холсте Quality,
    // растянутая на весь кадр; у ленты — сам холст
    let base = clip_base_quality(clip, quality);
    let (qw, qh) = (base.width.max(2), base.height.max(2));
    let cap = (kt_fx::plan::MAX_INTER_DIM / qw.max(qh)).max(1);
    // ВЛОЖЕНИЕ (Р.50, Б5): «натива» у ленты нет — суб-композит можно
    // собрать в любом размере, запас не ограничен ничем, кроме потолка
    // промежуточных буферов. Ступень берётся ОДНА на ленту (максимум по
    // всем её экземплярам): иначе у внутреннего клипа было бы два ключа
    // кэша, две полоски и два заказа — вечный промах (t312/t317).
    if clip.nest.is_some() {
        return clip.nest_scale.min(cap).max(1);
    }
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
    want.min(spare).min(cap)
}

/// ВХОД ЦЕПОЧКИ клипа (Р.58, п.459) — ступень, которую цепочка ЭТОГО
/// экземпляра способна потребить.
///
/// У листа равен ступени декода: она и выведена из его собственной
/// цепочки. У ЛЕНТЫ ступень — свойство ленты (максимум по экземплярам,
/// чтобы у внутреннего листа был один ключ кэша, t450), и отдавать её
/// входом цепочки КАЖДОМУ экземпляру нельзя: пустая цепочка, одна сила
/// слоя, ведущий стоп (морфер-слой) и цепочка без ресемпла (Blur/Glow)
/// ступени не потребляют — S×-кадр никто не вернул бы в Quality. До Р.58
/// это роняло debug-сборку и МОЛЧА выбрасывало вложение из кадра в
/// релизной.
fn clip_chain_scale(clip: &VideoClip, quality: Quality) -> u32 {
    let stage = clip_decode_scale(clip, quality);
    if clip.nest.is_none() {
        return stage;
    }
    // тот же квант, которым считал `nest_scales` — иначе «равно» никогда
    // не совпало бы и лишний ресемпл появлялся бы на ровном месте
    stage.min(kt_fx::geom::ss_taps(clip.head_zoom).max(1))
}

/// Ступень суб-композита каждой ленты: МАКСИМУМ запроса по всем её
/// экземплярам в ДЕРЕВЕ (Р.50, Б5).
///
/// Ступень обязана быть свойством ЛЕНТЫ, а не экземпляра: содержимое у них
/// общее, и разные ступени дали бы внутреннему клипу два ключа кэша, две
/// полоски прогретости и два заказа префетча — вечный промах (t312/t317).
/// Чистая функция от проекта: от устройства и бюджетов не зависит.
fn nest_scales(project: &Project) -> std::collections::HashMap<MediaId, u32> {
    let mut out: std::collections::HashMap<MediaId, u32> = std::collections::HashMap::new();
    let grid = FpsGrid::new(project.settings.fps);
    let size = (project.settings.width, project.settings.height);
    // ОБХОД ДЕРЕВА ЭКЗЕМПЛЯРОВ, а не плоский перебор лент (Р.59, стадия
    // 3): содержимое ленты теперь зависит от набора переопределений того
    // экземпляра, через который в неё вошли, — а значит и зум цепочки
    // внутри неё. Ступень при этом остаётся ОДНОЙ на ленту (t450):
    // разные ступени дали бы внутреннему клипу два ключа кэша и два
    // заказа. Максимум просто стал честным.
    let mut seen: std::collections::HashSet<(MediaId, u64)> = std::collections::HashSet::new();
    let mut stack: Vec<(Option<MediaId>, Vec<kt_model::Override>, usize)> =
        vec![(None, Vec::new(), 0)];
    while let Some((reel, ov, depth)) = stack.pop() {
        let resolved = project.resolve_reel(reel, &ov);
        for c in &resolved.clips {
            if c.kind != ClipKind::Video || project.sequence(c.media).is_none() {
                continue;
            }
            let fx = c.active_fx();
            let auto = c.active_fx_auto();
            let (fx, auto) = materialize_lead(c, fx, auto);
            let plan = kt_model::AutoPlan::compile(&fx, &auto);
            let lead = Some(kt_fx::conform::LeadCtx::new(
                project.sequence(c.media).and_then(|s| s.raster).map(|(w, h)| (w as f64, h as f64)),
                c.geometry.rule.resolve(project.settings.conform),
            ));
            let zoom = clip_head_zoom(
                &fx,
                plan.as_ref(),
                c.start,
                c.start + c.duration,
                c.source_in,
                c.reversed,
                &grid,
                &resolved.cameras,
                size,
                lead,
            );
            let want = kt_fx::geom::ss_taps(zoom).max(1);
            let e = out.entry(c.media).or_insert(1);
            *e = (*e).max(want);
            if depth < kt_model::MAX_NEST_DEPTH
                && seen.insert((c.media, instance_content(project, c.media, &c.overrides)))
            {
                stack.push((Some(c.media), c.overrides.clone(), depth + 1));
            }
        }
    }
    out
}

/// `a \ b` на отсортированных наборах интервалов.
fn subtract(
    a: Vec<std::ops::Range<i64>>,
    mut b: Vec<std::ops::Range<i64>>,
) -> Vec<std::ops::Range<i64>> {
    if b.is_empty() {
        return a;
    }
    b.sort_by_key(|r| r.start);
    let mut out = Vec::new();
    for r in a {
        let mut cur = r.start;
        for h in &b {
            if h.end <= cur || h.start >= r.end {
                continue;
            }
            if h.start > cur {
                out.push(cur..h.start);
            }
            cur = cur.max(h.end);
            if cur >= r.end {
                break;
            }
        }
        if cur < r.end {
            out.push(cur..r.end);
        }
    }
    out
}

fn shift_runs(v: Vec<std::ops::Range<i64>>, d: i64) -> Vec<std::ops::Range<i64>> {
    v.into_iter().map(|r| (r.start + d)..(r.end + d)).collect()
}

fn scaled(q: Quality, s: u32) -> Quality {
    if s <= 1 { q } else { Quality { width: q.width * s, height: q.height * s } }
}

/// Размер приведения клипа (Р.62): в какой размер декодер тянет ПОЛНЫЙ
/// кадр при ступени 1 — рамка правила на холсте `quality`, растянутая
/// обратно на весь кадр (кроп вырезает узел). У ленты — холст.
fn clip_base_quality(clip: &VideoClip, quality: Quality) -> Quality {
    let lead =
        clip.lead.unwrap_or(kt_fx::conform::LeadCtx::new(None, kt_model::ConformRule::Stretch));
    // лента без своего растра — холст родителя (закон Э); со своим (Р.62) —
    // рамка правила, как у файла: суб-композит собирается в ней
    if clip.nest.is_some() && lead.view.is_none() {
        return quality;
    }
    let g = kt_fx::conform::conform_geometry(
        lead.view,
        (quality.width, quality.height),
        lead.rule,
        clip.crop,
    );
    Quality { width: g.decode.0, height: g.decode.1 }
}

/// Качество декода клипа. ЕДИНСТВЕННОЕ место решения: заказ префетча,
/// чтение композита и полоски прогретости обязаны спрашивать его — иначе
/// греется не то, что читается (t247d по оси размера, t312).
fn clip_decode_quality(clip: &VideoClip, quality: Quality) -> Quality {
    scaled(clip_base_quality(clip, quality), clip_decode_scale(clip, quality))
}

/// Ведущий узел «Геометрия» — в цепочку движка (Р.62). Позиция —
/// `geometry_pos` в счёте АКТИВНЫХ слотов (bypass-слоты отфильтрованы);
/// кривые (`auto`) получают пустой слот на той же позиции, чтобы индексы
/// остались параллельны; `LayerMix.from`, указывающий на слот после
/// точки вставки, сдвигается на один. У аудио-события узла нет.
fn materialize_lead(
    c: &kt_model::ClipEvent,
    mut fx: Vec<kt_model::FxInstance>,
    mut auto: Vec<Vec<kt_model::ParamCurve>>,
) -> (Vec<kt_model::FxInstance>, Vec<Vec<kt_model::ParamCurve>>) {
    if c.kind != ClipKind::Video {
        return (fx, auto);
    }
    let want = c.geometry_pos.min(c.fx.len());
    let pos = (0..want).filter(|&i| !c.fx_bypass.get(i).copied().unwrap_or(false)).count();
    let pos = pos.min(fx.len());
    for f in fx.iter_mut() {
        if let kt_model::FxInstance::LayerMix { from, .. } = f
            && (*from as usize) >= pos
        {
            *from += 1;
        }
    }
    fx.insert(pos, c.geometry.node());
    if auto.len() >= pos {
        auto.insert(pos, c.geometry_auto.clone());
    } else if !c.geometry_auto.is_empty() {
        auto.resize(pos, Vec::new());
        auto.push(c.geometry_auto.clone());
    }
    (fx, auto)
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
    plan: Option<&kt_model::AutoPlan>,
    start: Tick,
    end: Tick,
    source_in: Tick,
    reversed: bool,
    grid: &FpsGrid,
    cameras: &[kt_model::CameraEvent],
    project_size: (u32, u32),
    lead: Option<kt_fx::conform::LeadCtx>,
) -> f64 {
    let (pw, ph) = (project_size.0.max(2), project_size.1.max(2));
    let live = plan.is_some();
    // Тождество проверяется по СТАТИЧЕСКИМ значениям: узел «Геометрия» с
    // zoom 1 и кривой 1 → 4 статически тождествен, а к концу клипа
    // просит ступень 4 (t311). С живым планом ранний выход невозможен —
    // до Р.62 его спасал Pan, который тождеством не считался никогда.
    if !live && !kt_fx::video::has_video_fx_in(fx, lead, pw, ph) {
        return 1.0;
    }
    let has_3d = fx.iter().any(|f| matches!(f, kt_model::FxInstance::Pan3d { .. }));
    let span = (end.0 - start.0).max(1);
    let mut probes: Vec<Tick> = Vec::new();
    if live || has_3d || !cameras.is_empty() {
        const GRID: i64 = 32;
        for i in 0..=GRID {
            probes.push(Tick(start.0 + span * i / GRID));
        }
        // изломы кривых — тоже пробы: между двумя ключами максимум зума
        // может стоять ровно на ключе
        for k_t in plan.into_iter().flat_map(|p| p.key_ticks()) {
            // ключ живёт в МАТЕРИАЛЬНОЙ оси — переводим в таймлайн
            let t = kt_model::clip_timeline_at(
                source_in,
                start,
                Tick(span),
                reversed,
                grid.frame_step(),
                k_t,
            );
            if start.0 <= t.0 && t.0 <= end.0 {
                probes.push(t);
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
        if let Some(plan) = plan {
            owned.clone_from_slice(fx);
            plan.apply(&mut owned, mat);
        }
        let cam3d = has_3d.then(|| {
            let ev = kt_model::active_camera_in(cameras, t);
            let cam = ev.map(|e| e.params_at(t)).unwrap_or_default();
            // логика считается на размере проекта — world_scale ровно 1
            Cam3d { cam, world_scale: 1.0 }
        });
        let u = (t.0 - start.0) as f64 / span as f64;
        best = best.max(kt_fx::plan::chain_head_zoom_in(&owned, pw, ph, u, cam3d, lead));
    }
    best
}

/// Что рендерер знает об ОТКРЫТЫХ источниках (Р.46 п.321, Р.56 п.448).
///
/// План знает о медиа ровно `MediaId` (закон П): ЧТО лежит по адресу —
/// свойство ИСТОЧНИКА, и рендерер узнаёт об этом только от того, кто
/// источник открыл. Отсюда и форма: не путь и не `MediaRef`, а два факта,
/// которые плану действительно нужны — запас деталей (натив) и наличие
/// собственной сетки кадров.
#[derive(Debug, Clone, Default)]
pub struct SourceFacts {
    /// Нативный размер видео-источника: запас для ступени декода.
    pub natives: std::collections::HashMap<MediaId, (u32, u32)>,
    /// Источники БЕЗ своей сетки кадров (Р.56): кадр — чистая функция
    /// тика. Им пол по сетке не ставится.
    pub procedural: std::collections::HashSet<MediaId>,
    /// Вид кадра источника (натив × пиксельное соотношение, Р.62) — вход
    /// правила приведения. Считается ТЕМ ЖЕ каналом, что нативы.
    pub views: std::collections::HashMap<MediaId, (f64, f64)>,
}

impl SourceFacts {
    /// Запомнить факты открытого источника. ЕДИНСТВЕННОЕ место, где
    /// решается «процедурный ли он»: разойдись плеер, экспорт и kt-cli в
    /// этом вопросе — заказ и чтение считали бы разные ключи (инвариант 4).
    pub fn observe(&mut self, media: MediaId, info: &kt_media::MediaInfo) {
        if info.has_video {
            self.natives.insert(media, (info.width, info.height));
            self.views.insert(media, info.view_size());
        } else {
            self.natives.remove(&media);
            self.views.remove(&media);
        }
        // «Своей сетки кадров нет» — это и есть определение процедурного
        // источника: у файла fps берётся из контейнера, у генератора,
        // титра и заглушки его не существует, и кадр считается на любой
        // запрошенный тик.
        if info.has_video && info.fps.is_none() {
            self.procedural.insert(media);
        } else {
            self.procedural.remove(&media);
        }
    }

    pub fn forget(&mut self, media: MediaId) {
        self.natives.remove(&media);
        self.procedural.remove(&media);
    }
}

impl VideoRenderer {
    pub fn new(project: &Project) -> VideoRenderer {
        VideoRenderer::new_with_facts(project, &SourceFacts::default())
    }

    /// Рендерер, знающий НАТИВНЫЕ размеры источников (Р.46, п.320): без
    /// них ступень декода всегда 1 — раздувать декод, не зная запаса
    /// файла, нельзя.
    pub fn new_with_natives(
        project: &Project,
        natives: &std::collections::HashMap<MediaId, (u32, u32)>,
    ) -> VideoRenderer {
        VideoRenderer::new_with_facts(
            project,
            &SourceFacts {
                natives: natives.clone(),
                procedural: Default::default(),
                views: Default::default(),
            },
        )
    }

    /// Рендерер, знающий ФАКТЫ об открытых источниках (Р.56): натив и
    /// процедурность. Плеер, экспорт и kt-cli зовут этот конструктор —
    /// иначе замедленный титр остаётся ступенчатым.
    pub fn new_with_facts(project: &Project, facts: &SourceFacts) -> VideoRenderer {
        VideoRenderer::build(project, facts)
    }

    fn build(project: &Project, facts: &SourceFacts) -> VideoRenderer {
        VideoRenderer::build_reel(project, facts, &nest_scales(project), None, &[], 0)
    }

    fn build_reel(
        project: &Project,
        facts: &SourceFacts,
        nest_scales: &std::collections::HashMap<MediaId, u32>,
        seq: Option<MediaId>,
        overrides: &[kt_model::Override],
        depth: usize,
    ) -> VideoRenderer {
        // морфер-слои применены здесь, движок их не видит (H1); вложения
        // НЕ раскрываются — у каждого свой узел плана (Р.50, волна Б).
        // Переопределения ЭКЗЕМПЛЯРА тоже разрешает модель (Р.59, стадия
        // 3): движок про публикацию не знает вовсе, ему достаются готовые
        // клипы. Пустой набор идёт байт-в-байт прежним путём.
        let resolved = project.resolve_reel(seq, overrides);
        let effective = &resolved.clips;
        let grid = FpsGrid::new(project.settings.fps);
        // Камеры — СВОИ у каждой ленты, и `head_zoom` обязан считаться по
        // ним же: `nest_scales` выбирает камеры именно так, а ступень
        // ленты и вход цепочки экземпляра сравниваются между собой
        // (`clip_chain_scale`). Разные наборы камер разводили бы их сами
        // по себе на Pan3d внутри ленты со своими камерами.
        let cams: &[kt_model::CameraEvent] = &resolved.cameras;
        // СОЛЬ адреса источника (Р.59, стадия 3) — одна на всю ленту
        // экземпляра. Считается ТЕМ ЖЕ кодом, что и в `gen_sources`:
        // заказ и чтение обязаны спрашивать один адрес (инвариант 4).
        let salt = gen_salt(
            match seq.and_then(|id| project.sequence(id)) {
                Some(s) => &s.published,
                None => &project.published,
            },
            overrides,
        );
        // ГАШЕНИЕ ВИДЕОДОРОЖКИ (Р.62): mute либо соло-множество СВОЕГО
        // вида. Уровень и режим наложения дорожки — стадия плана (она
        // приходит отдельно), а гашение слоя стадии не требует.
        let tracks: &[kt_model::Track] = match seq.and_then(|id| project.sequence(id)) {
            Some(s) => &s.tracks,
            None => &project.tracks,
        };
        let solo_on = tracks.iter().any(|t| t.kind == kt_model::LaneKind::Video && t.solo);
        let silent_lane = |lane: u32| -> bool {
            kt_model::track_index(tracks, kt_model::LaneKind::Video, lane)
                .map(|i| &tracks[i])
                .is_some_and(|t| t.mute || (solo_on && !t.solo))
        };
        let mut clips: Vec<VideoClip> = effective
            .iter()
            .filter(|c| c.kind == ClipKind::Video)
            .map(|c| {
                let fx = c.active_fx(); // bypass-слоты отфильтрованы (п.2)
                let auto = c.active_fx_auto();
                // Ведущий узел «Геометрия» (Р.62) материализуется В ЦЕПОЧКУ
                // на позиции `geometry_pos` (в счёте активных слотов):
                // индексы слотов модели не двигаются, а цепочка движка —
                // одна последовательность, как у Vegas.
                let (fx, auto) = materialize_lead(c, fx, auto);
                let warp = kt_model::clip_warp_of(&fx, &auto, c.source_in, c.duration, c.reversed);
                let nest = project
                    .sequence(c.media)
                    .map(|_| (c.media, instance_content(project, c.media, &c.overrides)));
                let content = nest.map(|(_, h)| h).unwrap_or(0);
                let fx_hash = fx_chain_hash(
                    &fx,
                    &auto,
                    c.start,
                    c.start + c.duration,
                    c.source_in,
                    c.reversed,
                    grid.frame_step(),
                    content,
                );
                // ступень декода — свойство клипа (Р.46): максимум зума по
                // всей длительности, считается до перемещения цепочки
                // План компилируется ОДИН раз на сборку рендерера —
                // дальше ни одного поиска параметра по имени (Р.59).
                // Хэш выше посчитан от СЫРЫХ кривых: ключи кэша не
                // двигаются от смены способа адресации.
                let auto_plan = kt_model::AutoPlan::compile(&fx, &auto);
                let addr = clip_source_addr_in(c, salt);
                // контекст приведения: вид источника (у ленты — неизвестен,
                // рамка = холст) и правило события, разрешённое проектом
                // ПРОЦЕДУРНЫЙ источник (генератор, титр, заглушка) — функция
                // запрошенного качества, пропорций у него нет: вид None →
                // рамка = холст, узел тождествен (spec generators, Р.62)
                let procedural = facts.procedural.contains(&addr);
                // ЛЕНТА со своим растром (Р.62) — вид = её растр, экземпляр
                // приводится как файл; без своего растра вид неизвестен и
                // рамка равна холсту (закон Э)
                let reel_raster = nest
                    .and_then(|_| project.sequence(c.media))
                    .and_then(|s| s.raster)
                    .map(|(w, h)| (w as f64, h as f64));
                let view = if nest.is_some() {
                    reel_raster
                } else if procedural {
                    None
                } else {
                    facts.views.get(&addr).copied()
                };
                let lead = Some(kt_fx::conform::LeadCtx::new(
                    view,
                    c.geometry.rule.resolve(project.settings.conform),
                ));
                let nest_fps = nest.and_then(|_| project.sequence(c.media)).and_then(|s| s.fps);
                let crop =
                    [c.geometry.crop_l, c.geometry.crop_t, c.geometry.crop_r, c.geometry.crop_b];
                let head_zoom = clip_head_zoom(
                    &fx,
                    auto_plan.as_ref(),
                    c.start,
                    c.start + c.duration,
                    c.source_in,
                    c.reversed,
                    &grid,
                    cams,
                    (project.settings.width, project.settings.height),
                    lead,
                );
                VideoClip {
                    id: c.id,
                    start: c.start,
                    end: c.start + c.duration,
                    source_in: c.source_in,
                    lane: c.lane,
                    silent: silent_lane(c.lane),
                    media: clip_source_addr_in(c, salt),
                    reversed: c.reversed,
                    fx,
                    auto_plan,
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
                    native: facts.natives.get(&addr).copied(),
                    lead,
                    crop,
                    nest,
                    content,
                    nest_scale: nest.and_then(|(id, _)| nest_scales.get(&id).copied()).unwrap_or(1),
                    nest_fps,
                    procedural,
                }
            })
            .collect();
        // окна dissolve из переходов (Р.24): пары находят друг друга по
        // равенству окна на одной дорожке (compose блендит их одним слоем)
        // Геометрию стыков даёт ЛЕНТА КАК ЕСТЬ: переопределения меняют
        // значения, а не `start`/`duration`, поэтому `joint`/`overlap_pair`
        // считаются по ней. Вид и кривые перехода берутся из РАЗРЕШЁННОГО
        // набора — их-то публикация и правит.
        let reel: &dyn kt_model::Reel = match seq.and_then(|id| project.sequence(id)) {
            Some(s) => s,
            None => project,
        };
        for t in resolved.transitions.iter() {
            // НАЛОЖЕННЫЙ переход (Р.51, п.399): окно — сама зона
            // перекрытия, ручек нет (материал обеих сторон настоящий,
            // ext = 0). Дальше пара находит себя обычным find_pair по
            // равенству окна, и build_plan схлопывает её в Dissolve —
            // вместо жёсткого среза Vegas-порядка в зоне наложения.
            if let Some((pa, pb)) = t.overlap_pair(reel) {
                let (w0, w1) = (pb.start, pa.start + pa.duration);
                if w1.0 <= w0.0 {
                    continue;
                }
                if let Some(a) =
                    clips.iter_mut().find(|c| kt_model::same_clip(c.id, pa.id) && c.end == w1)
                {
                    a.fade_out = Some((w0, w1));
                    a.fade_out_solo = false;
                    a.fade_out_trans = Some(TransAnim::of(t, w0));
                }
                if let Some(b) =
                    clips.iter_mut().find(|c| kt_model::same_clip(c.id, pb.id) && c.start == w0)
                {
                    b.fade_in = Some((w0, w1));
                    b.fade_in_solo = false;
                    b.fade_in_trans = Some(TransAnim::of(t, w0));
                }
                continue;
            }
            let Some(joint) = t.joint(reel) else { continue };
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
        // Поддеревья: по одному рендереру на КАЖДУЮ использованную ленту.
        // Предел глубины держит модель (`validate`), здесь он страховка от
        // вражеского файла.
        let mut nests = std::collections::HashMap::new();
        if depth < kt_model::MAX_NEST_DEPTH {
            for c in effective.iter().filter(|c| c.kind == ClipKind::Video) {
                let Some(id) = project.sequence(c.media).map(|_| c.media) else { continue };
                let key = (id, instance_content(project, id, &c.overrides));
                nests.entry(key).or_insert_with(|| {
                    VideoRenderer::build_reel(
                        project,
                        facts,
                        nest_scales,
                        Some(id),
                        &c.overrides,
                        depth + 1,
                    )
                });
            }
        }
        VideoRenderer {
            clips,
            grid,
            fx_scratch: Default::default(),
            blend: Vec::new(),
            // камеры — СВОИ у каждой ленты: внутрь вложения можно класть
            // камеры, и правило активной камеры считается по её ленте
            cameras: resolved.cameras.to_vec(),
            project_size: (project.settings.width, project.settings.height),
            nests,
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
        let mut out = Vec::new();
        self.layers_at_into(t, quality, false, &mut out);
        out
    }

    /// Заказ УХОДИТ ВГЛУБЬ (Р.50, волна Б): у ленты источника нет, есть
    /// содержимое. Не спустись он — префетч грел бы адрес, которого не
    /// существует, а композит читал бы настоящие кадры: вечный промах
    /// кэша, тот самый, против которого стоит инвариант 4.
    fn layers_at_into(
        &self,
        t: Tick,
        quality: Quality,
        blend_footage: bool,
        out: &mut Vec<(MediaId, Tick, Quality)>,
    ) {
        for c in self.clips.iter().filter(|c| c.active_at(t)) {
            // в режиме «смешивание» слой читает ДВА кадра — префетч
            // обязан согреть оба, иначе второй не запросят никогда
            let (a, b) = clip_frame_keys(c, t, &self.grid, blend_footage);
            match c.nest.and_then(|k| self.nests.get(&k)) {
                // внутреннее время ленты НЕПРЕРЫВНО (Р.56, п.448): заказ
                // спускается по тому же тику, на котором план собирает
                // суб-композит, а КАЧЕСТВО увеличено ступенью ленты —
                // заказ и чтение обязаны спрашивать одно и то же
                // (инвариант 4). Режим «смешивание» едет ВНИЗ: смешивают
                // листья, и греть им надо обе стороны пары.
                Some(sub) => sub.layers_at_into(
                    a,
                    clip_decode_quality(c, quality),
                    nest_blend(c, blend_footage),
                    out,
                ),
                None => {
                    let dq = clip_decode_quality(c, quality);
                    out.push((c.media, a, dq));
                    if let Some((p, _)) = b {
                        out.push((c.media, p, dq));
                    }
                }
            }
        }
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
        self.clips
            .iter()
            .map(|c| clip_decode_quality(c, quality))
            .chain(self.clips.iter().filter_map(|c| {
                let sub = self.nests.get(&c.nest?)?;
                Some(sub.max_decode_quality(clip_decode_quality(c, quality)))
            }))
            .max()
            .unwrap_or(quality)
    }

    /// Прогретость ВЛОЖЕНИЙ в тиках их содержимого (Р.50, Б6).
    ///
    /// Coverage вложения = **ПЕРЕСЕЧЕНИЕ** прогретости его детей. Не
    /// объединение: объединение врало бы В ХОРОШУЮ сторону — показывало бы
    /// прогретым место, где половина слоёв ещё не пришла, а это ровно то,
    /// против чего стоит инвариант 4 («кэш — единственная правда»).
    ///
    /// Считается интервальной арифметикой: из пролёта ленты вычитается
    /// НЕПРОГРЕТАЯ часть каждого ребёнка. Дырка между детьми остаётся
    /// прогретой — там нечего декодировать, и это правда.
    ///
    /// **Ребёнок с растяжением времени участвует целиком**: карта варпа
    /// переводит интервалы источника в интервалы содержимого нелинейно, и
    /// приблизительный перевод тут хуже честного огрубления — такой
    /// ребёнок считается прогретым, только если прогрет весь.
    pub fn nest_coverage(
        &self,
        cache: &BurstCache,
        quality: Quality,
    ) -> Vec<(kt_model::ClipId, Vec<std::ops::Range<i64>>)> {
        let mut out = Vec::new();
        for c in &self.clips {
            let Some(sub) = c.nest.and_then(|k| self.nests.get(&k)) else { continue };
            // Окно — по ПРОЛЁТУ материала (Р.55 С4, ось уточнена в Р.57):
            // у растянутого экземпляра оно не равно его длине на
            // таймлайне, и полоска врала бы в ХОРОШУЮ сторону — то, чего
            // инвариант 4 не разрешает. У зеркального интервал тот же,
            // меняется направление, а не границы; у знаковой кривой
            // границы даёт пролёт, а не «сколько съедено».
            let win = match &c.warp {
                Some(w) => {
                    let (lo, hi) = w.span();
                    (c.source_in.0 + lo)..(c.source_in.0 + hi).max(c.source_in.0 + lo + 1)
                }
                None => c.source_in.0..(c.source_in.0 + c.end.0 - c.start.0),
            };
            let dq = clip_decode_quality(c, quality);
            out.push((c.id, sub.reel_coverage(cache, dq, win, nest_blend(c, false))));
        }
        out
    }

    /// Прогретость СОДЕРЖИМОГО ленты на окне `win` (тики ленты).
    fn reel_coverage(
        &self,
        cache: &BurstCache,
        quality: Quality,
        win: std::ops::Range<i64>,
        blend_footage: bool,
    ) -> Vec<std::ops::Range<i64>> {
        // при смешивании лист читает ЕЩЁ ОДИН кадр справа (Р.56, п.448):
        // не расширь окно — полоска звала бы прогретым место, у которого
        // не пришла вторая половина пары, а это вранье в ХОРОШУЮ сторону
        let pair_ext = |c: &VideoClip| -> i64 {
            if blend_footage || c.warp.as_ref().is_some_and(|w| w.blend()) {
                self.grid.frame_step().max(1)
            } else {
                0
            }
        };
        let mut warm = vec![win.clone()];
        for c in &self.clips {
            let body = c.start.0.max(win.start)..(c.end.0 + c.tail_ext.0).min(win.end);
            if body.start >= body.end {
                continue;
            }
            // вложенная лента внутри ленты — та же арифметика этажом ниже
            let cold = if let Some(sub) = c.nest.and_then(|k| self.nests.get(&k)) {
                // РАСТЯНУТОЕ или ЗЕРКАЛЬНОЕ вложение — целиком или никак,
                // ровно как растянутый файловый ребёнок (Р.55, С4):
                // `shift_runs` — ЛИНЕЙНАЯ карта, и под варпом она врёт, а
                // врать в ХОРОШУЮ сторону полоска не имеет права.
                let inner = match &c.warp {
                    Some(w) => {
                        let (lo, hi) = w.span();
                        (c.source_in.0 + lo)..(c.source_in.0 + hi).max(c.source_in.0 + lo + 1)
                    }
                    None => c.source_in.0..(c.source_in.0 + c.end.0 - c.start.0),
                };
                let sub_warm = sub.reel_coverage(
                    cache,
                    clip_decode_quality(c, quality),
                    inner.clone(),
                    nest_blend(c, blend_footage),
                );
                if c.warp.is_some() || c.reversed {
                    let full =
                        sub_warm.iter().any(|r| r.start <= inner.start && r.end >= inner.end);
                    if full { Vec::new() } else { vec![body.clone()] }
                } else {
                    subtract(vec![body.clone()], shift_runs(sub_warm, c.start.0 - c.source_in.0))
                }
            } else if let Some(w) = &c.warp {
                // растянутый ребёнок — целиком или никак (см. док).
                // Окно — ПРОЛЁТ материала (Р.57): длина события тут ни при
                // чём, на 0.5× она вдвое больше прочитанного, и полоска
                // врала бы в ХОЛОДНУЮ сторону.
                let (lo, hi) = w.span();
                let b0 = c.source_in.0 + lo;
                let b1 = (c.source_in.0 + hi).max(b0 + 1) + pair_ext(c);
                let runs = cache.coverage(
                    SourceId(c.media.0),
                    kt_cache::BurstKind::video(Some(clip_decode_quality(c, quality))),
                    b0..b1,
                );
                let full = runs.iter().any(|r| r.start <= b0 && r.end >= b1);
                if full { Vec::new() } else { vec![body.clone()] }
            } else {
                let ext = pair_ext(c);
                let b0 = c.source_in.0;
                let b1 = b0 + (c.end.0 - c.start.0).max(1) + ext;
                let runs = cache.coverage(
                    SourceId(c.media.0),
                    kt_cache::BurstKind::video(Some(clip_decode_quality(c, quality))),
                    b0..b1,
                );
                // источник → время ленты; у reversed материал идёт навстречу.
                // При смешивании прогон УКОРАЧИВАЕТСЯ справа на кадр: место
                // готово, только если пришла и вторая половина пары.
                let mapped: Vec<std::ops::Range<i64>> = runs
                    .into_iter()
                    .map(|r| r.start..(r.end - ext))
                    .filter(|r| r.start < r.end)
                    .map(|r| {
                        if c.reversed {
                            let len = c.end.0 - c.start.0;
                            let s = c.start.0 + (len - (r.end - c.source_in.0));
                            let e = c.start.0 + (len - (r.start - c.source_in.0));
                            s..e
                        } else {
                            (c.start.0 + (r.start - c.source_in.0))
                                ..(c.start.0 + (r.end - c.source_in.0))
                        }
                    })
                    .collect();
                subtract(vec![body.clone()], mapped)
            };
            warm = subtract(warm, cold);
        }
        warm
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
            nests: &self.nests,
            memoize: true,
            blend_footage: false,
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
            nests: &self.nests,
            memoize: false,
            blend_footage: false,
        };
        let plan = build_plan(&self.clips, &ctx, cache, t, quality, false);
        execute_plan_cpu(&plan, cache, &mut self.fx_scratch, &mut self.blend, out)
    }

    /// Сколько СУБ-РЕНДЕРЕРОВ держит эта лента (Р.59, стадия 3).
    ///
    /// Сторож выигрыша t442/t443: два экземпляра одной ленты БЕЗ
    /// переопределений обязаны делить ОДИН узел — в этом и был смысл
    /// дерева. Ключ стал парой (лента, редакция), и без счётчика это
    /// свойство проверить снаружи нечем.
    pub fn nest_count(&self) -> usize {
        self.nests.len()
    }

    /// Максимум одновременных видео-слоёв (с ручками переходов) — экспорт
    /// делит бюджет чанка на это число (Р.31 п.4).
    pub fn max_concurrent_layers(&self) -> usize {
        self.max_concurrent_layers_in(false)
    }

    /// То же внутри ленты, на которую унаследовано смешивание (Р.56,
    /// п.448): пара считается у ЛИСТА, и считать её надо там же, где она
    /// живёт, — иначе бюджет чанка разойдётся с тем, что план делает.
    fn max_concurrent_layers_in(&self, blend_footage: bool) -> usize {
        let mut evs: Vec<(i64, i32)> = Vec::with_capacity(self.clips.len() * 2);
        for c in &self.clips {
            // смешивающий слой держит ДВА кадра сразу (Р.37 С5). Не учесть
            // это — чанк экспорта вытеснит соседа до композиции, сработает
            // ветка голода, и БАЙТЫ ЭКСПОРТА станут зависеть от тайминга
            // кэша.
            // У САМОГО ВЛОЖЕНИЯ пары не бывает (Р.56, п.448): смешивают
            // его листья, и двойной буфер считается ВНУТРИ — посчитай мы
            // его и здесь, бюджет разошёлся бы с тем, что план делает.
            let blends =
                c.nest.is_none() && (blend_footage || c.warp.as_ref().is_some_and(|w| w.blend()));
            let mut w = if blends { 2 } else { 1 };
            // Вложение держит СВОЙ буфер суб-композита плюс всё, что
            // одновременно живо внутри него (Р.50). Недосчёт делает БАЙТЫ
            // ЭКСПОРТА зависящими от тайминга кэша — дословно регрессия
            // Р.37 С5.
            if let Some(sub) = c.nest.and_then(|k| self.nests.get(&k)) {
                w += sub.max_concurrent_layers_in(nest_blend(c, blend_footage)) as i32;
            }
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
        let ctx = PlanCtx {
            grid: &self.grid,
            cameras: &self.cameras,
            project_size: self.project_size,
            nests: &self.nests,
            memoize: true,
            blend_footage: false,
        };
        build_plan(&self.clips, &ctx, cache, t, quality, true)
    }
}

/// Контекст построения плана: сетка, камеры и мировой размер проекта.
struct PlanCtx<'a> {
    grid: &'a FpsGrid,
    cameras: &'a [kt_model::CameraEvent],
    project_size: (u32, u32),
    /// Рендереры вложенных лент — из них строятся суб-планы (Р.50).
    nests: &'a std::collections::HashMap<NestKey, VideoRenderer>,
    /// Мемоизировать ли пост-FX: наследуется ВНИЗ. Иначе суб-композиты
    /// вытеснят ещё не использованные пре-кадры соседних слоёв из
    /// 192-мегабайтного бюджета экспорта — дословно регрессия Р.31 п.4.
    memoize: bool,
    /// Смешивание кадров, унаследованное от вложения СВЕРХУ (Р.56, п.448).
    /// Режим стоит на экземпляре ленты, а исполняют его ЛИСТЬЯ: так на
    /// кадр остаётся ОДИН суб-композит (два стоили бы вдвое и на CPU, и на
    /// GPU), и процедурное внутри не оказывается на сетке вместо
    /// непрерывного времени. В корне всегда `false` — байты GOLDEN_VIDEO
    /// на этом стоят.
    blend_footage: bool,
}

/// Камера цепочки: прибитая по id, иначе активная по `t`, иначе дефолтная
/// «50 мм» (тождество для Pan3d с дефолтами). Нет Pan3d в цепочке — нет и
/// камеры: `None` означает «Pan3d пропускается», как было до Р.44.
///
/// Pub с Р.54 (п.429) и зовётся ИЗ `layer_src`: правило разрешения должно
/// остаться одно на программу. Второй потребитель — миниатюры пикера
/// (kt-app): полоска показывает цепочку клипа с кандидатом в хвосте, и
/// если бы она собирала камеру сама, вариант `Pan 3D` в пикере выглядел
/// бы иначе, чем после клика. `out_w`/`project_w` дают `world_scale` —
/// мир камеры заморожен форматом в мировых пикселях проекта (Р.29),
/// поэтому масштаб цели входит в него, а не в саму камеру.
pub fn resolve_cam3d(
    fx: &[kt_model::FxInstance],
    cameras: &[kt_model::CameraEvent],
    t: Tick,
    out_w: u32,
    project_w: u32,
) -> Option<Cam3d> {
    fx.iter()
        .find_map(|f| match f {
            kt_model::FxInstance::Pan3d { camera, .. } => Some(*camera),
            _ => None,
        })
        .map(|pinned| {
            let ev = pinned
                .and_then(|id| cameras.iter().find(|c| c.id == id))
                .or_else(|| kt_model::active_camera_in(cameras, t));
            let cam = ev.map(|e| e.params_at(t)).unwrap_or_default();
            Cam3d { cam, world_scale: out_w as f64 / project_w.max(1) as f64 }
        })
}

/// Источник слоя: pts, пре-FX кадр (читается ВСЕГДА — бамп recency, чтобы
/// полоски прогретости и prefetch не «остывали» под живым пост-FX кэшем),
/// цепочка и прогресс клипа.
fn layer_src<'a>(
    clip: &'a VideoClip,
    ctx: &PlanCtx<'a>,
    cache: &BurstCache,
    quality: Quality,
    t: Tick,
) -> LayerSrc<'a> {
    // У ЛЕНТЫ `src_pts` — непрерывный внутренний тик, у листа — pts кадра
    // (Р.56, п.448). Обе ветки ниже читают ровно то, что заказал
    // `layers_at_into` той же функцией.
    let (src_pts, pair) = clip_frame_keys(clip, t, ctx.grid, ctx.blend_footage);
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
    // Адрес ЛЕНТЫ обязан нести `NEST_TAG` (Р.55, С4). Без него он численно
    // совпадает с адресом ФАЙЛА с тем же id — ровно то, что `validate`
    // запрещает, чтобы файл и секвенция не делили кэш. До сих пор спасало
    // только различие `key_hash`; ветка смешивания кадров читала бы по
    // этому адресу кадр ЧУЖОГО ФАЙЛА.
    let source = match clip.nest {
        Some((seq, _)) => nest_source_id(seq),
        None => SourceId(clip.media.0),
    };
    // Р.46 (п.318): слой читает кадр СВОЕГО качества декода — того же,
    // которое заказал префетч через layers_at. У ЛЕНТЫ это ступень
    // СУБ-ПЛАНА (одна на ленту), а вход цепочки экземпляра считается
    // отдельно (Р.58, п.459) — см. `clip_chain_scale`.
    let stage = clip_decode_scale(clip, quality);
    let src_scale = clip_chain_scale(clip, quality);
    let dq = clip_decode_quality(clip, quality);
    // Кадр берётся ВМЕСТЕ с серийным номером содержимого (Р.53, п.409):
    // адрес `(source, cache_idx, размер)` у нового кадра генератора тот
    // же, что у старого, и без номера GPU отдал бы старую текстуру.
    let (frame, content) = if clip.nest.is_some() {
        (None, 0) // вход поддерева собирается ниже
    } else {
        match cache.get_video_stamped(source, Some(dq), cache_idx) {
            Some((f, serial)) => (Some(f), serial),
            None => (None, 0),
        }
    };
    // СМЕШИВАНИЕ (Р.37 С5) делается ЗДЕСЬ, в общем коде плана: оба
    // исполнителя получают уже смешанный пре-FX кадр, и держать одну и ту
    // же арифметику синхронной в CPU-ядре и в WGSL не приходится
    // (инвариант №7). Ядро — готовый `dissolve_into`: байт-заморожено,
    // с AVX2-близнецом.
    // СМЕШИВАНИЕ КАДРОВ НА САМОМ ВЛОЖЕНИИ не делается (Р.56, п.448): два
    // суб-композита на кадр вдвое дороже на обоих бэкендах и ничего не
    // добавляют — внутреннее время ленты и так непрерывно. Режим едет
    // ВНИЗ, к листьям, и пару отдаёт `clip_frame_keys` уже там.
    debug_assert!(
        pair.is_none() || clip.nest.is_none(),
        "a nest never gets a frame pair: the leaf does the blending"
    );
    let (frame, blend) = match pair {
        None => (frame, None),
        Some((pts, k)) => {
            let idx = video_cache_idx(pts);
            let (nb, nb_content) = match cache.get_video_stamped(source, Some(dq), idx) {
                Some((f, serial)) => (Some(f), serial),
                None => (None, 0),
            };
            (
                blend_frames(frame, nb, k),
                Some(crate::video_plan::LayerBlend { cache_idx: idx, k, content: nb_content }),
            )
        }
    };
    let span = (clip.end.0 - clip.start.0).max(1);
    let u = (t.0 - clip.start.0) as f64 / span as f64;
    // автоматизация (Р.30): кривые разрешаются ЗДЕСЬ, один раз на кадр —
    // CPU и GPU получают уже вычисленные параметры (клон цепочки только
    // при живых кривых)
    let fx: std::borrow::Cow<'_, [kt_model::FxInstance]> = match &clip.auto_plan {
        Some(plan) => {
            let mut owned = clip.fx.clone();
            plan.apply(&mut owned, mat);
            std::borrow::Cow::Owned(owned)
        }
        // ноль цены: у клипа без кривых цепочка ЗАИМСТВУЕТСЯ
        None => std::borrow::Cow::Borrowed(&clip.fx[..]),
    };
    let cam3d = resolve_cam3d(&clip.fx, ctx.cameras, t, quality.width, ctx.project_size.0);
    debug_assert!(
        src_scale == 1
            || kt_fx::video::has_video_fx_in(&fx, clip.lead, quality.width, quality.height),
        "a clip without video fx never gets a chain input above 1x"
    );
    // ВХОД слоя. У вложения это не кадр из кэша, а СУБ-КОМПОЗИТ: план
    // ленты, собранный на ВНУТРЕННЕМ времени экземпляра. Внутреннее время
    // НЕПРЕРЫВНО (`clip_inner_tick`, Р.56 п.448): реверс и ручки переходов
    // те же, что у файла, а пола нет — у ленты нет кадров, чтобы к ним
    // прижиматься, зато есть титры и автоматизация, которым пол виден как
    // дёрганье по два кадра.
    let input = match clip.nest.and_then(|k| ctx.nests.get(&k).map(|r| (k.0, r))) {
        Some((id, sub)) => {
            let blend = nest_blend(clip, ctx.blend_footage);
            let sub_ctx = PlanCtx {
                grid: &sub.grid,
                cameras: &sub.cameras,
                project_size: sub.project_size,
                nests: &sub.nests,
                memoize: ctx.memoize,
                blend_footage: blend,
            };
            // Суб-план собирается в S×Quality (Р.50, Б5): цепочка на
            // вложении зумит СОБРАННУЮ картинку, и без запаса она была бы
            // мягкой. Дети считают свою ступень уже против УВЕЛИЧЕННОГО
            // Quality — конъюгация правила п.317 этажом выше.
            let plan = build_plan(
                &sub.clips,
                &sub_ctx,
                cache,
                src_pts,
                clip_decode_quality(clip, quality),
                ctx.memoize,
            );
            crate::video_plan::LayerInput::Nest(Box::new(crate::video_plan::NestSrc {
                seq: id,
                content_hash: clip.content,
                inner_t: src_pts,
                blend,
                nest_scale: stage,
                plan,
            }))
        }
        None => crate::video_plan::LayerInput::Frame(frame),
    };
    LayerSrc {
        source,
        cache_idx,
        content,
        src_scale,
        input,
        fx,
        fx_hash: clip.fx_hash,
        u,
        cam3d,
        blend,
        lead: clip.lead,
    }
}

/// Построение плана кадра: какие слои, в каких ролях и с какими весами.
/// Пара клипов в окне перехода блендится линейным dissolve в ОДИН слой
/// (п.137); фейд у края/Gap (Р.25) — соло-слой, уходящий в прозрачность.
fn build_plan<'a>(
    clips: &'a [VideoClip],
    ctx: &PlanCtx<'a>,
    cache: &BurstCache,
    t: Tick,
    quality: Quality,
    memoize_postfx: bool,
) -> FramePlan<'a> {
    let mut layers = Vec::new();
    for (i, clip) in clips.iter().enumerate() {
        // заглушённая дорожка в композит не идёт; её ЗАКАЗ при этом
        // остаётся (`layers_at_into`/`wanted_frames` флага не смотрят)
        if !clip.active_at(t) || clip.silent {
            continue;
        }
        // фейд у края/Gap: пары нет по модели, слой всегда идёт через
        // масштабирование альфы (scale_alpha)
        if let Some((w, incoming, mix, inv)) = solo_fade(clip, t) {
            // кроссфейд — прежний путь (вес прямо в альфе, байты те же);
            // любой другой вид форму даёт ядром, а уровень остаётся
            // уровнем события
            let (alpha_k, fade) = if mix.is_crossfade() {
                (w * clip.opacity_k / 4096, None)
            } else {
                // инверсия соло-фейда: флип роли ДО вычисления k — его
                // выражение само даёт комплемент (п.398)
                let incoming = incoming != inv;
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
            let mut k = (((t.0 - win.0.0) * 4096) / (win.1.0 - win.0.0)).clamp(0, 4096) as u32;
            // вид разрешается по позиции ВНУТРИ окна: у его параметров
            // могут быть кривые (Р.34 К4)
            let ta = clip.fade_in_trans.as_ref();
            let mix = ta.map(|a| a.mix_at(t)).unwrap_or_default();
            // уровни соседей смешиваются тем же весом, что и картинка;
            // выражение симметрично к (swap, 1−k) — инверсия его не трогает
            let op = (outgoing.opacity_k * (4096 - k) + clip.opacity_k * k) / 4096;
            let mut a = layer_src(outgoing, ctx, cache, quality, t);
            let mut b = layer_src(clip, ctx, cache, quality, t);
            apply_invert(&mut a, &mut b, &mut k, &mix, ta.is_some_and(|a| a.invert));
            layers.push(PlanLayer::Dissolve { a, b, k, alpha_k: op, mix });
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
                match layer_frame_cpu(
                    src,
                    cache,
                    q_key,
                    scratch,
                    blend,
                    plan.t,
                    plan.memoize_postfx,
                    &mut complete,
                ) {
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
                    Some(f) => {
                        // Размер суб-композита ленты — НАША арифметика
                        // (`NestSrc::fit_to`): разойдись он с планом,
                        // слой молча исчезал бы из кадра — до Р.58 так и
                        // было (п.459). У файла чужой размер — сообщение
                        // декодера, и ответ на него один: кадр не полон.
                        debug_assert!(
                            src.input.nest().is_none(),
                            "nest sub-composite {}x{} does not fit the plan {w}x{h}",
                            f.width,
                            f.height
                        );
                        complete = false;
                    }
                    None => complete = false,
                }
            }
            PlanLayer::Dissolve { a, b, k, alpha_k, mix } => {
                let fa = layer_frame_cpu(
                    a,
                    cache,
                    q_key,
                    scratch,
                    blend,
                    plan.t,
                    plan.memoize_postfx,
                    &mut complete,
                );
                let fb = layer_frame_cpu(
                    b,
                    cache,
                    q_key,
                    scratch,
                    blend,
                    plan.t,
                    plan.memoize_postfx,
                    &mut complete,
                );
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
                    (fa, fb) => {
                        for (side, f) in [(a, &fa), (b, &fb)] {
                            if let Some(f) = f {
                                debug_assert!(
                                    side.input.nest().is_none() || fits(f),
                                    "nest sub-composite {}x{} does not fit the plan {w}x{h}",
                                    f.width,
                                    f.height
                                );
                            }
                        }
                        complete = false;
                    }
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

/// Суб-композит вложенной ленты (Р.50, волна Б).
///
/// **Ловушка, ради которой эта функция существует отдельно.**
/// `execute_plan_cpu` начинает с прозрачно-чёрного и накладывает слои
/// `alpha_over`; при `da == 0` получается `dst.rgb = s·sa/255`, `dst.a = sa`
/// — то есть результат композита хранит ПРЕМУЛЬТИПЛИЦИРОВАННЫЙ RGB при
/// ПРЯМОЙ альфе, а `alpha_over` ждёт на входе прямую. Подать такой буфер
/// обратно в композит — картинка вдвое темнее РОВНО в полупрозрачных
/// местах, что легко списать на «так и задумано».
///
/// Лечение — ровно ОДНА конверсия здесь, на границе вложения. Сам
/// `alpha_over` не трогаем: на его байтах стоят GOLDEN_VIDEO и AVX2-близнец
/// (t198).
fn nest_frame_cpu(
    n: &crate::video_plan::NestSrc<'_>,
    cache: &BurstCache,
    scratch: &mut kt_fx::video::FxScratch,
    blend: &mut Vec<u8>,
    memoize: bool,
    complete: &mut bool,
) -> Option<Arc<VideoFrame>> {
    let (w, h) = (n.plan.out_w, n.plan.out_h);
    let q_key = (w, h);
    let source = nest_source_id(n.seq);
    let key = nest_memo_key(n.content_hash, n.blend);
    if memoize && let Some(f) = cache.get_postfx_video(source, q_key, key, n.inner_t.0) {
        return Some(f);
    }
    let mut buf = scratch.take(w as usize * h as usize * 4);
    let whole = execute_plan_cpu(&n.plan, cache, scratch, blend, &mut buf);
    if !whole {
        *complete = false;
    }
    unpremultiply(&mut buf);
    let rgba: Box<[u8]> = buf[..].into();
    scratch.put(buf);
    let frame = Arc::new(VideoFrame { pts: n.inner_t, width: w, height: h, rgba, has_alpha: true });
    // МЕМОИЗИРУЕТСЯ ТОЛЬКО ПОЛНЫЙ суб-композит (Р.59, стадия 2).
    //
    // Голодный кадр — это НЕ функция редакции ленты, а функция того, что
    // успел положить в кэш воркер. Записав его под ключом редакции, мы
    // получали чёрную ленту НАВСЕГДА: ключ от прогрева не двигается,
    // перезапросить нечего, и снаружи композит объявлен полным (голод
    // виден только в тот единственный проход, где он случился). Живой
    // плеер попадал в это на каждом холодном старте.
    if memoize && whole {
        cache.insert_postfx_video(source, q_key, key, n.inner_t.0, frame.clone());
    }
    Some(frame)
}

/// Вписать суб-композит ленты во вход цепочки (Р.58, п.459).
///
/// Билинейка ОДНА на программу — `kt_fx::video::fit_into` (у неё же
/// AVX2-близнец и зеркало в WGSL), своей арифметики здесь не заводим:
/// разойдись она с GPU — превью и экспорт показали бы разное.
fn fit_frame_cpu(
    f: &Arc<VideoFrame>,
    ow: u32,
    oh: u32,
    scratch: &mut kt_fx::video::FxScratch,
) -> Arc<VideoFrame> {
    let mut src = scratch.take(f.rgba.len());
    src.copy_from_slice(&f.rgba);
    let src = kt_fx::video::Frame { w: f.width, h: f.height, rgba: src, has_alpha: f.has_alpha };
    let mut out = scratch.take(ow as usize * oh as usize * 4);
    kt_fx::video::fit_into(&src, ow, oh, &mut out);
    scratch.put(src.rgba);
    let rgba: Box<[u8]> = out[..].into();
    scratch.put(out);
    Arc::new(VideoFrame { pts: f.pts, width: ow, height: oh, rgba, has_alpha: f.has_alpha })
}

/// Ключ мемоизации суб-композита (Р.56, п.448): редакция ленты + режим
/// кадров. Один и тот же `inner_t` при «смешивании» и при «ближайшем»
/// даёт РАЗНЫЕ байты — листья внутри читают либо пару, либо один кадр, —
/// и без флага кэш отдал бы чужую картинку.
///
/// При `blend == false` ключ РАВЕН `content_hash`: прежние записи и
/// прежние тесты (t441-t443) не двигаются.
pub fn nest_memo_key(content_hash: u64, blend: bool) -> u64 {
    if !blend {
        return content_hash;
    }
    xxhash_rust::xxh3::xxh3_64(&[content_hash.to_le_bytes().as_slice(), b"blend"].concat())
}

/// Адрес ИСТОЧНИКА события (Р.59, стадия 2) — ЕДИНСТВЕННОЕ правило.
///
/// У генератора значения живут на событии, значит и источник у каждого
/// события свой: адрес берётся от ЕГО id. У всех остальных — адрес медиа,
/// как было. Форма заказа `(адрес, тик, качество)` не изменилась, поэтому
/// «заказ == чтение», полоски прогретости, префетч, экспорт-чанки и
/// покрытие лент работают без правок (инвариант 4).
///
/// Спрашивать это обязаны ВСЕ: план, префетч, экспорт, kt-cli и полоски.
/// Разойдись двое — вечный промах кэша по адресу, ровно тот класс
/// дефектов, против которого стоит инвариант 4.
pub fn clip_source_addr(c: &kt_model::ClipEvent) -> MediaId {
    match c.r#gen {
        Some(_) => gen_source_addr(c.id),
        None => c.media,
    }
}

/// Адрес источника генератора по событию.
pub fn gen_source_addr(id: kt_model::ClipId) -> MediaId {
    MediaId(kt_model::GEN_TAG | id.0)
}

/// СОЛЬ адреса источника внутри ленты (Р.59, стадия 3).
///
/// Ноль — ровно тогда, когда ни одна ручка набора не способна дотянуться
/// до источника (`overrides_touch_sources`), и тогда адреса остаются
/// побайтово прежними: корень, лента без публикаций и экземпляр без
/// переопределений не платят ничего.
pub fn gen_salt(published: &[kt_model::PublishedParam], ov: &[kt_model::Override]) -> u64 {
    if !kt_model::overrides_touch_sources(published, ov) {
        return 0;
    }
    // Debug-представление детерминировано (f64 печатается
    // shortest-roundtrip), и новое поле переопределения попадает в соль
    // само — тот же приём, что у `content_hash`.
    xxhash_rust::xxh3::xxh3_64(format!("{ov:?}").as_bytes()) | 1
}

/// Адрес источника генератора ВНУТРИ экземпляра с переопределениями.
///
/// До Р.59 адрес однозначно определял значения: событие одно — значения
/// одни. Переопределение снимает эту посылку (одно событие внутри ленты
/// играет в двух выпусках разным заголовком), и адрес обязан её вернуть:
/// иначе заказ и чтение сойдутся на одном адресе — инвариант 4 формально
/// цел, — а кадр придёт ЧУЖОЙ, и «второй выпуск» молча покажет первый.
///
/// Свободных битов в адресе нет (клип < кусок < генератор < лента), и
/// адрес солится ХЭШЕМ. Это законно ровно потому же, почему законен
/// `content_hash`: 62 бита, чистая функция, и `gen_sources` считает её
/// ТЕМ ЖЕ кодом — заказ и чтение спрашивают одно и то же.
pub fn gen_source_addr_in(id: kt_model::ClipId, salt: u64) -> MediaId {
    if salt == 0 {
        return gen_source_addr(id);
    }
    let h = xxhash_rust::xxh3::xxh3_64(
        &[id.0.to_le_bytes().as_slice(), salt.to_le_bytes().as_slice()].concat(),
    );
    MediaId(kt_model::GEN_TAG | (h & (kt_model::GEN_TAG - 1)))
}

/// `clip_source_addr` внутри экземпляра ленты: у генератора — солёный
/// адрес, у всех остальных — адрес медиа, как было.
pub fn clip_source_addr_in(c: &kt_model::ClipEvent, salt: u64) -> MediaId {
    match c.r#gen {
        Some(_) => gen_source_addr_in(c.id, salt),
        None => c.media,
    }
}

/// Все генераторы ДОКУМЕНТА: корень и каждая лента, через ту же дверь
/// выдачи клипов, что и рендер (`effective_reel` — морферы уже
/// применены). Плеер, экспорт и kt-cli поднимают источники ровно по
/// этому списку: второго перечисления быть не должно, иначе экспорт
/// отрендерил бы не то, что показывало превью.
pub fn gen_sources(project: &Project) -> Vec<(MediaId, kt_model::GenState)> {
    let mut out: Vec<(MediaId, kt_model::GenState)> = Vec::new();
    let mut have: std::collections::HashSet<MediaId> = std::collections::HashSet::new();
    let mut push = |addr: MediaId, st: kt_model::GenState| {
        if have.insert(addr) {
            out.push((addr, st));
        }
    };
    // 1) КАЖДАЯ лента со своим шаблонным содержимым — как было до Р.59.
    //    Ленту, никуда не поставленную, тоже видно: её показывает бин и
    //    её считает служба превью, и терять её источники нельзя.
    for seq in std::iter::once(None).chain(project.sequences.iter().map(|s| Some(s.id))) {
        for c in project.effective_reel(seq).iter() {
            if let Some(k) = &c.r#gen {
                push(
                    gen_source_addr(c.id),
                    kt_model::GenState { kind: k.clone(), auto: c.gen_auto.clone() },
                );
            }
        }
    }
    // 2) ОБХОД ДЕРЕВА ЭКЗЕМПЛЯРОВ (Р.59, стадия 3): у экземпляра с
    //    переопределениями свои значения, а значит и свой адрес. Список
    //    строится ТЕМ ЖЕ кодом, что и адреса в плане (`build_reel`), —
    //    разойдись они, префетч грел бы адрес, которого нет.
    let mut seen: std::collections::HashSet<(MediaId, u64)> = std::collections::HashSet::new();
    let mut stack: Vec<(Option<MediaId>, Vec<kt_model::Override>, usize)> =
        vec![(None, Vec::new(), 0)];
    while let Some((seq, ov, depth)) = stack.pop() {
        let published: &[kt_model::PublishedParam] = match seq {
            None => &project.published,
            Some(id) => match project.sequence(id) {
                Some(s) => &s.published,
                None => continue,
            },
        };
        let salt = gen_salt(published, &ov);
        for c in project.resolve_reel(seq, &ov).clips.iter() {
            if let Some(k) = &c.r#gen {
                push(
                    gen_source_addr_in(c.id, salt),
                    kt_model::GenState { kind: k.clone(), auto: c.gen_auto.clone() },
                );
            }
            if project.sequence(c.media).is_some()
                && depth < kt_model::MAX_NEST_DEPTH
                && seen.insert((c.media, instance_content(project, c.media, &c.overrides)))
            {
                stack.push((Some(c.media), c.overrides.clone(), depth + 1));
            }
        }
    }
    out
}

/// Лента, В КОТОРОЙ лежит событие, и все её предки — по этому списку
/// протухает мемоизация суб-композитов при перенастройке источника
/// (Р.59, стадия 2).
///
/// Скрытый дефект, который здесь закрывается: правка титра ВНУТРИ ленты
/// идёт по retune-полосе, проект при этом не меняется, а ключ мемо
/// суб-композита — `content_hash` ленты — считается от МОДЕЛИ. Значит он
/// не двигается, и лента продолжает отдавать картинку со старым текстом
/// до первой правки плана. Сторож — t663.
///
/// Корень в список не входит: у него мемо нет, его композит собирается
/// каждый кадр.
pub fn reel_chain_of_clip(project: &Project, id: kt_model::ClipId) -> Vec<MediaId> {
    let origin = kt_model::slice_origin(id).unwrap_or(id);
    let Some(home) =
        project.sequences.iter().find(|s| s.clips.iter().any(|c| c.id == origin)).map(|s| s.id)
    else {
        return Vec::new();
    };
    let mut chain = vec![home];
    let mut i = 0;
    while i < chain.len() {
        let cur = chain[i];
        i += 1;
        for s in &project.sequences {
            if !chain.contains(&s.id) && s.clips.iter().any(|c| c.media == cur) {
                chain.push(s.id);
            }
        }
    }
    chain
}

/// Адрес ленты в кэше. Старший бит `MediaId` отдан вложениям (Р.50):
/// файл с таким адресом молча делил бы кэш с секвенцией, и это отвергает
/// `validate`.
pub fn nest_source_id(seq: MediaId) -> SourceId {
    SourceId(kt_model::NEST_TAG | seq.0)
}

/// Прямая альфа из премультиплицированной — ОДНА на проект, дом в kt-fx
/// (её же зовёт premul-гейт ядер, Р.53 п.418). Здесь остался только
/// реэкспорт под прежним именем.
use kt_fx::video::unpremultiply;

#[allow(clippy::too_many_arguments)]
/// Кадр слоя для CPU-исполнителя: пре-FX кадр из плана либо результат
/// FX-цепочки (мемоизированный пост-FX кэшем). None — кадра нет (голод).
/// Свежий пост-FX кадр возвращается ЧЕРЕЗ кэш (одна лишняя выборка на
/// холодную цепочку — дешевле клона кадра).
fn layer_frame_cpu(
    src: &LayerSrc<'_>,
    cache: &BurstCache,
    q_key: (u32, u32),
    scratch: &mut kt_fx::video::FxScratch,
    blend: &mut Vec<u8>,
    t: Tick,
    memoize: bool,
    complete: &mut bool,
) -> Option<Arc<VideoFrame>> {
    // цепочка видео-FX (п.25): кэш хранит ПРЕ-FX кадры (правда о
    // coverage), пост-FX кадр мемоизируется отдельным kind'ом
    // (п.2 р.7 — иначе цепочка платится на КАЖДЫЙ compose и loop
    // играет 6.8 fps даже прогретым); эффекты детерминированы по
    // (t, fx_hash) → байт-идентичность сохранена. GPU-исполнитель эту
    // мемоизацию не трогает (у него свой ReadbackCache в kt-gpu).
    let has_fx = kt_fx::video::has_video_fx_in(&src.fx, src.lead, q_key.0, q_key.1);
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
    let nest_scale = src.input.nest().map_or(0, |n| n.nest_scale);
    let key_hash = postfx_key_hash(src.fx_hash, &src.cam3d, src.src_scale, nest_scale, &src.lead);
    if has_fx && let Some(f) = cache.get_postfx_video(src.source, q_key, key_hash, t.0) {
        return Some(f);
    }
    // Вход слоя: кадр из кэша или СУБ-КОМПОЗИТ вложенной ленты.
    let resolved;
    let frame = match &src.input {
        crate::video_plan::LayerInput::Frame(f) => f.as_ref(),
        crate::video_plan::LayerInput::Nest(n) => {
            let sub = nest_frame_cpu(n, cache, scratch, blend, memoize, complete);
            // Ступень ленты выше входа цепочки (Р.58, п.459): суб-композит
            // собран КРУПНЕЕ, чем эта цепочка умеет потребить — вписываем
            // его ТОЙ ЖЕ билинейкой, что и сухую сторону смешивания
            // (`fit_into`, зеркало на GPU — `Resample` во весь кадр).
            resolved = match (sub, n.fit_to(src.src_scale, q_key.0, q_key.1)) {
                (Some(f), Some((fw, fh))) => Some(fit_frame_cpu(&f, fw, fh, scratch)),
                (f, _) => f,
            };
            resolved.as_ref()
        }
    };
    if !has_fx {
        // цепочка из одной силы пикселей не трогает: смешивать вход с
        // самим собой нечего
        return frame.cloned();
    }
    let frame = frame?;
    let ctx = kt_fx::video::VideoFxCtx {
        out_w: q_key.0,
        out_h: q_key.1,
        u: src.u,
        burst_index: t.0,
        cam3d: src.cam3d,
        in_scale: src.src_scale,
        lead: src.lead,
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
fn postfx_key_hash(
    fx_hash: u64,
    cam3d: &Option<Cam3d>,
    src_scale: u32,
    nest_scale: u32,
    lead: &Option<kt_fx::conform::LeadCtx>,
) -> u64 {
    use xxhash_rust::xxh3::Xxh3;
    let mut h = Xxh3::new();
    h.update(&fx_hash.to_le_bytes());
    h.update(&src_scale.to_le_bytes());
    // контекст ведущего узла (Р.62): та же цепочка на том же кадре с
    // другим правилом приведения — другая картинка, другой ключ
    match lead {
        None => h.update(&[0u8]),
        Some(l) => {
            h.update(&[1u8, l.rule as u8]);
            if let Some((vw, vh)) = l.view {
                h.update(&vw.to_bits().to_le_bytes());
                h.update(&vh.to_bits().to_le_bytes());
            }
        }
    }
    // Ступень ЛЕНТЫ — тоже в ключ (Р.58, п.459). У экземпляра без зума
    // вход цепочки равен 1 и при ступени 1, и при ступени 3 (её поднял
    // зумящий БЛИЗНЕЦ), а байты разные: во втором случае цепочка получает
    // вписанный суб-композит. Без этого слагаемого кэш отдавал бы стейл —
    // тот же класс, что t313 по оси размера.
    h.update(&nest_scale.to_le_bytes());
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
fn solo_fade(clip: &VideoClip, t: Tick) -> Option<(u32, bool, crate::TransMix, bool)> {
    let frac =
        |w: (Tick, Tick)| (((t.0 - w.0.0) * 4096) / (w.1.0 - w.0.0).max(1)).clamp(0, 4096) as u32;
    if let Some(w) = clip.fade_in
        && clip.fade_in_solo
        && t.0 >= w.0.0
        && t.0 < w.1.0
    {
        let ta = clip.fade_in_trans.as_ref();
        return Some((
            frac(w),
            true,
            ta.map(|a| a.mix_at(t)).unwrap_or_default(),
            ta.is_some_and(|a| a.invert),
        ));
    }
    if let Some(w) = clip.fade_out
        && clip.fade_out_solo
        && t.0 >= w.0.0
        && t.0 < w.1.0
    {
        let ta = clip.fade_out_trans.as_ref();
        return Some((
            4096 - frac(w),
            false,
            ta.map(|a| a.mix_at(t)).unwrap_or_default(),
            ta.is_some_and(|a| a.invert),
        ));
    }
    None
}

/// Инверсия перехода (Р.51, п.398): «жест задом наперёд» — поменять
/// стороны местами и обратить прогресс, `kernel(b, a, 1−k)`. Кроссфейд
/// не трогаем: для него это тождество, а целочисленный `dissolve_into`
/// держит GOLDEN_VIDEO. КОНТРАКТ: любая сборка `PlanLayer::Dissolve`
/// (в т.ч. будущая пара наложения) обязана пройти через этот хелпер —
/// иначе инверсия на ней молча потеряется. Ключи кэша едут сами: своп
/// меняет порядок `put_src`, k уже хэшируется.
fn apply_invert<'a>(
    a: &mut crate::video_plan::LayerSrc<'a>,
    b: &mut crate::video_plan::LayerSrc<'a>,
    k: &mut u32,
    mix: &crate::TransMix,
    invert: bool,
) {
    if invert && !mix.is_crossfade() {
        std::mem::swap(a, b);
        *k = 4096 - *k;
    }
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
