//! Effect graphs (fx-graph-dag, v2a): a chain's wiring, kept as ONE sparse list parallel to the slots.
//!
//! A chain stays `fx: Vec<FxInstance>`; `fx_in[i]` says where slot `i` takes its frame (or its
//! samples) when it is not "the slot before". An empty list is today's line, byte for byte: the
//! field is not written, nothing hashes it, nothing reads it (design D1). Inputs name only EARLIER
//! slots or the chain's Source, so the slot order is always a processing order and a cycle cannot be
//! written. Every rule about a graph lives here, so the model, the reader, the editor and the engine
//! ask one place: `check` (validation), `held_peak` (`MAX_HELD`), `trunk_len`, `reads` (a time kind
//! in an arm is a second read of the layer, the owner's Q3 b, design D2), `remove_slot` and
//! `insert_slot` (one rule for every removal and insertion), `canonical` (a line is the empty list).

use serde::{Deserialize, Serialize};

use crate::{BlendMode, FxInstance};

/// The most frames a lowered program holds at once (design D3): bounds the memory of a layer.
pub const MAX_HELD: usize = 4;
/// The most reads of its source one layer makes (design D2): each read is a decode and a cache
/// footprint, the cost the owner accepted for a time kind in a branch.
pub const MAX_READS: usize = 4;

/// Where a slot takes an input from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxIn {
    /// The slot before (the Source for slot 0): the line.
    #[default]
    Prev,
    /// The chain's input: the decoded frame after the leading Geometry node, a patch's target.
    Source,
    /// An earlier slot's output.
    Slot(u32),
}

/// How a matte input becomes a coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatteFrom {
    Alpha,
    Luma,
    InvAlpha,
    InvLuma,
}

/// A Merge's Matte input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Matte {
    pub from: FxIn,
    pub by: MatteFrom,
}

/// One slot's inputs: `main` (a Merge's Background), `second` (a Merge's Foreground), `matte`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SlotInputs {
    #[serde(default, skip_serializing_if = "is_prev")]
    pub main: FxIn,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second: Option<FxIn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matte: Option<Matte>,
}

fn is_prev(i: &FxIn) -> bool {
    *i == FxIn::Prev
}

impl SlotInputs {
    /// The line's entry: the slot before, nothing else.
    pub fn is_line(&self) -> bool {
        *self == SlotInputs::default()
    }
}

/// A Merge's mode: the picture's nine layer blend modes (their kernels, design D3), the sound's sum
/// and crossfade (design D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    Blend(BlendMode),
    Sum,
    Crossfade,
}

impl Default for MergeMode {
    fn default() -> Self {
        MergeMode::Blend(BlendMode::Normal)
    }
}

/// A node of the graph with its address resolved: the Source or a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Node {
    Source,
    Slot(usize),
}

impl Node {
    /// Position in processing order: the Source before slot 0.
    fn pos(self) -> isize {
        match self {
            Node::Source => -1,
            Node::Slot(j) => j as isize,
        }
    }
}

/// Slot `i`'s inputs (the tail of a short list is the line).
pub fn inputs(fx_in: &[SlotInputs], i: usize) -> SlotInputs {
    fx_in.get(i).copied().unwrap_or_default()
}

/// `input` of slot `i`, resolved.
pub fn resolve(i: usize, input: FxIn) -> Node {
    match input {
        FxIn::Prev if i == 0 => Node::Source,
        FxIn::Prev => Node::Slot(i - 1),
        FxIn::Source => Node::Source,
        FxIn::Slot(j) => Node::Slot(j as usize),
    }
}

/// `node` as slot `i` writes it: the slot before is `Prev`.
pub fn encode(i: usize, node: Node) -> FxIn {
    match node {
        Node::Source if i == 0 => FxIn::Prev,
        Node::Source => FxIn::Source,
        Node::Slot(j) if j + 1 == i => FxIn::Prev,
        Node::Slot(j) => FxIn::Slot(j as u32),
    }
}

/// Every input of slot `i`, resolved: (node, which socket).
pub fn reads_of(fx_in: &[SlotInputs], i: usize) -> impl Iterator<Item = (Node, Socket)> {
    let s = inputs(fx_in, i);
    std::iter::once((resolve(i, s.main), Socket::Main))
        .chain(s.second.map(|x| (resolve(i, x), Socket::Second)))
        .chain(s.matte.map(|m| (resolve(i, m.from), Socket::Matte)))
}

/// A slot's input socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Socket {
    Main,
    Second,
    Matte,
}

/// The canonical list: every entry re-encoded (`Slot(i-1)` is `Prev`), the line's tail dropped, so
/// a line is the EMPTY list and is written, hashed and compared as before graphs existed.
pub fn canonical(fx_in: &mut Vec<SlotInputs>, n: usize) {
    fx_in.truncate(n);
    for (i, s) in fx_in.iter_mut().enumerate() {
        s.main = encode(i, resolve(i, s.main));
        if let Some(x) = s.second.as_mut() {
            *x = encode(i, resolve(i, *x));
        }
        if let Some(m) = s.matte.as_mut() {
            m.from = encode(i, resolve(i, m.from));
        }
    }
    while fx_in.last().is_some_and(SlotInputs::is_line) {
        fx_in.pop();
    }
}

/// Whether the wiring is a line.
pub fn is_line(fx_in: &[SlotInputs]) -> bool {
    fx_in.iter().all(SlotInputs::is_line)
}

/// Why a graph is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphWhy {
    /// An input names this slot or a later one (it would close a loop or point forward).
    Forward,
    /// A Foreground or a Matte on a kind that has no such socket.
    SocketOnPlain,
    /// The leading Geometry node stands after a branch (it places the frame and sets the decode
    /// step for the whole layer, design D2).
    LeadInArm,
    /// More than `MAX_HELD` frames held at once.
    TooManyHeld,
    /// More than `MAX_READS` reads of the source.
    TooManyReads,
    /// The wiring list is longer than the chain.
    TooLong,
}

/// A refusal with the slot's address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphError {
    pub slot: usize,
    pub why: GraphWhy,
}

/// Validate a chain's wiring (`Project::validate`, the reader, the editor before a command).
/// `lead` is the leading Geometry node's position (`geometry_pos`; it stands before that slot).
pub fn check(fx: &[FxInstance], fx_in: &[SlotInputs], lead: usize) -> Result<(), GraphError> {
    if fx_in.is_empty() {
        return Ok(());
    }
    if fx_in.len() > fx.len() {
        return Err(GraphError { slot: fx_in.len() - 1, why: GraphWhy::TooLong });
    }
    for (i, f) in fx.iter().enumerate() {
        let s = inputs(fx_in, i);
        for (node, _) in reads_of(fx_in, i) {
            if node.pos() >= i as isize {
                return Err(GraphError { slot: i, why: GraphWhy::Forward });
            }
        }
        if (s.second.is_some() || s.matte.is_some()) && !f.is_merge() {
            return Err(GraphError { slot: i, why: GraphWhy::SocketOnPlain });
        }
    }
    if lead > 0 && trunk_len(fx_in, fx.len()) < lead {
        return Err(GraphError { slot: lead.min(fx.len().saturating_sub(1)), why: GraphWhy::LeadInArm });
    }
    if held_peak(fx_in, fx.len()) > MAX_HELD {
        return Err(GraphError { slot: fx.len().saturating_sub(1), why: GraphWhy::TooManyHeld });
    }
    if read_count(fx, fx_in) > MAX_READS {
        return Err(GraphError { slot: fx.len().saturating_sub(1), why: GraphWhy::TooManyReads });
    }
    Ok(())
}

/// The trunk: how many leading slots run before the first branch point. A value read by anything
/// but the next slot's main input is a branch point; the trunk ends with it (the Source read twice:
/// an empty trunk). A line is all trunk.
pub fn trunk_len(fx_in: &[SlotInputs], n: usize) -> usize {
    let mut t = n;
    for i in 0..n {
        for (node, socket) in reads_of(fx_in, i) {
            let trivial = socket == Socket::Main && node.pos() + 1 == i as isize;
            if !trivial {
                t = t.min((node.pos() + 1) as usize);
            }
        }
    }
    t
}

/// The slots that reach the last slot (the Output) through some path; the others dangle (kept,
/// drawn dim, not run).
pub fn reaches_output(fx_in: &[SlotInputs], n: usize) -> Vec<bool> {
    let mut live = vec![false; n];
    if n == 0 {
        return live;
    }
    live[n - 1] = true;
    for i in (0..n).rev() {
        if !live[i] {
            continue;
        }
        for (node, _) in reads_of(fx_in, i) {
            if let Node::Slot(j) = node {
                live[j] = true;
            }
        }
    }
    live
}

/// The most values held at once by the lowered program (design D3): a value read by anything but
/// the next slot's main input is held from where it is made until its last reader.
pub fn held_peak(fx_in: &[SlotInputs], n: usize) -> usize {
    if is_line(fx_in) {
        return 0;
    }
    // value index: -1 the Source, j a slot; (made at, last read, held)
    let mut last = std::collections::BTreeMap::<isize, (isize, bool)>::new();
    for i in 0..n {
        for (node, socket) in reads_of(fx_in, i) {
            let v = node.pos();
            let trivial = socket == Socket::Main && v + 1 == i as isize;
            let e = last.entry(v).or_insert((i as isize, false));
            e.0 = e.0.max(i as isize);
            e.1 |= !trivial;
        }
    }
    let mut peak = 0;
    for p in -1..n as isize {
        let held = last.iter().filter(|(v, (end, held))| *held && **v <= p && p < *end).count();
        peak = peak.max(held);
    }
    peak
}

/// The ancestors of slot `t` (the slots its inputs reach back to), with `t`, ascending.
pub fn ancestors(fx_in: &[SlotInputs], t: usize) -> Vec<usize> {
    let mut keep = vec![false; t + 1];
    keep[t] = true;
    for i in (0..=t).rev() {
        if !keep[i] {
            continue;
        }
        for (node, _) in reads_of(fx_in, i) {
            if let Node::Slot(j) = node {
                keep[j] = true;
            }
        }
    }
    (0..=t).filter(|i| keep[*i]).collect()
}

/// The derived chain made of `keep` (ascending original slots): its wiring re-addressed, an input
/// from outside it becomes the Source. The slots themselves are the caller's to pick.
pub fn derive(fx_in: &[SlotInputs], keep: &[usize]) -> Vec<SlotInputs> {
    let at = |node: Node, i: usize| -> FxIn {
        let n = match node {
            Node::Source => Node::Source,
            Node::Slot(j) => keep.iter().position(|k| *k == j).map_or(Node::Source, Node::Slot),
        };
        encode(i, n)
    };
    let mut out: Vec<SlotInputs> = keep
        .iter()
        .enumerate()
        .map(|(i, &k)| {
            let s = inputs(fx_in, k);
            SlotInputs {
                main: at(resolve(k, s.main), i),
                second: s.second.map(|x| at(resolve(k, x), i)),
                matte: s.matte.map(|m| Matte { from: at(resolve(k, m.from), i), by: m.by }),
            }
        })
        .collect();
    canonical(&mut out, keep.len());
    out
}

/// A read of the layer's source opened by a time kind in an arm (design D2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// The time kind's slot: the read's Output.
    pub slot: usize,
    /// The derived chain's slots (the time kind's ancestors and itself), ascending.
    pub keep: Vec<usize>,
    /// The derived chain's wiring.
    pub fx_in: Vec<SlotInputs>,
    /// The reads the derived chain opens itself (a time kind in ITS arms).
    pub nested: Vec<Read>,
}

/// The reads a chain opens beyond its own (read 0): one per time kind that stands in an arm and
/// reaches the Output. A graph with none reads one source, as its line does.
pub fn reads(fx: &[FxInstance], fx_in: &[SlotInputs]) -> Vec<Read> {
    reads_owned(fx, fx_in, &(0..fx.len()).collect::<Vec<_>>(), None)
}

/// `idx` maps the derived chain's slots to `fx`; `own` is the derived chain's own time kind (its
/// Output), which decides how the read reads and opens nothing again.
fn reads_owned(fx: &[FxInstance], fx_in: &[SlotInputs], idx: &[usize], own: Option<usize>) -> Vec<Read> {
    if is_line(fx_in) {
        return Vec::new();
    }
    let n = idx.len();
    let trunk = trunk_len(fx_in, n);
    let live = reaches_output(fx_in, n);
    let mut out = Vec::new();
    for t in trunk..n {
        if Some(t) == own || !live[t] || !fx[idx[t]].is_time_kind() {
            continue;
        }
        let keep = ancestors(fx_in, t);
        let sub = derive(fx_in, &keep);
        let sub_idx: Vec<usize> = keep.iter().map(|k| idx[*k]).collect();
        let nested = reads_owned(fx, &sub, &sub_idx, Some(keep.len() - 1));
        out.push(Read { slot: idx[t], keep: sub_idx, fx_in: sub, nested });
    }
    out
}

/// How many reads the layer makes: its own and every nested one.
pub fn read_count(fx: &[FxInstance], fx_in: &[SlotInputs]) -> usize {
    fn count(r: &[Read]) -> usize {
        r.iter().map(|r| 1 + count(&r.nested)).sum()
    }
    1 + count(&reads(fx, fx_in))
}

/// Remove slot `i` from the wiring (ONE rule for every removal, design D6: the cross, Delete, the
/// pair mirror, a bake): a slot that read `i` reads `i`'s own main input instead, and every later
/// address moves down by one. The caller removes the slot from `fx` and its parallel lists.
pub fn remove_slot(fx_in: &mut Vec<SlotInputs>, n: usize, i: usize) {
    if fx_in.is_empty() || i >= n {
        return;
    }
    let gone = resolve(i, inputs(fx_in, i).main);
    let fix = |node: Node| -> Node {
        match node {
            Node::Slot(j) if j == i => gone,
            Node::Slot(j) if j > i => Node::Slot(j - 1),
            other => other,
        }
    };
    let mut out = Vec::with_capacity(n - 1);
    for k in (0..n).filter(|k| *k != i) {
        let s = inputs(fx_in, k);
        let at = out.len();
        out.push(SlotInputs {
            main: encode(at, fix(resolve(k, s.main))),
            second: s.second.map(|x| encode(at, fix(resolve(k, x)))),
            matte: s.matte.map(|m| Matte { from: encode(at, fix(resolve(k, m.from))), by: m.by }),
        });
    }
    canonical(&mut out, n - 1);
    *fx_in = out;
}

/// Insert a slot at `i` ON THE WIRE into slot `i` (design D8, "insert on a wire"): the new slot takes
/// the old slot `i`'s main input, the old slot reads the new one, every later address moves up. A
/// line stays a line. The caller inserts into `fx` and its parallel lists.
pub fn insert_slot(fx_in: &mut Vec<SlotInputs>, n: usize, i: usize) {
    if fx_in.is_empty() {
        return;
    }
    let i = i.min(n);
    let up = |node: Node| -> Node {
        match node {
            Node::Slot(j) if j >= i => Node::Slot(j + 1),
            other => other,
        }
    };
    let mut out = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let at = k;
        if k == i {
            let main = if i < n { resolve(i, inputs(fx_in, i).main) } else { resolve(i, FxIn::Prev) };
            out.push(SlotInputs { main: encode(at, up(main)), second: None, matte: None });
            continue;
        }
        let o = if k < i { k } else { k - 1 };
        let s = inputs(fx_in, o);
        let main = if o == i { Node::Slot(i) } else { up(resolve(o, s.main)) };
        out.push(SlotInputs {
            main: encode(at, main),
            second: s.second.map(|x| encode(at, up(resolve(o, x)))),
            matte: s.matte.map(|m| Matte { from: encode(at, up(resolve(o, m.from))), by: m.by }),
        });
    }
    canonical(&mut out, n + 1);
    *fx_in = out;
}

/// A stable topological order of `n` slots whose inputs are `edges(i)` (any slots, possibly later):
/// the editor's wire that points forward re-sorts the slots with it (design D1). `None` — a loop.
/// Stable: among the slots ready at a step, the lowest current index goes first, so a valid order
/// is returned unchanged.
pub fn topo_order(n: usize, edges: impl Fn(usize) -> Vec<usize>) -> Option<Vec<usize>> {
    let mut indeg = vec![0usize; n];
    let mut readers = vec![Vec::new(); n];
    for (i, deg) in indeg.iter_mut().enumerate() {
        for j in edges(i) {
            if j >= n {
                return None;
            }
            *deg += 1;
            readers[j].push(i);
        }
    }
    let mut ready: std::collections::BTreeSet<usize> = (0..n).filter(|i| indeg[*i] == 0).collect();
    let mut out = Vec::with_capacity(n);
    while let Some(&i) = ready.iter().next() {
        ready.remove(&i);
        out.push(i);
        for &r in &readers[i] {
            indeg[r] -= 1;
            if indeg[r] == 0 {
                ready.insert(r);
            }
        }
    }
    (out.len() == n).then_some(out)
}

/// The wiring of the chain without the slots `drop` names (bypassed, inert), each removed by the one
/// rule (`remove_slot`): a reader of a dropped slot reads its input, as a bypassed slot passes it.
pub fn without(fx_in: &[SlotInputs], n: usize, drop: impl Fn(usize) -> bool) -> Vec<SlotInputs> {
    let mut out = fx_in.to_vec();
    if out.is_empty() {
        return out;
    }
    let mut len = n;
    for i in (0..n).rev() {
        if drop(i) {
            remove_slot(&mut out, len, i);
            len -= 1;
        }
    }
    out
}

/// Splice a patch's wiring after a target of `n0` slots (design D6): the patch's Source is the
/// target's Output, its slot addresses move by `n0`; `m` is the patch's length. A line spliced after
/// a line stays the empty list.
pub fn splice(target: &mut Vec<SlotInputs>, n0: usize, patch: &[SlotInputs], m: usize) {
    if patch.is_empty() {
        return;
    }
    let out_of = |node: Node| -> Node {
        match node {
            Node::Source if n0 == 0 => Node::Source,
            Node::Source => Node::Slot(n0 - 1),
            Node::Slot(j) => Node::Slot(n0 + j),
        }
    };
    target.resize(n0, SlotInputs::default());
    for j in 0..m {
        let s = inputs(patch, j);
        let at = n0 + j;
        target.push(SlotInputs {
            main: encode(at, out_of(resolve(j, s.main))),
            second: s.second.map(|x| encode(at, out_of(resolve(j, x)))),
            matte: s.matte.map(|mt| Matte { from: encode(at, out_of(resolve(j, mt.from))), by: mt.by }),
        });
    }
    canonical(target, n0 + m);
}
