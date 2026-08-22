//! Rolling-графики перф-панели (порт MultiLineGraphWidget): кольцо 180
//! сэмплов, до 5 серий, легенда слева, два режима оси — процентная сетка
//! и linear auto-range с быстрой атакой/медленным спадом (0.35 / 0.08).

use eframe::egui;
use std::collections::VecDeque;

pub const SAMPLES: usize = 180;

pub struct Series {
    pub name: &'static str,
    pub color: egui::Color32,
    pub data: VecDeque<f32>,
    /// Мягкий потолок (для linear-режима).
    pub soft_cap: f32,
}

pub enum Axis {
    /// 0..100% с сеткой.
    Percent,
    /// Автомасштаб: атака 0.35, спад 0.08 (порт).
    LinearAuto,
}

pub struct RollingGraph {
    pub title: &'static str,
    pub series: Vec<Series>,
    pub axis: Axis,
    scale: f32,
}

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

    pub fn draw(&mut self, ui: &mut egui::Ui, height: f32) {
        ui.label(egui::RichText::new(self.title).small().strong());
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 3.0, egui::Color32::from_gray(16));

        // масштаб оси
        let target = match self.axis {
            Axis::Percent => 100.0,
            Axis::LinearAuto => {
                let peak =
                    self.series.iter().flat_map(|s| s.data.iter().copied()).fold(1.0f32, f32::max);
                // атака быстрая, спад медленный
                if peak > self.scale {
                    self.scale += (peak - self.scale) * 0.35;
                } else {
                    self.scale += (peak - self.scale) * 0.08;
                }
                self.scale.max(1.0)
            }
        };

        // сетка (25/50/75% или трети)
        for frac in [0.25f32, 0.5, 0.75] {
            let y = rect.bottom() - frac * rect.height();
            p.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                egui::Stroke::new(1.0, egui::Color32::from_gray(28)),
            );
        }

        for s in &self.series {
            if s.data.len() < 2 {
                continue;
            }
            let cap = match self.axis {
                Axis::Percent => target,
                Axis::LinearAuto => target.min(s.soft_cap.max(1.0)).max(target * 0.2),
            };
            let pts: Vec<egui::Pos2> = s
                .data
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    let x = rect.left() + (i as f32 / (SAMPLES - 1) as f32) * rect.width();
                    let y =
                        rect.bottom() - (v / cap.max(1e-6)).clamp(0.0, 1.0) * (rect.height() - 2.0);
                    egui::pos2(x, y)
                })
                .collect();
            p.add(egui::Shape::line(pts, egui::Stroke::new(1.2, s.color)));
        }
        // легенда слева (закреплённая, порт pinned legends)
        let mut y = rect.top() + 3.0;
        for s in &self.series {
            let last = s.data.back().copied().unwrap_or(0.0);
            p.text(
                egui::pos2(rect.left() + 4.0, y),
                egui::Align2::LEFT_TOP,
                format!("{} {:.0}", s.name, last),
                egui::FontId::proportional(9.0),
                s.color,
            );
            y += 11.0;
        }
    }
}
