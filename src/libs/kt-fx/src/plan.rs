//! Execution plan of the FX chain (R.45, items 311-314): the decisions on segmentation,
//! legacy/fused, absorption of per-pixel ops and K× barrier resolution are taken HERE,
//! exactly once — the CPU (`apply_video_chain_pooled` in video.rs) and the GPU
//! (`chain_ops` in kt-gpu) execute the READY plan. This is
//! invariant No. 7 at the chain level: a divergence of segmentation = a divergence of
//! pictures.
//!
//! K× barriers (item 314): a non-local effect between a compressing geo series and
//! a zooming tail is executed at K× resolution — detail squeezed by the grid
//! is carried through the barrier. K = ss_taps(min(net zoom of the tail,
//! net compression up to the barrier)): the second factor is “how much detail is
//! actually squeezed”; without compression above K=1 there is nowhere to take the detail from (its source,
//! “decode larger than Quality”, is a separate pass). The ceiling is MAX_INTER_DIM
//! (a PROJECT constant, not a device limit: otherwise export would depend on the
//! machine). Geo runs are rendered straight into the K×-target (conjugation in geom.rs) —
//! there are no resamples between segments at all.
//!
//! Layer strength: the GPU passes the snapshot points (`from` indices of LayerMix) into
//! `stops` and gets back Snapshot/Mix markers; the CPU cuts the chain at the stops
//! one level up (`apply_chain_with_mixes`) and plans the slices — the plan of a slice
//! must match the piece of the whole plan (t306). The stops zero both K accumulators
//! (the mixing is always at 1×).

use crate::conform::LeadCtx;
use crate::geom::{
    Cam3d, GeomExec, MAX_GEO_STEPS, MAX_PRIM_SLOTS, MAX_PRIMS, MAX_TAP_OPS, build_geom_exec_in,
    is_geo, is_geo_active, is_pointwise, ss_taps,
};
use kt_model::FxInstance;

/// Ceiling of the side of the K× buffer, px. PROJECT constant (item 314): the preview
/// 640×360 gets full K=4, export 1080p — up to K=2, 4K — no K×.
/// It is raised with one constant; devices with a weak max_dim divert such
/// plans to the CPU entirely (guard in kt-gpu resolve_chain).
pub const MAX_INTER_DIM: u32 = 4096;

/// One op of the plan. Audio FX do not enter the plan at all (they are transparent
/// both for segmentation and for execution).
pub enum PlanOp {
    /// Geo run `fx[i..j)` (inside — geo, transparent audio and absorbed
    /// INTERNAL per-pixel ops, item 312) with the execution decision already taken.
    Run { i: usize, j: usize, exec: GeomExec },
    /// Barrier effect `fx[idx]` at the `res_scale`× resolution (item 314).
    Barrier { idx: usize, res_scale: u32 },
    /// Snapshot of the dry side of the layer strength (position from `stops`).
    Snapshot,
    /// Strength slot `fx[idx]` (LayerMix): the executor mixes the snapshot with the
    /// current state (GPU) or skips it (CPU — strength is one level up).
    Mix { idx: usize },
}

pub struct ChainPlan {
    pub ops: Vec<PlanOp>,
}

/// The END of the WINDOW of a geo run started at geo effect `i`: geo steps (no more than
/// `max_geo`), transparent audio and INTERNAL per-pixel ops; trailing non-geo ones are
/// cut off (leading ones are impossible by construction — the window starts with geo).
/// A barrier, a strength slot, a stop index or the step ceiling ends the window.
fn run_window_end(
    fx: &[FxInstance],
    i: usize,
    stops: &[usize],
    max_geo: usize,
    lead: Option<LeadCtx>,
    out_w: u32,
    out_h: u32,
) -> usize {
    let mut j = i;
    let mut end = i;
    let mut geo = 0usize;
    while j < fx.len() {
        if j > i && stops.contains(&j) {
            break;
        }
        let f = &fx[j];
        if is_geo_active(f, lead, out_w, out_h) {
            geo += 1;
            if geo > max_geo {
                break;
            }
            j += 1;
            end = j; // the window always ends with a geo step
        } else if f.is_audio() || is_pointwise(f) || is_geo(f) {
            // audio, internal per-pixel ops and the IDENTITY “Geometry” node are
            // transparent: they enter the window only if more geo follows
            j += 1;
        } else {
            break;
        }
    }
    end
}

/// Magnifier of a Legacy run (a single Pan/CamMotion/Mirror) for the
/// K× accumulators: the pan_frame frame in fractions of the output; Mirror is identity.
fn legacy_lens(step: &FxInstance, lw: u32, lh: u32, out_w: u32, out_h: u32, u: f64) -> (f64, f64) {
    let (mow, moh) = (out_w.max(2) as f64, out_h.max(2) as f64);
    match step {
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

/// Phase A: segmentation of the chain and the 1× program (items 311-312). `in_w/in_h` are
/// the LOGICAL input sizes (the physical ones divided by the in_scale step).
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
    lead: Option<LeadCtx>,
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
        if is_geo_active(f, lead, out_w, out_h) {
            // GPU uniform budgets (primes/slots/tap ops) are known only
            // after the program is built — build it and, if it does not fit,
            // shrink the window by the number of geo steps (at the cut point the quality =
            // the former step-by-step). The programs are tiny, the rebuild is cheap.
            let mut max_geo = MAX_GEO_STEPS;
            let (j, exec) = loop {
                let j = run_window_end(fx, idx, stops, max_geo, lead, out_w, out_h);
                let steps: Vec<&FxInstance> = fx[idx..j]
                    .iter()
                    .filter(|g| is_geo_active(g, lead, out_w, out_h) || is_pointwise(g))
                    .collect();
                let exec = build_geom_exec_in(
                    &steps,
                    w,
                    h,
                    out_w,
                    out_h,
                    u,
                    cam3d,
                    burst_index,
                    1,
                    1,
                    lead,
                );
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
                    let step = fx[idx..j]
                        .iter()
                        .find(|g| is_geo_active(g, lead, out_w, out_h))
                        .expect("legacy run has a geo step");
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
        } else if !f.is_audio() && !is_geo(f) {
            // barrier: a non-local effect or a leading/trailing per-pixel one
            raw.push(RawKind::Barrier { idx });
        } // audio FX and the identity “Geometry” node are transparent
        idx += 1;
    }
    raw
}

/// Phase B: backward — the net zoom of the tail at each barrier. The second result is
/// the zoom of the PREFIX (the accumulator value at position 0): “by how many times the
/// chain needs a larger input”, the source of the decode step (R.46).
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

/// Net zoom of the chain PREFIX (up to the first layer-strength stop) at the logical
/// size `out_w×out_h` — “by how many times the chain needs a larger input”
/// (R.46, item 318: the clip's decode step). A pure function; stops are derived from the
/// chain the same way the GPU path derives them. Returns ≥ 1.
pub fn chain_head_zoom(
    fx: &[FxInstance],
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
) -> f64 {
    chain_head_zoom_in(fx, out_w, out_h, u, cam3d, None)
}

/// The same with the context of the leading “Geometry” node (R.62).
pub fn chain_head_zoom_in(
    fx: &[FxInstance],
    out_w: u32,
    out_h: u32,
    u: f64,
    cam3d: Option<Cam3d>,
    lead: Option<LeadCtx>,
) -> f64 {
    let stops: Vec<usize> = fx
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f {
            FxInstance::LayerMix { from, .. } => Some((*from as usize).min(i)),
            _ => None,
        })
        .collect();
    let raw = segment(fx, out_w, out_h, out_w, out_h, u, cam3d, 0, &stops, lead);
    let (_, head) = tail_zooms(&raw);
    head.0.max(head.1).max(1.0)
}

/// Build the chain plan for the input frame `in_w×in_h` and the output
/// `out_w×out_h` (Quality). A deterministic pure function — both
/// backends must get ONE plan. `burst_index` is the absolute tick
/// (the seed of the absorbed Noise, as in the kernel).
///
/// `in_scale` (R.46, item 318) — the decode step: the physical frame is in_scale
/// times larger than the logical size of the chain. All step arithmetic lives in
/// logic (conjugation, item 314), and the details squeezed in the large input
/// are carried to the barriers and the tail. At in_scale = 1 the plan is the former one.
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
    plan_chain_in(fx, in_w, in_h, in_scale, out_w, out_h, u, cam3d, burst_index, stops, None)
}

/// The same with the context of the leading “Geometry” node (R.62): both backends
/// must pass the same `lead`.
#[allow(clippy::too_many_arguments)]
pub fn plan_chain_in(
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
    lead: Option<LeadCtx>,
) -> ChainPlan {
    let (mow, moh) = (out_w.max(2), out_h.max(2));
    let s = in_scale.max(1);
    debug_assert!(
        in_w.is_multiple_of(s) && in_h.is_multiple_of(s),
        "physical input is in_scale x logical"
    );
    // a stop at zero zeroes the prefix zoom ⇒ the step there is always 1
    debug_assert!(s == 1 || !stops.contains(&0), "leading stop forbids a decode stage");

    // --- A: segmentation and 1× programs (items 311-312) ---
    let raw = segment(fx, in_w / s, in_h / s, out_w, out_h, u, cam3d, burst_index, stops, lead);

    // --- B: backward — net zoom of the tail at each barrier ---
    let (tail, _) = tail_zooms(&raw);

    // --- C: forward — net compression; K = quantum(min(zoom of the tail, compression)) ---
    let k_cap = (MAX_INTER_DIM / mow.max(moh)).max(1);
    debug_assert!(s <= k_cap, "decode stage above the intermediate ceiling");
    let mut kmap: Vec<u32> = vec![1; raw.len()];
    // the input step is already squeezed detail: it is the starting “compression”
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
                    // LEADING barrier (there was no resample before it): the frame under
                    // it is physically S×, the view must be computed at this
                    // resolution — the benefit formula does not vote here
                    kmap[ri] = s;
                }
            }
            RawKind::Snapshot | RawKind::Mix { .. } => {
                resampled = true; // stops are always at 1× — there are no leading barriers further on
                sq = (1.0, 1.0);
            }
        }
    }

    // --- D: assembly. Input 1× and all K == 1 — ready 1× programs (bytes of
    // items 311-313); otherwise the runs are rebuilt with conjugation (k_in, k_out).
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
                // run target: K of the next barrier; the budget cut, the stop and
                // the end of the chain — 1×
                let k_out = match raw.get(ri + 1) {
                    Some(RawKind::Barrier { .. }) => kmap[ri + 1],
                    _ => 1,
                };
                let steps: Vec<&FxInstance> = fx[*i..*j]
                    .iter()
                    .filter(|g| is_geo_active(g, lead, out_w, out_h) || is_pointwise(g))
                    .collect();
                let exec = build_geom_exec_in(
                    &steps,
                    w,
                    h,
                    out_w,
                    out_h,
                    u,
                    cam3d,
                    burst_index,
                    cur_k,
                    k_out,
                    lead,
                );
                match &exec {
                    GeomExec::Fused(p) => {
                        w = p.out_w;
                        h = p.out_h;
                    }
                    GeomExec::Legacy => {
                        // Legacy with a SCALE CHANGE is impossible (forced
                        // Fused); here — a mirror at equal edges
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
