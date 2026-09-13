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

use crate::video::{Frame, FxScratch, VideoFxCtx};
use kt_model::FxInstance;

/// Потолок гео-шагов в одном слитом проходе: длиннее — серия режется на
/// два прохода по границе шага (в точке разреза качество = сегодняшнее).
/// Ограничен ёмкостью юниформы GPU-пасса: худший случай — 6 EventGrid =
/// 6 Fold (по 2 vec4) + 7 Affine = 13 примитивов ≤ MAX_PRIMS.
pub const MAX_GEO_STEPS: usize = 6;
/// Потолок ЛОГИЧЕСКИХ примитивов программы (kinds пакуются 4 битами в два
/// u32 заголовка юниформы — максимум 16).
pub const MAX_PRIMS: usize = 16;

/// Геометрический ли эффект (обратимое отображение «выходной пиксель →
/// точка входа»). Pan3d станет гео-шагом в К4 (пока — свой пасс на GPU).
pub fn is_geo(f: &FxInstance) -> bool {
    matches!(
        f,
        FxInstance::Pan { .. }
            | FxInstance::CamMotion { .. }
            | FxInstance::Mirror { .. }
            | FxInstance::EventGrid { .. }
    )
}

/// Конец гео-рана, начатого на гео-эффекте `i`: индекс сразу ЗА последним
/// гео-шагом рана. Аудио-FX прозрачны ВНУТРИ рана (не рвут серию — GPU их
/// тоже пропускает); любой другой эффект, стоп-индекс (снапшоты силы слоя)
/// или потолок MAX_GEO_STEPS завершают ран. Оба бэкенда обязаны звать
/// ЭТУ функцию — расхождение сегментации = расхождение картинок.
pub fn geo_run_end(fx: &[FxInstance], i: usize, stops: &[usize]) -> usize {
    debug_assert!(is_geo(&fx[i]));
    let mut j = i;
    let mut end = i;
    let mut geo = 0usize;
    while j < fx.len() {
        if j > i && stops.contains(&j) {
            break;
        }
        let f = &fx[j];
        if is_geo(f) {
            geo += 1;
            if geo > MAX_GEO_STEPS {
                break;
            }
            j += 1;
            end = j;
        } else if f.is_audio() {
            j += 1; // прозрачен; войдёт в ран, только если дальше ещё гео
        } else {
            break;
        }
    }
    end
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
}

/// Сентинел отключённой оси Clamp (влезает в f32 юниформы без inf).
pub const CLAMP_OFF: f64 = 1.0e30;

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
pub fn build_geom_exec(
    steps: &[&FxInstance],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
) -> GeomExec {
    debug_assert!(!steps.is_empty() && steps.len() <= MAX_GEO_STEPS);
    if steps.len() == 1 && !matches!(steps[0], FxInstance::EventGrid { .. }) {
        return GeomExec::Legacy;
    }
    GeomExec::Fused(build_program(steps, in_w, in_h, out_w, out_h, u))
}

/// Прямые параметры одного шага (собираются проходом вперёд — размеры
/// каждого шага зависят от предыдущих).
enum Step {
    /// Pan/CamMotion: рамка pan_frame в координатах входа шага.
    Frame { x0: f64, y0: f64, cw: f64, ch: f64, iw: u32, ih: u32 },
    Mirror { h: bool, v: bool, iw: u32, ih: u32 },
    Grid { offx: u32, offy: u32, gw: u32, gh: u32, cw: u32, ch: u32, iw: u32, ih: u32 },
}

fn build_program(
    steps: &[&FxInstance],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
) -> GeomProgram {
    let (mow, moh) = (out_w.max(2), out_h.max(2));
    // --- вперёд: размеры и прямые параметры каждого шага ---
    let (mut w, mut h) = (in_w, in_h);
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
            _ => debug_assert!(false, "non-geo step in build_program"),
        }
    }
    // --- назад: обратные карты, соседние аффинные ПЕРЕМНОЖЕНЫ в f64 ---
    let mut prims: Vec<GeomPrim> = Vec::new();
    let mut pending = Aff::IDENTITY;
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
        }
    }
    flush(&mut pending, &mut prims);
    debug_assert!(prims.len() <= MAX_PRIMS, "geom program exceeds MAX_PRIMS: {}", prims.len());
    GeomProgram { prims, in_w, in_h, out_w: mow, out_h: moh, has_alpha_known, has_alpha }
}

/// Результат обратного прохода координаты.
enum Mapped {
    At(f64, f64),
    Black,
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
        }
    }
    Mapped::At(x, y)
}

/// Финальный тап: та же арифметика, что внутренний цикл
/// `bilinear_into_scalar` (f64-аккумулятор, кламп тапов к краям, round
/// half-away). Дублирование сознательное: скаляр билинейки НЕ рефакторится
/// (kt-fx/CLAUDE.md) — оба текста и есть определение байтов.
#[inline]
fn sample_bilinear(src: &Frame, sx: f64, sy: f64) -> [u8; 4] {
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
    let mut out = [0u8; 4];
    for (c, o) in out.iter_mut().enumerate() {
        *o = px[c].round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// CPU-исполнитель слитой программы: скалярный f64-эталон (инвариант №11).
/// SIMD у слитого прохода в этом раунде НЕТ (замер и порог 1.3× — отдельная
/// работа, kt-fx/CLAUDE.md).
pub fn exec_geom_cpu(p: &GeomProgram, src: &Frame, scratch: &mut FxScratch) -> Frame {
    let (ow, oh) = (p.out_w, p.out_h);
    let mut out = scratch.take((ow * oh * 4) as usize);
    for oy in 0..oh {
        for ox in 0..ow {
            let px = match map_pos(&p.prims, ox as f64, oy as f64) {
                Mapped::At(sx, sy) => sample_bilinear(src, sx, sy),
                Mapped::Black => [0, 0, 0, 255],
            };
            let oi = ((oy * ow + ox) * 4) as usize;
            out[oi..oi + 4].copy_from_slice(&px);
        }
    }
    let has_alpha = if p.has_alpha_known { p.has_alpha } else { src.has_alpha };
    Frame { w: ow, h: oh, rgba: out, has_alpha }
}

/// Применить гео-ран `fx[i..j)` на CPU: решение (legacy/fused) — общее.
pub fn apply_geo_run(
    run: &[FxInstance],
    frame: Frame,
    ctx: &VideoFxCtx,
    scratch: &mut FxScratch,
) -> Frame {
    let steps: Vec<&FxInstance> = run.iter().filter(|f| is_geo(f)).collect();
    match build_geom_exec(&steps, frame.w, frame.h, ctx.out_w, ctx.out_h, ctx.u) {
        GeomExec::Legacy => crate::video::apply_legacy_geo(steps[0], frame, ctx, scratch),
        GeomExec::Fused(prog) => {
            let nf = exec_geom_cpu(&prog, &frame, scratch);
            scratch.put(frame.rgba);
            nf
        }
    }
}
