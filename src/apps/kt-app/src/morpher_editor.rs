//! The morpher editor window (port of MorpherEditWindow v1, sourceeditwidgets):
//! it shows the frozen targets; for Replace — the choice of a new source from the
//! project’s media + a shift of source-in (delta or absolute — fix D4 v1); for an
//! FX batch — editing of two chains (video/audio) acting LIVE
//! (fix D1 v1). All edits — apply_coalesced(SetMorpher): the whole session of the
//! window = one undo step. Automation in batch chains is not edited —
//! after bake the full FX editor of the clip is available.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::*;
use kt_time::{TICK_HZ, Tick};

pub struct MorphEditor {
    pub id: MorphId,
    pub picker_query: String,
    /// Search in the source picker (Replace morpher).
    pub src_query: String,
    pub gesture: u64,
    pub open: bool,
    /// Selected in ONE strip of the chain (R.35 item 3): the head is
    /// a source substitution, and further effects of both kinds one after another.
    pub sel: crate::widgets::ChainSel,
    /// The rectangle of the source field on screen: by it `handle_drops`
    /// learns that the file was dropped EXACTLY HERE (R.33 item 2 — “it would be nice
    /// if one could also do dnd of a file there”).
    pub src_rect: Option<egui::Rect>,
    /// The state snapshot at the moment of opening — for “Cancel” (in the FX window
    /// there was a rollback, the morpher had none at all).
    pub backup: Option<kt_model::MorpherEvent>,
    /// The state of the automation widget for each slot (R.38 item 13).
    pub auto_views: std::collections::HashMap<usize, crate::automation::AutoView>,
}

const MS: i64 = TICK_HZ / 1000; // ticks in ms
/// A day in milliseconds — the limit of the source-input shift: the quantity has no
/// passport, but “infinity” is never a limit (rule 6 of the checklist).
const DAY_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;

/// The SOURCE INPUT row of the morpher (ms). The limits and the unit are declared in ONE
/// place: the absolute input and the shift differ only in the lower bound.
fn source_in_row(probe: &'static str, min: f64) -> crate::widgets::NumRow<'static> {
    crate::widgets::NumRow {
        label: "",
        probe,
        unit: ParamUnit::Ms,
        min,
        max: DAY_MS,
        step: 10.0,
        slider: None,
    }
}

impl KtApp {
    pub fn open_morph_editor(&mut self, id: MorphId) {
        if self.morph_editors.iter().any(|e| e.id == id) {
            return;
        }
        self.gesture_counter += 1;
        let backup = self.model.project().morphers.iter().find(|m| m.id == id).cloned();
        self.morph_editors.push(MorphEditor {
            id,
            picker_query: String::new(),
            src_query: String::new(),
            gesture: self.gesture_counter,
            open: true,
            sel: Default::default(),
            src_rect: None,
            backup,
            auto_views: std::collections::HashMap::new(),
        });
    }

    /// A new source of a Replace morpher from a FILE: the media is added to the
    /// project (if it was not there) and substituted into the morpher — in one
    /// undo step. A common point for the system dialog and for dropping a file into the
    /// source field (R.33 item 2).
    pub(crate) fn set_morph_source_file(&mut self, id: MorphId, path: &str) {
        let Some(mut ev) = self.model.project().morphers.iter().find(|m| m.id == id).cloned()
        else {
            return;
        };
        let existing = self.model.project().media.iter().find(|m| m.path == path).map(|m| m.id);
        let mut cmds = Vec::new();
        let mid = match existing {
            Some(mid) => mid,
            None => {
                let mid = MediaId(self.next_media_id());
                cmds.push(Command::AddMedia { id: mid, path: path.to_string() });
                mid
            }
        };
        ev.patch.media = Some(mid);
        cmds.push(Command::SetMorpher(ev));
        self.apply_batch(cmds);
    }

    /// The source field of the morph slot: the picker of project media with search, a drop of a
    /// file from the system, a card from the bin, and a shift of the entry point. Shown
    /// when the HEAD is selected in the strip (R.35 item 3).
    fn morph_source_section(
        &self,
        ui: &mut egui::Ui,
        ed: &mut MorphEditor,
        ev: &mut MorpherEvent,
        pick_file: &mut bool,
    ) -> bool {
        let mut changed = false;
        let media_list: Vec<(MediaId, String)> =
            self.model.project().media.iter().map(|m| (m.id, m.path.clone())).collect();
        let cur_name = ev
            .patch
            .media
            .and_then(|id| media_list.iter().find(|(i, _)| *i == id).map(|(_, p)| p.clone()))
            .unwrap_or_else(|| tr(k::mrph::KEEP_SOURCE).to_string());
        // The source field (R.33 item 2): accepts a drop of a file from the system and
        // cards from the media bin, opens the list of project media with search and the
        // system file dialog.
        let frame =
            egui::Frame::group(ui.style()).fill(ui.visuals().extreme_bg_color).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::mrph::NEW_SOURCE));
                    // the same truncation as in the strip: the file name here is exactly the
                    // same, and without a limit it stretched the window
                    let nm = short_name(&cur_name);
                    let txt = crate::widgets::elided_name(
                        ui,
                        &nm,
                        ui.visuals().strong_text_color(),
                        false,
                    );
                    let r = ui.label(txt);
                    if nm.chars().count() > crate::widgets::SLOT_NAME_CHARS {
                        r.on_hover_text(nm.clone());
                    }
                    if let Some(id) = crate::widgets::media_picker(
                        ui,
                        ("morph_src", ed.id.0),
                        &media_list,
                        &mut ed.src_query,
                    ) {
                        ev.patch.media = id;
                        changed = true;
                    }
                    if ui.button(tr(k::mrph::BROWSE)).clicked() {
                        *pick_file = true;
                    }
                });
                ui.weak(tr(k::mrph::SOURCE_DROP));
            });
        let rect = frame.response.rect;
        ed.src_rect = Some(rect);
        // a card from the bin: the same payload that a drag onto the timeline carries
        if ui.rect_contains_pointer(rect)
            && let Some(id) = egui::DragAndDrop::payload::<crate::bin_panel::BinDrag>(ui.ctx())
        {
            ui.painter().rect_stroke(
                rect,
                3.0,
                egui::Stroke::new(2.0, crate::theme::ACCENT),
                egui::StrokeKind::Inside,
            );
            if ui.input(|i| i.pointer.any_released()) {
                ev.patch.media = Some(id.0);
                changed = true;
                let _ = egui::DragAndDrop::take_payload::<crate::bin_panel::BinDrag>(ui.ctx());
            }
        }
        ui.separator();
        // --- shift of source-in (delta / absolute, D4) ---------
        let mut absolute = ev.patch.absolute_source_in.is_some();
        if ui.checkbox(&mut absolute, tr(k::mrph::ABSOLUTE_IN)).changed() {
            ev.patch.absolute_source_in = absolute.then_some(Tick(0));
            changed = true;
        }
        if let Some(abs) = &mut ev.patch.absolute_source_in {
            let mut ms = (abs.0 / MS) as f64;
            ui.horizontal(|ui| {
                ui.label(tr(k::mrph::SOURCE_IN_MS));
                if crate::widgets::plain_row(ui, &source_in_row("mrph.source_in", 0.0), &mut ms)
                    .changed()
                {
                    *abs = Tick(ms as i64 * MS);
                    changed = true;
                }
            });
        } else {
            let mut ms = (ev.patch.source_in_delta.0 / MS) as f64;
            ui.horizontal(|ui| {
                ui.label(tr(k::mrph::SOURCE_IN_DELTA_MS));
                if crate::widgets::plain_row(
                    ui,
                    &source_in_row("mrph.source_in_delta", -DAY_MS),
                    &mut ms,
                )
                .changed()
                {
                    ev.patch.source_in_delta = Tick(ms as i64 * MS);
                    changed = true;
                }
            });
        }
        changed
    }

    pub fn draw_morph_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.morph_editors);
        for ed in &mut editors {
            self.draw_one_morph_editor(ctx, ed);
        }
        editors.retain(|e| e.open);
        self.morph_editors = editors;
    }

    #[allow(clippy::too_many_lines)]
    fn draw_one_morph_editor(&mut self, ctx: &egui::Context, ed: &mut MorphEditor) {
        // the morpher is legitimately given both sides — we show the whole library
        let preset_rows: Vec<crate::user_store::PresetRow> = self.presets.rows().to_vec();
        let mut preset_save: Option<kt_model::ops::FxChainBlock> = None;
        let mut preset_action: Option<crate::widgets::PresetAction> = None;
        // the event disappeared (undo/deletion) — the window closes itself (v1)
        let Some(orig) = self.model.project().morphers.iter().find(|m| m.id == ed.id).cloned()
        else {
            ed.open = false;
            return;
        };
        // from R.35 the morpher carries both the substitution and the chains — the window shows everything
        // at once; the label is chosen by the CONTENT, not by the “kind”
        let is_batch = !orig.patch.video_fx.is_empty() || !orig.patch.audio_fx.is_empty();
        // the camera for the Pan3d combo in the FX batch (R.29)
        let cameras: Vec<(u64, String)> = self
            .model
            .project()
            .cameras
            .iter()
            .map(|c| {
                (c.id.0, if c.name.is_empty() { format!("C{}", c.id.0) } else { c.name.clone() })
            })
            .collect();
        // media names for the morph-slot label
        let media_names: Vec<(MediaId, String)> =
            self.model.project().media.iter().map(|m| (m.id, m.path.clone())).collect();
        let title = if is_batch {
            crate::tf!(k::mrph::TITLE_BATCH, ed.id.0)
        } else {
            crate::tf!(k::mrph::TITLE, ed.id.0)
        };
        let mut ev = orig.clone();
        let mut changed = false;
        let mut bake = false;
        let mut delete = false;
        // “Cancel” rolls back the window session (in the FX window it was always so; the
        // morpher had no rollback at all — R.33)
        let mut revert = false;
        let mut pick_file = false;
        let mut open = ed.open;
        // what fills the frozen list: the region takes priority over the
        // selection, both are expanded by link groups (v1 rule —
        // the single point of target collection for the whole program)
        let selection: Vec<ClipId> = self.morph_target_ids();
        // names of ONE namespace: areas and marker ranges. `live` —
        // those that really exist: a dead reference stays in the file and
        // is drawn in the error style (rule 16), not thrown away
        let areas_mode = matches!(orig.targets, MorphTargets::Areas { .. });
        let (all_names, live_names): (Vec<String>, Vec<(String, Tick)>) = {
            let p = self.model.project();
            if areas_mode {
                (
                    p.areas.iter().map(|a| a.name.clone()).collect(),
                    p.areas.iter().map(|a| (a.name.clone(), a.start)).collect(),
                )
            } else {
                let ranges: Vec<(String, Tick)> = p
                    .markers
                    .iter()
                    .filter(|m| !m.name.is_empty() && m.duration.0 > 0)
                    .map(|m| (m.name.clone(), m.t))
                    .collect();
                (ranges.iter().map(|(n, _)| n.clone()).collect(), ranges)
            }
        };
        let mut seek_to: Option<Tick> = None;
        // the request about the axis: the lock is one per program (R.40 item 15)
        let mut axis_req = crate::automation::AxisReq::default();
        let mut need_selection = false;
        // everything the layer’s automation needs (R.38 item 13)
        let playhead = self.player.transport.playhead();
        let frame_step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        let tl_view = self.tl_view();
        // the event’s toggle (R.33) — the same master checkbox as the event’s chain;
        // the FX buffer is shared with the event windows
        let mut enabled = ev.enabled;
        let mut fx_clip = self.fx_clipboard.clone();
        let dflt_shape = self.model.document().settings.default_shape;
        let mut paste_req = false;
        // hints — into the status bar, as everywhere (there are no popups in the program)
        let mut hover_out: Option<String> = None;
        // the target of a right click in the chain strip (item 363): the strip is a widget,
        // it knows nothing about KtApp, and it hands the target outward
        let mut panel_pick: Option<crate::ctx_menu::PanelTarget> = None;
        // parts of the context of the picker’s thumbnails (R.54): the service and the texture cache
        // are borrowed BEFORE the window — inside the closure `self` is taken by the model
        let pv_svc = &self.fx_thumbs;
        let pv_tick = playhead.0 - playhead.0.rem_euclid(frame_step);
        let pv_cam = kt_engine::resolve_cam3d(
            &[kt_model::FxInstance::Pan3d {
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
                rot_x: 0.0,
                rot_y: 0.0,
                rot_z: 0.0,
                anchor_x: 0.5,
                anchor_y: 0.5,
                scale: 1.0,
                camera: None,
            }],
            &self.model.project().cameras,
            kt_time::Tick(pv_tick),
            crate::fx_thumbs::REF_W,
            self.model.project().settings.width,
        );
        let shown = egui::Window::new(title)
            .id(egui::Id::new(("morph", ed.id.0)))
            .open(&mut open)
            .default_size(if is_batch { [560.0, 480.0] } else { [420.0, 260.0] })
            .min_size([380.0, 200.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                // --- on whom and when it acts (R.35) ------------------------
                // GROUPS after the “Transform” model (R.66): targets, source, patch
                let frozen = ev.targets.clip_ids().to_vec();
                crate::widgets::group(ui, "mrph.targets", tr(k::grp::TARGETS), |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::mrph::ON));
                    let cur = match &ev.targets {
                        MorphTargets::Clips { ids } => crate::tf!(k::mrph::ON_CLIPS, ids.len()),
                        MorphTargets::Areas { names } => {
                            crate::tf!(k::mrph::ON_AREAS_N, names.len())
                        }
                        MorphTargets::Marks { names } => {
                            crate::tf!(k::mrph::ON_MARKS_N, names.len())
                        }
                        MorphTargets::Below => tr(k::mrph::ON_BELOW).to_string(),
                    };
                    egui::ComboBox::from_id_salt(("morph_on", ed.id.0)).selected_text(cur).show_ui(
                        ui,
                        |ui| {
                            if ui.selectable_label(false, tr(k::mrph::ON_BELOW)).clicked()
                                && !matches!(ev.targets, MorphTargets::Below)
                            {
                                ev.targets = MorphTargets::Below;
                                changed = true;
                            }
                            // named references (R.35 item 2): an area and
                            // a marker range live in ONE namespace,
                            // therefore there are exactly two modes, not
                            // “choose the reference kind first”
                            if ui.selectable_label(false, tr(k::mrph::ON_AREAS)).clicked()
                                && !matches!(ev.targets, MorphTargets::Areas { .. })
                            {
                                ev.targets = MorphTargets::Areas { names: Vec::new() };
                                changed = true;
                            }
                            if ui.selectable_label(false, tr(k::mrph::ON_MARKS)).clicked()
                                && !matches!(ev.targets, MorphTargets::Marks { .. })
                            {
                                ev.targets = MorphTargets::Marks { names: Vec::new() };
                                changed = true;
                            }
                            // we freeze the CURRENT selection; an empty
                            // list for Clips is forbidden by the model, therefore
                            // a refusal is shown as an error, not as silence
                            let sel: Vec<ClipId> =
                                if frozen.is_empty() { selection.clone() } else { frozen.clone() };
                            let label = crate::tf!(k::mrph::ON_CLIPS, sel.len());
                            if ui.selectable_label(false, label).clicked() {
                                if sel.is_empty() {
                                    need_selection = true;
                                } else if ev.targets.clip_ids() != sel.as_slice() {
                                    ev.targets = MorphTargets::Clips { ids: sel };
                                    changed = true;
                                }
                            }
                        },
                    );
                    let mut timed = ev.timed;
                    // in reference modes the time is set by the reference itself —
                    // the checkbox decides nothing here and honestly fades out
                    let by_ref = matches!(
                        ev.targets,
                        MorphTargets::Areas { .. } | MorphTargets::Marks { .. }
                    );
                    if crate::app::hint_into(
                        &mut hover_out,
                        ui.add_enabled(
                            !by_ref,
                            egui::Checkbox::new(&mut timed, tr(k::mrph::TIMED)),
                        ),
                        if by_ref { tr(k::mrph::TIMED_BY_REF) } else { tr(k::mrph::TIMED_HINT) },
                    )
                    .changed()
                    {
                        ev.timed = timed;
                        changed = true;
                    }
                });
                if let MorphTargets::Areas { names } | MorphTargets::Marks { names } =
                    &mut ev.targets
                {
                    let areas = matches!(orig.targets, MorphTargets::Areas { .. });
                    changed |= refs_row(
                        ui,
                        ed.id.0,
                        names,
                        &all_names,
                        &live_names,
                        areas,
                        &mut ed.src_query,
                        &mut seek_to,
                        &mut hover_out,
                    );
                } else if matches!(ev.targets, MorphTargets::Below) {
                    ui.label(egui::RichText::new(tr(k::mrph::BELOW_NOTE)).weak().small());
                } else {
                    let mut ids: Vec<String> =
                        frozen.iter().take(4).map(|t| format!("#{}", t.0)).collect();
                    if frozen.len() > 4 {
                        ids.push("…".into());
                    }
                    ui.label(
                        egui::RichText::new(crate::tf!(
                            k::mrph::FROZEN,
                            frozen.len(),
                            ids.join(", ")
                        ))
                        .weak(),
                    );
                }
                });
                ui.separator();

                // R.35 item 3: ONE strip of the chain, in which the first
                // is the purple morph slot (source substitution). The effect kinds in the patch
                // lie separately (video/audio), but the user is not shown this: the strip glues them
                // together and
                // splits them back by `FxInstance::class()`.
                let (mut fx, mut bypass, mut auto, split) = merge_chains(&ev.patch);
                let mut src_on = !ev.patch.src_off;
                let mut pv = crate::fx_thumbs::PickerCtx::new(
                    pv_svc,
                    crate::fx_thumbs::FxThumbBase::Pattern,
                    None,
                    Vec::new(),
                    0.5,
                    pv_tick,
                    pv_cam,
                );
                let ch = crate::widgets::chain_strip(
                    ui,
                    ("morphchain", ed.id.0),
                    crate::widgets::ChainEdit {
                        fx: &mut fx,
                        bypass: &mut bypass,
                        auto: &mut auto,
                        // the morpher shows ALL presets: chains of both kinds are legitimate in the patch
                        // (split_chains will sort them)
                        presets: crate::widgets::ChainPresets {
                            rows: &preset_rows,
                            save: &mut preset_save,
                            action: &mut preset_action,
                        },
                        selected: &mut ed.sel,
                        master_on: &mut enabled,
                        head: Some(crate::widgets::HeadSlot {
                            label: head_label(&ev.patch, &media_names),
                            on: Some(&mut src_on),
                            hint: tr(k::mrph::SRC_SLOT_HINT),
                            color: crate::theme::MORPH_SLOT,
                        }),
                    },
                    false,
                    &mut ed.picker_query,
                    &mut fx_clip,
                    kt_time::Tick(0),
                    &mut hover_out,
                    // a morpher chain has no owning clip: we address it by the morpher’s
                    // number — both live in their own namespace
                    &mut crate::widgets::ChainCtx {
                        chain: kt_model::ClipId(ed.id.0),
                        picked: &mut panel_pick,
                    },
                    // Morpher: WYSIWYG is impossible by construction — the layer
                    // paints MANY targets (Below/Marks/Areas), and “the first
                    // target under the cursor” is unstable. We show the built-in
                    // sample: it is deterministic and honestly says WHAT
                    // the variant does, without pretending to be a frame of the edit.
                    Some(&mut pv),
                );
                if ch.any() {
                    ev.patch.src_off = !src_on;
                    split_chains(&mut ev.patch, &fx, &bypass, &auto, split);
                    changed = true;
                }
                // STRENGTH and FADES of the layer (R.38 item 12, verbatim: “the morpher has no
                // way to apply fade and opacity — the plan is not
                // kept”). One line for everything: the strength is the share of blending,
                // the fades are its rise and fall at the edges of the block.
                ui.horizontal(|ui| {
                    ui.label(tr(k::mrph::STRENGTH));
                    let mut s = ev.strength;
                    if crate::app::hint_into(
                        &mut hover_out,
                        crate::widgets::plain_row(
                            ui,
                            &crate::widgets::NumRow {
                                label: "",
                                probe: "mrph.strength",
                                unit: kt_model::ParamUnit::Norm,
                                min: 0.0,
                                max: 1.0,
                                step: 0.01,
                                slider: Some(90.0),
                            },
                            &mut s,
                        ),
                        tr(k::mrph::STRENGTH_HINT),
                    )
                    .changed()
                    {
                        ev.strength = s;
                        changed = true;
                    }
                    let max = ev.duration.0 as f64 / 90_000.0;
                    for (label, cur) in [
                        (tr(k::mrph::FADE_IN), &mut ev.fade_in),
                        (tr(k::mrph::FADE_OUT), &mut ev.fade_out),
                    ] {
                        ui.label(label);
                        let mut v = cur.0 as f64 / 90_000.0;
                        if crate::widgets::plain_row(
                            ui,
                            &crate::widgets::NumRow {
                                label: "",
                                probe: "mrph.fade",
                                unit: kt_model::ParamUnit::Sec,
                                min: 0.0,
                                max,
                                step: 0.05,
                                slider: None,
                            },
                            &mut v,
                        )
                        .changed()
                        {
                            *cur = kt_time::Tick((v * 90_000.0).round() as i64);
                            changed = true;
                        }
                    }
                });
                ui.label(egui::RichText::new(tr(k::mrph::LIVE_NOTE)).weak().small());
                ui.separator();

                match ed.sel {
                    crate::widgets::ChainSel::Head | crate::widgets::ChainSel::Lead => {
                        if let Some(ch) =
                            crate::widgets::group(ui, "mrph.source", tr(k::grp::SOURCE), |ui| {
                                self.morph_source_section(ui, ed, &mut ev, &mut pick_file)
                            })
                        {
                            changed |= ch;
                        }
                    }
                    crate::widgets::ChainSel::Fx(i) => {
                        // PATCH — its own group (R.66): the layer’s parameters and their curves
                        crate::widgets::group(ui, "mrph.patch", tr(k::grp::PATCH), |ui| {
                        // Parameters and AUTOMATION — by the same code as in the
                        // event window (R.38 item 13). The layer’s curve domain is the
                        // time of the BLOCK: the morpher has no material, it is a layer of
                        // edits. The same domain as the camera and the transition.
                        let at = kt_time::Tick((playhead.0 - ev.start.0).clamp(0, ev.duration.0));
                        let view_sel = ed.auto_views.get(&i).and_then(|v| v.sel);
                        let mut slot_auto = auto.get(i).cloned().unwrap_or_default();
                        let mut bind =
                            crate::widgets::CurveBind { curves: &mut slot_auto, sel: view_sel, at };
                        let mut slot_changed = false;
                        if let Some(f) = fx.get_mut(i) {
                            // for the morpher both edits travel in one command
                            // SetMorpher, therefore the flags collapse
                            slot_changed |= crate::fx_editor::draw_video_fx_params(
                                ui,
                                f,
                                Some(&mut bind),
                                &cameras,
                                false,
                                &mut hover_out,
                                // the morpher PATCH slot has no address
                                // (`ParamOwner` does not know it),
                                // and no dead menu item will appear here
                                None,
                            )
                            .any();
                            let start = ev.start;
                            let to_tl = move |t: kt_time::Tick| kt_time::Tick(start.0 + t.0);
                            let to_cur = move |t: kt_time::Tick| kt_time::Tick(t.0 - start.0);
                            let view = ed.auto_views.entry(i).or_default();
                            let r = crate::fx_editor::draw_auto_for(
                                ui,
                                f,
                                &mut slot_auto,
                                view,
                                crate::automation::AutoCtx {
                                    links: None,
                                    specs: f.param_specs(),
                                    range: (
                                        kt_time::Tick::ZERO,
                                        kt_time::Tick(ev.duration.0.max(1)),
                                    ),
                                    view: crate::automation::view_in_curve(&to_cur, tl_view),
                                    to_timeline: &to_tl,
                                    to_curve: &to_cur,
                                    frame_step,
                                    linked: self.auto_axis_linked,
                                    default_shape: dflt_shape,
                                },
                                playhead,
                                &mut paste_req,
                                (ed.id.0, i),
                                &mut hover_out,
                            );
                            slot_changed |= r.changed;
                            if let Some(t) = r.seek {
                                seek_to = Some(t);
                            }
                            axis_req = axis_req.merge(&r);
                        }
                        if slot_changed {
                            if let Some(a) = auto.get_mut(i) {
                                *a = slot_auto;
                            }
                            split_chains(&mut ev.patch, &fx, &bypass, &auto, split);
                            changed = true;
                        }
                        });
                    }
                }

                });
                crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button(if is_batch {
                            tr(k::mrph::BAKE_BATCH)
                        } else {
                            tr(k::mrph::BAKE_MORPH)
                        })
                        .clicked()
                    {
                        bake = true;
                    }
                    if ui.button(tr(k::mrph::REMOVE)).clicked() {
                        delete = true;
                    }
                    if crate::app::hint_into(
                        &mut hover_out,
                        ui.button(tr(k::fx::CANCEL)),
                        tr(k::mrph::REVERT_HINT),
                    )
                    .clicked()
                    {
                        revert = true;
                    }
                });
                });
            });
        // the window frame as a whole — landing-page screenshots are cropped by it (site/screenshots.py)
        if let Some(r) = &shown {
            crate::widgets::field_probe::note("win.morpher", r.response.rect);
        }
        ed.open = open;

        // system file dialog: the new source may not be in the project
        if pick_file && let Some(path) = rfd::FileDialog::new().pick_file() {
            // the window stays open: the user sees the new source
            self.set_morph_source_file(ed.id, &path.to_string_lossy());
            return;
        }
        if revert {
            if let Some(b) = ed.backup.clone() {
                self.apply_coalesced(Command::SetMorpher(b), ed.gesture);
            }
            ed.open = false;
            return;
        }
        if bake {
            if self.apply(Command::BakeMorpher { id: ed.id }) {
                self.selected_morph.clear();
            }
            ed.open = false;
            return;
        }
        if delete {
            if self.apply(Command::RemoveMorpher { id: ed.id }) {
                self.selected_morph.clear();
            }
            ed.open = false;
            return;
        }
        self.fx_clipboard = fx_clip;
        if need_selection {
            // an impossible action is an error, not a faded line (item 16 round 28)
            self.note(tr(k::mrph::NEED_SELECTION));
        }
        // the graph’s requests about the AXIS are applied in one place (R.40 item 15)
        self.apply_axis_req(axis_req);
        if let Some(t) = seek_to {
            // clicking a chip scrolls the timeline to this place
            self.player.seek(t);
        }
        if let Some(t) = panel_pick {
            self.set_panel_target(t); // goals cancel each other out (item 363)
        }
        if let Some(h) = hover_out {
            self.hover_hint = Some(h);
        }
        ev.enabled = enabled;
        changed |= ev.enabled != orig.enabled;
        if changed && ev != orig {
            self.apply_coalesced(Command::SetMorpher(ev), ed.gesture);
        }

        // requests of the strip for presets (R.48): the strip only puts them there —
        // the file is written by the window, because the name dialog lives in the app
        if let Some(block) = preset_save.take() {
            self.preset_save = Some(crate::app::PresetSaveDialog {
                block,
                name: String::new(),
                rename_of: None,
                focus: true,
            });
        }
        match preset_action.take() {
            Some(crate::widgets::PresetAction::Rename(old)) => {
                self.preset_save = Some(crate::app::PresetSaveDialog {
                    // rename does not use the block
                    block: kt_model::ops::FxChainBlock {
                        slots: vec![],
                        src_in: kt_time::Tick(0),
                        fx_off: false,
                    },
                    name: old.clone(),
                    rename_of: Some(old),
                    focus: true,
                });
            }
            Some(crate::widgets::PresetAction::Delete(name)) => match self.presets.remove(&name) {
                Ok(()) => self.note(format!("{} {name}", tr(k::msg::PRESET_DELETED))),
                Err(e) => self.note(format!("{}: {e}", tr(k::msg::PRESET_SAVE_FAILED))),
            },
            None => {}
        }
    }
}

fn short_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Caption of the purple morph slot: what the substitution DOES. “Do not change source”
/// is an honest state, not an empty place: the slot is always pinned.
fn head_label(patch: &MorphPatch, media: &[(MediaId, String)]) -> String {
    match patch.media {
        Some(id) => {
            let name = media
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, p)| short_name(p))
                .unwrap_or_else(|| format!("#{}", id.0));
            crate::tf!(k::mrph::SRC_SLOT, name)
        }
        None if patch.absolute_source_in.is_some() || patch.source_in_delta.0 != 0 => {
            tr(k::mrph::SRC_SLOT_SHIFT).to_string()
        }
        None => tr(k::mrph::SRC_SLOT_NONE).to_string(),
    }
}

/// Glue both patch chains into ONE strip. `split` remembers how many slots were video:
/// the reverse layout goes by `FxInstance::class()`, and the count is needed only to check
/// that the order was not lost.
type Merged = (Vec<FxInstance>, Vec<bool>, Vec<Vec<kt_model::ParamCurve>>, usize);

fn merge_chains(patch: &MorphPatch) -> Merged {
    let mut fx = patch.video_fx.clone();
    let mut bypass = patch.video_bypass.clone();
    bypass.resize(fx.len(), false);
    // curves travel together with the slots (R.38 item 13) — otherwise moving an effect by mouse
    // in the strip would carry it away from its automation
    let mut auto = patch.video_auto.clone();
    auto.resize(fx.len(), Vec::new());
    let n = fx.len();
    fx.extend(patch.audio_fx.iter().cloned());
    let mut ab = patch.audio_bypass.clone();
    ab.resize(patch.audio_fx.len(), false);
    bypass.extend(ab);
    let mut aa = patch.audio_auto.clone();
    aa.resize(patch.audio_fx.len(), Vec::new());
    auto.extend(aa);
    (fx, bypass, auto, n)
}

/// Split the strip back by kind. The slot kind is a function of the effect itself
/// (`is_audio`), so the gluing is reversible without side tables; the relative order
/// within each kind is preserved.
fn split_chains(
    patch: &mut MorphPatch,
    fx: &[FxInstance],
    bypass: &[bool],
    auto: &[Vec<kt_model::ParamCurve>],
    _was_video: usize,
) {
    patch.video_fx.clear();
    patch.video_bypass.clear();
    patch.video_auto.clear();
    patch.audio_fx.clear();
    patch.audio_bypass.clear();
    patch.audio_auto.clear();
    for (i, f) in fx.iter().enumerate() {
        let off = bypass.get(i).copied().unwrap_or(false);
        let cur = auto.get(i).cloned().unwrap_or_default();
        if f.is_audio() {
            patch.audio_fx.push(f.clone());
            patch.audio_bypass.push(off);
            patch.audio_auto.push(cur);
        } else {
            patch.video_fx.push(f.clone());
            patch.video_bypass.push(off);
            patch.video_auto.push(cur);
        }
    }
}

/// Morpher link row: chips with crosses and `[+ link]` by the same popup with search
/// as the media picker. A dead link (a name that is not in the project) is drawn in the
/// error style — it is not “empty” but “broken” (item 16 round 28). A click on a live
/// chip scrolls the timeline.
#[allow(clippy::too_many_arguments)]
fn refs_row(
    ui: &mut egui::Ui,
    id: u64,
    names: &mut Vec<String>,
    all: &[String],
    live: &[(String, Tick)],
    areas: bool,
    query: &mut String,
    seek: &mut Option<Tick>,
    hover: &mut Option<String>,
) -> bool {
    let mut changed = false;
    let mut remove: Option<usize> = None;
    ui.horizontal_wrapped(|ui| {
        for (i, n) in names.iter().enumerate() {
            let at = live.iter().find(|(ln, _)| ln == n).map(|(_, t)| *t);
            let text = egui::RichText::new(n.clone()).color(match at {
                Some(_) => crate::theme::AREA,
                None => egui::Color32::from_rgb(255, 105, 97),
            });
            let resp = crate::app::hint_into(
                hover,
                ui.selectable_label(false, text),
                match at {
                    Some(_) => tr(k::mrph::REF_GOTO),
                    None => tr(k::mrph::REF_DEAD),
                },
            );
            if resp.clicked()
                && let Some(t) = at
            {
                *seek = Some(t);
            }
            if ui.small_button(crate::icons::CLEAR).clicked() {
                remove = Some(i);
            }
        }
        // adding — the same popup with search as the media picker
        let free: Vec<(u64, String)> = all
            .iter()
            .enumerate()
            .filter(|(_, n)| !names.contains(n))
            .map(|(i, n)| (i as u64, n.clone()))
            .collect();
        if let Some(i) = crate::widgets::media_picker_raw(
            ui,
            ("morph_refs", id),
            &free,
            query,
            tr(if areas { k::mrph::ADD_AREA_REF } else { k::mrph::ADD_MARK_REF }),
        ) && let Some((_, n)) = free.iter().find(|(j, _)| *j == i)
        {
            names.push(n.clone());
            changed = true;
        }
        if all.is_empty() {
            ui.weak(tr(if areas { k::mrph::NO_AREAS } else { k::mrph::NO_MARKS }));
        }
    });
    if let Some(i) = remove {
        names.remove(i);
        changed = true;
    }
    changed
}
