//! The deck face of the VHS tape simulation (vhs-tape-simulation, D11): above the parameter grid,
//! a drum with its heads (two or four; one drum, or a record and a play drum) coloured by their
//! condition, the standard / speed / deck / head / transport switches, the TBC, the dropout
//! compensator and the comb, the tracking field, and the condition of the head clicked on the drum.
//! It edits the SAME passport values the grid would (rule 17): the switches write the base, the
//! head's three numbers and the tracking go through `widgets::param_row_bound` (curves included).
//! The physics is in `fx-video-tape`; this is only its front panel.

use eframe::egui::{self, Color32, Pos2, Stroke};
use kt_model::FxInstance;

use crate::i18n::{k, tr};

/// The rows the face draws; the grid skips them (`fx_editor::draw_video_fx_params`).
pub const ROWS: &[&str] = &[
    "rec_a_spacing",
    "rec_a_gap",
    "rec_a_output",
    "rec_b_spacing",
    "rec_b_gap",
    "rec_b_output",
    "play_a_spacing",
    "play_a_gap",
    "play_a_output",
    "play_b_spacing",
    "play_b_gap",
    "play_b_output",
    "tracking_um",
];

/// A head's condition: what its spacing, gap and output say (the presets of the selector).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cond {
    Clean,
    Dirty,
    Clogged,
    Chipped,
}

impl Cond {
    pub const ALL: [Cond; 4] = [Cond::Clean, Cond::Dirty, Cond::Clogged, Cond::Chipped];
    /// spacing µm, gap µm, output dB
    pub fn values(self) -> [f64; 3] {
        match self {
            Cond::Clean => [0.05, 0.3, 0.0],
            Cond::Dirty => [0.5, 0.3, 0.0],
            Cond::Clogged => [3.0, 0.3, 0.0],
            Cond::Chipped => [0.05, 0.6, -6.0],
        }
    }
    pub fn of(spacing: f64, gap: f64, output: f64) -> Cond {
        if spacing >= 1.5 {
            Cond::Clogged
        } else if output <= -3.0 || gap >= 0.5 {
            Cond::Chipped
        } else if spacing >= 0.25 {
            Cond::Dirty
        } else {
            Cond::Clean
        }
    }
    fn color(self) -> Color32 {
        match self {
            Cond::Clean => Color32::from_rgb(90, 200, 120),
            Cond::Dirty => Color32::from_rgb(220, 190, 70),
            Cond::Clogged => Color32::from_rgb(225, 70, 60),
            Cond::Chipped => Color32::from_rgb(235, 130, 50),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Cond::Clean => tr(k::deck::CLEAN),
            Cond::Dirty => tr(k::deck::DIRTY),
            Cond::Clogged => tr(k::deck::CLOGGED),
            Cond::Chipped => tr(k::deck::CHIPPED),
        }
    }
}

/// The rows of drum `d` (0 records, 1 plays), head `h` (0 A, 1 B).
pub fn head_rows(d: usize, h: usize) -> [&'static str; 3] {
    let i = (d * 2 + h) * 3;
    [ROWS[i], ROWS[i + 1], ROWS[i + 2]]
}

fn get(f: &FxInstance, name: &str) -> f64 {
    f.get_param(name).unwrap_or(0.0)
}

/// The condition of a head as the passport says it.
pub fn cond_of(f: &FxInstance, d: usize, h: usize) -> Cond {
    let [s, g, o] = head_rows(d, h);
    Cond::of(get(f, s), get(f, g), get(f, o))
}

/// A row of selectable buttons writing a choice/flag row's value.
fn switch(ui: &mut egui::Ui, f: &mut FxInstance, row: &str, labels: &[&str], ch: &mut crate::widgets::ParamsChanged) {
    let cur = get(f, row).round() as usize;
    ui.horizontal(|ui| {
        for (i, l) in labels.iter().enumerate() {
            let r = ui.add(egui::Button::selectable(i == cur, *l).frame_when_inactive(true));
            crate::widgets::field_probe::note(&format!("deck.{row}.{i}"), r.rect);
            if r.clicked() && i != cur {
                f.set_param(row, i as f64);
                ch.base = true;
            }
        }
    });
}

fn flag(ui: &mut egui::Ui, f: &mut FxInstance, row: &str, label: &str, ch: &mut crate::widgets::ParamsChanged) {
    let mut on = get(f, row) >= 0.5;
    if ui.checkbox(&mut on, label).changed() {
        f.set_param(row, if on { 1.0 } else { 0.0 });
        ch.base = true;
    }
}

const DRUM_R: f32 = 44.0;

/// One drum: the cylinder, the tape wrapped over half of it, its heads. Returns the head clicked.
fn drum(ui: &mut egui::Ui, f: &FxInstance, d: usize, title: &str, sel: Option<(usize, usize)>) -> Option<usize> {
    let four = get(f, "heads") >= 0.5;
    let mut clicked = None;
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(title).small());
        let size = egui::vec2(2.0 * DRUM_R + 24.0, 2.0 * DRUM_R + 16.0);
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let c = rect.center();
        let p = ui.painter_at(rect);
        let vis = ui.visuals();
        p.circle_filled(c, DRUM_R, vis.extreme_bg_color);
        p.circle_stroke(c, DRUM_R, Stroke::new(1.5, vis.widgets.noninteractive.fg_stroke.color));
        // the tape: a band over the left half of the drum (the 180° wrap)
        let tape = Color32::from_rgb(110, 80, 50);
        let n = 24;
        let arc: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / n as f32;
                c + egui::vec2(a.cos(), a.sin()) * (DRUM_R + 4.0)
            })
            .collect();
        p.add(egui::Shape::line(arc, Stroke::new(5.0, tape)));
        p.circle_filled(c, 4.0, vis.widgets.noninteractive.fg_stroke.color);
        // the heads: A at 0°, B at 180°; a four-head drum adds the narrow pair beside them
        let mut heads: Vec<(usize, f32, f32)> = vec![(0, 0.0, 7.0), (1, std::f32::consts::PI, 7.0)];
        if four {
            heads.push((0, 0.35, 4.5));
            heads.push((1, std::f32::consts::PI + 0.35, 4.5));
        }
        for (h, a, sz) in heads {
            let pos = c + egui::vec2(a.cos(), a.sin()) * (DRUM_R - 2.0);
            let hr = egui::Rect::from_center_size(pos, egui::vec2(2.0 * sz, 2.0 * sz));
            let id = ui.id().with(("deck_head", d, h, sz as u32));
            let r = ui.interact(hr.expand(4.0), id, egui::Sense::click());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("deck.head.{d}.{h}")));
            crate::widgets::field_probe::note(&format!("deck.head.{d}.{h}"), hr);
            let col = cond_of(f, d, h).color();
            p.rect_filled(hr, 2.0, col);
            if sel == Some((d, h)) {
                p.rect_stroke(hr.expand(3.0), 3.0, Stroke::new(2.0, vis.selection.stroke.color), egui::StrokeKind::Outside);
            }
            p.text(
                pos - egui::vec2(a.cos(), a.sin()) * 14.0,
                egui::Align2::CENTER_CENTER,
                if h == 0 { "A" } else { "B" },
                egui::FontId::proportional(11.0),
                vis.text_color(),
            );
            if r.clicked() {
                clicked = Some(h);
            }
        }
    });
    clicked
}

/// The face, drawn above the grid. Its selection (which head) lives in egui's memory.
pub fn draw(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    mut bind: Option<&mut crate::widgets::CurveBind<'_>>,
    pubc: Option<&crate::widgets::PublishCtx>,
) -> crate::widgets::ParamsChanged {
    let mut ch = crate::widgets::ParamsChanged::default();
    let sel_id = ui.id().with("deck_sel");
    let mut sel: Option<(usize, usize)> = ui.data(|d| d.get_temp::<Option<(usize, usize)>>(sel_id)).flatten();
    let two = get(f, "decks") >= 0.5;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            if two {
                if let Some(h) = drum(ui, f, 0, tr(k::deck::REC_DECK), sel) {
                    sel = Some((0, h));
                }
                if let Some(h) = drum(ui, f, 1, tr(k::deck::PLAY_DECK), sel) {
                    sel = Some((1, h));
                }
            } else if let Some(h) = drum(ui, f, 0, tr(k::deck::ONE_DECK), sel) {
                sel = Some((0, h));
            }
            ui.vertical(|ui| {
                switch(ui, f, "standard", &[tr(k::deck::PAL), tr(k::deck::NTSC)], &mut ch);
                switch(ui, f, "speed", &[tr(k::deck::SP), tr(k::deck::LP), tr(k::deck::EP)], &mut ch);
                switch(ui, f, "decks", &[tr(k::deck::ONE), tr(k::deck::TWO)], &mut ch);
                switch(ui, f, "heads", &[tr(k::deck::HEADS2), tr(k::deck::HEADS4)], &mut ch);
                switch(ui, f, "transport", &[tr(k::deck::PLAY), tr(k::deck::STILL)], &mut ch);
                ui.horizontal(|ui| {
                    flag(ui, f, "tbc", tr(k::deck::TBC), &mut ch);
                    flag(ui, f, "doc", tr(k::deck::DOC), &mut ch);
                    flag(ui, f, "comb", tr(k::deck::COMB), &mut ch);
                });
            });
        });
        // the tracking knob: the shared field with its slider
        if let Some(spec) = f.param_specs().iter().find(|s| s.name == "tracking_um").cloned() {
            ch |= crate::widgets::param_row_bound_ex(ui, f, &spec, bind.as_deref_mut(), pubc, Some(160.0));
        }
        // with one deck there is no play drum: a selection there falls back to the drum it shares
        if !two && matches!(sel, Some((1, _))) {
            sel = sel.map(|(_, h)| (0, h));
        }
        match sel {
            Some((d, h)) => {
                let cur = cond_of(f, d, h);
                ui.horizontal(|ui| {
                    ui.label(crate::tf!(k::deck::HEAD, if h == 0 { "A" } else { "B" }));
                    for c in Cond::ALL {
                        let r = ui.add(egui::Button::selectable(c == cur, c.label()).frame_when_inactive(true));
                        crate::widgets::field_probe::note(&format!("deck.cond.{c:?}"), r.rect);
                        if r.clicked() && c != cur {
                            for (row, v) in head_rows(d, h).iter().zip(c.values()) {
                                f.set_param(row, v);
                            }
                            ch.base = true;
                        }
                    }
                });
                for row in head_rows(d, h) {
                    if let Some(spec) = f.param_specs().iter().find(|s| s.name == row).cloned() {
                        ch |= crate::widgets::param_row_bound(ui, f, &spec, bind.as_deref_mut(), pubc);
                    }
                }
            }
            None => {
                ui.label(egui::RichText::new(tr(k::deck::PICK_HEAD)).weak().small());
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(sel_id, sel));
    ch
}
