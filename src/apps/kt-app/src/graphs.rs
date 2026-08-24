//! Rolling-графики перф-панели (порт MultiLineGraphWidget v1): кольцо 180
//! сэмплов (~90 с при тике 500 мс), до 5 серий, фирменные «pinned legends» —
//! подписи слева с коленчатыми выносками к линиям и раздвижкой при
//! наложении, маркер-точка текущего значения с числом у правого края.
//! Отличия от v1 (осознанные, по разведке): ОДНА общая шкала на график
//! (в v1 у каждой серии была своя при общих подписях оси — линии были
//! несравнимы), явное пустое состояние «—», DPI-независимые размеры egui.

use eframe::egui;
use std::collections::VecDeque;

pub const SAMPLES: usize = 180;

pub struct Series {
    pub name: &'static str,
    pub color: egui::Color32,
    pub data: VecDeque<f32>,
    /// Жёсткий потолок шкалы (для linear-режима).
    pub soft_cap: f32,
}

pub enum Axis {
    /// 0..100% с сеткой.
    Percent,
    /// Автомасштаб: атака 0.35, спад 0.08 (порт adaptive scale v1).
    LinearAuto,
}

pub struct RollingGraph {
    pub title: &'static str,
    pub series: Vec<Series>,
    pub axis: Axis,
    scale: f32,
}

const LEGEND_W: f32 = 86.0;
const VALUE_W: f32 = 44.0;

impl RollingGraph {
    pub fn new(
        title: &'static str,
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
        }
    }

    pub fn push(&mut self, values: &[f32]) {
        for (s, &v) in self.series.iter_mut().zip(values) {
            if s.data.len() >= SAMPLES {
                s.data.pop_front();
            }
            s.data.push_back(v.max(0.0));
        }
    }

    pub fn clear(&mut self) {
        for s in &mut self.series {
            s.data.clear();
        }
        self.scale = 1.0;
    }

    #[allow(clippy::too_many_lines)]
    pub fn draw(&mut self, ui: &mut egui::Ui, height: f32) {
        ui.label(egui::RichText::new(self.title).small().strong());
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 3.0, egui::Color32::from_gray(16));

        // зона графика: слева колонка легенд, справа колонка значений
        let g = egui::Rect::from_min_max(
            egui::pos2(rect.left() + LEGEND_W, rect.top() + 4.0),
            egui::pos2(rect.right() - VALUE_W, rect.bottom() - 4.0),
        );

        // пустое состояние (в v1 было только у LinearAutoRange)
        if self.series.iter().all(|s| s.data.len() < 2) {
            p.text(
                g.center(),
                egui::Align2::CENTER_CENTER,
                "—",
                egui::FontId::proportional(14.0),
                egui::Color32::from_gray(90),
            );
            return;
        }

        // общая шкала: атака быстрая, спад медленный (порт 0.35/0.08)
        let cap = match self.axis {
            Axis::Percent => 100.0,
            Axis::LinearAuto => {
                let peak =
                    self.series.iter().flat_map(|s| s.data.iter().copied()).fold(1.0f32, f32::max);
                let hard = self.series.iter().map(|s| s.soft_cap).fold(1.0f32, f32::max);
                let target = (peak * 1.25).clamp(1.0, hard);
                let alpha = if target > self.scale { 0.35 } else { 0.08 };
                self.scale += (target - self.scale) * alpha;
                self.scale.max(1.0)
            }
        };

        // сетка: верх/середина/низ + подписи шкалы (порт 100%/50%/0%)
        let grid = egui::Color32::from_gray(30);
        for frac in [0.0f32, 0.5, 1.0] {
            let y = g.bottom() - frac * g.height();
            p.line_segment(
                [egui::pos2(g.left(), y), egui::pos2(g.right(), y)],
                egui::Stroke::new(1.0, grid),
            );
            let label = match self.axis {
                Axis::Percent => format!("{:.0}%", frac * 100.0),
                Axis::LinearAuto => fmt_short(cap * frac),
            };
            p.text(
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

        // линии серий
        for s in &self.series {
            if s.data.len() < 2 {
                continue;
            }
            let pts: Vec<egui::Pos2> = s
                .data
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    let x = g.left() + (i as f32 / (SAMPLES - 1) as f32) * g.width();
                    egui::pos2(x, y_of(v))
                })
                .collect();
            p.add(egui::Shape::line(pts, egui::Stroke::new(1.4, s.color)));
            // маркер текущего значения у правого края линии + число (порт)
            let last = *s.data.back().unwrap();
            let last_x = g.left()
                + ((s.data.len() - 1) as f32 / (SAMPLES - 1) as f32) * g.width();
            let last_y = y_of(last);
            p.circle_filled(egui::pos2(last_x, last_y), 2.6, s.color);
            p.text(
                egui::pos2(last_x + 5.0, last_y),
                egui::Align2::LEFT_CENTER,
                fmt_short(last),
                egui::FontId::proportional(9.0),
                s.color,
            );
        }

        // --- pinned legends (фирменное v1): подпись слева на уровне линии,
        // коленчатая выноска, раздвижка при наложении -----------------------
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
            p.line_segment(
                [egui::pos2(legend_right, pin.pinned_y), egui::pos2(elbow_x, pin.pinned_y)],
                stroke,
            );
            p.line_segment(
                [egui::pos2(elbow_x, pin.pinned_y), egui::pos2(g.left(), pin.source_y)],
                stroke,
            );
            p.circle_filled(egui::pos2(g.left(), pin.source_y), 2.0, s.color);
            p.circle_filled(egui::pos2(rect.left() + 6.0, pin.pinned_y), 2.6, s.color);
            p.text(
                egui::pos2(rect.left() + 12.0, pin.pinned_y),
                egui::Align2::LEFT_CENTER,
                s.name,
                egui::FontId::proportional(9.0),
                s.color,
            );
        }
    }
}

/// Короткий формат чисел для подписей: 1234 → «1.2k».
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
