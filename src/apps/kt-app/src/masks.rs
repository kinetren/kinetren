//! The mask list (shape-masks) — ONE widget for the three places a person looks for masks
//! (rule 24): the Geometry node's form (the event's picture), the effects window under a slot's
//! parameters (limit an effect), the track window (the track's composite). No new window
//! (rule 22, kkt-26.11.01 line 55 "do not throw features anywhere").
//!
//! A row: the edit button (selects the mask on the preview), the name, the mode, Invert, the
//! path's stopwatch, ↑ ↓ ×; under it Opacity, Feather (+ falloff), Expansion — passport fields
//! through the shared `param_row_bound_ex` (limits from the passport, keys only by gesture,
//! R.67). The selected mask's curves are drawn by the one automation widget.

use crate::i18n::{k, tr};
use crate::maskctl::{MaskOwner, MaskTool};
use eframe::egui;
use kt_model::{Falloff, Mask, MaskId, MaskMode, ParamSpec};
use kt_time::Tick;

/// The label of a mask row (the passport rows have no catalogue entries of the effects').
pub fn row_label(name: &str) -> String {
    match name {
        "opacity" => tr(k::masks::OPACITY).to_string(),
        "feather" => tr(k::masks::FEATHER).to_string(),
        "expansion" => tr(k::masks::EXPANSION).to_string(),
        "cx" => tr(k::masks::LABEL_CX).to_string(),
        "cy" => tr(k::masks::LABEL_CY).to_string(),
        "w" => tr(k::masks::LABEL_W).to_string(),
        "h" => tr(k::masks::LABEL_H).to_string(),
        "rot" => tr(k::masks::LABEL_ROT).to_string(),
        "round" => tr(k::masks::LABEL_ROUND).to_string(),
        n => match kt_model::parse_vertex_row(n) {
            Some((id, r)) => crate::tf!(k::masks::LABEL_POINT, id, r),
            None => n.to_string(),
        },
    }
}

fn mode_name(m: MaskMode) -> &'static str {
    match m {
        MaskMode::Add => tr(k::masks::MODE_ADD),
        MaskMode::Subtract => tr(k::masks::MODE_SUBTRACT),
        MaskMode::Intersect => tr(k::masks::MODE_INTERSECT),
        MaskMode::Difference => tr(k::masks::MODE_DIFFERENCE),
    }
}

/// The "Masks" group of a form: collapsed while there are no masks (one header line — the
/// automation below stays where it was), open as soon as there are. The header is the live
/// scenarios' target (`grp.masks`, as `widgets::group`'s).
pub fn section<R>(ui: &mut egui::Ui, has_masks: bool, body: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    let r = egui::CollapsingHeader::new(tr(k::masks::GROUP))
        .id_salt(("form_group", "masks", has_masks))
        .default_open(has_masks)
        .show(ui, body);
    crate::widgets::field_probe::note("grp.masks", r.header_response.rect);
    r.body_returned
}

/// What the list is drawn for.
pub struct ListCtx<'a> {
    pub owner: MaskOwner,
    /// The cursor in the owner's curve domain (the event's material tick).
    pub at: Tick,
    pub default_shape: kt_model::SegShape,
    /// The mask selected on the preview (its row is highlighted, its curves are shown).
    pub editing: Option<MaskId>,
    /// The curve widget's context (`None` — a track: its masks are static).
    pub auto: Option<&'a crate::automation::AutoCtx<'a>>,
    pub playhead: Tick,
}

/// What happened in the list.
#[derive(Default)]
pub struct ListOut {
    pub changed: bool,
    pub tool: Option<MaskTool>,
    /// Select this mask on the preview.
    pub edit: Option<MaskId>,
    pub refused: Option<String>,
    pub seek: Option<Tick>,
}

/// The mask list of one owner. `list` is edited in place; `changed` says it must be written.
pub fn mask_list(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + Copy + std::fmt::Debug,
    list: &mut Vec<Mask>,
    cx: &ListCtx<'_>,
    view: &mut crate::automation::AutoView,
    hover: &mut Option<String>,
) -> ListOut {
    let mut out = ListOut::default();
    let tracked = matches!(cx.owner, MaskOwner::Track(_));
    // the tools: they arm the preview for this owner
    ui.horizontal(|ui| {
        for (tool, label, hint) in [
            (MaskTool::Rect, tr(k::masks::RECT), tr(k::masks::RECT_HINT)),
            (MaskTool::Ellipse, tr(k::masks::ELLIPSE), tr(k::masks::ELLIPSE_HINT)),
            (MaskTool::Pen, tr(k::masks::PEN), tr(k::masks::PEN_HINT)),
        ] {
            let r = ui.button(label);
            // the live scenario draws an effect's mask with the ellipse
            if tool == MaskTool::Ellipse {
                crate::widgets::field_probe::note("masks.tool.ellipse", r.rect);
            }
            if crate::app::hint_into(hover, r, hint).clicked() {
                out.tool = Some(tool);
            }
        }
    });
    if tracked {
        ui.label(egui::RichText::new(tr(k::masks::TRACK_STATIC)).weak().small());
    }
    if list.is_empty() {
        ui.label(egui::RichText::new(tr(k::masks::NONE)).weak());
        return out;
    }
    let mut remove: Option<usize> = None;
    let mut swap: Option<(usize, usize)> = None;
    let n = list.len();
    for (i, m) in list.iter_mut().enumerate() {
        let on = cx.editing == Some(m.id);
        ui.push_id((salt, "mask", m.id), |ui| {
            ui.horizontal(|ui| {
                let short: String = m.name.chars().take(18).collect();
                let r = ui.selectable_label(on, format!("{} {short}", crate::icons::SELECTION));
                if crate::app::hint_into(hover, r, tr(k::masks::EDIT_HINT)).clicked() {
                    out.edit = Some(m.id);
                }
                // the mode — a row of four words, not a drop-down: one click, all visible (as
                // the conform rule of the Geometry node)
                let mut mode = m.mode;
                for md in MaskMode::ALL {
                    let r = ui.selectable_value(&mut mode, md, mode_name(md));
                    crate::app::hint_into(hover, r, tr(k::masks::MODE_HINT));
                }
                if mode != m.mode {
                    m.mode = mode;
                    out.changed = true;
                }
                let r = ui.checkbox(&mut m.invert, tr(k::masks::INVERT));
                crate::widgets::field_probe::note(&format!("masks.row.{i}.invert"), r.rect);
                if crate::app::hint_into(hover, r, tr(k::masks::INVERT_HINT)).changed() {
                    out.changed = true;
                }
                // the path's stopwatch: keys every shape row at the cursor (AE's "Mask Path")
                if !tracked {
                    let animated = m.path_animated();
                    let r = ui.selectable_label(animated, crate::icons::TIMER);
                    crate::widgets::field_probe::note(&format!("masks.row.{i}.key"), r.rect);
                    if crate::app::hint_into(hover, r, tr(k::masks::PATH_KEY_HINT)).clicked() {
                        if animated {
                            m.auto.retain(|c| !kt_model::is_shape_row(&c.param));
                        } else {
                            let now = m.at(cx.at);
                            m.key_path(cx.at, &now, cx.default_shape);
                        }
                        out.changed = true;
                    }
                }
                let r = ui.add_enabled(i > 0, egui::Button::new(crate::icons::LANE_UP));
                if crate::app::hint_into(hover, r, tr(k::masks::UP_HINT)).clicked() {
                    swap = Some((i, i - 1));
                }
                let r = ui.add_enabled(i + 1 < n, egui::Button::new(crate::icons::LANE_DOWN));
                if crate::app::hint_into(hover, r, tr(k::masks::DOWN_HINT)).clicked() {
                    swap = Some((i, i + 1));
                }
                let r = ui.button(crate::icons::CLEAR);
                if crate::app::hint_into(hover, r, tr(k::masks::DELETE_HINT)).clicked() {
                    remove = Some(i);
                }
            });
            // the numbers: passport fields; a row with a live curve edits its key at the cursor
            ui.horizontal(|ui| {
                let specs = m.specs();
                let mut auto = std::mem::take(&mut m.auto);
                for name in ["opacity", "feather", "expansion"] {
                    let Some(spec) = specs.iter().find(|s| s.name == name) else { continue };
                    let mut bind = (!tracked).then_some(crate::widgets::CurveBind { curves: &mut auto, sel: None, at: cx.at });
                    let ch = crate::widgets::param_row_bound_ex(ui, &mut m.params(), spec, bind.as_mut(), None, None);
                    if ch.any() {
                        out.changed = true;
                    }
                }
                m.auto = auto;
                let mut fall = m.falloff;
                egui::ComboBox::from_id_salt((salt, "falloff", m.id))
                    .selected_text(if fall == Falloff::Smooth { tr(k::masks::FALLOFF_SMOOTH) } else { tr(k::masks::FALLOFF_LINEAR) })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut fall, Falloff::Smooth, tr(k::masks::FALLOFF_SMOOTH));
                        ui.selectable_value(&mut fall, Falloff::Linear, tr(k::masks::FALLOFF_LINEAR));
                    });
                if fall != m.falloff {
                    m.falloff = fall;
                    out.changed = true;
                }
            });
        });
    }
    if let Some((a, b)) = swap {
        list.swap(a, b);
        out.changed = true;
    }
    if let Some(i) = remove {
        list.remove(i);
        out.changed = true;
    }
    // the curves of the selected mask (or the only one) — the one widget (rule: no own editor)
    if let Some(actx) = cx.auto
        && let Some(m) = list.iter_mut().find(|m| Some(m.id) == cx.editing)
    {
        let specs: Vec<ParamSpec> = m
            .specs()
            .into_iter()
            .map(|mut s| {
                s.label = std::borrow::Cow::Owned(row_label(&s.name));
                s
            })
            .collect();
        let ctx = crate::automation::AutoCtx {
            specs: &specs,
            range: actx.range,
            view: actx.view,
            linked: actx.linked,
            default_shape: actx.default_shape,
            to_timeline: actx.to_timeline,
            to_curve: actx.to_curve,
            frame_step: actx.frame_step,
            links: actx.links,
        };
        ui.label(egui::RichText::new(tr(k::masks::LABEL_CURVES)).strong());
        let r = crate::automation::draw(ui, (salt, "maskauto", m.id), &ctx, &mut m.auto, view, cx.playhead, hover);
        if r.changed {
            out.changed = true;
        }
        out.seek = r.seek;
        if let Some(e) = r.error {
            out.refused = Some(e);
        }
    }
    out
}
