//! План исполнения FX-цепочки (Р.45, п.311): решения о сегментации,
//! legacy/fused и (с п.314) разрешении барьеров принимаются ЗДЕСЬ ровно
//! один раз — CPU (`apply_video_chain_pooled` в video.rs) и GPU
//! (`chain_ops` в kt-gpu) исполняют ГОТОВЫЙ план. Это инвариант №7 на
//! уровне цепочки: расхождение сегментации = расхождение картинок.
//!
//! Сила слоя: GPU передаёт снапшот-точки (`from`-индексы LayerMix) в
//! `stops` и получает маркеры Snapshot/Mix; CPU режет цепочку по стопам
//! уровнем выше (`apply_chain_with_mixes`) и планирует слайсы — план
//! слайса обязан совпадать с куском плана целого (t306).

use crate::geom::{Cam3d, GeomExec, build_geom_exec, geo_run_end, is_geo};
use kt_model::FxInstance;

/// Один оп плана. Аудио-FX в план не попадают вовсе (прозрачны и для
/// сегментации, и для исполнения).
pub enum PlanOp {
    /// Гео-ран `fx[i..j)` (внутри могут лежать прозрачные аудио) с уже
    /// принятым решением исполнения.
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

/// Построить план цепочки для входного кадра `in_w×in_h` и выхода
/// `out_w×out_h` (Quality). Детерминированная чистая функция — оба
/// бэкенда обязаны получать ОДИН план.
#[allow(clippy::too_many_arguments)]
pub fn plan_chain(
    fx: &[FxInstance],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
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
            let j = geo_run_end(fx, idx, stops);
            let steps: Vec<&FxInstance> = fx[idx..j].iter().filter(|f| is_geo(f)).collect();
            let exec = build_geom_exec(&steps, w, h, out_w, out_h, u, cam3d);
            match &exec {
                GeomExec::Fused(prog) => {
                    w = prog.out_w;
                    h = prog.out_h;
                }
                // Legacy: одиночный Pan/CamMotion ресемплит в выход,
                // одиночное зеркало размеров не меняет
                GeomExec::Legacy => {
                    if !matches!(steps[0], FxInstance::Mirror { .. }) {
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
            ops.push(PlanOp::Barrier { idx, res_scale: 1 });
        } // аудио-FX прозрачны
        idx += 1;
    }
    ChainPlan { ops }
}
