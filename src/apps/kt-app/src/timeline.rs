//! Таймлайн: отрисовка и интеракции (U2 доведёт до полного паритета
//! и разделит на draw/interact при росте).

use crate::app::KtApp;
use eframe::egui;
use kt_cache::BurstKind;
use kt_engine::source_id;
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, Tick};
use std::collections::HashMap;
use std::path::Path;

pub const RULER_H: f32 = 22.0;
pub const GUTTER_W: f32 = 52.0;
pub const EDGE_GRAB_PX: f32 = 7.0;
pub const SNAP_PX: f64 = 10.0;

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
impl KtApp {
    pub fn snap_targets(&self, exclude: ClipId) -> Vec<i64> {
        let mut t: Vec<i64> = vec![0, self.player.transport.playhead().0];
        for c in &self.model.project().clips {
            if c.id != exclude {
                t.push(c.start.0);
                t.push(c.start.0 + c.duration.0);
            }
        }
        t
    }

    pub fn snap(&mut self, tick: i64, exclude: ClipId) -> i64 {
        let threshold = (SNAP_PX * self.ticks_per_px) as i64;
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
        let v = p.clips.iter().filter(|c| c.kind == ClipKind::Video).map(|c| c.lane + 1).max();
        let a = p.clips.iter().filter(|c| c.kind == ClipKind::Audio).map(|c| c.lane + 1).max();
        (v.unwrap_or(1).max(1), a.unwrap_or(1).max(1))
    }

    #[allow(clippy::too_many_lines)]
    pub fn draw_timeline(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        let gutter =
            egui::Rect::from_min_max(full.min, egui::pos2(full.left() + GUTTER_W, full.bottom()));
        let rect = egui::Rect::from_min_max(egui::pos2(gutter.right(), full.top()), full.max);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 0.0, egui::Color32::from_gray(22));
        painter.rect_filled(gutter, 0.0, egui::Color32::from_gray(28));

        // зум/пан
        let mut view_start = self.view_start;
        let mut tpp = self.ticks_per_px;
        if let Some(pos) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            let zoom = ui.input(|i| i.zoom_delta());
            if scroll.y != 0.0 && ui.input(|i| i.modifiers.shift) {
                view_start -= scroll.y as f64 * tpp * 4.0;
            } else if scroll.x != 0.0 {
                view_start -= scroll.x as f64 * tpp * 4.0;
            } else if scroll.y != 0.0 || zoom != 1.0 {
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
            }
        }
        view_start = view_start.max(-(tpp * 40.0));
        self.view_start = view_start;
        self.ticks_per_px = tpp;
        let x_to_tick =
            move |x: f32| -> i64 { (view_start + (x - rect.left()) as f64 * tpp) as i64 };
        let tick_to_x =
            move |t: i64| -> f32 { rect.left() + ((t as f64 - view_start) / tpp) as f32 };

        // линейка
        let sec_px = (90_000.0 / tpp) as f32;
        let label_every = (70.0 / sec_px).ceil().max(1.0) as i64;
        for s in (x_to_tick(rect.left()) / 90_000).max(0)..=(x_to_tick(rect.right()) / 90_000 + 1) {
            let x = tick_to_x(s * 90_000);
            let major = s % label_every == 0;
            painter.line_segment(
                [
                    egui::pos2(x, rect.top()),
                    egui::pos2(x, rect.top() + if major { RULER_H } else { 7.0 }),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
            );
            if major {
                painter.text(
                    egui::pos2(x + 4.0, rect.top() + 3.0),
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

        // дорожки
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
            painter.rect_filled(
                lane_rect,
                0.0,
                egui::Color32::from_gray(if i % 2 == 0 { 26 } else { 24 }),
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

        // -- интеракция -------------------------------------------------------
        let pointer = response.interact_pointer_pos();
        let hover = response.hover_pos();
        self.snap_line = None;

        let clip_at = |app: &KtApp, pos: egui::Pos2| -> Option<(ClipId, DragMode)> {
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
                let mode = if (pos.x - x0).abs() <= EDGE_GRAB_PX {
                    DragMode::TrimStart
                } else if (pos.x - x1).abs() <= EDGE_GRAB_PX {
                    DragMode::TrimEnd
                } else {
                    DragMode::Move
                };
                return Some((c.id, mode));
            }
            None
        };

        if self.drag.is_none()
            && let Some(pos) = hover
            && pos.y > rect.top() + RULER_H
        {
            match clip_at(self, pos) {
                Some((_, DragMode::TrimStart)) | Some((_, DragMode::TrimEnd)) => {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal)
                }
                Some((_, DragMode::Move)) => ui.ctx().set_cursor_icon(egui::CursorIcon::Grab),
                None => {}
            }
        }

        if (response.drag_started() || response.clicked())
            && let Some(pos) = pointer
        {
            if pos.y <= rect.top() + RULER_H {
                self.player.seek(Tick(x_to_tick(pos.x).max(0)));
            } else if let Some((id, mode)) = clip_at(self, pos) {
                self.selected = Some(id);
                if response.drag_started()
                    && let Some(orig) = self.model.clip(id).cloned()
                {
                    self.drag = Some(DragState {
                        id,
                        mode,
                        grab_dt: x_to_tick(pos.x) - orig.start.0,
                        cur_start: orig.start.0,
                        cur_end: orig.start.0 + orig.duration.0,
                        cur_lane: orig.lane,
                        orig,
                        moved: false,
                    });
                }
            } else {
                self.selected = None;
                self.player.seek(Tick(x_to_tick(pos.x).max(0)));
            }
        }

        if response.dragged()
            && let Some(pos) = pointer
        {
            if let Some(mut d) = self.drag.take() {
                d.moved = true;
                match d.mode {
                    DragMode::Move => {
                        let raw = x_to_tick(pos.x) - d.grab_dt;
                        let dur = d.orig.duration.0;
                        let s1 = self.snap(raw, d.id);
                        let s2 = self.snap(raw + dur, d.id) - dur;
                        let snapped = if (s1 - raw).abs() <= (s2 - raw).abs() { s1 } else { s2 };
                        d.cur_start = snapped.max(0);
                        d.cur_end = d.cur_start + dur;
                        let rel = pos.y - (rect.top() + RULER_H + 3.0);
                        let idx = (rel / lane_h).floor().max(0.0) as u32;
                        d.cur_lane = match d.orig.kind {
                            ClipKind::Video => idx.min(video_lanes.saturating_sub(1)),
                            ClipKind::Audio => {
                                idx.saturating_sub(video_lanes).min(audio_lanes.saturating_sub(1))
                            }
                        };
                    }
                    DragMode::TrimStart => {
                        let raw = x_to_tick(pos.x);
                        let snapped = self.snap(raw, d.id);
                        let max_start = d.cur_end - MIN_CLIP_DURATION.0;
                        let min_start = d.orig.start.0 - d.orig.source_in.0;
                        d.cur_start = snapped.clamp(min_start, max_start);
                    }
                    DragMode::TrimEnd => {
                        let raw = x_to_tick(pos.x);
                        let snapped = self.snap(raw, d.id);
                        d.cur_end = snapped.max(d.cur_start + MIN_CLIP_DURATION.0);
                    }
                }
                self.drag = Some(d);
            } else if pos.y <= rect.top() + RULER_H {
                self.player.seek(Tick(x_to_tick(pos.x).max(0)));
            }
        }

        if response.drag_stopped()
            && let Some(d) = self.drag.take()
            && d.moved
        {
            match d.mode {
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
            }
        }

        // -- клипы -------------------------------------------------------------
        let peaks = self.peaks.lock().unwrap();
        let media_names: HashMap<MediaId, String> = self
            .model
            .project()
            .media
            .iter()
            .map(|m| {
                let name = Path::new(&m.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| m.path.clone());
                (m.id, name)
            })
            .collect();

        for c in &self.model.project().clips {
            let (start, end, lane) = match &self.drag {
                Some(d) if d.id == c.id => (d.cur_start, d.cur_end, d.cur_lane),
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
            let dragging = matches!(&self.drag, Some(d) if d.id == c.id);
            let (mut fill, mut edge) = match c.kind {
                ClipKind::Video => {
                    (egui::Color32::from_rgb(40, 68, 106), egui::Color32::from_rgb(95, 145, 205))
                }
                ClipKind::Audio => {
                    (egui::Color32::from_rgb(42, 86, 62), egui::Color32::from_rgb(95, 185, 125))
                }
            };
            if dragging {
                fill = fill.gamma_multiply(0.75);
            }
            if selected {
                edge = egui::Color32::from_rgb(255, 200, 90);
            }
            painter.rect(
                clip_rect,
                4.0,
                fill,
                egui::Stroke::new(if selected { 2.0 } else { 1.0 }, edge),
                egui::StrokeKind::Inside,
            );

            if c.kind == ClipKind::Audio
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
                let blocks = &levels[level];
                let mid = clip_rect.center().y;
                let half = (clip_rect.height() - 4.0) * 0.5;
                let eff_src_in = c.source_in.0 + (start - c.start.0).max(0);
                let src_in_frames = Tick(eff_src_in.max(0)).to_audio_frames() as f64;
                let wave = egui::Color32::from_rgb(140, 220, 160).gamma_multiply(0.9);
                let mut x = clip_rect.left();
                while x < clip_rect.right() {
                    let tl0 = x_to_tick(x) - start;
                    let f0 = src_in_frames + tl0 as f64 * 48_000.0 / 90_000.0;
                    if f0 >= 0.0
                        && let Some(&(mn, mx)) = blocks.get((f0 / block) as usize)
                    {
                        let y0 = mid - mx as f32 / 32767.0 * half;
                        let y1 = mid - mn as f32 / 32767.0 * half;
                        painter.line_segment(
                            [egui::pos2(x, y0), egui::pos2(x, y1.max(y0 + 1.0))],
                            egui::Stroke::new(1.0, wave),
                        );
                    }
                    x += 1.0;
                }
            }

            if clip_rect.width() > 40.0 {
                let mut label = media_names.get(&c.media).cloned().unwrap_or_default();
                if !c.fx.is_empty() {
                    label.push_str("  ⚡");
                }
                painter.text(
                    clip_rect.left_top() + egui::vec2(6.0, 3.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    egui::FontId::proportional(10.5),
                    egui::Color32::from_gray(220),
                );
            }

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
            for run in self.player.cache.coverage(source_id(c.media), cov_kind, b0..b1) {
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
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 80, 80)),
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
    }
}
