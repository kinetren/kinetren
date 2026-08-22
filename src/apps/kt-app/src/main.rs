//! GUI-редактор (M4+M5): превью, редактируемый таймлайн (move/trim/split/
//! delete c магнитным снапом, undo/redo), сохранение, drag&drop медиа.
//!
//! Инвариант №8: отрисовка таймлайна НЕ порождает декод-запросов — волноформы
//! из peak-пирамиды, полоски из cache.coverage(). Единственные запросы UI —
//! Playback-кадры текущего момента превью.
//!
//! Правки модели идут командами kt-model (валидация до undo-снапшота);
//! каждая успешная команда → player.set_project → движок диффит FX-состояние
//! (инвариант №3: правка таймлайна не убивает хвосты реверба).

mod audio_out;

use eframe::egui;
use kt_cache::BurstKind;
use kt_engine::{Player, source_id};
use kt_media::MediaSource;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, Tick};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type Peaks = Vec<Vec<(i16, i16)>>; // [level][block] (канал 0)

const RULER_H: f32 = 22.0;
const GUTTER_W: f32 = 52.0;
const EDGE_GRAB_PX: f32 = 7.0;
const SNAP_PX: f64 = 10.0;

fn synth_pattern(path: &str) -> Option<AudioPattern> {
    let rest = path.strip_prefix("synth:")?;
    let mut it = rest.split(':');
    match it.next()? {
        "silence" => Some(AudioPattern::Silence),
        "ramp" => Some(AudioPattern::Ramp),
        "impulse" => Some(AudioPattern::Impulse),
        "sine" => Some(AudioPattern::Sine { hz: it.next()?.parse().ok()? }),
        "dc" => Some(AudioPattern::Dc { value: it.next()?.parse().ok()? }),
        _ => None,
    }
}

fn open_source(path: &str) -> Result<Box<dyn MediaSource>, String> {
    if let Some(p) = synth_pattern(path) {
        return Ok(Box::new(SyntheticSource::new(Tick(90_000 * 3600), p)));
    }
    kt_media::ffmpeg::FfmpegSource::open(path).map(|s| Box::new(s) as _).map_err(|e| e.to_string())
}

fn resolve_media_path(base: Option<&Path>, path: &str) -> String {
    if path.starts_with("synth:") || Path::new(path).is_absolute() {
        return path.to_string();
    }
    match base {
        Some(b) => b.join(path).to_string_lossy().to_string(),
        None => path.to_string(),
    }
}

/// Фоновое построение индекс-БД + загрузка peak-пирамид для отрисовки.
fn spawn_index_worker(
    db_path: Option<PathBuf>,
    media: Vec<(MediaId, String)>,
    out: Arc<Mutex<HashMap<MediaId, Peaks>>>,
) {
    std::thread::Builder::new()
        .name("index-worker".into())
        .spawn(move || {
            let mut db = db_path.and_then(|p| kt_media::index::MediaIndexDb::open(&p).ok());
            for (id, path) in media {
                if out.lock().unwrap().contains_key(&id) {
                    continue;
                }
                let peaks: Option<Peaks> = if path.starts_with("synth:") {
                    open_source(&path).ok().map(|mut s| {
                        kt_media::index::PeakPyramid::build(s.as_mut())
                            .levels
                            .into_iter()
                            .map(|chs| chs.into_iter().next().unwrap_or_default())
                            .collect()
                    })
                } else if let Some(db) = &mut db {
                    match db.ensure_media(Path::new(&path)) {
                        Ok((mid, status)) => {
                            let complete = db.is_complete(mid).unwrap_or(false)
                                && status == kt_media::index::MediaEntryStatus::Reused;
                            if !complete && let Ok(mut src) = open_source(&path) {
                                // отдельный короткоживущий источник —
                                // не декодеры воспроизведения
                                let _ = db.build_audio_peaks(mid, src.as_mut());
                                let _ = db.mark_complete(mid);
                            }
                            let mut levels = Vec::new();
                            let mut level = 0u32;
                            while let Ok(Some(blocks)) = db.audio_peaks(mid, level, 0) {
                                levels.push(blocks);
                                level += 1;
                            }
                            Some(levels)
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                };
                if let Some(p) = peaks {
                    out.lock().unwrap().insert(id, p);
                }
            }
        })
        .expect("spawn index worker");
}

#[derive(Clone, Copy, PartialEq)]
enum DragMode {
    Move,
    TrimStart,
    TrimEnd,
}

struct DragState {
    id: ClipId,
    mode: DragMode,
    /// Тики от начала клипа до точки захвата (для Move).
    grab_dt: i64,
    orig: ClipEvent,
    /// Текущее (превью) положение.
    cur_start: i64,
    cur_end: i64,
    cur_lane: u32,
    moved: bool,
}

struct KtApp {
    model: TimelineModel,
    project_path: Option<PathBuf>,
    dirty: bool,
    player: Player,
    _audio: Option<audio_out::AudioOutput>,
    peaks: Arc<Mutex<HashMap<MediaId, Peaks>>>,
    texture: Option<egui::TextureHandle>,
    view_start: f64, // тики
    ticks_per_px: f64,
    selected: Option<ClipId>,
    drag: Option<DragState>,
    snap_line: Option<i64>,
    status: Option<(String, std::time::Instant)>,
}

impl KtApp {
    fn empty_project() -> Project {
        Project::new(ProjectSettings { fps: 30, width: 1280, height: 720, sample_rate: 48000 })
    }

    fn build_player(
        project: &Project,
        base: Option<&Path>,
    ) -> (Player, Option<audio_out::AudioOutput>, Option<String>) {
        let mut sources = Vec::new();
        let mut err = None;
        for m in &project.media {
            match open_source(&resolve_media_path(base, &m.path)) {
                Ok(s) => sources.push((m.id, s)),
                Err(e) => err = Some(format!("{}: {e}", m.path)),
            }
        }
        let (player, ring) = Player::new(project.clone(), sources);
        let audio = match audio_out::start(ring, player.shared.clone(), player.transport.clone()) {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("аудио-вывод недоступен: {e}");
                None
            }
        };
        (player, audio, err)
    }

    fn new_empty(error: Option<String>) -> KtApp {
        let project = Self::empty_project();
        let (player, audio, _) = Self::build_player(&project, None);
        KtApp {
            model: TimelineModel::new(project),
            project_path: None,
            dirty: false,
            player,
            _audio: audio,
            peaks: Arc::new(Mutex::new(HashMap::new())),
            texture: None,
            view_start: 0.0,
            ticks_per_px: 900.0,
            selected: None,
            drag: None,
            snap_line: None,
            status: error.map(|e| (e, std::time::Instant::now())),
        }
    }

    fn open(path: &Path) -> Result<KtApp, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let project = Project::from_json(&text).map_err(|e| e.to_string())?;
        let base = path.parent();
        let (player, audio, err) = Self::build_player(&project, base);
        let peaks = Arc::new(Mutex::new(HashMap::new()));
        let media: Vec<_> =
            project.media.iter().map(|m| (m.id, resolve_media_path(base, &m.path))).collect();
        spawn_index_worker(Some(path.with_extension("ktindex.db")), media, peaks.clone());
        Ok(KtApp {
            model: TimelineModel::new(project),
            project_path: Some(path.to_path_buf()),
            dirty: false,
            player,
            _audio: audio,
            peaks,
            texture: None,
            view_start: 0.0,
            ticks_per_px: 900.0,
            selected: None,
            drag: None,
            snap_line: None,
            status: err.map(|e| (e, std::time::Instant::now())),
        })
    }

    fn note(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), std::time::Instant::now()));
    }

    fn apply(&mut self, cmd: Command) -> bool {
        match self.model.apply(cmd) {
            Ok(()) => {
                self.player.set_project(self.model.project().clone());
                self.dirty = true;
                true
            }
            Err(e) => {
                self.note(e.to_string());
                false
            }
        }
    }

    fn undo(&mut self) {
        if self.model.undo().is_ok() {
            self.player.set_project(self.model.project().clone());
            self.dirty = true;
        }
    }

    fn redo(&mut self) {
        if self.model.redo().is_ok() {
            self.player.set_project(self.model.project().clone());
            self.dirty = true;
        }
    }

    fn save(&mut self) {
        let Some(path) = self.project_path.clone() else {
            self.note("нет пути проекта (открой .ktproj)");
            return;
        };
        match std::fs::write(&path, self.model.project().to_json()) {
            Ok(()) => {
                self.dirty = false;
                self.note(format!("сохранено: {}", path.display()));
            }
            Err(e) => self.note(format!("ошибка сохранения: {e}")),
        }
    }

    fn next_clip_id(&self) -> u64 {
        self.model.project().clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 1
    }

    fn next_group_id(&self) -> u64 {
        self.model
            .project()
            .clips
            .iter()
            .filter_map(|c| c.link_group.map(|g| g.0))
            .max()
            .unwrap_or(0)
            + 1
    }

    fn next_media_id(&self) -> u64 {
        self.model.project().media.iter().map(|m| m.id.0).max().unwrap_or(0) + 1
    }

    fn split_selected(&mut self) {
        let Some(sel) = self.selected else {
            self.note("нечего резать: клип не выбран");
            return;
        };
        let at = self.player.transport.playhead();
        let Some(clip) = self.model.clip(sel).cloned() else { return };
        let group = clip.link_group;
        let new_group = group.map(|_| GroupId(self.next_group_id()));
        // linked-партнёры, содержащие точку разреза, режутся тем же разрезом
        let targets: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| {
                (c.id == sel || (group.is_some() && c.link_group == group))
                    && c.start < at
                    && at < c.start + c.duration
            })
            .map(|c| c.id)
            .collect();
        if targets.is_empty() {
            self.note("курсор вне выбранного клипа");
            return;
        }
        let mut next_id = self.next_clip_id();
        for id in targets {
            let ok = self.apply(Command::Split { id, at, new_id: ClipId(next_id), new_group });
            if ok {
                next_id += 1;
            }
        }
    }

    fn delete_selected(&mut self) {
        let Some(sel) = self.selected else { return };
        // удаляем весь link-групп (как Del по A/V-паре в старом редакторе)
        let group = self.model.clip(sel).and_then(|c| c.link_group);
        let ids: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| c.id == sel || (group.is_some() && c.link_group == group))
            .map(|c| c.id)
            .collect();
        for id in ids {
            self.apply(Command::RemoveClip { id });
        }
        self.selected = None;
    }

    /// Пересобрать плеер (после добавления медиа — воркерам нужны источники).
    fn rebuild_player(&mut self) {
        let pos = self.player.transport.playhead();
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let (player, audio, err) = Self::build_player(self.model.project(), base.as_deref());
        self.player = player;
        self._audio = audio;
        self.player.seek(pos);
        if let Some(e) = err {
            self.note(e);
        }
    }

    fn add_media_clip(&mut self, path: &str) {
        let src = match open_source(path) {
            Ok(s) => s,
            Err(e) => {
                self.note(format!("{path}: {e}"));
                return;
            }
        };
        let info = src.info().clone();
        drop(src);
        let media_id = MediaId(self.next_media_id());
        if !self.apply(Command::AddMedia { id: media_id, path: path.to_string() }) {
            return;
        }
        let at = self.player.transport.playhead();
        let dur = if info.duration.0 > 0 { info.duration } else { Tick(90_000 * 5) };
        let group = (info.has_video && info.has_audio).then(|| GroupId(self.next_group_id()));
        let mut next_id = self.next_clip_id();
        for (kind, present) in
            [(ClipKind::Video, info.has_video), (ClipKind::Audio, info.has_audio)]
        {
            if !present {
                continue;
            }
            self.apply(Command::AddClip(ClipEvent {
                id: ClipId(next_id),
                link_group: group,
                kind,
                lane: 0,
                start: at,
                duration: dur,
                source_in: Tick(0),
                media: media_id,
                gain_db: 0.0,
                fx: vec![],
            }));
            next_id += 1;
        }
        self.rebuild_player();
        spawn_index_worker(
            self.project_path.as_ref().map(|p| p.with_extension("ktindex.db")),
            vec![(media_id, path.to_string())],
            self.peaks.clone(),
        );
        self.note(format!("добавлено: {path}"));
    }

    // -- снап ---------------------------------------------------------------

    /// Магнитные цели: границы чужих клипов, playhead, ноль.
    fn snap_targets(&self, exclude: ClipId) -> Vec<i64> {
        let mut t: Vec<i64> = vec![0, self.player.transport.playhead().0];
        for c in &self.model.project().clips {
            if c.id != exclude {
                t.push(c.start.0);
                t.push(c.start.0 + c.duration.0);
            }
        }
        t
    }

    fn snap(&mut self, tick: i64, exclude: ClipId) -> i64 {
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
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

impl KtApp {
    fn draw_preview(&mut self, ui: &mut egui::Ui) {
        let t = self.player.transport.playhead();
        let (rgba, q, complete) = self.player.compose_preview(t);
        let image =
            egui::ColorImage::from_rgba_unmultiplied([q.width as usize, q.height as usize], &rgba);
        match &mut self.texture {
            Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
            None => {
                self.texture =
                    Some(ui.ctx().load_texture("preview", image, egui::TextureOptions::LINEAR));
            }
        }
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(12));
        if let Some(tex) = &self.texture {
            let scale =
                (rect.width() / q.width as f32).min(rect.height() / q.height as f32).max(0.05);
            let size = egui::vec2(q.width as f32 * scale, q.height as f32 * scale);
            let img_rect = egui::Rect::from_center_size(rect.center(), size);
            egui::Image::new((tex.id(), size)).paint_at(ui, img_rect);
            if !complete {
                ui.painter().text(
                    rect.left_top() + egui::vec2(8.0, 8.0),
                    egui::Align2::LEFT_TOP,
                    "буферизация…",
                    egui::FontId::proportional(12.0),
                    egui::Color32::YELLOW,
                );
            }
        }
    }

    fn lane_counts(&self) -> (u32, u32) {
        let p = self.model.project();
        let v = p.clips.iter().filter(|c| c.kind == ClipKind::Video).map(|c| c.lane + 1).max();
        let a = p.clips.iter().filter(|c| c.kind == ClipKind::Audio).map(|c| c.lane + 1).max();
        (v.unwrap_or(1).max(1), a.unwrap_or(1).max(1))
    }

    #[allow(clippy::too_many_lines)]
    fn draw_timeline(&mut self, ui: &mut egui::Ui) {
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

    fn draw_transport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let playing = self.player.transport.is_playing();
            if ui
                .add_sized([34.0, 24.0], egui::Button::new(if playing { "⏸" } else { "▶" }))
                .clicked()
            {
                if playing {
                    self.player.pause();
                } else {
                    self.player.play();
                }
            }
            if ui.add_sized([28.0, 24.0], egui::Button::new("⏮")).clicked() {
                self.player.seek(Tick(0));
            }
            ui.separator();
            let t = self.player.transport.playhead();
            let ms = t.as_ms();
            let fps = self.model.project().settings.fps as i64;
            let frame = t.0 % 90_000 / (90_000 / fps);
            ui.monospace(format!(
                "{:02}:{:02}:{:02}.{:02}",
                ms / 3_600_000,
                ms % 3_600_000 / 60_000,
                ms % 60_000 / 1000,
                frame
            ));
            ui.separator();
            if ui.button("✂ split (S)").clicked() {
                self.split_selected();
            }
            if ui.button("🗑 del").clicked() {
                self.delete_selected();
            }
            if ui.button("↶").on_hover_text("Ctrl+Z").clicked() {
                self.undo();
            }
            if ui.button("↷").on_hover_text("Ctrl+Shift+Z").clicked() {
                self.redo();
            }
            ui.separator();
            if ui.button(if self.dirty { "💾 ●" } else { "💾" }).on_hover_text("Ctrl+S").clicked()
            {
                self.save();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(p) = &self.project_path {
                    ui.weak(format!(
                        "{}{}",
                        p.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                        if self.dirty { " ●" } else { "" }
                    ));
                }
            });
        });
    }

    fn draw_statusbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let ord = std::sync::atomic::Ordering::Relaxed;
            ui.weak(format!(
                "кэш: {:.0}/{:.0} МБ",
                self.player.cache.audio_bytes() as f64 / 1e6,
                self.player.cache.video_bytes() as f64 / 1e6,
            ));
            ui.separator();
            ui.weak(format!(
                "underruns: {}/{}",
                self.player.underruns.load(ord),
                self.player.shared.callback_underruns.load(ord)
            ));
            ui.separator();
            ui.weak(format!("зум: {:.0} px/с", 90_000.0 / self.ticks_per_px));
            if let Some((msg, at)) = &self.status {
                if at.elapsed().as_secs() < 6 {
                    let msg = msg.clone();
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(255, 210, 120), msg);
                } else {
                    self.status = None;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak("перетащи медиафайл на окно — клип встанет на курсор");
            });
        });
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (space, s, del, undo, redo, save) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Space),
                i.key_pressed(egui::Key::S) && !i.modifiers.ctrl,
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
                i.modifiers.ctrl
                    && (i.modifiers.shift && i.key_pressed(egui::Key::Z)
                        || i.key_pressed(egui::Key::Y)),
                i.modifiers.ctrl && i.key_pressed(egui::Key::S),
            )
        });
        if space {
            if self.player.transport.is_playing() {
                self.player.pause();
            } else {
                self.player.play();
            }
        }
        if s {
            self.split_selected();
        }
        if del {
            self.delete_selected();
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if save {
            self.save();
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        for path in dropped {
            if path.extension().map(|e| e == "ktproj").unwrap_or(false) {
                match KtApp::open(&path) {
                    Ok(app) => *self = app,
                    Err(e) => self.note(format!("{}: {e}", path.display())),
                }
            } else {
                self.add_media_clip(&path.to_string_lossy());
            }
        }
    }
}

impl eframe::App for KtApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_keys(&ctx);
        self.handle_drops(&ctx);

        egui::Panel::top("transport").show(ui, |ui| {
            ui.add_space(3.0);
            self.draw_transport(ui);
            ui.add_space(3.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            self.draw_statusbar(ui);
        });
        egui::Panel::bottom("timeline").resizable(true).default_size(260.0).show(ui, |ui| {
            self.draw_timeline(ui);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_preview(ui);
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn main() -> eframe::Result {
    let arg = std::env::args().nth(1);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 860.0])
            .with_title("Kinotrain"),
        ..Default::default()
    };
    eframe::run_native(
        "kinotrain",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            let app = match arg {
                Some(p) => {
                    let path = std::path::PathBuf::from(&p);
                    KtApp::open(&path).unwrap_or_else(|e| KtApp::new_empty(Some(e)))
                }
                None => KtApp::new_empty(Some(
                    "Открой проект: kt-app <file.ktproj> или перетащи его на окно".into(),
                )),
            };
            Ok(Box::new(app))
        }),
    )
}
