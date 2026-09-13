//! Слияние геометрических видео-FX (Р.44, конкатенация трансформов,
//! F-CMP-001): подряд идущие Pan/CamMotion/Mirror/EventGrid исполняются
//! ОДНИМ проходом обратных отображений — без промежуточных растеризаций и
//! 8-битных квантований между шагами. Нелокальные FX — барьеры.
//!
//! Здесь живут ВСЕ решения (сегментация, программа, legacy-vs-fused):
//! kt-engine (CPU) и kt-gpu — исполнители одной и той же программы
//! (инвариант №7; прецедент — общая параметрика `pan_frame`/`cam_at`).
//! Правишь построение или CPU-исполнитель — правь WGSL-зеркало
//! `geom_fused.wgsl` в kt-gpu ТЕМ ЖЕ КОММИТОМ (t181).
//!
//! Гарантия байтов (Р.44): серия длины 1 из Pan/CamMotion/Mirror идёт
//! СТАРЫМИ ядрами (`GeomExec::Legacy`) — байт-в-байт как до слияния (t297);
//! одиночный EventGrid идёт слитым проходом, но арифметика тайл-локальной
//! фазы воспроизводит старую формулу дословно — голден t26 стоит на этом.
//! Мульти-шаговые серии дают НОВЫЕ байты (лучше: без потерь) — это
//! санкционировано владельцем (docs/user-feedback.md Р.44).

use crate::video::{Frame, FxScratch};
use kt_model::FxInstance;

/// Потолок гео-шагов в одном слитом проходе: длиннее — серия режется на
/// два прохода по границе шага (в точке разреза качество = сегодняшнее).
/// Ограничен ёмкостью юниформы GPU-пасса: худший случай — 6 EventGrid =
/// 6 Fold (по 2 vec4) + 7 Affine = 13 примитивов ≤ MAX_PRIMS.
pub const MAX_GEO_STEPS: usize = 6;
/// Потолок ЛОГИЧЕСКИХ примитивов программы (kinds пакуются 4 битами в два
/// u32 заголовка юниформы — максимум 16).
pub const MAX_PRIMS: usize = 16;

/// Разрешённая 3D-камера слоя (Р.29; с Р.44 живёт здесь — Pan3d считается
/// обоими бэкендами): активная/прибитая камера с уже интерполированными
/// кейфреймами. Считается при построении плана — исполнители получают
/// готовые параметры.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cam3d {
    pub cam: kt_model::CameraParams,
    /// Мировой масштаб: quality px / project px (превью меньше проекта,
    /// а координаты Pan3d/камер — в px ПРОЕКТА).
    pub world_scale: f64,
}

/// Геометрический ли эффект (обратимое отображение «выходной пиксель →
/// точка входа»). Pan3d — чистая 3×3-гомография (плоский квад, z_clip=0),
/// с Р.44 он гео-шаг слитого прохода и считается на ОБОИХ бэкендах.
pub fn is_geo(f: &FxInstance) -> bool {
    matches!(
        f,
        FxInstance::Pan { .. }
            | FxInstance::CamMotion { .. }
            | FxInstance::Mirror { .. }
            | FxInstance::EventGrid { .. }
            | FxInstance::Pan3d { .. }
    )
}

/// Диагональная аффинная карта index-space: `x_in = ax·x + bx` (линейная
/// часть всех наших 2D-гео-FX осе-выровнена, поворотов нет).
#[derive(Debug, Clone, Copy)]
pub struct Aff {
    pub ax: f64,
    pub bx: f64,
    pub ay: f64,
    pub by: f64,
}

impl Aff {
    pub const IDENTITY: Aff = Aff { ax: 1.0, bx: 0.0, ay: 1.0, by: 0.0 };
    fn is_identity(&self) -> bool {
        self.ax == 1.0 && self.bx == 0.0 && self.ay == 1.0 && self.by == 0.0
    }
    /// Композиция `m ∘ self` (сначала self, потом m) — считается в f64 при
    /// построении; GPU получает УЖЕ перемноженные коэффициенты (однократное
    /// округление в f32, риск дрейфа f32-цепочек снят).
    fn then(&self, m: &Aff) -> Aff {
        Aff {
            ax: m.ax * self.ax,
            bx: m.ax * self.bx + m.bx,
            ay: m.ay * self.ay,
            by: m.ay * self.by + m.by,
        }
    }
}

/// Примитив программы обратного прохода. Порядок примитивов = порядок
/// применения (от ПОСЛЕДНЕГО эффекта серии к первому); после последнего
/// примитива координата — в index-space входного кадра серии.
#[derive(Debug, Clone, Copy)]
pub enum GeomPrim {
    /// Композиция обратных аффинных шагов (Pan/CamMotion/Mirror и
    /// пред/пост-карты соседей).
    Affine(Aff),
    /// Позиционный кламп к прямоугольнику виртуального входа шага —
    /// СЕМАНТИЧЕСКОЕ «плато» Pan при zoom<1/Stretch (рамка шире входа).
    /// Для влезающей рамки НЕ ставится: вылет ≤ 0.5 px у кромки — артефакт
    /// пошаговой растеризации, а не семантика. Отключённая ось несёт
    /// сентинел ±BIG.
    Clamp { x0: f64, x1: f64, y0: f64, y1: f64 },
    /// Тайл-modulo EventGrid: вход — «d»-координата (центр-space минус
    /// смещение сетки), вне [0,gw)×[0,gh) — чёрный непрозрачный (короткое
    /// замыкание), внутри — тайл-локальная фаза и точная карта в index-space
    /// входа шага: `(r / cell) * in − 0.5` — ДОСЛОВНО арифметика старого
    /// event_grid, на ней стоит байт-идентичность одиночной сетки (t26).
    Fold { gw: f64, gh: f64, cw: f64, ch: f64, in_w: f64, in_h: f64 },
    /// Обратная гомография Pan3d: вход — ЦЕНТР-space выхода шага (пред-
    /// аффин +0.5 скомпонован построителем), `t = m·(c,1)`; `t.z ≤ 0` —
    /// точка за камерой (клиппинг растеризатора) → прозрачно; `q = t.xy/t.z`
    /// вне [0,1]² — вне квада → прозрачно; иначе выход = q (uv), пост-аффин
    /// `q·in − 0.5` скомпонован построителем (дословно старый pan3d.wgsl).
    Proj { m: [[f64; 3]; 3] },
}

/// Сентинел отключённой оси Clamp (влезает в f32 юниформы без inf).
pub const CLAMP_OFF: f64 = 1.0e30;

/// Потолок поточечных tap-опов в одной программе (таблица в юниформе GPU:
/// 8 × 2 vec4-слота за примами).
pub const MAX_TAP_OPS: usize = 8;
/// Потолок vec4-слотов примов (за ними в юниформе живёт tap-таблица).
pub const MAX_PRIM_SLOTS: usize = 46;

/// Поточечный ли эффект (цвет пикселя без соседей) — кандидат на
/// поглощение внутрь слитого прохода (Р.45, п.312). Поглощаются только
/// ВНУТРЕННИЕ (между гео-шагами рана): ведущие/замыкающие остаются
/// барьерами — их байты при K=1 неотличимы от прежних.
pub fn is_pointwise(f: &FxInstance) -> bool {
    matches!(f, FxInstance::Greenscreen { .. } | FxInstance::Noise { .. })
}

/// Поточечный оп, применяемый к ЗНАЧЕНИЮ САБТАПА (после билинейки, до
/// box-среднего), в цепочечном порядке (Р.45, п.312).
#[derive(Debug, Clone, Copy)]
pub enum TapKind {
    /// Greenscreen: f64/f32-континуум целочисленной формулы ядра.
    Gs { key: [f64; 3], tol: f64, hard: f64, spill: f64, kc: usize },
    /// Noise: зерно прибито к сетке СВОЕГО шага (решение Р.45) — офсет
    /// считается от floor координаты стадии; `special` — особая формула
    /// пользовательского `size == 1 && !color` (у неё другое зерно).
    Noise { seed: u32, k: i64, cell: u32, color: bool, special: bool, stage_w: u32, stage_h: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct TapOp {
    /// Координата опа записывается после применения этого числа примов
    /// обратного прохода (построитель разрезает склейки аффин на стадиях
    /// опов). При коротком замыкании на приме d жив оп с after_prim ≤ d.
    pub after_prim: usize,
    pub kind: TapKind,
}

/// Потолок тапов суперсемплинга на ось (максимум 4×4 = 16 сабтапов).
pub const SS_MAX: u32 = 4;

/// Суперсемплинг по якобиану (Р.44, п.309): сколько сабтапов на ось.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsSpec {
    /// Аффинная программа (в т.ч. с Fold): якобиан КОНСТАНТЕН по кадру —
    /// число тапов посчитано один раз в f64 при построении и едет в
    /// юниформу. Паритетный риск CPU/GPU нулевой по построению.
    Const { nx: u32, ny: u32 },
    /// В программе есть Proj (перспектива): якобиан меняется по кадру —
    /// оба исполнителя меряют его конечными разностями (две координатные
    /// пробы на пиксель) и считают тапы ОДНОЙ формулой `ss_taps`.
    PerPixel,
}

/// Число сабтапов на ось из длины образа единичного выходного шага в
/// пространстве источника. Квант 1/32 px гасит float-шум на тождестве:
/// суперсемплинг включается строго при len > 1 + 1/64 (len == 1.0 —
/// ровно один тап, прежний путь байт-в-байт; сторожит t26
/// pan-stretch-identity и t300).
pub fn ss_taps(len: f64) -> u32 {
    let q = (len * 32.0 + 0.5).floor() / 32.0;
    (q.ceil() as u32).clamp(1, SS_MAX)
}

/// Слитая программа гео-серии.
#[derive(Debug, Clone)]
pub struct GeomProgram {
    pub prims: Vec<GeomPrim>,
    /// Размер входного кадра серии (для финального тапа).
    pub in_w: u32,
    pub in_h: u32,
    /// Размер выхода (= размеру выхода плана, max(2) как у старых ядер).
    pub out_w: u32,
    pub out_h: u32,
    /// Свёртка правил альфы по шагам (EventGrid ⇒ known, непрозрачный).
    pub has_alpha_known: bool,
    pub has_alpha: bool,
    /// Суперсемплинг при минификации (п.309).
    pub ss: SsSpec,
    /// Поглощённые поточечные опы (Р.45, п.312) в цепочечном порядке;
    /// их after_prim идут по УБЫВАНИЮ (ранний в цепочке — глубже в
    /// обратном проходе).
    pub tap_ops: Vec<TapOp>,
    /// Абсолютная длина образа единичного выходного шага по осям (лупа
    /// планировщика K×-барьеров, п.314): для аффинных программ — точная
    /// (та же, что у ss-тапов); для Proj — оценка пробами (центр + углы).
    pub lens: (f64, f64),
}

impl GeomProgram {
    /// vec4-слоты примов в юниформе GPU (лимит MAX_PRIM_SLOTS; бюджет
    /// проверяет планировщик — при превышении ран режется по гео-границе).
    pub fn prim_slots(&self) -> usize {
        self.prims
            .iter()
            .map(|p| match p {
                GeomPrim::Affine(_) | GeomPrim::Clamp { .. } => 1,
                GeomPrim::Fold { .. } => 2,
                GeomPrim::Proj { .. } => 3,
            })
            .sum()
    }
}

/// Решение исполнения гео-рана. Принимается ЗДЕСЬ и только здесь — оба
/// бэкенда обязаны спрашивать эту функцию, а не решать сами.
pub enum GeomExec {
    /// Серия длины 1 Pan/CamMotion/Mirror: старые ядра (CPU: pan_zoom /
    /// mirror; GPU: Resample / Mirror) — байты как до Р.44.
    Legacy,
    /// Слитый проход обратных отображений.
    Fused(GeomProgram),
}

/// Построить решение для гео-шагов рана (`steps` — ТОЛЬКО гео, в порядке
/// применения; прозрачные аудио уже выброшены сборщиком).
#[allow(clippy::too_many_arguments)]
pub fn build_geom_exec(
    steps: &[&FxInstance],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
    burst_index: i64,
    k_in: u32,
    k_out: u32,
) -> GeomExec {
    debug_assert!(!steps.is_empty());
    if steps.len() == 1 {
        // одиночное зеркало ресемпла не делает — всегда старое ядро
        // (in-place перестановка верна на любом физическом размере)
        if matches!(steps[0], FxInstance::Mirror { .. }) {
            return GeomExec::Legacy;
        }
        // одиночный Pan/CamMotion БЕЗ минификации И БЕЗ K×-краёв — старое
        // ядро байт-в-байт; иначе — слитый проход (суперсемплинг п.309 /
        // конъюгация п.314; legacy pan_zoom умеет только 1×-выход)
        if k_in == 1
            && k_out == 1
            && matches!(steps[0], FxInstance::Pan { .. } | FxInstance::CamMotion { .. })
        {
            let prog =
                build_program(steps, in_w, in_h, out_w, out_h, u, cam3d, burst_index, 1, 1);
            if prog.ss == (SsSpec::Const { nx: 1, ny: 1 }) {
                return GeomExec::Legacy;
            }
            return GeomExec::Fused(prog);
        }
    }
    GeomExec::Fused(build_program(
        steps, in_w, in_h, out_w, out_h, u, cam3d, burst_index, k_in, k_out,
    ))
}

/// Прямые параметры одного шага (собираются проходом вперёд — размеры
/// каждого шага зависят от предыдущих).
enum Step {
    /// Pan/CamMotion: рамка pan_frame в координатах входа шага.
    Frame { x0: f64, y0: f64, cw: f64, ch: f64, iw: u32, ih: u32 },
    Mirror { h: bool, v: bool, iw: u32, ih: u32 },
    Grid { offx: u32, offy: u32, gw: u32, gh: u32, cw: u32, ch: u32, iw: u32, ih: u32 },
    /// Pan3d: ОБРАТНАЯ гомография (центр-space выхода → uv квада).
    Proj { m: [[f64; 3]; 3], iw: u32, ih: u32 },
    /// Поглощённый поточечный оп (Р.45): применяется к значению сабтапа
    /// на координате СВОЕЙ стадии.
    Tap(TapKind),
}

#[allow(clippy::too_many_arguments)]
fn build_program(
    steps: &[&FxInstance],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
    burst_index: i64,
    k_in: u32,
    k_out: u32,
) -> GeomProgram {
    let (mow, moh) = (out_w.max(2), out_h.max(2));
    // Конъюгация K× (Р.45, п.314): ВСЯ арифметика шагов живёт в
    // 1×-семантике (pan_frame, целочисленная сетка EventGrid,
    // pan3d_forward — нетронуты); физические масштабы вносят два аффина
    // по краям, вливаемые в существующие композиции. При K=1 оба —
    // тождество (умножение на 1.0 и сложение с 0.0 точны в f64 — байты
    // Р.44 нетронуты). in_w/in_h приходят ФИЗИЧЕСКИМИ (k_in×логика).
    let (k_in, k_out) = (k_in.max(1), k_out.max(1));
    debug_assert!(
        in_w.is_multiple_of(k_in) && in_h.is_multiple_of(k_in),
        "physical input is k_in x logical"
    );
    let (lin_w, lin_h) = (in_w / k_in, in_h / k_in);
    // --- вперёд: ЛОГИЧЕСКИЕ размеры и прямые параметры каждого шага ---
    let (mut w, mut h) = (lin_w, lin_h);
    let mut fw: Vec<Step> = Vec::with_capacity(steps.len());
    let mut has_alpha_known = false;
    let mut has_alpha = false;
    for f in steps {
        match f {
            FxInstance::Pan { mode, pan_x, pan_y, zoom } => {
                let (x0, y0, cw, ch) =
                    crate::video::pan_frame(w, h, out_w, out_h, *mode, *pan_x, *pan_y, *zoom);
                fw.push(Step::Frame { x0, y0, cw, ch, iw: w, ih: h });
                w = mow;
                h = moh;
            }
            FxInstance::CamMotion { mode, ease, time_scale, keyframes } => {
                let (px, py, z) = crate::video::cam_at(keyframes, u, *ease, *time_scale);
                let (x0, y0, cw, ch) =
                    crate::video::pan_frame(w, h, out_w, out_h, *mode, px, py, z);
                fw.push(Step::Frame { x0, y0, cw, ch, iw: w, ih: h });
                w = mow;
                h = moh;
            }
            FxInstance::Mirror { horizontal, vertical } => {
                if *horizontal || *vertical {
                    fw.push(Step::Mirror { h: *horizontal, v: *vertical, iw: w, ih: h });
                }
            }
            FxInstance::EventGrid { cols, rows, center } => {
                let cols = (*cols).clamp(1, 512);
                let rows = (*rows).clamp(1, 512);
                let (cw, ch) = ((mow / cols).max(1), (moh / rows).max(1));
                let (gw, gh) = (cw * cols, ch * rows);
                // гард «сетка крупнее кадра» — общий с ядрами (п.306)
                let (offx, offy) = if *center && gw <= mow && gh <= moh {
                    ((mow - gw) / 2, (moh - gh) / 2)
                } else {
                    (0, 0)
                };
                fw.push(Step::Grid { offx, offy, gw, gh, cw, ch, iw: w, ih: h });
                w = mow;
                h = moh;
                has_alpha_known = true;
                has_alpha = false;
            }
            FxInstance::Pan3d {
                pos_x,
                pos_y,
                pos_z,
                rot_x,
                rot_y,
                rot_z,
                anchor_x,
                anchor_y,
                scale,
                camera: _, // камера уже разрешена движком в cam3d
            } => {
                // нет камеры — нет 3D: шаг пропускается целиком, размеры
                // не меняются (зеркало прежнего `continue` в chain_ops)
                let Some(c3) = cam3d else { continue };
                let g = pan3d_forward(
                    &c3,
                    (mow, moh),
                    [*pos_x, *pos_y, *pos_z],
                    [*rot_x, *rot_y, *rot_z],
                    *scale,
                    [*anchor_x, *anchor_y],
                    (w as f64, h as f64),
                );
                // вырожденная гомография (|det| ≈ 0, квад ребром) —
                // прозрачный слой: m с t.z ≡ −1
                let m = invert3(&g)
                    .unwrap_or([[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]]);
                fw.push(Step::Proj { m, iw: w, ih: h });
                w = mow;
                h = moh;
                // вне квада слой прозрачен
                has_alpha_known = true;
                has_alpha = true;
            }
            FxInstance::Greenscreen { key_r, key_g, key_b, tolerance, softness, spill } => {
                let tol = (*tolerance).clamp(1, 200) as f64;
                let soft = (*softness).clamp(0, 100) as f64;
                let key = [*key_r, *key_g, *key_b];
                fw.push(Step::Tap(TapKind::Gs {
                    key: [key[0] as f64, key[1] as f64, key[2] as f64],
                    tol,
                    hard: (tol - soft).max(0.0),
                    spill: spill.clamp(0.0, 1.0),
                    kc: (0..3).max_by_key(|&i| key[i]).unwrap_or(1),
                }));
                has_alpha_known = true;
                has_alpha = true;
            }
            FxInstance::Noise { amount, size, color } => {
                let k = (amount.clamp(0.0, 1.0) * 32.0) as i64;
                // k == 0 — эффект пуст (как ранний выход ядра и ветка GPU)
                if k > 0 {
                    let size = (*size).clamp(1, 64);
                    fw.push(Step::Tap(TapKind::Noise {
                        seed: crate::video::noise_seed(burst_index),
                        k,
                        cell: size,
                        color: *color,
                        special: size == 1 && !*color,
                        stage_w: w,
                        stage_h: h,
                    }));
                }
            }
            _ => debug_assert!(false, "non-geo step in build_program"),
        }
    }
    // --- назад: обратные карты, соседние аффинные ПЕРЕМНОЖЕНЫ в f64 ---
    let mut prims: Vec<GeomPrim> = Vec::new();
    let mut taps_rev: Vec<TapOp> = Vec::new();
    // S_out: физический индекс K×-выхода → логический (полутексельная
    // конвенция, как bilinear_into); k_out=1 → тождество точно
    let ko = k_out as f64;
    let mut pending = Aff {
        ax: 1.0 / ko,
        bx: 0.5 / ko - 0.5,
        ay: 1.0 / ko,
        by: 0.5 / ko - 0.5,
    };
    let flush = |pending: &mut Aff, prims: &mut Vec<GeomPrim>| {
        if !pending.is_identity() {
            prims.push(GeomPrim::Affine(*pending));
        }
        *pending = Aff::IDENTITY;
    };
    for st in fw.iter().rev() {
        match st {
            Step::Frame { x0, y0, cw, ch, iw, ih } => {
                let ax = cw / mow as f64;
                let ay = ch / moh as f64;
                pending = pending
                    .then(&Aff { ax, bx: x0 + 0.5 * ax - 0.5, ay, by: y0 + 0.5 * ay - 0.5 });
                // рамка шире входа (zoom<1 / Stretch): «плато» из растянутой
                // кромки — semantика старого пошагового клампа тапов
                let cx = *cw > *iw as f64;
                let cy = *ch > *ih as f64;
                if cx || cy {
                    flush(&mut pending, &mut prims);
                    prims.push(GeomPrim::Clamp {
                        x0: if cx { 0.0 } else { -CLAMP_OFF },
                        x1: if cx { (*iw - 1) as f64 } else { CLAMP_OFF },
                        y0: if cy { 0.0 } else { -CLAMP_OFF },
                        y1: if cy { (*ih - 1) as f64 } else { CLAMP_OFF },
                    });
                }
            }
            Step::Mirror { h, v, iw, ih } => {
                pending = pending.then(&Aff {
                    ax: if *h { -1.0 } else { 1.0 },
                    bx: if *h { (*iw - 1) as f64 } else { 0.0 },
                    ay: if *v { -1.0 } else { 1.0 },
                    by: if *v { (*ih - 1) as f64 } else { 0.0 },
                });
            }
            Step::Grid { offx, offy, gw, gh, cw, ch, iw, ih } => {
                // индекс → «d»-координата сетки (центр-space минус смещение);
                // для одиночной сетки composed = ox + (0.5 − offx) — точно
                pending = pending.then(&Aff {
                    ax: 1.0,
                    bx: 0.5 - *offx as f64,
                    ay: 1.0,
                    by: 0.5 - *offy as f64,
                });
                flush(&mut pending, &mut prims);
                prims.push(GeomPrim::Fold {
                    gw: *gw as f64,
                    gh: *gh as f64,
                    cw: *cw as f64,
                    ch: *ch as f64,
                    in_w: *iw as f64,
                    in_h: *ih as f64,
                });
                // выход Fold — уже index-space входа шага
            }
            Step::Proj { m, iw, ih } => {
                // индекс → центр-space выхода шага
                pending = pending.then(&Aff { ax: 1.0, bx: 0.5, ay: 1.0, by: 0.5 });
                flush(&mut pending, &mut prims);
                prims.push(GeomPrim::Proj { m: *m });
                // uv → index-space входа: sx = q·in − 0.5 (как pan3d.wgsl)
                pending = Aff { ax: *iw as f64, bx: -0.5, ay: *ih as f64, by: -0.5 };
            }
            Step::Tap(kind) => {
                // стадия опа: текущая координата обратного прохода — это
                // растр его шага; склейка аффин разрезается здесь
                flush(&mut pending, &mut prims);
                taps_rev.push(TapOp { after_prim: prims.len(), kind: *kind });
            }
        }
    }
    // S_in⁻¹: логический входной индекс → физический K_in×-буфер;
    // k_in=1 → тождество точно
    let ki = k_in as f64;
    pending = pending.then(&Aff {
        ax: ki,
        bx: (ki - 1.0) / 2.0,
        ay: ki,
        by: (ki - 1.0) / 2.0,
    });
    flush(&mut pending, &mut prims);
    // цепочечный порядок опов (обратный проход собрал их с конца);
    // after_prim при этом убывают
    taps_rev.reverse();
    let tap_ops = taps_rev;
    // бюджеты (MAX_PRIMS/MAX_PRIM_SLOTS/MAX_TAP_OPS) проверяет
    // планировщик и при превышении режет ран по гео-границе
    // Физический выход = k_out × логический (серия из одних Mirror / Pan3d
    // без камеры логических размеров не меняет — как старый путь).
    let (pout_w, pout_h) = (w * k_out, h * k_out);
    // Суперсемплинг (п.309): у аффинной программы (в т.ч. с Fold) якобиан
    // КОНСТАНТЕН — тапы считаются один раз здесь, в f64, и едут в юниформу
    // (конъюгационные аффины уже в примах — лупа автоматически физическая);
    // Proj (перспектива) делает его по-пиксельным.
    let (ss, lens) = if prims.iter().any(|p| matches!(p, GeomPrim::Proj { .. })) {
        // лупа для планировщика K× — оценка пробами обратной карты
        (SsSpec::PerPixel, probe_program_lens(&prims, pout_w, pout_h))
    } else {
        let (mut lx, mut ly) = (1.0f64, 1.0f64);
        for p in &prims {
            match p {
                GeomPrim::Affine(a) => {
                    lx *= a.ax.abs();
                    ly *= a.ay.abs();
                }
                GeomPrim::Fold { cw, ch, in_w, in_h, .. } => {
                    lx *= in_w / cw;
                    ly *= in_h / ch;
                }
                GeomPrim::Clamp { .. } | GeomPrim::Proj { .. } => {}
            }
        }
        (SsSpec::Const { nx: ss_taps(lx), ny: ss_taps(ly) }, (lx, ly))
    };
    GeomProgram {
        prims,
        in_w,
        in_h,
        out_w: pout_w,
        out_h: pout_h,
        has_alpha_known,
        has_alpha,
        ss,
        tap_ops,
        lens,
    }
}

/// Оценка лупы Proj-программы для планировщика K× (п.314): длины образов
/// единичных шагов в пяти точках (центр + окрестности углов), максимум по
/// валидным пробам. Неточность даёт мягче/дороже, но ОДИНАКОВО на обоих
/// бэкендах — K в плане один.
fn probe_program_lens(prims: &[GeomPrim], out_w: u32, out_h: u32) -> (f64, f64) {
    let (ow, oh) = (out_w.max(2) as f64, out_h.max(2) as f64);
    let pts = [
        (ow * 0.5, oh * 0.5),
        (1.0, 1.0),
        (ow - 2.0, 1.0),
        (1.0, oh - 2.0),
        (ow - 2.0, oh - 2.0),
    ];
    let coords = |x: f64, y: f64| -> Option<(f64, f64)> {
        match map_pos(prims, x, y) {
            Mapped::At(sx, sy) => Some((sx, sy)),
            _ => None,
        }
    };
    let (mut lx, mut ly) = (1.0f64, 1.0f64);
    for (px, py) in pts {
        if let (Some(a), Some(bx)) = (coords(px, py), coords(px + 1.0, py)) {
            lx = lx.max(((a.0 - bx.0).powi(2) + (a.1 - bx.1).powi(2)).sqrt());
        }
        if let (Some(a), Some(by)) = (coords(px, py), coords(px, py + 1.0)) {
            ly = ly.max(((a.0 - by.0).powi(2) + (a.1 - by.1).powi(2)).sqrt());
        }
    }
    (lx, ly)
}

// ---- Pan3d: гомография (Р.44) ----------------------------------------------
// Квад слоя планарный (z = 0) и z_clip ≡ 0, поэтому отображение
// (q.x, q.y) → экран — чистая 3×3-гомография. Математика ДОСЛОВНО повторяет
// прежний pan3d_mvp (glam f32) + вьюпорт растеризатора, но считается в f64;
// семантика мира/камеры заморожена форматом файла (kt-gpu/CLAUDE.md):
// мир — px проекта × world_scale, X вправо, Y вниз, Z вглубь, повороты
// Rz·Rx·Ry, камера «50 мм» на авто-дистанции −(H/2)·cot(fov/2).

type V3 = [f64; 3];
type M3 = [[f64; 3]; 3];

fn m3_mul(a: &M3, b: &M3) -> M3 {
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    r
}

fn cross(a: &V3, b: &V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: &V3, b: &V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn rot_z(a: f64) -> M3 {
    let (s, c) = a.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

fn rot_x(a: f64) -> M3 {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

fn rot_y(a: f64) -> M3 {
    let (s, c) = a.sin_cos();
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}

/// Обращение 3×3 через adj/det. None — вырождение (квад ребром к камере).
/// Знак t.z обратного образа при adj/det корректен: t = G⁻¹·(c,1) = q̃/w,
/// то есть sign(t.z) = sign(w) — тест «за камерой» получается сам.
fn invert3(g: &M3) -> Option<M3> {
    let adj = [
        [
            g[1][1] * g[2][2] - g[1][2] * g[2][1],
            g[0][2] * g[2][1] - g[0][1] * g[2][2],
            g[0][1] * g[1][2] - g[0][2] * g[1][1],
        ],
        [
            g[1][2] * g[2][0] - g[1][0] * g[2][2],
            g[0][0] * g[2][2] - g[0][2] * g[2][0],
            g[0][2] * g[1][0] - g[0][0] * g[1][2],
        ],
        [
            g[1][0] * g[2][1] - g[1][1] * g[2][0],
            g[0][1] * g[2][0] - g[0][0] * g[2][1],
            g[0][0] * g[1][1] - g[0][1] * g[1][0],
        ],
    ];
    let det = g[0][0] * adj[0][0] + g[0][1] * adj[1][0] + g[0][2] * adj[2][0];
    // масштаб детерминанта у наших матриц ~ (ow·oh·f)² — порог относительный
    let scale = g.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs()));
    if det.abs() <= scale * scale * scale * 1e-12 {
        return None;
    }
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = adj[i][j] / det;
        }
    }
    Some(m)
}

/// ПРЯМАЯ гомография Pan3d: (q.x, q.y, 1) → (c.x·w, c.y·w, w), где q — uv
/// квада 0..1, c — ЦЕНТР-space пикселей выхода.
fn pan3d_forward(
    c3: &Cam3d,
    out: (u32, u32),
    pos: [f64; 3],
    rot: [f64; 3],
    scale: f64,
    anchor: [f64; 2],
    quad: (f64, f64),
) -> M3 {
    let s = c3.world_scale;
    let cam = &c3.cam;
    let (ow, oh) = (out.0 as f64, out.1 as f64);
    let sc = scale.max(1e-6);

    // модель: мир(q) = pos·s + R·(sc·lx, sc·ly, 0), lx = (q.x − ax)·qw
    let r = m3_mul(
        &m3_mul(&rot_z(rot[2].to_radians()), &rot_x(rot[0].to_radians())),
        &rot_y(rot[1].to_radians()),
    );
    let col = |j: usize| -> V3 { [r[0][j], r[1][j], r[2][j]] };
    let (c0, c1) = (col(0), col(1));
    let k1 = sc * quad.0;
    let k2 = sc * quad.1;
    let e1: V3 = [c0[0] * k1, c0[1] * k1, c0[2] * k1];
    let e2: V3 = [c1[0] * k2, c1[1] * k2, c1[2] * k2];
    let e0: V3 = [
        pos[0] * s - anchor[0] * e1[0] - anchor[1] * e2[0],
        pos[1] * s - anchor[0] * e1[1] - anchor[1] * e2[1],
        pos[2] * s - anchor[0] * e1[2] - anchor[1] * e2[2],
    ];

    // камера: авто-дистанция «50 мм» — слой в z=0 рендерится 1:1
    let fov = cam.fov_deg.clamp(1.0, 179.0).to_radians();
    let f = 1.0 / (fov * 0.5).tan();
    let auto_z = -(oh * 0.5) * f;
    let eye: V3 = [
        cam.pos_x * s,
        cam.pos_y * s,
        if cam.pos_z == 0.0 { auto_z } else { cam.pos_z * s },
    ];
    // взгляд: на target (two-node) либо вдоль +Z; up экрана в мире — −Y
    let dir: V3 = match cam.target {
        Some((tx, ty, tz)) => {
            let d = [tx * s - eye[0], ty * s - eye[1], tz * s - eye[2]];
            let len2 = dot(&d, &d);
            if len2 < 1e-9 {
                [0.0, 0.0, 1.0]
            } else {
                let l = len2.sqrt();
                [d[0] / l, d[1] / l, d[2] / l]
            }
        }
        None => [0.0, 0.0, 1.0],
    };
    let up: V3 = [0.0, -1.0, 0.0];
    let rt = cross(&dir, &up);
    let rl2 = dot(&rt, &rt);
    let right: V3 = if rl2 > 1e-24 {
        let l = rl2.sqrt();
        [rt[0] / l, rt[1] / l, rt[2] / l]
    } else {
        [1.0, 0.0, 0.0]
    };
    let cdown = cross(&dir, &right);
    // basis-строки: right / cdown / dir; orientation-доворот ПОСЛЕ target
    let basis: M3 = [right, cdown, dir];
    let orient = m3_mul(
        &m3_mul(&rot_z(-cam.rot_z.to_radians()), &rot_x(-cam.rot_x.to_radians())),
        &rot_y(-cam.rot_y.to_radians()),
    );
    let view = m3_mul(&orient, &basis);
    // направления: v = view·e; точка: v0 = view·(e0 − eye)
    let mv = |e: &V3| -> V3 {
        [
            view[0][0] * e[0] + view[0][1] * e[1] + view[0][2] * e[2],
            view[1][0] * e[0] + view[1][1] * e[1] + view[1][2] * e[2],
            view[2][0] * e[0] + view[2][1] * e[1] + view[2][2] * e[2],
        ]
    };
    let v1 = mv(&e1);
    let v2 = mv(&e2);
    let d0: V3 = [e0[0] - eye[0], e0[1] - eye[1], e0[2] - eye[2]];
    let v0 = mv(&d0);

    // проекция: перспектива x = f/aspect·v.x, y = −f·v.y, w = v.z;
    // ortho — аффинная (w точки = 1, направлений = 0)
    let aspect = ow / oh;
    let cl = |v: &V3, point: bool| -> V3 {
        if cam.ortho {
            [2.0 / ow * v[0], -2.0 / oh * v[1], if point { 1.0 } else { 0.0 }]
        } else {
            [f / aspect * v[0], -f * v[1], v[2]]
        }
    };
    let p1 = cl(&v1, false);
    let p2 = cl(&v2, false);
    let p0 = cl(&v0, true);

    // экран: c.x·w = ow/2·(clip.x + w); c.y·w = oh/2·(w − clip.y)
    [
        [
            ow * 0.5 * (p1[0] + p1[2]),
            ow * 0.5 * (p2[0] + p2[2]),
            ow * 0.5 * (p0[0] + p0[2]),
        ],
        [
            oh * 0.5 * (p1[2] - p1[1]),
            oh * 0.5 * (p2[2] - p2[1]),
            oh * 0.5 * (p0[2] - p0[1]),
        ],
        [p1[2], p2[2], p0[2]],
    ]
}

/// Результат обратного прохода координаты.
enum Mapped {
    At(f64, f64),
    /// Вне сетки EventGrid: чёрный непрозрачный холст.
    Black,
    /// Вне квада Pan3d / за камерой: прозрачно.
    Transparent,
}

#[inline]
fn map_pos(prims: &[GeomPrim], ox: f64, oy: f64) -> Mapped {
    let (mut x, mut y) = (ox, oy);
    for p in prims {
        match p {
            GeomPrim::Affine(a) => {
                x = a.ax * x + a.bx;
                y = a.ay * y + a.by;
            }
            GeomPrim::Clamp { x0, x1, y0, y1 } => {
                x = x.clamp(*x0, *x1);
                y = y.clamp(*y0, *y1);
            }
            GeomPrim::Fold { gw, gh, cw, ch, in_w, in_h } => {
                if x < 0.0 || y < 0.0 || x >= *gw || y >= *gh {
                    return Mapped::Black;
                }
                let kx = (x / cw).floor();
                x = (x - kx * cw) / cw * in_w - 0.5;
                let ky = (y / ch).floor();
                y = (y - ky * ch) / ch * in_h - 0.5;
            }
            GeomPrim::Proj { m } => {
                let tx = m[0][0] * x + m[0][1] * y + m[0][2];
                let ty = m[1][0] * x + m[1][1] * y + m[1][2];
                let tz = m[2][0] * x + m[2][1] * y + m[2][2];
                // t.z ≤ 0 — за камерой (клиппинг растеризатора)
                if tz <= 0.0 {
                    return Mapped::Transparent;
                }
                let qx = tx / tz;
                let qy = ty / tz;
                if !(0.0..=1.0).contains(&qx) || !(0.0..=1.0).contains(&qy) {
                    return Mapped::Transparent;
                }
                x = qx;
                y = qy;
            }
        }
    }
    Mapped::At(x, y)
}

/// Финальный тап БЕЗ округления: та же арифметика, что внутренний цикл
/// `bilinear_into_scalar` (f64-аккумулятор, кламп тапов к краям).
/// Дублирование сознательное: скаляр билинейки НЕ рефакторится
/// (kt-fx/CLAUDE.md) — оба текста и есть определение байтов.
#[inline]
fn sample_bilinear_f64(src: &Frame, sx: f64, sy: f64) -> [f64; 4] {
    let (sw, sh) = (src.w as i64, src.h as i64);
    let y = sy.floor() as i64;
    let fy = sy - y as f64;
    let x = sx.floor() as i64;
    let fx = sx - x as f64;
    let mut px = [0f64; 4];
    for (dy, wy) in [(0i64, 1.0 - fy), (1, fy)] {
        for (dx, wx) in [(0i64, 1.0 - fx), (1, fx)] {
            let xi = (x + dx).clamp(0, sw - 1);
            let yi = (y + dy).clamp(0, sh - 1);
            let si = ((yi * sw + xi) * 4) as usize;
            let w = wy * wx;
            for (c, acc) in px.iter_mut().enumerate() {
                *acc += src.rgba[si + c] as f64 * w;
            }
        }
    }
    px
}

/// Обратный проход С ЗАПИСЬЮ координат tap-стадий (Р.45, п.312).
/// Возвращает результат и число ПРИМЕНЁННЫХ примов: при коротком
/// замыкании на приме d жив оп с after_prim ≤ d («noise поверх чёрного
/// холста сетки» работает, «noise до сетки в вырезанной зоне» — нет).
#[inline]
fn map_pos_rec(
    prims: &[GeomPrim],
    taps: &[TapOp],
    ox: f64,
    oy: f64,
    rec: &mut [(f64, f64); MAX_TAP_OPS],
) -> (Mapped, usize) {
    let (mut x, mut y) = (ox, oy);
    // tap'ы лежат в цепочечном порядке с УБЫВАЮЩИМИ after_prim — курсор
    // идёт с конца списка (возрастание after_prim) вместе с проходом
    let mut cursor = taps.len();
    while cursor > 0 && taps[cursor - 1].after_prim == 0 {
        rec[cursor - 1] = (x, y);
        cursor -= 1;
    }
    for (pi, p) in prims.iter().enumerate() {
        match p {
            GeomPrim::Affine(a) => {
                x = a.ax * x + a.bx;
                y = a.ay * y + a.by;
            }
            GeomPrim::Clamp { x0, x1, y0, y1 } => {
                x = x.clamp(*x0, *x1);
                y = y.clamp(*y0, *y1);
            }
            GeomPrim::Fold { gw, gh, cw, ch, in_w, in_h } => {
                if x < 0.0 || y < 0.0 || x >= *gw || y >= *gh {
                    return (Mapped::Black, pi);
                }
                let kx = (x / cw).floor();
                x = (x - kx * cw) / cw * in_w - 0.5;
                let ky = (y / ch).floor();
                y = (y - ky * ch) / ch * in_h - 0.5;
            }
            GeomPrim::Proj { m } => {
                let tx = m[0][0] * x + m[0][1] * y + m[0][2];
                let ty = m[1][0] * x + m[1][1] * y + m[1][2];
                let tz = m[2][0] * x + m[2][1] * y + m[2][2];
                if tz <= 0.0 {
                    return (Mapped::Transparent, pi);
                }
                let qx = tx / tz;
                let qy = ty / tz;
                if !(0.0..=1.0).contains(&qx) || !(0.0..=1.0).contains(&qy) {
                    return (Mapped::Transparent, pi);
                }
                x = qx;
                y = qy;
            }
        }
        while cursor > 0 && taps[cursor - 1].after_prim == pi + 1 {
            rec[cursor - 1] = (x, y);
            cursor -= 1;
        }
    }
    (Mapped::At(x, y), prims.len())
}

/// Greenscreen на значении сабтапа: f64-континуум целочисленной формулы
/// ядра (round происходит один раз в конце пайплайна).
#[inline]
fn tap_gs(v: &mut [f64; 4], key: &[f64; 3], tol: f64, hard: f64, spill: f64, kc: usize) {
    let d = ((v[0] - key[0]).powi(2) + (v[1] - key[1]).powi(2) + (v[2] - key[2]).powi(2)).sqrt();
    if d <= hard {
        v[3] = 0.0;
    } else if d < tol {
        v[3] *= (d - hard) / (tol - hard).max(1e-9);
    }
    if spill > 0.0 && v[3] > 0.0 {
        let other = v[(kc + 1) % 3].max(v[(kc + 2) % 3]);
        if v[kc] > other {
            v[kc] -= (v[kc] - other) * spill;
        }
    }
}

/// Noise на значении сабтапа: офсет от floor координаты СВОЕЙ стадии
/// (зерно прибито к её сетке, решение Р.45); формулы хэша — дословно
/// ядро noise (обе ветки).
#[inline]
#[allow(clippy::too_many_arguments)]
fn tap_noise(
    v: &mut [f64; 4],
    coord: (f64, f64),
    seed: u32,
    k: i64,
    cell: u32,
    color: bool,
    special: bool,
    stage_w: u32,
    stage_h: u32,
) {
    use crate::video::noise_hash as nh;
    let xi = (coord.0.floor() as i64).clamp(0, stage_w as i64 - 1) as u32;
    let yi = (coord.1.floor() as i64).clamp(0, stage_h as i64 - 1) as u32;
    if special {
        let hsh = nh(xi ^ nh(yi ^ seed));
        let n = ((hsh >> 16) & 0x7fff) as i64 - 16384;
        let off = (n * k / 32767) as f64;
        for c in v.iter_mut().take(3) {
            *c += off;
        }
    } else {
        let (gx, gy) = (xi / cell, yi / cell);
        let off_at = |ch: u32| -> f64 {
            let hsh = nh(gx ^ nh(gy ^ nh(ch ^ seed)));
            let n = ((hsh >> 16) & 0x7fff) as i64 - 16384;
            (n * k / 32767) as f64
        };
        if color {
            for (c, val) in v.iter_mut().take(3).enumerate() {
                *val += off_at(c as u32 + 1);
            }
        } else {
            let off = off_at(0);
            for c in v.iter_mut().take(3) {
                *c += off;
            }
        }
    }
}

/// Один сабтап: обратная цепочка целиком + тап (кайма Fold и силуэт Proj
/// анти-алиасятся честно — каждый сабтап решает короткие замыкания сам);
/// затем поглощённые поточечные опы в порядке цепочки (Р.45, п.312).
#[inline]
fn sample_at(p: &GeomProgram, src: &Frame, x: f64, y: f64) -> [f64; 4] {
    if p.tap_ops.is_empty() {
        // быстрый путь — в точности Р.44
        return match map_pos(&p.prims, x, y) {
            Mapped::At(sx, sy) => sample_bilinear_f64(src, sx, sy),
            Mapped::Black => [0.0, 0.0, 0.0, 255.0],
            Mapped::Transparent => [0.0, 0.0, 0.0, 0.0],
        };
    }
    let mut rec = [(0.0f64, 0.0f64); MAX_TAP_OPS];
    let (mapped, applied) = map_pos_rec(&p.prims, &p.tap_ops, x, y, &mut rec);
    let mut v = match mapped {
        Mapped::At(sx, sy) => sample_bilinear_f64(src, sx, sy),
        Mapped::Black => [0.0, 0.0, 0.0, 255.0],
        Mapped::Transparent => [0.0, 0.0, 0.0, 0.0],
    };
    for (ti, op) in p.tap_ops.iter().enumerate() {
        if op.after_prim > applied {
            continue; // стадия опа отрезана коротким замыканием
        }
        match op.kind {
            TapKind::Gs { key, tol, hard, spill, kc } => tap_gs(&mut v, &key, tol, hard, spill, kc),
            TapKind::Noise { seed, k, cell, color, special, stage_w, stage_h } => {
                tap_noise(&mut v, rec[ti], seed, k, cell, color, special, stage_w, stage_h)
            }
        }
    }
    v
}

/// Координатная проба (без тапа) для по-пиксельного якобиана Proj-серий.
#[inline]
fn coords_at(p: &GeomProgram, x: f64, y: f64) -> Option<(f64, f64)> {
    match map_pos(&p.prims, x, y) {
        Mapped::At(sx, sy) => Some((sx, sy)),
        _ => None,
    }
}

/// Тапы по-пиксельно: конечные разности (координатные пробы). Мёртвая
/// прямая проба (сосед за силуэтом/краем) заменяется ОБРАТНОЙ — иначе
/// последний живой столбец тождественной карты получал бы лишние тапы и
/// терял байт-точность (t298). Обе пробы мертвы или мёртв центр — один
/// тап: цвет там всё равно решают короткие замыкания сабтапов.
#[inline]
fn probe_taps(p: &GeomProgram, ox: f64, oy: f64) -> (u32, u32) {
    let c0 = coords_at(p, ox, oy);
    let axis = |dx: f64, dy: f64| -> f64 {
        let Some(a) = c0 else { return 1.0 };
        let dist = |b: (f64, f64)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
        if let Some(b) = coords_at(p, ox + dx, oy + dy) {
            return dist(b);
        }
        if let Some(b) = coords_at(p, ox - dx, oy - dy) {
            return dist(b);
        }
        1.0
    };
    (ss_taps(axis(1.0, 0.0)), ss_taps(axis(0.0, 1.0)))
}

/// CPU-исполнитель слитой программы: скалярный f64-эталон (инвариант №11).
/// SIMD у слитого прохода в этом раунде НЕТ (замер и порог 1.3× — отдельная
/// работа, kt-fx/CLAUDE.md). Один тап (нет минификации) — прежние байты;
/// суперсемплинг — box-среднее сабтапов без промежуточных округлений,
/// round один раз (п.309).
pub fn exec_geom_cpu(p: &GeomProgram, src: &Frame, scratch: &mut FxScratch) -> Frame {
    let (ow, oh) = (p.out_w, p.out_h);
    let mut out = scratch.take((ow * oh * 4) as usize);
    for oy in 0..oh {
        for ox in 0..ow {
            let (nx, ny) = match p.ss {
                SsSpec::Const { nx, ny } => (nx, ny),
                SsSpec::PerPixel => probe_taps(p, ox as f64, oy as f64),
            };
            let px = if nx == 1 && ny == 1 {
                sample_at(p, src, ox as f64, oy as f64)
            } else {
                let mut acc = [0f64; 4];
                for j in 0..ny {
                    let sy = oy as f64 - 0.5 + (j as f64 + 0.5) / ny as f64;
                    for i in 0..nx {
                        let sx = ox as f64 - 0.5 + (i as f64 + 0.5) / nx as f64;
                        let s = sample_at(p, src, sx, sy);
                        for (c, a) in acc.iter_mut().enumerate() {
                            *a += s[c];
                        }
                    }
                }
                let n = (nx * ny) as f64;
                [acc[0] / n, acc[1] / n, acc[2] / n, acc[3] / n]
            };
            let oi = ((oy * ow + ox) * 4) as usize;
            for c in 0..4 {
                out[oi + c] = px[c].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let has_alpha = if p.has_alpha_known { p.has_alpha } else { src.has_alpha };
    Frame { w: ow, h: oh, rgba: out, has_alpha }
}

// Исполнение гео-рана живёт в исполнителях плана (video.rs / kt-gpu):
// решение legacy/fused уже принято `plan_chain` (Р.45, п.311).
