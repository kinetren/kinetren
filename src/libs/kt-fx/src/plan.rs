//! План исполнения FX-цепочки (Р.45, пп.311-314): решения о сегментации,
//! legacy/fused, поглощении поточечных и K×-разрешении барьеров
//! принимаются ЗДЕСЬ ровно один раз — CPU (`apply_video_chain_pooled` в
//! video.rs) и GPU (`chain_ops` в kt-gpu) исполняют ГОТОВЫЙ план. Это
//! инвариант №7 на уровне цепочки: расхождение сегментации = расхождение
//! картинок.
//!
//! K×-барьеры (п.314): нелокальный эффект между сжимающей гео-серией и
//! зумящим хвостом исполняется в K× разрешении — деталь, стиснутая
//! сеткой, доносится сквозь барьер. K = ss_taps(min(нетто-зум хвоста,
//! нетто-сжатие до барьера)): второй множитель — «сколько деталей реально
//! стиснуто»; без сжатия выше K=1 — взять детали неоткуда (их источник,
//! «декод крупнее Quality», — отдельный заход). Потолок — MAX_INTER_DIM
//! (ПРОЕКТ-константа, не лимит устройства: иначе экспорт зависел бы от
//! машины). Гео-раны рендерятся сразу в K×-цель (конъюгация в geom.rs) —
//! ресемплов между сегментами нет вовсе.
//!
//! Сила слоя: GPU передаёт снапшот-точки (`from`-индексы LayerMix) в
//! `stops` и получает маркеры Snapshot/Mix; CPU режет цепочку по стопам
//! уровнем выше (`apply_chain_with_mixes`) и планирует слайсы — план
//! слайса обязан совпадать с куском плана целого (t306). Стопы обнуляют
//! оба аккумулятора K (смешение всегда на 1×).

use crate::geom::{
    Cam3d, GeomExec, MAX_GEO_STEPS, MAX_PRIM_SLOTS, MAX_PRIMS, MAX_TAP_OPS, build_geom_exec,
    is_geo, is_pointwise, ss_taps,
};
use kt_model::FxInstance;

/// Потолок стороны K×-буфера, px. ПРОЕКТ-константа (п.314): превью
/// 640×360 получает полный K=4, экспорт 1080p — до K=2, 4K — без K×.
/// Поднимается одной константой; устройства со слабым max_dim уводят
/// такие планы в CPU целиком (гард в kt-gpu resolve_chain).
pub const MAX_INTER_DIM: u32 = 4096;

/// Один оп плана. Аудио-FX в план не попадают вовсе (прозрачны и для
/// сегментации, и для исполнения).
pub enum PlanOp {
    /// Гео-ран `fx[i..j)` (внутри — гео, прозрачные аудио и поглощённые
    /// ВНУТРЕННИЕ поточечные, п.312) с уже принятым решением исполнения.
    Run { i: usize, j: usize, exec: GeomExec },
    /// Барьерный эффект `fx[idx]` в разрешении `res_scale`× (п.314).
    Barrier { idx: usize, res_scale: u32 },
    /// Снимок сухой стороны силы слоя (позиция из `stops`).
    Snapshot,
    /// Слот силы `fx[idx]` (LayerMix): исполнитель смешивает снимок с
    /// текущим состоянием (GPU) либо пропускает (CPU — сила уровнем выше).
    Mix { idx: usize },
}

pub struct ChainPlan {
    pub ops: Vec<PlanOp>,
}

/// Конец ОКНА гео-рана, начатого на гео-эффекте `i`: гео-шаги (не больше
/// `max_geo`), прозрачные аудио и ВНУТРЕННИЕ поточечные; хвостовые не-гео
/// отрезаются (ведущие невозможны по построению — окно начинается с гео).
/// Барьер, слот силы, стоп-индекс или потолок шагов завершают окно.
fn run_window_end(fx: &[FxInstance], i: usize, stops: &[usize], max_geo: usize) -> usize {
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
            if geo > max_geo {
                break;
            }
            j += 1;
            end = j; // окно всегда заканчивается гео-шагом
        } else if f.is_audio() || is_pointwise(f) {
            j += 1; // войдёт в окно, только если дальше ещё гео
        } else {
            break;
        }
    }
    end
}

/// Лупа Legacy-рана (одиночный Pan/CamMotion/Mirror) для аккумуляторов
/// K×: рамка pan_frame в долях выхода; Mirror — тождество.
fn legacy_lens(
    step: &FxInstance,
    lw: u32,
    lh: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
) -> (f64, f64) {
    let (mow, moh) = (out_w.max(2) as f64, out_h.max(2) as f64);
    match step {
        FxInstance::Pan { mode, pan_x, pan_y, zoom } => {
            let (_, _, cw, ch) =
                crate::video::pan_frame(lw, lh, out_w, out_h, *mode, *pan_x, *pan_y, *zoom);
            (cw / mow, ch / moh)
        }
        FxInstance::CamMotion { mode, ease, time_scale, keyframes } => {
            let (px, py, z) = crate::video::cam_at(keyframes, u, *ease, *time_scale);
            let (_, _, cw, ch) = crate::video::pan_frame(lw, lh, out_w, out_h, *mode, px, py, z);
            (cw / mow, ch / moh)
        }
        _ => (1.0, 1.0),
    }
}

enum RawKind {
    Run { i: usize, j: usize, exec: GeomExec, lens: (f64, f64) },
    Barrier { idx: usize },
    Snapshot,
    Mix { idx: usize },
}

/// Фаза A: сегментация цепочки и 1×-программы (пп.311-312). `in_w/in_h` —
/// ЛОГИЧЕСКИЕ размеры входа (физические поделены на ступень in_scale).
#[allow(clippy::too_many_arguments)]
fn segment(
    fx: &[FxInstance],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
    burst_index: i64,
    stops: &[usize],
) -> Vec<RawKind> {
    let (mow, moh) = (out_w.max(2), out_h.max(2));
    let (mut w, mut h) = (in_w, in_h);
    let mut raw: Vec<RawKind> = Vec::new();
    let mut idx = 0usize;
    while idx < fx.len() {
        if stops.contains(&idx) {
            raw.push(RawKind::Snapshot);
        }
        let f = &fx[idx];
        if is_geo(f) {
            // Бюджеты юниформы GPU (примы/слоты/tap-опы) известны только
            // после построения программы — строим и, если не влезла,
            // ужимаем окно по числу гео-шагов (в точке разреза качество =
            // прежнее пошаговое). Программы крошечные, перестройка дёшева.
            let mut max_geo = MAX_GEO_STEPS;
            let (j, exec) = loop {
                let j = run_window_end(fx, idx, stops, max_geo);
                let steps: Vec<&FxInstance> = fx[idx..j]
                    .iter()
                    .filter(|g| is_geo(g) || is_pointwise(g))
                    .collect();
                let exec =
                    build_geom_exec(&steps, w, h, out_w, out_h, u, cam3d, burst_index, 1, 1);
                let fits = match &exec {
                    GeomExec::Legacy => true,
                    GeomExec::Fused(p) => {
                        p.prims.len() <= MAX_PRIMS
                            && p.prim_slots() <= MAX_PRIM_SLOTS
                            && p.tap_ops.len() <= MAX_TAP_OPS
                    }
                };
                if fits || max_geo == 1 {
                    debug_assert!(fits, "geom program does not fit even at one step");
                    break (j, exec);
                }
                max_geo -= 1;
            };
            let lens = match &exec {
                GeomExec::Fused(p) => {
                    w = p.out_w;
                    h = p.out_h;
                    p.lens
                }
                GeomExec::Legacy => {
                    let step =
                        fx[idx..j].iter().find(|g| is_geo(g)).expect("legacy run has a geo step");
                    let l = legacy_lens(step, w, h, out_w, out_h, u);
                    if !matches!(step, FxInstance::Mirror { .. }) {
                        w = mow;
                        h = moh;
                    }
                    l
                }
            };
            raw.push(RawKind::Run { i: idx, j, exec, lens });
            idx = j;
            continue;
        }
        if f.is_layer_mix() {
            raw.push(RawKind::Mix { idx });
        } else if !f.is_audio() {
            // барьер: нелокальный эффект либо ведущий/замыкающий поточечный
            raw.push(RawKind::Barrier { idx });
        } // аудио-FX прозрачны
        idx += 1;
    }
    raw
}

/// Фаза B: назад — нетто-зум хвоста на каждый барьер. Второй результат —
/// зум ПРЕФИКСА (значение аккумулятора в позиции 0): «во сколько раз
/// цепочке нужен вход крупнее», источник ступени декода (Р.46).
fn tail_zooms(raw: &[RawKind]) -> (Vec<(f64, f64)>, (f64, f64)) {
    let mut tail: Vec<(f64, f64)> = vec![(1.0, 1.0); raw.len()];
    let mut tm = (1.0f64, 1.0f64);
    for (ri, r) in raw.iter().enumerate().rev() {
        match r {
            RawKind::Run { lens, .. } => {
                tm.0 *= 1.0 / lens.0.max(1e-9);
                tm.1 *= 1.0 / lens.1.max(1e-9);
            }
            RawKind::Barrier { .. } => tail[ri] = tm,
            RawKind::Snapshot | RawKind::Mix { .. } => tm = (1.0, 1.0),
        }
    }
    (tail, tm)
}

/// Нетто-зум ПРЕФИКСА цепочки (до первого стопа силы слоя) на логическом
/// размере `out_w×out_h` — «во сколько раз цепочке нужен вход крупнее»
/// (Р.46, п.318: ступень декода клипа). Чистая функция; стопы выводятся из
/// цепочки так же, как их выводит GPU-путь. Возврат ≥ 1.
pub fn chain_head_zoom(
    fx: &[FxInstance],
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
) -> f64 {
    let stops: Vec<usize> = fx
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f {
            FxInstance::LayerMix { from, .. } => Some((*from as usize).min(i)),
            _ => None,
        })
        .collect();
    let raw = segment(fx, out_w, out_h, out_w, out_h, u, cam3d, 0, &stops);
    let (_, head) = tail_zooms(&raw);
    head.0.max(head.1).max(1.0)
}

/// Построить план цепочки для входного кадра `in_w×in_h` и выхода
/// `out_w×out_h` (Quality). Детерминированная чистая функция — оба
/// бэкенда обязаны получать ОДИН план. `burst_index` — абсолютный тик
/// (seed поглощённого Noise, как у ядра).
///
/// `in_scale` (Р.46, п.318) — ступень декода: физический кадр в in_scale
/// раз крупнее логического размера цепочки. Вся арифметика шагов живёт в
/// логике (конъюгация п.314), а детали, стиснутые в крупном входе,
/// доносятся до барьеров и хвоста. При in_scale = 1 план — прежний.
#[allow(clippy::too_many_arguments)]
pub fn plan_chain(
    fx: &[FxInstance],
    in_w: u32,
    in_h: u32,
    in_scale: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
    burst_index: i64,
    stops: &[usize],
) -> ChainPlan {
    let (mow, moh) = (out_w.max(2), out_h.max(2));
    let s = in_scale.max(1);
    debug_assert!(
        in_w.is_multiple_of(s) && in_h.is_multiple_of(s),
        "physical input is in_scale x logical"
    );
    // стоп в нуле обнуляет зум префикса ⇒ ступень там всегда 1
    debug_assert!(s == 1 || !stops.contains(&0), "leading stop forbids a decode stage");

    // --- A: сегментация и 1×-программы (пп.311-312) ---
    let raw = segment(fx, in_w / s, in_h / s, out_w, out_h, u, cam3d, burst_index, stops);

    // --- B: назад — нетто-зум хвоста на каждый барьер ---
    let (tail, _) = tail_zooms(&raw);

    // --- C: вперёд — нетто-сжатие; K = квант(min(зум хвоста, сжатие)) ---
    let k_cap = (MAX_INTER_DIM / mow.max(moh)).max(1);
    debug_assert!(s <= k_cap, "decode stage above the intermediate ceiling");
    let mut kmap: Vec<u32> = vec![1; raw.len()];
    // ступень входа — уже стиснутая деталь: она и есть стартовое «сжатие»
    let mut sq = (s as f64, s as f64);
    let mut resampled = false;
    for (ri, r) in raw.iter().enumerate() {
        match r {
            RawKind::Run { lens, .. } => {
                resampled = true;
                sq.0 *= lens.0;
                sq.1 *= lens.1;
            }
            RawKind::Barrier { .. } => {
                if resampled {
                    let benefit = tail[ri].0.max(tail[ri].1).min(sq.0.max(sq.1));
                    kmap[ri] = ss_taps(benefit).min(k_cap);
                } else {
                    // ВЕДУЩИЙ барьер (ресемпла до него не было): кадр под
                    // ним физически S×, вид обязан считаться в этом
                    // разрешении — benefit-формула здесь не голосует
                    kmap[ri] = s;
                }
            }
            RawKind::Snapshot | RawKind::Mix { .. } => {
                resampled = true; // стопы всегда на 1× — ведущих барьеров дальше нет
                sq = (1.0, 1.0);
            }
        }
    }

    // --- D: сборка. Вход 1× и все K == 1 — готовые 1×-программы (байты
    // пп.311-313); иначе пересборка ранов с конъюгацией (k_in, k_out).
    if s == 1 && kmap.iter().all(|&k| k == 1) {
        let ops = raw
            .into_iter()
            .map(|r| match r {
                RawKind::Run { i, j, exec, .. } => PlanOp::Run { i, j, exec },
                RawKind::Barrier { idx } => PlanOp::Barrier { idx, res_scale: 1 },
                RawKind::Snapshot => PlanOp::Snapshot,
                RawKind::Mix { idx } => PlanOp::Mix { idx },
            })
            .collect();
        return ChainPlan { ops };
    }
    let mut ops = Vec::with_capacity(raw.len());
    let (mut w, mut h) = (in_w, in_h);
    let mut cur_k = s;
    for ri in 0..raw.len() {
        match &raw[ri] {
            RawKind::Run { i, j, .. } => {
                // цель рана: K следующего барьера; разрез бюджета, стоп и
                // конец цепочки — 1×
                let k_out = match raw.get(ri + 1) {
                    Some(RawKind::Barrier { .. }) => kmap[ri + 1],
                    _ => 1,
                };
                let steps: Vec<&FxInstance> = fx[*i..*j]
                    .iter()
                    .filter(|g| is_geo(g) || is_pointwise(g))
                    .collect();
                let exec = build_geom_exec(
                    &steps, w, h, out_w, out_h, u, cam3d, burst_index, cur_k, k_out,
                );
                match &exec {
                    GeomExec::Fused(p) => {
                        w = p.out_w;
                        h = p.out_h;
                    }
                    GeomExec::Legacy => {
                        // Legacy со СМЕНОЙ масштаба невозможен (форс
                        // Fused); здесь — зеркало на равных краях
                        debug_assert!(cur_k == k_out, "legacy run must not change scale");
                    }
                }
                cur_k = k_out;
                ops.push(PlanOp::Run { i: *i, j: *j, exec });
            }
            RawKind::Barrier { idx } => {
                debug_assert_eq!(cur_k, kmap[ri], "barrier not at its segment resolution");
                ops.push(PlanOp::Barrier { idx: *idx, res_scale: kmap[ri] });
            }
            RawKind::Snapshot => {
                debug_assert_eq!(cur_k, 1, "layer-mix snapshot must be 1x");
                ops.push(PlanOp::Snapshot);
            }
            RawKind::Mix { idx } => {
                debug_assert_eq!(cur_k, 1, "layer-mix blend must be 1x");
                ops.push(PlanOp::Mix { idx: *idx });
            }
        }
    }
    ChainPlan { ops }
}
