//! The FX chain window of a clip (a port of ClipFxChainWindow + FxChainEditorWidget):
//! a strip of cards “Media → fx → fx”, a parameter form, an automation
//! strip (diamonds: click = select, drag = move t, double-click = delete),
//! live application via apply_coalesced (the whole session of the window = 1 undo),
//! Cancel = rollback to the state at the moment of opening.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::*;

/// Snapshot of one chain at the moment the window was opened: the chain itself, the curves,
/// the slot switches and the master switch.
pub type ChainSnapshot = (ClipId, Vec<FxInstance>, Vec<Vec<kt_model::ParamCurve>>, Vec<bool>, bool);

pub struct FxEditor {
    /// Anchor of the window: the event by which it was opened (its id is the window id).
    pub clip: ClipId,
    /// All chains the window shows: the anchor and its link partners
    /// (R.33 item 2 — “for one and the same event with magnetized
    /// tracks you can give all chains at once in one window”).
    pub members: Vec<ClipId>,
    pub picker_query: String,
    /// The MEDIA picker row in the head of “Composition parameters” (R.63 item 12):
    /// the media-type handle has its own search, and it must not be mixed with the effects search —
    /// these are different lists.
    pub inst_query: String,
    /// The state of EACH chain at the moment of opening — “Cancel” rolls back
    /// the whole window, not just the anchor.
    pub backup: Vec<ChainSnapshot>,
    /// Generator values at the moment of opening (R.59, stage 2): for
    /// an event-generator the head of the strip is its SOURCE, and “Cancel”
    /// must return it too, otherwise the window would roll back half of its own
    /// session.
    pub gen_backup: Vec<(ClipId, Option<GeneratorKind>, Vec<kt_model::ParamCurve>)>,
    /// The leading “Geometry” node at the moment of opening (R.62): “Cancel”
    /// returns it too — it is part of the same strip.
    pub geo_backup: Vec<(ClipId, kt_model::EventGeometry, usize, Vec<kt_model::ParamCurve>)>,
    /// Overrides of the reel instance at the moment of opening (R.63 item 12):
    /// “Composition parameters” moved as the HEAD into this window, so
    /// “Cancel” must return them too — otherwise the window would roll back half
    /// of its own session (the same argument as for the generator and the geometry).
    pub ov_backup: Vec<(ClipId, Vec<kt_model::Override>)>,
    pub gesture: u64,
    /// What is selected in each chain: an effect slot or the pinned head.
    pub selected: std::collections::HashMap<ClipId, crate::widgets::ChainSel>,
    /// The state of the automation widget for each (chain, slot):
    /// the view, the selected key, the uncommitted time input.
    pub auto_views: std::collections::HashMap<(ClipId, usize), crate::automation::AutoView>,
    /// The same for the curves of the GENERATOR: the head has no slot index of its own.
    pub gen_views: std::collections::HashMap<ClipId, crate::automation::AutoView>,
    /// Cache of title warnings per frame (item 413).
    pub gen_warn: std::collections::HashMap<ClipId, crate::gen_ui::GenWarn>,
    /// The moment at which the thumbnails of the picker are computed (R.54).
    /// While NOT playing it follows the cursor (it is motionless,
    /// so the picture is stable); during playback it stays
    /// at the last value: recomputing dozens of cells on every clock tick is pointless, and a flickering strip is
    /// worse than a slightly stale one.
    pub preview_t: Option<kt_time::Tick>,
    /// When the window was opened — the misclick guard for decode ORDERS (item 430).
    pub opened_at: std::time::Instant,
    pub open: bool,
}

impl KtApp {
    /// Open the FX window. A window that already shows this chain (through its anchor
    /// or as a partner) is not opened a second time: an A/V pair
    /// has one window for both tracks.
    /// Open the event’s effects window ON THE HEAD, that is, on the form of its
    /// source (R.59, stage 2).
    ///
    /// The former generator and title windows have disappeared: the owner asked to see
    /// the generator as a crumb BEFORE the effects, in the same window. All entry points
    /// (“Generator parameters…” of the event, of the bin card, creation from the
    /// menu) lead here — this action has no second body (rule 17).
    pub fn open_fx_editor_head(&mut self, clip: ClipId) {
        self.open_fx_editor(clip);
        if let Some(ed) = self.fx_editors.iter_mut().find(|e| e.members.contains(&clip)) {
            ed.open = true;
            ed.selected.insert(clip, crate::widgets::ChainSel::Head);
        }
    }

    /// Effects window of a MEDIA generator: the first event on this
    /// media edits it. Values live on the EVENT, so “media parameters” without an
    /// event no longer exist — and one must not stay silent about it (rule
    /// 16): the kind is changed from the bin card, and there is nothing to edit until
    /// the generator is placed on the timeline.
    pub fn open_generator_editor(&mut self, media: MediaId) {
        let Some(kt_model::Source::File(m)) = self.model.project().source(media) else {
            return;
        };
        let path = m.path.clone();
        // not a generator (or a kind newer than this build — a stub): we must NOT stay silent
        if kt_model::parse_gen_path(&path).is_none() {
            self.note(crate::tf!(k::genw::UNKNOWN_KIND, path));
            return;
        }
        // we search in the WORKING VIEW: the window edits what the person sees on
        // the timeline, and events of a foreign (unexpanded) reel are unavailable from here —
        // he reaches them through the reel itself
        let first = self
            .model
            .project()
            .clips
            .iter()
            .find(|c| c.media == media && c.r#gen.is_some())
            .map(|c| c.id);
        match first {
            Some(id) => self.open_fx_editor_head(id),
            None => self.note(tr(k::genw::NO_EVENTS)),
        }
    }

    pub fn open_fx_editor(&mut self, clip: ClipId) {
        if self.fx_editors.iter().any(|e| e.members.contains(&clip)) {
            return;
        }
        let Some(c) = self.model.clip(clip).cloned() else { return };
        let members: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|m| m.id == clip || (c.link_group.is_some() && m.link_group == c.link_group))
            // the anchor first, then by view and id — the order is stable
            .map(|m| (m.id != clip, m.kind == ClipKind::Audio, m.id.0, m.id))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|(_, _, _, id)| id)
            .collect();
        self.gesture_counter += 1;
        let backup = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| (m.id, m.fx.clone(), m.fx_auto.clone(), m.fx_bypass.clone(), m.fx_off))
            .collect();
        let gen_backup = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| (m.id, m.r#gen.clone(), m.gen_auto.clone()))
            .collect();
        let geo_backup = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| (m.id, m.geometry, m.geometry_pos, m.geometry_auto.clone()))
            .collect();
        let ov_backup = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| (m.id, m.overrides.clone()))
            .collect();
        // an event-generator with an empty chain has nothing to choose but
        // the head: `Fx(0)` on an empty chain would show emptiness. For a reel
        // INSTANCE the head is “Composition parameters” (R.63 item 12), and
        // the rule is the same.
        let doc = self.model.document();
        let selected = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| {
                let has_head = m.r#gen.is_some() || doc.sequence(m.media).is_some();
                let sel = if m.fx.is_empty() && has_head {
                    crate::widgets::ChainSel::Head
                } else {
                    crate::widgets::ChainSel::Fx(m.fx.len().saturating_sub(1))
                };
                (m.id, sel)
            })
            .collect();
        self.fx_editors.push(FxEditor {
            clip,
            members,
            picker_query: String::new(),
            inst_query: String::new(),
            backup,
            gen_backup,
            geo_backup,
            ov_backup,
            gesture: self.gesture_counter,
            selected,
            auto_views: std::collections::HashMap::new(),
            gen_views: std::collections::HashMap::new(),
            gen_warn: std::collections::HashMap::new(),
            preview_t: None,
            opened_at: std::time::Instant::now(),
            open: true,
        });
    }

    pub(crate) fn set_fx_coalesced(&mut self, clip: ClipId, fx: Vec<FxInstance>, gesture: u64) {
        self.apply_coalesced(Command::SetFx { id: clip, fx }, gesture);
    }

    /// Automation curves: the same gesture as SetFx — one undo step
    /// per window session.
    pub(crate) fn set_fx_auto_coalesced(
        &mut self,
        clip: ClipId,
        mut auto: Vec<Vec<kt_model::ParamCurve>>,
        gesture: u64,
    ) {
        while auto.last().is_some_and(|v| v.is_empty()) {
            auto.pop(); // the tail without curves is not serialized needlessly
        }
        self.apply_coalesced(Command::SetFxAuto { id: clip, auto }, gesture);
    }

    pub fn draw_fx_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.fx_editors);
        for ed in &mut editors {
            self.draw_one_fx_editor(ctx, ed);
        }
        // the window is closed — counting its cells is no longer needed (R.54): everything
        // still in the thumbnail queue is dropped by a generation change
        if editors.iter().any(|e| !e.open) {
            self.fx_thumbs.bump_generation();
        }
        editors.retain(|e| e.open);
        self.fx_editors = editors;
    }

    #[allow(clippy::too_many_lines)]
    fn draw_one_fx_editor(&mut self, ctx: &egui::Context, ed: &mut FxEditor) {
        // hints from the ui closures (there `self` is busy) go to the frame accumulator,
        // then into hover_hint: there are no popups (item 14 of round 8)
        let mut hover_out: Option<String> = None;
        // the right-click target in the chain strip (item 363): the strip is a widget,
        // it knows nothing about KtApp, and it hands the target outward
        let mut panel_pick: Option<crate::ctx_menu::PanelTarget> = None;
        // the window lives as long as at least one of its chains is alive
        ed.members.retain(|id| self.model.clip(*id).is_some());
        // Publication (R.59, stage 3): the document is cloned ONCE per
        // drawing of the window — `publish_ctx` is called for each carrier, and
        // cloning the project per row would be an allocation in the drawing path.
        let pub_doc = self.model.document();
        // edits to the composition handles are applied AFTER the drawing, by the window’s
        // token — inside the ui closure `self` is busy with a borrow
        let mut inst_ops: Vec<kt_model::Command> = Vec::new();
        // formula rejection — an error in the status line (R.67, rule 16)
        let mut auto_error: Option<String> = None;
        let Some(anchor) = self.model.clip(ed.clip).cloned() else {
            ed.open = false;
            return;
        };
        // presets: we show only those that are legal for THIS kind
        // of event (compatibility is computed from the effects registry)
        let is_audio_anchor = anchor.kind == ClipKind::Audio;
        let preset_rows: Vec<crate::user_store::PresetRow> = self
            .presets
            .rows()
            .iter()
            .filter(|r| if is_audio_anchor { r.audio_ok } else { r.video_ok })
            .cloned()
            .collect();
        // the state of the library is reported ONCE per window session: the file
        // is broken, the file is newer than us, some entries did not parse
        if !self.presets_reported {
            self.presets_reported = true;
            if self.presets.is_broken() {
                self.note(tr(k::msg::PRESETS_BROKEN));
            }
            if self.presets.is_readonly() {
                self.note(tr(k::msg::PRESETS_READONLY));
            }
            let skipped = self.presets.skipped();
            if skipped > 0 {
                self.note(format!("{} {skipped}", tr(k::msg::PRESETS_SKIPPED)));
            }
        }
        let mut preset_save: Option<kt_model::ops::FxChainBlock> = None;
        let mut preset_action: Option<crate::widgets::PresetAction> = None;
        // cameras for the Pan3d combo (R.29): id + caption
        let cameras: Vec<(u64, String)> = self
            .model
            .project()
            .cameras
            .iter()
            .map(|c| {
                (c.id.0, if c.name.is_empty() { format!("C{}", c.id.0) } else { c.name.clone() })
            })
            .collect();
        let title = if ed.members.len() > 1 {
            crate::tf!(k::fx::TITLE_GROUP, ed.clip.0)
        } else {
            crate::tf!(
                k::fx::TITLE,
                if anchor.kind == ClipKind::Audio {
                    tr(k::fx::KIND_AUDIO)
                } else {
                    tr(k::fx::KIND_VIDEO)
                },
                ed.clip.0
            )
        };
        let playhead = self.player.transport.playhead();
        // the buffers live in the application; we hand them to the window for the duration of the frame,
        // so as not to hold &mut self inside the ui closure
        // a press of “paste envelope”: the target address is put here
        let mut paste_req = false;
        let mut paste_target: Option<(ClipId, usize)> = None;
        let mut fx_clip = self.fx_clipboard.clone();
        // what the window edited during the frame: applied AFTER the drawing
        struct Edited {
            clip: ClipId,
            fx: Option<Vec<FxInstance>>,
            auto: Option<Vec<Vec<kt_model::ParamCurve>>>,
            bypass: Option<Vec<bool>>,
            off: Option<bool>,
            /// Generator values of the event (R.59, stage 2) — `SetClipGen`.
            r#gen: Option<GeneratorKind>,
            /// Its curves — `SetClipGenAuto`.
            gen_auto: Option<Vec<kt_model::ParamCurve>>,
            /// A title preset sets EVERYTHING at once — and this is its OWN history step.
            own_step: bool,
            /// “Apply to all”: `ApplyGenToAll`, one history step.
            apply_all: bool,
            /// Change of the generator KIND: a property of the media, its own history step.
            switch_kind: Option<GeneratorKind>,
            /// The leading “Geometry” node, its position and curves (R.62) —
            /// `SetGeometry`, the same history step as the chain.
            geometry: Option<(kt_model::EventGeometry, usize, Vec<kt_model::ParamCurve>)>,
        }
        let mut edits: Vec<Edited> = Vec::new();
        let mut revert = false;
        let mut lead_refused = false;
        // the mini-timeline of the automation widget moves the MAIN cursor
        let mut seek: Option<kt_time::Tick> = None;
        let frame_step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        // the window of the main timeline: the automation graph shows the same
        // thing until the lock is released (R.38 item 14)
        let tl_view = self.tl_view();
        // the axis lock is one for the whole program (R.40 item 15)
        let axis_linked = self.auto_axis_linked;
        let dflt_shape = self.model.document().settings.default_shape;
        let mut axis_req = crate::automation::AxisReq::default();
        // “places: N” of the generator and the path of the scrolling text are computed BEFORE
        // drawing: inside the ui closure `&mut self` is already busy, and the places counter
        // walks the tree linearly, so there is no point computing it per frame
        let uses_of = self.media_uses();
        let frame_size = {
            let st = &self.model.project().settings;
            (st.width, st.height)
        };
        let mut open = ed.open;
        let members = ed.members.clone();
        // ---- context of picker thumbnails (R.54, item 432) -------------------
        // WYSIWYG: the cell shows the EVENT’S CHAIN plus the candidate in
        // the tail, that is, exactly what will result after the click. The frame is
        // the nearest ready thumbnail to the cursor position: the conversion formula
        // “event tick → source frame” is ONE with the filmstrip
        // (`thumbs::material_key_t`), so the keys are shared and the strip does not produce
        // a new decode (invariant No. 8).
        let pv_t = {
            let want = anchor
                .start
                .0
                .max(playhead.0 - playhead.0.rem_euclid(frame_step))
                .min((anchor.start.0 + anchor.duration.0 - 1).max(anchor.start.0));
            if self.player.transport.is_playing() {
                *ed.preview_t.get_or_insert(kt_time::Tick(want))
            } else {
                let t = kt_time::Tick(want);
                ed.preview_t = Some(t);
                t
            }
        };
        let pv_span = anchor.duration.0.max(1);
        let pv_u = (pv_t.0 - anchor.start.0) as f64 / pv_span as f64;
        // the camera is resolved BY THE ANCHOR, but as if Pan3d were in the chain:
        // the “Pan 3D” variant in the strip must show perspective also on
        // a clip where 3D is not yet set. The resolution rule is one with the engine
        // (kt_engine::resolve_cam3d, item 429).
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
            pv_t,
            crate::fx_thumbs::REF_W,
            self.model.project().settings.width,
        );
        // the ANCHOR’s chain with resolved curves: values at the moment of the
        // cursor, the domain is the event’s material (R.37: speed does not squeeze the axis)
        let pv_chain: Vec<FxInstance> = {
            let mat = kt_model::clip_material_at(
                anchor.source_in,
                anchor.start,
                anchor.duration,
                anchor.reversed,
                frame_step,
                pv_t,
            );
            anchor
                .fx
                .iter()
                .enumerate()
                .filter(|(i, f)| {
                    !anchor.fx_bypass.get(*i).copied().unwrap_or(false) && !f.is_audio()
                })
                .map(|(i, f)| {
                    let mut fx = f.clone();
                    if let Some(curves) = anchor.fx_auto.get(i) {
                        kt_model::apply_fx_auto(&mut fx, curves, mat);
                    }
                    fx
                })
                .collect()
        };
        // base frame: exact thumbnail → media poster → built-in sample.
        // Empty strips do not occur, and decode ORDERS are placed only after
        // the misclick guard (item 430).
        let decode_ok = crate::fx_thumbs::decode_orders_allowed(ed.opened_at.elapsed());
        let is_nest = self.model.project().sequence(anchor.media).is_some();
        let pv_step = crate::thumbs::THUMB_STEP_LADDER[1];
        let (pv_base, pv_img) = if is_nest {
            // a reel has no file of its own, and it has no thumbnails (item 395) —
            // we show the sample, not emptiness
            (crate::fx_thumbs::FxThumbBase::Pattern, None)
        } else {
            let warps = self.warp_maps();
            // the address of the event’s SOURCE, not its media (R.59, stage 2):
            // for a generator the values live on the event, and the frame is different for each
            // one. The rule is one for the whole program — `clip_source_addr`.
            let pv_src = kt_engine::clip_source_addr(&anchor);
            let key = crate::thumbs::ThumbKey {
                media: pv_src,
                t: crate::thumbs::material_key_t(
                    anchor.source_in.0,
                    warps.get(&anchor.id),
                    anchor.duration,
                    anchor.reversed,
                    pv_t.0 - anchor.start.0,
                    pv_step,
                ),
            };
            match self.thumbs.get(&key) {
                Some(img) => (crate::fx_thumbs::FxThumbBase::Thumb(key), Some(img)),
                None => {
                    if decode_ok {
                        // the nearest ready thumbnail from the sidecar DB instead of
                        // decoding (the bin scrub mechanism, item 14 of round 7)
                        self.thumbs.request_near(key, pv_step * 2);
                    }
                    let poster =
                        if decode_ok { self.thumbs.poster_t_or_request(pv_src) } else { None };
                    let pk = poster
                        .map(|t| crate::thumbs::ThumbKey { media: pv_src, t })
                        .and_then(|pk| self.thumbs.get(&pk).map(|img| (pk, img)));
                    match pk {
                        Some((pk, img)) => (crate::fx_thumbs::FxThumbBase::Thumb(pk), Some(img)),
                        None => (crate::fx_thumbs::FxThumbBase::Pattern, None),
                    }
                }
            }
        };
        let mut pv = crate::fx_thumbs::PickerCtx::new(
            &self.fx_thumbs,
            pv_base,
            pv_img,
            pv_chain,
            pv_u,
            pv_t.0,
            pv_cam,
        );
        // There is one scroll — of the BODY (`window_body`): the outer `vscroll` of the window
        // would scroll the action buttons too, and “Done” went past the bottom
        // edge (R.67, found by the live scenario `curves`). The window height
        // is the same as before — the body itself leaves room for the buttons.
        let shown = egui::Window::new(title)
            .id(egui::Id::new(("fx", ed.clip)))
            .open(&mut open)
            .default_size([580.0, 520.0])
            .min_size([460.0, 320.0])
            .resizable(true)
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                for (n, id) in members.iter().enumerate() {
                    let Some(clip) = self.model.clip(*id).cloned() else { continue };
                    let mut fx = clip.fx.clone();
                    let mut bypass = clip.fx_bypass.clone();
                    bypass.resize(fx.len(), false);
                    let mut auto = clip.fx_auto.clone();
                    auto.resize(fx.len(), Vec::new());
                    // The LEADING “Geometry” node (R.62) is
                    // spliced into the strip at its own position,
                    // like Pan/Crop in Vegas: it is dragged, not deleted, and the same node
                    // is added from the catalog. In the model it is a field of the event,
                    // not a slot: slot indices and curve addresses do not move because of it.
                    // The splice lives ONLY inside the strip call: outward,
                    // slot indices stay raw (curve addresses, publication and the context
                    // menu are addressed by them), and choosing the node is its own variant
                    // `ChainSel::Lead`.
                    let lead_pos0 =
                        (clip.kind == ClipKind::Video).then(|| clip.geometry_pos.min(fx.len()));
                    let mut lead_pos = lead_pos0.unwrap_or(0);
                    let mut lead_node = clip.geometry.node();
                    let mut lead_auto = clip.geometry_auto.clone();
                    let mut lead_rule = clip.geometry.rule;
                    let mut chain_on = !clip.fx_off;
                    // HEAD of the strip (R.59, stage 2): for an event-generator
                    // it is its SOURCE, the first crumb before the effects —
                    // exactly as the owner asked. The values are edited right here,
                    // in this same window; the generator has no window of its own any more.
                    let mut gen_kind = clip.r#gen.clone();
                    let mut gen_auto = clip.gen_auto.clone();
                    let mut gen_edit = crate::gen_ui::GenEdit::default();
                    let mut ch = crate::widgets::ChainChanged::default();
                    let mut params_changed = crate::widgets::ParamsChanged::default();
                    let head = crate::tf!(
                        k::fx::CHAIN_OF,
                        if clip.kind == ClipKind::Audio {
                            tr(k::fx::KIND_AUDIO)
                        } else {
                            tr(k::fx::KIND_VIDEO)
                        },
                        id.0
                    );
                    // collapsing per chain: a pair shows both, and the extra one can be
                    // collapsed (“one can give the option of hide/toggle”)
                    egui::CollapsingHeader::new(head)
                        .id_salt(("fxgrp", id.0))
                        .default_open(n == 0 || members.len() <= 2)
                        .show(ui, |ui| {
                            // the selected slot and the search row are taken into
                            // local variables: otherwise two fields of the same
                            // `ed` would be borrowed at the same time
                            let mut sel_slot =
                                *ed.selected.get(id).unwrap_or(&crate::widgets::ChainSel::Fx(0));
                            // there is no head — nothing to choose it with: for an event without
                            // a generator it is not drawn at all
                            if gen_kind.is_none()
                                && !pub_doc.sequence(clip.media).is_some()
                                && sel_slot == crate::widgets::ChainSel::Head
                            {
                                sel_slot = crate::widgets::ChainSel::Fx(0);
                            }
                            if lead_pos0.is_none() && sel_slot == crate::widgets::ChainSel::Lead {
                                sel_slot = crate::widgets::ChainSel::Fx(0);
                            }
                            // COMPOSITION PARAMETERS — the HEAD of the strip (R.63
                            // item 12), not a button of its own and not a window of its own:
                            // “they can be placed in the same fx-chain for
                            // convenience” (the owner). There is one door, and it is there,
                            // where the generator’s head is.
                            let is_nest = pub_doc.sequence(clip.media).is_some();
                            let mut query = std::mem::take(&mut ed.picker_query);
                            let head_slot = match (gen_kind.as_ref(), is_nest) {
                                (Some(g), _) => Some(crate::widgets::HeadSlot {
                                    label: crate::tf!(
                                        k::genw::HEAD,
                                        crate::gen_ui::kind_name(g)
                                    ),
                                    // the source has no switch: switched off, it
                                    // would leave the event with no picture at all
                                    on: None,
                                    hint: tr(k::genw::HEAD_HINT),
                                    color: crate::theme::ACCENT,
                                }),
                                // a reel INSTANCE of a generator source has none,
                                // but the handles declared by the reel are there —
                                // and they live exactly here (R.63 item 12).
                                //
                                // The caption is the NAME OF THE REEL, not
                                // “Composition parameters”: a long string covered
                                // the strip, and the accent color on
                                // selection gave a “long blue rectangle”
                                // (the owner, R.67 item 8). The color is as for a slot;
                                // the full name is in the status-line hint.
                                (None, true) => Some(crate::widgets::HeadSlot {
                                    label: format!(
                                        "{} {}",
                                        crate::icons::PIN,
                                        pub_doc
                                            .sequence(clip.media)
                                            .map(|s| s.name.clone())
                                            .unwrap_or_else(|| tr(k::pubp::INST_TITLE).to_string())
                                    ),
                                    on: None,
                                    hint: tr(k::pubp::INST_OPEN),
                                    color: ui.visuals().text_color(),
                                }),
                                (None, false) => None,
                            };
                            // splice of the leading node at its position + conversion of the choice
                            // into strip indices
                            let (mut fx_s, mut bypass_s, mut auto_s) =
                                (fx.clone(), bypass.clone(), auto.clone());
                            let mut sel_s = sel_slot;
                            if let Some(lp) = lead_pos0 {
                                let lp = lp.min(fx_s.len());
                                fx_s.insert(lp, lead_node.clone());
                                bypass_s.insert(lp, false);
                                auto_s.insert(lp, lead_auto.clone());
                                sel_s = match sel_slot {
                                    crate::widgets::ChainSel::Fx(i) => {
                                        crate::widgets::ChainSel::Fx(if i >= lp {
                                            i + 1
                                        } else {
                                            i
                                        })
                                    }
                                    crate::widgets::ChainSel::Lead => {
                                        crate::widgets::ChainSel::Fx(lp)
                                    }
                                    other => other,
                                };
                            }
                            ch = crate::widgets::chain_strip(
                                ui,
                                ("fxchain", id.0),
                                crate::widgets::ChainEdit {
                                    fx: &mut fx_s,
                                    bypass: &mut bypass_s,
                                    auto: &mut auto_s,
                                    selected: &mut sel_s,
                                    master_on: &mut chain_on,
                                    head: head_slot,
                                    presets: crate::widgets::ChainPresets {
                                        rows: &preset_rows,
                                        save: &mut preset_save,
                                        action: &mut preset_action,
                                    },
                                },
                                clip.kind == ClipKind::Audio,
                                &mut query,
                                &mut fx_clip,
                                clip.source_in,
                                &mut hover_out,
                                &mut crate::widgets::ChainCtx {
                                    chain: *id,
                                    picked: &mut panel_pick,
                                },
                                // an audio chain has no pictures: an audio thumbnail cannot be drawn
                                // (the owner’s decision)
                                if clip.kind == ClipKind::Audio { None } else { Some(&mut pv) },
                            );
                            ed.picker_query = query;
                            // the leading node is UNSTUCK back: the
                            // model gets the raw chain and, separately,
                            // the node with its position and curves. The cross on it is a refusal (rule 16):
                            // the node stays in the model, the person gets an error.
                            if lead_pos0.is_some() {
                                match fx_s.iter().position(|f| f.is_lead_geometry()) {
                                    Some(li) => {
                                        lead_node = fx_s.remove(li);
                                        if li < bypass_s.len() {
                                            bypass_s.remove(li);
                                        }
                                        if li < auto_s.len() {
                                            lead_auto = auto_s.remove(li);
                                        }
                                        lead_pos = li;
                                        sel_s = match sel_s {
                                            crate::widgets::ChainSel::Fx(j) if j == li => {
                                                crate::widgets::ChainSel::Lead
                                            }
                                            crate::widgets::ChainSel::Fx(j) if j > li => {
                                                crate::widgets::ChainSel::Fx(j - 1)
                                            }
                                            other => other,
                                        };
                                    }
                                    None => lead_refused = true,
                                }
                            }
                            fx = fx_s;
                            bypass = bypass_s;
                            auto = auto_s;
                            sel_slot = sel_s;
                            ed.selected.insert(*id, sel_slot);
                            // ---- HEAD OF THE INSTANCE: composition
                            // parameters (R.63 item 12) — the same form
                            // that lived in its own window, now in the same FX window
                            if sel_slot == crate::widgets::ChainSel::Head && is_nest {
                                let hc = crate::pubparam::InstHeadCtx {
                                    doc: &pub_doc,
                                    inst: &clip,
                                    seq: clip.media,
                                    playhead,
                                    tl_view,
                                    axis_linked,
                                };
                                // the automation view of the head is its own;
                                // a generator reel instance does not have one,
                                // so the map `gen_views` is free
                                let view = ed.gen_views.entry(*id).or_default();
                                let out = crate::app::KtApp::draw_inst_head(
                                    &hc,
                                    ui,
                                    view,
                                    &mut ed.inst_query,
                                );
                                if let Some(t) = out.seek {
                                    seek = Some(t);
                                }
                                axis_req = axis_req.merge_req(&out.axis);
                                inst_ops.extend(out.ops);
                            }
                            // ---- HEAD: source form (R.59) --------
                            if let (crate::widgets::ChainSel::Head, Some(g)) =
                                (sel_slot, gen_kind.as_mut())
                            {
                                let gen_pubc = self.publish_ctx(
                                    &pub_doc,
                                    *id,
                                    kt_model::ParamOwner::Generator,
                                );
                                let gen_link_doc = &pub_doc;
                                let gen_me = *id;
                                let gen_links =
                                    move || crate::automation::link_events(gen_link_doc, gen_me);
                                let gctx = crate::gen_ui::GenCtx {
                                    links: Some(&gen_links),
                                    default_shape: dflt_shape,
                                    clip: &clip,
                                    pubc: Some(&gen_pubc),
                                    uses: uses_of.get(&clip.media).copied().unwrap_or(1),
                                    frame: frame_size,
                                    frame_step,
                                    playhead,
                                    tl_view,
                                    axis_linked,
                                };
                                let view = ed.gen_views.entry(*id).or_default();
                                let warn = ed.gen_warn.entry(*id).or_default();
                                gen_edit = crate::gen_ui::draw_gen_section(
                                    ui,
                                    &gctx,
                                    g,
                                    &mut gen_auto,
                                    view,
                                    warn,
                                    &mut hover_out,
                                );
                                if let Some(t) = gen_edit.seek {
                                    seek = Some(t);
                                }
                                // requests about the AXIS are collected from ALL
                                // graphs of the window and applied once (R.40 item 15)
                                axis_req.set_linked =
                                    gen_edit.axis.set_linked.or(axis_req.set_linked);
                                axis_req.set_view = gen_edit.axis.set_view.or(axis_req.set_view);
                            }
                            // the LEADING node is edited by the same code as a slot: for the duration of the section
                            // it is put into the tail of a local chain and
                            // taken back out (its curves’ address is its own —
                            // `usize::MAX`)
                            let lead_sel =
                                sel_slot == crate::widgets::ChainSel::Lead && lead_pos0.is_some();
                            if lead_sel {
                                fx.push(lead_node.clone());
                                auto.push(lead_auto.clone());
                            }
                            let sel = if lead_sel {
                                fx.len() - 1
                            } else {
                                sel_slot.fx().unwrap_or(usize::MAX)
                            };
                            let view_key = if lead_sel { usize::MAX } else { sel };
                            if let Some(f) = fx.get_mut(sel) {
                                // parameters — by ONE code, by the model specs;
                                // the cursor position in the domain of the
                                // slot curve is the event’s material
                                // (R.37: speed does not squeeze the axis)
                                let at = kt_model::clip_material_at(
                                    clip.source_in,
                                    clip.start,
                                    clip.duration,
                                    clip.reversed,
                                    frame_step,
                                    playhead,
                                );
                                let view_sel =
                                    ed.auto_views.get(&(*id, view_key)).and_then(|v| v.sel);
                                let mut bind = auto.get_mut(sel).map(|curves| {
                                    crate::widgets::CurveBind { curves, sel: view_sel, at }
                                });
                                // R.43 item 291: a key edit by NUMBER goes into
                                // `auto`, not into `fx` — and it must be committed
                                // by the SetFxAuto command. While there was one flag,
                                // the local clone of `auto` died together with the
                                // frame, and the field “snapped
                                // back to the previous value”.
                                // THE PUBLICATION DOOR ALSO EXISTS FOR THE
                                // LEADING NODE (R.63 item 9): “why does the first
                                // geometry have no way to publish its parameters
                                // while the second one does?” — the difference
                                // was only that the node had no address.
                                // Now it does, and the field captions lead to
                                // the same door by the same code.
                                let pubc = Some(self.publish_ctx(
                                    &pub_doc,
                                    *id,
                                    if lead_sel {
                                        kt_model::ParamOwner::Geometry
                                    } else {
                                        kt_model::ParamOwner::FxSlot { slot: sel }
                                    },
                                ));
                                // the conform rule — only on the LEADING
                                // node: “follow the project” or its own
                                if f.is_lead_geometry() {
                                    let label = |c: kt_model::Conform| match c {
                                        kt_model::Conform::Project => tr(k::fx::CONFORM_PROJECT),
                                        kt_model::Conform::Fit => tr(k::fx::CONFORM_FIT),
                                        kt_model::Conform::Fill => tr(k::fx::CONFORM_FILL),
                                        kt_model::Conform::Stretch => tr(k::fx::CONFORM_STRETCH),
                                        kt_model::Conform::AsIs => tr(k::fx::CONFORM_AS_IS),
                                    };
                                    // a row of five words, not a drop-down
                                    // list: one click, everything is visible
                                    // (rule 9 — the mode is visible, not hidden)
                                    let r = ui.horizontal(|ui| {
                                        ui.label(tr(k::fx::CONFORM));
                                        for c in [
                                            kt_model::Conform::Project,
                                            kt_model::Conform::Fit,
                                            kt_model::Conform::Fill,
                                            kt_model::Conform::Stretch,
                                            kt_model::Conform::AsIs,
                                        ] {
                                            params_changed.base |= ui
                                                .selectable_value(&mut lead_rule, c, label(c))
                                                .changed();
                                        }
                                    });
                                    crate::widgets::field_probe::note(
                                        "geo.conform",
                                        r.response.rect,
                                    );
                                    crate::app::hint_into(
                                        &mut hover_out,
                                        r.response,
                                        tr(k::fx::LEAD_HINT),
                                    );
                                }
                                // the “Parameters” GROUP is the model of “Transform”
                                // (R.66); the leading node has its own form
                                // with its own groups, which we do not duplicate
                                if f.is_lead_geometry() {
                                    params_changed |= draw_video_fx_params(
                                        ui,
                                        f,
                                        bind.as_mut(),
                                        &cameras,
                                        clip.kind == ClipKind::Video,
                                        &mut hover_out,
                                        pubc.as_ref(),
                                    );
                                } else if let Some(ch) = crate::widgets::group(
                                    ui,
                                    "fx.params",
                                    tr(k::grp::PARAMS),
                                    |ui| {
                                        draw_video_fx_params(
                                            ui,
                                            f,
                                            bind.as_mut(),
                                            &cameras,
                                            clip.kind == ClipKind::Video,
                                            &mut hover_out,
                                            pubc.as_ref(),
                                        )
                                    },
                                ) {
                                    params_changed |= ch;
                                }
                                let mut auto_ch = false;
                                if let Some(curves) = auto.get_mut(sel) {
                                    let view = ed.auto_views.entry((*id, view_key)).or_default();
                                    // the domain of the event’s slot is the
                                    // material; the conversion accounts for REVERSE
                                    // and nothing else (R.37: speed
                                    // does not squeeze the axis)
                                    let to_tl = |t: kt_time::Tick| {
                                        kt_model::clip_timeline_at(
                                            clip.source_in,
                                            clip.start,
                                            clip.duration,
                                            clip.reversed,
                                            frame_step,
                                            t,
                                        )
                                    };
                                    let to_mat = |t: kt_time::Tick| {
                                        kt_model::clip_material_at(
                                            clip.source_in,
                                            clip.start,
                                            clip.duration,
                                            clip.reversed,
                                            frame_step,
                                            t,
                                        )
                                    };
                                    let link_doc = &pub_doc;
                                    let me = *id;
                                    let links = move || crate::automation::link_events(link_doc, me);
                                    let actx = crate::automation::clip_ctx(
                                        &clip,
                                        f,
                                        frame_step,
                                        &to_tl,
                                        &to_mat,
                                        tl_view,
                                        axis_linked,
                                        dflt_shape,
                                        Some(&links),
                                    );
                                    if paste_req && paste_target.is_none() && !lead_sel {
                                        paste_target = Some((*id, sel));
                                    }
                                    let r = draw_auto_for(
                                        ui,
                                        f,
                                        curves,
                                        view,
                                        actx,
                                        playhead,
                                        &mut paste_req,
                                        (id.0, view_key),
                                        &mut hover_out,
                                    );
                                    auto_ch = r.changed;
                                    if let Some(e) = &r.error {
                                        auto_error = Some(e.clone());
                                    }
                                    // a right click on a key is also a target (item
                                    // 364); the leading node has no slot address
                                    if let (Some((ci, ki)), false) = (r.key_menu, lead_sel) {
                                        panel_pick = Some(crate::ctx_menu::PanelTarget::CurveKey {
                                            chain: *id,
                                            slot: sel,
                                            curve: ci,
                                            key: ki,
                                        });
                                    }
                                    // The parameter field reads the selected key
                                    // BEFORE the graph is drawn, so a change of
                                    // selection is visible to it only on the next
                                    // frame. In idle there may be no next frame
                                    // (repaint = motion, item 38), and the field
                                    // got stuck on the value of the previous key —
                                    // we request the frame
                                    // explicitly (R.43 item 291).
                                    if view.sel != view_sel {
                                        ui.ctx().request_repaint();
                                    }
                                    if let Some(t) = r.seek {
                                        seek = Some(t);
                                    }
                                    axis_req = axis_req.merge(&r);
                                }
                                ch.auto |= auto_ch;
                            }
                            if lead_sel {
                                lead_auto = auto.pop().unwrap_or_default();
                                lead_node = fx.pop().unwrap_or_else(FxInstance::geometry);
                            }
                            // the strip itself says that the chain
                            // is empty — there is no need to repeat it
                        });
                    // the leading node is UNSTUCK back: the model
                    // gets the raw chain and, separately, the node
                    // with its position and curves. The cross on it is a refusal (rule 16): the node stays in
                    // the model, the person gets an error in the status line.
                    let geometry_edit = lead_pos0.map(|_| {
                        let g = kt_model::EventGeometry { rule: lead_rule, ..Default::default() }
                            .with_node(&lead_node);
                        (g, lead_pos.min(fx.len()), lead_auto.clone())
                    });
                    let geo_changed = geometry_edit.as_ref().is_some_and(|(g, p, a)| {
                        *g != clip.geometry || *p != clip.geometry_pos || *a != clip.geometry_auto
                    });
                    if ch.any()
                        || params_changed.any()
                        || gen_edit.changed.any()
                        || gen_edit.apply_all
                        || gen_edit.switch_kind.is_some()
                        || geo_changed
                    {
                        edits.push(Edited {
                            clip: *id,
                            fx: (ch.fx || params_changed.base).then_some(fx),
                            auto: (ch.auto || params_changed.curve).then_some(auto),
                            bypass: ch.bypass.then_some(bypass),
                            off: ch.master.then_some(!chain_on),
                            r#gen: gen_edit.changed.base.then_some(gen_kind).flatten(),
                            gen_auto: gen_edit.changed.curve.then_some(gen_auto),
                            own_step: gen_edit.preset,
                            apply_all: gen_edit.apply_all,
                            switch_kind: gen_edit.switch_kind,
                            geometry: geo_changed.then_some(geometry_edit).flatten(),
                        });
                    }
                }
                });
                crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    // button rectangles are exposed to live scenarios (R.27)
                    let r = ui.button(tr(k::fx::DONE));
                    crate::widgets::field_probe::note("fx.done", r.rect);
                    if r.clicked() {
                        ed.open = false;
                    }
                    let r = ui.button(tr(k::fx::CANCEL));
                    crate::widgets::field_probe::note("fx.cancel", r.rect);
                    if r.clicked() {
                        revert = true;
                        ed.open = false;
                    }
                });
                });
            });
        // the whole window frame — the landing-page screenshots are cropped by it (site/screenshots.py)
        if let Some(r) = &shown {
            crate::widgets::field_probe::note("win.fx", r.response.rect);
        }

        // the strip’s requests about presets (R.48): the strip only puts them — the window writes
        // the file, because the name dialog lives in the application
        if let Some(mut block) = preset_save.take() {
            // the leading node is a property of the event and does not go into a chain preset
            block.slots.retain(|s| !s.fx.is_lead_geometry());
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
        if let Some(t) = panel_pick {
            self.set_panel_target(t); // targets cancel each other out (item 363)
        }
        for c in inst_ops {
            self.apply_coalesced(c, ed.gesture);
        }
        if let Some(h) = hover_out {
            self.hover_hint = Some(h); // the hint goes to the status line
        }
        if let Some(e) = auto_error {
            self.refuse(e);
        }
        if lead_refused {
            self.refuse(tr(k::fx::LEAD_LOCKED));
        }
        // the requests of the graph about the AXIS are applied in one place (R.40 item 15)
        self.apply_axis_req(axis_req);
        if let Some(t) = seek {
            // “the cursor inside the mini-timeline moves the cursor on the main timeline, so that one
            // immediately sees what it refers to” (R.33 item 5)
            self.player.seek(kt_time::Tick(t.0.max(0)));
        }
        if !open {
            ed.open = false;
        }
        self.fx_clipboard = fx_clip;
        // The clipboard read order — AFTER the drawing and in the BACKGROUND (R.61): one cannot read
        // synchronously; the clipboard owner on X11 is our own window.
        if paste_req && self.env_read.is_none() {
            let t = paste_target.unwrap_or((ed.clip, 0));
            self.env_read = Some(crate::envclip::spawn_read(t));
        }
        if revert {
            // “Cancel” rolls back the WHOLE window: for a pair, both chains
            for (id, fx, auto, bypass, off) in ed.backup.clone() {
                self.set_fx_coalesced(id, fx, ed.gesture);
                self.set_fx_auto_coalesced(id, auto, ed.gesture);
                self.apply_coalesced(Command::SetFxBypass { id, bypass }, ed.gesture);
                self.apply_coalesced(Command::SetFxOff { id, off }, ed.gesture);
            }
            // ...and the SOURCE VALUES too: the head of the strip is part of the same
            // window session, and rolling back half of it would be a lie
            for (id, kind, auto) in ed.gen_backup.clone() {
                if self.model.clip(id).is_none_or(|c| c.r#gen == kind && c.gen_auto == auto) {
                    continue;
                }
                self.apply_coalesced(Command::SetClipGen { id, kind: kind.clone() }, ed.gesture);
                self.apply_coalesced(Command::SetClipGenAuto { id, auto }, ed.gesture);
            }
            // ...and the leading “Geometry” node (R.62) — the same strip
            for (id, geometry, pos, auto) in ed.geo_backup.clone() {
                if self.model.clip(id).is_none_or(|c| {
                    c.geometry == geometry && c.geometry_pos == pos && c.geometry_auto == auto
                }) {
                    continue;
                }
                self.apply_coalesced(Command::SetGeometry { id, geometry, pos, auto }, ed.gesture);
            }
            // ...and the COMPOSITION HANDLES from the head of the strip (R.63 item 12)
            for (id, was) in ed.ov_backup.clone() {
                let now = self.model.clip(id).map(|c| c.overrides.clone()).unwrap_or_default();
                if now == was {
                    continue;
                }
                for o in &now {
                    if !was.iter().any(|b| b.pub_id == o.pub_id) {
                        self.apply_coalesced(
                            Command::ClearOverride { id, pub_id: o.pub_id },
                            ed.gesture,
                        );
                    }
                }
                for b in was {
                    self.apply_coalesced(
                        Command::SetOverride { id, ov: Box::new(b) },
                        ed.gesture,
                    );
                }
            }
            return;
        }
        for e in edits {
            // Changing the KIND is a property of the MEDIA and its OWN history step (rule 20): one Ctrl+Z
            // must undo “accidentally clicked another kind”,
            // and not all the work in the window. The model
            // itself re-dresses the event values (`fit_gen_to_media`), so the command is one.
            if let Some(kind) = e.switch_kind {
                let Some(media) = self.model.clip(e.clip).map(|c| c.media) else { continue };
                self.gesture_counter += 1;
                let g = self.gesture_counter;
                self.apply_coalesced(
                    Command::SetMediaPath { id: media, path: kt_model::gen_kind_path(&kind) },
                    g,
                );
                continue;
            }
            // A preset sets EVERYTHING (values and curves) — also its own history step: pressed it,
            // did not like it, one Ctrl+Z restored the set.
            let gesture = if e.own_step {
                self.gesture_counter += 1;
                self.gesture_counter
            } else {
                ed.gesture
            };
            if let Some(fx) = e.fx {
                self.set_fx_coalesced(e.clip, fx, gesture);
            }
            if let Some(mut auto) = e.auto {
                // keys are kept sorted (the model validates this)
                for curves in &mut auto {
                    for c in curves.iter_mut() {
                        c.keys.sort_by_key(|k| k.t.0);
                    }
                }
                self.set_fx_auto_coalesced(e.clip, auto, gesture);
            }
            if let Some(mut b) = e.bypass {
                while b.last() == Some(&false) {
                    b.pop(); // trailing false values are not serialized needlessly
                }
                self.apply_coalesced(Command::SetFxBypass { id: e.clip, bypass: b }, gesture);
            }
            if let Some(off) = e.off {
                self.apply_coalesced(Command::SetFxOff { id: e.clip, off }, gesture);
            }
            if let Some((geometry, pos, mut auto)) = e.geometry {
                for c in auto.iter_mut() {
                    c.keys.sort_by_key(|k| k.t.0);
                }
                self.apply_coalesced(
                    Command::SetGeometry { id: e.clip, geometry, pos, auto },
                    gesture,
                );
            }
            // Generator values: the command ONLY on a really changed value (item 200) — otherwise the
            // revision would grow on every frame of the gesture.
            if let Some(kind) = e.r#gen
                && self.model.clip(e.clip).is_some_and(|c| c.r#gen.as_ref() != Some(&kind))
            {
                self.apply_coalesced(Command::SetClipGen { id: e.clip, kind: Some(kind) }, gesture);
            }
            if let Some(mut auto) = e.gen_auto {
                for c in auto.iter_mut() {
                    c.keys.sort_by_key(|k| k.t.0);
                }
                if self.model.clip(e.clip).is_some_and(|c| c.gen_auto != auto) {
                    self.apply_coalesced(Command::SetClipGenAuto { id: e.clip, auto }, gesture);
                }
            }
            // “Apply to all” is a SEPARATE command and a separate history step: editing one event
            // does not touch its neighbours, while this gesture
            // touches all of them and says so in its caption.
            if e.apply_all
                && let Some(c) = self.model.clip(e.clip).cloned()
                && let Some(kind) = c.r#gen.clone()
            {
                self.gesture_counter += 1;
                let g = self.gesture_counter;
                self.apply_coalesced(
                    Command::ApplyGenToAll { media: c.media, kind, auto: c.gen_auto.clone() },
                    g,
                );
            }
        }
    }
}

/// Automation section of the slot: choice of curves, copy-paste of envelopes, key at the cursor —
/// and the widget itself (`automation::draw`: graph ↔ table). A slot without automatable
/// parameters (Mirror) is not drawn at all.
///
/// Returns the whole `AutoResult` (R.40 item 15): besides “changed” and “where to move the cursor”
/// it carries requests about the axis — to toggle the lock and move the window of the MAIN timeline.
/// The pair `(bool, Option<Tick>)` would silently lose
/// them, while the lock is now one for the whole program.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_auto_for(
    ui: &mut egui::Ui,
    f: &FxInstance,
    curves: &mut Vec<kt_model::ParamCurve>,
    view: &mut crate::automation::AutoView,
    ctx: crate::automation::AutoCtx<'_>,
    playhead: kt_time::Tick,
    // Whether “paste envelope” is pressed: the buffer itself is read LATER and in the BACKGROUND
    // — see `crate::envclip` (R.61).
    paste_req: &mut bool,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    hover_out: &mut Option<String>,
) -> crate::automation::AutoResult {
    let (src_in, mat_end) = (ctx.range.0, ctx.range.1.0);
    // only Smooth parameters: structural ones (the reverb line length) are not subject to a
    // curve — changing them rebuilds the DSP (R.33)
    let specs: Vec<&kt_model::ParamSpec> = f.auto_params().collect();
    if specs.is_empty() {
        return crate::automation::AutoResult::default();
    }
    let mut changed = false;
    ui.separator();
    ui.horizontal(|ui| {
        // parameters without a curve are candidates for adding
        let free: Vec<&kt_model::ParamSpec> =
            specs.iter().copied().filter(|s| !curves.iter().any(|c| c.param == s.name)).collect();
        if !free.is_empty() {
            // a MENU, not a ComboBox: its selected text never
            // changed (it is a list of actions, not a selector),
            // and at the same time the button gets a name for accessibility — the ComboBox gave empty,
            // and the item could not be reached other than with the mouse
            let add = ui.menu_button(tr(k::fx::AUTO_ADD), |ui| {
                {
                    for spec in &free {
                        let n: &str = &spec.name;
                        let item = ui.button(crate::widgets::param_label(n));
                        // a probe on each item: otherwise a live
                        // scenario has nothing to aim at (the R.27 rule —
                        // coordinates come from the program itself)
                        crate::widgets::field_probe::note(&format!("auto.add.{n}"), item.rect);
                        if item.clicked() {
                            // the starting curve is a constant of the current value
                            let v0 = f.get_param(n).unwrap_or(spec.default);
                            curves.push(kt_model::ParamCurve::constant(
                                n,
                                src_in,
                                kt_time::Tick(mat_end),
                                v0,
                                ctx.default_shape,
                            ));
                            // a new parameter becomes ACTIVE (R.38 item 17:
                            // “added a second parameter — but only the first
                            // one is edited”): previously a click on the
                            // graph kept putting keys into the first curve,
                            // and the second looked dead
                            view.active = curves.len() - 1;
                            view.sel = None;
                            changed = true;
                        }
                    }
                }
            });
            crate::widgets::field_probe::note("auto.add", add.response.rect);
        }
        // the choice of the active curve, the key buttons and the CURVE BLOCK (the form of all segments,
        // formula, link, delete) — IN THE WIDGET ITSELF
        // (R.67): all carriers of curves receive them at once
    });
    ui.horizontal(|ui| {
        // copy-paste of envelopes (model — kt_model::ops, R.33)
        let cp = ui.add_enabled(!curves.is_empty(), egui::Button::new(tr(k::fx::COPY_ENV)));
        crate::widgets::field_probe::note("env.copy", cp.rect);
        if cp.clicked() {
            let block = kt_model::ops::EnvelopeBlock {
                fx_tag: kt_model::ops::fx_tag(f),
                curves: curves.clone(),
                src_in,
            };
            if let Ok(json) = serde_json::to_string_pretty(&block) {
                ui.ctx().copy_text(json);
            }
        }
        // The button NEVER goes dark: the buffer contents are read ON PRESS, not kept as a mirror
        // inside the program (R.61). The read is a finite
        // background task: on X11 the window owns the
        // buffer, and a synchronous request to ITSELF would hang
        // in the event loop, which is drawing at that moment.
        let pb = ui.button(tr(k::fx::PASTE_ENV));
        crate::widgets::field_probe::note("env.paste", pb.rect);
        if crate::app::hint_into(hover_out, pb, tr(k::fx::PASTE_ENV_HINT)).clicked() {
            *paste_req = true;
        }
        // The “+ keyframe at playhead” button has been REMOVED here (R.40 item 18, verbatim:
        // “and also “+keyframe at playhead” is a superfluous button, because there
        // is a “+” in the automation timeline”). Its only difference from “+” was that
        // it wrote to ALL curves of the slot, which the caption never
        // said. This behaviour moved to “+” with the Ctrl modifier,
        // and the hint says so (rule 18).
    });
    // a single widget: graph ↔ table (R.33 S3)
    let mut res =
        crate::automation::draw(ui, ("auto", id), &ctx, curves, view, playhead, hover_out);
    res.changed |= changed;
    res
}
pub(crate) fn fx_display_name(f: &FxInstance) -> &'static str {
    match f {
        FxInstance::Gain { .. } => "Gain",
        // layer strength is not a user effect: it is absent from the picker, while in the morpher strip
        // it is captioned “Strength” (R.38 item 12)
        FxInstance::LayerMix { .. } => "Layer mix",
        FxInstance::SlowedReverb { .. } => "Slowed+Reverb",
        // the leading node is marked with a dot: it is single, cannot be deleted and is not from the catalog
        FxInstance::Geometry { lead: true, .. } => "• Geometry",
        FxInstance::Geometry { .. } => "Geometry",
        FxInstance::CamMotion { .. } => "CamMotion",
        FxInstance::Mirror { .. } => "Mirror",
        FxInstance::Blur { .. } => "Blur",
        FxInstance::Greenscreen { .. } => "Greenscreen",
        FxInstance::Edges { .. } => "Edges",
        FxInstance::Noise { .. } => "Noise",
        FxInstance::Vhs { .. } => "VHS",
        FxInstance::Glow { .. } => "Glow",
        FxInstance::EventGrid { .. } => "EventGrid",
        FxInstance::Pan3d { .. } => "Pan 3D",
        FxInstance::Filter { .. } => "Filter",
        FxInstance::Eq { .. } => "EQ",
        FxInstance::Distortion { .. } => "Distortion",
        FxInstance::Bitcrusher { .. } => "Bitcrusher",
        FxInstance::StereoWidth { .. } => "Stereo Width",
        FxInstance::Tremolo { .. } => "Tremolo",
        FxInstance::AutoPan { .. } => "AutoPan",
        FxInstance::Vibrato { .. } => "Vibrato",
        FxInstance::Chorus { .. } => "Chorus",
        FxInstance::Flanger { .. } => "Flanger",
        FxInstance::Phaser { .. } => "Phaser",
        FxInstance::Delay { .. } => "Delay",
        FxInstance::Compressor { .. } => "Compressor",
        FxInstance::Gate { .. } => "Gate",
        FxInstance::Limiter { .. } => "Limiter",
        FxInstance::PitchShift { .. } => "Pitch Shift",
        FxInstance::ColorGrade { .. } => "Color Grade",
        FxInstance::Duotone { .. } => "Duotone",
        FxInstance::Posterize { .. } => "Posterize",
        FxInstance::Vignette { .. } => "Vignette",
        FxInstance::CornerPin { .. } => "Corner Pin",
        FxInstance::Sharpen { .. } => "Sharpen",
        FxInstance::Pixelate { .. } => "Pixelate",
        FxInstance::ChromaticAberration { .. } => "Chromatic Aberration",
        FxInstance::RadialBlur { .. } => "Radial Blur",
        FxInstance::DropShadow { .. } => "Drop Shadow",
        FxInstance::Halftone { .. } => "Halftone",
        FxInstance::LensDistort { .. } => "Lens Distort",
        FxInstance::Twirl { .. } => "Twirl",
        FxInstance::Ripple { .. } => "Ripple",
        FxInstance::Kaleidoscope { .. } => "Kaleidoscope",
        FxInstance::Cylinder3d { .. } => "Cylinder 3D",
    }
}

/// Video FX templates with the v1 defaults (the “+ video FX…” button).
pub(crate) fn video_fx_templates() -> Vec<(&'static str, FxInstance)> {
    vec![
        // CamMotion is removed from the list (R.34 K2): its keyframes are Pan with curves, and old projects
        // are converted on reading. R.62: the “Geometry” node from the catalog is an additional
        // one (the leading one is always present on an event and does not live in the catalog);
        // the former Pan is abolished
        ("Geometry", FxInstance::geometry_node()),
        ("Mirror", FxInstance::Mirror { horizontal: true, vertical: false }),
        ("Blur", FxInstance::blur(8)),
        ("Greenscreen", FxInstance::greenscreen([0, 255, 0], 70, 25)),
        ("Edges", FxInstance::edges(48)),
        ("Noise", FxInstance::noise(0.08)),
        ("VHS", FxInstance::vhs(4, 0.4)),
        ("Glow", FxInstance::glow(0.65)),
        (
            // R.29: the first 3D effect; defaults are identity (layer 1:1)
            "Pan 3D",
            FxInstance::Pan3d {
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
            },
        ),
        ("EventGrid", FxInstance::EventGrid { cols: 2, rows: 2, center: true }),
        // R.47: per-point color/style + the first honest perspective
        ("Color Grade", FxInstance::color_grade()),
        ("Duotone", FxInstance::duotone()),
        ("Posterize", FxInstance::posterize()),
        ("Vignette", FxInstance::vignette()),
        ("Corner Pin", FxInstance::corner_pin()),
        // R.47 Q3: barriers
        ("Sharpen", FxInstance::sharpen()),
        ("Pixelate", FxInstance::pixelate()),
        ("Chromatic Aberration", FxInstance::chromatic_aberration()),
        ("Radial Blur", FxInstance::radial_blur()),
        ("Drop Shadow", FxInstance::drop_shadow()),
        ("Halftone", FxInstance::halftone()),
        // R.47 Q4: non-linear geometry (Warp primitives) + cylinder
        ("Lens Distort", FxInstance::lens_distort()),
        ("Twirl", FxInstance::twirl()),
        ("Ripple", FxInstance::ripple()),
        ("Kaleidoscope", FxInstance::kaleidoscope()),
        ("Cylinder 3D", FxInstance::cylinder3d()),
    ]
}

/// Effect parameters: numeric ones — in ONE grid by the model specs (`widgets::params_grid`),
/// and next to it — what cannot be expressed by a number: the crop-frame mode, mirror flags,
/// the key color eyedropper, the camera choice, CamMotion
/// keyframes. `cameras` — (id, caption) for the Pan3d combo.
///
/// Before R.33 each variant had its own manual form: the same values were drawn as a slider in one window
/// and as a drag field in another, and the ranges diverged between
/// windows and the spec. Now the range, step and unit come from one place.
/// Rows of the Pan-camera key (R.66). Limits and units are
/// declared HERE, next to each other: a key has no `ParamSpec`
/// (it is a trajectory point, not an effect parameter), yet
/// numbers cannot be written into the drawing code — otherwise
/// the same key would have different bounds in two places.
fn pan_key_rows() -> (
    crate::widgets::NumRow<'static>,
    crate::widgets::NumRow<'static>,
    crate::widgets::NumRow<'static>,
    crate::widgets::NumRow<'static>,
) {
    use kt_model::ParamUnit;
    (
        crate::widgets::NumRow {
            label: "",
            probe: "pan.key.t",
            unit: ParamUnit::Norm,
            min: 0.0,
            max: 1.0,
            step: 0.005,
            slider: None,
        },
        crate::widgets::NumRow {
            label: tr(k::fxp::PAN_X),
            probe: "pan.key.x",
            unit: ParamUnit::Norm,
            min: -4.0,
            max: 4.0,
            step: 0.01,
            slider: None,
        },
        crate::widgets::NumRow {
            label: tr(k::fxp::PAN_Y),
            probe: "pan.key.y",
            unit: ParamUnit::Norm,
            min: -4.0,
            max: 4.0,
            step: 0.01,
            slider: None,
        },
        crate::widgets::NumRow {
            label: tr(k::fxp::ZOOM),
            probe: "pan.key.zoom",
            unit: ParamUnit::Ratio,
            min: 0.05,
            max: 32.0,
            step: 0.01,
            slider: None,
        },
    )
}

pub(crate) fn draw_video_fx_params(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    mut bind: Option<&mut crate::widgets::CurveBind<'_>>,
    cameras: &[(u64, String)],
    on_video: bool,
    // frame hint accumulator (R.48: the frequency response graph has its own hint)
    hover: &mut Option<String>,
    // the PUBLICATION context of the caption (R.59, stage 3). `None` for a carrier whose address
    // does not exist — the morpher patch.
    pubc: Option<&crate::widgets::PublishCtx>,
) -> crate::widgets::ParamsChanged {
    use kt_model::PanMode;
    // R.43 item 291: editing the BASE and editing the CURVE are different commands. Everything
    // non-numeric (checkboxes, modes, eyedroppers, camera choice) is always the base.
    let mut ch = crate::widgets::ParamsChanged::default();
    // a key color is more convenient with an eyedropper than with three numbers
    let skip: &[&str] = match f {
        FxInstance::Greenscreen { .. } => &["key_r", "key_g", "key_b"],
        // the outline line color and the glow tint — also with an eyedropper (R.33 S5)
        FxInstance::Edges { .. } => &["color_r", "color_g", "color_b"],
        FxInstance::Glow { .. } => &["tint_r", "tint_g", "tint_b"],
        FxInstance::DropShadow { .. } => &["tint_r", "tint_g", "tint_b"],
        // both duotone colors — with eyedroppers (R.47 Q2)
        FxInstance::Duotone { .. } => {
            &["shadow_r", "shadow_g", "shadow_b", "high_r", "high_g", "high_b"]
        }
        // on a VIDEO instance the warp means only speed: the reverb does not sound there,
        // and showing its handles would lie to the user (R.37)
        FxInstance::SlowedReverb { .. } if on_video => {
            &["mix", "reverb_delay_ms", "damp", "decay", "wet_gain", "predelay_ms", "width"]
        }
        _ => &[],
    };

    // --- non-numeric controls of the variant ------------------------------------
    let mode_row =
        |ui: &mut egui::Ui, mode: &mut PanMode, ch: &mut crate::widgets::ParamsChanged| {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::MODE));
                egui::ComboBox::from_id_salt("panmode").selected_text(format!("{mode:?}")).show_ui(
                    ui,
                    |ui| {
                        for m in [PanMode::Fit, PanMode::Fill, PanMode::Stretch] {
                            ch.base |= ui.selectable_value(mode, m, format!("{m:?}")).changed();
                        }
                    },
                );
            });
        };
    match f {
        FxInstance::Delay { pingpong, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(pingpong, tr(k::fx::PINGPONG)).changed();
            });
        }
        // the cut kind (R.47): captions from the catalog, values are serde kinds
        FxInstance::Filter { mode, .. } => {
            let label = |m: kt_model::FilterMode| match m {
                kt_model::FilterMode::Lowpass => tr(k::fx::FILTER_LP),
                kt_model::FilterMode::Highpass => tr(k::fx::FILTER_HP),
                kt_model::FilterMode::Bandpass => tr(k::fx::FILTER_BP),
            };
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::MODE));
                egui::ComboBox::from_id_salt("filtermode").selected_text(label(*mode)).show_ui(
                    ui,
                    |ui| {
                        for m in [
                            kt_model::FilterMode::Lowpass,
                            kt_model::FilterMode::Highpass,
                            kt_model::FilterMode::Bandpass,
                        ] {
                            ch.base |= ui.selectable_value(mode, m, label(m)).changed();
                        }
                    },
                );
            });
        }
        FxInstance::CamMotion { mode, ease, .. } => {
            let mut e = *ease;
            mode_row(ui, mode, &mut ch);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut e, tr(k::fx::EASE)).changed() {
                    ch.base = true;
                }
            });
            if let FxInstance::CamMotion { ease, .. } = f {
                *ease = e;
            }
        }
        FxInstance::SlowedReverb { frames, .. } if on_video => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::FRAMES));
                egui::ComboBox::from_id_salt("slowedframes")
                    .selected_text(tr(match frames {
                        kt_model::FrameMode::Nearest => k::fx::FRAMES_NEAREST,
                        kt_model::FrameMode::Blend => k::fx::FRAMES_BLEND,
                    }))
                    .show_ui(ui, |ui| {
                        // R.56 (item 448): “blend” is legal also on a
                        // NESTED reel — the mode is inherited by the content,
                        // and the pair is blended by its leaves. There is nothing left to disable here.
                        for (m, key) in [
                            (kt_model::FrameMode::Nearest, k::fx::FRAMES_NEAREST),
                            (kt_model::FrameMode::Blend, k::fx::FRAMES_BLEND),
                        ] {
                            ch.base |= ui.selectable_value(frames, m, tr(key)).changed();
                        }
                    });
            });
            ui.label(egui::RichText::new(tr(k::fx::RATE_LINKED)).weak().small());
        }
        FxInstance::Mirror { horizontal, vertical } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(horizontal, tr(k::fx::HORIZONTAL)).changed();
                ch.base |= ui.checkbox(vertical, tr(k::fx::VERTICAL)).changed();
            });
        }
        FxInstance::Greenscreen { key_r, key_g, key_b, .. } => {
            ui.horizontal(|ui| {
                let mut col = [*key_r as f32 / 255.0, *key_g as f32 / 255.0, *key_b as f32 / 255.0];
                ui.label(tr(k::fx::KEY_COLOR));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *key_r = (col[0] * 255.0) as u8;
                    *key_g = (col[1] * 255.0) as u8;
                    *key_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::EventGrid { center, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(center, tr(k::fx::CENTER)).changed();
            });
        }
        FxInstance::Edges { color_r, color_g, color_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*color_r as f32 / 255.0, *color_g as f32 / 255.0, *color_b as f32 / 255.0];
                ui.label(tr(k::fx::LINE_COLOR));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *color_r = (col[0] * 255.0) as u8;
                    *color_g = (col[1] * 255.0) as u8;
                    *color_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::Glow { tint_r, tint_g, tint_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*tint_r as f32 / 255.0, *tint_g as f32 / 255.0, *tint_b as f32 / 255.0];
                ui.label(tr(k::fx::TINT));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *tint_r = (col[0] * 255.0) as u8;
                    *tint_g = (col[1] * 255.0) as u8;
                    *tint_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::Duotone { shadow_r, shadow_g, shadow_b, high_r, high_g, high_b, .. } => {
            ui.horizontal(|ui| {
                let mut sh =
                    [*shadow_r as f32 / 255.0, *shadow_g as f32 / 255.0, *shadow_b as f32 / 255.0];
                ui.label(tr(k::fx::SHADOW_COLOR));
                if ui.color_edit_button_rgb(&mut sh).changed() {
                    *shadow_r = (sh[0] * 255.0) as u8;
                    *shadow_g = (sh[1] * 255.0) as u8;
                    *shadow_b = (sh[2] * 255.0) as u8;
                    ch.base = true;
                }
                let mut hi =
                    [*high_r as f32 / 255.0, *high_g as f32 / 255.0, *high_b as f32 / 255.0];
                ui.label(tr(k::fx::HIGH_COLOR));
                if ui.color_edit_button_rgb(&mut hi).changed() {
                    *high_r = (hi[0] * 255.0) as u8;
                    *high_g = (hi[1] * 255.0) as u8;
                    *high_b = (hi[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::DropShadow { tint_r, tint_g, tint_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*tint_r as f32 / 255.0, *tint_g as f32 / 255.0, *tint_b as f32 / 255.0];
                ui.label(tr(k::fx::TINT));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *tint_r = (col[0] * 255.0) as u8;
                    *tint_g = (col[1] * 255.0) as u8;
                    *tint_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::RadialBlur { spin, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(spin, tr(k::fx::SPIN)).changed();
            });
        }
        FxInstance::PitchShift { .. } => {
            // rule 19: the dialog must say WHERE to change what is not
            // in it — the pitch engine lives in the project settings
            ui.label(tr(k::fx::PITCH_ENGINE_NOTE));
        }
        FxInstance::Noise { color, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(color, tr(k::fx::COLOR_NOISE)).changed();
            });
        }
        _ => {}
    }

    // --- frequency response graph of the equalizer (R.48, item 347) ------------------------------
    // Drawn BEFORE the grid: the drag edits the values
    // before the fields show them, and in one frame they are
    // already in sync. The fields remain: the graph complements them, it does not replace them.
    if matches!(f, FxInstance::Eq { .. }) {
        ch |= crate::eq_graph::draw(ui, f, bind.as_deref_mut(), hover);
    }

    // --- numeric parameters: one grid for all variants -------------------
    // linked to curves (R.38 items 15–16): for a parameter with a live curve the field edits the SELECTED KEY,
    // not the invisible base
    // LEADING NODE — its own form (R.64, section 5): groups,
    // X/Y pairs in one row, units, sliders and the frame scheme.
    // The bricks are the same, the value-writing rule is the same — only the layout changes.
    if f.is_lead_geometry() {
        ch |= crate::geomctl::transform_form(ui, f, bind, hover, pubc);
        return ch;
    }
    ch |= crate::widgets::params_grid_bound(ui, ("fxp", fx_display_name(f)), f, skip, bind, pubc);

    // --- tail: Pan3d camera and CamMotion keyframes ------------------------
    match f {
        FxInstance::Pan3d { camera, .. } => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::CAMERA));
                let sel = match camera {
                    None => tr(k::fx::CAMERA_ACTIVE).to_string(),
                    Some(id) => cameras
                        .iter()
                        .find(|(cid, _)| *cid == id.0)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| format!("C{}", id.0)),
                };
                egui::ComboBox::from_id_salt("pan3dcam").selected_text(sel).show_ui(ui, |ui| {
                    ch.base |=
                        ui.selectable_value(camera, None, tr(k::fx::CAMERA_ACTIVE)).changed();
                    for (cid, name) in cameras {
                        ch.base |= ui
                            .selectable_value(camera, Some(kt_model::CameraId(*cid)), name.clone())
                            .changed();
                    }
                });
            });
        }
        FxInstance::CamMotion { ease: true, keyframes, .. } => {
            ui.weak(crate::tf!(k::fx::KEYFRAMES_N, keyframes.len()));
            ui.horizontal(|ui| {
                if ui.button(tr(k::fx::ADD_KF)).clicked() {
                    let t = keyframes.last().map(|k| (k.t + 0.25).min(1.0)).unwrap_or(0.0);
                    keyframes.push(kt_model::CamKeyframe { t, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 });
                    ch.base = true;
                }
                if !keyframes.is_empty() && ui.button(tr(k::fx::DROP_LAST)).clicked() {
                    keyframes.pop();
                    ch.base = true;
                }
            });
            // Pan-camera keys: the limits are DECLARED in
            // ONE place next to each other, and they are drawn
            // by the common row (R.66) — there are no `DragValue`s of their own in the windows
            let (t_row, x_row, y_row, z_row) = pan_key_rows();
            for (i, kf) in keyframes.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("#{i}"));
                    ch.base |= crate::widgets::plain_row(ui, &t_row, &mut kf.t).changed();
                    ch.base |= crate::widgets::pair_row(
                        ui,
                        "",
                        (&x_row, &mut kf.pan_x),
                        (&y_row, &mut kf.pan_y),
                    );
                    ch.base |= crate::widgets::plain_row(ui, &z_row, &mut kf.zoom).changed();
                });
            }
        }
        _ => {}
    }
    ch
}
