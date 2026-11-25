//! The sound graph (fx-graph-dag D4): a chain whose slots name their inputs, run by the event's
//! (or the track's) processor in slot order on one working buffer.
//!
//! The plan is decided ONCE per chain revision (`GraphPlan::new`) and the pass only follows it:
//! - a value read by anything but the next step's main input in place is SPILLED into a register
//!   right before the working buffer is overwritten, and a step whose main input is not the
//!   working buffer LOADS it — the picture's `Hold`/`Load` (D3) with the same rule;
//! - each value carries the look-ahead of its path (PDC, R.48): a Merge (or a keyed node) delays
//!   the input that has less by the difference, so both sides stand at one content moment — the
//!   layer strength's dry ring, generalised; the chain's look-ahead is the Output path's;
//! - a step that reaches no Output is not run (a dangling node does not ring);
//! - a sound read's slot (`ChainGraph::reads`) takes the read's output (`Feed::input`) — a time kind
//!   in an arm reads the source on its own (D2), and the slots only it needs run there.
//!
//! An empty graph is the line: the processors keep their former pass for it, byte for byte.

use kt_model::FxInstance;
use kt_model::fx_graph::{self, Node, SlotInputs};

/// A sound chain's wiring as its processor runs it. Empty — the line.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ChainGraph {
    /// The chain's wiring (canonical: empty for a line), aligned with the processor's `fx`.
    pub fx_in: Vec<SlotInputs>,
    /// The slots a sound read feeds, read `k` at `reads[k]` (`fx_graph::reads` order).
    pub reads: Vec<usize>,
}

impl ChainGraph {
    pub fn is_line(&self) -> bool {
        self.fx_in.is_empty() && self.reads.is_empty()
    }
}

/// A sound chain split for its processors (D2, D4): the main chain keeps every slot's index; a slot
/// the main processor does not run itself (a read's time kind, a slot only a read needs, a dangling
/// one) is `FxInstance::Missing`, so the chain's tail, warp, context and source windows are those of
/// what this processor runs. Each read is a LINE of the original slots (`keep`, ascending): the
/// derived chain an event would have, its Output the time kind.
#[derive(Debug, Clone, Default)]
pub struct SoundSplit {
    pub main: Vec<FxInstance>,
    pub graph: ChainGraph,
    pub reads: Vec<Vec<usize>>,
}

/// The placeholder of a slot this processor does not run.
fn skipped() -> FxInstance {
    FxInstance::Missing(kt_model::MissingSlot::named("graph-skip"))
}

/// Split a sound chain (`fx` and its wiring, both the ACTIVE chain's) for its processors. A line is
/// returned as it is.
pub fn sound_split(fx: &[FxInstance], fx_in: &[SlotInputs]) -> SoundSplit {
    if fx_graph::is_line(fx_in) {
        return SoundSplit { main: fx.to_vec(), graph: ChainGraph::default(), reads: Vec::new() };
    }
    let reads = fx_graph::reads(fx, fx_in);
    let graph = ChainGraph { fx_in: fx_in.to_vec(), reads: reads.iter().map(|r| r.slot).collect() };
    let live = live_slots(fx, &graph);
    let main = fx
        .iter()
        .enumerate()
        .map(|(i, f)| if live[i] && !graph.reads.contains(&i) { f.clone() } else { skipped() })
        .collect();
    SoundSplit { main, graph, reads: reads.into_iter().map(|r| r.keep).collect() }
}

/// Whether slot `i` of `fx` is a STEP of the pass: it has a node, or a read stands there.
fn is_step(fx: &[FxInstance], graph: &ChainGraph, i: usize) -> bool {
    graph.reads.contains(&i) || super::AudioNode::has_node(&fx[i])
}

/// The slot's second input socket is honoured: a Merge's Foreground, a keyed node's key (D4).
fn takes_second(f: &FxInstance) -> bool {
    f.is_merge() || f.has_key_socket()
}

/// A value of the pass: the chain's input or a step's output (by slot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Val {
    Src,
    Out(usize),
}

/// The value standing at `node`: a slot without a step passes its main input.
fn value(fx: &[FxInstance], graph: &ChainGraph, node: Node) -> Val {
    let mut node = node;
    loop {
        match node {
            Node::Source => return Val::Src,
            Node::Slot(j) if is_step(fx, graph, j) => return Val::Out(j),
            Node::Slot(j) => node = fx_graph::resolve(j, fx_graph::inputs(&graph.fx_in, j).main),
        }
    }
}

/// A step's (main, second) inputs; a read's step takes none.
fn step_inputs(fx: &[FxInstance], graph: &ChainGraph, i: usize) -> (Option<Val>, Option<Val>) {
    if graph.reads.contains(&i) {
        return (None, None);
    }
    let s = fx_graph::inputs(&graph.fx_in, i);
    let main = value(fx, graph, fx_graph::resolve(i, s.main));
    let second = s.second.filter(|_| takes_second(&fx[i])).map(|x| value(fx, graph, fx_graph::resolve(i, x)));
    (Some(main), second)
}

/// The Output's value (the last slot's).
fn output(fx: &[FxInstance], graph: &ChainGraph) -> Val {
    match fx.len() {
        0 => Val::Src,
        n => value(fx, graph, Node::Slot(n - 1)),
    }
}

/// The slots whose step reaches the Output (a read's step reads nothing of the main chain).
fn live_slots(fx: &[FxInstance], graph: &ChainGraph) -> Vec<bool> {
    let n = fx.len();
    let mut live = vec![false; n];
    let mark = |v: Val, live: &mut Vec<bool>| {
        if let Val::Out(j) = v {
            live[j] = true;
        }
    };
    mark(output(fx, graph), &mut live);
    for i in (0..n).rev() {
        if !live[i] {
            continue;
        }
        let (m, s) = step_inputs(fx, graph, i);
        for v in m.into_iter().chain(s) {
            mark(v, &mut live);
        }
    }
    live
}

/// One step's part of the plan (indexed like the processor's nodes).
#[derive(Debug, Clone, Default)]
pub(crate) struct Step {
    /// The step runs (it reaches the Output).
    pub live: bool,
    /// Save the working buffer into this register first (its value is read again later).
    pub spill: Option<usize>,
    /// Then load the main input from this register.
    pub load: Option<usize>,
    /// The second input (a Merge's Foreground, a key) from this register.
    pub second: Option<usize>,
    /// The alignment delays of the main and the second input (frames).
    pub d_main: i64,
    pub d_second: i64,
    /// The look-ahead of the step's input path (after alignment).
    pub lat_in: i64,
}

/// The plan of a graph pass over the processor's nodes.
#[derive(Debug, Clone, Default)]
pub(crate) struct GraphPlan {
    pub steps: Vec<Step>,
    /// After the last step the Output is loaded from this register (it is not the working buffer).
    pub out_load: Option<usize>,
    pub regs: usize,
    /// The look-ahead of the Output path.
    pub la: i64,
}

impl GraphPlan {
    /// The plan of `fx` under `graph` for the steps at `slots` (ascending; every slot with a step),
    /// `la(k)` the look-ahead of step `k`.
    pub fn new(fx: &[FxInstance], graph: &ChainGraph, slots: &[usize], la: impl Fn(usize) -> i64) -> GraphPlan {
        let live = live_slots(fx, graph);
        let idx = |slot: usize| slots.iter().position(|s| *s == slot);
        let n = slots.len();
        let inputs: Vec<(Option<Val>, Option<Val>)> = slots.iter().map(|&s| step_inputs(fx, graph, s)).collect();
        let out = output(fx, graph);
        // look-ahead of every value along the live steps
        let mut lat_out = vec![0i64; n];
        let lat = |v: Val, lat_out: &[i64]| match v {
            Val::Src => 0,
            Val::Out(s) => idx(s).map_or(0, |k| lat_out[k]),
        };
        let mut steps: Vec<Step> = (0..n).map(|_| Step::default()).collect();
        for k in 0..n {
            if !live[slots[k]] {
                continue;
            }
            let (m, s) = inputs[k];
            let lm = m.map_or(0, |v| lat(v, &lat_out));
            let ls = s.map(|v| lat(v, &lat_out));
            let lin = lm.max(ls.unwrap_or(0));
            steps[k].live = true;
            steps[k].lat_in = lin;
            steps[k].d_main = lin - lm;
            steps[k].d_second = ls.map_or(0, |l| lin - l);
            lat_out[k] = lin + la(k);
        }
        let la_out = lat(out, &lat_out);
        // the reads of every value, in pass order; the Output reads its value after the last step
        let mut uses: Vec<(usize, Val)> = Vec::new();
        for (k, st) in steps.iter().enumerate() {
            if st.live {
                let (m, s) = inputs[k];
                uses.extend(m.into_iter().chain(s).map(|v| (k, v)));
            }
        }
        uses.push((n, out));
        let reads_from = |v: Val, k: usize| uses.iter().filter(|(at, u)| *at >= k && *u == v).count();
        // registers: one per spilled value
        let mut reg_of: Vec<(Val, usize)> = Vec::new();
        let mut regs = 0;
        let mut cur = Val::Src;
        for k in 0..n {
            if !steps[k].live {
                continue;
            }
            let (m, s) = inputs[k];
            let in_place = m == Some(cur);
            let later = reads_from(cur, k) - usize::from(in_place);
            if later > 0 && !reg_of.iter().any(|(v, _)| *v == cur) {
                reg_of.push((cur, regs));
                steps[k].spill = Some(regs);
                regs += 1;
            }
            let reg = |v: Val| reg_of.iter().find(|(u, _)| *u == v).map(|(_, r)| *r);
            if let Some(m) = m.filter(|_| !in_place) {
                steps[k].load = reg(m);
                debug_assert!(steps[k].load.is_some(), "a main input neither in place nor held");
            }
            if let Some(s) = s {
                steps[k].second = reg(s);
                debug_assert!(steps[k].second.is_some(), "a second input not held");
            }
            cur = Val::Out(slots[k]);
        }
        let out_load = (out != cur).then(|| reg_of.iter().find(|(u, _)| *u == out).map(|(_, r)| *r)).flatten();
        GraphPlan { steps, out_load, regs, la: la_out }
    }

    pub fn live(&self, k: usize) -> bool {
        self.steps.get(k).is_some_and(|s| s.live)
    }
}

/// The look-ahead of a graph chain's Output path, without building state (the static twin of the
/// processor's; with an empty graph it is `chain_lookahead`).
pub fn chain_lookahead_in(fx: &[FxInstance], graph: &ChainGraph, auto: &kt_model::AutoProfiles) -> i64 {
    if graph.is_line() {
        return super::chain_lookahead(fx, auto);
    }
    let slots: Vec<usize> = (0..fx.len()).filter(|&i| is_step(fx, graph, i)).collect();
    GraphPlan::new(fx, graph, &slots, |k| {
        if graph.reads.contains(&slots[k]) { 0 } else { super::slot_lookahead(fx, auto, slots[k]) }
    })
    .la
}

/// The structure fingerprint of a graph chain: the kinds, and the wiring when there is one (a rewire
/// is a structural change, as a reorder is). With an empty graph it is `chain_fingerprint`.
pub fn chain_fingerprint_in(fx: &[FxInstance], graph: &ChainGraph) -> u64 {
    use std::hash::{Hash, Hasher};
    let line = super::chain_fingerprint(fx);
    if graph.is_line() {
        return line;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    line.hash(&mut h);
    graph.hash(&mut h);
    h.finish()
}

/// What a processor's pass needs besides the source: the reads' outputs.
pub trait Feed {
    /// The source (false = underrun), as `ClipProcessor::read`'s `pull`.
    fn pull(&mut self, kind: crate::PullKind, src_frame: i64, dst: &mut [f32]) -> bool;
    /// Read `k`'s output at clip-local frame `local` (fx-graph-dag D4): the caller's read
    /// processor, read as an event is. Silence by default.
    fn input(&mut self, _k: usize, _local: i64, dst: &mut [f32]) {
        dst.fill(0.0);
    }
}

/// A `Feed` with no reads.
pub(crate) struct PullOnly<'a>(pub &'a mut dyn FnMut(crate::PullKind, i64, &mut [f32]) -> bool);

impl Feed for PullOnly<'_> {
    fn pull(&mut self, kind: crate::PullKind, src_frame: i64, dst: &mut [f32]) -> bool {
        (self.0)(kind, src_frame, dst)
    }
}

/// The pass's kept buffers and alignment rings (per step).
#[derive(Default, Clone)]
pub(crate) struct GraphState {
    pub regs: Vec<Vec<f32>>,
    pub second: Vec<f32>,
    pub ring_main: Vec<Option<super::DryRing>>,
    pub ring_second: Vec<Option<super::DryRing>>,
}

impl GraphState {
    pub fn new(plan: &GraphPlan) -> GraphState {
        let ring = |d: i64| (d > 0).then(|| super::DryRing::new(d));
        GraphState {
            regs: vec![Vec::new(); plan.regs],
            second: Vec::new(),
            ring_main: plan.steps.iter().map(|s| ring(s.d_main)).collect(),
            ring_second: plan.steps.iter().map(|s| ring(s.d_second)).collect(),
        }
    }

    pub fn reset(&mut self) {
        for r in self.ring_main.iter_mut().chain(self.ring_second.iter_mut()).flatten() {
            r.reset();
        }
    }

    /// Before step `k` runs: spill, load and align its main input in `sub`, and give back its
    /// aligned second input (in `self.second` when it was delayed).
    pub fn enter(&mut self, plan: &GraphPlan, k: usize, sub: &mut [f32]) -> bool {
        let st = &plan.steps[k];
        if let Some(r) = st.spill {
            let v = std::mem::take(&mut self.regs[r]);
            self.regs[r] = super::copy_into(v, sub);
        }
        if let Some(r) = st.load {
            sub.copy_from_slice(&self.regs[r][..sub.len()]);
        }
        if let Some(ring) = self.ring_main[k].as_mut() {
            ring.run(sub);
        }
        match (st.second, self.ring_second[k].as_mut()) {
            (Some(r), Some(ring)) => {
                let v = std::mem::take(&mut self.second);
                self.second = super::copy_into(v, &self.regs[r][..sub.len()]);
                ring.run(&mut self.second);
                true
            }
            _ => false,
        }
    }

    /// Step `k`'s second input after `enter` (`delayed` — what `enter` returned).
    pub fn second<'a>(&'a self, plan: &GraphPlan, k: usize, delayed: bool, len: usize) -> Option<&'a [f32]> {
        if delayed {
            return Some(&self.second[..len]);
        }
        plan.steps[k].second.map(|r| &self.regs[r][..len])
    }

    /// After the last step: the Output into `sub`.
    pub fn leave(&self, plan: &GraphPlan, sub: &mut [f32]) {
        if let Some(r) = plan.out_load {
            sub.copy_from_slice(&self.regs[r][..sub.len()]);
        }
    }
}

/// A Merge over the working buffer (the Background): the sum order is fixed, frame by frame —
/// Background, then Foreground × gain (invariant 11).
pub(crate) fn merge(sub: &mut [f32], fg: Option<&[f32]>, crossfade: bool, k: f32) {
    let Some(fg) = fg else { return };
    if crossfade {
        let ik = 1.0 - k;
        for (v, f) in sub.iter_mut().zip(fg) {
            *v = *v * ik + *f * k;
        }
    } else {
        for (v, f) in sub.iter_mut().zip(fg) {
            *v += *f * k;
        }
    }
}
