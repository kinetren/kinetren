//! An effect's centre dragged on the preview (fx-center-on-preview, `v26.10` T10): "я еще не могу
//! как в том же Vegas указать центр по самой превью".
//!
//! The selected slot of an open effects window that has a centre pair (`center_x`/`center_y`,
//! −1..1 across its frame) shows a crosshair over the preview's picture at that centre; a drag moves
//! it and writes the pair as one history step — the base, or the key at the playhead when the pair
//! has a curve, as the fields do (R.43 item 291). The frame is the one the slot works on: the source
//! placed by the Geometry node before it, the canvas after it (`maskctl::slot_frame_view`).

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::maskctl::MaskOwner;
use eframe::egui;
use kt_model::{ClipId, Command};
use kt_time::Tick;

/// The centre pair's rows.
pub const PAIR: [&str; 2] = ["center_x", "center_y"];
/// The crosshair's radius and its grab, screen px.
const R: f32 = 7.0;
const GRAB: f32 = crate::geomctl::GRAB;

/// A drag in progress: the event, the raw slot and the gesture token.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CenterDrag {
    pub clip: ClipId,
    pub slot: usize,
    pub gesture: u64,
}

impl KtApp {
    /// The selected effect slot of an open effects window that has a centre pair: (event, raw slot).
    pub(crate) fn center_target(&self) -> Option<(ClipId, usize)> {
        self.fx_editors.iter().find_map(|ed| {
            ed.selected.iter().find_map(|(c, sel)| match sel {
                crate::widgets::ChainSel::Fx(i) => {
                    let clip = self.model.clip(*c)?;
                    let f = clip.fx.get(*i)?;
                    PAIR.iter().all(|n| f.get_param(n).is_some()).then_some((*c, *i))
                }
                _ => None,
            })
        })
    }

    /// The pair's value now: the curve at the playhead's material moment, else the base.
    fn center_value(&self, c: ClipId, slot: usize) -> Option<[f64; 2]> {
        let clip = self.model.clip(c)?;
        let f = clip.fx.get(slot)?;
        let at = self.center_material_at(c)?;
        let curves = clip.fx_auto.get(slot);
        Some(PAIR.map(|n| {
            curves
                .and_then(|cs| cs.iter().find(|p| p.param == n && !p.keys.is_empty()))
                .map(|p| kt_model::curve_value(&p.keys, at))
                .unwrap_or_else(|| f.get_param(n).unwrap_or(0.0))
        }))
    }

    fn center_material_at(&self, c: ClipId) -> Option<Tick> {
        let clip = self.model.clip(c)?;
        let step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        let t = self.player.transport.playhead();
        Some(kt_model::clip_material_at(clip.source_in, clip.start, clip.duration, clip.reversed, step, t))
    }

    /// The crosshair over the picture and its drag. Called from the one place in `draw_preview`
    /// where the frame and the masks are, after them: the smaller target wins the pointer.
    pub(crate) fn center_controls(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, img: egui::Rect, canvas: (u32, u32)) {
        // during playback there are no controls (the frame's rule: they cover what is watched)
        if self.player.transport.is_playing() {
            self.center_drag = None;
            return;
        }
        let Some((c, slot)) = self.center_target() else {
            self.center_drag = None;
            return;
        };
        if !self.mask_owner_live(MaskOwner::Slot(c, slot)) {
            return;
        }
        let Some(view) = self.slot_frame_view(c, slot, img, canvas) else { return };
        let Some(v) = self.center_value(c, slot) else { return };
        let (rw, rh) = (view.sp.ref_w, view.sp.ref_h);
        let at = view.to_screen(((0.5 + v[0] * 0.5) * rw, (0.5 + v[1] * 0.5) * rh));
        let grab = egui::Rect::from_center_size(at, egui::vec2(2.0 * GRAB, 2.0 * GRAB));
        crate::widgets::field_probe::note("fx.center", grab);
        let resp = ui.interact(grab, ui.id().with(("fx-center", c.0, slot)), egui::Sense::drag());
        let hot = resp.hovered() || resp.dragged();
        if hot {
            crate::widgets::hint_sink::put(tr(k::fx::CENTER_DRAG_HINT));
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        let col = if hot { crate::theme::ACCENT } else { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220) };
        let shadow = egui::Stroke::new(3.0, egui::Color32::from_black_alpha(140));
        for s in [shadow, egui::Stroke::new(1.5, col)] {
            painter.circle_stroke(at, R, s);
            painter.line_segment([at - egui::vec2(R + 5.0, 0.0), at - egui::vec2(R - 3.0, 0.0)], s);
            painter.line_segment([at + egui::vec2(R - 3.0, 0.0), at + egui::vec2(R + 5.0, 0.0)], s);
            painter.line_segment([at - egui::vec2(0.0, R + 5.0), at - egui::vec2(0.0, R - 3.0)], s);
            painter.line_segment([at + egui::vec2(0.0, R - 3.0), at + egui::vec2(0.0, R + 5.0)], s);
        }
        if resp.drag_started() {
            self.gesture_counter += 1;
            self.center_drag = Some(CenterDrag { clip: c, slot, gesture: self.gesture_counter });
        }
        if resp.dragged()
            && let Some(d) = self.center_drag
            && d.clip == c
            && d.slot == slot
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let (x, y) = view.screen_to_ref(pos);
            let want = [((x / rw.max(1e-9)) * 2.0 - 1.0).clamp(-1.0, 1.0), ((y / rh.max(1e-9)) * 2.0 - 1.0).clamp(-1.0, 1.0)];
            self.write_center(c, slot, want, d.gesture);
        }
        if resp.drag_stopped() {
            self.center_drag = None;
        }
    }

    /// Write the pair: the key at the playhead when it has a curve, else the base — only on a real
    /// change (input-without-work), coalesced into the drag's one history step.
    fn write_center(&mut self, c: ClipId, slot: usize, want: [f64; 2], gesture: u64) {
        let Some(clip) = self.model.clip(c) else { return };
        let Some(at) = self.center_material_at(c) else { return };
        let shape = self.model.project().settings.default_shape;
        let want = want.map(|v| (v * 1000.0).round() / 1000.0);
        let mut fx = clip.fx.clone();
        let mut auto = clip.fx_auto.clone();
        let (mut base, mut keyed) = (false, false);
        for (n, v) in PAIR.iter().zip(want) {
            let curve = auto.get_mut(slot).and_then(|cs| cs.iter_mut().find(|p| p.param == *n && !p.keys.is_empty()));
            match curve {
                Some(p) => {
                    if (kt_model::curve_value(&p.keys, at) - v).abs() > 1e-9 {
                        kt_model::insert_key(&mut p.keys, at, v, shape);
                        keyed = true;
                    }
                }
                None => {
                    if let Some(f) = fx.get_mut(slot)
                        && f.get_param(n).is_some_and(|old| (old - v).abs() > 1e-9)
                    {
                        f.set_param(n, v);
                        base = true;
                    }
                }
            }
        }
        if base {
            self.apply_coalesced(Command::SetFx { id: c, fx }, gesture);
        }
        if keyed {
            self.apply_coalesced(Command::SetFxAuto { id: c, auto }, gesture);
        }
    }
}
