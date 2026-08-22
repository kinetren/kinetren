//! Таймлайн: отрисовка и интеракции. Паритет со старым Qt-таймлайном
//! (см. docs/porting-notes.md и план UI-паритета): маркеры, регион,
//! volume rail, стерео-волноформы, цвет клипа из хэша пути, офлайн-тинт,
//! edge-autoscroll, контекст-меню, снап к маркерам/региону/клипам.

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
        let full = ui.available_rect_before_wrap();
        let gutter =
            egui::Rect::from_min_max(full.min, egui::pos2(full.left() + GUTTER_W, full.bottom()));
        let rect = egui::Rect::from_min_max(egui::pos2(gutter.right(), full.top()), full.max);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 0.0, egui::Color32::from_gray(22));
        painter.rect_filled(gutter, 0.0, egui::Color32::from_gray(28));

        // ---- зум/пан + edge-autoscroll при драге --------------------------
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
        // edge-autoscroll: порт heat^4.15 * 0.18, шаг 1..42 мс за кадр
        if self.drag.is_some()
            && let Some(pos) = response.hover_pos()
        {
            let margin = (rect.width() / 6.0).max(24.0);
            let heat = if pos.x < rect.left() + margin {
                -((rect.left() + margin - pos.x) / margin).clamp(0.0, 1.0)
            } else if pos.x > rect.right() - margin {
                ((pos.x - (rect.right() - margin)) / margin).clamp(0.0, 1.0)
            } else {
                0.0
            };
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

        // ---- линейка --------------------------------------------------------
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

        // ---- регион: заливка ---------------------------------------------------
        if let Some(r) = self.model.project().region {
            let x0 = tick_to_x(r.start.0).max(rect.left());
            let x1 = tick_to_x(r.end.0).min(rect.right());
            if x1 > x0 {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x0, rect.top()),
                        egui::pos2(x1, rect.bottom()),
                    ),
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(84, 124, 255, 22),
                );
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
        let region_edge_at = |app: &KtApp, pos: egui::Pos2| -> Option<bool> {
            if pos.y > rect.top() + RULER_H {
                return None;
            }
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
            } else if let Some((id, _)) = clip_at(self, pos) {
                self.selected = Some(id);
                CtxTarget::Clip(id)
            } else {
                CtxTarget::Empty
            });
        }

        // ---- press ------------------------------------------------------------
        if (response.drag_started() || response.clicked())
            && let Some(pos) = pointer
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
                self.player.seek(Tick(x_to_tick(pos.x).max(0)));
            } else if let Some((id, zone)) = clip_at(self, pos) {
                self.selected = Some(id);
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
            } else if response.drag_started() {
                // пустое место: выделение региона (порт RegionSelect)
                let anchor = x_to_tick(pos.x).max(0);
                self.drag = Some(Drag::Region { anchor });
            } else {
                self.selected = None;
                self.player.seek(Tick(x_to_tick(pos.x).max(0)));
            }
        }

        // ---- drag move ----------------------------------------------------------
        if response.dragged()
            && let Some(pos) = pointer
        {
            match self.drag.take() {
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
                None => {
                    if pos.y <= rect.top() + RULER_H {
                        self.player.seek(Tick(x_to_tick(pos.x).max(0)));
                    }
                }
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
                }
                _ => {}
            }
        }

        // ---- клипы -----------------------------------------------------------------
        let peaks = self.peaks.lock().unwrap();
        let offline = self.offline.lock().unwrap().clone();
        let media_paths: HashMap<MediaId, String> =
            self.model.project().media.iter().map(|m| (m.id, m.path.clone())).collect();
        let media_names: HashMap<MediaId, String> = media_paths
            .iter()
            .map(|(id, p)| {
                let name = Path::new(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.clone());
                (*id, name)
            })
            .collect();

        // подавление имени у аудио-двойника magnetized-пары
        let suppress_name: std::collections::HashSet<ClipId> = {
            let p = self.model.project();
            p.clips
                .iter()
                .filter(|a| a.kind == ClipKind::Audio && a.link_group.is_some())
                .filter(|a| {
                    p.clips.iter().any(|v| {
                        v.kind == ClipKind::Video
                            && v.link_group == a.link_group
                            && v.media == a.media
                            && v.start == a.start
                            && v.duration == a.duration
                            && v.source_in == a.source_in
                    })
                })
                .map(|a| a.id)
                .collect()
        };

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
                let mut label = media_names.get(&c.media).cloned().unwrap_or_default();
                if !c.fx.is_empty() {
                    label.push_str(" ⚡");
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
                if ui.button("Удалить (Del)").clicked() {
                    self.selected = Some(id);
                    self.delete_action();
                    ui.close();
                }
                if ui.button("Разрезать по курсору (S)").clicked() {
                    self.selected = Some(id);
                    self.cut_action();
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
            Some(CtxTarget::Marker(m)) => {
                if ui.button("Удалить маркер").clicked() {
                    self.apply(Command::RemoveMarker { at: Tick(m) });
                    ui.close();
                }
            }
            _ => {
                if self.model.project().region.is_some() {
                    if ui.button("Удалить клипы в регионе (Del)").clicked() {
                        self.delete_action();
                        ui.close();
                    }
                    if ui.button("Вырезать интерьер региона (S)").clicked() {
                        self.cut_action();
                        ui.close();
                    }
                    if ui.button("Снять регион").clicked() {
                        self.clear_region();
                        ui.close();
                    }
                } else {
                    ui.weak("Выдели регион или клип");
                }
            }
        });
    }
}
