//! Execution plan of the FX chain (R.45, items 311-314): the decisions on segmentation,
//! legacy/fused, absorption of per-pixel ops and K× barrier resolution are taken HERE,
//! exactly once — the GPU (`chain_ops` in kt-gpu) executes the READY plan. This is
//! invariant No. 7 at the chain level: one place decides the segmentation.
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
//! `stops` and gets back Snapshot/Mix markers; the plan of a slice between stops
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
/// It is raised with one constant; devices with a weak max_dim refuse such
/// plans as TooLarge (guard in kt-gpu resolve_chain).
pub const MAX_INTER_DIM: u32 = kt_plugin_api::barrier::MAX_INTER_DIM;
// the largest K the planner hands a barrier is the supersampling ceiling; the plugin API checks
// barrier graphs at that K (plugin-api-barriers)
const _: () = assert!(crate::geom::SS_MAX == kt_plugin_api::barrier::MAX_RES_SCALE);

/// One op of the plan. Audio FX do not enter the plan at all (they are transparent
/// both for segmentation and for execution).
pub enum PlanOp {
    /// Geo run `fx[i..j)` (inside — geo, transparent audio and absorbed
    /// INTERNAL per-pixel ops, item 312) with the execution decision already taken.
    Run { i: usize, j: usize, exec: GeomExec },
    /// Barrier effect `fx[idx]` at the `res_scale`× resolution (item 314).
    Barrier { idx: usize, res_scale: u32 },
    /// A RUN of point-wise stages `fx[i..j)` at a barrier position (fused-pointwise-runs): two or
    /// more stages that fuse in runs (`fuses_in_runs`), the transparent slots between them included.
    /// The executor runs it as one generated pass; `res_scale` is what its first stage had as a
    /// barrier (every stage of a run gets the same K: no run or stop stands between them).
    PointRun { i: usize, j: usize, res_scale: u32 },
    /// Snapshot of the dry side of the layer strength (position from `stops`).
    Snapshot,
    /// Strength slot `fx[idx]` (LayerMix): the executor mixes the snapshot with the
    /// current state.
    Mix { idx: usize },
    /// Keep the current frame in register `reg` (fx-graph-dag D3): a value an arm reads later.
    /// `at` — the chain position it stands at (before slot `at`).
    Hold { reg: usize, at: usize },
    /// Make register `reg`'s frame current: slot `at` reads that value.
    Load { reg: usize, at: usize },
    /// The Merge slot `fx[idx]`: the current frame is the Background, register `fg` the
    /// Foreground, `matte` the register and the rule of its coverage.
    Merge { idx: usize, fg: usize, matte: Option<(usize, kt_model::MatteFrom)> },
    /// Slot `at` is a time kind in an arm: its output is read `k`'s finished frame (design D2).
    Input { k: usize, at: usize },
}

/// A graph step the lowering puts before a slot (fx-graph-dag D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphStep {
    Hold { reg: usize },
    Load { reg: usize },
}

/// A Merge's registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeIn {
    pub fg: usize,
    pub matte: Option<(usize, kt_model::MatteFrom)>,
}

/// A chain graph lowered into a straight program (fx-graph-dag D3): the steps before each slot,
/// the slots that reach no Output (skipped), each Merge's registers. A pure function of the wiring:
/// the same chain gives the same program on every executor (invariant 7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lowered {
    pub before: Vec<Vec<GraphStep>>,
    pub dead: Vec<bool>,
    /// `None` on a Merge with no Foreground wired: it passes its Background.
    pub merges: Vec<Option<MergeIn>>,
    /// A slot whose output is a read's frame (design D2): `Some(k)`, read `k`.
    pub inputs: Vec<Option<usize>>,
    /// The registers the program uses (at most `fx_graph::MAX_HELD`).
    pub regs: usize,
}

impl Lowered {
    /// The chain positions a geometry window or a point-wise run must not cross: a position with
    /// graph steps, a skipped slot, a Merge.
    fn cuts(&self) -> Vec<usize> {
        (0..self.dead.len())
            .filter(|&i| !self.before[i].is_empty() || self.dead[i] || self.merges[i].is_some() || self.inputs[i].is_some())
            .collect()
    }
}

/// Lower a chain's wiring (`fx_in`, aligned with `fx`). `None` for a line: the plan is today's.
/// The rule: every value read by anything but the next slot's main input is held right after it is
/// made (the Source before the first live slot); a slot whose main input is not the current frame
/// loads it; a register is free after its last reader.
pub fn lower(fx: &[FxInstance], fx_in: &[kt_model::SlotInputs]) -> Option<Lowered> {
    lower_with(fx, fx_in, &[])
}

/// `lower` where the slots `reads` are each a read's output (design D2): such a slot reads nothing
/// in this program, so the slots only it needed are skipped here (they run in the read).
pub fn lower_with(fx: &[FxInstance], fx_in: &[kt_model::SlotInputs], reads: &[usize]) -> Option<Lowered> {
    use kt_model::fx_graph::{self, Socket};
    if fx_graph::is_line(fx_in) {
        return None;
    }
    let n = fx.len();
    // a line written out as entries is the line: the planner does not rest on its caller's form
    let mut canon = fx_in.to_vec();
    fx_graph::canonical(&mut canon, n);
    if canon.is_empty() {
        return None;
    }
    let fx_in = &canon[..];
    // the slots reaching the Output, a read's slot reading nothing here
    let mut live = vec![false; n];
    if n > 0 {
        live[n - 1] = true;
    }
    for i in (0..n).rev() {
        if !live[i] || reads.contains(&i) {
            continue;
        }
        for (node, _) in fx_graph::reads_of(fx_in, i) {
            if let fx_graph::Node::Slot(j) = node {
                live[j] = true;
            }
        }
    }
    // value v: -1 the Source, j a slot; index v + 1
    let mut held = vec![false; n + 1];
    let mut last = vec![-1isize; n + 1];
    // the frame current before a live slot is the previous LIVE slot's (a dangling one runs nothing)
    let mut prev: isize = -1;
    for i in (0..n).filter(|&i| live[i]) {
        for (node, socket) in fx_graph::reads_of(fx_in, i).filter(|_| !reads.contains(&i)) {
            let v = match node {
                fx_graph::Node::Source => -1,
                fx_graph::Node::Slot(j) => j as isize,
            };
            let trivial = socket == Socket::Main && v == prev;
            last[(v + 1) as usize] = last[(v + 1) as usize].max(i as isize);
            held[(v + 1) as usize] |= !trivial;
        }
        prev = i as isize;
    }
    let mut before: Vec<Vec<GraphStep>> = vec![Vec::new(); n];
    let mut merges: Vec<Option<MergeIn>> = vec![None; n];
    let mut inputs: Vec<Option<usize>> = vec![None; n];
    let mut regs: Vec<Option<isize>> = Vec::new();
    let mut reg_of = vec![usize::MAX; n + 1];
    let mut pending: Option<isize> = held[0].then_some(-1);
    let mut cur: isize = -1;
    let alloc = |regs: &mut Vec<Option<isize>>, v: isize| -> usize {
        match regs.iter().position(Option::is_none) {
            Some(r) => {
                regs[r] = Some(v);
                r
            }
            None => {
                regs.push(Some(v));
                regs.len() - 1
            }
        }
    };
    let mut peak = 0;
    for i in (0..n).filter(|&i| live[i]) {
        if let Some(v) = pending.take() {
            let r = alloc(&mut regs, v);
            reg_of[(v + 1) as usize] = r;
            before[i].push(GraphStep::Hold { reg: r });
        }
        peak = peak.max(regs.len());
        let s = fx_graph::inputs(fx_in, i);
        let pos = |x: kt_model::FxIn| match fx_graph::resolve(i, x) {
            fx_graph::Node::Source => -1,
            fx_graph::Node::Slot(j) => j as isize,
        };
        if let Some(k) = reads.iter().position(|&r| r == i) {
            // the read's frame stands here; it loads nothing
            inputs[i] = Some(k);
        } else {
            let bg = pos(s.main);
            if bg != cur {
                before[i].push(GraphStep::Load { reg: reg_of[(bg + 1) as usize] });
            }
        }
        if fx[i].is_merge()
            && let Some(fg) = s.second
        {
            merges[i] = Some(MergeIn {
                fg: reg_of[(pos(fg) + 1) as usize],
                matte: s.matte.map(|m| (reg_of[(pos(m.from) + 1) as usize], m.by)),
            });
        }
        cur = i as isize;
        // the registers whose last reader this slot was are free again
        for v in regs.iter_mut() {
            if v.is_some_and(|val| last[(val + 1) as usize] <= i as isize) {
                *v = None;
            }
        }
        if held[i + 1] {
            pending = Some(i as isize);
        }
    }
    let dead = live.iter().map(|l| !l).collect();
    Some(Lowered { before, dead, merges, inputs, regs: peak })
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
        } else if f.is_audio() || f.is_stream() || is_pointwise(f) || is_geo(f) {
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
    PointRun { i: usize, j: usize },
    Snapshot,
    Mix { idx: usize },
    Graph { at: usize, steps: Vec<GraphStep> },
    Merge { idx: usize, m: MergeIn },
    Input { k: usize, at: usize },
}

/// Whether the stage fuses into a point-wise run (fused-pointwise-runs): the plugin contract's answer,
/// `kt_plugin_api::fuses_in_runs` — never guessed from a kind.
pub fn fuses_in_runs(f: &FxInstance) -> bool {
    match f {
        FxInstance::Plugin(slot) => slot.plugin().is_some_and(|p| kt_plugin_api::fuses_in_runs(p.as_ref())),
        _ => false,
    }
}

/// A slot a point-wise run passes through: audio, a stream stage (a source stage the planner never
/// executes) and an identity “Geometry” node, as a geometry window does.
fn run_transparent(f: &FxInstance, lead: Option<LeadCtx>, out_w: u32, out_h: u32) -> bool {
    f.is_audio() || f.is_stream() || (is_geo(f) && !is_geo_active(f, lead, out_w, out_h))
}

/// The end of a point-wise run started at the fusable stage `i` and how many fusable stages it holds:
/// fusable stages and transparent slots, ended by any other stage or a stop; trailing transparent
/// slots are left out (they stay where they were).
fn point_run_end(fx: &[FxInstance], i: usize, stops: &[usize], lead: Option<LeadCtx>, out_w: u32, out_h: u32) -> (usize, usize) {
    let (mut j, mut end, mut n) = (i, i, 0usize);
    while j < fx.len() {
        if j > i && stops.contains(&j) {
            break;
        }
        let f = &fx[j];
        if fuses_in_runs(f) {
            n += 1;
            j += 1;
            end = j;
        } else if run_transparent(f, lead, out_w, out_h) {
            j += 1;
        } else {
            break;
        }
    }
    (end, n)
}

/// The point-wise runs a chain holds when nothing else is known (fused-pointwise-runs): the plugin
/// ids of each run of two or more fusable stages, ended by every other stage and every strength
/// slot's snapshot point. What the player hands the executor to prewarm; a run the plan cuts
/// otherwise (a mask point, a geometry series that absorbs the stages) compiles when a frame meets it.
pub fn run_shapes(fx: &[FxInstance]) -> Vec<Vec<&'static str>> {
    let stops: Vec<usize> = fx
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f {
            FxInstance::LayerMix { from, .. } => Some((*from as usize).min(i)),
            _ => None,
        })
        .collect();
    let geo_ahead = |k: usize| fx[k..].iter().any(|g| is_geo_active(g, None, 2, 2));
    let mut out = Vec::new();
    let mut k = 0;
    while k < fx.len() {
        // a stage between two geometry steps is absorbed into their series (`run_window_end`)
        let absorbed = fx[..k].iter().any(|g| is_geo_active(g, None, 2, 2)) && geo_ahead(k);
        if fuses_in_runs(&fx[k]) && !absorbed {
            let (end, n) = point_run_end(fx, k, &stops, None, 2, 2);
            if n >= 2 {
                out.push(fx[k..end].iter().filter(|f| fuses_in_runs(f)).map(|f| f.tag()).collect());
            }
            k = end.max(k + 1);
        } else {
            k += 1;
        }
    }
    out
}

/// `run_shapes` of a chain with its wiring (fx-graph-dag D3): the runs of each stretch between the
/// graph's steps, as the plan cuts them; a slot that reaches no Output runs nothing.
pub fn run_shapes_in(fx: &[FxInstance], fx_in: &[kt_model::SlotInputs]) -> Vec<Vec<&'static str>> {
    let Some(g) = lower(fx, fx_in) else { return run_shapes(fx) };
    let cuts = g.cuts();
    let mut out = Vec::new();
    let mut piece: Vec<FxInstance> = Vec::new();
    for (i, f) in fx.iter().enumerate() {
        if cuts.contains(&i) {
            out.extend(run_shapes(&std::mem::take(&mut piece)));
        }
        if !g.dead[i] {
            piece.push(f.clone());
        }
    }
    out.extend(run_shapes(&piece));
    out
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
    graph: Option<&Lowered>,
    input_wh: &[(u32, u32)],
) -> Vec<RawKind> {
    let (mow, moh) = (out_w.max(2), out_h.max(2));
    let (mut w, mut h) = (in_w, in_h);
    let mut raw: Vec<RawKind> = Vec::new();
    // the windows stop at the snapshot points and at every graph position (fx-graph-dag D3)
    let cut: Vec<usize>;
    let snaps = stops;
    let stops: &[usize] = match graph {
        Some(g) => {
            let mut all = snaps.to_vec();
            all.extend(g.cuts());
            all.sort_unstable();
            all.dedup();
            cut = all;
            &cut
        }
        None => snaps,
    };
    // a register's frame size: a Load makes it current again
    let mut reg_wh: Vec<(u32, u32)> = vec![(0, 0); graph.map_or(0, |g| g.regs)];
    let mut idx = 0usize;
    while idx < fx.len() {
        if snaps.contains(&idx) {
            raw.push(RawKind::Snapshot);
        }
        if let Some(g) = graph {
            if g.dead[idx] {
                idx += 1;
                continue;
            }
            if !g.before[idx].is_empty() {
                for st in &g.before[idx] {
                    match *st {
                        GraphStep::Hold { reg } => reg_wh[reg] = (w, h),
                        GraphStep::Load { reg } => (w, h) = reg_wh[reg],
                    }
                }
                raw.push(RawKind::Graph { at: idx, steps: g.before[idx].clone() });
            }
        }
        if let Some(k) = graph.and_then(|g| g.inputs[idx]) {
            // the read's frame: its own size (the read ran its own chain)
            (w, h) = input_wh.get(k).copied().unwrap_or((w, h));
            raw.push(RawKind::Input { k, at: idx });
            idx += 1;
            continue;
        }
        let f = &fx[idx];
        if f.is_merge() {
            // a Merge with no Foreground (or a line's Merge) passes its Background; the size of a
            // merge is its Background's (design D3)
            if let Some(m) = graph.and_then(|g| g.merges[idx]) {
                raw.push(RawKind::Merge { idx, m });
            }
            idx += 1;
            continue;
        }
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
                    if crate::geom::plugin_lone(step).is_none() {
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
        if fuses_in_runs(f) {
            // two or more fusable point-wise stages: one generated pass (fused-pointwise-runs)
            let (end, n) = point_run_end(fx, idx, stops, lead, out_w, out_h);
            if n >= 2 {
                raw.push(RawKind::PointRun { i: idx, j: end });
                idx = end;
                continue;
            }
        }
        if f.is_layer_mix() {
            raw.push(RawKind::Mix { idx });
        } else if !f.is_audio() && !f.is_stream() && !is_geo(f) {
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
            RawKind::Barrier { .. } | RawKind::PointRun { .. } => tail[ri] = tm,
            RawKind::Snapshot | RawKind::Mix { .. } | RawKind::Graph { .. } | RawKind::Merge { .. } | RawKind::Input { .. } => tm = (1.0, 1.0),
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
    let raw = segment(fx, out_w, out_h, out_w, out_h, u, cam3d, 0, &stops, lead, None, &[]);
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
    plan_graph_in(fx, None, &[], in_w, in_h, in_scale, out_w, out_h, u, cam3d, burst_index, stops, lead)
}

/// The same for a chain graph (fx-graph-dag D3): `graph` is `lower`'s program (`None` — a line,
/// the plan of `plan_chain_in` byte for byte). Every graph position is a stop: the geometry series,
/// the point-wise runs and the K× barriers fuse only between them, and a run between two stops is
/// planned as the line made of its slots.
#[allow(clippy::too_many_arguments)]
pub fn plan_graph_in(
    fx: &[FxInstance],
    graph: Option<&Lowered>,
    // each read's output size (design D2), by read
    input_wh: &[(u32, u32)],
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
    let mut raw = segment(fx, in_w / s, in_h / s, out_w, out_h, u, cam3d, burst_index, stops, lead, graph, input_wh);
    // The step is the clip's maximum over its length, and THIS moment may have nothing that brings it
    // back to 1x: a zoom curve at 1, slots that are all identity (found-defects 843). Nothing then ends at
    // the canvas; the frame comes back first, through the identity placement, so the picture is the limit
    // of the zoom's own.
    let still = FxInstance::geometry_zoom(1.0);
    let back = |r: &RawKind| matches!(r, RawKind::Run { .. });
    let at_1x = |r: &RawKind| !matches!(r, RawKind::Barrier { .. } | RawKind::PointRun { .. });
    if s > 1 && !raw.iter().find(|r| at_1x(r)).is_some_and(back) {
        raw.insert(0, RawKind::Run { i: 0, j: 0, exec: GeomExec::Legacy, lens: (1.0, 1.0) });
    }

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
            RawKind::Barrier { .. } | RawKind::PointRun { .. } => {
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
            RawKind::Snapshot | RawKind::Mix { .. } | RawKind::Graph { .. } | RawKind::Merge { .. } | RawKind::Input { .. } => {
                resampled = true; // stops are always at 1× — there are no leading barriers further on
                sq = (1.0, 1.0);
            }
        }
    }

    // --- D: assembly. Input 1× and all K == 1 — ready 1× programs (bytes of
    // items 311-313); otherwise the runs are rebuilt with conjugation (k_in, k_out).
    if s == 1 && kmap.iter().all(|&k| k == 1) {
        let mut ops = Vec::with_capacity(raw.len());
        for r in raw {
            match r {
                RawKind::Run { i, j, exec, .. } => ops.push(PlanOp::Run { i, j, exec }),
                RawKind::Barrier { idx } => ops.push(PlanOp::Barrier { idx, res_scale: 1 }),
                RawKind::PointRun { i, j } => ops.push(PlanOp::PointRun { i, j, res_scale: 1 }),
                RawKind::Snapshot => ops.push(PlanOp::Snapshot),
                RawKind::Mix { idx } => ops.push(PlanOp::Mix { idx }),
                RawKind::Graph { at, steps } => ops.extend(steps.iter().map(|st| graph_op(st, at))),
                RawKind::Merge { idx, m } => ops.push(PlanOp::Merge { idx, fg: m.fg, matte: m.matte }),
                RawKind::Input { k, at } => ops.push(PlanOp::Input { k, at }),
            }
        }
        return ChainPlan { ops };
    }
    let mut ops = Vec::with_capacity(raw.len());
    let (mut w, mut h) = (in_w, in_h);
    let mut reg_wh: Vec<(u32, u32)> = vec![(0, 0); graph.map_or(0, |g| g.regs)];
    let mut cur_k = s;
    for ri in 0..raw.len() {
        match &raw[ri] {
            RawKind::Run { i, j, .. } => {
                // run target: K of the next barrier; the budget cut, the stop and
                // the end of the chain — 1×
                let k_out = match raw.get(ri + 1) {
                    Some(RawKind::Barrier { .. } | RawKind::PointRun { .. }) => kmap[ri + 1],
                    _ => 1,
                };
                let steps: Vec<&FxInstance> = match i == j {
                    true => vec![&still],
                    false => fx[*i..*j]
                        .iter()
                        .filter(|g| is_geo_active(g, lead, out_w, out_h) || is_pointwise(g))
                        .collect(),
                };
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
            RawKind::PointRun { i, j } => {
                debug_assert_eq!(cur_k, kmap[ri], "point run not at its segment resolution");
                ops.push(PlanOp::PointRun { i: *i, j: *j, res_scale: kmap[ri] });
            }
            RawKind::Snapshot => {
                debug_assert_eq!(cur_k, 1, "layer-mix snapshot must be 1x");
                ops.push(PlanOp::Snapshot);
            }
            RawKind::Mix { idx } => {
                debug_assert_eq!(cur_k, 1, "layer-mix blend must be 1x");
                ops.push(PlanOp::Mix { idx: *idx });
            }
            RawKind::Graph { at, steps } => {
                // a held frame is at 1×: a branch point is the trunk's end, and the engine takes the
                // decode step from the trunk alone, whose last op ends at 1× before a stop
                debug_assert_eq!(cur_k, 1, "a graph step must be 1x");
                for st in steps {
                    match *st {
                        GraphStep::Hold { reg } => reg_wh[reg] = (w, h),
                        GraphStep::Load { reg } => (w, h) = reg_wh[reg],
                    }
                }
                ops.extend(steps.iter().map(|st| graph_op(st, *at)));
            }
            RawKind::Merge { idx, m } => {
                debug_assert_eq!(cur_k, 1, "a merge must be 1x");
                ops.push(PlanOp::Merge { idx: *idx, fg: m.fg, matte: m.matte });
            }
            RawKind::Input { k, at } => {
                (w, h) = input_wh.get(*k).copied().unwrap_or((w, h));
                cur_k = 1;
                ops.push(PlanOp::Input { k: *k, at: *at });
            }
        }
    }
    ChainPlan { ops }
}

fn graph_op(st: &GraphStep, at: usize) -> PlanOp {
    match *st {
        GraphStep::Hold { reg } => PlanOp::Hold { reg, at },
        GraphStep::Load { reg } => PlanOp::Load { reg, at },
    }
}
