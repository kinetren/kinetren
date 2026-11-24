//! The node editor of an effect chain (fx-graph v1b; capability `ui-reach`).
//!
//! The owner, kkt-26.11.18b items 3 and U7–U8: the chain strip's control was clumsy, "нужно что-то
//! типо компактного удобного графа", "везде в окнах разных единым стилем в одном месте чтобы не
//! скролить … типо как в blender", "2-3 ползунка, но … как уместить огромные эффекты типо isp".
//! It replaces `chain_strip` everywhere a chain is edited (the effects window, the morpher, the
//! track) and edits the SAME vectors through the SAME `ChainEdit`: it adds no model.
//!
//! - **Lanes.** A chain of both kinds (the morpher) is two lanes, picture over sound; a single-kind
//!   chain is one. A slot never leaves its lane: a drag onto the other lane is refused with a reason.
//! - **A device is one node.** The two halves of a simulated device stand in one column inside one
//!   frame, each half with its own switch (fx-graph D2); halves standing in crossing orders are tied
//!   by a dotted line instead.
//! - **Laid out by order, nothing stored** (his Q1): the columns are the processing order. A node
//!   shows at most three main rows (`kt_plugin_api::main_rows`, his Q2); the whole form of the
//!   selected node stays below the graph.
//! - **It fits without scrolling** — by LEVEL OF DETAIL, not by scaling the drawing: the detailed
//!   nodes when they fit the width, else small chips wrapped into bands. The wheel over the graph
//!   switches detail ↔ chips (it is consumed only when it switches), a middle drag pans the detail,
//!   Fit or a double click on empty space returns to the fit. A scaled layer would blur the text and
//!   egui's accessibility rectangles ignore a layer transform — the harness and the live scenarios
//!   press nodes by those rectangles.
//! - **At rest it costs nothing**: no animation, no repaint request; the layout is a few
//!   rectangles a frame and only while the window draws.
//!
//! No new keys in v1b: M/X/Home would collide with the timeline's keys (marker, delete, start);
//! they join the one key table (`controls.rs`) with v1c.

use crate::i18n::{k, tr};
use crate::widgets::{ChainChanged, ChainCtx, ChainEdit, ChainSel, SLOT_GRAB_PAD};

use eframe::egui;
use kt_model::FxInstance;
use kt_model::fx_graph::{GraphWhy, MatteFrom, MergeMode, Node, Socket};
use kt_model::fx_strip::{Strip, To};
use crate::graph_layout::{Edge, LaneLayout, Via};

/// Width of a detailed node: a main row is a label, a number and a short slider.
const NODE_W: f32 = 212.0;
/// Width of a detailed node without main rows (the geometry node, the layer strength, an effect
/// with no number): its name, its switch and its cross.
const NARROW_W: f32 = 132.0;
/// The wire between two columns.
const GAP: f32 = 20.0;
/// The In / Out ends of a lane.
const END_W: f32 = 36.0;
/// The source head (a morpher's swap, a generator's source).
const HEAD_W: f32 = 168.0;
const HEADER_H: f32 = 22.0;
const ROW_H: f32 = 22.0;
/// The label of a main row in a node: elided, the full name is in the form below.
const LABEL_W: f32 = 70.0;
const NODE_PAD: f32 = 2.0;
const LANE_GAP: f32 = 12.0;
const CHIP_W: f32 = 124.0;
const CHIP_H: f32 = 22.0;
const CHIP_GAP: f32 = 12.0;
const PAD: f32 = 3.0;
/// The most of the window's visible height the detailed graph opens with; past it, chips.
const DETAIL_SHARE: f32 = 0.4;
/// A context box (a layer over the event, its track).
const CTX_W: f32 = 150.0;
/// A sidechain key's box under the lane.
const KEY_H: f32 = 18.0;
/// Between the rows of a lane that branches.
const ARM_GAP: f32 = 8.0;
/// A wire no row carries runs on a track of this height under the rows.
const TRACK_H: f32 = 8.0;
/// Below the graph a dropped wire still starts a branch: a graph of one row has no empty space.
const BRANCH_ZONE: f32 = 44.0;
/// How near a wire a dropped node lands on it.
const WIRE_GRAB: f32 = 9.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lane {
    Picture,
    Sound,
}

/// The view of one graph, kept for the window's session (egui's temporary memory: nothing is
/// stored in the project). `detail: None` — the fit decides.
#[derive(Clone, Copy, Default, Debug)]
struct View {
    detail: Option<bool>,
    pan: f32,
}

/// A command of the key table acting on the graph under the pointer (fx-graph v1c): the table's
/// own keys, scoped by where the pointer is, as the mask keys are on the preview — no new key and
/// no second meaning to learn. `KtApp::graph_keys` takes them before the timeline does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum GraphKey {
    /// Delete: the selected node goes (a device: both halves, the model's rule)
    Remove,
    /// the mute key: the selected node's switch
    Mute,
    /// zoom to fit: Frame all
    Fit,
    /// zoom in / out: the detailed nodes / the chips
    Detail,
    Chips,
}

/// Where a graph was under the pointer in the frame before: (its view, its canvas, the frame).
pub(crate) fn hover_slot() -> egui::Id {
    egui::Id::new("kt_fx_graph_hover")
}

/// A command for the graph of a view, left by `KtApp::graph_keys` for the graph's next draw.
pub(crate) fn key_slot() -> egui::Id {
    egui::Id::new("kt_fx_graph_key")
}

/// A column of the layout: the slot on each lane (picture, sound).
type Col = [Option<usize>; 2];

fn lane_of(f: &FxInstance) -> Lane {
    if f.class() == kt_model::FxClass::Audio { Lane::Sound } else { Lane::Picture }
}

fn lane_color(l: Lane) -> egui::Color32 {
    match l {
        Lane::Picture => egui::Color32::from_rgb(214, 178, 64),
        Lane::Sound => egui::Color32::from_rgb(88, 182, 112),
    }
}

/// The halves of simulated devices: picture slot → sound slot, the n-th occurrence of a kind to the
/// n-th of its counterpart (the model's rule, `kt_model::model::mirror_halves`).
fn device_pairs(fx: &[FxInstance], pic: &[usize], snd: &[usize]) -> Vec<(usize, usize)> {
    let reg = kt_model::plugin_slot::registry();
    let mut used = vec![false; snd.len()];
    let mut out = Vec::new();
    for &p in pic {
        let FxInstance::Plugin(slot) = &fx[p] else { continue };
        let Some(other) = reg.counterpart(&slot.id) else { continue };
        if let Some(k) = snd
            .iter()
            .enumerate()
            .position(|(k, &s)| !used[k] && matches!(&fx[s], FxInstance::Plugin(q) if q.id == other))
        {
            used[k] = true;
            out.push((p, snd[k]));
        }
    }
    out
}

/// The columns: both lanes in their own order, a device's halves in one column when the orders
/// allow it; two unrelated slots share a column (compact).
fn columns(pic: &[usize], snd: &[usize], pairs: &[(usize, usize)]) -> Vec<Col> {
    let pos = |list: &[usize], i: usize| list.iter().position(|&x| x == i);
    let partner_of_pic = |p: usize| pairs.iter().find(|(a, _)| *a == p).map(|(_, b)| *b);
    let partner_of_snd = |s: usize| pairs.iter().find(|(_, b)| *b == s).map(|(a, _)| *a);
    let (mut a, mut b) = (0, 0);
    let mut cols = Vec::new();
    while a < pic.len() || b < snd.len() {
        match (pic.get(a).copied(), snd.get(b).copied()) {
            (Some(p), Some(s)) => {
                let waits_p = partner_of_pic(p).and_then(|x| pos(snd, x)).is_some_and(|x| x > b);
                let waits_s = partner_of_snd(s).and_then(|x| pos(pic, x)).is_some_and(|x| x > a);
                if partner_of_pic(p) == Some(s) || (!waits_p && !waits_s) {
                    cols.push([Some(p), Some(s)]);
                    a += 1;
                    b += 1;
                } else if waits_p && !waits_s {
                    cols.push([None, Some(s)]);
                    b += 1;
                } else {
                    cols.push([Some(p), None]);
                    a += 1;
                }
            }
            (Some(p), None) => {
                cols.push([Some(p), None]);
                a += 1;
            }
            (None, Some(s)) => {
                cols.push([None, Some(s)]);
                b += 1;
            }
            (None, None) => break,
        }
    }
    cols
}

/// The rows a node of this slot shows: the plugin's main set (fx-graph D4a), nothing for the
/// host's own nodes (geometry, the layer strength — the form edits them).
fn main_specs(f: &FxInstance) -> Vec<kt_model::ParamSpec> {
    if f.is_merge() {
        return f.param_specs().to_vec();
    }
    let FxInstance::Plugin(p) = f else { return Vec::new() };
    let names = kt_plugin_api::main_rows(p.manifest());
    names.iter().filter_map(|n| f.param_specs().iter().find(|s| s.name == *n).cloned()).collect()
}

/// A choice row's caption: the panels' own catalogue entries, else the key itself (the codec
/// sizes and bitrates are numbers).
fn choice_caption(row: &str, key: &str) -> String {
    let deck = match (row, key) {
        ("speed", "sp") => Some(tr(k::deck::SP)),
        ("speed", "lp") => Some(tr(k::deck::LP)),
        ("speed", "ep") => Some(tr(k::deck::EP)),
        ("audio_out", "hifi") => Some(tr(k::deck::HIFI)),
        ("audio_out", "normal") => Some(tr(k::deck::NORMAL)),
        ("audio_out", "mix") => Some(tr(k::deck::MIX)),
        _ => None,
    };
    deck.or_else(|| crate::isp_panel::choice_label(row, key)).map(str::to_string).unwrap_or_else(|| key.to_string())
}

/// A node dragged by its name.
#[derive(Clone, Copy)]
struct NodeDrag {
    view: egui::Id,
    slot: usize,
    lane: u8,
}

/// A wire dragged from an output (`was: None`) or off the socket it stands in.
#[derive(Clone, Copy)]
struct WireDrag {
    view: egui::Id,
    lane: u8,
    from: Node,
    was: Option<(usize, Socket)>,
}

/// A branch waiting for its effect: the picker stands where the wire was dropped.
#[derive(Clone, Copy)]
struct Pending {
    lane: u8,
    from: Node,
    at: egui::Pos2,
}

/// A slot with what belongs to it.
type Item = (FxInstance, bool, Vec<kt_model::ParamCurve>);

/// One gesture on the graph (fx-graph-dag D8): every edit of the slots and the wires is one of these.
enum Op {
    /// The picker, a preset or a paste: after the selected node of a lane that branches, else at the
    /// end. `fx_in` — the block's own wiring.
    Add { items: Vec<Item>, fx_in: Vec<kt_model::SlotInputs> },
    /// A new arm.
    Branch { lane: u8, from: Node, items: Vec<Item>, fx_in: Vec<kt_model::SlotInputs> },
    /// A Merge before the Output; `from` goes to its Foreground.
    Merge { lane: u8, from: Option<Node> },
    Remove(usize),
    Shift(usize, To),
    Connect(usize, Socket, Node),
    Disconnect(usize, Socket),
    /// The Output reads this slot.
    Out(usize),
    /// What a Merge's matte is made of.
    By(usize, MatteFrom),
}

/// What a layout is made of: the wiring, the devices, the lanes, the time kinds.
type LaidKey = (Strip, Vec<(usize, usize)>, bool, Vec<bool>);

/// The layout of a graph: a column per slot, the lanes' rows and wires, the slots that reach the
/// Output, the slots that read the source again.
struct Laid {
    col: Vec<usize>,
    lanes: Vec<LaneLayout>,
    live: Vec<bool>,
    reads: Vec<usize>,
}

/// The slot new effects go after: the selected one of a lane that branches, unless it is the Output.
fn add_after(s: &Strip, lane: u8, sel: Option<usize>) -> Option<usize> {
    sel.filter(|i| s.lane.get(*i) == Some(&lane) && !s.is_line(lane) && s.out[usize::from(lane == 1)] != Some(*i))
}

/// Why a gesture was refused, in words (rule 16).
fn why_text(why: GraphWhy) -> String {
    use kt_model::fx_graph::{MAX_HELD, MAX_READS};
    match why {
        GraphWhy::Forward => tr(k::fx::graph::WHY_LOOP).to_string(),
        GraphWhy::SocketOnPlain | GraphWhy::TooLong => tr(k::fx::graph::WHY_SOCKET).to_string(),
        GraphWhy::LeadInArm => tr(k::fx::graph::WHY_LEAD).to_string(),
        GraphWhy::TooManyHeld => crate::tf!(k::fx::graph::WHY_HELD, MAX_HELD),
        GraphWhy::TooManyReads => crate::tf!(k::fx::graph::WHY_READS, MAX_READS),
        GraphWhy::MatteOnSound => tr(k::fx::graph::WHY_MATTE_SOUND).to_string(),
        GraphWhy::ReadBranches => tr(k::fx::graph::WHY_READ_BRANCHES).to_string(),
        GraphWhy::ModeOnLane => tr(k::fx::graph::WHY_MODE).to_string(),
    }
}

/// The graph a gesture would leave, checked by the model's own rules lane by lane.
fn check(fx: &[FxInstance], s: &Strip, sound: impl Fn(u8) -> bool) -> Result<(), GraphWhy> {
    for lane in [0u8, 1] {
        let mut of: Vec<FxInstance> = (0..fx.len()).filter(|i| s.lane[*i] == lane).map(|i| fx[i].clone()).collect();
        let mut w = s.wiring(lane);
        let mut pos = 0;
        if let Some(l) = of.iter().position(|f| f.is_lead_geometry()) {
            w = kt_model::fx_strip::without_lead(&w, of.len(), l)?;
            of.remove(l);
            pos = l;
        }
        kt_model::fx_graph::check_lane(&of, &w, pos, sound(lane)).map_err(|e| e.why)?;
    }
    Ok(())
}

/// The slots of `lane` that open a second read of the source (fx-graph-dag D2).
fn read_slots(fx: &[FxInstance], s: &Strip, lane: u8) -> Vec<usize> {
    if s.is_line(lane) {
        return Vec::new();
    }
    let mine: Vec<usize> = (0..fx.len()).filter(|i| s.lane[*i] == lane).collect();
    let of: Vec<FxInstance> = mine.iter().map(|i| fx[*i].clone()).collect();
    kt_model::fx_graph::reads(&of, &s.wiring(lane)).iter().map(|r| mine[r.slot]).collect()
}

/// Carry out a gesture: wire, order, check, and only then write — a refusal changes nothing.
fn perform(op: Op, edit: &mut ChainEdit<'_>, lanes: &[u8], sound: impl Fn(u8) -> bool, kind_lane: impl Fn(&FxInstance) -> u8, ch: &mut ChainChanged) {
    let n = edit.fx.len();
    let old = Strip::new(edit.links, lanes);
    let mut s = old.clone();
    let mut added: Vec<Item> = Vec::new();
    // the slot the gesture leaves selected, by its number in `Strip::from`
    let mut select = edit.selected.fx();
    let mut what = String::new();
    match op {
        Op::Add { items, fx_in } => {
            let of: Vec<u8> = items.iter().map(|(f, _, _)| kind_lane(f)).collect();
            let wired = if fx_in.is_empty() { kt_model::fx_strip::line_links(&of) } else { kt_model::fx_strip::links_of(&fx_in, of.len(), 0) };
            for lane in [0u8, 1] {
                let mine: Vec<Item> = items.iter().zip(&of).filter(|(_, l)| **l == lane).map(|(x, _)| x.clone()).collect();
                let w = kt_model::fx_strip::wiring_of(&wired, &of, lane);
                let after = add_after(&old, lane, edit.selected.fx());
                s.block(lane, after, &w, mine.len());
                added.extend(mine);
            }
            select = (!added.is_empty()).then_some(n + added.len().saturating_sub(1)).or(select);
        }
        Op::Branch { lane, from, items, fx_in } => {
            what = "a branch".into();
            let Some(first) = items.first().cloned() else { return };
            if fx_in.is_empty() && items.len() == 1 {
                s.branch(lane, from);
                added.push(first);
            } else {
                // a preset as an arm: its first slot starts it, the block's wiring follows
                let base = s.links.len();
                let out = s.out;
                s.block(lane, None, &fx_in, items.len());
                let src = out[usize::from(lane == 1)].map_or(Node::Source, Node::Slot);
                if from != src {
                    for i in base..s.links.len() {
                        for (x, sk) in s.links[i].each().collect::<Vec<_>>() {
                            if x == src {
                                s.connect(i, sk, from);
                            }
                        }
                    }
                    s.out = out;
                }
                added = items;
            }
            select = Some(n + added.len() - 1);
        }
        Op::Merge { lane, from } => {
            what = "a Merge".into();
            s.merge(lane, from.unwrap_or(s.out[usize::from(lane == 1)].map_or(Node::Source, Node::Slot)));
            let mode = if sound(lane) { MergeMode::Sum } else { MergeMode::default() };
            added.push((FxInstance::Merge { mode, mix: 1.0 }, false, Vec::new()));
            select = Some(n);
        }
        Op::Remove(i) => s.remove(i),
        Op::Shift(i, to) => {
            s.shift(i, to);
            select = Some(i);
        }
        Op::Connect(i, socket, node) => {
            what = "a wire".into();
            s.connect(i, socket, node);
        }
        Op::Disconnect(i, socket) => {
            what = "a wire off".into();
            s.disconnect(i, socket);
        }
        Op::Out(i) => {
            what = "the Output".into();
            s.set_out(i);
        }
        Op::By(i, by) => {
            if let Some(m) = s.links[i].matte.as_mut() {
                m.1 = by;
            }
        }
    }
    if s == old {
        return;
    }
    let slot = |id: usize, edit: &ChainEdit<'_>| -> Item {
        match id.checked_sub(n) {
            Some(k) => added[k].clone(),
            None => (edit.fx[id].clone(), edit.bypass[id], edit.auto.get(id).cloned().unwrap_or_default()),
        }
    };
    let settled = s.settle().map(|_| ());
    let slots: Vec<Item> = s.from.iter().map(|id| slot(*id, edit)).collect();
    let fx: Vec<FxInstance> = slots.iter().map(|x| x.0.clone()).collect();
    if let Err(why) = settled.and_then(|()| check(&fx, &s, sound)) {
        ch.refused = Some(why_text(why));
        return;
    }
    let line = |s: &Strip| s.is_line(0) && s.is_line(1);
    let wires = !line(&old) || !line(&s);
    let appended = s.from.len() >= n && s.from.iter().take(n).copied().eq(0..n);
    if wires && !what.is_empty() {
        crate::tele_journal::record(crate::tele_journal::Kind::Act, &format!("graph: {what}"));
    }
    ch.wires |= wires;
    if s.from.len() != n || !appended {
        ch.fx = true;
        // a slot added at the end moves nothing that stood: its switch and its curves go only when
        // it brought them (a preset, a paste)
        let brought = added.iter().any(|(_, off, c)| *off || !c.is_empty());
        ch.bypass |= !appended || brought || added.len() > 1;
        ch.auto |= !appended || brought || added.len() > 1;
        if !appended {
            ch.from = Some(s.from.iter().map(|id| Some(*id).filter(|id| *id < n)).collect());
        }
    }
    edit.bypass.clear();
    edit.auto.clear();
    for (_, off, curves) in slots {
        edit.bypass.push(off);
        edit.auto.push(curves);
    }
    *edit.fx = fx;
    if let Some(l) = edit.lane_of.as_deref_mut() {
        *l = s.lane.clone();
    }
    *edit.links = s.links;
    if let Some(sel) = select {
        // the selected slot where it stands now; one removed leaves its place to the next
        *edit.selected = match s.from.iter().position(|id| *id == sel) {
            Some(at) => ChainSel::Fx(at),
            None if edit.fx.is_empty() => ChainSel::Head,
            None => ChainSel::Fx(sel.min(edit.fx.len() - 1)),
        };
    }
}

/// The points of a wire along its way (`y` — the row or track it runs on; `wrap` — to another band of
/// chips, its socket's number keeping it beside the wires that go the same way).
fn wire_points(a: egui::Pos2, b: egui::Pos2, y: Option<f32>, gap: f32, wrap: Option<usize>) -> Vec<egui::Pos2> {
    let curve = |a: egui::Pos2, b: egui::Pos2, out: &mut Vec<egui::Pos2>| {
        let dx = ((b.x - a.x) * 0.5).max(gap * 0.6);
        let (c, d) = (egui::pos2(a.x + dx, a.y), egui::pos2(b.x - dx, b.y));
        for n in 0..=10 {
            let t = n as f32 / 10.0;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            out.push(egui::pos2(
                w[0] * a.x + w[1] * c.x + w[2] * d.x + w[3] * b.x,
                w[0] * a.y + w[1] * c.y + w[2] * d.y + w[3] * b.y,
            ));
        }
    };
    let mut out = Vec::new();
    if b.x < a.x - 1.0 || wrap.is_some() {
        // a wrap to the next band of chips: down the column gap, back along the gap above the
        // destination's row, never across a node
        let k = wrap.unwrap_or(0) as f32 * 3.5;
        let gx = a.x + CHIP_GAP * 0.5 - k * 0.6;
        let gy = b.y - CHIP_H * 0.5 - LANE_GAP * 0.5 - k;
        let bx = b.x - CHIP_GAP * 0.5 + k * 0.6;
        return vec![a, egui::pos2(gx, a.y), egui::pos2(gx, gy), egui::pos2(bx, gy), egui::pos2(bx, b.y), b];
    }
    match y {
        Some(y) if b.x - a.x > gap * 2.0 => {
            curve(a, egui::pos2(a.x + gap, y), &mut out);
            curve(egui::pos2(b.x - gap, y), b, &mut out);
        }
        _ => curve(a, b, &mut out),
    }
    out
}

/// The distance of `p` to a wire.
fn off_wire(p: egui::Pos2, pts: &[egui::Pos2]) -> f32 {
    pts.windows(2)
        .map(|w| {
            let (a, d) = (w[0], w[1] - w[0]);
            let t = if d.length_sq() > 0.0 { ((p - a).dot(d) / d.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
            (a + d * t).distance(p)
        })
        .fold(f32::INFINITY, f32::min)
}

/// The node graph of a chain — the drop-in successor of the chain strip, with its signature.
#[allow(clippy::too_many_arguments)]
pub fn chain_graph(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    mut edit: ChainEdit<'_>,
    kinds: impl Into<crate::fx_registry::PickKinds>,
    query: &mut String,
    clipboard: &mut Option<kt_model::ops::FxChainBlock>,
    src_in: kt_time::Tick,
    hover: &mut Option<String>,
    ctx: &mut ChainCtx<'_>,
    mut preview: Option<&mut crate::fx_thumbs::PickerCtx<'_>>,
) -> ChainChanged {
    let kinds = kinds.into();
    // two lanes: a chain of both kinds (the morpher), or a pair's virtual chain with its slots'
    // lanes given (fx-graph D10: the video event's slowed stays on the picture lane though it is
    // an audio kind)
    let both = matches!(kinds, crate::fx_registry::PickKinds::Both) || edit.lane_of.is_some();
    let mut ch = ChainChanged::default();
    edit.bypass.resize(edit.fx.len(), false);
    edit.auto.resize(edit.fx.len(), Vec::new());
    let view_id = egui::Id::new((&id, "fx_graph_view"));
    let mut view: View = ui.data(|d| d.get_temp(view_id)).unwrap_or_default();
    // a key of the table pressed over this graph in the frame before
    let mut key_remove: Option<usize> = None;
    if let Some((vid, key)) = ui.data(|d| d.get_temp::<(egui::Id, GraphKey)>(key_slot()))
        && vid == view_id
    {
        ui.data_mut(|d| d.remove::<(egui::Id, GraphKey)>(key_slot()));
        match (key, edit.selected.fx()) {
            (GraphKey::Remove, Some(i)) if i < edit.fx.len() && !edit.fx[i].is_lead_geometry() => key_remove = Some(i),
            (GraphKey::Mute, Some(i)) if i < edit.bypass.len() => {
                edit.bypass[i] = !edit.bypass[i];
                ch.bypass = true;
            }
            (GraphKey::Fit, _) => view = View::default(),
            (GraphKey::Detail, _) => view.detail = Some(true),
            (GraphKey::Chips, _) => view.detail = Some(false),
            _ => {}
        }
    }

    // the lanes
    let single = match kinds {
        crate::fx_registry::PickKinds::Audio => Lane::Sound,
        _ => Lane::Picture,
    };
    let sound = move |lane: u8| if both { lane == 1 } else { single == Lane::Sound };
    let kind_lane = move |f: &FxInstance| u8::from(both && lane_of(f) == Lane::Sound);
    if edit.links.len() != edit.fx.len() {
        // an owner that changed its slots without the wires: the chain as a line
        let of: Vec<u8> = edit.fx.iter().map(kind_lane).collect();
        *edit.links = kt_model::fx_strip::line_links(&of);
    }
    let lanes_now = |edit: &ChainEdit<'_>| -> Vec<u8> {
        match edit.lane_of.as_deref() {
            Some(l) => (0..edit.fx.len()).map(|i| l.get(i).copied().unwrap_or_else(|| kind_lane(&edit.fx[i]))).collect(),
            None => edit.fx.iter().map(kind_lane).collect(),
        }
    };
    let before = lanes_now(&edit);
    if let Some(op) = toolbar(ui, &mut edit, &before, kinds, query, clipboard, src_in, hover, preview.as_deref_mut(), ctx, &mut ch, &mut view) {
        perform(op, &mut edit, &before, sound, kind_lane, &mut ch);
    }

    let (mut pic, mut snd) = (Vec::new(), Vec::new());
    // a single-kind chain is ONE lane, the layout's first (its kind is only its colour): a sound
    // chain put into the second slot was never laid out, and an audio event's nodes did not show
    // (found-defects 811)
    let of = lanes_now(&edit);
    for (i, l) in of.iter().enumerate() {
        if both && *l == 1 { snd.push(i) } else { pic.push(i) }
    }
    let strip = Strip::new(edit.links, &of);
    let lanes: Vec<Lane> = if both { vec![Lane::Picture, Lane::Sound] } else { vec![single] };
    let pairs = if both { device_pairs(edit.fx, &pic, &snd) } else { Vec::new() };
    let lane_slots = |l: Lane| if l == Lane::Picture || !both { 0 } else { 1 };
    // the layout: a chain that does not branch is one row in its order, as ever; one that does has
    // its arms as rows under it (fx-graph-dag D8)
    let memo_id = view_id.with("laid");
    let key = (strip.clone(), pairs.clone(), both, edit.fx.iter().map(|f| f.is_time_kind()).collect::<Vec<bool>>());
    let kept = ui.data(|d| d.get_temp::<std::sync::Arc<(LaidKey, Laid)>>(memo_id)).filter(|m| m.0 == key);
    // laid out once per wiring: a hover draws what is kept
    let plan = kept.unwrap_or_else(|| {
        let col: Vec<usize> = if strip.is_line(0) && strip.is_line(1) {
            let mut col = vec![0; edit.fx.len()];
            for (c, slots) in columns(&pic, &snd, &pairs).iter().enumerate() {
                for i in slots.iter().flatten() {
                    col[*i] = c;
                }
            }
            col
        } else {
            crate::graph_layout::columns(&strip, &pairs)
        };
        let lane_ids: Vec<u8> = if both { vec![0, 1] } else { vec![0] };
        let lanes: Vec<LaneLayout> = lane_ids.iter().map(|l| crate::graph_layout::lane(&strip, *l, &col)).collect();
        let mut live = vec![false; edit.fx.len()];
        for l in &lane_ids {
            for (a, b) in live.iter_mut().zip(strip.live(*l)) {
                *a |= b;
            }
        }
        let reads = lane_ids.iter().flat_map(|l| read_slots(edit.fx, &strip, *l)).collect();
        let m = std::sync::Arc::new((key, Laid { col, lanes, live, reads }));
        ui.data_mut(|d| d.insert_temp(memo_id, m.clone()));
        m
    });
    let Laid { col, lanes: laid, live, reads } = &plan.1;
    let n_fx = col.iter().map(|c| c + 1).max().unwrap_or(0);
    // rows a node has beyond its parameters: a Merge's mode and matte, a second read's word
    let extra_rows = |edit: &ChainEdit<'_>, i: usize| -> usize {
        let merge = if edit.fx[i].is_merge() { 1 + usize::from(strip.links[i].matte.is_some()) } else { 0 };
        merge + usize::from(reads.contains(&i))
    };

    // the fit: the detailed nodes when they fit the width, else chips in bands
    let aw = ui.available_width().max(200.0);
    let head_full = if edit.head.is_some() { HEAD_W } else { END_W };
    let head_chip = if edit.head.is_some() { CHIP_W } else { END_W };
    // a column is as wide as its widest node: one with sliders, or a narrow one without
    let mut col_w: Vec<f32> = vec![NARROW_W; n_fx];
    for (i, c) in col.iter().enumerate() {
        if !main_specs(&edit.fx[i]).is_empty() || reads.contains(&i) {
            col_w[*c] = NODE_W;
        }
    }
    // the context boxes (the layers over the event, its track) are columns after the chain's own
    // each lane's boxes after its own nodes (a pair: the video event's on the picture lane, the
    // audio event's on the sound lane)
    let ctx_in = |lane: u8| edit.context.iter().filter(move |b| b.lane == lane).count();
    let n_ctx = ctx_in(0).max(ctx_in(1));
    let total = n_fx + n_ctx;
    col_w.extend(std::iter::repeat_n(CTX_W, n_ctx));
    let col_x: Vec<f32> = col_w.iter().scan(0.0, |acc, w| {
        let x = *acc;
        *acc += w + GAP;
        Some(x)
    }).collect();
    let detail_w = PAD * 2.0 + head_full + GAP + col_w.iter().map(|w| w + GAP).sum::<f32>() + END_W;
    // the detailed nodes when they fit the width AND leave the form below its room: at most
    // `DETAIL_SHARE` of the window's visible height (a pair's two lanes of sliders took half a
    // window, and the form's fields went under the fold — live `formula`, `vhs_sound`)
    let node_h = |edit: &ChainEdit<'_>, i: usize| HEADER_H + (main_specs(&edit.fx[i]).len() + extra_rows(edit, i)) as f32 * ROW_H + NODE_PAD * 2.0;
    // the height of every row of every lane, and of the lane with its tracks
    let rows_of = |detail: bool| -> Vec<Vec<f32>> {
        lanes
            .iter()
            .zip(laid)
            .map(|(&l, lay)| {
                let li = lane_slots(l) as u8;
                (0..lay.rows)
                    .map(|r| {
                        if !detail {
                            return CHIP_H;
                        }
                        let nodes = (0..of.len())
                            .filter(|i| (!both || of[*i] == li) && lay.row[*i] == r)
                            .map(|i| node_h(&edit, i))
                            .fold(HEADER_H + NODE_PAD * 2.0, f32::max);
                        // a lane's context box with a key has a second line
                        let keyed = r == 0 && edit.context.iter().any(|b| b.lane == li.min(1) && b.key.is_some());
                        if keyed { nodes.max(HEADER_H + ROW_H) } else { nodes }
                    })
                    .collect()
            })
            .collect()
    };
    let height_of = |rows: &[f32], lay: &LaneLayout| rows.iter().sum::<f32>() + ARM_GAP * (rows.len() as f32 - 1.0) + TRACK_H * lay.tracks as f32;
    let detail_rows = rows_of(true);
    let detail_h = PAD * 2.0
        + detail_rows.iter().zip(laid).map(|(r, l)| height_of(r, l)).sum::<f32>()
        + (lanes.len() as f32 - 1.0) * LANE_GAP;
    let room_h = ui.clip_rect().height().max(1.0);
    let detail = view.detail.unwrap_or(detail_w <= aw && detail_h <= room_h * DETAIL_SHARE);
    let head_w = if detail { head_full } else { head_chip };
    let row_h: Vec<Vec<f32>> = if detail { detail_rows } else { rows_of(false) };
    let per_band = (((aw - PAD * 2.0 - head_w - END_W - GAP * 2.0) / (CHIP_W + CHIP_GAP)).floor() as usize).max(1);
    let bands = if detail { 1 } else { total.div_ceil(per_band).max(1) };
    let band_of = |c: usize| if detail { 0 } else { c / per_band };
    let band_of_node = |x: Node| match x {
        Node::Slot(j) => band_of(col[j]),
        Node::Source => 0,
    };
    // a band of chips is as tall as the arms that stand in it
    let lane_rows: Vec<Vec<usize>> = (0..bands)
        .map(|b| {
            lanes
                .iter()
                .zip(laid)
                .map(|(&l, lay)| {
                    let li = lane_slots(l) as u8;
                    (0..of.len()).filter(|i| (!both || of[*i] == li) && band_of(col[*i]) == b).map(|i| lay.row[i] + 1).max().unwrap_or(1)
                })
                .collect()
        })
        .collect();
    let lane_h: Vec<Vec<f32>> = lane_rows
        .iter()
        .enumerate()
        .map(|(b, rows)| {
            rows.iter()
                .enumerate()
                .map(|(n, r)| {
                    let tracks = laid[n].edges.iter().filter(|e| matches!(e.via, Via::Track(_)) && band_of_node(e.from) == b && band_of(col[e.to]) == b).count().min(laid[n].tracks);
                    row_h[n][..*r].iter().sum::<f32>() + ARM_GAP * (*r as f32 - 1.0) + TRACK_H * tracks as f32
                })
                .collect()
        })
        .collect();
    let band_h: Vec<f32> = lane_h.iter().map(|h| h.iter().sum::<f32>() + LANE_GAP * (lanes.len() as f32 - 1.0)).collect();
    let keys_h = if edit.keys.is_empty() { 0.0 } else { KEY_H + LANE_GAP };
    let height = PAD * 2.0 + band_h.iter().sum::<f32>() + (bands as f32 - 1.0) * LANE_GAP * 1.5 + keys_h;
    let (canvas, canvas_resp) = ui.allocate_exact_size(egui::vec2(aw, height), egui::Sense::click_and_drag());
    crate::widgets::field_probe::note("fx.graph", canvas);
    // the table's keys act here while the pointer is over the graph (`KtApp::graph_keys`)
    if ui.rect_contains_pointer(canvas) {
        let frame = ui.ctx().cumulative_frame_nr();
        ui.data_mut(|d| d.insert_temp(hover_slot(), (view_id, canvas, frame)));
    }
    // Ctrl+wheel (or a pinch): detail ↔ chips — the plain wheel stays the window body's scroll, as
    // everywhere in the program (a graph that ate the wheel stopped the form from scrolling)
    if canvas_resp.hovered() {
        let z = ui.input(|i| i.zoom_delta());
        let want = if z > 1.0 { Some(true) } else if z < 1.0 { Some(false) } else { None };
        if let Some(w) = want
            && w != detail
        {
            view.detail = Some(w);
        }
        crate::app::hint_into(hover, canvas_resp.clone(), tr(k::fx::graph::FIT_HINT));
    }
    if canvas_resp.dragged_by(egui::PointerButton::Middle) {
        view.pan = (view.pan - canvas_resp.drag_delta().x).clamp(0.0, (detail_w - aw).max(0.0));
    }
    if crate::clicks::double(&canvas_resp) {
        view = View::default();
    }
    // Ctrl+Shift+click on empty space: the whole picture again
    if edit.viewer && canvas_resp.clicked() && ctrl_shift(ui) {
        ch.viewer = Some(None);
    }
    if !detail {
        view.pan = 0.0;
    }
    let painter = ui.painter_at(canvas);
    painter.rect_filled(canvas, 4.0, ui.visuals().extreme_bg_color);

    // geometry of every node
    let mut rects: Vec<Option<egui::Rect>> = vec![None; edit.fx.len()];
    let mut ctx_rects: Vec<Option<egui::Rect>> = vec![None; edit.context.len()];
    let mut ends: Vec<(Lane, egui::Rect, egui::Rect)> = Vec::new(); // (lane, in, out)
    let lane_y = |li: usize, band: usize| -> f32 {
        let base = canvas.top() + PAD + band_h[..band].iter().map(|h| h + LANE_GAP * 1.5).sum::<f32>();
        base + lane_h[band][..li].iter().sum::<f32>() + LANE_GAP * li as f32
    };
    let x0 = canvas.left() + PAD - view.pan;
    for (li, &l) in lanes.iter().enumerate() {
        let lslot = lane_slots(l);
        let row_y = |r: usize| row_h[li][..r].iter().map(|h| h + ARM_GAP).sum::<f32>();
        let input = egui::Rect::from_min_size(egui::pos2(x0, lane_y(li, 0)), egui::vec2(head_w, if detail { HEADER_H } else { CHIP_H }));
        for i in (0..of.len()).filter(|i| !both || usize::from(of[*i] == 1) == lslot) {
            let c = col[i];
            let (band, k) = if detail { (0, c) } else { (c / per_band, c % per_band) };
            let (w, x) = if detail {
                (col_w[c], x0 + head_w + GAP + col_x[c])
            } else {
                (CHIP_W, x0 + head_w + GAP + k as f32 * (CHIP_W + CHIP_GAP))
            };
            let r = laid[li].row[i];
            let nh = if detail { node_h(&edit, i) } else { CHIP_H };
            rects[i] = Some(egui::Rect::from_min_size(egui::pos2(x, lane_y(li, band) + row_y(r)), egui::vec2(w, nh.min(row_h[li][r]))));
        }
        let mine: Vec<usize> = edit.context.iter().enumerate().filter(|(_, b)| b.lane as usize == lslot).map(|(n, _)| n).collect();
        {
            for (m, &n) in mine.iter().enumerate() {
                let c = n_fx + m;
                let (band, k) = if detail { (0, c) } else { (c / per_band, c % per_band) };
                let (w, x) = if detail {
                    (CTX_W, x0 + head_w + GAP + col_x[c])
                } else {
                    (CHIP_W, x0 + head_w + GAP + k as f32 * (CHIP_W + CHIP_GAP))
                };
                let hh = if !detail {
                    CHIP_H
                } else if edit.context[n].key.is_some() {
                    HEADER_H + ROW_H
                } else {
                    HEADER_H
                };
                ctx_rects[n] = Some(egui::Rect::from_min_size(egui::pos2(x, lane_y(li, band)), egui::vec2(w, hh)));
            }
        }
        let cols_here = n_fx + mine.len();
        let (last_band, last_k) = if detail || cols_here == 0 {
            (0, cols_here)
        } else {
            ((cols_here - 1) / per_band, (cols_here - 1) % per_band + 1)
        };
        let out_x = if detail {
            x0 + head_w + GAP + col_w[..cols_here].iter().map(|w| w + GAP).sum::<f32>()
        } else {
            x0 + head_w + GAP + last_k as f32 * (CHIP_W + CHIP_GAP)
        };
        let output = egui::Rect::from_min_size(egui::pos2(out_x, lane_y(li, last_band)), egui::vec2(END_W, if detail { HEADER_H } else { CHIP_H }));
        ends.push((l, input, output));
    }

    // the sockets of a slot: its main input; a Merge's Foreground and Matte; a keyed effect's key
    let sockets = |edit: &ChainEdit<'_>, i: usize| -> Vec<Socket> {
        let snd = sound(of[i]);
        let mut v = vec![Socket::Main];
        if edit.fx[i].is_merge() || (snd && edit.fx[i].has_key_socket()) {
            v.push(Socket::Second);
        }
        if edit.fx[i].is_merge() && !snd {
            v.push(Socket::Matte);
        }
        v
    };
    let in_pos = |edit: &ChainEdit<'_>, r: egui::Rect, i: usize, socket: Socket| -> egui::Pos2 {
        let many = sockets(edit, i).len();
        let y = match (socket, detail) {
            (Socket::Main, true) => HEADER_H * 0.5,
            (Socket::Second, true) => HEADER_H + ROW_H * 0.5,
            (Socket::Matte, true) => HEADER_H + ROW_H * 1.5,
            (Socket::Main, false) => if many > 1 { CHIP_H * 0.22 } else { CHIP_H * 0.5 },
            (Socket::Second, false) => CHIP_H * 0.55,
            (Socket::Matte, false) => CHIP_H * 0.86,
        };
        egui::pos2(r.left(), r.top() + y)
    };
    let out_pos = |r: egui::Rect| egui::pos2(r.right(), r.top() + HEADER_H * 0.5);
    let gap_x = if detail { GAP } else { CHIP_GAP };
    // every wire with its points: a node's input (`to`), or the lane's Out (`None`)
    struct Wire {
        lane: u8,
        from: Node,
        to: Option<(usize, Socket)>,
        pts: Vec<egui::Pos2>,
    }
    let mut wires: Vec<Wire> = Vec::new();
    for (n, &(l, input, output)) in ends.iter().enumerate() {
        let lslot = lane_slots(l);
        let start = |x: Node| match x {
            Node::Slot(j) => rects[j].map(out_pos),
            Node::Source => Some(input.right_center()),
        };
        let tracks_top = |band: usize| lane_y(n, band) + row_h[n][..lane_rows[band][n]].iter().map(|h| h + ARM_GAP).sum::<f32>() - ARM_GAP;
        for e in &laid[n].edges {
            let e: &Edge = e;
            let (Some(a), Some(r)) = (start(e.from), rects[e.to]) else { continue };
            let b = in_pos(&edit, r, e.to, e.socket);
            let from_band = band_of_node(e.from);
            let wrap = (from_band != band_of(col[e.to])).then_some(match e.socket {
                Socket::Main => 0,
                Socket::Second => 1,
                Socket::Matte => 2,
            });
            let row_mid = |k: usize| lane_y(n, from_band) + row_h[n][..k].iter().map(|h| h + ARM_GAP).sum::<f32>() + HEADER_H * 0.5;
            let y = match e.via {
                Via::Direct => None,
                Via::Row(k) => Some(row_mid(k)),
                Via::Track(t) => Some(tracks_top(from_band) + TRACK_H * (t as f32 + 0.75)),
            };
            wires.push(Wire { lane: lslot as u8, from: e.from, to: Some((e.to, e.socket)), pts: wire_points(a, b, y.filter(|_| wrap.is_none()), gap_x, wrap) });
        }
        // the Output, through the context boxes
        let from = strip.out[lslot].map_or(Node::Source, Node::Slot);
        let Some(mut a) = start(from) else { continue };
        for r in edit.context.iter().zip(ctx_rects.iter()).filter(|(b, _)| b.lane as usize == lslot).filter_map(|(_, r)| r.as_ref()) {
            let y = r.top() + r.height().min(HEADER_H) * 0.5;
            wires.push(Wire { lane: lslot as u8, from, to: None, pts: wire_points(a, egui::pos2(r.left(), y), None, gap_x, None) });
            a = egui::pos2(r.right(), y);
        }
        let b = egui::pos2(output.left(), output.top() + output.height().min(HEADER_H) * 0.5);
        wires.push(Wire { lane: lslot as u8, from, to: None, pts: wire_points(a, b, None, gap_x, None) });
    }
    // what is dragged over this graph
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let released = ui.input(|i| i.pointer.any_released());
    let node_drag = egui::DragAndDrop::payload::<NodeDrag>(ui.ctx()).filter(|d| d.view == view_id).map(|d| *d);
    let wire_drag = egui::DragAndDrop::payload::<WireDrag>(ui.ctx()).filter(|d| d.view == view_id).map(|d| *d);
    let over_node = pointer.and_then(|p| rects.iter().position(|r| r.is_some_and(|r| r.expand(1.0).contains(p))));
    // a node dropped off every node, near a wire of its lane, goes onto that wire
    let onto = match (node_drag, pointer) {
        (Some(d), Some(p)) if over_node.is_none() && canvas.contains(p) => wires
            .iter()
            .enumerate()
            .map(|(n, w)| (n, off_wire(p, &w.pts)))
            .filter(|(_, x)| *x <= WIRE_GRAB)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(n, _)| wires[*n].from != Node::Slot(d.slot) && wires[*n].to.is_none_or(|t| t.0 != d.slot))
            .map(|x| x.0),
        _ => None,
    };
    for (n, w) in wires.iter().enumerate() {
        let dead = |x: usize| !live[x] || edit.bypass[x];
        let dim = matches!(w.from, Node::Slot(j) if dead(j)) || w.to.is_some_and(|(i, _)| !live[i]);
        let color = lane_color(if both && w.lane == 1 { Lane::Sound } else if both { Lane::Picture } else { single });
        // the wire in the hand is drawn to the pointer, not where it stood
        if wire_drag.is_some_and(|d| d.was.is_some() && d.was == w.to) {
            continue;
        }
        let width = if onto == Some(n) { 3.5 } else { 1.6 };
        painter.add(egui::Shape::line(w.pts.clone(), egui::Stroke::new(width, if dim { color.gamma_multiply(0.4) } else { color })));
        if crate::widgets::field_probe::on() || cfg!(test) {
            // the middle of a wire, where a node is dropped on it
            let name = match w.to {
                Some((i, Socket::Main)) => format!("fx.wire.{i}"),
                Some((i, Socket::Second)) => format!("fx.wire.{i}.fg"),
                Some((i, Socket::Matte)) => format!("fx.wire.{i}.matte"),
                None => format!("fx.wire.out.{}", w.lane),
            };
            crate::widgets::field_probe::note(&name, egui::Rect::from_center_size(w.pts[w.pts.len() / 2], egui::vec2(4.0, 4.0)));
        }
    }
    // a device's frame around both halves; crossing halves are tied by a dotted line
    for &(p, s) in &pairs {
        let (Some(a), Some(b)) = (rects[p], rects[s]) else { continue };
        if (a.left() - b.left()).abs() < 0.5 {
            painter.rect_stroke(a.union(b).expand(3.0), 6.0, egui::Stroke::new(1.0, ui.visuals().weak_text_color()), egui::StrokeKind::Outside);
            painter.line_segment([a.center_bottom(), b.center_top()], egui::Stroke::new(1.5, ui.visuals().weak_text_color()));
        } else {
            painter.extend(egui::Shape::dashed_line(&[a.center(), b.center()], egui::Stroke::new(1.0, ui.visuals().weak_text_color()), 3.0, 3.0));
        }
    }

    // the ends
    for &(l, input, output) in &ends {
        // with a head, every lane starts from it (it spans the lanes); the lane's kind is its colour
        // and its name is the hint
        if edit.head.is_none() {
            end_node(&painter, ui, input, lane_color(l), tr(k::fx::graph::INPUT));
            // a pair's lanes each carry their event's master switch (fx-graph D10)
            if let Some(m2) = edit.master2.as_deref_mut() {
                let on = if lane_slots(l) == 0 { &mut *edit.master_on } else { m2 };
                let cb_rect = egui::Rect::from_min_size(input.left_top() - egui::vec2(0.0, 0.0), egui::vec2(18.0, input.height()));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(cb_rect.translate(egui::vec2(-20.0, 0.0)).intersect(canvas)));
                if crate::app::hint_into(hover, child.checkbox(on, ""), tr(k::fx::CHAIN_ON_HINT)).changed() {
                    ch.master = true;
                    if lane_slots(l) == 1 {
                        ch.master2 = true;
                    }
                }
                if crate::widgets::field_probe::on() || cfg!(test) {
                    crate::widgets::field_probe::note(&format!("fx.lane.{}.on", lane_slots(l)), cb_rect);
                }
            }
            let name = match l {
                Lane::Picture => tr(k::fx::graph::PICTURE),
                Lane::Sound => tr(k::fx::graph::SOUND),
            };
            if ui.rect_contains_pointer(input) {
                *hover = Some(name.to_string());
            }
        }
        end_node(&painter, ui, output, lane_color(l), tr(k::fx::graph::OUTPUT));
        crate::widgets::field_probe::note(&format!("fx.dst.{}", lane_slots(l)), output);
        // the Source's own output: a wire from it starts a branch at the very beginning
        let at = input.right_center();
        let r = egui::Rect::from_min_max(egui::pos2(at.x - 3.0, at.y - 8.0), egui::pos2(at.x + gap_x * 0.5, at.y + 8.0));
        crate::widgets::field_probe::note(&format!("fx.src.{}", lane_slots(l)), r);
        let resp = ui.interact(r.intersect(canvas), egui::Id::new((&id, "src", lane_slots(l))), egui::Sense::drag());
        painter.circle_filled(at, if resp.hovered() { 5.0 } else { 3.0 }, lane_color(l));
        let resp = crate::app::hint_into(hover, resp, tr(k::fx::graph::OUT_SOCKET_HINT));
        if resp.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), WireDrag { view: view_id, lane: lane_slots(l) as u8, from: Node::Source, was: None });
        }
    }
    // the head: one node in front of every lane (a morpher's swap acts on both)
    if let Some(h) = edit.head.as_mut() {
        let top = ends.first().map(|e| e.1.top()).unwrap_or(canvas.top() + PAD);
        let bottom = ends.last().map(|e| e.1.bottom()).unwrap_or(top + HEADER_H);
        let r = egui::Rect::from_min_max(egui::pos2(x0, top), egui::pos2(x0 + head_w, bottom));
        painter.rect_filled(r, 5.0, ui.visuals().faint_bg_color);
        painter.rect_stroke(r, 5.0, egui::Stroke::new(1.0, h.color), egui::StrokeKind::Inside);
        let inner = r.shrink2(egui::vec2(NODE_PAD, NODE_PAD));
        // a child the parent does not grow by: the graph's height is the canvas's, whatever stands in it
        let clip = canvas.intersect(ui.clip_rect());
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Min)));
        ui.set_clip_rect(clip);
        {
            if let Some(on) = h.on.as_deref_mut()
                && crate::app::hint_into(hover, ui.checkbox(on, ""), h.hint).changed()
            {
                ch.head = true;
            }
            let head_on = h.on.as_deref().copied().unwrap_or(true);
            let text = node_name(ui, &h.label, h.color, !head_on, inner.width() - 28.0);
            let resp = crate::app::hint_into(hover, ui.selectable_label(*edit.selected == ChainSel::Head, text), h.hint);
            crate::widgets::field_probe::note("fx.head", resp.rect);
            if resp.clicked() {
                *edit.selected = ChainSel::Head;
                crate::tele_journal::record(crate::tele_journal::Kind::Act, &format!("effect selected: {} (the head)", h.label));
            }
            let head_label = h.label.clone();
            let switchable = h.on.is_some();
            let mut head_toggle = false;
            resp.clone().context_menu(|ui| {
                *ctx.picked = Some(crate::ctx_menu::PanelTarget::FxHead { chain: ctx.chain });
                ui.label(egui::RichText::new(head_label.clone()).strong());
                if switchable {
                    let label = if head_on { tr(k::fx::DISABLE) } else { tr(k::fx::ENABLE) };
                    if ui.button(label).clicked() {
                        head_toggle = true;
                        ui.close();
                    }
                }
            });
            if head_toggle && let Some(on) = h.on.as_deref_mut() {
                *on = !head_on;
                ch.head = true;
            }
            if h.label.chars().count() > crate::widgets::SLOT_NAME_CHARS {
                resp.on_hover_text(h.label.clone());
            }
        }
    }

    // the context boxes: read-only, a double click opens their own window
    for (n, (b, r)) in edit.context.iter().zip(ctx_rects.iter()).enumerate() {
        let Some(r) = r else { continue };
        let (glyph, color) = match b.target {
            crate::widgets::ContextTarget::Layer(_) => (crate::icons::CARET_RIGHT, crate::theme::MORPH_SLOT),
            crate::widgets::ContextTarget::Track(_) => (crate::icons::CAT_VIDEO, ui.visuals().weak_text_color()),
        };
        painter.rect_filled(*r, 5.0, ui.visuals().faint_bg_color);
        painter.rect_stroke(*r, 5.0, egui::Stroke::new(1.0, color), egui::StrokeKind::Inside);
        let text = format!("{glyph} {} ({})", b.label, b.count);
        // a chip has one line: the key follows the name after an arrow; the box's accessible label
        // always names it
        let shown = match (&b.key, detail) {
            (Some(key), false) => format!("{text} {} {key}", crate::icons::ARROW_LEFT),
            _ => text.clone(),
        };
        let spoken = match &b.key {
            Some(key) => format!("{text} {}", crate::tf!(k::fx::graph::KEY, key)),
            None => text.clone(),
        };
        let room = r.width() - 8.0;
        let galley = node_name(ui, &shown, ui.visuals().text_color(), false, room).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            room,
            egui::TextStyle::Button,
        );
        let gy = r.top() + (r.height().min(HEADER_H) - galley.size().y) * 0.5;
        painter.galley(egui::pos2(r.left() + 4.0, gy), galley, ui.visuals().text_color());
        if let Some(key) = &b.key
            && detail
        {
            painter.text(
                egui::pos2(r.left() + 4.0, r.top() + HEADER_H + ROW_H * 0.5 - 2.0),
                egui::Align2::LEFT_CENTER,
                crate::tf!(k::fx::graph::KEY, key),
                egui::TextStyle::Small.resolve(ui.style()),
                ui.visuals().weak_text_color(),
            );
        }
        let resp = ui.interact(r.intersect(canvas), egui::Id::new((&id, "ctx", n)), egui::Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &spoken));
        crate::widgets::field_probe::note(&format!("fx.ctx.{n}"), *r);
        let resp = crate::app::hint_into(hover, resp, tr(k::fx::graph::CONTEXT_HINT));
        if crate::clicks::double(&resp) {
            ch.open = Some(b.target);
        }
    }

    // the nodes
    let mut op: Option<Op> = key_remove.map(Op::Remove);
    for (i, r) in rects.iter().enumerate() {
        let Some(r) = *r else { continue };
        // a node panned out of the canvas has no widgets (they would stand outside the window)
        if !r.intersects(canvas) {
            continue;
        }
        let lane = if !both { single } else if of[i] == 1 { Lane::Sound } else { Lane::Picture };
        let off = edit.bypass[i];
        let dead = off || !live[i];
        let selected = *edit.selected == ChainSel::Fx(i);
        let stroke = if selected {
            ui.visuals().selection.stroke
        } else {
            egui::Stroke::new(1.0, lane_color(lane).gamma_multiply(if dead { 0.35 } else { 0.8 }))
        };
        painter.rect_filled(r, 5.0, if dead { ui.visuals().extreme_bg_color } else { ui.visuals().faint_bg_color });
        if crate::widgets::field_probe::on() || cfg!(test) {
            crate::widgets::field_probe::note(&format!("fx.node.{i}"), r);
        }
        painter.rect_stroke(r, 5.0, stroke, egui::StrokeKind::Inside);
        let name = crate::fx_editor::fx_display_name(&edit.fx[i]);
        // a device's sound half carries the picture's name: the speaker tells them apart
        let sound_half = matches!(&edit.fx[i], FxInstance::Plugin(p)
            if p.manifest().media == kt_plugin_api::Media::AudioFx
                && kt_model::plugin_slot::registry().counterpart(&p.id).is_some());
        let marked;
        let name = if both && sound_half {
            marked = format!("{} {name}", crate::icons::CAT_VOLUME);
            marked.as_str()
        } else {
            name
        };
        let inner = r.shrink2(egui::vec2(NODE_PAD, NODE_PAD * 0.5));
        let clip = canvas.intersect(ui.clip_rect());
        let mut node_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
        node_ui.set_clip_rect(clip);
        {
            let ui = &mut node_ui;
            ui.horizontal(|ui| {
                let mut on = !off;
                let cb = crate::app::hint_into(hover, ui.checkbox(&mut on, ""), if off { tr(k::fx::ENABLE) } else { tr(k::fx::DISABLE) });
                if cb.changed() {
                    edit.bypass[i] = !on;
                    ch.bypass = true;
                }
                if crate::widgets::field_probe::on() {
                    crate::widgets::field_probe::note(&format!("fx.slot.{i}.on"), cb.rect);
                }
                // the switch, the cross and their gaps: the cross stays off the output socket
                let room = inner.width() - 58.0;
                let color = if live[i] { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
                let text = node_name(ui, name, color, off, room);
                let drawn = ui.selectable_label(selected, text);
                let slot_id = egui::Id::new((&id, "slot", i));
                let resp = crate::app::hint_into(
                    hover,
                    ui.interact(drawn.rect.expand2(egui::vec2(0.0, SLOT_GRAB_PAD)), slot_id, egui::Sense::click_and_drag()),
                    if live[i] { tr(k::fx::SLOT_HINT) } else { tr(k::fx::graph::DANGLING) },
                );
                if crate::widgets::field_probe::on() {
                    crate::widgets::field_probe::note(&format!("fx.slot.{i}"), resp.rect);
                }
                // Ctrl+Shift+click: the Viewer — the picture after this node (Blender's viewer node,
                // fx-graph D11), a pointer gesture of the graph like a drag
                let viewer_click = edit.viewer && resp.clicked() && ctrl_shift(ui);
                if viewer_click {
                    ch.viewer = Some(Some(i));
                } else if resp.clicked() {
                    *edit.selected = ChainSel::Fx(i);
                    crate::tele_journal::record(crate::tele_journal::Kind::Act, &format!("effect selected: {name} (slot {i})"));
                }
                if crate::clicks::double(&resp) && !detail {
                    // a chip opens to the detailed nodes, scrolled to it
                    view.detail = Some(true);
                    view.pan = ((r.left() - canvas.left()) - aw * 0.5).max(0.0);
                }
                if resp.drag_started() {
                    egui::DragAndDrop::set_payload(ui.ctx(), NodeDrag { view: view_id, slot: i, lane: of[i] });
                }
                if name.chars().count() > crate::widgets::SLOT_NAME_CHARS {
                    resp.clone().on_hover_text(name.to_string());
                }
                if let Some(d) = node_drag
                    && resp.contains_pointer()
                {
                    if d.lane != of[i] {
                        *hover = Some(tr(k::fx::graph::LANE_REFUSED).to_string());
                    } else if released && d.slot != i {
                        // onto a later node — after it; onto an earlier one — before it
                        op = Some(Op::Shift(d.slot, if d.slot < i { To::After(i) } else { To::Before(i) }));
                    }
                }
                resp.context_menu(|ui| {
                    *ctx.picked = Some(crate::ctx_menu::PanelTarget::FxSlot { chain: ctx.chain, slot: i });
                    ui.label(egui::RichText::new(name).strong());
                    let label = if off { tr(k::fx::ENABLE) } else { tr(k::fx::DISABLE) };
                    if ui.button(label).clicked() {
                        edit.bypass[i] = !off;
                        ch.bypass = true;
                        ui.close();
                    }
                    if ui.button(tr(k::fx::REMOVE_SLOT)).clicked() {
                        op = Some(Op::Remove(i));
                        ui.close();
                    }
                    // the same as dragging the wire off its socket (rule 17)
                    if strip.links[i].second.is_some() && ui.button(crate::tf!(k::fx::graph::TAKE_OFF, socket_name(&edit.fx[i], Socket::Second))).clicked() {
                        op = Some(Op::Disconnect(i, Socket::Second));
                        ui.close();
                    }
                    if strip.links[i].matte.is_some() && ui.button(crate::tf!(k::fx::graph::TAKE_OFF, socket_name(&edit.fx[i], Socket::Matte))).clicked() {
                        op = Some(Op::Disconnect(i, Socket::Matte));
                        ui.close();
                    }
                    if let Some((_, by)) = strip.links[i].matte {
                        for b in MATTES {
                            if ui.selectable_label(by == b, matte_name(b)).clicked() {
                                op = Some(Op::By(i, b));
                                ui.close();
                            }
                        }
                    }
                    if edit.viewer && ui.button(tr(k::fx::graph::VIEWER_MENU)).clicked() {
                        ch.viewer = Some(Some(i));
                        ui.close();
                    }
                });
                // every node has its cross, a chip too (R.33: removing is one click on the slot)
                if crate::app::hint_into(hover, ui.small_button(crate::icons::CLEAR), tr(k::fx::REMOVE_SLOT)).clicked() {
                    op = Some(Op::Remove(i));
                }
            });
            if detail {
                if let FxInstance::Merge { mode, .. } = &mut edit.fx[i] {
                    ui.horizontal(|ui| {
                        row_label(ui, tr(k::fx::graph::MODE));
                        ch.fx |= merge_mode_combo(ui, (&id, "mode", i), mode, 96.0);
                    });
                }
                main_rows_of(ui, &id, i, &mut edit, hover, &mut ch);
                if let Some((_, by)) = strip.links[i].matte {
                    let mut pick = by;
                    egui::ComboBox::from_id_salt((&id, "by", i)).width(NODE_W - 16.0).selected_text(matte_name(by)).show_ui(ui, |ui| {
                        for b in MATTES {
                            ui.selectable_value(&mut pick, b, matte_name(b));
                        }
                    });
                    if pick != by {
                        op = Some(Op::By(i, pick));
                    }
                }
                if reads.contains(&i) {
                    let l = ui.weak(format!("{} {}", crate::icons::LOOP, tr(k::fx::graph::READS_AGAIN)));
                    crate::widgets::field_probe::note(&format!("fx.read.{i}"), l.rect);
                    crate::app::hint_into(hover, l, tr(k::fx::graph::READS_AGAIN_HINT));
                }
            }
        }
        if !detail && reads.contains(&i) {
            // a chip says it by a mark at its corner
            let at = egui::Rect::from_center_size(r.right_bottom() - egui::vec2(6.0, 1.0), egui::vec2(11.0, 11.0));
            painter.circle_filled(at.center(), 6.0, ui.visuals().extreme_bg_color);
            painter.text(at.center(), egui::Align2::CENTER_CENTER, crate::icons::LOOP, egui::FontId::proportional(10.0), lane_color(lane));
            crate::widgets::field_probe::note(&format!("fx.read.{i}"), at);
            if ui.rect_contains_pointer(at) {
                *hover = Some(tr(k::fx::graph::READS_AGAIN_HINT).to_string());
            }
        }
        // the sockets, over the node: drag a wire out of the output, off an input
        let socket_rect = |at: egui::Pos2, out: bool, h: f32| {
            let (inn, outw) = (3.0, gap_x * 0.5);
            let x = if out { (at.x - inn, at.x + outw) } else { (at.x - outw, at.x + inn) };
            egui::Rect::from_min_max(egui::pos2(x.0, at.y - h), egui::pos2(x.1, at.y + h))
        };
        let at = out_pos(r);
        let sr = socket_rect(at, true, 8.0);
        crate::widgets::field_probe::note(&format!("fx.out.{i}"), sr);
        let resp = ui.interact(sr.intersect(canvas), egui::Id::new((&id, "out", i)), egui::Sense::drag());
        painter.circle_filled(at, if resp.hovered() { 5.0 } else { 3.5 }, lane_color(lane));
        let resp = crate::app::hint_into(hover, resp, tr(k::fx::graph::OUT_SOCKET_HINT));
        if resp.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), WireDrag { view: view_id, lane: of[i], from: Node::Slot(i), was: None });
        }
        let mine = sockets(&edit, i);
        for &sk in &mine {
            let at = in_pos(&edit, r, i, sk);
            let sr = socket_rect(at, false, if detail { 8.0 } else if mine.len() > 1 { 3.5 } else { 8.0 });
            let tail = match sk {
                Socket::Main => "in",
                Socket::Second => "fg",
                Socket::Matte => "matte",
            };
            crate::widgets::field_probe::note(&format!("fx.{tail}.{i}"), sr);
            let resp = ui.interact(sr.intersect(canvas), egui::Id::new((&id, tail, i)), egui::Sense::drag());
            let wired = strip.links[i].get(sk);
            let lit = resp.contains_pointer() && (wire_drag.is_some() || resp.hovered());
            let radius = if lit { 5.0 } else { 3.5 };
            if wired.is_some() {
                painter.circle_filled(at, radius, lane_color(lane));
            } else {
                painter.circle_filled(at, radius, ui.visuals().extreme_bg_color);
                painter.circle_stroke(at, radius, egui::Stroke::new(1.2, lane_color(lane)));
            }
            let resp = crate::app::hint_into(hover, resp, &crate::tf!(k::fx::graph::IN_SOCKET_HINT, socket_name(&edit.fx[i], sk)));
            if resp.drag_started()
                && let Some(from) = wired
            {
                egui::DragAndDrop::set_payload(ui.ctx(), WireDrag { view: view_id, lane: of[i], from, was: Some((i, sk)) });
            }
        }
    }
    // a node dropped on a wire
    if released
        && let (Some(d), Some(n)) = (node_drag, onto)
    {
        let w = &wires[n];
        if w.lane != d.lane {
            ch.refused = Some(tr(k::fx::graph::LANE_REFUSED).to_string());
        } else {
            op = Some(Op::Shift(d.slot, match w.to {
                Some((i, sk)) => To::Wire(i, sk),
                None => To::Out,
            }));
        }
    }
    // the wire in the hand, and where it is let go
    let zone = egui::Rect::from_min_max(canvas.min, egui::pos2(canvas.right(), (canvas.bottom() + BRANCH_ZONE).min(ui.clip_rect().bottom())));
    let pending_id = view_id.with("branch");
    if let (Some(d), Some(p)) = (wire_drag, pointer) {
        let at = match d.from {
            Node::Slot(j) => rects.get(j).copied().flatten().map(out_pos),
            Node::Source => ends.iter().find(|e| lane_slots(e.0) as u8 == d.lane).map(|e| e.1.right_center()),
        };
        if let Some(at) = at {
            let color = lane_color(if sound(d.lane) { Lane::Sound } else { Lane::Picture });
            let hand = ui.painter().with_clip_rect(zone);
            hand.add(egui::Shape::line(wire_points(at, p, None, gap_x, None), egui::Stroke::new(2.0, color)));
        }
        if d.was.is_none() {
            *hover = Some(tr(k::fx::graph::OUT_SOCKET_HINT).to_string());
        }
        if released {
            // the socket under the pointer, else the node it stands on: a Merge takes the wire into
            // its free Foreground, any other node into its main input
            let near = (0..rects.len())
                .filter_map(|i| rects[i].map(|r| (i, r)))
                .flat_map(|(i, r)| sockets(&edit, i).into_iter().map(move |sk| (i, r, sk)))
                .map(|(i, r, sk)| (i, sk, in_pos(&edit, r, i, sk).distance(p)))
                .filter(|x| x.2 <= if detail { 12.0 } else { 9.0 })
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|x| (x.0, x.1));
            let target = near.or_else(|| {
                over_node.map(|i| (i, if edit.fx[i].is_merge() && strip.links[i].second.is_none() { Socket::Second } else { Socket::Main }))
            });
            let to_out = ends.iter().find(|e| e.2.expand(6.0).contains(p)).map(|e| lane_slots(e.0) as u8);
            let refuse = |ch: &mut ChainChanged, t: &str| ch.refused = Some(t.to_string());
            match (target, to_out) {
                (Some((i, _)), _) if of[i] != d.lane => refuse(&mut ch, tr(k::fx::graph::LANE_REFUSED)),
                (Some((i, sk)), _) if d.was == Some((i, sk)) => {}
                (Some((i, sk)), _) => {
                    if d.from == Node::Slot(i) {
                        refuse(&mut ch, tr(k::fx::graph::WHY_LOOP));
                    } else {
                        op = Some(Op::Connect(i, sk, d.from));
                    }
                }
                (None, Some(l)) if l != d.lane => refuse(&mut ch, tr(k::fx::graph::LANE_REFUSED)),
                (None, Some(_)) => match d.from {
                    Node::Slot(j) if strip.out[usize::from(d.lane == 1)] == Some(j) => {}
                    Node::Slot(j) if strip.readers(d.lane, d.from).is_empty() => op = Some(Op::Out(j)),
                    _ => refuse(&mut ch, tr(k::fx::graph::WHY_OUT)),
                },
                (None, None) if zone.contains(p) => match d.was {
                    Some((_, Socket::Main)) => refuse(&mut ch, tr(k::fx::graph::WHY_MAIN)),
                    Some((i, sk)) => op = Some(Op::Disconnect(i, sk)),
                    None => {
                        ui.data_mut(|m| m.insert_temp(pending_id, Pending { lane: d.lane, from: d.from, at: p }));
                        egui::Popup::open_id(ui.ctx(), pending_id);
                    }
                },
                (None, None) => {}
            }
        }
    }
    // the picker of a branch, where its wire was dropped
    if egui::Popup::is_id_open(ui.ctx(), pending_id)
        && let Some(pend) = ui.data(|m| m.get_temp::<Pending>(pending_id))
    {
        let snd = sound(pend.lane);
        let pk = if snd { crate::fx_registry::PickKinds::Audio } else { crate::fx_registry::PickKinds::from(false) };
        let arm_pv = preview.as_deref_mut().filter(|_| !snd).and_then(|p| p.place_arm(ctx.chain, pend.from).then_some(p));
        let rows = edit.presets.rows;
        match crate::fx_registry::fx_picker_at(ui, pending_id, pend.at, pk, query, rows, hover, arm_pv) {
            Some(crate::fx_registry::Picked::Fx(f)) => op = Some(Op::Branch { lane: pend.lane, from: pend.from, items: vec![(f, false, Vec::new())], fx_in: Vec::new() }),
            Some(crate::fx_registry::Picked::Merge) => op = Some(Op::Merge { lane: pend.lane, from: Some(pend.from) }),
            Some(crate::fx_registry::Picked::Preset(i)) => {
                if let Some(row) = rows.get(i) {
                    op = Some(Op::Branch { lane: pend.lane, from: pend.from, items: items_of(&row.block, src_in), fx_in: row.block.fx_in.clone() });
                }
            }
            Some(crate::fx_registry::Picked::PresetAction(a)) => *edit.presets.action = Some(a),
            Some(crate::fx_registry::Picked::Head) | None => {}
        }
    }

    // the sidechain keys: a box under the lane, a dashed wire into its compressor
    for (slot, name) in &edit.keys {
        let Some(r) = rects.get(*slot).copied().flatten() else { continue };
        let kr = egui::Rect::from_min_size(
            egui::pos2(r.left(), canvas.bottom() - PAD - KEY_H),
            egui::vec2(r.width().min(CTX_W), KEY_H),
        );
        painter.rect_stroke(kr, 8.0, egui::Stroke::new(1.0, lane_color(Lane::Sound)), egui::StrokeKind::Inside);
        let label = crate::tf!(k::fx::graph::KEY, name);
        painter.text(
            kr.left_center() + egui::vec2(6.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &label,
            egui::TextStyle::Small.resolve(ui.style()),
            ui.visuals().text_color(),
        );
        painter.extend(egui::Shape::dashed_line(
            &[kr.center_top(), egui::pos2(kr.center().x, r.bottom())],
            egui::Stroke::new(1.2, lane_color(Lane::Sound)),
            4.0,
            3.0,
        ));
        crate::widgets::field_probe::note(&format!("fx.key.{slot}"), kr);
    }

    ui.data_mut(|d| d.insert_temp(view_id, view));

    if let Some(op) = op {
        perform(op, &mut edit, &of, sound, kind_lane, &mut ch);
    }
    if edit.fx.is_empty() {
        ui.weak(tr(k::fx::CHAIN_EMPTY));
    }
    ch
}

const MATTES: [MatteFrom; 4] = [MatteFrom::Luma, MatteFrom::InvLuma, MatteFrom::Alpha, MatteFrom::InvAlpha];

fn matte_name(b: MatteFrom) -> &'static str {
    match b {
        MatteFrom::Luma => tr(k::fx::graph::BY_LUMA),
        MatteFrom::InvLuma => tr(k::fx::graph::BY_INV_LUMA),
        MatteFrom::Alpha => tr(k::fx::graph::BY_ALPHA),
        MatteFrom::InvAlpha => tr(k::fx::graph::BY_INV_ALPHA),
    }
}

/// An input's name for its hint.
fn socket_name(f: &FxInstance, s: Socket) -> &'static str {
    match s {
        Socket::Main if f.is_merge() => tr(k::fx::graph::BACKGROUND),
        Socket::Main => tr(k::fx::graph::INPUT),
        Socket::Second if f.is_merge() => tr(k::fx::graph::FOREGROUND),
        Socket::Second => tr(k::fx::graph::KEY_SOCKET),
        Socket::Matte => tr(k::fx::graph::MATTE),
    }
}

/// A Merge's mode: the lane's own list (the picture's blend modes, the sound's sum and crossfade).
pub(crate) fn merge_mode_combo(ui: &mut egui::Ui, salt: impl std::hash::Hash + std::fmt::Debug, mode: &mut MergeMode, w: f32) -> bool {
    let name = |m: MergeMode| match m {
        MergeMode::Blend(b) => crate::track_window::blend_label(b),
        MergeMode::Sum => tr(k::fx::graph::SUM),
        MergeMode::Crossfade => tr(k::fx::graph::CROSSFADE),
    };
    let all: Vec<MergeMode> = if mode.is_sound() {
        vec![MergeMode::Sum, MergeMode::Crossfade]
    } else {
        kt_model::BlendMode::ALL.iter().map(|b| MergeMode::Blend(*b)).collect()
    };
    let was = *mode;
    let r = egui::ComboBox::from_id_salt(salt).width(w).selected_text(name(was)).show_ui(ui, |ui| {
        for m in all {
            let r = ui.selectable_value(mode, m, name(m));
            let tag = match m {
                MergeMode::Blend(b) => b.tag(),
                MergeMode::Sum => "sum",
                MergeMode::Crossfade => "crossfade",
            };
            crate::widgets::field_probe::note(&format!("fx.mode.{tag}"), r.rect);
        }
    });
    crate::widgets::field_probe::note("fx.mode", r.response.rect);
    *mode != was
}

/// A block's slots with their keys moved to this carrier's domain ("input to input").
fn items_of(block: &kt_model::ops::FxChainBlock, src_in: kt_time::Tick) -> Vec<Item> {
    let (mut fx, mut bypass, mut auto) = (Vec::new(), Vec::new(), Vec::new());
    kt_model::ops::append_chain_block(&mut fx, &mut bypass, &mut auto, block, src_in);
    fx.into_iter().zip(bypass).zip(auto).map(|((f, b), a)| (f, b, a)).collect()
}

/// A node's main rows (fx-graph D4a): the common row widget at node width; a row under a curve is
/// shown, not edited here — the form below sets its key at the cursor, the one rule of a curved
/// row.
fn main_rows_of(
    ui: &mut egui::Ui,
    id: &(impl std::hash::Hash + std::fmt::Debug),
    i: usize,
    edit: &mut ChainEdit<'_>,
    hover: &mut Option<String>,
    ch: &mut ChainChanged,
) {
    for spec in main_specs(&edit.fx[i]) {
        // a row under a curve is named, not edited, here: the form below sets its key at the
        // cursor (the one rule of a curved row)
        if edit.auto.get(i).is_some_and(|c| c.iter().any(|c| c.param == spec.name)) {
            let l = ui.weak(format!("{} {}", crate::icons::CAT_DISTORT, node_row_name(&spec.name)));
            crate::app::hint_into(hover, l, tr(k::fx::graph::CURVED));
            continue;
        }
        ui.horizontal(|ui| {
            if let kt_model::ParamType::Choice { variants } = &spec.ty {
                row_label(ui, &node_row_name(&spec.name));
                let cur = edit.fx[i].get_param(&spec.name).unwrap_or(0.0).round() as usize;
                let shown = variants.get(cur).map(|v| choice_caption(&spec.name, v)).unwrap_or_default();
                let mut pick = cur;
                egui::ComboBox::from_id_salt((id, "main", i, &*spec.name)).width((ui.available_width() - 8.0).max(48.0)).selected_text(shown).show_ui(ui, |ui| {
                    for (n, v) in variants.iter().enumerate() {
                        ui.selectable_value(&mut pick, n, choice_caption(&spec.name, v));
                    }
                });
                if pick != cur && edit.fx[i].set_param(&spec.name, pick as f64) {
                    ch.fx = true;
                }
            } else {
                // the common number of a passport row (`param_value`: range, step, unit) and a
                // slider to the node's edge; written only on a human gesture, and a value the slot
                // already holds is no edit (input-without-work)
                let Some(base) = edit.fx[i].get_param(&spec.name) else { return };
                let mut v = base;
                row_label(ui, &node_row_name(&spec.name));
                let r = crate::widgets::param_value(ui, &spec, &mut v);
                ui.spacing_mut().slider_width = (ui.available_width() - 4.0).max(24.0);
                let sl = ui.add(egui::Slider::new(&mut v, spec.min..=spec.max).show_value(false).clamping(egui::SliderClamping::Edits));
                let gesture = |r: &egui::Response| r.dragged() || r.drag_stopped() || r.has_focus() || r.lost_focus() || r.clicked();
                if ((r.changed() && gesture(&r)) || (sl.changed() && gesture(&sl)))
                    && edit.fx[i].set_param(&spec.name, v)
                    && edit.fx[i].get_param(&spec.name) != Some(base)
                {
                    ch.fx = true;
                }
            }
        });
    }
}

/// A main row's caption in a node: the device panels' own captions (the ISP's rows), the common
/// passport captions, and for the rows only a device's face names by its switches (the tape speed,
/// the sound out, the codec size, the audio bitrate) a caption of the graph's own — never the raw
/// row name.
fn node_row_name(name: &str) -> String {
    let own = match name {
        "speed" => Some(tr(k::fx::graph::ROW_SPEED)),
        "audio_out" => Some(tr(k::fx::graph::ROW_SOUND)),
        "lines" => Some(tr(k::fx::graph::ROW_LINES)),
        "audio_kbps" => Some(tr(k::fx::graph::ROW_AUDIO_KBPS)),
        _ => None,
    };
    if let Some(t) = own.or_else(|| crate::isp_panel::row_label(name)) {
        return t.trim_end_matches(':').to_string();
    }
    crate::widgets::param_label(name).trim_end_matches(':').to_string()
}

/// A main row's name in a node: painted, elided to `LABEL_W`. Not a `Label` on purpose — the row's
/// label with its doors (reset by double click, publication by right click) is the form's below;
/// a second widget of the same name would be a second door that does less (rule 17).
fn row_label(ui: &mut egui::Ui, text: &str) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(LABEL_W, ROW_H - 4.0), egui::Sense::hover());
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::TextFormat { font_id: font, color: ui.visuals().text_color(), ..Default::default() },
    );
    job.wrap = egui::text::TextWrapping { max_width: LABEL_W - 2.0, max_rows: 1, overflow_character: Some('…'), ..Default::default() };
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    ui.painter().galley(egui::pos2(r.left(), r.center().y - galley.size().y * 0.5), galley, ui.visuals().text_color());
}

/// A slot's name in one line elided to the node's room — the strip's `elided_name` with the node's
/// width instead of a count of letters.
fn node_name(ui: &egui::Ui, name: &str, color: egui::Color32, off: bool, max_w: f32) -> egui::WidgetText {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut job = egui::text::LayoutJob::single_section(
        name.to_string(),
        egui::TextFormat {
            font_id: font,
            color,
            strikethrough: if off { egui::Stroke::new(1.0, color) } else { egui::Stroke::NONE },
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping { max_width: max_w.max(24.0), max_rows: 1, overflow_character: Some('…'), ..Default::default() };
    egui::WidgetText::Galley(ui.fonts_mut(|f| f.layout_job(job)))
}

fn end_node(painter: &egui::Painter, ui: &egui::Ui, r: egui::Rect, color: egui::Color32, text: &str) {
    painter.rect_filled(r, 10.0, ui.visuals().faint_bg_color);
    painter.rect_stroke(r, 10.0, egui::Stroke::new(1.0, color), egui::StrokeKind::Inside);
    painter.text(r.center(), egui::Align2::CENTER_CENTER, text, egui::TextStyle::Small.resolve(ui.style()), ui.visuals().text_color());
}

/// The bar above the graph: the master switch, the picker, the presets, copy-paste and Fit — the
/// strip's own buttons, so every door stays where a person and a live scenario find it.
#[allow(clippy::too_many_arguments)]
fn toolbar(
    ui: &mut egui::Ui,
    edit: &mut ChainEdit<'_>,
    lanes: &[u8],
    kinds: crate::fx_registry::PickKinds,
    query: &mut String,
    clipboard: &mut Option<kt_model::ops::FxChainBlock>,
    src_in: kt_time::Tick,
    hover: &mut Option<String>,
    mut preview: Option<&mut crate::fx_thumbs::PickerCtx<'_>>,
    ctx: &ChainCtx<'_>,
    ch: &mut ChainChanged,
    view: &mut View,
) -> Option<Op> {
    let strip = Strip::new(edit.links, lanes);
    let sel = edit.selected.fx();
    let mut op = None;
    // the cells show what the click does: in a chain that branches it adds after the selected node
    if let (Some(p), Some(i)) = (preview.as_mut(), add_after(&strip, 0, sel)) {
        p.place_after(ctx.chain, i);
    }
    ui.horizontal_wrapped(|ui| {
        // a pair's switches stand at its lanes' In nodes (fx-graph D10)
        if edit.master2.is_none() {
            if crate::app::hint_into(hover, ui.checkbox(edit.master_on, tr(k::fx::CHAIN_ON)), tr(k::fx::CHAIN_ON_HINT)).changed() {
                ch.master = true;
            }
            ui.separator();
        }
        let head_name = edit.head.as_ref().map(|h| h.label.clone());
        match crate::fx_registry::fx_picker_with_head(ui, kinds, query, head_name.as_deref(), edit.presets.rows, hover, preview) {
            Some(crate::fx_registry::Picked::Preset(i)) => {
                if let Some(row) = edit.presets.rows.get(i) {
                    op = Some(Op::Add { items: items_of(&row.block, src_in), fx_in: row.block.fx_in.clone() });
                }
            }
            Some(crate::fx_registry::Picked::PresetAction(a)) => *edit.presets.action = Some(a),
            Some(crate::fx_registry::Picked::Fx(tmpl)) => {
                op = Some(Op::Add { items: vec![(tmpl, false, Vec::new())], fx_in: Vec::new() });
            }
            // in the selected node's lane: the selected node to the Foreground
            Some(crate::fx_registry::Picked::Merge) => {
                let lane = sel.and_then(|i| lanes.get(i).copied()).unwrap_or(0);
                op = Some(Op::Merge { lane, from: sel.filter(|_| !strip.is_line(lane)).map(Node::Slot) });
            }
            Some(crate::fx_registry::Picked::Head) => *edit.selected = ChainSel::Head,
            None => {}
        }
        let block = |edit: &ChainEdit<'_>| kt_model::ops::FxChainBlock {
            slots: edit
                .fx
                .iter()
                .enumerate()
                .map(|(i, f)| kt_model::ops::FxSlotBlock {
                    fx: f.clone(),
                    bypass: edit.bypass[i],
                    curves: edit.auto.get(i).cloned().unwrap_or_default(),
                    masks: Vec::new(),
                })
                .collect(),
            src_in,
            fx_off: !*edit.master_on,
            // a chain that does not branch is copied as ever: no wiring
            fx_in: if strip.is_line(0) && strip.is_line(1) { Vec::new() } else { kt_model::fx_strip::wiring_of(edit.links, &vec![0; edit.fx.len()], 0) },
        };
        if !edit.fx.is_empty() && crate::app::hint_into(hover, ui.button(tr(k::fx::SAVE_PRESET)), tr(k::fx::SAVE_PRESET_HINT)).clicked() {
            *edit.presets.save = Some(block(edit));
        }
        let hint = Some(tr(k::fx::COPY_CHAIN_HINT));
        if !edit.fx.is_empty() && crate::widgets::icon_action(ui, hover, crate::icons::COPY, tr(k::fx::COPY_CHAIN), hint, true).clicked() {
            let b = block(edit);
            if let Ok(json) = serde_json::to_string_pretty(&b) {
                ui.ctx().copy_text(json);
            }
            *clipboard = Some(b);
        }
        let hint = Some(tr(k::fx::PASTE_CHAIN_HINT));
        if crate::widgets::icon_action(ui, hover, crate::icons::CLIPBOARD, tr(k::fx::PASTE_CHAIN), hint, clipboard.is_some()).clicked()
            && let Some(b) = clipboard.clone()
        {
            op = Some(Op::Add { items: items_of(&b, src_in), fx_in: b.fx_in.clone() });
        }
        let fit = crate::app::hint_into(hover, ui.button(tr(k::fx::graph::FIT)), tr(k::fx::graph::FIT_HINT));
        if crate::widgets::field_probe::on() {
            crate::widgets::field_probe::note("fx.graph.fit", fit.rect);
        }
        if fit.clicked() {
            *view = View::default();
        }
    });
    op
}

/// The Viewer (fx-graph D11): the preview shows the project with ONE event's chain cut to a node and
/// what it is made from — the same composition path, the same sound path (a sound node is heard up to
/// it). In a chain that branches this is the node's ancestors (fx-graph-dag D3), not a prefix.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewer {
    pub clip: kt_model::ClipId,
    /// The model's slots that stay, ascending: the node and its ancestors.
    pub keep: Vec<usize>,
    /// The leading Geometry node stays: the node is at or after it.
    pub lead_on: bool,
    /// The node's name, for the badge over the preview.
    pub name: String,
}

impl Viewer {
    /// The project the player holds while the Viewer is on: the event (at the root or in a reel)
    /// with its chain cut, its wiring, curves and slot masks with it; nothing else moves.
    pub fn cut(&self, p: &mut kt_model::Project) {
        if let Some(c) = p.clips.iter_mut().find(|c| c.id == self.clip) {
            self.cut_event(c);
        } else if let Some(c) = p.sequences.iter_mut().flat_map(|s| s.clips.iter_mut()).find(|c| c.id == self.clip) {
            self.cut_event(c);
        }
    }

    /// The event's chain cut to the node and its ancestors.
    pub fn cut_event(&self, c: &mut kt_model::ClipEvent) {
        let keep: Vec<usize> = self.keep.iter().copied().filter(|i| *i < c.fx.len()).collect();
        c.fx_in = kt_model::fx_graph::derive(&c.fx_in, &keep);
        c.geometry_pos = keep.iter().filter(|i| **i < c.geometry_pos).count();
        c.fx = keep.iter().map(|i| c.fx[*i].clone()).collect();
        c.fx_bypass = keep.iter().map(|i| c.fx_bypass.get(*i).copied().unwrap_or(false)).collect();
        c.fx_auto = keep.iter().map(|i| c.fx_auto.get(*i).cloned().unwrap_or_default()).collect();
        c.masks.slots = keep.iter().map(|i| c.masks.slot(*i).to_vec()).collect();
        c.masks.canonicalize();
        if !self.lead_on {
            c.geometry = Default::default();
            c.geometry_auto.clear();
        }
    }
}

/// Ctrl+Shift held for this frame's click: the modifiers the press itself carried (a press and its
/// release can come in one frame, before the input's held modifiers say so), or the held ones.
fn ctrl_shift(ui: &egui::Ui) -> bool {
    ui.input(|i| {
        (i.modifiers.ctrl && i.modifiers.shift)
            || i.events.iter().any(|e| matches!(e, egui::Event::PointerButton { modifiers, .. } if modifiers.ctrl && modifiers.shift))
    })
}
