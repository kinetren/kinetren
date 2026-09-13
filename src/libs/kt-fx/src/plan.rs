//! План исполнения FX-цепочки (Р.45, пп.311-312): решения о сегментации,
//! legacy/fused, поглощении поточечных и (с п.314) разрешении барьеров
//! принимаются ЗДЕСЬ ровно один раз — CPU (`apply_video_chain_pooled` в
//! video.rs) и GPU (`chain_ops` в kt-gpu) исполняют ГОТОВЫЙ план. Это
//! инвариант №7 на уровне цепочки: расхождение сегментации = расхождение
//! картинок.
//!
//! Сила слоя: GPU передаёт снапшот-точки (`from`-индексы LayerMix) в
//! `stops` и получает маркеры Snapshot/Mix; CPU режет цепочку по стопам
//! уровнем выше (`apply_chain_with_mixes`) и планирует слайсы — план
//! слайса обязан совпадать с куском плана целого (t306).

use crate::geom::{
    Cam3d, GeomExec, MAX_GEO_STEPS, MAX_PRIM_SLOTS, MAX_PRIMS, MAX_TAP_OPS, build_geom_exec,
    is_geo, is_pointwise,
};
use kt_model::FxInstance;

/// Один оп плана. Аудио-FX в план не попадают вовсе (прозрачны и для
/// сегментации, и для исполнения).
pub enum PlanOp {
    /// Гео-ран `fx[i..j)` (внутри — гео, прозрачные аудио и поглощённые
    /// ВНУТРЕННИЕ поточечные, п.312) с уже принятым решением исполнения.
    Run { i: usize, j: usize, exec: GeomExec },
    /// Барьерный эффект `fx[idx]`; `res_scale` — разрешение исполнения
    /// (Р.45 п.314: K× при зумящем хвосте; пока всегда 1).
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

/// Построить план цепочки для входного кадра `in_w×in_h` и выхода
/// `out_w×out_h` (Quality). Детерминированная чистая функция — оба
/// бэкенда обязаны получать ОДИН план. `burst_index` — абсолютный тик
/// (seed поглощённого Noise, как у ядра).
#[allow(clippy::too_many_arguments)]
pub fn plan_chain(
    fx: &[FxInstance],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
    burst_index: i64,
    stops: &[usize],
) -> ChainPlan {
    let (mow, moh) = (out_w.max(2), out_h.max(2));
    let (mut w, mut h) = (in_w, in_h);
    let mut ops = Vec::new();
    let mut idx = 0usize;
    while idx < fx.len() {
        if stops.contains(&idx) {
            ops.push(PlanOp::Snapshot);
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
                let exec = build_geom_exec(&steps, w, h, out_w, out_h, u, cam3d, burst_index);
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
            match &exec {
                GeomExec::Fused(prog) => {
                    w = prog.out_w;
                    h = prog.out_h;
                }
                // Legacy: одиночный Pan/CamMotion ресемплит в выход,
                // одиночное зеркало размеров не меняет
                GeomExec::Legacy => {
                    let step =
                        fx[idx..j].iter().find(|g| is_geo(g)).expect("legacy run has a geo step");
                    if !matches!(step, FxInstance::Mirror { .. }) {
                        w = mow;
                        h = moh;
                    }
                }
            }
            ops.push(PlanOp::Run { i: idx, j, exec });
            idx = j;
            continue;
        }
        if f.is_layer_mix() {
            ops.push(PlanOp::Mix { idx });
        } else if !f.is_audio() {
            // барьер: нелокальный эффект либо ведущий/замыкающий поточечный
            ops.push(PlanOp::Barrier { idx, res_scale: 1 });
        } // аудио-FX прозрачны
        idx += 1;
    }
    ChainPlan { ops }
}
