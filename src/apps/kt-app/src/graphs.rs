//! Rolling graphs of the performance panel (port of MultiLineGraphWidget v1): a ring of 180
//! samples (~90 s at a 500 ms tick), up to 5 series, the signature “pinned legends” —
//! labels on the left with bent leader lines to the lines and spreading out
//! on overlap, a marker dot for the current value with the number at the right edge.
//! Differences from v1 (deliberate, from reconnaissance): ONE common scale per graph
//! (in v1 each series had its own, with shared axis labels — the lines were
//! incomparable), an explicit empty state “—”, DPI-independent egui sizes.

use eframe::egui;
use std::collections::VecDeque;

pub const SAMPLES: usize = 180;

pub struct Series {
    pub name: &'static str,
    pub color: egui::Color32,
    pub data: VecDeque<f32>,
    /// Hard ceiling of the scale (for linear mode).
    pub soft_cap: f32,
}

pub enum Axis {
    /// 0..100% with a grid.
    Percent,
    /// Auto-scale: attack 0.35, decay 0.08 (port of adaptive scale v1).
    LinearAuto,
}

pub struct RollingGraph {
    pub title: crate::i18n::Key,
    pub series: Vec<Series>,
    pub axis: Axis,
    scale: f32,
    /// Grows on every `push`/`clear`: the picture is a function of (data, scale, rectangle), and
    /// the first two change only here (diagnostics-three-graphs D5).
    generation: u64,
    cache: Option<DrawCache>,
    /// How many times the picture was built — t1149 holds "a frame without a new sample
    /// rebuilds nothing" by it.
    pub(crate) builds: u64,
}

/// What one build produced: the lines of all series as ONE mesh and every label as a laid-out
/// galley. Drawing it again clones `Arc`s only — no vector, no string, no layout (invariant 12).
struct DrawCache {
    key: CacheKey,
    shapes: Vec<egui::Shape>,
}

#[derive(Clone, Copy, PartialEq)]
struct CacheKey {
    rect: egui::Rect,
    generation: u64,
    ppp: f32,
    /// The translated title's address: a language switch gives another `&'static str`.
    title: usize,
}

const LEGEND_W: f32 = 86.0;
const VALUE_W: f32 = 44.0;

impl RollingGraph {
    pub fn new(
        title: crate::i18n::Key,
        axis: Axis,
        series: Vec<(&'static str, egui::Color32, f32)>,
    ) -> Self {
        RollingGraph {
            title,
            axis,
            series: series
                .into_iter()
                .map(|(name, color, soft_cap)| Series {
                    name,
                    color,
                    data: VecDeque::with_capacity(SAMPLES),
                    soft_cap,
                })
                .collect(),
            scale: 1.0,
            generation: 0,
            cache: None,
            builds: 0,
        }
    }

    pub fn push(&mut self, values: &[f32]) {
        for (s, &v) in self.series.iter_mut().zip(values) {
            if s.data.len() >= SAMPLES {
                s.data.pop_front();
            }
            s.data.push_back(v.max(0.0));
        }
        // common scale: fast attack, slow decay (port of 0.35/0.08) — stepped once per SAMPLE,
        // as v1's 500 ms timer did. Stepping it per draw made its speed a function of the UI
        // frame rate (60 fps decayed fifteen times faster than the 250 ms idle tick) and the
        // picture a function of the frame count, which no cache can key.
        if let Axis::LinearAuto = self.axis {
            let peak =
                self.series.iter().flat_map(|s| s.data.iter().copied()).fold(1.0f32, f32::max);
            let hard = self.series.iter().map(|s| s.soft_cap).fold(1.0f32, f32::max);
            let target = (peak * 1.25).clamp(1.0, hard);
            let alpha = if target > self.scale { 0.35 } else { 0.08 };
            self.scale += (target - self.scale) * alpha;
        }
        self.generation += 1;
    }

    pub fn clear(&mut self) {
        for s in &mut self.series {
            s.data.clear();
        }
        self.scale = 1.0;
        self.generation += 1;
    }

    /// The scale the next picture is drawn with (t1149).
    #[cfg(test)]
    pub(crate) fn scale(&self) -> f32 {
        self.scale
    }

    /// Draw the title and the graph of `height` points under it; returns the whole rectangle.
    /// The picture is rebuilt only when a sample came, the rectangle moved or the language
    /// changed — otherwise the last build is painted again.
    pub fn draw(&mut self, ui: &mut egui::Ui, height: f32) -> egui::Rect {
        let title_h = ui.text_style_height(&egui::TextStyle::Small);
        let (outer, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), title_h + 2.0 + height),
            egui::Sense::hover(),
        );
        let key = CacheKey {
            rect: outer,
            generation: self.generation,
            ppp: ui.ctx().pixels_per_point(),
            title: crate::i18n::tr(self.title).as_ptr() as usize,
        };
        if self.cache.as_ref().map(|c| c.key) != Some(key) {
            self.rebuild(ui, key, title_h);
        }
        if let Some(c) = &self.cache {
            ui.painter_at(outer).extend(c.shapes.iter().cloned());
        }
        outer
    }

    #[allow(clippy::too_many_lines)]
    fn rebuild(&mut self, ui: &egui::Ui, key: CacheKey, title_h: f32) {
        self.builds += 1;
        let mut shapes = self.cache.take().map(|c| c.shapes).unwrap_or_default();
        shapes.clear();
        let ctx = ui.ctx().clone();
        let text = |shapes: &mut Vec<egui::Shape>,
                    pos: egui::Pos2,
                    align: egui::Align2,
                    s: String,
                    font: egui::FontId,
                    color: egui::Color32| {
            let g = ctx.fonts_mut(|f| f.layout_no_wrap(s, font, color));
            let at = align.anchor_size(pos, g.size()).min;
            shapes.push(egui::Shape::galley(at, g, color));
        };
        let outer = key.rect;
        text(
            &mut shapes,
            outer.left_top(),
            egui::Align2::LEFT_TOP,
            crate::i18n::tr(self.title).to_string(),
            egui::TextStyle::Small.resolve(ui.style()),
            ui.visuals().strong_text_color(),
        );
        let rect = egui::Rect::from_min_max(
            egui::pos2(outer.left(), outer.top() + title_h + 2.0),
            outer.max,
        );
        shapes.push(egui::Shape::rect_filled(rect, 3.0, egui::Color32::from_gray(16)));

        // graph zone: the legend column on the left, the value column on the right
        let g = egui::Rect::from_min_max(
            egui::pos2(rect.left() + LEGEND_W, rect.top() + 4.0),
            egui::pos2(rect.right() - VALUE_W, rect.bottom() - 4.0),
        );

        // empty state (in v1 it existed only in LinearAutoRange)
        if self.series.iter().all(|s| s.data.len() < 2) {
            text(
                &mut shapes,
                g.center(),
                egui::Align2::CENTER_CENTER,
                "—".to_string(),
                egui::FontId::proportional(14.0),
                egui::Color32::from_gray(90),
            );
            self.cache = Some(DrawCache { key, shapes });
            return;
        }

        let cap = match self.axis {
            Axis::Percent => 100.0,
            Axis::LinearAuto => self.scale.max(1.0),
        };

        // grid: top/middle/bottom + scale labels (port of 100%/50%/0%)
        let grid = egui::Color32::from_gray(30);
        for frac in [0.0f32, 0.5, 1.0] {
            let y = g.bottom() - frac * g.height();
            shapes.push(egui::Shape::line_segment(
                [egui::pos2(g.left(), y), egui::pos2(g.right(), y)],
                egui::Stroke::new(1.0, grid),
            ));
            let label = match self.axis {
                Axis::Percent => format!("{:.0}%", frac * 100.0),
                Axis::LinearAuto => fmt_short(cap * frac),
            };
            text(
                &mut shapes,
                egui::pos2(g.left() - 4.0, y),
                egui::Align2::RIGHT_CENTER,
                label,
                egui::FontId::proportional(8.0),
                egui::Color32::from_gray(110),
            );
        }

        let y_of = |v: f32| -> f32 {
            g.bottom() - (v / cap.max(1e-6)).clamp(0.0, 1.0) * (g.height() - 2.0)
        };

        // series lines — tessellated here, once, into one mesh with the feathering egui's own
        // tessellator gives `Shape::line` (one physical pixel)
        let feather = 1.0 / key.ppp.max(1e-3);
        let mut mesh = egui::Mesh::default();
        let mut path = egui::epaint::tessellator::Path::default();
        let mut pts: Vec<egui::Pos2> = Vec::with_capacity(SAMPLES);
        for s in &self.series {
            if s.data.len() < 2 {
                continue;
            }
            pts.clear();
            pts.extend(s.data.iter().enumerate().map(|(i, &v)| {
                let x = g.left() + (i as f32 / (SAMPLES - 1) as f32) * g.width();
                egui::pos2(x, y_of(v))
            }));
            path.clear();
            path.add_open_points(&pts);
            path.stroke_open(feather, &egui::epaint::PathStroke::new(1.4, s.color), &mut mesh);
        }
        shapes.push(egui::Shape::mesh(mesh));
        // marker of the current value at the right edge of the line + number (port)
        for s in &self.series {
            if s.data.len() < 2 {
                continue;
            }
            let last = *s.data.back().unwrap();
            let last_x = g.left() + ((s.data.len() - 1) as f32 / (SAMPLES - 1) as f32) * g.width();
            let last_y = y_of(last);
            shapes.push(egui::Shape::circle_filled(egui::pos2(last_x, last_y), 2.6, s.color));
            text(
                &mut shapes,
                egui::pos2(last_x + 5.0, last_y),
                egui::Align2::LEFT_CENTER,
                fmt_short(last),
                egui::FontId::proportional(9.0),
                s.color,
            );
        }

        // --- pinned legends (the v1 signature): label on the left at the line's level,
        // bent leader line, spreading out on overlap -----------------------
        struct Pin {
            idx: usize,
            source_y: f32,
            pinned_y: f32,
        }
        let min_gap = 12.0;
        let (top, bottom) = (g.top() + 7.0, g.bottom() - 7.0);
        let mut pins: Vec<Pin> = self
            .series
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.data.is_empty())
            .map(|(idx, s)| {
                let y = y_of(*s.data.front().unwrap()).clamp(top, bottom);
                Pin { idx, source_y: y, pinned_y: y }
            })
            .collect();
        pins.sort_by(|a, b| a.source_y.total_cmp(&b.source_y));
        for i in 1..pins.len() {
            pins[i].pinned_y = pins[i].pinned_y.max(pins[i - 1].pinned_y + min_gap);
        }
        if let Some(lastp) = pins.last()
            && lastp.pinned_y > bottom
        {
            let n = pins.len();
            pins[n - 1].pinned_y = bottom;
            for i in (0..n - 1).rev() {
                pins[i].pinned_y = pins[i].pinned_y.min(pins[i + 1].pinned_y - min_gap);
            }
            for pin in &mut pins {
                pin.pinned_y = pin.pinned_y.clamp(top, bottom);
            }
        }
        let elbow_x = g.left() - 5.0;
        let legend_right = g.left() - 30.0;
        for pin in &pins {
            let s = &self.series[pin.idx];
            let stroke = egui::Stroke::new(1.2, s.color.gamma_multiply(0.8));
            shapes.push(egui::Shape::line_segment(
                [egui::pos2(legend_right, pin.pinned_y), egui::pos2(elbow_x, pin.pinned_y)],
                stroke,
            ));
            shapes.push(egui::Shape::line_segment(
                [egui::pos2(elbow_x, pin.pinned_y), egui::pos2(g.left(), pin.source_y)],
                stroke,
            ));
            shapes.push(egui::Shape::circle_filled(egui::pos2(g.left(), pin.source_y), 2.0, s.color));
            shapes.push(egui::Shape::circle_filled(
                egui::pos2(rect.left() + 6.0, pin.pinned_y),
                2.6,
                s.color,
            ));
            text(
                &mut shapes,
                egui::pos2(rect.left() + 12.0, pin.pinned_y),
                egui::Align2::LEFT_CENTER,
                s.name.to_string(),
                egui::FontId::proportional(9.0),
                s.color,
            );
        }
        self.cache = Some(DrawCache { key, shapes });
    }
}

/// Short number format for labels: 1234 → “1.2k”.
fn fmt_short(v: f32) -> String {
    if v >= 10_000.0 {
        format!("{:.0}k", v / 1000.0)
    } else if v >= 1000.0 {
        format!("{:.1}k", v / 1000.0)
    } else if v >= 100.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1149 (diagnostics-three-graphs D5): a graph is drawn from its last sample's build.
    /// Repeated draws reuse it, a sample rebuilds once, a larger rectangle rebuilds once, and the
    /// auto-scale moves only on a sample — before the change it stepped on every draw.
    #[test]
    fn t1149_graph_draws_from_its_last_sample_build() {
        let g = RollingGraph::new(
            crate::i18n::k::perf::GRAPH_CHURN,
            Axis::LinearAuto,
            vec![("evict", egui::Color32::RED, 2000.0), ("miss", egui::Color32::GREEN, 2000.0)],
        );
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(480.0, 200.0))
            .build_ui_state(
                |ui, g: &mut (RollingGraph, f32)| {
                    let h = g.1;
                    g.0.draw(ui, h);
                },
                (g, 96.0),
            );
        for v in [10.0, 400.0, 900.0] {
            h.state_mut().0.push(&[v, v / 2.0]);
        }
        h.step();
        let built = h.state().0.builds;
        let scale = h.state().0.scale();
        assert!(built >= 1, "the graph was never built");
        for _ in 0..10 {
            h.step();
        }
        assert_eq!(h.state().0.builds, built, "a frame without a new sample rebuilt the graph");
        assert_eq!(h.state().0.scale(), scale, "the scale moved without a sample");
        // the lines are one mesh among the painted shapes, not a vector per series per frame
        let meshes = h
            .output()
            .shapes
            .iter()
            .filter(|c| matches!(c.shape, egui::Shape::Mesh(_)))
            .count();
        assert!(meshes >= 1, "the series lines are not painted as a mesh");

        h.state_mut().0.push(&[1500.0, 10.0]);
        h.step();
        h.step();
        assert_eq!(h.state().0.builds, built + 1, "a new sample must rebuild exactly once");
        assert!(h.state().0.scale() > scale, "a higher peak must raise the scale on the sample");

        h.state_mut().1 = 140.0;
        h.step();
        h.step();
        assert_eq!(h.state().0.builds, built + 2, "a larger rectangle must rebuild exactly once");
    }
}
