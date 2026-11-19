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
/// A context box (a layer over the event, its track).
const CTX_W: f32 = 150.0;
/// A sidechain key's box under the lane.
const KEY_H: f32 = 18.0;

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
    preview: Option<&mut crate::fx_thumbs::PickerCtx<'_>>,
) -> ChainChanged {
    let kinds = kinds.into();
    let both = matches!(kinds, crate::fx_registry::PickKinds::Both);
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

    toolbar(ui, &id, &mut edit, kinds, query, clipboard, src_in, hover, preview, &mut ch, &mut view);

    // the lanes
    let single = match kinds {
        crate::fx_registry::PickKinds::Audio => Lane::Sound,
        _ => Lane::Picture,
    };
    let (mut pic, mut snd) = (Vec::new(), Vec::new());
    // a single-kind chain is ONE lane, the layout's first (its kind is only its colour): a sound
    // chain put into the second slot was never laid out, and an audio event's nodes did not show
    // (found-defects 811)
    for (i, f) in edit.fx.iter().enumerate() {
        match if both { lane_of(f) } else { Lane::Picture } {
            Lane::Picture => pic.push(i),
            Lane::Sound => snd.push(i),
        }
    }
    let lanes: Vec<Lane> = if both { vec![Lane::Picture, Lane::Sound] } else { vec![single] };
    let pairs = if both { device_pairs(edit.fx, &pic, &snd) } else { Vec::new() };
    let cols = columns(&pic, &snd, &pairs);
    let lane_slots = |l: Lane| if l == Lane::Picture || !both { 0 } else { 1 };

    // the fit: the detailed nodes when they fit the width, else chips in bands
    let aw = ui.available_width().max(200.0);
    let head_full = if edit.head.is_some() { HEAD_W } else { END_W };
    let head_chip = if edit.head.is_some() { CHIP_W } else { END_W };
    // a column is as wide as its widest node: one with sliders, or a narrow one without
    let mut col_w: Vec<f32> = cols
        .iter()
        .map(|c| {
            let wide = c.iter().flatten().any(|&i| !main_specs(&edit.fx[i]).is_empty());
            if wide { NODE_W } else { NARROW_W }
        })
        .collect();
    // the context boxes (the layers over the event, its track) are columns after the chain's own
    let n_fx = cols.len();
    let n_ctx = edit.context.len();
    let total = n_fx + n_ctx;
    col_w.extend(std::iter::repeat_n(CTX_W, n_ctx));
    let col_x: Vec<f32> = col_w.iter().scan(0.0, |acc, w| {
        let x = *acc;
        *acc += w + GAP;
        Some(x)
    }).collect();
    let detail_w = PAD * 2.0 + head_full + GAP + col_w.iter().map(|w| w + GAP).sum::<f32>() + END_W;
    let detail = view.detail.unwrap_or(detail_w <= aw);
    let head_w = if detail { head_full } else { head_chip };
    let node_h = |i: usize| HEADER_H + main_specs(&edit.fx[i]).len() as f32 * ROW_H + NODE_PAD * 2.0;
    let lane_h: Vec<f32> = lanes
        .iter()
        .enumerate()
        .map(|(n, &l)| {
            let li = lane_slots(l);
            let nodes = cols.iter().filter_map(|c| c[li]).map(node_h).fold(HEADER_H + NODE_PAD * 2.0, f32::max);
            // the first lane carries the context boxes: one with a key has a second line
            let keyed = n == 0 && edit.context.iter().any(|b| b.key.is_some());
            if keyed { nodes.max(HEADER_H + ROW_H) } else { nodes }
        })
        .collect();
    let per_band = (((aw - PAD * 2.0 - head_w - END_W - GAP * 2.0) / (CHIP_W + CHIP_GAP)).floor() as usize).max(1);
    let bands = if detail { 1 } else { total.div_ceil(per_band).max(1) };
    let band_h = if detail {
        lane_h.iter().sum::<f32>() + LANE_GAP * (lanes.len() as f32 - 1.0)
    } else {
        lanes.len() as f32 * CHIP_H + LANE_GAP * (lanes.len() as f32 - 1.0)
    };
    let keys_h = if edit.keys.is_empty() { 0.0 } else { KEY_H + LANE_GAP };
    let height = PAD * 2.0 + bands as f32 * band_h + (bands as f32 - 1.0) * LANE_GAP * 1.5 + keys_h;
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
    if canvas_resp.double_clicked() {
        view = View::default();
    }
    if !detail {
        view.pan = 0.0;
    }
    let painter = ui.painter_at(canvas);
    painter.rect_filled(canvas, 4.0, ui.visuals().extreme_bg_color);

    // geometry of every node
    let mut rects: Vec<Option<egui::Rect>> = vec![None; edit.fx.len()];
    let mut ctx_rects: Vec<egui::Rect> = Vec::new();
    let mut ends: Vec<(Lane, egui::Rect, egui::Rect)> = Vec::new(); // (lane, in, out)
    let lane_y = |li: usize, band: usize| -> f32 {
        let base = canvas.top() + PAD + band as f32 * (band_h + LANE_GAP * 1.5);
        if detail {
            base + lane_h[..li].iter().sum::<f32>() + LANE_GAP * li as f32
        } else {
            base + li as f32 * (CHIP_H + LANE_GAP)
        }
    };
    let x0 = canvas.left() + PAD - view.pan;
    for (li, &l) in lanes.iter().enumerate() {
        let lslot = lane_slots(l);
        let h = if detail { lane_h[li] } else { CHIP_H };
        let input = egui::Rect::from_min_size(egui::pos2(x0, lane_y(li, 0)), egui::vec2(head_w, if detail { HEADER_H } else { CHIP_H }));
        for (c, col) in cols.iter().enumerate() {
            let Some(i) = col[lslot] else { continue };
            let (band, k) = if detail { (0, c) } else { (c / per_band, c % per_band) };
            let (w, x) = if detail {
                (col_w[c], x0 + head_w + GAP + col_x[c])
            } else {
                (CHIP_W, x0 + head_w + GAP + k as f32 * (CHIP_W + CHIP_GAP))
            };
            let nh = if detail { node_h(i) } else { CHIP_H };
            rects[i] = Some(egui::Rect::from_min_size(egui::pos2(x, lane_y(li, band)), egui::vec2(w, nh.min(h))));
        }
        if li == 0 {
            for n in 0..n_ctx {
                let c = n_fx + n;
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
                ctx_rects.push(egui::Rect::from_min_size(egui::pos2(x, lane_y(li, band)), egui::vec2(w, hh)));
            }
        }
        let cols_here = if li == 0 { total } else { n_fx };
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

    // wires: In → the lane's nodes in order → Out
    for &(l, input, output) in &ends {
        let lslot = lane_slots(l);
        let color = lane_color(l);
        let mut prev = input.right_center();
        let mut prev_off = false;
        for col in &cols {
            let Some(i) = col[lslot] else { continue };
            let Some(r) = rects[i] else { continue };
            let to = egui::pos2(r.left(), r.top() + HEADER_H * 0.5);
            wire(&painter, prev, to, color, prev_off);
            prev = egui::pos2(r.right(), r.top() + HEADER_H * 0.5);
            prev_off = edit.bypass[i];
        }
        if lslot == 0 {
            for r in &ctx_rects {
                let y = r.top() + r.height().min(HEADER_H) * 0.5;
                wire(&painter, prev, egui::pos2(r.left(), y), color, prev_off);
                prev = egui::pos2(r.right(), y);
                prev_off = false;
            }
        }
        wire(&painter, prev, egui::pos2(output.left(), output.top() + output.height().min(HEADER_H) * 0.5), color, prev_off);
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
            let name = match l {
                Lane::Picture => tr(k::fx::graph::PICTURE),
                Lane::Sound => tr(k::fx::graph::SOUND),
            };
            if ui.rect_contains_pointer(input) {
                *hover = Some(name.to_string());
            }
        }
        end_node(&painter, ui, output, lane_color(l), tr(k::fx::graph::OUTPUT));
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
        if resp.double_clicked() {
            ch.open = Some(b.target);
        }
    }

    // the nodes
    let mut remove: Option<usize> = key_remove;
    let mut moved: Option<(usize, usize)> = None;
    for (i, r) in rects.iter().enumerate() {
        let Some(r) = *r else { continue };
        // a node panned out of the canvas has no widgets (they would stand outside the window)
        if !r.intersects(canvas) {
            continue;
        }
        let lane = if both { lane_of(&edit.fx[i]) } else { single };
        let off = edit.bypass[i];
        let selected = *edit.selected == ChainSel::Fx(i);
        let stroke = if selected {
            ui.visuals().selection.stroke
        } else {
            egui::Stroke::new(1.0, lane_color(lane).gamma_multiply(if off { 0.35 } else { 0.8 }))
        };
        painter.rect_filled(r, 5.0, if off { ui.visuals().extreme_bg_color } else { ui.visuals().faint_bg_color });
        if crate::widgets::field_probe::on() || cfg!(test) {
            crate::widgets::field_probe::note(&format!("fx.node.{i}"), r);
        }
        painter.rect_stroke(r, 5.0, stroke, egui::StrokeKind::Inside);
        // the sockets
        let sy = r.top() + HEADER_H * 0.5;
        painter.circle_filled(egui::pos2(r.left(), sy), 3.5, lane_color(lane));
        painter.circle_filled(egui::pos2(r.right(), sy), 3.5, lane_color(lane));
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
        let mut drop_here = false;
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
                let room = inner.width() - 46.0;
                let text = node_name(ui, name, ui.visuals().text_color(), off, room);
                let drawn = ui.selectable_label(selected, text);
                let slot_id = egui::Id::new((&id, "slot", i));
                let resp = crate::app::hint_into(
                    hover,
                    ui.interact(drawn.rect.expand2(egui::vec2(0.0, SLOT_GRAB_PAD)), slot_id, egui::Sense::click_and_drag()),
                    tr(k::fx::SLOT_HINT),
                );
                if crate::widgets::field_probe::on() {
                    crate::widgets::field_probe::note(&format!("fx.slot.{i}"), resp.rect);
                }
                if resp.clicked() {
                    *edit.selected = ChainSel::Fx(i);
                    crate::tele_journal::record(crate::tele_journal::Kind::Act, &format!("effect selected: {name} (slot {i})"));
                }
                if resp.double_clicked() && !detail {
                    // a chip opens to the detailed nodes, scrolled to it
                    view.detail = Some(true);
                    view.pan = ((r.left() - canvas.left()) - aw * 0.5).max(0.0);
                }
                if resp.drag_started() {
                    egui::DragAndDrop::set_payload(ui.ctx(), (i, lane));
                }
                if name.chars().count() > crate::widgets::SLOT_NAME_CHARS {
                    resp.clone().on_hover_text(name.to_string());
                }
                if let Some(p) = resp.dnd_hover_payload::<(usize, Lane)>()
                    && p.1 != lane
                {
                    *hover = Some(tr(k::fx::graph::LANE_REFUSED).to_string());
                }
                if let Some(p) = resp.dnd_release_payload::<(usize, Lane)>() {
                    if p.1 == lane && p.0 != i {
                        moved = Some((p.0, i));
                    }
                    drop_here = true;
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
                        remove = Some(i);
                        ui.close();
                    }
                });
                // every node has its cross, a chip too (R.33: removing is one click on the slot)
                if crate::app::hint_into(hover, ui.small_button(crate::icons::CLEAR), tr(k::fx::REMOVE_SLOT)).clicked() {
                    remove = Some(i);
                }
            });
            if detail {
                main_rows_of(ui, &id, i, &mut edit, hover, &mut ch);
            }
        }
        let _ = drop_here;
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

    if let Some((from, to)) = moved {
        let slot = edit.fx.remove(from);
        edit.fx.insert(to, slot);
        let b = edit.bypass.remove(from);
        edit.bypass.insert(to, b);
        let c = edit.auto.remove(from);
        edit.auto.insert(to, c);
        *edit.selected = ChainSel::Fx(to);
        ch.fx = true;
        ch.bypass = true;
        ch.auto = true;
        ch.moved = Some((from, to));
    }
    if let Some(i) = remove {
        ch.removed = Some(i);
        edit.fx.remove(i);
        edit.bypass.remove(i);
        if i < edit.auto.len() {
            edit.auto.remove(i);
            ch.auto = true;
        }
        if let Some(sel) = edit.selected.fx() {
            *edit.selected = if edit.fx.is_empty() { ChainSel::Head } else { ChainSel::Fx(sel.min(edit.fx.len() - 1)) };
        }
        ch.fx = true;
        ch.bypass = true;
    }
    if edit.fx.is_empty() {
        ui.weak(tr(k::fx::CHAIN_EMPTY));
    }
    ch
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
            let l = ui.weak(format!("{} {}", crate::icons::CAT_DISTORT, crate::widgets::param_label(&spec.name)));
            crate::app::hint_into(hover, l, tr(k::fx::graph::CURVED));
            continue;
        }
        ui.horizontal(|ui| {
            if let kt_model::ParamType::Choice { variants } = &spec.ty {
                row_label(ui, &crate::widgets::param_label(&spec.name));
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
                row_label(ui, &crate::widgets::param_label(&spec.name));
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

fn wire(painter: &egui::Painter, a: egui::Pos2, b: egui::Pos2, color: egui::Color32, dim: bool) {
    let c = if dim { color.gamma_multiply(0.4) } else { color };
    if b.x < a.x - 1.0 {
        // a wrap to the next band of chips: down the column gap, back along the gap above the
        // destination's row, never across a node
        let gx = a.x + CHIP_GAP * 0.5;
        let gy = b.y - CHIP_H * 0.5 - LANE_GAP * 0.5;
        let bx = b.x - CHIP_GAP * 0.5;
        let pts = vec![a, egui::pos2(gx, a.y), egui::pos2(gx, gy), egui::pos2(bx, gy), egui::pos2(bx, b.y), b];
        painter.add(egui::Shape::line(pts, egui::Stroke::new(1.6, c)));
        return;
    }
    let dx = ((b.x - a.x) * 0.5).max(12.0);
    let pts = [a, egui::pos2(a.x + dx, a.y), egui::pos2(b.x - dx, b.y), b];
    painter.add(egui::epaint::CubicBezierShape::from_points_stroke(pts, false, egui::Color32::TRANSPARENT, egui::Stroke::new(1.6, c)));
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
    id: &(impl std::hash::Hash + std::fmt::Debug),
    edit: &mut ChainEdit<'_>,
    kinds: crate::fx_registry::PickKinds,
    query: &mut String,
    clipboard: &mut Option<kt_model::ops::FxChainBlock>,
    src_in: kt_time::Tick,
    hover: &mut Option<String>,
    preview: Option<&mut crate::fx_thumbs::PickerCtx<'_>>,
    ch: &mut ChainChanged,
    view: &mut View,
) {
    ui.horizontal_wrapped(|ui| {
        if crate::app::hint_into(hover, ui.checkbox(edit.master_on, tr(k::fx::CHAIN_ON)), tr(k::fx::CHAIN_ON_HINT)).changed() {
            ch.master = true;
        }
        ui.separator();
        let head_name = edit.head.as_ref().map(|h| h.label.clone());
        match crate::fx_registry::fx_picker_with_head(ui, (id, "add"), kinds, query, head_name.as_deref(), edit.presets.rows, hover, preview) {
            Some(crate::fx_registry::Picked::Preset(i)) => {
                if let Some(row) = edit.presets.rows.get(i) {
                    kt_model::ops::append_chain_block(edit.fx, edit.bypass, edit.auto, &row.block, src_in);
                    *edit.selected = ChainSel::Fx(edit.fx.len().saturating_sub(1));
                    ch.fx = true;
                    ch.bypass = true;
                    ch.auto = true;
                }
            }
            Some(crate::fx_registry::Picked::PresetAction(a)) => *edit.presets.action = Some(a),
            Some(crate::fx_registry::Picked::Fx(tmpl)) => {
                edit.fx.push(tmpl);
                edit.bypass.push(false);
                edit.auto.push(Vec::new());
                *edit.selected = ChainSel::Fx(edit.fx.len() - 1);
                ch.fx = true;
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
        };
        if !edit.fx.is_empty() && crate::app::hint_into(hover, ui.button(tr(k::fx::SAVE_PRESET)), tr(k::fx::SAVE_PRESET_HINT)).clicked() {
            *edit.presets.save = Some(block(edit));
        }
        if !edit.fx.is_empty() && crate::app::hint_into(hover, ui.button(tr(k::fx::COPY_CHAIN)), tr(k::fx::COPY_CHAIN_HINT)).clicked() {
            let b = block(edit);
            if let Ok(json) = serde_json::to_string_pretty(&b) {
                ui.ctx().copy_text(json);
            }
            *clipboard = Some(b);
        }
        if crate::app::hint_into(hover, ui.add_enabled(clipboard.is_some(), egui::Button::new(tr(k::fx::PASTE_CHAIN))), tr(k::fx::PASTE_CHAIN_HINT)).clicked()
            && let Some(b) = clipboard.clone()
        {
            kt_model::ops::append_chain_block(edit.fx, edit.bypass, edit.auto, &b, src_in);
            *edit.selected = ChainSel::Fx(edit.fx.len().saturating_sub(1));
            ch.fx = true;
            ch.bypass = true;
            ch.auto = true;
        }
        let fit = crate::app::hint_into(hover, ui.button(tr(k::fx::graph::FIT)), tr(k::fx::graph::FIT_HINT));
        if crate::widgets::field_probe::on() {
            crate::widgets::field_probe::note("fx.graph.fit", fit.rect);
        }
        if fit.clicked() {
            *view = View::default();
        }
    });
}
