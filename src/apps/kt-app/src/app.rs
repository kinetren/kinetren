//! KtApp: состояние приложения, панели, клавиши, дропы, lifecycle.

use crate::audio_out;
use crate::timeline::DragState;
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
    pub drag: Option<DragState>,
    pub snap_line: Option<i64>,
    pub status: Option<(String, std::time::Instant)>,
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
            audio,
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
            audio,
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
