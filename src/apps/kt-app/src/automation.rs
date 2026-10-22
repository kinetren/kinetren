//! A single automation widget (R.33 C3).
//!
//! The user's requirement, verbatim: “I would like a unified automation
//! widget, and to be able to switch between
//! graph<->list table… The graph should be as convenient as the project
//! timeline (reuse the widget code). And in the current
//! list table you need several parameters per row instead of one,
//! and they should be movable with the mouse (right now there is a stupidity —
//! the program seems to reset/sort neighbouring values if I
//! enter t=0.5 on the line after t=1.0)… And the preview is missing:
//! you guess blindly from normalised time, but it could be done so that
//! the cursor inside the mini-timeline moves the cursor in the
//! main timeline… Of course reverse playback has to be taken into account
//! as well as speed.”
//!
//! So here: ONE widget for all curves of the slot, a ruler and snap from the
//! project timeline, keys in seconds of material (the clip fraction — on a second
//! line, without clutter), a table with all parameters on the row and
//! sorting ONLY on release, a mini-timeline that moves the main
//! cursor.

use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::{ClipEvent, FxInstance, HasParams, ParamCurve, ParamSpec};
use kt_time::Tick;

/// The key grab radius. Wider than the drawing (rule 1 of the checklist): one should not need
/// to aim at a diamond with a 4.5 px radius.
const KEY_GRAB_PX: f32 = 9.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutoMode {
    #[default]
    Graph,
    Table,
}

/// Widget state (lives in the window, not in the model).
///
/// There is no lock among the fields any more: there is one for the whole program and it lives in
/// `UiState` (R.40 item 15) — the window only reads it via `AutoCtx`.
/// What the right-click on the graph aims at (R.63 item 5).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MenuTarget {
    /// Key (curve, index).
    Key(usize, usize),
    /// The segment between keys `ki` and `ki+1` of curve `ci`: the shape lives on
    /// the LEFT key of the segment.
    Segment(usize, usize),
}

#[derive(Debug, Clone, Default)]
pub struct AutoView {
    pub mode: AutoMode,
    /// Selected key: (curve, index).
    pub sel: Option<(usize, usize)>,
    /// Key under the mouse during a drag.
    pub drag: Option<(usize, usize)>,
    /// The curve into which a click on an empty spot of the graph puts a key.
    pub active: usize,
    /// Own axis window (the curve's domain) when the lock is off.
    pub win: Option<(i64, i64)>,
    /// Uncommitted time input in the table: (curve, key, seconds).
    /// While a row is being edited, the keys are NOT re-sorted — otherwise
    /// the row slides out from under the fingers on every digit.
    pub t_edit: Option<(usize, usize, f64)>,
    /// Screen positions of keys from the LAST draw: (curve, key, point).
    ///
    /// The drawing and the probe come from one place of truth (R.49, item 364). Before this
    /// round the hit test lived inside the draw loop and fired only on
    /// `interact_pointer_pos` — that is, with the button PRESSED. So the key
    /// stayed silent both on hover and on the right button: there was no hint, no menu,
    /// and “RMB deletes” was not introduced on purpose — deletion already exists
    /// by two paths, and a destructive right button would devalue the promise that
    /// “the right button is safe to try” everywhere else.
    ///
    /// The buffer is REUSED (`clear`, not `Vec::new`): invariant 12 —
    /// we do not allocate per frame in the draw paths.
    pub key_marks: Vec<(usize, usize, egui::Pos2)>,
    /// The frozen value window of the curve being pulled: (curve, lo, hi).
    /// Without freezing, the scale would be recomputed from the same keys and
    /// would “breathe” under the cursor.
    pub drag_win: Option<(usize, f64, f64)>,
    /// TARGET OF THE RIGHT CLICK while the menu is open (R.63 item 5): either the key or
    /// the segment between two keys.
    ///
    /// Before this, both the key and the segment were computed from `hover_pos()` EVERY FRAME,
    /// while the `context_menu` contents are redrawn every frame: the pointer
    /// moved to the items — the hit was lost — and the menu closed by itself.
    /// “The context menu just disappears, I don't have time to reach
    /// it” (the owner, R.63 item 5).
    pub menu_target: Option<MenuTarget>,
    /// TYPED formula text: (curve, text). R.63 item 3.
    ///
    /// The field holds its own text between frames and gives it to the model ONLY on
    /// confirmation (Enter or focus loss). Before this, the text was read from the
    /// model every frame and applied at the first legal prefix:
    /// typing `sin(t)` died at `sin(` (“erases the formula”), while the first letter
    /// `t` is legal by itself — it was applied at once and zeroed the keys of the curve
    /// (“either nothing happens at all”). The owner, R.63 item 3.
    pub formula: Option<(usize, String)>,
    /// TYPED formula text of the SEGMENT: (curve, key, text) — R.67.
    pub seg_formula: Option<(usize, usize, String)>,
    /// The segment selected by a right click (curve, left key): its block shows the
    /// segment formula field.
    pub sel_seg: Option<(usize, usize)>,
    /// The segment under the pointer — its shape-parameter handle is drawn.
    pub hover_seg: Option<(usize, usize)>,
    /// Drag of the shape-PARAMETER HANDLE of the segment (R.67): vertical — the main
    /// parameter (tension/amplitude), Shift+horizontal — periods.
    pub shape_drag: Option<ShapeDrag>,
    /// Screen midpoints of the segments of the active curve from the LAST draw:
    /// (key, point) — the probe of the handle by what is drawn, as for keys.
    pub seg_marks: Vec<(usize, egui::Pos2)>,
}

/// Drag of the shape-parameter handle (R.67).
#[derive(Debug, Clone, Copy)]
pub struct ShapeDrag {
    pub ci: usize,
    pub ki: usize,
    pub from: egui::Pos2,
    pub shape0: kt_model::SegShape,
    /// Value window at the moment of capture: the amplitude is pulled in axis units.
    pub span: f64,
    pub plot_h: f32,
}

/// A timeline event as a link target (R.67): the caption, “this is mine” and the
/// properties — (`LinkPath` address, caption).
pub struct LinkEvent {
    pub label: String,
    pub this: bool,
    pub params: Vec<(String, String)>,
}

/// The grab of the shape-parameter handle — wider than the drawing (rule 1).
const SEG_GRAB_PX: f32 = 14.0;

impl AutoView {
    /// The segment under the point — by the drawn midpoints of the active curve.
    pub fn seg_at(&self, pos: egui::Pos2) -> Option<usize> {
        self.seg_marks.iter().find(|(_, p)| (*p - pos).length() < SEG_GRAB_PX).map(|(ki, _)| *ki)
    }

    /// The key under the point. One probe for all channels: press, hover and
    /// the right button ask ONE AND THE SAME thing.
    pub fn key_at(&self, pos: egui::Pos2) -> Option<(usize, usize)> {
        self.key_marks
            .iter()
            .find(|(_, _, p)| (*p - pos).length() < KEY_GRAB_PX)
            .map(|(ci, ki, _)| (*ci, *ki))
    }

    /// Visible window of the time axis in the curve's domain.
    ///
    /// **The linked graph shows the timeline window AS IS** (R.40).
    /// Previously it was clamped to the range of the carrier — and that was the defect
    /// the user complained about: “when I drag the left edge of an
    /// event, my automation keys also shift toward the start.”
    /// The key did NOT move (`clip_slice` does not touch it, it stays on the same
    /// frame of the material and at the same moment of the timeline) — the AXIS was lying: it
    /// was normalised to the event, the event got shorter, and the key moved,
    /// say, from the middle of the graph to a third. It is fixed by the axis, not by the data.
    ///
    /// An unlinked graph still lives in its own window, and without a window
    /// it shows the carrier in full.
    pub fn axis(&self, ctx: &AutoCtx<'_>) -> (i64, i64) {
        let (r0, r1) = (ctx.range.0.0, ctx.range.1.0.max(ctx.range.0.0 + 1));
        let (a, b) = match (ctx.linked, self.win) {
            (true, _) => (ctx.view.0.0, ctx.view.1.0),
            (false, Some(w)) => (w.0.clamp(r0, r1), w.1.clamp(r0, r1)),
            (false, None) => (r0, r1),
        };
        // a window narrower than one frame is meaningless and divides by zero
        if b - a < ctx.frame_step.max(1) { (r0, r1) } else { (a, b) }
    }
}

/// Vertical axis of the graph: how a value is placed on the height (R.57, item 456).
///
/// A rule of the CLASS, not of the parameter name: a signed RATIO is
/// a speed, and its −32…32 on a linear axis makes an ordinary curve 0.66…1.0
/// a straight line at the very edge (the very complaint of R.34 about the old slowed graph).
/// A logarithm of the absolute value stretches the neighbourhood of one and leaves zero
/// as zero; inside the band ±knee the axis is linear, otherwise the logarithm would go to −∞.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axis {
    Linear,
    SymLog { knee: f64 },
}

/// Axis of the parameter by its SPEC.
pub fn axis_of(spec: &ParamSpec) -> Axis {
    if spec.unit == kt_model::ParamUnit::Ratio && spec.min < 0.0 {
        Axis::SymLog { knee: 0.25 }
    } else {
        Axis::Linear
    }
}

impl Axis {
    /// Value → axis coordinate (odd, monotonic, continuous).
    pub fn map(self, v: f64) -> f64 {
        match self {
            Axis::Linear => v,
            Axis::SymLog { knee } => {
                let a = v.abs();
                let g = if a <= knee { a / knee } else { 1.0 + (a / knee).log2() };
                g.copysign(v)
            }
        }
    }

    /// Back: axis coordinate → value.
    pub fn unmap(self, g: f64) -> f64 {
        match self {
            Axis::Linear => g,
            Axis::SymLog { knee } => {
                let a = g.abs();
                let v = if a <= 1.0 { a * knee } else { knee * (a - 1.0).exp2() };
                v.copysign(g)
            }
        }
    }
}

/// Value window of the curve: frozen for the duration of a drag or computed from
/// its keys.
fn win_of(c: &ParamCurve, spec: &ParamSpec, view: &AutoView, ci: usize) -> (f64, f64) {
    match view.drag_win {
        Some((i, lo, hi)) if i == ci => (lo, hi),
        _ => value_window(c, spec),
    }
}

/// What the widget needs to know about an event.
/// The owner of the curves through the widget's eyes (R.34 K1). The widget no longer knows about the
/// clip: it needs only the parameter specs, the time-axis range and the
/// translation “curve position ↔ timeline tick”. Thanks to this the same
/// widget serves an effect slot, a camera and a transition — otherwise every
/// new carrier of parameters would get its own keyframe editor, and that is exactly how
/// slowed and pan3d drifted apart (item 221).
pub struct AutoCtx<'a> {
    /// Parameter specs of the carrier (min/max/step/unit).
    pub specs: &'a [ParamSpec],
    /// FULL range of the time axis in the curve's domain (the carrier's bounds).
    pub range: (Tick, Tick),
    /// The window of the MAIN timeline in the curve's domain — what the graph
    /// shows with the lock on (R.38 item 14).
    pub view: (Tick, Tick),
    /// The axis lock. ONE FOR THE WHOLE PROGRAM, living in `UiState` (R.40 item 15):
    /// previously it was a field of `AutoView` and died together with the window. The widget
    /// only READS it, and returns a request to toggle in `AutoResult` —
    /// otherwise one window would silently rule the state of all the others.
    pub linked: bool,
    /// The segment shape of NEW curves (a project field, R.61). A new KEY
    /// inherits the shape from the segment it splits — the default is needed only
    /// when there is nothing to split.
    pub default_shape: kt_model::SegShape,
    /// Curve domain → timeline tick (the mini-timeline moves the main cursor).
    pub to_timeline: &'a dyn Fn(Tick) -> Tick,
    /// Timeline tick → curve domain (the cursor position on the graph).
    pub to_curve: &'a dyn Fn(Tick) -> Tick,
    /// Step of the project frame — the key position is aligned to it.
    pub frame_step: i64,
    /// “Driven by…” link targets (R.67): timeline events and their properties. `None`
    /// — a carrier of a link never exists (transition, camera, morpher patch), and there
    /// will be no dead item in the block. Built ON PRESS, not per frame.
    pub links: Option<&'a dyn Fn() -> Vec<LinkEvent>>,
}

impl AutoCtx<'_> {
    /// Fraction of the range (the second time label: “seconds and percentages”).
    fn frac(&self, t: Tick) -> f64 {
        let span = (self.range.1.0 - self.range.0.0).max(1);
        ((t.0 - self.range.0.0) as f64 / span as f64).clamp(0.0, 1.0)
    }

    fn spec(&self, param: &str) -> Option<&ParamSpec> {
        self.specs.iter().find(|s| s.name == param)
    }

    /// Curve caption. Built-in series have their snake_case names translated by the
    /// catalogue; for a DYNAMIC owner (a public parameter of a composition, a plugin tomorrow)
    /// the name was made up by its AUTHOR and cannot get into the catalogue — it lies as a
    /// ready-made caption in `ParamSpec::label`
    /// (R.59). The widget must draw it verbatim, bypassing `tr()` (t684).
    pub(crate) fn label(&self, param: &str) -> String {
        match self.spec(param) {
            Some(s) if !s.label.is_empty() => s.label.to_string(),
            _ => crate::widgets::param_label(param),
        }
    }
}

/// Window of the main timeline in the curve's domain. It is translated by the SAME
/// mapping as everything else; for a reversed carrier the ends swap
/// places — hence min/max, not “first-second”.
pub fn view_in_curve(to_curve: &dyn Fn(Tick) -> Tick, tl_view: (Tick, Tick)) -> (Tick, Tick) {
    let (a, b) = (to_curve(tl_view.0).0, to_curve(tl_view.1).0);
    (Tick(a.min(b)), Tick(a.max(b)))
}

/// Context for an effect slot on an event — the former behaviour of the widget
/// in one line (domain = the clip's material).
#[allow(clippy::too_many_arguments)]
pub fn clip_ctx<'a>(
    clip: &'a ClipEvent,
    fx: &'a FxInstance,
    frame_step: i64,
    to_tl: &'a dyn Fn(Tick) -> Tick,
    to_mat: &'a dyn Fn(Tick) -> Tick,
    tl_view: (Tick, Tick),
    linked: bool,
    default_shape: kt_model::SegShape,
    links: Option<&'a dyn Fn() -> Vec<LinkEvent>>,
) -> AutoCtx<'a> {
    AutoCtx {
        default_shape,
        specs: fx.param_specs(),
        range: (clip.source_in, Tick(clip.source_in.0 + clip.duration.0.max(1))),
        view: view_in_curve(to_mat, tl_view),
        to_timeline: to_tl,
        to_curve: to_mat,
        frame_step,
        linked,
        links,
    }
}

/// Link targets for the event `me` in the document (R.67): each timeline event
/// and its numeric properties — chain slots and the generator — by `LinkPath`
/// addresses. Its own event — first, with the address `self/…`.
pub fn link_events(doc: &kt_model::Project, me: kt_model::ClipId) -> Vec<LinkEvent> {
    let mut out = Vec::new();
    let mut clips: Vec<&ClipEvent> = doc.clips.iter().collect();
    clips.sort_by_key(|c| (c.id != me, c.start.0, c.id.0));
    for c in clips {
        let this = c.id == me;
        let who = if this { "self".to_string() } else { c.id.0.to_string() };
        let mut params = Vec::new();
        for (i, f) in c.fx.iter().enumerate() {
            let fx_name = crate::fx_editor::fx_display_name(f);
            for sp in f.auto_params() {
                let n: &str = &sp.name;
                params.push((format!("{who}/fx{i}/{n}"), format!("{fx_name}: {}", crate::widgets::param_label(n))));
            }
        }
        if let Some(g) = &c.r#gen {
            for sp in g.param_specs() {
                if matches!(sp.ty, kt_model::ParamType::Num) && sp.kind == kt_model::ParamKind::Smooth {
                    let n: &str = &sp.name;
                    params.push((
                        format!("{who}/gen/{n}"),
                        format!("{}: {}", crate::gen_ui::kind_name(g), crate::widgets::param_label(n)),
                    ));
                }
            }
        }
        if params.is_empty() {
            continue;
        }
        let label = format!("#{} {:.1} s", c.id.0, c.start.0 as f64 / 90_000.0);
        out.push(LinkEvent { label, this, params });
    }
    out
}

#[derive(Default)]
pub struct AutoResult {
    pub changed: bool,
    /// A formula rejection (parse error, a foreign name) — as an ERROR in the status line
    /// (rule 16), not as a per-frame hint: a hover hint would immediately cover it
    /// with the text of the field.
    pub error: Option<String>,
    /// Where to move the MAIN cursor (a gesture on the mini-timeline ruler).
    pub seek: Option<Tick>,
    /// A request to toggle the axis lock (R.40 item 15). Only the lock button writes it:
    /// “it must be made so that it has to be TURNED OFF for the timeline
    /// synchronisation to be disabled” — verbatim.
    pub set_linked: Option<bool>,
    /// A request to change the window of the MAIN timeline: with the lock ON the wheel over the graph
    /// zooms and pans precisely it — the axes stay
    /// in sync, and the gesture is not lost.
    pub set_view: Option<(Tick, Tick)>,
    /// Which key the context menu is open on (R.49, item 364). Needed by the
    /// owner of the window to record the panel target: the widget itself does not
    /// know about `KtApp`.
    pub key_menu: Option<(usize, usize)>,
}

/// Requests of the widget about the AXIS, collected from several graphs of one window
/// (R.40 item 15). The lock is one per program, so a window with several
/// slots must merge them and apply once.
#[derive(Default, Clone, Copy)]
pub struct AxisReq {
    pub set_linked: Option<bool>,
    pub set_view: Option<(Tick, Tick)>,
}

impl AxisReq {
    /// Merge with the requests of ANOTHER section of the same window (the head of the strip).
    pub fn merge_req(self, r: &AxisReq) -> Self {
        AxisReq {
            set_linked: r.set_linked.or(self.set_linked),
            set_view: r.set_view.or(self.set_view),
        }
    }

    pub fn merge(self, r: &AutoResult) -> Self {
        AxisReq {
            set_linked: r.set_linked.or(self.set_linked),
            set_view: r.set_view.or(self.set_view),
        }
    }
}

fn secs(t: Tick) -> f64 {
    t.0 as f64 / 90_000.0
}

pub fn curve_color(i: usize) -> egui::Color32 {
    const C: [egui::Color32; 6] = [
        egui::Color32::from_rgb(120, 180, 255),
        egui::Color32::from_rgb(255, 190, 120),
        egui::Color32::from_rgb(140, 230, 150),
        egui::Color32::from_rgb(230, 140, 210),
        egui::Color32::from_rgb(230, 220, 130),
        egui::Color32::from_rgb(160, 200, 220),
    ];
    C[i % C.len()]
}

/// Value window of the curve VERTICALLY. Taken from its own keys with a
/// margin, not from the whole spec range: for speed the spec is 0.25..32, and
/// on the full range an ordinary curve 0.7..1.0 degenerates into a line at the
/// lower edge (exactly the complaint that existed about the old slowed graph).
/// The window is computed IN AXIS COORDINATES (R.57): on a signed speed a
/// linear window would press an ordinary curve 0.66…1.0 against the very edge of
/// −32…32 and draw a straight line (complaint of R.34).
fn value_window(c: &ParamCurve, spec: &ParamSpec) -> (f64, f64) {
    let ax = axis_of(spec);
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for k in &c.keys {
        let g = ax.map(k.v);
        lo = lo.min(g);
        hi = hi.max(g);
    }
    let (gmin, gmax) = (ax.map(spec.min), ax.map(spec.max));
    if !lo.is_finite() || !hi.is_finite() {
        return (gmin, gmax);
    }
    let span = (hi - lo).max((gmax - gmin) * 0.02);
    let pad = span * 0.15;
    ((lo - pad).max(gmin), (hi + pad).min(gmax))
}

/// The whole widget: a view switch + graph or table.
pub fn draw(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    ctx: &AutoCtx<'_>,
    curves: &mut Vec<ParamCurve>,
    view: &mut AutoView,
    playhead: Tick,
    hover: &mut Option<String>,
) -> AutoResult {
    let mut out = AutoResult::default();
    ui.horizontal(|ui| {
        ui.label(tr(k::fx::AUTO_CLIP));
        // switching the view — the requirement “to be able to switch between
        // graph<->list table”
        for (m, label) in
            [(AutoMode::Graph, tr(k::fx::VIEW_GRAPH)), (AutoMode::Table, tr(k::fx::VIEW_TABLE))]
        {
            if ui.selectable_label(view.mode == m, label).clicked() {
                view.mode = m;
            }
        }
        if let Some((c, k)) = view.sel
            && let Some(key) = curves.get(c).and_then(|c| c.keys.get(k))
        {
            // BOTH times at once, but on one line: seconds of the material and the
            // share of the event (the user: “write both, but without
            // cluttering”)
            ui.weak(crate::tf!(
                k::fx::KEY_AT,
                format!("{:.3}", secs(key.t)),
                format!("{:.0}", ctx.frac(key.t) * 100.0)
            ));
        }
    });
    if curves.is_empty() {
        return out;
    }
    // the active curve could disappear (a parameter was deleted) — otherwise a click on an
    // empty spot would silently go nowhere
    view.active = view.active.min(curves.len() - 1);
    toolbar(ui, ctx, curves, view, playhead, &mut out, hover);
    match view.mode {
        AutoMode::Graph => graph(ui, id, ctx, curves, view, playhead, &mut out, hover),
        AutoMode::Table => table(ui, id, ctx, curves, view, &mut out),
    }
    // The “CURVE AS PARAMETER” BLOCK — one for all carriers (R.67), below the graph,
    // for the curve that the person sees as selected
    curve_block(ui, id, ctx, curves, view, &mut out, hover);
    out
}

/// Buttons controlling the graph elements (R.38 item 18, verbatim: “still missing
/// buttons to control the elements of the automation timeline”).
///
/// They live IN THE WIDGET ITSELF, not in the FX window: all carriers of curves get them
/// at once — an effect slot, a camera, a transition, a morpher. That is exactly how
/// slowed and pan3d drifted apart, when each had its own key editor (item 221).
#[allow(clippy::too_many_arguments)]
fn toolbar(
    ui: &mut egui::Ui,
    ctx: &AutoCtx<'_>,
    curves: &mut [ParamCurve],
    view: &mut AutoView,
    playhead: Tick,
    out: &mut AutoResult,
    hover: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        // selecting the active curve — by the COLOUR of its line on the graph: otherwise
        // “added a second parameter, but the first one is edited” (item 17). Until
        // R.38 a new parameter did not become active at all.
        for (ci, c) in curves.iter().enumerate() {
            let name = ctx.label(&c.param);
            // a derived curve is DISTINGUISHABLE: next to its name one sees what it is
            // driven by — the source address or a formula (R.61)
            let name = match &c.source {
                kt_model::CurveSource::Keys => name,
                kt_model::CurveSource::Link { path, .. } => format!("{name} <- {path}"),
                kt_model::CurveSource::Formula { .. } => format!("{name} = ƒ"),
            };
            let text = egui::RichText::new(name).color(curve_color(ci));
            let text = if c.source.is_keys() { text } else { text.italics() };
            if crate::app::hint_into(
                hover,
                ui.selectable_label(view.active == ci, text),
                tr(k::fx::ACTIVE_CURVE_HINT),
            )
            .clicked()
            {
                view.active = ci;
                view.sel = None;
            }
        }
        // the formula field, the link and “unlink” live in the CURVE BLOCK below the
        // graph (R.67, `curve_block`) — one for all carriers
        ui.separator();
        // ---- keys ----
        //
        // THREE buttons (R.40 item 18, verbatim: “beyond the key-setting button
        // there are no two buttons: a key BEFORE and a key AFTER the current position
        // (to fix the previous and next parameters)”). “Before” and “after” set a key
        // on the NEIGHBOURING FRAME with the value of the curve at this point: the shape
        // of the curve does not change, but the segment to the left (or right) of the
        // cursor becomes pinned — and
        // further editing of the key under the cursor no longer pulls it.
        //
        // Ctrl — onto ALL curves of the slot at once. This is the former button
        // “+ keyframe at playhead” from the FX window: it differed from “+” by exactly
        // this and did not say a word about it (rule 18: the name of an item says
        // WHAT will happen).
        let ph = (ctx.to_curve)(playhead);
        let step = ctx.frame_step.max(1);
        let all = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        for (icon, at, hint) in [
            (crate::icons::KEY_BEFORE, Tick(ph.0 - step), tr(k::fx::KEY_BEFORE_HINT)),
            (crate::icons::PLUS, ph, tr(k::fx::KEY_ADD_HINT)),
            (crate::icons::KEY_AFTER, Tick(ph.0 + step), tr(k::fx::KEY_AFTER_HINT)),
        ] {
            if !crate::app::hint_into(hover, ui.small_button(icon), hint).clicked() {
                continue;
            }
            let active = view.active;
            let targets: Vec<usize> = if all { (0..curves.len()).collect() } else { vec![active] };
            for ci in targets {
                let Some(c) = curves.get_mut(ci) else { continue };
                let v = kt_model::curve_value(&c.keys, at);
                kt_model::insert_key(&mut c.keys, at, v, ctx.default_shape);
            }
            if let Some(c) = curves.get(active) {
                view.sel = c.keys.iter().position(|k| k.t == at).map(|ki| (active, ki));
            }
            out.changed = true;
        }
        // a jump between the keys of the active curve moves the MAIN cursor: “it is immediately
        // visible what it belongs to”
        for (icon, fwd, hint) in [
            (crate::icons::SKIP_BACK, false, tr(k::fx::KEY_PREV_HINT)),
            (crate::icons::SKIP_FWD, true, tr(k::fx::KEY_NEXT_HINT)),
        ] {
            if crate::app::hint_into(hover, ui.small_button(icon), hint).clicked() {
                let c = &curves[view.active];
                let next = if fwd {
                    c.keys.iter().position(|k| k.t.0 > ph.0)
                } else {
                    c.keys.iter().rposition(|k| k.t.0 < ph.0)
                };
                if let Some(ki) = next {
                    view.sel = Some((view.active, ki));
                    out.seek = Some((ctx.to_timeline)(c.keys[ki].t));
                }
            }
        }
        let has_sel =
            view.sel.is_some_and(|(c, k)| curves.get(c).is_some_and(|c| k < c.keys.len()));
        if crate::app::hint_into(
            hover,
            ui.add_enabled(has_sel, egui::Button::new(crate::icons::TRASH).small()),
            tr(k::fx::KEY_DEL_HINT),
        )
        .clicked()
            && let Some((ci, ki)) = view.sel
        {
            curves[ci].keys.remove(ki);
            view.sel = None;
            out.changed = true;
        }
        ui.separator();
        // ---- time axis (item 14) ----
        // THE ONLY place that releases the lock (R.40 item 15, verbatim: “it must be made so that it
        // has to be RELEASED for the timeline synchronisation to be disabled”). Formerly ANY wheel
        // over the graph released it — including
        // scrolling the FX window itself past the graph.
        if crate::app::hint_into(
            hover,
            ui.selectable_label(ctx.linked, crate::icons::LOCK),
            tr(k::fx::AXIS_LINK_HINT),
        )
        .clicked()
        {
            out.set_linked = Some(!ctx.linked);
            if ctx.linked {
                // releasing the lock, we keep THE SAME window that was visible:
                // “nothing jumps” (rule 3 of the checklist)
                let (a, b) = view.axis(ctx);
                view.win = Some((a, b));
            }
        }
        // “fit” shows the carrier in full — this is an EXPLICIT request about the
        // axis of the graph, so it releases the lock, and releases it VISIBLY
        // (the lock button goes out at once), not silently on the way
        if crate::app::hint_into(
            hover,
            ui.small_button(crate::icons::ARROWS_OUT),
            tr(k::fx::AXIS_FIT_HINT),
        )
        .clicked()
        {
            out.set_linked = Some(false);
            view.win = None;
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn graph(
    ui: &mut egui::Ui,
    _id: impl std::hash::Hash + std::fmt::Debug + Copy,
    ctx: &AutoCtx<'_>,
    curves: &mut [ParamCurve],
    view: &mut AutoView,
    playhead: Tick,
    out: &mut AutoResult,
    hover: &mut Option<String>,
) {
    const RULER_H: f32 = 16.0;
    let h = 150.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, egui::Color32::from_gray(18));

    // the X axis — the VISIBLE window (the timeline's lock or its own); the ruler and snap
    // are taken from the project timeline
    let (m0, m1) = view.axis(ctx);
    let tpp = (m1 - m0) as f64 / rect.width().max(1.0) as f64;

    // the wheel over the graph: scroll — pan, Ctrl — zoom anchored under the
    // pointer. The same gestures as on the main timeline (rule 17: paired
    // methods behave identically), and any of them releases the lock.
    if resp.hovered() {
        let (scroll, zoom) = ui.input(|i| {
            let mut dx = 0.0f32;
            let mut z = 1.0f32;
            for e in &i.events {
                if let egui::Event::MouseWheel { delta, modifiers, .. } = e {
                    if modifiers.ctrl || modifiers.command {
                        z *= (1.0 + delta.y * 0.15).clamp(0.2, 5.0);
                    } else {
                        dx += delta.x + delta.y;
                    }
                }
            }
            (dx, z)
        });
        if scroll != 0.0 || zoom != 1.0 {
            let anchor = ui
                .input(|i| i.pointer.hover_pos())
                .map(|p| ((p.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0) as f64)
                .unwrap_or(0.5);
            let span = (m1 - m0) as f64;
            let at = m0 as f64 + span * anchor;
            let ns = (span / zoom as f64).max(ctx.frame_step.max(1) as f64);
            let shift = -scroll as f64 * tpp * 8.0;
            let a = at - ns * anchor + shift;
            let (na, nb) = (a.round() as i64, (a + ns).round() as i64);
            if ctx.linked {
                // THE LOCK IS ON — the gesture goes to the MAIN timeline, the axes stay
                // in sync (R.40 item 15). Formerly it silently released the lock, and
                // even scrolling the FX window itself with the wheel over the
                // graph released it: “the little lock does not fully work”.
                out.set_view = Some(((ctx.to_timeline)(Tick(na)), (ctx.to_timeline)(Tick(nb))));
            } else {
                view.win = Some((na, nb));
            }
        }
    }
    let x_of = |t: i64| rect.left() + ((t - m0) as f64 / tpp) as f32;
    let t_of = |x: f32| m0 + ((x - rect.left()) as f64 * tpp) as i64;
    let plot = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top() + RULER_H),
        egui::pos2(rect.right(), rect.bottom()),
    );
    // the graph rectangle — for the live scenario (R.67): clicks on an empty
    // spot place keys, one must aim at them by the program's data
    crate::widgets::field_probe::note("auto.graph", plot);

    // ruler: the same ladder of steps and captions as on the project timeline
    let (minor, label) = crate::timeline::ruler_steps(tpp);
    let mut t = m0 - m0.rem_euclid(minor);
    while t <= m1 {
        let x = x_of(t);
        if x >= rect.left() && x <= rect.right() {
            let major = t % label == 0;
            p.line_segment(
                [
                    egui::pos2(x, rect.top()),
                    egui::pos2(x, rect.top() + if major { RULER_H } else { 5.0 }),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_gray(if major { 110 } else { 60 })),
            );
            if major {
                p.text(
                    egui::pos2(x + 3.0, rect.top() + 1.0),
                    egui::Align2::LEFT_TOP,
                    crate::timeline::format_ruler_time_step(t, label),
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_gray(150),
                );
            }
        }
        t += minor;
    }
    // a caption of the tick that went off to the left is pressed against the edge (R.40 item 14): the
    // graph's axis is the MATERIAL of the event; when zooming its start goes away, and without this
    // no time remained on the ruler
    {
        let pinned = crate::timeline::ruler_pinned_label(m0, label);
        if x_of(pinned) < rect.left() {
            let next = x_of(pinned + label);
            let g = p.layout_no_wrap(
                crate::timeline::format_ruler_time_step(pinned, label),
                egui::FontId::proportional(10.0),
                egui::Color32::from_gray(120),
            );
            if g.size().x + 6.0 <= next - rect.left() {
                p.galley(
                    egui::pos2(rect.left() + 2.0, rect.top() + 1.0),
                    g,
                    egui::Color32::from_gray(120),
                );
            }
        }
    }

    // the main timeline cursor in material coordinates
    let ph_mat = (ctx.to_curve)(playhead);
    if ph_mat.0 >= m0 && ph_mat.0 <= m1 {
        let x = x_of(ph_mat.0);
        p.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 90, 90)),
        );
    }

    // curves and keys
    view.key_marks.clear();
    for (ci, c) in curves.iter().enumerate() {
        let Some(spec) = ctx.spec(&c.param) else { continue };
        let (mut lo, mut hi) = win_of(c, spec, view, ci);
        // the window is already IN AXIS COORDINATES (R.57): the value is translated into them
        let ax = axis_of(spec);
        // the result of the layer/source — by the same evaluator as the render
        // (`t` from the start of the material, the body edge is held); the value window
        // widens to fit it, otherwise the stroke was pinned to the edge (R.67)
        let derived = !c.source.is_keys();
        let pv: Vec<kt_model::AutoKey> = if derived {
            kt_model::formula_preview(c, ctx.range, Tick(m0), Tick(m1), (m1 - m0).max(1) / 160)
        } else {
            Vec::new()
        };
        if view.drag_win.is_none_or(|(i, _, _)| i != ci) {
            for k in &pv {
                let g = ax.map(k.v);
                if g.is_finite() {
                    lo = lo.min(g).max(ax.map(spec.min));
                    hi = hi.max(g).min(ax.map(spec.max));
                }
            }
        }
        let y_of = |v: f64| -> f32 {
            let u = ((ax.map(v) - lo) / (hi - lo).max(1e-9)).clamp(0.0, 1.0) as f32;
            plot.bottom() - 6.0 - u * (plot.height() - 12.0)
        };
        // ZERO LINE (R.57, item 456): the window crosses zero — so
        // “forward” and “backward” must be distinguished not only by the caption. This is
        // a CLASS, not a speed case: pan and dB are signed too.
        if lo < 0.0 && hi > 0.0 {
            let y0 = y_of(0.0);
            p.add(egui::Shape::line_segment(
                [egui::pos2(plot.left(), y0), egui::pos2(plot.right(), y0)],
                egui::Stroke::new(1.0, crate::theme::AXIS_ZERO),
            ));
            p.text(
                egui::pos2(plot.left() + 2.0, y0),
                egui::Align2::LEFT_CENTER,
                "0",
                egui::FontId::proportional(9.0),
                crate::theme::AXIS_ZERO,
            );
        }
        // THE SEGMENT UNDER THE MENU is highlighted (R.63 item 5): otherwise it is unclear what
        // the shape being chosen now belongs to
        if let Some(MenuTarget::Segment(mc, mk)) = view.menu_target
            && mc == ci
            && let (Some(a), Some(b)) = (c.keys.get(mk), c.keys.get(mk + 1))
        {
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x_of(a.t.0), plot.top()),
                    egui::pos2(x_of(b.t.0), plot.bottom()),
                ),
                0.0,
                egui::Color32::from_rgba_unmultiplied(120, 160, 255, 28),
            );
        }
        let col = curve_color(ci);
        // THE VALUE LINE — by sampling the same `curve_value` that the render computes, per pixel (R.67):
        // a zigzag is seen as a zigzag, a step as a step,
        // a segment formula as its own line. Before this the line ran straight between keys,
        // and “you cannot visually see what the curve is there” (the owner).
        // The step is 2 px, no coarser than one frame per pixel.
        if c.keys.len() > 1 {
            let (ka, kb) = (c.keys[0].t.0, c.keys[c.keys.len() - 1].t.0);
            let (xa, xb) = (x_of(ka).max(plot.left()), x_of(kb).min(plot.right()));
            let mut pts: Vec<egui::Pos2> = Vec::with_capacity(((xb - xa) / 2.0).max(0.0) as usize + 2);
            let mut x = xa;
            while x <= xb {
                let t = t_of(x).clamp(ka, kb);
                pts.push(egui::pos2(x, y_of(kt_model::curve_value(&c.keys, Tick(t)))));
                x += 2.0;
            }
            pts.push(egui::pos2(xb, y_of(kt_model::curve_value(&c.keys, Tick(kb)))));
            // for a curve with a layer the `k` line is solid but quieter: the result
            // (the stroke) is what sounds
            let stroke = egui::Stroke::new(1.0, if derived { col.gamma_multiply(0.55) } else { col });
            p.add(egui::Shape::line(pts, stroke));
        }
        // the RESULT of the layer/source — as a stroke, by the same evaluator as the
        // render (`t` from the start of the material, the body edge is held): a correct
        // formula no longer gives an empty space, and the line does not drift on scroll
        if derived && pv.len() > 1 {
            let pts: Vec<egui::Pos2> = pv.iter().map(|k| egui::pos2(x_of(k.t.0), y_of(k.v))).collect();
            p.add(egui::Shape::dashed_line(&pts, egui::Stroke::new(1.5, col), 5.0, 3.0));
        }
        // THE SHAPE-PARAMETER HANDLE (R.67) — at the midpoint of the segment of the active
        // curve, for shapes with a parameter: under the pointer and on the one being dragged
        if ci == view.active {
            view.seg_marks.clear();
            for ki in 0..c.keys.len().saturating_sub(1) {
                let (a, b) = (&c.keys[ki], &c.keys[ki + 1]);
                let mid = Tick((a.t.0 + b.t.0) / 2);
                let m = egui::pos2(x_of(mid.0), y_of(kt_model::curve_value(&c.keys, mid)));
                if shape_main_param(a.shape).is_some() && m.x >= plot.left() && m.x <= plot.right() {
                    view.seg_marks.push((ki, m));
                    let shown = view.hover_seg == Some((ci, ki))
                        || view.shape_drag.is_some_and(|d| d.ci == ci && d.ki == ki);
                    if shown {
                        let r = egui::Rect::from_center_size(m, egui::vec2(8.0, 8.0));
                        p.rect_filled(r, 1.0, col);
                        p.rect_stroke(r, 1.0, egui::Stroke::new(1.0, egui::Color32::WHITE), egui::StrokeKind::Outside);
                    }
                }
            }
        }
        // a DERIVED curve (link or formula) — the key markers are there
        // if the keys exist: the layer stands ABOVE them, and the person edits exactly the
        // keys; the result stroke cannot be edited with the mouse (it has no points).
        if derived && c.keys.is_empty() {
            continue;
        }
        for (ki, k) in c.keys.iter().enumerate() {
            let (x, y) = (x_of(k.t.0), y_of(k.v));
            let selected = view.sel == Some((ci, ki));
            let r = if selected { 6.0 } else { 4.5 };
            p.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x, y - r),
                    egui::pos2(x + r, y),
                    egui::pos2(x, y + r),
                    egui::pos2(x - r, y),
                ],
                if selected { egui::Color32::WHITE } else { col },
                egui::Stroke::new(1.0, col),
            ));
            view.key_marks.push((ci, ki, egui::pos2(x, y)));
        }
        // SEGMENTS of the active curve — the target of the live scenario (R.63 item 5):
        // the right click must land BETWEEN keys, and only the drawing knows where they are.
        // `format!` under a flag: in the draw paths we do not
        // allocate per frame (invariant 12).
        if ci == view.active && crate::widgets::field_probe::on() {
            for ki in 0..c.keys.len().saturating_sub(1) {
                crate::widgets::field_probe::note(
                    &format!("auto.seg.{ki}"),
                    egui::Rect::from_min_max(
                        egui::pos2(x_of(c.keys[ki].t.0), plot.top()),
                        egui::pos2(x_of(c.keys[ki + 1].t.0), plot.bottom()),
                    ),
                );
            }
        }
    }
    // the probe — by what is DRAWN, and one for all channels
    let hit = resp.interact_pointer_pos().and_then(|pos| view.key_at(pos));
    // The hint at a key (R.49, item 364): previously it stayed silent in BOTH channels —
    // neither hover nor the right button reported anything, and what this point
    // was could be learned only by selecting it and reading the line
    // at the top. The format is the same as for the selected key: seconds of the material and
    // the share of the event on one line.
    if let Some(hp) = resp.hover_pos()
        && let Some((ci, ki)) = view.key_at(hp)
        && let Some(key) = curves.get(ci).and_then(|c| c.keys.get(ki))
    {
        crate::app::hint_into(
            hover,
            resp.clone(),
            &crate::tf!(
                k::fx::KEY_AT,
                format!("{:.3}", secs(key.t)),
                format!("{:.0}", ctx.frac(key.t) * 100.0)
            ),
        );
    }
    // The key menu — BEFORE the early `return`s below (the ruler, double click): after
    // them a right click on a key would not reach the binding at all.
    {
        // THE TARGET IS FIXED ON PRESS (R.63 item 5), and is not recomputed from the
        // pointer position every frame: otherwise the movement toward the menu items
        // lost the hit, and the menu closed by itself.
        if resp.secondary_clicked()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            view.menu_target = view
                .key_at(pos)
                .map(|(ci, ki)| MenuTarget::Key(ci, ki))
                .or_else(|| {
                    let ci = view.active;
                    let t = t_of(pos.x);
                    let keys = &curves.get(ci)?.keys;
                    keys.windows(2)
                        .position(|w| t >= w[0].t.0 && t < w[1].t.0)
                        .map(|ki| MenuTarget::Segment(ci, ki))
                });
            // the segment is remembered AFTER the menu too (R.67): its block of the curve shows
            // the segment formula field
            if let Some(MenuTarget::Segment(ci, ki)) = view.menu_target {
                view.sel_seg = Some((ci, ki));
                view.seg_formula = None;
            }
        }
        let menu_hit = match view.menu_target {
            Some(MenuTarget::Key(ci, ki)) => Some((ci, ki)),
            _ => None,
        };
        let mut del: Option<(usize, usize)> = None;
        let mut picked: Option<(usize, usize)> = None;
        // Right click on a SEGMENT (not on a key) — the shape of this segment
        // (R.61). Neighbouring segments are untouched: the shape belongs to the
        // LEFT key of the segment, and exactly it changes.
        let seg_hit: Option<(usize, usize)> = match view.menu_target {
            Some(MenuTarget::Segment(ci, ki)) => Some((ci, ki)),
            _ => None,
        };
        let mut set_shape: Option<(usize, usize, kt_model::SegShape)> = None;
        let mut shape_all: Option<(usize, kt_model::SegShape)> = None;
        let menu = resp.clone().context_menu(|ui| {
            let Some((ci, ki)) = menu_hit else {
                if let Some((ci, ki)) = seg_hit {
                    let rate = curves.get(ci).is_some_and(|c| c.param == "rate");
                    let cur = curves[ci].keys[ki].shape;
                    // THE PERSON SEES BETWEEN WHAT the shape is being set (R.63 item 5):
                    // the header names both ends of the segment, and the segment itself is
                    // highlighted on the graph
                    let (a, b) = (curves[ci].keys[ki].t, curves[ci].keys[ki + 1].t);
                    ui.label(
                        egui::RichText::new(crate::tf!(
                            k::fx::SHAPE_SEGMENT_AT,
                            format!("{:.3}", secs(a)),
                            format!("{:.3}", secs(b)),
                            format!("{:.0}", ctx.frac(a) * 100.0),
                            format!("{:.0}", ctx.frac(b) * 100.0)
                        ))
                        .strong(),
                    );
                    if rate {
                        // impossibility — an ERROR with a reason, not silence
                        ui.label(egui::RichText::new(tr(k::fx::SHAPE_RATE_LINEAR)).weak().small());
                    } else {
                        for (i, sh) in crate::widgets::all_shapes().into_iter().enumerate() {
                            let on = std::mem::discriminant(&sh) == std::mem::discriminant(&cur);
                            let r = ui.selectable_label(on, crate::widgets::shape_label(sh));
                            // menu items — the target of the live scenario: it must
                            // REACH them with the pointer (R.63 item 5)
                            if crate::widgets::field_probe::on() {
                                crate::widgets::field_probe::note(
                                    &format!("auto.shape.{i}"),
                                    r.rect,
                                );
                            }
                            if r.clicked() {
                                set_shape = Some((ci, ki, sh));
                                ui.close();
                            }
                        }
                        // SEGMENT FORMULA (R.67): the shape of the segment with its own
                        // text; the text is edited in the curve block below the graph
                        let is_formula = matches!(cur, kt_model::SegShape::Formula { .. });
                        let r = ui.selectable_label(is_formula, tr(k::fx::SHAPE_FORMULA_ITEM));
                        if crate::widgets::field_probe::on() {
                            crate::widgets::field_probe::note("auto.shape.formula", r.rect);
                        }
                        if r.clicked() {
                            set_shape = Some((
                                ci,
                                ki,
                                kt_model::SegShape::Formula {
                                    expr: kt_model::ExprId::intern("a + (b - a) * s"),
                                },
                            ));
                            ui.close();
                        }
                        ui.separator();
                        ui.menu_button(tr(k::fx::SHAPE_ALL), |ui| {
                            for sh in crate::widgets::all_shapes() {
                                if ui.button(crate::widgets::shape_label(sh)).clicked() {
                                    shape_all = Some((ci, sh));
                                    ui.close();
                                }
                            }
                        });
                    }
                    return;
                }
                ui.close();
                return;
            };
            picked = Some((ci, ki));
            if let Some(key) = curves.get(ci).and_then(|c| c.keys.get(ki)) {
                ui.label(
                    egui::RichText::new(crate::tf!(
                        k::fx::KEY_AT,
                        format!("{:.3}", secs(key.t)),
                        format!("{:.0}", ctx.frac(key.t) * 100.0)
                    ))
                    .strong(),
                );
            }
            // “Delete” WITHOUT a chord: Del deletes the selected timeline events,
            // not the key (item 362)
            if crate::widgets::menu_item(ui, tr(k::tl::menu::DELETE), None) {
                del = Some((ci, ki));
                ui.close();
            }
        });
        // the menu closed — the target is forgotten (otherwise the next right click on an
        // empty spot would show the shape of the previous segment)
        if menu.is_none() {
            view.menu_target = None;
        }
        out.key_menu = picked;
        if let Some((ci, ki, sh)) = set_shape
            && curves.get(ci).is_some_and(|c| ki < c.keys.len())
        {
            curves[ci].keys[ki].shape = sh;
            out.changed = true;
        }
        if let Some((ci, sh)) = shape_all
            && let Some(c) = curves.get_mut(ci)
        {
            for k in &mut c.keys {
                k.shape = sh;
            }
            out.changed = true;
        }
        if let Some((ci, ki)) = del
            && curves.get(ci).is_some_and(|c| ki < c.keys.len())
        {
            curves[ci].keys.remove(ki);
            view.sel = None;
            out.changed = true;
        }
    }
    // the caption of the active curve (a click on an empty spot places a key into it)
    if let Some(c) = curves.get(view.active)
        && let Some(spec) = ctx.spec(&c.param)
    {
        let (lo, hi) = win_of(c, spec, view, view.active);
        let ax = axis_of(spec);
        let col = curve_color(view.active);
        p.text(
            egui::pos2(rect.right() - 6.0, plot.top() + 2.0),
            egui::Align2::RIGHT_TOP,
            ctx.label(&c.param),
            egui::FontId::proportional(11.0),
            col,
        );
        // the bounds of the value scale: without them it is unclear “how much” the curve
        // rises — and each curve has its own scale
        for (g, y) in [(hi, plot.top() + 2.0), (lo, plot.bottom() - 12.0)] {
            let v = ax.unmap(g);
            p.text(
                egui::pos2(rect.left() + 4.0, y),
                egui::Align2::LEFT_TOP,
                format!("{v:.2}"),
                egui::FontId::proportional(10.0),
                egui::Color32::from_gray(120),
            );
        }
    }

    let Some(pos) = resp.interact_pointer_pos() else { return };
    let on_ruler = pos.y < rect.top() + RULER_H;
    // mini-timeline: a gesture on the ruler moves the MAIN cursor — “it is immediately
    // visible what it belongs to” (reverse and speed are accounted for by the inverse
    // mapping material → timeline)
    if on_ruler && (resp.clicked() || resp.dragged()) {
        out.seek = Some((ctx.to_timeline)(Tick(t_of(pos.x))));
        crate::app::hint_into(hover, resp.clone(), tr(k::fx::GRAPH_RULER_HINT));
        return;
    }
    if resp.double_clicked() {
        if let Some((ci, ki)) = hit {
            curves[ci].keys.remove(ki);
            view.sel = None;
            out.changed = true;
        }
        return;
    }
    // the shape-parameter handle: hover and grab — before the keys on empty space
    view.hover_seg = resp.hover_pos().and_then(|hp| {
        if view.key_at(hp).is_some() {
            None
        } else {
            view.seg_at(hp).map(|ki| (view.active, ki))
        }
    });
    // grab — by the PRESS POINT: egui declares a drag when the pointer has already
    // moved past the threshold, and the current position may be farther than the handle's grab
    let press = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
    if resp.drag_started()
        && hit.is_none()
        && let Some(ki) = view.seg_at(press)
        && let Some(c) = curves.get(view.active)
        && ki < c.keys.len()
        && let Some(spec) = ctx.spec(&c.param)
    {
        let (lo, hi) = win_of(c, spec, view, view.active);
        view.shape_drag = Some(ShapeDrag {
            ci: view.active,
            ki,
            from: press,
            shape0: c.keys[ki].shape,
            span: (hi - lo).max(1e-9),
            plot_h: (plot.height() - 12.0).max(1.0),
        });
    }
    if resp.dragged()
        && let Some(d) = view.shape_drag
        && d.ci < curves.len()
        && d.ki < curves[d.ci].keys.len()
    {
        let dy = (d.from.y - pos.y) as f64; // up — more
        let dx = (pos.x - d.from.x) as f64;
        let shift = ui.input(|i| i.modifiers.shift);
        let next = shape_dragged(d.shape0, dx, dy, shift, d.span / d.plot_h as f64);
        if curves[d.ci].keys[d.ki].shape != next {
            curves[d.ci].keys[d.ki].shape = next;
            out.changed = true;
        }
        // the number next to the handle — for the duration of the gesture (an overlay, not a popup)
        if let Some((_, m)) = view.seg_marks.iter().find(|(ki, _)| *ki == d.ki) {
            let (label, v) = shape_main_param(next).unwrap_or(("", 0.0));
            let text = if shift {
                format!("{}: {:.2}", tr(k::fx::SHAPE_PERIODS), shape_periods(next).unwrap_or(0.0))
            } else {
                format!("{label}: {v:.2}")
            };
            p.text(
                *m + egui::vec2(10.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                text,
                egui::FontId::proportional(11.0),
                egui::Color32::WHITE,
            );
        }
        return;
    }
    if resp.drag_stopped() && view.shape_drag.take().is_some() {
        return;
    }
    if resp.drag_started() || resp.clicked() {
        if let Some((ci, ki)) = hit {
            view.sel = Some((ci, ki));
            view.active = ci;
            view.drag = Some((ci, ki));
            if let Some(spec) = ctx.spec(&curves[ci].param) {
                let (lo, hi) = value_window(&curves[ci], spec);
                view.drag_win = Some((ci, lo, hi));
            }
        } else if resp.clicked() && view.seg_at(pos).is_none() {
            // a click on an empty spot — a new key on the active curve
            if let Some(spec) = curves.get(view.active).and_then(|c| ctx.spec(&c.param)).cloned() {
                let (lo, hi) = win_of(&curves[view.active], &spec, view, view.active);
                let c = &mut curves[view.active];
                let t = Tick(t_of(pos.x).clamp(m0, m1));
                let u = ((plot.bottom() - 6.0 - pos.y) / (plot.height() - 12.0)).clamp(0.0, 1.0);
                let v = axis_of(&spec).unmap(lo + (hi - lo) * u as f64).clamp(spec.min, spec.max);
                kt_model::insert_key(&mut c.keys, t, v, ctx.default_shape);
                view.sel = c.keys.iter().position(|k| k.t == t).map(|ki| (view.active, ki));
                out.changed = true;
            }
        }
    }
    if resp.dragged()
        && let Some((ci, ki)) = view.drag
        && ci < curves.len()
        && ki < curves[ci].keys.len()
    {
        let Some(spec) = ctx.spec(&curves[ci].param).cloned() else { return };
        // snap to the frame grid: the key sits on a frame, not “near”
        let raw = t_of(pos.x).clamp(m0, m1);
        let step = ctx.frame_step.max(1);
        let t = Tick(raw - raw.rem_euclid(step));
        let (lo, hi) = win_of(&curves[ci], &spec, view, ci);
        let u = ((plot.bottom() - 6.0 - pos.y) / (plot.height() - 12.0)).clamp(0.0, 1.0);
        let v = axis_of(&spec).unmap(lo + (hi - lo) * u as f64).clamp(spec.min, spec.max);
        // the shape of the segment belongs to the key and must
        // remain during dragging: otherwise a drag would silently turn a zigzag into smoothing
        let shape = curves[ci].keys[ki].shape;
        curves[ci].keys[ki] = kt_model::AutoKey::shaped(t, v, shape);
        // the order is restored at once, but the selection follows ITS OWN key
        curves[ci].keys.sort_by_key(|k| k.t.0);
        view.sel = curves[ci].keys.iter().position(|k| k.t == t).map(|ki| (ci, ki));
        view.drag = view.sel;
        out.changed = true;
    }
    if resp.drag_stopped() {
        view.drag = None;
        view.drag_win = None;
    }
}

fn table(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    ctx: &AutoCtx<'_>,
    curves: &mut [ParamCurve],
    view: &mut AutoView,
    out: &mut AutoResult,
) {
    // rows are MOMENTS, columns are parameters: “several parameters per row
    // instead of one”
    let mut times: Vec<i64> = curves.iter().flat_map(|c| c.keys.iter().map(|k| k.t.0)).collect();
    times.sort_unstable();
    times.dedup();
    let mut drop_key: Option<(usize, usize)> = None;
    let mut commit = false;
    egui::Grid::new(id).num_columns(curves.len() + 2).spacing([10.0, 4.0]).striped(true).show(
        ui,
        |ui| {
            ui.strong(tr(k::fx::COL_TIME));
            ui.strong(tr(k::fx::COL_FRAC));
            for c in curves.iter() {
                ui.strong(ctx.label(&c.param));
            }
            ui.end_row();
            for t in &times {
                let tick = Tick(*t);
                // row time: edited by ONE field; sorting — only on
                // release, otherwise the row slides out from under the fingers
                let edit = view
                    .t_edit
                    .filter(|(ci, ki, _)| {
                        curves.get(*ci).and_then(|c| c.keys.get(*ki)).is_some_and(|k| k.t == tick)
                    })
                    .map(|(_, _, s)| s);
                let mut s = edit.unwrap_or_else(|| secs(tick));
                // a key's time is a value WITHOUT a spec (it is a position, not a
                // parameter): its bounds and unit are declared here in one place,
                // the shared row draws it (R.66)
                let resp = crate::widgets::plain_row(
                    ui,
                    &crate::widgets::NumRow {
                        label: "",
                        probe: "auto.key.t",
                        unit: kt_model::ParamUnit::Sec,
                        min: 0.0,
                        max: f64::MAX,
                        step: 0.01,
                        slider: None,
                    },
                    &mut s,
                );
                if resp.changed()
                    && let Some((ci, ki)) = curves.iter().enumerate().find_map(|(ci, c)| {
                        c.keys.iter().position(|k| k.t == tick).map(|ki| (ci, ki))
                    })
                {
                    view.t_edit = Some((ci, ki, s));
                }
                if resp.drag_stopped() || resp.lost_focus() {
                    commit = true;
                }
                ui.weak(format!("{:.0}%", ctx.frac(tick) * 100.0));
                for (ci, c) in curves.iter_mut().enumerate() {
                    let Some(spec) = ctx.spec(&c.param).cloned() else {
                        ui.label("");
                        continue;
                    };
                    match c.keys.iter().position(|k| k.t == tick) {
                        Some(ki) => {
                            ui.horizontal(|ui| {
                                let mut v = c.keys[ki].v;
                                if crate::widgets::param_value(ui, &spec, &mut v).changed() {
                                    c.keys[ki].v = v;
                                    out.changed = true;
                                }
                                if ui.small_button(crate::icons::CLEAR).clicked() {
                                    drop_key = Some((ci, ki));
                                }
                            });
                        }
                        None => {
                            // an empty cell — a “place a key here” button
                            if ui.small_button(tr(k::fx::CELL_ADD)).clicked() {
                                let v = kt_model::curve_value(&c.keys, tick);
                                kt_model::insert_key(
                                    &mut c.keys,
                                    tick,
                                    v,
                                    kt_model::SegShape::Linear,
                                );
                                out.changed = true;
                            }
                        }
                    }
                }
                ui.end_row();
            }
        },
    );
    if let Some((ci, ki)) = drop_key {
        curves[ci].keys.remove(ki);
        view.sel = None;
        out.changed = true;
    }
    if commit && let Some((ci, ki, s)) = view.t_edit.take() {
        if let Some(k) = curves.get_mut(ci).and_then(|c| c.keys.get_mut(ki)) {
            k.t = Tick((s * 90_000.0).round() as i64);
        }
        if let Some(c) = curves.get_mut(ci) {
            let moved = c.keys[ki.min(c.keys.len().saturating_sub(1))].t;
            c.keys.sort_by_key(|k| k.t.0);
            view.sel = c.keys.iter().position(|k| k.t == moved).map(|ki| (ci, ki));
        }
        out.changed = true;
    }
}

// ---- R.67: shape-parameter handle, block “curve as parameter” -----------------

/// The main shape parameter of the segment for the handle: caption and value. A straight line,
/// a step and a segment formula have no handle (rule 9: nothing superfluous).
fn shape_main_param(s: kt_model::SegShape) -> Option<(&'static str, f64)> {
    use kt_model::SegShape as S;
    match s {
        S::Smooth { tension } | S::EaseIn { tension } | S::EaseOut { tension } => {
            Some((tr(k::fx::SHAPE_TENSION), tension))
        }
        S::Zigzag { amplitude, .. } => Some((tr(k::fx::SHAPE_AMP), amplitude)),
        S::Linear | S::Hold | S::Formula { .. } => None,
    }
}

fn shape_periods(s: kt_model::SegShape) -> Option<f64> {
    match s {
        kt_model::SegShape::Zigzag { periods, .. } => Some(periods),
        _ => None,
    }
}

/// The shape after a handle drag: the vertical drives the main parameter (tension —
/// 0.02 per pixel, amplitude — in axis units), Shift+horizontal —
/// periods (0.05 per pixel).
fn shape_dragged(
    s0: kt_model::SegShape,
    dx: f64,
    dy: f64,
    shift: bool,
    units_per_px: f64,
) -> kt_model::SegShape {
    use kt_model::SegShape as S;
    match s0 {
        S::Smooth { tension } => S::Smooth { tension: (tension + dy * 0.02).clamp(0.0, 4.0) },
        S::EaseIn { tension } => S::EaseIn { tension: (tension + dy * 0.02).clamp(0.0, 4.0) },
        S::EaseOut { tension } => S::EaseOut { tension: (tension + dy * 0.02).clamp(0.0, 4.0) },
        S::Zigzag { periods, amplitude, profile } => {
            if shift {
                S::Zigzag { periods: (periods + dx * 0.05).clamp(0.25, 256.0), amplitude, profile }
            } else {
                S::Zigzag { periods, amplitude: (amplitude + dy * units_per_px).max(0.0), profile }
            }
        }
        other => other,
    }
}

/// Names known to the formula: the carrier's variables and the registry functions.
fn known_names(vars: &[&str]) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = vars
        .iter()
        .map(|s| -> &'static str {
            match *s {
                "t" => "t",
                "k" => "k",
                "src" => "src",
                "s" => "s",
                "a" => "a",
                "b" => "b",
                _ => "",
            }
        })
        .filter(|s| !s.is_empty())
        .collect();
    v.extend(kt_model::formula::FUNCTIONS.iter().map(|(n, _)| *n));
    v
}

/// The nearest names to an unknown one (rule 16: the rejection names WHAT exists):
/// by prefix, then by edit distance ≤ 2.
fn nearest_names(bad: &str, vars: &[&str]) -> Vec<&'static str> {
    let names = known_names(vars);
    let mut out: Vec<&'static str> =
        names.iter().copied().filter(|n| !bad.is_empty() && n.starts_with(&bad[..bad.len().min(2)])).collect();
    if out.is_empty() {
        out = names.iter().copied().filter(|n| edit_distance(n, bad) <= 2).collect();
    }
    out.truncate(4);
    out
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Completion by Tab (R.67, without popups): the last name in the text
/// is completed to the single candidate or to the common prefix;
/// the candidates are returned for the status line.
fn complete_prefix(text: &str, vars: &[&str]) -> (String, Vec<&'static str>) {
    let start = text.rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map(|i| i + 1).unwrap_or(0);
    let prefix = &text[start..];
    if prefix.is_empty() {
        return (text.to_string(), Vec::new());
    }
    let cands: Vec<&'static str> =
        known_names(vars).into_iter().filter(|n| n.starts_with(prefix) && *n != prefix).collect();
    let done = match cands.len() {
        0 => text.to_string(),
        1 => {
            let n = cands[0];
            let is_fn = kt_model::formula::FUNCTIONS.iter().any(|(f, _)| *f == n);
            format!("{}{}{}", &text[..start], n, if is_fn { "(" } else { "" })
        }
        _ => {
            let mut common = cands[0].to_string();
            for c in &cands[1..] {
                let l = common.chars().zip(c.chars()).take_while(|(x, y)| x == y).count();
                common.truncate(l);
            }
            format!("{}{}", &text[..start], common)
        }
    };
    (done, cands)
}

/// The formula field with LIVE text (the widget state), confirmation by
/// Enter/focus loss, an error with a position and the nearest names, and
/// completion by Tab. `Some(text)` — the confirmed text to apply.
#[allow(clippy::too_many_arguments)]
fn formula_field(
    ui: &mut egui::Ui,
    probe: &str,
    live: &mut Option<String>,
    from_model: String,
    vars: &[&str],
    hint: &str,
    hover: &mut Option<String>,
    err: &mut Option<String>,
) -> Option<String> {
    let mut text = live.clone().unwrap_or(from_model);
    let r = ui.add(
        egui::TextEdit::singleline(&mut text)
            .desired_width(220.0)
            .lock_focus(true)
            .hint_text("t * 2"),
    );
    crate::widgets::field_probe::note(probe, r.rect);
    crate::app::hint_into(hover, r.clone(), hint);
    // Tab (the focus is locked — navigation does not take it): we complete in place;
    // a tab character, if the field inserted one, is removed
    let tab = r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Tab));
    let mut changed = r.changed();
    if tab || text.contains('\t') {
        let bare: String = text.chars().filter(|c| *c != '\t').collect();
        let (done, cands) = complete_prefix(&bare, vars);
        changed |= done != text;
        text = done;
        if cands.len() > 1 {
            *hover = Some(crate::tf!(k::fx::FORMULA_COMPLETE_MANY, cands.join(", ")));
        }
    }
    if changed {
        *live = Some(text.clone());
        if !text.trim().is_empty()
            && let Err(e) = kt_model::Formula::parse(&text)
        {
            *hover = Some(explain_error(&e, vars));
        }
    }
    let entered = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if let Some(t) = (entered || r.lost_focus()).then(|| live.take()).flatten() {
        {
            if t.trim().is_empty() {
                return Some(String::new());
            }
            match kt_model::Formula::parse(&t) {
                Ok(f) => {
                    if let Some(bad) = f.reads.iter().find(|n| !vars.contains(&n.as_str())) {
                        let msg = crate::tf!(
                            k::fx::FORMULA_UNKNOWN,
                            bad,
                            nearest_names(bad, vars).join(", ")
                        );
                        *hover = Some(msg.clone());
                        *err = Some(msg);
                        *live = Some(t);
                        return None;
                    }
                    return Some(t);
                }
                Err(e) => {
                    let msg = explain_error(&e, vars);
                    *hover = Some(msg.clone());
                    *err = Some(msg);
                    *live = Some(t);
                }
            }
        }
    }
    None
}

fn explain_error(e: &kt_model::FormulaError, vars: &[&str]) -> String {
    match &e.kind {
        kt_model::FormulaErrorKind::UnknownName(n) => {
            crate::tf!(k::fx::FORMULA_UNKNOWN, n, nearest_names(n, vars).join(", "))
        }
        _ => e.to_string(),
    }
}

/// The “CURVE AS PARAMETER” BLOCK (R.67): one per slot, generator and timeline instance;
/// it belongs to the curve the person sees as selected, and says so in its
/// header. Inside: the shape of all segments with parameters, the formula of the
/// selected segment, the layer formula, the link door, “unlink”, the help and
/// deletion of the curve. Before this, “the shape of all segments” stood in the slot window
/// without the curve's name and silently governed the active one — “only one field changes,
/// though I set zigzags in two fields” (the owner).
#[allow(clippy::too_many_arguments)]
fn curve_block(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    ctx: &AutoCtx<'_>,
    curves: &mut Vec<ParamCurve>,
    view: &mut AutoView,
    out: &mut AutoResult,
    hover: &mut Option<String>,
) {
    let Some(ci) = (view.active < curves.len()).then_some(view.active) else { return };
    let name = ctx.label(&curves[ci].param);
    let name = name.trim_end_matches(':').trim_end().to_string();
    ui.separator();
    let title = crate::tf!(k::fx::CURVE_TITLE, name);
    let r = ui.label(egui::RichText::new(title).color(curve_color(ci)).strong());
    crate::widgets::field_probe::note("curve.title", r.rect);

    // 1. the shape of all segments and its parameters
    ui.horizontal(|ui| {
        out.changed |= crate::widgets::shape_all_control(ui, &mut curves[ci], hover);
    });

    // 2. the formula of the SELECTED segment — only if it has a formula shape
    if let Some((sc, sk)) = view.sel_seg
        && sc == ci
        && let Some(kt_model::SegShape::Formula { expr }) = curves[ci].keys.get(sk).map(|k| k.shape)
    {
        ui.horizontal(|ui| {
            ui.label(tr(k::fx::SEG_FORMULA));
            let mut live = view.seg_formula.take().filter(|(c, k, _)| (*c, *k) == (sc, sk)).map(|(_, _, t)| t);
            let got = formula_field(
                ui,
                "auto.seg_formula",
                &mut live,
                expr.text(),
                kt_model::SEGMENT_VARS,
                tr(k::fx::SEG_FORMULA_HINT),
                hover,
                &mut out.error,
            );
            if let Some(t) = live {
                view.seg_formula = Some((sc, sk, t));
            }
            if let Some(t) = got
                && !t.trim().is_empty()
            {
                curves[ci].keys[sk].shape = kt_model::SegShape::Formula { expr: kt_model::ExprId::intern(&t) };
                out.changed = true;
            }
        });
    }

    // 3. layer formula (over the keys) / link recalculation text
    ui.horizontal(|ui| {
        ui.label(tr(k::fx::FORMULA));
        let from_model = match &curves[ci].source {
            kt_model::CurveSource::Formula { expr } => expr.clone(),
            kt_model::CurveSource::Link { expr, .. } => expr.clone().unwrap_or_default(),
            kt_model::CurveSource::Keys => String::new(),
        };
        let mut live = view.formula.take().filter(|(c, _)| *c == ci).map(|(_, t)| t);
        let got = formula_field(
            ui,
            "auto.formula",
            &mut live,
            from_model,
            kt_model::LAYER_VARS,
            tr(k::fx::FORMULA_HINT),
            hover,
            &mut out.error,
        );
        if let Some(t) = live {
            view.formula = Some((ci, t));
        }
        if let Some(t) = got {
            let c = &mut curves[ci];
            if t.trim().is_empty() {
                // an empty text — removes the layer; the keys under it remain
                match &c.source {
                    kt_model::CurveSource::Link { path, stops, .. } => {
                        c.source = kt_model::CurveSource::Link { path: path.clone(), expr: None, stops: stops.clone() };
                        out.changed = true;
                    }
                    kt_model::CurveSource::Formula { .. } => {
                        c.source = kt_model::CurveSource::Keys;
                        out.changed = true;
                    }
                    kt_model::CurveSource::Keys => {}
                }
            } else {
                c.source = match &c.source {
                    kt_model::CurveSource::Link { path, stops, .. } => kt_model::CurveSource::Link {
                        path: path.clone(),
                        expr: Some(t.clone()),
                        stops: stops.clone(),
                    },
                    // the layer does NOT erase the keys (R.67)
                    _ => kt_model::CurveSource::Formula { expr: t.clone() },
                };
                out.changed = true;
            }
        }
    });

    // 4. the link door and “unlink”
    ui.horizontal(|ui| {
        if let Some(links) = ctx.links {
            let mb = ui.menu_button(tr(k::fx::DRIVE_BY), |ui| {
                let events = links();
                let mut pick: Option<String> = None;
                let mut said_others = false;
                for ev in &events {
                    // foreign events — under their own subheading: the person sees
                    // where “this event” ends and the reel begins
                    if !ev.this && !said_others {
                        said_others = true;
                        ui.separator();
                        ui.label(egui::RichText::new(tr(k::fx::DRIVE_OTHERS)).weak().small());
                    }
                    let title = if ev.this { tr(k::fx::DRIVE_THIS).to_string() } else { ev.label.clone() };
                    ui.menu_button(title, |ui| {
                        for (path, label) in &ev.params {
                            let b = ui.button(label);
                            if crate::widgets::field_probe::on() {
                                crate::widgets::field_probe::note(&format!("auto.link.{path}"), b.rect);
                            }
                            if b.clicked() {
                                pick = Some(path.clone());
                                ui.close();
                            }
                        }
                    });
                }
                if let Some(path) = pick {
                    let c = &mut curves[ci];
                    let expr = c.source.expr().map(|e| e.to_string());
                    c.source = kt_model::CurveSource::Link { path, expr, stops: Vec::new() };
                    out.changed = true;
                    ui.close();
                }
            });
            crate::widgets::field_probe::note("auto.drive_by", mb.response.rect);
            crate::app::hint_into(hover, mb.response, tr(k::fx::DRIVE_BY_HINT));
        }
        if !curves[ci].source.is_keys() {
            let b = ui.button(tr(k::fx::UNLINK));
            crate::app::hint_into(hover, b.clone(), tr(k::fx::UNLINK_HINT));
            crate::widgets::field_probe::note("auto.unlink", b.rect);
            if b.clicked() {
                // the values are already resolved by the model at assembly — here
                // only the source itself is removed
                curves[ci].source = kt_model::CurveSource::Keys;
                out.changed = true;
            }
        }
        // delete the curve — here too, at the named curve
        let del = ui.button(tr(k::fx::AUTO_REMOVE));
        crate::widgets::field_probe::note("auto.remove", del.rect);
        if del.clicked() {
            curves.remove(ci);
            view.active = 0;
            view.sel = None;
            view.sel_seg = None;
            out.changed = true;
        }
    });
    if ci >= curves.len() {
        return;
    }

    // 5. help — a group in the block from the evaluator's registry; no popups
    let hdr = egui::CollapsingHeader::new(tr(k::fx::HELP))
        .id_salt(("formula_help", id))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(tr(k::fx::HELP_VARS)).strong());
            for key in [
                k::fx::HELP_VAR_T,
                k::fx::HELP_VAR_K,
                k::fx::HELP_VAR_SRC,
                k::fx::HELP_VAR_S,
                k::fx::HELP_VAR_A,
                k::fx::HELP_VAR_B,
            ] {
                ui.label(tr(key));
            }
            ui.label(egui::RichText::new(tr(k::fx::HELP_FUNCS)).strong());
            ui.horizontal_wrapped(|ui| {
                for (name, arity) in kt_model::formula::FUNCTIONS {
                    let args = (0..*arity).map(|i| ["x", "y", "z"][i]).collect::<Vec<_>>().join(", ");
                    let sig = format!("{name}({args})");
                    let b = ui.small_button(&sig);
                    if crate::widgets::field_probe::on() {
                        crate::widgets::field_probe::note(&format!("auto.help.{name}"), b.rect);
                    }
                    // a click inserts a sample into the formula field (the AE form —
                    // an insert menu, but without a popup)
                    if b.clicked() {
                        let cur = view.formula.take().filter(|(c, _)| *c == ci).map(|(_, t)| t);
                        let base = cur.unwrap_or_else(|| curves[ci].source.expr().unwrap_or("").to_string());
                        view.formula = Some((ci, format!("{base}{sig}")));
                    }
                }
            });
            ui.label(tr(k::fx::HELP_OPS));
        });
    crate::widgets::field_probe::note("auto.help", hdr.header_response.rect);
    crate::app::hint_into(hover, hdr.header_response, tr(k::fx::HELP_HINT));
}
