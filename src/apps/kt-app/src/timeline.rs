//! Таймлайн: отрисовка и интеракции. Паритет со старым Qt-таймлайном
//! (см. docs/porting-notes.md и план UI-паритета): маркеры, регион,
//! volume rail, стерео-волноформы, цвет клипа из хэша пути, офлайн-тинт,
//! edge-autoscroll, контекст-меню, снап к маркерам/региону/клипам.

use crate::app::KtApp;
use eframe::egui;
use kt_cache::BurstKind;
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, Tick};
use std::collections::HashMap;

pub const STRIPES_H: f32 = 11.0; // 4+1+4+2: глобальные полоски кэша
pub const RULER_H: f32 = 22.0 + STRIPES_H;
pub const GUTTER_W: f32 = 52.0;
pub const EDGE_GRAB_PX: f32 = 7.0;
pub const SNAP_PX: f64 = 10.0;
pub const REGION_EDGE_PX: f32 = 8.0;
pub const VOLUME_RAIL_W: f32 = 9.0;
/// ±6.02 dB на весь ход рейла: gain = 2^((0.5−t)·2)  (порт старого).
pub const RAIL_DB_SPAN: f32 = 12.0412;

#[derive(Clone, Copy, PartialEq)]
pub enum DragMode {
    Move,
    TrimStart,
    TrimEnd,
}

pub struct DragState {
    pub id: ClipId,
    pub mode: DragMode,
    /// Тики от начала клипа до точки захвата (для Move).
    pub grab_dt: i64,
    pub orig: ClipEvent,
    /// Текущее (превью) положение.
    pub cur_start: i64,
    pub cur_end: i64,
    pub cur_lane: u32,
    pub moved: bool,
}

/// Все виды перетаскиваний на таймлайне.
pub enum Drag {
    Clip(DragState),
    /// Перетаскивание заголовка дорожки (реордер, п.7).
    Lane {
        kind: ClipKind,
        from: u32,
        cur: u32,
    },
    /// Скраб курсора (захват треугольника playhead в руллере).
    Scrub,
    /// Выделение региона по пустому месту.
    Region {
        anchor: i64,
    },
    /// Перетаскивание границы региона (true = start).
    RegionEdge {
        start_edge: bool,
    },
    /// Перетаскивание маркера: (исходный тик, текущий).
    Marker {
        orig: i64,
        cur: i64,
    },
    /// Volume rail аудио-клипа: live-громкость, SetGain на отпускании.
    Volume {
        id: ClipId,
        moved: bool,
    },
}

/// Цель контекст-меню, захваченная на right-click.
#[derive(Clone, Copy, PartialEq)]
pub enum CtxTarget {
    Clip(ClipId),
    Marker(i64),
    /// Правый клик у границы региона (тик границы).
    RegionEdge(i64),
    /// Правый клик по заголовку дорожки (п.8).
    Lane {
        kind: ClipKind,
        lane: u32,
    },
    Empty,
}

/// Цвет клипа из хэша пути (порт colorFromPath: r=80+h%120 ...).
pub fn clip_color(path: &str, kind: ClipKind) -> egui::Color32 {
    let mut h: u64 = 1469598103934665603;
    let suffix = match kind {
        ClipKind::Video => ":v",
        ClipKind::Audio => ":a",
    };
    for b in path.bytes().chain(suffix.bytes()) {
        h = h.wrapping_mul(131).wrapping_add(b as u64);
    }
    egui::Color32::from_rgb(
        (80 + h % 120) as u8,
        (80 + (h / 7) % 120) as u8,
        (80 + (h / 17) % 120) as u8,
    )
}

impl KtApp {
    /// Магнитные цели: границы чужих клипов, маркеры, границы региона,
    /// playhead, ноль (порт snapTimeMs).
    pub fn snap_targets(&self, exclude: Option<ClipId>) -> Vec<i64> {
        let p = self.model.project();
        let mut t: Vec<i64> = vec![0, self.player.transport.playhead().0];
        for c in &p.clips {
            if Some(c.id) != exclude {
                t.push(c.start.0);
                t.push(c.start.0 + c.duration.0);
            }
        }
        for m in &p.markers {
            t.push(m.0);
        }
        if let Some(r) = p.region {
            t.push(r.start.0);
            t.push(r.end.0);
        }
        t
    }

    pub fn snap(&mut self, tick: i64, exclude: Option<ClipId>) -> i64 {
        // v1 snapTimeMs: порог 10 px, но не меньше 12 мс и не больше 100 мс
        let threshold = ((SNAP_PX * self.ticks_per_px) as i64).clamp(1_080, 9_000);
        let mut best = tick;
        let mut best_d = threshold + 1;
        for target in self.snap_targets(exclude) {
            let d = (target - tick).abs();
            if d < best_d {
                best_d = d;
                best = target;
            }
        }
        if best != tick {
            self.snap_line = Some(best);
        }
        best
    }

    pub fn lane_counts(&self) -> (u32, u32) {
        let p = self.model.project();
        (p.video_lanes.max(1), p.audio_lanes.max(1))
    }

    /// Слои (клипы), активные на курсоре — для overlay-строки.
    pub fn layers_at_cursor(&self) -> (usize, usize) {
        let t = self.player.transport.playhead();
        let mut v = 0;
        let mut a = 0;
        for c in &self.model.project().clips {
            if c.start <= t && t < c.start + c.duration {
                match c.kind {
                    ClipKind::Video => v += 1,
                    ClipKind::Audio => a += 1,
                }
            }
        }
        (v, a)
    }

    #[allow(clippy::too_many_lines)]
    pub fn draw_timeline(&mut self, ui: &mut egui::Ui) {
        // один проход coverage-опросов на кадр (кэш по epoch, п.1)
        let coverage_snapshot: HashMap<ClipId, Vec<std::ops::Range<i64>>> =
            self.coverage_runs().clone();
        let full = ui.available_rect_before_wrap();
        let gutter =
            egui::Rect::from_min_max(full.min, egui::pos2(full.left() + GUTTER_W, full.bottom()));
        let rect = egui::Rect::from_min_max(egui::pos2(gutter.right(), full.top()), full.max);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 0.0, egui::Color32::from_gray(22));
        painter.rect_filled(gutter, 0.0, egui::Color32::from_gray(28));
        // заголовки дорожек интерактивны: драг = реордер (п.7), ПКМ = меню
        // удаления/вставки (п.8)
        let gutter_resp =
            ui.interact(gutter, ui.id().with("lane-gutter"), egui::Sense::click_and_drag());

        // ---- зум/пан + edge-autoscroll при драге --------------------------
        let mut view_start = self.view_start;
        let mut tpp = self.ticks_per_px;
        if let Some(pos) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            let zoom = ui.input(|i| i.zoom_delta());
            let ctrl = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
            if ctrl && scroll.y != 0.0 || zoom != 1.0 {
                // зум: Ctrl+колесо или пинч, якорь под курсором (порт старого)
                let factor = if zoom != 1.0 {
                    1.0 / zoom as f64
                } else if scroll.y > 0.0 {
                    0.8
                } else {
                    1.25
                };
                let anchor = view_start + (pos.x - rect.left()) as f64 * tpp;
                tpp = (tpp * factor).clamp(30.0, 90_000.0);
                view_start = anchor - (pos.x - rect.left()) as f64 * tpp;
            } else if scroll.x != 0.0 || scroll.y != 0.0 {
                // голое колесо (любая ось) = скролл по таймлайну (порт старого)
                let delta = if scroll.x.abs() > scroll.y.abs() { scroll.x } else { scroll.y };
                view_start -= delta as f64 * tpp * 2.5;
            }
        }
        // Зоны «прыжка» у краёв (порт): heat = близость к краю [-1..1].
        let edge_margin = (rect.width() / 8.0).max(40.0);
        let edge_heat = |x: f32| -> f32 {
            if x < rect.left() + edge_margin {
                -((rect.left() + edge_margin - x) / edge_margin).clamp(0.0, 1.0)
            } else if x > rect.right() - edge_margin {
                ((x - (rect.right() - edge_margin)) / edge_margin).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        // при драге — плавный autoscroll (heat^4.15, порт)
        if self.drag.is_some()
            && let Some(pos) = response.hover_pos()
        {
            let heat = edge_heat(pos.x);
            if heat != 0.0 {
                let ms = (heat.abs().powf(4.15) * 0.18 * 240.0).clamp(1.0, 42.0) as f64;
                view_start += heat.signum() as f64 * ms * 90.0;
            }
        }
        // ensureCursorVisible при воспроизведении
        if self.player.transport.is_playing() && self.drag.is_none() {
            let ph = self.player.transport.playhead().0 as f64;
            let vis = rect.width() as f64 * tpp;
            if ph < view_start + vis * 0.02 || ph > view_start + vis * 0.85 {
                view_start = ph - vis * 0.15;
            }
        }
        view_start = view_start.max(-(tpp * 40.0));
        self.view_start = view_start;
        self.ticks_per_px = tpp;
        let x_to_tick =
            move |x: f32| -> i64 { (view_start + (x - rect.left()) as f64 * tpp) as i64 };
        let tick_to_x =
            move |t: i64| -> f32 { rect.left() + ((t as f64 - view_start) / tpp) as f32 };

        // ---- глобальные полоски кэша (порт: cyan=аудио, green=видео) --------
        {
            let y0 = rect.top() + 1.0;
            let cyan = egui::Color32::from_rgb(114, 214, 255);
            let green = egui::Color32::from_rgb(72, 168, 96);
            // фоновые подложки
            for (row, _) in [(0.0f32, cyan), (5.0, green)] {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(rect.left(), y0 + row),
                        egui::pos2(rect.right(), y0 + row + 4.0),
                    ),
                    0.0,
                    egui::Color32::from_gray(30),
                );
            }
            let q = self.player.preview_quality();
            for c in &self.model.project().clips {
                let (kind, row, col) = match c.kind {
                    ClipKind::Audio => (BurstKind::Audio, 0.0f32, cyan),
                    ClipKind::Video => (BurstKind::video(Some(q)), 5.0, green),
                };
                let (b0, b1) = match c.kind {
                    ClipKind::Audio => {
                        let f0 = c.source_in.to_audio_frames();
                        let f1 = (c.source_in + c.duration).to_audio_frames();
                        (
                            f0.div_euclid(AUDIO_BURST_FRAMES as i64),
                            (f1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1,
                        )
                    }
                    ClipKind::Video => (c.source_in.0, c.source_in.0 + c.duration.0),
                };
                let _ = (kind, b0, b1);
                for run in coverage_snapshot.get(&c.id).cloned().unwrap_or_default() {
                    let (t0, t1) = match c.kind {
                        ClipKind::Audio => (
                            c.start.0
                                + Tick::from_audio_frames(
                                    run.start * AUDIO_BURST_FRAMES as i64
                                        - c.source_in.to_audio_frames(),
                                )
                                .0,
                            c.start.0
                                + Tick::from_audio_frames(
                                    run.end * AUDIO_BURST_FRAMES as i64
                                        - c.source_in.to_audio_frames(),
                                )
                                .0,
                        ),
                        ClipKind::Video => (
                            c.start.0 + run.start - c.source_in.0,
                            c.start.0 + run.end - c.source_in.0,
                        ),
                    };
                    let x0 = tick_to_x(t0).max(rect.left());
                    let x1 = tick_to_x(t1).min(rect.right());
                    if x1 > x0 {
                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(x0, y0 + row),
                                egui::pos2(x1, y0 + row + 4.0),
                            ),
                            0.0,
                            col,
                        );
                    }
                }
            }
            // янтарная «голова» loop-региона поверх видео-полоски (порт head)
            if let Some(r) = self.model.project().region
                && self.model.project().loop_enabled
            {
                let head_end = (r.start.0 + 90_000 / 2).min(r.end.0);
                let x0 = tick_to_x(r.start.0).max(rect.left());
                let x1 = tick_to_x(head_end).min(rect.right());
                if x1 > x0 {
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x0, y0 + 5.0),
                            egui::pos2(x1, y0 + 9.0),
                        ),
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(255, 210, 72, 150),
                    );
                }
            }
        }

        // ---- линейка --------------------------------------------------------
        let sec_px = (90_000.0 / tpp) as f32;
        let label_every = (70.0 / sec_px).ceil().max(1.0) as i64;
        for s in (x_to_tick(rect.left()) / 90_000).max(0)..=(x_to_tick(rect.right()) / 90_000 + 1) {
            let x = tick_to_x(s * 90_000);
            let major = s % label_every == 0;
            painter.line_segment(
                [
                    egui::pos2(x, rect.top() + STRIPES_H),
                    egui::pos2(x, rect.top() + if major { RULER_H } else { STRIPES_H + 7.0 }),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
            );
            if major {
                painter.text(
                    egui::pos2(x + 4.0, rect.top() + STRIPES_H + 3.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}:{:02}", s / 60, s % 60),
                    egui::FontId::proportional(10.5),
                    egui::Color32::from_gray(150),
                );
            }
        }
        painter.line_segment(
            [
                egui::pos2(full.left(), rect.top() + RULER_H),
                egui::pos2(full.right(), rect.top() + RULER_H),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_gray(45)),
        );

        // ---- дорожки ---------------------------------------------------------
        let (video_lanes, audio_lanes) = self.lane_counts();
        let lanes_total = (video_lanes + audio_lanes).max(1);
        let lane_h = ((rect.height() - RULER_H - 6.0) / lanes_total as f32).clamp(30.0, 84.0);
        let lane_top = |kind: ClipKind, lane: u32| -> f32 {
            let idx = match kind {
                ClipKind::Video => lane,
                ClipKind::Audio => video_lanes + lane,
            };
            rect.top() + RULER_H + 3.0 + idx as f32 * lane_h
        };
        for i in 0..lanes_total {
            let y = rect.top() + RULER_H + 3.0 + i as f32 * lane_h;
            let lane_rect = egui::Rect::from_min_max(
                egui::pos2(rect.left(), y),
                egui::pos2(rect.right(), y + lane_h - 2.0),
            );
            let is_video = i < video_lanes;
            // фоны дорожек в тонах старого: видео синеватый, аудио тёплый
            painter.rect_filled(
                lane_rect,
                0.0,
                if is_video {
                    egui::Color32::from_rgb(30, 36, 44)
                } else {
                    egui::Color32::from_rgb(38, 32, 32)
                },
            );
            let label =
                if is_video { format!("V{}", i + 1) } else { format!("A{}", i - video_lanes + 1) };
            painter.text(
                egui::pos2(gutter.center().x, y + lane_h * 0.5),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(12.0),
                egui::Color32::from_gray(140),
            );
        }

        // ---- дорожки: реордер драгом и меню (пп. 7-8) ----------------------
        {
            let lane_at_y = |y: f32| -> Option<(ClipKind, u32)> {
                let rel = y - (rect.top() + RULER_H + 3.0);
                if rel < 0.0 {
                    return None;
                }
                let idx = (rel / lane_h).floor() as u32;
                if idx < video_lanes {
                    Some((ClipKind::Video, idx))
                } else if idx < lanes_total {
                    Some((ClipKind::Audio, idx - video_lanes))
                } else {
                    None
                }
            };
            if gutter_resp.drag_started()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some((kind, lane)) = lane_at_y(pos.y)
            {
                self.drag = Some(Drag::Lane { kind, from: lane, cur: lane });
            }
            if gutter_resp.dragged()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some(Drag::Lane { kind, from, .. }) = self.drag
                && let Some((k2, cur)) = lane_at_y(pos.y)
                && k2 == kind
            {
                self.drag = Some(Drag::Lane { kind, from, cur });
            }
            if gutter_resp.drag_stopped()
                && let Some(Drag::Lane { kind, from, cur }) = self.drag.take()
                && from != cur
            {
                self.reorder_lane(kind, from, cur);
            }
            // индикатор перетаскиваемой дорожки
            if let Some(Drag::Lane { kind, cur, .. }) = self.drag {
                let y = lane_top(kind, cur);
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(gutter.left(), y),
                        egui::pos2(rect.right(), y + lane_h - 2.0),
                    ),
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(120, 160, 255, 26),
                );
            }
            // ПКМ по заголовку — меню дорожки
            if gutter_resp.secondary_clicked()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some((kind, lane)) = lane_at_y(pos.y)
            {
                self.ctx_target = Some(CtxTarget::Lane { kind, lane });
            }
            gutter_resp.context_menu(|ui| {
                if let Some(CtxTarget::Lane { kind, lane }) = self.ctx_target {
                    let n = self
                        .model
                        .project()
                        .clips
                        .iter()
                        .filter(|c| c.kind == kind && c.lane == lane)
                        .count();
                    let name = match kind {
                        ClipKind::Video => format!("V{}", lane + 1),
                        ClipKind::Audio => format!("A{}", lane + 1),
                    };
                    ui.label(egui::RichText::new(format!("Дорожка {name}")).strong());
                    let del_label = if n > 0 {
                        format!("Удалить дорожку ({n} событ.)")
                    } else {
                        "Удалить дорожку".into()
                    };
                    let lanes_of_kind = match kind {
                        ClipKind::Video => video_lanes,
                        ClipKind::Audio => audio_lanes,
                    };
                    if ui.add_enabled(lanes_of_kind > 1, egui::Button::new(del_label)).clicked() {
                        self.delete_lane(kind, lane);
                        ui.close();
                    }
                    if ui.button("Вставить дорожку выше").clicked() {
                        self.apply(Command::InsertLane { kind, at: lane });
                        ui.close();
                    }
                    if ui.button("Вставить дорожку ниже").clicked() {
                        self.apply(Command::InsertLane { kind, at: lane + 1 });
                        ui.close();
                    }
                }
            });
        }

        // ---- регион: заливка ---------------------------------------------------
        if let Some(r) = self.model.project().region {
            let x0 = tick_to_x(r.start.0).max(rect.left());
            let x1 = tick_to_x(r.end.0).min(rect.right());
            if x1 > x0 {
                // v1: центр альфа 14, боковые градиенты к краям альфа 58,
                // ширина фейда min(56, w/2)
                let base = |a: u8| egui::Color32::from_rgba_unmultiplied(84, 124, 255, a);
                let fade_w = ((x1 - x0) / 2.0).clamp(1.0, 56.0);
                let grad_h = |xa: f32, xb: f32, ca: egui::Color32, cb: egui::Color32| {
                    let mut mesh = egui::Mesh::default();
                    let i = mesh.vertices.len() as u32;
                    for (pos, col) in [
                        (egui::pos2(xa, rect.top()), ca),
                        (egui::pos2(xb, rect.top()), cb),
                        (egui::pos2(xb, rect.bottom()), cb),
                        (egui::pos2(xa, rect.bottom()), ca),
                    ] {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos,
                            uv: egui::epaint::WHITE_UV,
                            color: col,
                        });
                    }
                    mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                    painter.add(egui::Shape::mesh(mesh));
                };
                grad_h(x0, x0 + fade_w, base(58), base(14));
                if x1 - fade_w > x0 + fade_w {
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x0 + fade_w, rect.top()),
                            egui::pos2(x1 - fade_w, rect.bottom()),
                        ),
                        0.0,
                        base(14),
                    );
                }
                grad_h(x1, x1 - fade_w, base(58), base(14));
                for (edge_t, x) in
                    [(r.start.0, tick_to_x(r.start.0)), (r.end.0, tick_to_x(r.end.0))]
                {
                    let _ = edge_t;
                    if x >= rect.left() && x <= rect.right() {
                        painter.line_segment(
                            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(102, 152, 255)),
                        );
                    }
                }
            }
        }

        // ---- хит-тесты --------------------------------------------------------
        let pointer = response.interact_pointer_pos();
        let hover = response.hover_pos();
        self.snap_line = None;

        let marker_at = |app: &KtApp, pos: egui::Pos2| -> Option<i64> {
            if pos.y > rect.top() + RULER_H {
                return None;
            }
            app.model
                .project()
                .markers
                .iter()
                .map(|m| m.0)
                .min_by_key(|m| (tick_to_x(*m) - pos.x).abs() as i64)
                .filter(|m| (tick_to_x(*m) - pos.x).abs() <= EDGE_GRAB_PX)
        };
        // границы региона хватаются на ВСЕЙ высоте (п.14): линии рисуются
        // на всю высоту и должны быть кликабельны везде
        let region_edge_at = |app: &KtApp, pos: egui::Pos2| -> Option<bool> {
            let r = app.model.project().region?;
            if (tick_to_x(r.start.0) - pos.x).abs() <= REGION_EDGE_PX {
                Some(true)
            } else if (tick_to_x(r.end.0) - pos.x).abs() <= REGION_EDGE_PX {
                Some(false)
            } else {
                None
            }
        };
        #[derive(PartialEq)]
        enum ClipZone {
            Body,
            TrimStart,
            TrimEnd,
            VolumeRail,
        }
        let clip_at = |app: &KtApp, pos: egui::Pos2| -> Option<(ClipId, ClipZone)> {
            for c in app.model.project().clips.iter().rev() {
                let top = lane_top(c.kind, c.lane);
                if pos.y < top || pos.y > top + lane_h - 2.0 {
                    continue;
                }
                let x0 = tick_to_x(c.start.0);
                let x1 = tick_to_x(c.start.0 + c.duration.0);
                if pos.x < x0 - 2.0 || pos.x > x1 + 2.0 {
                    continue;
                }
                let zone = if (pos.x - x0).abs() <= EDGE_GRAB_PX {
                    ClipZone::TrimStart
                } else if (pos.x - x1).abs() <= EDGE_GRAB_PX {
                    ClipZone::TrimEnd
                } else if c.kind == ClipKind::Audio
                    && x1 - x0 > 40.0
                    && pos.x >= x0 + EDGE_GRAB_PX + 3.0
                    && pos.x <= x0 + EDGE_GRAB_PX + 3.0 + VOLUME_RAIL_W
                {
                    ClipZone::VolumeRail
                } else {
                    ClipZone::Body
                };
                return Some((c.id, zone));
            }
            None
        };

        // курсор-иконка
        if self.drag.is_none()
            && let Some(pos) = hover
        {
            if marker_at(self, pos).is_some() || region_edge_at(self, pos).is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            } else if pos.y > rect.top() + RULER_H {
                match clip_at(self, pos) {
                    Some((_, ClipZone::TrimStart)) | Some((_, ClipZone::TrimEnd)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal)
                    }
                    Some((_, ClipZone::VolumeRail)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand)
                    }
                    Some((_, ClipZone::Body)) => ui.ctx().set_cursor_icon(egui::CursorIcon::Grab),
                    None => {}
                }
            }
        }

        // right-click: захват цели контекст-меню
        if response.secondary_clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.ctx_target = Some(if let Some(m) = marker_at(self, pos) {
                CtxTarget::Marker(m)
            } else if let Some(start_edge) = region_edge_at(self, pos) {
                let r = self.model.project().region.unwrap();
                CtxTarget::RegionEdge(if start_edge { r.start.0 } else { r.end.0 })
            } else if let Some((id, _)) = clip_at(self, pos) {
                self.selected = Some(id);
                CtxTarget::Clip(id)
            } else {
                CtxTarget::Empty
            });
        }

        // ---- press ------------------------------------------------------------
        // Зоны определяем по точке НАЖАТИЯ (press_origin), а не по текущему
        // указателю: drag_started приходит после сдвига в несколько px, и
        // указатель успевал выползти из узкой трим-зоны — трим превращался
        // в перенос клипа (п.28 «не могу изменить длину события»).
        let press_pos = ui.input(|i| i.pointer.press_origin());
        if (response.drag_started() || response.clicked())
            && let Some(pos) = press_pos.or(pointer)
        {
            if let Some(m) = marker_at(self, pos) {
                if response.drag_started() {
                    self.drag = Some(Drag::Marker { orig: m, cur: m });
                }
            } else if let Some(start_edge) = region_edge_at(self, pos) {
                if response.drag_started() {
                    self.drag = Some(Drag::RegionEdge { start_edge });
                }
            } else if pos.y <= rect.top() + RULER_H {
                // руллер: захват треугольника playhead = скраб; клик =
                // постановка курсора (снап к маркерам, п.29); драг =
                // выделение региона (решение пользователя, п.8)
                let head_x = tick_to_x(self.player.transport.playhead().0);
                if response.drag_started() && (pos.x - head_x).abs() <= 8.0 {
                    self.drag = Some(Drag::Scrub);
                } else if response.drag_started() {
                    let anchor = x_to_tick(pos.x).max(0);
                    self.drag = Some(Drag::Region { anchor });
                } else {
                    let t = self.snap(x_to_tick(pos.x).max(0), None);
                    self.player.seek(Tick(t.max(0)));
                }
            } else if let Some((id, zone)) = clip_at(self, pos) {
                self.selected = Some(id);
                // клик по СОБЫТИЮ тоже ставит курсор (v1, п.7)
                if response.clicked() {
                    let t = self.snap(x_to_tick(pos.x).max(0), None);
                    self.player.seek(Tick(t.max(0)));
                }
                if response.double_clicked() {
                    // регион на span всей link-группы (порт double-click)
                    let group = self.model.clip(id).and_then(|c| c.link_group);
                    let members: Vec<&ClipEvent> = self
                        .model
                        .project()
                        .clips
                        .iter()
                        .filter(|c| c.id == id || (group.is_some() && c.link_group == group))
                        .collect();
                    let start = members.iter().map(|c| c.start).min().unwrap();
                    let end = members.iter().map(|c| c.start + c.duration).max().unwrap();
                    self.model.set_region(Region::normalized(start, end));
                    self.on_region_changed();
                } else if response.drag_started() {
                    match zone {
                        ClipZone::VolumeRail => {
                            self.drag = Some(Drag::Volume { id, moved: false });
                        }
                        zone => {
                            if let Some(orig) = self.model.clip(id).cloned() {
                                let mode = match zone {
                                    ClipZone::TrimStart => DragMode::TrimStart,
                                    ClipZone::TrimEnd => DragMode::TrimEnd,
                                    _ => DragMode::Move,
                                };
                                self.drag = Some(Drag::Clip(DragState {
                                    id,
                                    mode,
                                    grab_dt: x_to_tick(pos.x) - orig.start.0,
                                    cur_start: orig.start.0,
                                    cur_end: orig.start.0 + orig.duration.0,
                                    cur_lane: orig.lane,
                                    orig,
                                    moved: false,
                                }));
                            }
                        }
                    }
                }
            } else if response.clicked() && edge_heat(pos.x) != 0.0 {
                // клик в зоне прыжка: курсор СТАВИТСЯ (по позиции до сдвига),
                // и вью прыгает — «и то и то» (решение пользователя, п.10)
                let t = self.snap(x_to_tick(pos.x).max(0), None);
                self.player.seek(Tick(t.max(0)));
                let heat = edge_heat(pos.x);
                let vis = rect.width() as f64 * tpp;
                self.view_start = (self.view_start
                    + heat.signum() as f64 * (heat.abs() as f64).powi(2) * vis * 0.6)
                    .max(-(tpp * 40.0));
            } else if response.drag_started() {
                // пустое место: выделение региона (порт RegionSelect)
                let anchor = x_to_tick(pos.x).max(0);
                self.drag = Some(Drag::Region { anchor });
            } else {
                self.selected = None;
                let t = self.snap(x_to_tick(pos.x).max(0), None);
                self.player.seek(Tick(t.max(0)));
            }
        }

        // ---- drag move ----------------------------------------------------------
        if response.dragged()
            && let Some(pos) = pointer
        {
            match self.drag.take() {
                Some(Drag::Lane { kind, from, cur }) => {
                    // обрабатывается gutter-областью; здесь просто держим
                    self.drag = Some(Drag::Lane { kind, from, cur });
                }
                Some(Drag::Clip(mut d)) => {
                    d.moved = true;
                    match d.mode {
                        DragMode::Move => {
                            let raw = x_to_tick(pos.x) - d.grab_dt;
                            let dur = d.orig.duration.0;
                            let s1 = self.snap(raw, Some(d.id));
                            let s2 = self.snap(raw + dur, Some(d.id)) - dur;
                            let snapped =
                                if (s1 - raw).abs() <= (s2 - raw).abs() { s1 } else { s2 };
                            d.cur_start = snapped.max(0);
                            d.cur_end = d.cur_start + dur;
                            let rel = pos.y - (rect.top() + RULER_H + 3.0);
                            let idx = (rel / lane_h).floor().max(0.0) as u32;
                            d.cur_lane = match d.orig.kind {
                                ClipKind::Video => idx.min(video_lanes.saturating_sub(1)),
                                ClipKind::Audio => idx
                                    .saturating_sub(video_lanes)
                                    .min(audio_lanes.saturating_sub(1)),
                            };
                        }
                        DragMode::TrimStart => {
                            let raw = x_to_tick(pos.x);
                            let snapped = self.snap(raw, Some(d.id));
                            let max_start = d.cur_end - MIN_CLIP_DURATION.0;
                            let min_start = d.orig.start.0 - d.orig.source_in.0;
                            d.cur_start = snapped.clamp(min_start, max_start);
                        }
                        DragMode::TrimEnd => {
                            let raw = x_to_tick(pos.x);
                            let snapped = self.snap(raw, Some(d.id));
                            d.cur_end = snapped.max(d.cur_start + MIN_CLIP_DURATION.0);
                        }
                    }
                    self.drag = Some(Drag::Clip(d));
                }
                Some(Drag::Scrub) => {
                    let t = self.snap(x_to_tick(pos.x).max(0), None);
                    self.player.seek(Tick(t.max(0)));
                    self.drag = Some(Drag::Scrub);
                }
                Some(Drag::Region { anchor }) => {
                    let cur = x_to_tick(pos.x).max(0);
                    self.model.set_region(Region::normalized(Tick(anchor), Tick(cur)));
                    self.drag = Some(Drag::Region { anchor });
                }
                Some(Drag::RegionEdge { start_edge }) => {
                    if let Some(r) = self.model.project().region {
                        let raw = x_to_tick(pos.x).max(0);
                        let snapped = self.snap(raw, None);
                        let newr = if start_edge {
                            Region::normalized(Tick(snapped), r.end)
                        } else {
                            Region::normalized(r.start, Tick(snapped))
                        };
                        self.model.set_region(newr.or(Some(r)));
                    }
                    self.drag = Some(Drag::RegionEdge { start_edge });
                }
                Some(Drag::Marker { orig, .. }) => {
                    let raw = x_to_tick(pos.x).max(0);
                    let cur = self.snap(raw, None);
                    self.drag = Some(Drag::Marker { orig, cur });
                }
                Some(Drag::Volume { id, .. }) => {
                    // t: 0 = верх (+6 dB), 1 = низ (−6 dB) — порт рейла
                    if let Some(c) = self.model.clip(id) {
                        let top = lane_top(c.kind, c.lane) + 3.0;
                        let h = lane_h - 8.0;
                        let t = ((pos.y - top) / h).clamp(0.0, 1.0);
                        let gain_db = (0.5 - t) * RAIL_DB_SPAN;
                        // live-превью без записи в историю
                        let mut p = self.model.project().clone();
                        if let Some(cm) = p.clips.iter_mut().find(|c| c.id == id) {
                            cm.gain_db = gain_db;
                        }
                        self.player.set_project(p);
                        self.live_gain = Some((id, gain_db));
                    }
                    self.drag = Some(Drag::Volume { id, moved: true });
                }
                None => {}
            }
        }

        // ---- release -------------------------------------------------------------
        if response.drag_stopped() {
            match self.drag.take() {
                Some(Drag::Clip(d)) if d.moved => match d.mode {
                    DragMode::Move => {
                        self.apply(Command::MoveClip {
                            id: d.id,
                            new_start: Tick(d.cur_start),
                            new_lane: d.cur_lane,
                        });
                    }
                    DragMode::TrimStart => {
                        self.apply(Command::TrimStart { id: d.id, new_start: Tick(d.cur_start) });
                    }
                    DragMode::TrimEnd => {
                        self.apply(Command::TrimEnd { id: d.id, new_end: Tick(d.cur_end) });
                    }
                },
                Some(Drag::Marker { orig, cur }) => {
                    if orig != cur {
                        self.apply(Command::MoveMarker { from: Tick(orig), to: Tick(cur) });
                    }
                }
                Some(Drag::Volume { id, moved }) => {
                    if moved && let Some((gid, gain)) = self.live_gain.take_if(|(g, _)| *g == id) {
                        self.apply(Command::SetGain { id: gid, gain_db: gain });
                    }
                }
                Some(Drag::Region { .. }) | Some(Drag::RegionEdge { .. }) => {
                    self.on_region_changed();
                    // играем — курсор немедленно в начало нового региона (п.4)
                    self.on_region_released();
                }
                _ => {}
            }
        }

        // ---- клипы -----------------------------------------------------------------
        // кэши по model_rev (п.1: HashMap'ы и O(n²) пересобирались кадр)
        let (names_ref, suppress_ref) = self.names();
        let media_names: HashMap<ClipId, String> = names_ref.clone();
        let suppress_name: std::collections::HashSet<ClipId> = suppress_ref.clone();
        let media_paths: HashMap<MediaId, String> =
            self.model.project().media.iter().map(|m| (m.id, m.path.clone())).collect();
        let peaks = self.peaks.lock().unwrap();
        let offline = self.offline.lock().unwrap().clone();

        for c in &self.model.project().clips {
            let (start, end, lane) = match &self.drag {
                Some(Drag::Clip(d)) if d.id == c.id => (d.cur_start, d.cur_end, d.cur_lane),
                _ => (c.start.0, c.start.0 + c.duration.0, c.lane),
            };
            let x0 = tick_to_x(start);
            let x1 = tick_to_x(end);
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let top = lane_top(c.kind, lane);
            let clip_rect = egui::Rect::from_min_max(
                egui::pos2(x0.max(rect.left()), top + 1.0),
                egui::pos2(x1.min(rect.right()), top + lane_h - 3.0),
            );
            let selected = self.selected == Some(c.id);
            let dragging = matches!(&self.drag, Some(Drag::Clip(d)) if d.id == c.id);
            let is_offline = offline.contains(&c.media);

            let mut fill = media_paths
                .get(&c.media)
                .map(|p| clip_color(p, c.kind))
                .unwrap_or(egui::Color32::from_gray(80));
            // аудио-клипы темнее — волноформа должна читаться
            if c.kind == ClipKind::Audio {
                fill = fill.gamma_multiply(0.55);
            }
            if dragging {
                fill = fill.gamma_multiply(0.75);
            }
            let edge =
                if selected { egui::Color32::WHITE } else { egui::Color32::from_rgb(25, 25, 25) };
            painter.rect(
                clip_rect,
                4.0,
                fill,
                egui::Stroke::new(if selected { 2.0 } else { 1.0 }, edge),
                egui::StrokeKind::Inside,
            );
            // лёгкий вертикальный градиент (свет сверху)
            crate::toolbar::gradient_rect(
                &painter,
                egui::Rect::from_min_max(
                    clip_rect.left_top() + egui::vec2(1.0, 1.0),
                    egui::pos2(clip_rect.right() - 1.0, clip_rect.center().y),
                ),
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22),
                egui::Color32::TRANSPARENT,
            );
            let hovered =
                hover.map(|h| clip_rect.contains(h) && self.drag.is_none()).unwrap_or(false);
            if hovered && !selected {
                painter.rect(
                    clip_rect,
                    4.0,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 90),
                    ),
                    egui::StrokeKind::Inside,
                );
            }
            if is_offline {
                painter.rect_filled(
                    clip_rect,
                    4.0,
                    egui::Color32::from_rgba_unmultiplied(55, 25, 25, 110),
                );
                painter.rect(
                    clip_rect.shrink(1.5),
                    3.0,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 140, 140)),
                    egui::StrokeKind::Inside,
                );
            }

            // филмстрип видео-клипа: тумбы из индекс-БД/сервиса, отрисовка
            // никогда не блокируется и не трогает декодеры плеера
            if c.kind == ClipKind::Video && !is_offline && clip_rect.width() > 30.0 {
                let thumb_h = clip_rect.height() - 2.0;
                let thumb_w = (thumb_h * 16.0 / 9.0).max(28.0);
                let step = crate::thumbs::thumb_source_step(
                    Tick(end - start),
                    (x1 - x0).max(1.0) * 1.0,
                    thumb_w,
                );
                let mut x = clip_rect.left();
                while x < clip_rect.right() {
                    // момент СЕРЕДИНЫ колонки (v1: тумб центрируется на
                    // своей позиции, а не floor'ится в прошлое — п.3)
                    let tl = x_to_tick(x + thumb_w * 0.5) - start;
                    if tl >= 0 {
                        // бакет от НАЧАЛА КУСКА (source_in), не от нуля
                        // файла: у клипа с source_in=3c первая колонка
                        // показывала кадр за 3 секунды ДО начала куска
                        let src_t = c.source_in.0 + (tl / step * step).max(0);
                        let key = crate::thumbs::ThumbKey { media: c.media, t: src_t };
                        let tex = match self.thumb_textures.get(&key) {
                            Some(t) => Some(t.clone()),
                            None => match self.thumbs.get(&key) {
                                Some(img) => {
                                    let t = ui.ctx().load_texture(
                                        format!("thumb-{}-{}", key.media.0, key.t),
                                        (*img).clone(),
                                        egui::TextureOptions::LINEAR,
                                    );
                                    self.thumb_textures.insert(key, t.clone());
                                    self.thumb_lru.retain(|k| *k != key);
                                    self.thumb_lru.push_back(key);
                                    // LRU вместо полного clear() (п.2: сброс
                                    // всех текстур давал шторм перезагрузок
                                    // при зуме)
                                    while self.thumb_textures.len() > crate::thumbs::MAX_READY
                                        && let Some(old) = self.thumb_lru.pop_front()
                                    {
                                        self.thumb_textures.remove(&old);
                                    }
                                    Some(t)
                                }
                                None => {
                                    self.thumbs.request(key);
                                    None
                                }
                            },
                        };
                        if let Some(t) = tex {
                            let dst = egui::Rect::from_min_size(
                                egui::pos2(x, clip_rect.top() + 1.0),
                                egui::vec2(thumb_w.min(clip_rect.right() - x), thumb_h),
                            );
                            let uv = egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2((dst.width() / thumb_w).min(1.0), 1.0),
                            );
                            painter.image(t.id(), dst, uv, egui::Color32::WHITE);
                        }
                    }
                    x += thumb_w;
                }
            }

            // стерео-волноформа
            if c.kind == ClipKind::Audio
                && !is_offline
                && let Some(levels) = peaks.get(&c.media)
                && !levels.is_empty()
            {
                let frames_per_px = tpp * 48_000.0 / 90_000.0;
                let mut level = 0usize;
                let mut block = kt_media::PEAK_BLOCK_FRAMES as f64;
                while block * 2.0 <= frames_per_px && level + 1 < levels.len() {
                    level += 1;
                    block *= 2.0;
                }
                let (left_ch, right_ch) = &levels[level];
                let mid = clip_rect.center().y;
                let half = (clip_rect.height() - 6.0) * 0.5;
                let eff_src_in = c.source_in.0 + (start - c.start.0).max(0);
                let src_in_frames = Tick(eff_src_in.max(0)).to_audio_frames() as f64;
                let col_l = egui::Color32::from_rgba_unmultiplied(170, 210, 255, 230);
                let col_r = egui::Color32::from_rgba_unmultiplied(230, 190, 255, 230);
                let mut x = clip_rect.left();
                while x < clip_rect.right() {
                    let tl0 = x_to_tick(x) - start;
                    let f0 = src_in_frames + tl0 as f64 * 48_000.0 / 90_000.0;
                    if f0 >= 0.0 {
                        let bi = (f0 / block) as usize;
                        if let Some(&(mn, mx)) = left_ch.get(bi) {
                            let amp = (mx.abs().max(mn.abs())) as f32 / 32767.0;
                            painter.line_segment(
                                [egui::pos2(x, mid), egui::pos2(x, mid - (amp * half).max(0.5))],
                                egui::Stroke::new(1.0, col_l),
                            );
                        }
                        if let Some(&(mn, mx)) = right_ch.get(bi) {
                            let amp = (mx.abs().max(mn.abs())) as f32 / 32767.0;
                            painter.line_segment(
                                [egui::pos2(x, mid), egui::pos2(x, mid + (amp * half).max(0.5))],
                                egui::Stroke::new(1.0, col_r),
                            );
                        }
                    }
                    x += 1.0;
                }
            }

            // volume rail (аудио, ширина > 40)
            if c.kind == ClipKind::Audio && clip_rect.width() > 40.0 {
                let rail = egui::Rect::from_min_size(
                    egui::pos2(x0.max(rect.left()) + EDGE_GRAB_PX + 3.0, top + 3.0),
                    egui::vec2(VOLUME_RAIL_W, lane_h - 8.0),
                );
                painter.rect_filled(rail, 2.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 70));
                let gain = match self.live_gain {
                    Some((id, g)) if id == c.id => g,
                    _ => c.gain_db,
                };
                let t = (0.5 - gain / RAIL_DB_SPAN).clamp(0.0, 1.0);
                let y = rail.top() + rail.height() * t;
                painter.line_segment(
                    [egui::pos2(rail.left(), y), egui::pos2(rail.right(), y)],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 220, 120)),
                );
            }

            // имя клипа на плашке
            if clip_rect.width() > 40.0 && !suppress_name.contains(&c.id) {
                let mut label = media_names.get(&c.id).cloned().unwrap_or_default();
                if !c.fx.is_empty() {
                    label.push(' ');
                    label.push_str(crate::icons::FX);
                }
                if c.reversed {
                    label.push_str(" ◀");
                }
                let galley = painter.layout_no_wrap(
                    label,
                    egui::FontId::proportional(10.5),
                    egui::Color32::WHITE,
                );
                let plate = egui::Rect::from_min_size(
                    clip_rect.left_top() + egui::vec2(4.0, 2.0),
                    galley.size() + egui::vec2(8.0, 3.0),
                );
                painter.rect_filled(
                    plate.intersect(clip_rect),
                    3.0,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 140),
                );
                painter.galley(plate.min + egui::vec2(4.0, 1.5), galley, egui::Color32::WHITE);
            }

            // трим-ручки на выделенном
            if selected && clip_rect.width() > 20.0 {
                for x in [clip_rect.left() + 2.0, clip_rect.right() - 4.0] {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x, clip_rect.center().y - 7.0),
                            egui::vec2(2.5, 14.0),
                        ),
                        1.0,
                        egui::Color32::from_gray(235),
                    );
                }
            }

            // coverage-полоска из кэша (инвариант №4)
            let cov_kind = match c.kind {
                ClipKind::Audio => BurstKind::Audio,
                ClipKind::Video => BurstKind::video(Some(self.player.preview_quality())),
            };
            let (b0, b1) = match c.kind {
                ClipKind::Audio => {
                    let f0 = c.source_in.to_audio_frames();
                    let f1 = (c.source_in + c.duration).to_audio_frames();
                    (
                        f0.div_euclid(AUDIO_BURST_FRAMES as i64),
                        (f1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1,
                    )
                }
                ClipKind::Video => (c.source_in.0, c.source_in.0 + c.duration.0),
            };
            let _ = (cov_kind, b0, b1);
            for run in coverage_snapshot.get(&c.id).cloned().unwrap_or_default() {
                let (t0, t1) = match c.kind {
                    ClipKind::Audio => (
                        c.start.0
                            + Tick::from_audio_frames(
                                run.start * AUDIO_BURST_FRAMES as i64
                                    - c.source_in.to_audio_frames(),
                            )
                            .0,
                        c.start.0
                            + Tick::from_audio_frames(
                                run.end * AUDIO_BURST_FRAMES as i64 - c.source_in.to_audio_frames(),
                            )
                            .0,
                    ),
                    ClipKind::Video => {
                        (c.start.0 + run.start - c.source_in.0, c.start.0 + run.end - c.source_in.0)
                    }
                };
                painter.line_segment(
                    [
                        egui::pos2(tick_to_x(t0).max(clip_rect.left()), clip_rect.bottom() - 1.5),
                        egui::pos2(tick_to_x(t1).min(clip_rect.right()), clip_rect.bottom() - 1.5),
                    ],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(110, 235, 110)),
                );
            }
        }
        drop(peaks);

        // ---- маркеры ----------------------------------------------------------------
        for m in &self.model.project().markers {
            let mt = match &self.drag {
                Some(Drag::Marker { orig, cur }) if *orig == m.0 => *cur,
                _ => m.0,
            };
            let x = tick_to_x(mt);
            if x < rect.left() || x > rect.right() {
                continue;
            }
            let amber = egui::Color32::from_rgb(255, 195, 0);
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, amber),
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x - 5.0, rect.top()),
                    egui::pos2(x + 5.0, rect.top()),
                    egui::pos2(x, rect.top() + 8.0),
                ],
                amber,
                egui::Stroke::NONE,
            ));
        }

        // ---- heat-зоны прыжка/edge-scroll: градиент при наведении ------------------
        if let Some(pos) = hover {
            let boost = if self.drag.is_some() { 1.0f32 } else { 0.6 };
            let margin = edge_margin;
            let warm = |heat: f32, zone: egui::Rect, from_left: bool| {
                if heat > 0.02 {
                    let a = ((36.0 + heat * 174.0) * boost) as u8;
                    let (c0, c1) = if from_left {
                        (
                            egui::Color32::from_rgba_unmultiplied(255, 178, 100, a),
                            egui::Color32::TRANSPARENT,
                        )
                    } else {
                        (
                            egui::Color32::TRANSPARENT,
                            egui::Color32::from_rgba_unmultiplied(255, 178, 100, a),
                        )
                    };
                    let mut mesh = egui::Mesh::default();
                    for (p, c) in [
                        (zone.left_top(), c0),
                        (zone.right_top(), c1),
                        (zone.right_bottom(), c1),
                        (zone.left_bottom(), c0),
                    ] {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos: p,
                            uv: egui::epaint::WHITE_UV,
                            color: c,
                        });
                    }
                    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                    painter.add(egui::Shape::mesh(mesh));
                }
            };
            let lh = ((rect.left() + margin - pos.x) / margin).clamp(0.0, 1.0);
            let rh = ((pos.x - (rect.right() - margin)) / margin).clamp(0.0, 1.0);
            warm(
                lh,
                egui::Rect::from_min_max(
                    egui::pos2(rect.left(), rect.top() + RULER_H),
                    egui::pos2(rect.left() + margin * 0.6, rect.bottom()),
                ),
                true,
            );
            warm(
                rh,
                egui::Rect::from_min_max(
                    egui::pos2(rect.right() - margin * 0.6, rect.top() + RULER_H),
                    egui::pos2(rect.right(), rect.bottom()),
                ),
                false,
            );
        }

        // ---- снап-линия / playhead -----------------------------------------------
        if let Some(t) = self.snap_line {
            let x = tick_to_x(t);
            painter.line_segment(
                [egui::pos2(x, rect.top() + RULER_H), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 220, 120)),
            );
        }
        let ph = self.player.transport.playhead();
        let x = tick_to_x(ph.0);
        if x >= rect.left() && x <= rect.right() {
            // мягкое свечение (п.4): горизонтальный градиент 0→α→0 + ядро
            let glow_half = 7.0f32;
            let mut mesh = egui::Mesh::default();
            let c0 = egui::Color32::from_rgba_unmultiplied(255, 60, 60, 0);
            let c1 = egui::Color32::from_rgba_unmultiplied(255, 60, 60, 90);
            for (xa, xb, ca, cb) in [(x - glow_half, x, c0, c1), (x, x + glow_half, c1, c0)] {
                let i = mesh.vertices.len() as u32;
                for (px, col) in [
                    (egui::pos2(xa, rect.top()), ca),
                    (egui::pos2(xb, rect.top()), cb),
                    (egui::pos2(xb, rect.bottom()), cb),
                    (egui::pos2(xa, rect.bottom()), ca),
                ] {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: px,
                        uv: egui::epaint::WHITE_UV,
                        color: col,
                    });
                }
                mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
            }
            painter.add(egui::Shape::mesh(mesh));
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 100, 100)),
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x - 6.0, rect.top()),
                    egui::pos2(x + 6.0, rect.top()),
                    egui::pos2(x, rect.top() + 9.0),
                ],
                egui::Color32::from_rgb(255, 80, 80),
                egui::Stroke::NONE,
            ));
        }

        // ---- контекст-меню ----------------------------------------------------------
        let target = self.ctx_target;
        response.context_menu(|ui| match target {
            Some(CtxTarget::Clip(id)) => {
                let (is_audio, has_group) = self
                    .model
                    .clip(id)
                    .map(|c| (c.kind == ClipKind::Audio, c.link_group.is_some()))
                    .unwrap_or((false, false));
                if ui.button("Копировать (Ctrl+C)").clicked() {
                    self.selected = Some(id);
                    self.copy_selection();
                    ui.close();
                }
                if ui.button("Вставить (Ctrl+V)").clicked() {
                    self.paste_at_cursor();
                    ui.close();
                }
                if ui.button("Удалить (Del)").clicked() {
                    self.selected = Some(id);
                    // именно ЭТОТ клип (+группа), а не содержимое региона (п.10)
                    self.delete_selected();
                    ui.close();
                }
                if ui.button("Разрезать по курсору (S)").clicked() {
                    self.selected = Some(id);
                    self.cut_action();
                    ui.close();
                }
                if ui.button("Эффекты…").clicked() {
                    self.open_fx_editor(id);
                    ui.close();
                }
                let is_rev = self.model.clip(id).map(|c| c.reversed).unwrap_or(false);
                if ui.button(if is_rev { "Реверс ✓" } else { "Реверс" }).clicked() {
                    self.apply(Command::SetReversed { id, reversed: !is_rev });
                    ui.close();
                }
                ui.separator();
                if has_group {
                    if ui.button("Разомкнуть A/V").clicked() {
                        self.selected = Some(id);
                        self.unlink_selected();
                        ui.close();
                    }
                } else if ui.button("Магнит A/V").clicked() {
                    self.selected = Some(id);
                    self.magnetize_selected();
                    ui.close();
                }
                if is_audio && ui.button("Сбросить громкость (0 dB)").clicked() {
                    self.apply(Command::SetGain { id, gain_db: 0.0 });
                    ui.close();
                }
            }
            Some(CtxTarget::RegionEdge(t)) => {
                if ui.button("Маркер здесь").clicked() {
                    self.apply(Command::AddMarker { at: Tick(t) });
                    ui.close();
                }
                if ui.button("Снять регион").clicked() {
                    self.clear_region();
                    ui.close();
                }
            }
            Some(CtxTarget::Marker(m)) => {
                if ui.button("Удалить маркер").clicked() {
                    self.apply(Command::RemoveMarker { at: Tick(m) });
                    ui.close();
                }
            }
            _ => {
                if !self.clipboard.is_empty() && ui.button("Вставить (Ctrl+V)").clicked() {
                    self.paste_at_cursor();
                    ui.close();
                }
                if self.model.project().region.is_some() {
                    if ui.button("Удалить клипы в регионе (Del)").clicked() {
                        self.delete_action();
                        ui.close();
                    }
                    if ui.button("Вырезать интерьер региона").clicked() {
                        self.punch_out_action();
                        ui.close();
                    }
                    if ui.button("Снять регион").clicked() {
                        self.clear_region();
                        ui.close();
                    }
                } else if self.clipboard.is_empty() {
                    ui.weak("Выдели регион или клип");
                }
            }
        });
    }
}
