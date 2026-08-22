//! KtApp: состояние приложения, панели, клавиши, дропы, lifecycle.

use crate::audio_out;
use crate::timeline::{CtxTarget, Drag};
use crate::util::*;
use eframe::egui;
use kt_engine::Player;
use kt_model::*;
use kt_time::Tick;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct KtApp {
    pub model: TimelineModel,
    pub project_path: Option<PathBuf>,
    pub dirty: bool,
    pub player: Player,
    pub audio: Option<audio_out::AudioOutput>,
    pub peaks: Arc<Mutex<HashMap<MediaId, Peaks>>>,
    pub texture: Option<egui::TextureHandle>,
    pub view_start: f64, // тики
    pub ticks_per_px: f64,
    pub selected: Option<ClipId>,
    pub drag: Option<Drag>,
    pub snap_line: Option<i64>,
    pub ctx_target: Option<CtxTarget>,
    /// Live-громкость volume rail до отпускания (id, dB).
    pub live_gain: Option<(ClipId, f32)>,
    pub status: Option<(String, std::time::Instant)>,
    /// Внутренний буфер обмена (блок клипов, ребейзнутых к нулю).
    pub clipboard: Vec<ClipEvent>,
    /// Медиа, чьи файлы сейчас недоступны (watchdog, 900 мс).
    pub offline: Arc<Mutex<std::collections::HashSet<MediaId>>>,
    pub meter: crate::meters::MeterState,
    pub smooth_scaling: bool,
    pub thumbs: crate::thumbs::ThumbService,
    /// Текстуры филмстрипа (LRU держит ThumbService, тут egui-хэндлы).
    pub thumb_textures: HashMap<crate::thumbs::ThumbKey, egui::TextureHandle>,
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
        let project2 = project.clone();
        let (player, audio, _) = Self::build_player(&project, None);
        KtApp {
            model: TimelineModel::new(project),
            project_path: None,
            dirty: false,
            player,
            audio,
            peaks: Arc::new(Mutex::new(HashMap::new())),
            texture: None,
            view_start: 0.0,
            ticks_per_px: 900.0,
            selected: None,
            drag: None,
            snap_line: None,
            ctx_target: None,
            live_gain: None,
            status: error.map(|e| (e, std::time::Instant::now())),
            clipboard: Vec::new(),
            offline: Arc::new(Mutex::new(std::collections::HashSet::new())),
            meter: Default::default(),
            smooth_scaling: true,
            thumbs: crate::thumbs::ThumbService::start(&project2, None),
            thumb_textures: HashMap::new(),
        }
    }

    fn open(path: &Path) -> Result<KtApp, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let project = Project::from_json(&text).map_err(|e| e.to_string())?;
        let base = path.parent();
        let (player, audio, err) = Self::build_player(&project, base);
        let offline = spawn_media_watchdog(&project, base);
        let thumbs_svc = crate::thumbs::ThumbService::start(&project, Some(path.to_path_buf()));
        let peaks = Arc::new(Mutex::new(HashMap::new()));
        let media: Vec<_> =
            project.media.iter().map(|m| (m.id, resolve_media_path(base, &m.path))).collect();
        spawn_index_worker(Some(path.with_extension("ktindex.db")), media, peaks.clone());
        Ok(KtApp {
            model: TimelineModel::new(project),
            project_path: Some(path.to_path_buf()),
            dirty: false,
            player,
            audio,
            peaks,
            texture: None,
            view_start: 0.0,
            ticks_per_px: 900.0,
            selected: None,
            drag: None,
            snap_line: None,
            ctx_target: None,
            live_gain: None,
            status: err.map(|e| (e, std::time::Instant::now())),
            clipboard: Vec::new(),
            offline,
            meter: Default::default(),
            smooth_scaling: true,
            thumbs: thumbs_svc,
            thumb_textures: HashMap::new(),
        })
    }

    pub fn note(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), std::time::Instant::now()));
    }

    pub fn apply(&mut self, cmd: Command) -> bool {
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

    /// Пакет команд = один undo-снапшот на жест (атомарно).
    pub fn apply_batch(&mut self, cmds: Vec<Command>) -> bool {
        match self.model.apply_batch(cmds) {
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

    // ---- операции паритета (регион/маркеры/клипборд/дорожки) ----------------

    pub fn marker_toggle_at_cursor(&mut self) {
        let at = self.player.transport.playhead();
        // магнит как в старом: удалить ближайший в пределах порога, иначе добавить
        let threshold = ((crate::timeline::SNAP_PX * self.ticks_per_px) as i64).clamp(900, 13_500);
        let near = self
            .model
            .project()
            .markers
            .iter()
            .copied()
            .min_by_key(|m| (m.0 - at.0).abs())
            .filter(|m| (m.0 - at.0).abs() <= threshold);
        match near {
            Some(m) => self.apply(Command::RemoveMarker { at: m }),
            None => self.apply(Command::AddMarker { at }),
        };
    }

    /// Регион in/out от курсора; без региона — затравка [cursor, cursor+1с].
    pub fn set_region_edge_from_cursor(&mut self, set_start: bool) {
        let at = self.player.transport.playhead();
        let cur =
            self.model.project().region.unwrap_or(Region { start: at, end: Tick(at.0 + 90_000) });
        let r = if set_start {
            Region::normalized(at, cur.end.max(Tick(at.0 + 90)))
        } else {
            Region::normalized(cur.start.min(at), at)
        };
        self.model.set_region(r);
        self.on_region_changed();
    }

    pub fn clear_region(&mut self) {
        self.model.set_region(None);
        self.on_region_changed();
    }

    pub fn toggle_loop(&mut self) {
        let on = !self.model.project().loop_enabled;
        self.model.set_loop_enabled(on);
        self.on_region_changed();
    }

    /// Синхронизация региона/loop с плеером.
    pub fn on_region_changed(&mut self) {
        self.player.set_project(self.model.project().clone());
        let p = self.model.project();
        let region =
            p.region.filter(|_| p.loop_enabled).map(|r| kt_time::TickRange::new(r.start, r.end));
        self.player.set_loop_region(region);
        self.dirty = true;
    }

    pub fn copy_selection(&mut self) {
        let block = match self.model.project().region {
            Some(r) => kt_model::ops::copy_region_block(self.model.project(), r),
            None => match self.selected {
                Some(id) => kt_model::ops::copy_group(self.model.project(), id),
                None => Vec::new(),
            },
        };
        if block.is_empty() {
            self.note("нечего копировать (нет региона и выделения)");
        } else {
            self.note(format!("скопировано клипов: {}", block.len()));
            self.clipboard = block;
        }
    }

    pub fn paste_at_cursor(&mut self) {
        if self.clipboard.is_empty() {
            self.note("буфер пуст");
            return;
        }
        let at = self.player.transport.playhead();
        let mut next_clip = self.next_clip_id();
        let mut next_group = self.next_group_id();
        let (cmds, end) =
            kt_model::ops::paste_block(&self.clipboard, at, &mut next_clip, &mut next_group);
        let first = cmds.iter().find_map(|c| match c {
            Command::AddClip(ev) => Some(ev.id),
            _ => None,
        });
        if self.apply_batch(cmds) {
            self.selected = first;
            // курсор в конец блока — chain-paste (семантика старого)
            self.player.seek(end);
        }
    }

    /// S: с регионом = punch-out, без — сплит выделенного.
    pub fn cut_action(&mut self) {
        if let Some(r) = self.model.project().region {
            let mut next = self.next_clip_id();
            let cmds = kt_model::ops::punch_out_region(self.model.project(), r, &mut next);
            if cmds.is_empty() {
                self.note("регион не пересекает клипы");
            } else {
                self.apply_batch(cmds);
                self.selected = None;
            }
        } else {
            self.split_selected();
        }
    }

    /// Del: с регионом = удалить пересекающиеся (+партнёры), без — выделение.
    pub fn delete_action(&mut self) {
        if let Some(r) = self.model.project().region {
            let cmds = kt_model::ops::delete_region(self.model.project(), r);
            if cmds.is_empty() {
                self.note("регион не пересекает клипы");
            } else {
                self.apply_batch(cmds);
                self.selected = None;
            }
        } else {
            self.delete_selected();
        }
    }

    pub fn trim_selected_start_to_cursor(&mut self) {
        let Some(sel) = self.selected else { return };
        let at = self.player.transport.playhead();
        let cmds = kt_model::ops::trim_group_start_to(self.model.project(), sel, at);
        if cmds.is_empty() {
            self.note("курсор вне выбранного клипа");
        } else {
            self.apply_batch(cmds);
        }
    }

    pub fn trim_selected_end_to_cursor(&mut self) {
        let Some(sel) = self.selected else { return };
        let at = self.player.transport.playhead();
        let cmds = kt_model::ops::trim_group_end_to(self.model.project(), sel, at);
        if cmds.is_empty() {
            self.note("курсор вне выбранного клипа");
        } else {
            self.apply_batch(cmds);
        }
    }

    /// Перенос выделенного на соседнюю дорожку своего типа (партнёр на месте).
    pub fn move_selected_lane(&mut self, delta: i32) {
        let Some(sel) = self.selected else { return };
        let Some(c) = self.model.clip(sel).cloned() else { return };
        let lanes = match c.kind {
            ClipKind::Video => self.model.project().video_lanes,
            ClipKind::Audio => self.model.project().audio_lanes,
        };
        let new_lane = (c.lane as i64 + delta as i64).clamp(0, lanes.saturating_sub(1) as i64);
        if new_lane as u32 != c.lane {
            // MoveClip двигает всю группу по времени; здесь время не меняется,
            // а lane меняется только у выбранного — семантика старого
            self.apply(Command::MoveClip {
                id: sel,
                new_start: c.start,
                new_lane: new_lane as u32,
            });
        }
    }

    pub fn add_track(&mut self, kind: ClipKind) {
        let p = self.model.project();
        let (v, a) = (p.video_lanes, p.audio_lanes);
        let cmd = match kind {
            ClipKind::Video => Command::SetLanes { video: v + 1, audio: a },
            ClipKind::Audio => Command::SetLanes { video: v, audio: a + 1 },
        };
        self.apply(cmd);
    }

    /// Магнит A/V: ближайший клип противоположного типа того же медиа
    /// в пределах 2×порога → общий старт/длительность/линк (старый magnetize).
    pub fn magnetize_selected(&mut self) {
        let Some(sel) = self.selected else { return };
        let Some(c) = self.model.clip(sel).cloned() else { return };
        let threshold =
            2 * ((crate::timeline::SNAP_PX * self.ticks_per_px) as i64).clamp(1080, 9000);
        let partner = self
            .model
            .project()
            .clips
            .iter()
            .filter(|o| o.kind != c.kind && o.media == c.media && o.id != c.id)
            .min_by_key(|o| (o.start.0 - c.start.0).abs())
            .filter(|o| (o.start.0 - c.start.0).abs() <= threshold)
            .cloned();
        let Some(partner) = partner else {
            self.note("нет подходящего A/V-партнёра рядом (то же медиа)");
            return;
        };
        let group = c.link_group.or(partner.link_group).unwrap_or(GroupId(self.next_group_id()));
        self.apply_batch(vec![
            Command::MoveClip { id: partner.id, new_start: c.start, new_lane: partner.lane },
            Command::TrimEnd { id: partner.id, new_end: c.start + c.duration },
            Command::LinkClips { ids: vec![c.id, partner.id], group },
        ]);
    }

    pub fn unlink_selected(&mut self) {
        let Some(sel) = self.selected else { return };
        let group = self.model.clip(sel).and_then(|c| c.link_group);
        let ids: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| c.id == sel || (group.is_some() && c.link_group == group))
            .map(|c| c.id)
            .collect();
        self.apply_batch(ids.into_iter().map(|id| Command::UnlinkClip { id }).collect());
    }

    pub fn undo(&mut self) {
        if self.model.undo().is_ok() {
            self.player.set_project(self.model.project().clone());
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if self.model.redo().is_ok() {
            self.player.set_project(self.model.project().clone());
            self.dirty = true;
        }
    }

    pub fn save(&mut self) {
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

    pub fn next_clip_id(&self) -> u64 {
        self.model.project().clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 1
    }

    pub fn next_group_id(&self) -> u64 {
        self.model
            .project()
            .clips
            .iter()
            .filter_map(|c| c.link_group.map(|g| g.0))
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn next_media_id(&self) -> u64 {
        self.model.project().media.iter().map(|m| m.id.0).max().unwrap_or(0) + 1
    }

    pub fn split_selected(&mut self) {
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
        let cmds: Vec<Command> = targets
            .into_iter()
            .map(|id| {
                let cmd = Command::Split { id, at, new_id: ClipId(next_id), new_group };
                next_id += 1;
                cmd
            })
            .collect();
        self.apply_batch(cmds);
    }

    pub fn delete_selected(&mut self) {
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
        self.apply_batch(ids.into_iter().map(|id| Command::RemoveClip { id }).collect());
        self.selected = None;
    }

    /// Пересобрать плеер (после добавления медиа — воркерам нужны источники).
    pub fn rebuild_player(&mut self) {
        let pos = self.player.transport.playhead();
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let (player, audio, err) = Self::build_player(self.model.project(), base.as_deref());
        self.player = player;
        self.audio = audio;
        self.player.seek(pos);
        if let Some(e) = err {
            self.note(e);
        }
    }

    pub fn add_media_clip(&mut self, path: &str) {
        let media_id = MediaId(self.next_media_id());
        if !self.apply(Command::AddMedia { id: media_id, path: path.to_string() }) {
            return;
        }
        self.place_media_at_cursor(media_id);
        self.rebuild_player();
        spawn_index_worker(
            self.project_path.as_ref().map(|p| p.with_extension("ktindex.db")),
            vec![(media_id, path.to_string())],
            self.peaks.clone(),
        );
        self.note(format!("добавлено: {path}"));
    }

    /// Клип(ы) существующего медиа на курсор (клик в бине).
    pub fn place_media_at_cursor(&mut self, media_id: MediaId) {
        let Some(path) =
            self.model.project().media.iter().find(|m| m.id == media_id).map(|m| m.path.clone())
        else {
            return;
        };
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let resolved = resolve_media_path(base.as_deref(), &path);
        let info = match open_source(&resolved) {
            Ok(s) => s.info().clone(),
            Err(e) => {
                self.note(format!("{path}: {e}"));
                return;
            }
        };
        let at = self.player.transport.playhead();
        let dur = if info.duration.0 > 0 { info.duration } else { Tick(90_000 * 5) };
        let group = (info.has_video && info.has_audio).then(|| GroupId(self.next_group_id()));
        let mut next_id = self.next_clip_id();
        let mut cmds = Vec::new();
        for (kind, present) in
            [(ClipKind::Video, info.has_video), (ClipKind::Audio, info.has_audio)]
        {
            if !present {
                continue;
            }
            cmds.push(Command::AddClip(ClipEvent {
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
        self.apply_batch(cmds);
    }

    // -- снап ---------------------------------------------------------------

    fn draw_preview(&mut self, ui: &mut egui::Ui) {
        let t = self.player.transport.playhead();
        let (rgba, q, complete) = self.player.compose_preview(t);
        let image =
            egui::ColorImage::from_rgba_unmultiplied([q.width as usize, q.height as usize], &rgba);
        let opts = if self.smooth_scaling {
            egui::TextureOptions::LINEAR
        } else {
            egui::TextureOptions::NEAREST
        };
        match &mut self.texture {
            Some(tex) => tex.set(image, opts),
            None => {
                self.texture = Some(ui.ctx().load_texture("preview", image, opts));
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
            if ui.button("🚩").on_hover_text("Маркер на курсоре (M)").clicked() {
                self.marker_toggle_at_cursor();
            }
            if ui.button("+V").on_hover_text("Добавить видео-дорожку").clicked()
            {
                self.add_track(ClipKind::Video);
            }
            if ui.button("+A").on_hover_text("Добавить аудио-дорожку").clicked()
            {
                self.add_track(ClipKind::Audio);
            }
            ui.separator();
            if ui.button("✂").on_hover_text("Разрез/вырез региона (S)").clicked()
            {
                self.cut_action();
            }
            if ui.button("🗐").on_hover_text("Копировать (Ctrl+C)").clicked() {
                self.copy_selection();
            }
            if ui.button("📋").on_hover_text("Вставить на курсор (Ctrl+V)").clicked()
            {
                self.paste_at_cursor();
            }
            if ui.button("🗑").on_hover_text("Удалить (Del)").clicked() {
                self.delete_action();
            }
            if ui.button("⇤").on_hover_text("Трим начала к курсору").clicked() {
                self.trim_selected_start_to_cursor();
            }
            if ui.button("⇥").on_hover_text("Трим конца к курсору").clicked() {
                self.trim_selected_end_to_cursor();
            }
            if ui.button("⬆").on_hover_text("На дорожку выше").clicked() {
                self.move_selected_lane(-1);
            }
            if ui.button("⬇").on_hover_text("На дорожку ниже").clicked() {
                self.move_selected_lane(1);
            }
            ui.separator();
            if ui.button("🔍-").on_hover_text("Отдалить (Ctrl+колесо)").clicked() {
                self.ticks_per_px = (self.ticks_per_px * 1.25).clamp(30.0, 90_000.0);
            }
            if ui.button("🔍+").on_hover_text("Приблизить").clicked() {
                self.ticks_per_px = (self.ticks_per_px * 0.8).clamp(30.0, 90_000.0);
            }
            ui.separator();
            if ui.button("⌊").on_hover_text("Регион: начало от курсора (I)").clicked()
            {
                self.set_region_edge_from_cursor(true);
            }
            if ui.button("⌋").on_hover_text("Регион: конец от курсора (O)").clicked()
            {
                self.set_region_edge_from_cursor(false);
            }
            if ui.button("✖").on_hover_text("Снять регион").clicked() {
                self.clear_region();
            }
            let loop_on = self.model.project().loop_enabled;
            if ui.selectable_label(loop_on, "🔁").on_hover_text("Loop региона (L)").clicked()
            {
                self.toggle_loop();
            }
            ui.separator();
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

    fn draw_preview_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // лестница качеств превью: доля разрешения проекта
            let proj = self.model.project().settings.clone();
            let cur_q = self.player.preview_quality();
            let mut frac_idx = {
                let ratio = cur_q.width as f32 / proj.width.max(1) as f32;
                [(1.0 / 16.0), 1.0 / 8.0, 1.0 / 4.0, 1.0 / 2.0, 1.0]
                    .iter()
                    .enumerate()
                    .min_by(|a, b| (a.1 - ratio).abs().partial_cmp(&(b.1 - ratio).abs()).unwrap())
                    .map(|(i, _)| i)
                    .unwrap_or(2)
            };
            ui.label("Качество:");
            let labels = ["1/16", "1/8", "1/4", "1/2", "1/1"];
            let before = frac_idx;
            egui::ComboBox::from_id_salt("quality").selected_text(labels[frac_idx]).show_ui(
                ui,
                |ui| {
                    for (i, l) in labels.iter().enumerate() {
                        ui.selectable_value(&mut frac_idx, i, *l);
                    }
                },
            );
            if frac_idx != before {
                let f = [1.0 / 16.0, 1.0 / 8.0, 1.0 / 4.0, 1.0 / 2.0, 1.0][frac_idx];
                let q = kt_media::Quality {
                    width: (((proj.width as f32 * f) as u32).max(2)) & !1,
                    height: (((proj.height as f32 * f) as u32).max(2)) & !1,
                };
                self.player.set_preview_quality(q);
            }
            ui.separator();
            ui.label("FPS:");
            let mut fps = proj.fps;
            let before_fps = fps;
            egui::ComboBox::from_id_salt("fps").selected_text(fps.to_string()).show_ui(ui, |ui| {
                for f in [24u32, 25, 30, 60] {
                    ui.selectable_value(&mut fps, f, f.to_string());
                }
            });
            if fps != before_fps {
                let mut st = proj.clone();
                st.fps = fps;
                self.apply(Command::SetSettings { settings: st });
            }
            ui.separator();
            ui.label("Разрешение:");
            let mut res = (proj.width, proj.height);
            let before_res = res;
            egui::ComboBox::from_id_salt("res")
                .selected_text(format!("{}×{}", res.0, res.1))
                .show_ui(ui, |ui| {
                    for r in [(640u32, 360u32), (1280, 720), (1920, 1080)] {
                        ui.selectable_value(&mut res, r, format!("{}×{}", r.0, r.1));
                    }
                });
            if res != before_res {
                let mut st = proj.clone();
                st.width = res.0;
                st.height = res.1;
                self.apply(Command::SetSettings { settings: st });
            }
            ui.separator();
            ui.checkbox(&mut self.smooth_scaling, "Сглаживание");
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
            ui.separator();
            let (lv, la) = self.layers_at_cursor();
            let mut overlay = format!("слои: V{lv} A{la}");
            if let Some(r) = self.model.project().region {
                overlay.push_str(&format!(
                    " | регион {:.2}–{:.2}с{}",
                    r.start.as_secs_f64(),
                    r.end.as_secs_f64(),
                    if self.model.project().loop_enabled { " (loop)" } else { "" }
                ));
            }
            ui.weak(overlay);
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
        struct Keys {
            space: bool,
            cut: bool,
            del: bool,
            undo: bool,
            redo: bool,
            save: bool,
            marker: bool,
            region_in: bool,
            region_out: bool,
            loop_toggle: bool,
            copy: bool,
            paste: bool,
        }
        let k = ctx.input(|i| Keys {
            space: i.key_pressed(egui::Key::Space),
            cut: i.key_pressed(egui::Key::S) && !i.modifiers.ctrl,
            del: i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            undo: i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
            redo: i.modifiers.ctrl
                && (i.modifiers.shift && i.key_pressed(egui::Key::Z)
                    || i.key_pressed(egui::Key::Y)),
            save: i.modifiers.ctrl && i.key_pressed(egui::Key::S),
            marker: i.key_pressed(egui::Key::M),
            region_in: i.key_pressed(egui::Key::I),
            region_out: i.key_pressed(egui::Key::O),
            loop_toggle: i.key_pressed(egui::Key::L),
            copy: i.modifiers.ctrl && i.key_pressed(egui::Key::C),
            paste: i.modifiers.ctrl && i.key_pressed(egui::Key::V),
        });
        if k.space {
            if self.player.transport.is_playing() {
                self.player.pause();
            } else {
                self.player.play();
            }
        }
        if k.cut {
            self.cut_action();
        }
        if k.del {
            self.delete_action();
        }
        if k.undo {
            self.undo();
        }
        if k.redo {
            self.redo();
        }
        if k.save {
            self.save();
        }
        if k.marker {
            self.marker_toggle_at_cursor();
        }
        if k.region_in {
            self.set_region_edge_from_cursor(true);
        }
        if k.region_out {
            self.set_region_edge_from_cursor(false);
        }
        if k.loop_toggle {
            self.toggle_loop();
        }
        if k.copy {
            self.copy_selection();
        }
        if k.paste {
            self.paste_at_cursor();
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
        egui::Panel::left("bin").resizable(true).default_size(210.0).show(ui, |ui| {
            ui.add_space(4.0);
            self.draw_bin(ui);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            // стоп на границе региона без loop (семантика старого)
            let p = self.model.project();
            if let Some(r) = p.region
                && !p.loop_enabled
                && self.player.transport.is_playing()
                && self.player.transport.playhead() >= r.end
                && self.player.transport.playhead() < r.end + Tick(9000)
            {
                self.player.pause();
            }
            ui.vertical(|ui| {
                let controls_h = 28.0;
                let avail = ui.available_height() - controls_h;
                ui.allocate_ui(egui::vec2(ui.available_width(), avail), |ui| {
                    ui.horizontal(|ui| {
                        let w = ui.available_width() - 70.0;
                        ui.allocate_ui(egui::vec2(w, ui.available_height()), |ui| {
                            self.draw_preview(ui);
                        });
                        self.draw_meters(ui);
                    });
                });
                self.draw_preview_controls(ui);
            });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

pub fn run() -> eframe::Result {
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
