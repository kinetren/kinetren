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
    /// Keep-alive хэндл cpal-стрима (Drop = остановка вывода).
    #[allow(dead_code)]
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
    pub fx_editors: Vec<crate::fx_editor::FxEditor>,
    pub gesture_counter: u64,
    pub render_job: Option<crate::render_dialog::RenderJob>,
    pub last_autosave: std::time::Instant,
    /// Хинт наведённой кнопки тулбара — дублируется в статусбар (порт).
    pub hover_hint: Option<String>,
    /// Rect таймлайна прошлого кадра (для плавающего тулбара).
    pub timeline_rect: egui::Rect,
    pub perf: PerfState,
    /// Ключ последнего ЗАГРУЖЕННОГО в текстуру кадра превью: (playhead-тик,
    /// качество, полный ли). Текстура обновляется только при смене —
    /// раньше кадр заливался в GPU 60 раз/с даже в паузе (п.1).
    pub preview_key: Option<(i64, (u32, u32), bool)>,
    /// Когда превью впервые стало неполным (скрим «Загрузка» — после 180 мс,
    /// как в v1, чтобы не мигал на быстрых промахах; п.14).
    pub incomplete_since: Option<std::time::Instant>,
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
            fx_editors: Vec::new(),
            gesture_counter: 0,
            render_job: None,
            last_autosave: std::time::Instant::now(),
            hover_hint: None,
            timeline_rect: egui::Rect::NOTHING,
            perf: PerfState::new(),
            preview_key: None,
            incomplete_since: None,
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
            fx_editors: Vec::new(),
            gesture_counter: 0,
            render_job: None,
            last_autosave: std::time::Instant::now(),
            hover_hint: None,
            timeline_rect: egui::Rect::NOTHING,
            perf: PerfState::new(),
            preview_key: None,
            incomplete_since: None,
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
        let path = match self.project_path.clone() {
            Some(p) => p,
            // безымянный проект: Save-As диалог (п.12 — отказ здесь был
            // «это не должно быть проблемой»)
            None => {
                let Some(p) = rfd::FileDialog::new()
                    .set_title("Сохранить проект")
                    .add_filter("Kinotrain project", &["ktproj"])
                    .set_file_name("untitled.ktproj")
                    .save_file()
                else {
                    return; // пользователь передумал
                };
                let p = if p.extension().is_none() { p.with_extension("ktproj") } else { p };
                self.project_path = Some(p.clone());
                // индекс-БД и тумбы теперь могут жить рядом с проектом
                spawn_index_worker(
                    Some(p.with_extension("ktindex.db")),
                    self.model.project().media.iter().map(|m| (m.id, m.path.clone())).collect(),
                    self.peaks.clone(),
                );
                p
            }
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

    pub fn add_media_clip(&mut self, path: &str) {
        let media_id = MediaId(self.next_media_id());
        if !self.apply(Command::AddMedia { id: media_id, path: path.to_string() }) {
            return;
        }
        self.place_media_at_cursor(media_id);
        // Инкрементально: новый воркер на общий кэш. Пересоздание плеера
        // здесь сбрасывало ВЕСЬ прогретый кэш (п.27) и ломало дельты
        // perf-счётчиков (п.24: «10^19 evictions/с»).
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let resolved = resolve_media_path(base.as_deref(), path);
        match open_source(&resolved) {
            Ok(src) => self.player.add_media(media_id, src),
            Err(e) => self.note(format!("{path}: {e}")),
        }
        self.thumbs.add_media(media_id, resolved.clone());
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
        let t = self.player.video_playhead();
        let q = self.player.preview_quality();
        let key_now = (t.0, (q.width, q.height));
        // композитим/грузим текстуру ТОЛЬКО если кадр мог смениться:
        // другой момент времени, другое качество, или прошлый был неполным
        let need = match self.preview_key {
            Some((pt, pq, was_complete)) => pt != key_now.0 || pq != key_now.1 || !was_complete,
            None => true,
        };
        let mut complete = self.preview_key.map(|k| k.2).unwrap_or(false);
        if need {
            let (rgba, cq, c) = self.player.compose_preview(t);
            complete = c;
            // держим последний ПОЛНЫЙ кадр: неполный композит не затирает
            // картинку (мигание «чёрный кадр ↔ Загрузка», п.14/15)
            if c || self.texture.is_none() {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [cq.width as usize, cq.height as usize],
                    &rgba,
                );
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
            }
            self.preview_key = Some((key_now.0, key_now.1, c));
        }
        if complete {
            self.incomplete_since = None;
        } else if self.incomplete_since.is_none() {
            self.incomplete_since = Some(std::time::Instant::now());
        }
        let (rect, _resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(10, 10, 12));
        if let Some(tex) = &self.texture {
            let scale =
                (rect.width() / q.width as f32).min(rect.height() / q.height as f32).max(0.05);
            let size = egui::vec2(q.width as f32 * scale, q.height as f32 * scale);
            let img_rect = egui::Rect::from_center_size(rect.center(), size);
            painter.image(
                tex.id(),
                img_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            // тонкая рамка кадра
            painter.rect(
                img_rect,
                2.0,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 24)),
                egui::StrokeKind::Outside,
            );
            // скрим — только если неполнота держится >180 мс (v1)
            let show_overlay = self
                .incomplete_since
                .map(|t0| t0.elapsed() > std::time::Duration::from_millis(180))
                .unwrap_or(false);
            if show_overlay {
                painter.rect_filled(img_rect, 2.0, egui::Color32::from_black_alpha(140));
                painter.text(
                    img_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Загрузка…",
                    egui::FontId::proportional(16.0),
                    egui::Color32::from_gray(230),
                );
            }
        }
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

    fn draw_perf_box(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Производительность").strong());
        ui.add_space(2.0);
        let ord = std::sync::atomic::Ordering::Relaxed;
        let q = self.player.preview_quality();
        ui.weak(format!(
            "Кэш: аудио {:.0} МБ / видео {:.0} МБ",
            self.player.cache.audio_bytes() as f64 / 1e6,
            self.player.cache.video_bytes() as f64 / 1e6
        ));
        ui.weak(format!(
            "Underruns: render {} / callback {}",
            self.player.underruns.load(ord),
            self.player.shared.callback_underruns.load(ord)
        ));
        ui.weak(format!("Превью: {}×{}", q.width, q.height));
        let s = &self.model.project().settings;
        ui.weak(format!("Проект: {} fps, {}×{}", s.fps, s.width, s.height));
        ui.weak(format!(
            "Клипов: {} | медиа: {}",
            self.model.project().clips.len(),
            self.model.project().media.len()
        ));
        ui.add_space(4.0);
        self.perf.quality.draw(ui, 96.0);
        ui.add_space(4.0);
        self.perf.decode.draw(ui, 96.0);
        ui.add_space(4.0);
        if ui.button("Диагностика…").clicked() {
            self.perf.diag_open = true;
        }
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
                match &self.hover_hint {
                    Some(h) => ui.weak(h.clone()),
                    None => ui.weak("перетащи медиафайл на окно — клип встанет на курсор"),
                };
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
            // Ctrl+C/Ctrl+V НЕ приходят как key_pressed: egui-winit
            // перехватывает их и шлёт Event::Copy / Event::Paste(..)
            // (регрессия п.21 — «Ctrl+V не работает»)
            copy: i.events.iter().any(|e| matches!(e, egui::Event::Copy))
                || (i.modifiers.ctrl && i.key_pressed(egui::Key::C)),
            paste: i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
                || (i.modifiers.ctrl && i.key_pressed(egui::Key::V)),
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
/// Состояние перф-панели: rolling-графики, дельты счётчиков, журнал проблем.
/// Задержка следующей перерисовки: 60 fps только когда есть что
/// анимировать, в простое — редкий тик (статусбар/watchdog).
/// Чистая функция — тест 20-repaint (п.1).
pub fn repaint_delay(animating: bool) -> std::time::Duration {
    if animating {
        std::time::Duration::from_millis(16)
    } else {
        std::time::Duration::from_millis(250)
    }
}

pub struct PerfState {
    pub quality: crate::graphs::RollingGraph,
    pub decode: crate::graphs::RollingGraph,
    pub churn: crate::graphs::RollingGraph,
    last_sample: std::time::Instant,
    // прошлые кумулятивные значения для дельт
    last_cache: kt_cache::CacheStats,
    last_decode: (u64, u64),
    last_underruns: (u64, u64),
    frames: u32,
    pub diag_open: bool,
    pub log: std::collections::VecDeque<String>,
    last_offline: usize,
}

impl PerfState {
    fn new() -> PerfState {
        use crate::graphs::{Axis, RollingGraph};
        PerfState {
            quality: RollingGraph::new(
                "Качество (FPS / hit%)",
                Axis::Percent,
                vec![
                    ("FPS", egui::Color32::from_rgb(120, 230, 90), 120.0),
                    ("A-hit", egui::Color32::from_rgb(114, 214, 255), 100.0),
                    ("V-hit", egui::Color32::from_rgb(240, 190, 70), 100.0),
                ],
            ),
            decode: RollingGraph::new(
                "Декодер (операций/с)",
                Axis::LinearAuto,
                vec![
                    ("A-dec", egui::Color32::from_rgb(140, 200, 255), 1500.0),
                    ("V-dec", egui::Color32::from_rgb(255, 160, 120), 500.0),
                ],
            ),
            churn: RollingGraph::new(
                "Кэш-чёрн (/с)",
                Axis::LinearAuto,
                vec![
                    ("evict", egui::Color32::from_rgb(255, 120, 120), 2000.0),
                    ("miss", egui::Color32::from_rgb(200, 160, 255), 2000.0),
                ],
            ),
            last_sample: std::time::Instant::now(),
            last_cache: Default::default(),
            last_decode: (0, 0),
            last_underruns: (0, 0),
            frames: 0,
            diag_open: false,
            log: std::collections::VecDeque::new(),
            last_offline: 0,
        }
    }

    fn note(&mut self, msg: String) {
        let now = chrono_like_now();
        self.log.push_back(format!("[{now}] {msg}"));
        while self.log.len() > 400 {
            self.log.pop_front();
        }
    }
}

/// hh:mm:ss без зависимости chrono.
fn chrono_like_now() -> String {
    let secs =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let s = secs % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

impl KtApp {
    /// Сэмплер метрик раз в 500 мс (порт performanceTimer).
    fn sample_perf(&mut self) {
        self.perf.frames += 1;
        let dt = self.perf.last_sample.elapsed().as_secs_f32();
        if dt < 0.5 {
            return;
        }
        self.perf.last_sample = std::time::Instant::now();
        let ord = std::sync::atomic::Ordering::Relaxed;

        // saturating: пересоздание плеера обнуляет счётчики нового кэша,
        // а perf.last_* хранит кумулятивы старого — голое вычитание давало
        // u64-wrap и «10870945131100372992 evictions/с» (п.24)
        let cache = self.player.cache.stats();
        let lc = self.perf.last_cache;
        let d_ah = cache.audio_hits.saturating_sub(lc.audio_hits);
        let d_am = cache.audio_misses.saturating_sub(lc.audio_misses);
        let d_vh = cache.video_hits.saturating_sub(lc.video_hits);
        let d_vm = cache.video_misses.saturating_sub(lc.video_misses);
        let d_ev = cache.evictions.saturating_sub(lc.evictions);
        self.perf.last_cache = cache;

        let a_dec = self.player.decode_stats.audio_decodes.load(ord);
        let v_dec = self.player.decode_stats.video_decodes.load(ord);
        let (la, lv) = self.perf.last_decode;
        self.perf.last_decode = (a_dec, v_dec);

        let fps = self.perf.frames as f32 / dt;
        self.perf.frames = 0;
        let a_hit =
            if d_ah + d_am > 0 { d_ah as f32 / (d_ah + d_am) as f32 * 100.0 } else { 100.0 };
        let v_hit =
            if d_vh + d_vm > 0 { d_vh as f32 / (d_vh + d_vm) as f32 * 100.0 } else { 100.0 };
        self.perf.quality.push(&[fps.min(120.0), a_hit, v_hit]);
        self.perf
            .decode
            .push(&[a_dec.saturating_sub(la) as f32 / dt, v_dec.saturating_sub(lv) as f32 / dt]);
        self.perf.churn.push(&[d_ev as f32 / dt, (d_am + d_vm) as f32 / dt]);

        // журнал проблем
        let ur = self.player.underruns.load(ord);
        let uc = self.player.shared.callback_underruns.load(ord);
        let (lur, luc) = self.perf.last_underruns;
        if ur > lur {
            self.perf.note(format!("render underruns +{} (итого {ur})", ur - lur));
        }
        if uc > luc {
            self.perf.note(format!("callback underruns +{} (итого {uc})", uc - luc));
        }
        self.perf.last_underruns = (ur, uc);
        if d_ev as f32 / dt > 500.0 {
            self.perf.note(format!("всплеск eviction: {:.0}/с — кэшу тесно", d_ev as f32 / dt));
        }
        let offline_now = self.offline.lock().unwrap().len();
        if offline_now != self.perf.last_offline {
            self.perf.note(format!("доступность медиа изменилась: офлайн {offline_now}"));
            self.perf.last_offline = offline_now;
        }
    }

    fn draw_diagnostics(&mut self, ctx: &egui::Context) {
        if !self.perf.diag_open {
            return;
        }
        let mut open = self.perf.diag_open;
        egui::Window::new("Диагностика").open(&mut open).default_width(520.0).show(ctx, |ui| {
            self.perf.churn.draw(ui, 110.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Журнал проблем").strong());
                if ui.button("Очистить").clicked() {
                    self.perf.log.clear();
                }
            });
            egui::ScrollArea::vertical().max_height(240.0).stick_to_bottom(true).show(ui, |ui| {
                for line in &self.perf.log {
                    ui.monospace(line);
                }
                if self.perf.log.is_empty() {
                    ui.weak("проблем не замечено");
                }
            });
        });
        self.perf.diag_open = open;
    }
}

impl eframe::App for KtApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_keys(&ctx);
        self.handle_drops(&ctx);
        self.hover_hint = None;

        // статусбар (низ)
        egui::Panel::bottom("status").show(ui, |ui| {
            self.draw_statusbar(ui);
        });
        // таймлайн (низ, на всю ширину) + плавающий тулбар поверх
        egui::Panel::bottom("timeline").resizable(true).default_size(300.0).show(ui, |ui| {
            let full = ui.available_rect_before_wrap();
            self.timeline_rect = full;
            let strip_h = 34.0;
            let tl_rect = egui::Rect::from_min_max(
                full.min,
                egui::pos2(full.right(), full.bottom() - strip_h),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(tl_rect), |ui| {
                self.draw_timeline(ui);
            });
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                    egui::pos2(full.left(), full.bottom() - strip_h),
                    full.max,
                )),
                |ui| {
                    self.draw_toolbar_strip(ui);
                },
            );
        });
        // полоса времени над таймлайном (порт Timeline time strip)
        egui::Panel::bottom("timestrip").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.weak("Timeline time:");
                let t = self.player.transport.playhead();
                let ms = t.as_ms();
                let fps = self.model.project().settings.fps as i64;
                let frame = t.0 % 90_000 / (90_000 / fps);
                ui.label(
                    egui::RichText::new(format!(
                        "{:02}:{:02}:{:02}.{:03} F{:02}",
                        ms / 3_600_000,
                        ms % 3_600_000 / 60_000,
                        ms % 60_000 / 1000,
                        ms % 1000,
                        frame
                    ))
                    .monospace()
                    .size(16.0)
                    .color(egui::Color32::from_rgb(230, 230, 235)),
                );
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
        });
        // левая колонка: медиа-бин + перфоманс-бокс (порт widget_3)
        egui::Panel::left("left_col").resizable(true).default_size(232.0).show(ui, |ui| {
            ui.add_space(6.0);
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_gray(24))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    self.draw_bin(ui);
                });
            ui.add_space(8.0);
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_gray(24))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    self.draw_perf_box(ui);
                });
        });
        // центр: превью + метры справа, под ними контролы
        egui::CentralPanel::default().show(ui, |ui| {
            let p = self.model.project();
            if let Some(r) = p.region
                && !p.loop_enabled
                && self.player.transport.is_playing()
                && self.player.transport.playhead() >= r.end
                && self.player.transport.playhead() < r.end + Tick(9000)
            {
                self.player.pause();
            }
            egui::Panel::bottom("prev_controls").show(ui, |ui| {
                ui.add_space(3.0);
                self.draw_preview_controls(ui);
                ui.add_space(3.0);
            });
            egui::Panel::right("meters_col").resizable(false).default_size(72.0).show(ui, |ui| {
                self.draw_meters(ui);
            });
            egui::CentralPanel::default().show(ui, |ui| {
                self.draw_preview(ui);
            });
        });

        self.sample_perf();
        self.draw_fx_editors(&ctx);
        self.draw_render_dialog(&ctx);
        self.draw_diagnostics(&ctx);

        // автосейв каждые 4 c (порт autosaveTimer)
        if self.dirty && self.project_path.is_some() && self.last_autosave.elapsed().as_secs() >= 4
        {
            self.last_autosave = std::time::Instant::now();
            let path = self.project_path.clone().unwrap();
            if std::fs::write(&path, self.model.project().to_json()).is_ok() {
                self.dirty = false;
            }
        }

        // Перерисовка ПО ПОТРЕБНОСТИ (п.1: безусловные 60 fps + заливка
        // текстуры каждый кадр грели систему даже с пустым проектом; egui
        // и так перерисовывает мгновенно на любой ввод).
        let animating = self.player.transport.is_playing()
            || self.drag.is_some()
            || self.render_job.is_some()
            || self.incomplete_since.is_some()
            || self.meter.is_active();
        ctx.request_repaint_after(repaint_delay(animating));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // последний проект + финальный сейв при выходе
        if let Some(p) = &self.project_path {
            storage.set_string("last_project", p.to_string_lossy().to_string());
            if self.dirty {
                let _ = std::fs::write(p, self.model.project().to_json());
            }
        }
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
            let mut v = egui::Visuals::dark();
            v.panel_fill = egui::Color32::from_rgb(27, 29, 32); // #1b1d20 старого
            v.window_fill = egui::Color32::from_rgb(30, 32, 36);
            v.extreme_bg_color = egui::Color32::from_rgb(18, 18, 20);
            v.widgets.noninteractive.corner_radius = 4.into();
            v.widgets.inactive.corner_radius = 4.into();
            v.widgets.hovered.corner_radius = 4.into();
            v.widgets.active.corner_radius = 4.into();
            v.selection.bg_fill = egui::Color32::from_rgb(62, 96, 180);
            v.widgets.hovered.bg_fill = egui::Color32::from_rgb(52, 56, 63);
            cc.egui_ctx.set_visuals(v);
            let restored =
                arg.clone().or_else(|| cc.storage.and_then(|s| s.get_string("last_project")));
            let app = match restored {
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

#[cfg(test)]
mod repaint_tests {
    use super::repaint_delay;

    /// Тест 20-repaint (п.1): в простое НЕ 60 fps.
    #[test]
    fn t20_idle_repaint_is_slow_playing_is_60fps() {
        assert_eq!(repaint_delay(true).as_millis(), 16);
        assert!(repaint_delay(false).as_millis() >= 100, "простой = редкий тик");
    }
}
