//! GUI-плеер (M4): превью с композитом из кэша, таймлайн с волноформами из
//! индекс-БД и полосками coverage из кэша, транспорт (Space, клик/драг-скраб,
//! колесо-зум). Редактирование — M5.
//!
//! Инвариант №8: отрисовка таймлайна НЕ порождает декод-запросов — волноформы
//! из peak-пирамиды, полоски из cache.coverage(). Единственные запросы UI —
//! Playback-кадры текущего момента превью.

mod audio_out;

use eframe::egui;
use kt_engine::{Player, source_id};
use kt_cache::BurstKind;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::MediaSource;
use kt_model::{ClipKind, MediaId, Project};
use kt_time::{AUDIO_BURST_FRAMES, Tick};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Peaks = Vec<Vec<(i16, i16)>>; // [level][block] (канал 0)

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

/// Фоновое построение индекс-БД + загрузка peak-пирамид для отрисовки.
fn spawn_index_worker(
    project_path: std::path::PathBuf,
    project: Project,
    out: Arc<Mutex<HashMap<MediaId, Peaks>>>,
) {
    std::thread::Builder::new()
        .name("index-worker".into())
        .spawn(move || {
            let db_path = project_path.with_extension("ktindex.db");
            let Ok(mut db) = kt_media::index::MediaIndexDb::open(&db_path) else { return };
            for m in &project.media {
                let is_synth = m.path.starts_with("synth:");
                let peaks: Option<Peaks> = if is_synth {
                    // синтетика не индексируется в БД (нет файла) — пирамида в памяти
                    open_source(&m.path).ok().map(|mut s| {
                        kt_media::index::PeakPyramid::build(s.as_mut())
                            .levels
                            .into_iter()
                            .map(|chs| chs.into_iter().next().unwrap_or_default())
                            .collect()
                    })
                } else if let Ok((id, status)) = db.ensure_media(std::path::Path::new(&m.path)) {
                    let complete = db.is_complete(id).unwrap_or(false)
                        && status == kt_media::index::MediaEntryStatus::Reused;
                    if !complete && let Ok(mut src) = open_source(&m.path) {
                        // отдельный короткоживущий источник — не декодеры воспроизведения
                        let _ = db.build_audio_peaks(id, src.as_mut());
                        let _ = db.mark_complete(id);
                    }
                    let mut levels = Vec::new();
                    let mut level = 0u32;
                    while let Ok(Some(blocks)) = db.audio_peaks(id, level, 0) {
                        levels.push(blocks);
                        level += 1;
                    }
                    Some(levels)
                } else {
                    None
                };
                if let Some(p) = peaks {
                    out.lock().unwrap().insert(m.id, p);
                }
            }
        })
        .expect("spawn index worker");
}

struct KtApp {
    player: Player,
    _audio: Option<audio_out::AudioOutput>,
    project: Project,
    peaks: Arc<Mutex<HashMap<MediaId, Peaks>>>,
    texture: Option<egui::TextureHandle>,
    // таймлайн-вью
    view_start: f64,    // тики
    ticks_per_px: f64,
    error: Option<String>,
}

impl KtApp {
    fn open(path: &std::path::Path) -> Result<KtApp, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let project = Project::from_json(&text).map_err(|e| e.to_string())?;
        let base = path.parent().unwrap_or(std::path::Path::new("."));
        let mut sources = Vec::new();
        for m in &project.media {
            let resolved = if m.path.starts_with("synth:") || std::path::Path::new(&m.path).is_absolute() {
                m.path.clone()
            } else {
                base.join(&m.path).to_string_lossy().to_string()
            };
            sources.push((m.id, open_source(&resolved)?));
        }
        let (player, ring) = Player::new(project.clone(), sources);
        let audio = match audio_out::start(ring, player.shared.clone(), player.transport.clone()) {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("аудио-вывод недоступен: {e}");
                None
            }
        };
        let peaks = Arc::new(Mutex::new(HashMap::new()));
        spawn_index_worker(path.to_path_buf(), project.clone(), peaks.clone());
        Ok(KtApp {
            player,
            _audio: audio,
            project,
            peaks,
            texture: None,
            view_start: 0.0,
            ticks_per_px: 90_000.0 / 100.0, // 100 px = 1 c
            error: None,
        })
    }

    fn draw_preview(&mut self, ui: &mut egui::Ui) {
        let t = self.player.transport.playhead();
        let (rgba, q, _complete) = self.player.compose_preview(t);
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [q.width as usize, q.height as usize],
            &rgba,
        );
        match &mut self.texture {
            Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
            None => {
                self.texture =
                    Some(ui.ctx().load_texture("preview", image, egui::TextureOptions::LINEAR));
            }
        }
        if let Some(tex) = &self.texture {
            let avail = ui.available_size();
            let scale = (avail.x / q.width as f32).min(avail.y / q.height as f32).max(0.05);
            let size = egui::vec2(q.width as f32 * scale, q.height as f32 * scale);
            ui.centered_and_justified(|ui| {
                ui.add(egui::Image::new((tex.id(), size)));
            });
        }
    }

    fn draw_timeline(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui
            .allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, egui::Color32::from_gray(24));

        // зум колесом вокруг курсора, пан — shift+колесо
        let mut view_start = self.view_start;
        let mut tpp = self.ticks_per_px;
        if let Some(pos) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            let zoom = ui.input(|i| i.zoom_delta());
            if scroll.y != 0.0 && ui.input(|i| i.modifiers.shift) {
                view_start -= scroll.y as f64 * tpp * 4.0;
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

        let x_to_tick = move |x: f32| -> i64 { (view_start + (x - rect.left()) as f64 * tpp) as i64 };
        let tick_to_x = move |t: i64| -> f32 { rect.left() + ((t as f64 - view_start) / tpp) as f32 };

        // линейка секунд
        let ruler_h = 18.0;
        let sec_px = (90_000.0 / self.ticks_per_px) as f32;
        let label_every = (60.0 / sec_px).ceil().max(1.0) as i64;
        let first_sec = (x_to_tick(rect.left()) / 90_000).max(0);
        let last_sec = x_to_tick(rect.right()) / 90_000 + 1;
        for s in first_sec..=last_sec {
            let x = tick_to_x(s * 90_000);
            let major = s % label_every == 0;
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.top() + if major { ruler_h } else { 6.0 })],
                egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
            );
            if major {
                painter.text(
                    egui::pos2(x + 3.0, rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}:{:02}", s / 60, s % 60),
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_gray(140),
                );
            }
        }

        // дорожки: видео сверху, аудио ниже
        let video_lanes =
            self.project.clips.iter().filter(|c| c.kind == ClipKind::Video).map(|c| c.lane + 1).max().unwrap_or(0);
        let audio_lanes =
            self.project.clips.iter().filter(|c| c.kind == ClipKind::Audio).map(|c| c.lane + 1).max().unwrap_or(0);
        let lane_h = ((rect.height() - ruler_h - 8.0) / (video_lanes + audio_lanes).max(1) as f32)
            .clamp(24.0, 72.0);
        let lane_top = |kind: ClipKind, lane: u32| -> f32 {
            let idx = match kind {
                ClipKind::Video => lane,
                ClipKind::Audio => video_lanes + lane,
            };
            rect.top() + ruler_h + 4.0 + idx as f32 * lane_h
        };

        let peaks = self.peaks.lock().unwrap();
        for c in &self.project.clips {
            let x0 = tick_to_x(c.start.0);
            let x1 = tick_to_x(c.start.0 + c.duration.0);
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let top = lane_top(c.kind, c.lane);
            let clip_rect = egui::Rect::from_min_max(
                egui::pos2(x0.max(rect.left()), top),
                egui::pos2(x1.min(rect.right()), top + lane_h - 3.0),
            );
            let (fill, edge) = match c.kind {
                ClipKind::Video => {
                    (egui::Color32::from_rgb(38, 66, 104), egui::Color32::from_rgb(90, 140, 200))
                }
                ClipKind::Audio => {
                    (egui::Color32::from_rgb(40, 84, 60), egui::Color32::from_rgb(90, 180, 120))
                }
            };
            painter.rect(clip_rect, 3.0, fill, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);

            // волноформа из peak-пирамиды (ноль декодов — инвариант №8)
            if c.kind == ClipKind::Audio
                && let Some(levels) = peaks.get(&c.media)
                && !levels.is_empty()
            {
                let frames_per_px = self.ticks_per_px * 48_000.0 / 90_000.0;
                let mut level = 0usize;
                let mut block = kt_media::PEAK_BLOCK_FRAMES as f64;
                while block * 2.0 <= frames_per_px && level + 1 < levels.len() {
                    level += 1;
                    block *= 2.0;
                }
                let blocks = &levels[level];
                let mid = clip_rect.center().y;
                let half = (lane_h - 8.0) * 0.5;
                let src_in_frames = c.source_in.to_audio_frames() as f64;
                let mut x = clip_rect.left();
                while x < clip_rect.right() {
                    let tl0 = x_to_tick(x) - c.start.0;
                    let f0 = src_in_frames + tl0 as f64 * 48_000.0 / 90_000.0;
                    let b = (f0 / block) as usize;
                    if let Some(&(mn, mx)) = blocks.get(b) {
                        let y0 = mid - mx as f32 / 32767.0 * half;
                        let y1 = mid - mn as f32 / 32767.0 * half;
                        painter.line_segment(
                            [egui::pos2(x, y0), egui::pos2(x, y1.max(y0 + 1.0))],
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(140, 220, 160)),
                        );
                    }
                    x += 1.0;
                }
            }

            // полоска coverage: правда из кэша (инвариант №4)
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
                        c.start.0 + Tick::from_audio_frames(run.start * AUDIO_BURST_FRAMES as i64
                            - c.source_in.to_audio_frames()).0,
                        c.start.0 + Tick::from_audio_frames(run.end * AUDIO_BURST_FRAMES as i64
                            - c.source_in.to_audio_frames()).0,
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
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(120, 230, 120)),
                );
            }
        }
        drop(peaks);

        // взаимодействие: клик/драг = seek (скраб)
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let t = x_to_tick(pos.x).max(0);
                self.player.seek(Tick(t));
            }
        }

        // playhead
        let ph = self.player.transport.playhead();
        let x = tick_to_x(ph.0);
        if x >= rect.left() && x <= rect.right() {
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 80, 80)),
            );
        }
    }
}

impl eframe::App for KtApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(err) = &self.error {
            let err = err.clone();
            egui::CentralPanel::default().show(ui, |ui| {
                ui.colored_label(egui::Color32::RED, err);
            });
            return;
        }
        // Space = play/pause
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            if self.player.transport.is_playing() {
                self.player.pause();
            } else {
                self.player.play();
            }
        }

        egui::Panel::top("transport").show(ui, |ui| {
            ui.horizontal(|ui| {
                let playing = self.player.transport.is_playing();
                if ui.button(if playing { "⏸" } else { "▶" }).clicked() {
                    if playing {
                        self.player.pause();
                    } else {
                        self.player.play();
                    }
                }
                if ui.button("⏮").clicked() {
                    self.player.seek(Tick(0));
                }
                let t = self.player.transport.playhead();
                let ms = t.as_ms();
                ui.monospace(format!(
                    "{:02}:{:02}.{:03}",
                    ms / 60_000,
                    ms % 60_000 / 1000,
                    ms % 1000
                ));
                ui.separator();
                ui.weak(format!(
                    "underruns: render {} / callback {}",
                    self.player.underruns.load(std::sync::atomic::Ordering::Relaxed),
                    self.player
                        .shared
                        .callback_underruns
                        .load(std::sync::atomic::Ordering::Relaxed)
                ));
            });
        });

        egui::Panel::bottom("timeline").resizable(true).default_size(240.0).show(ui, |ui| {
            self.draw_timeline(ui);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_preview(ui);
        });

        // во время воспроизведения — свой темп перерисовки
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn main() -> eframe::Result {
    let arg = std::env::args().nth(1);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Kinotrain"),
        ..Default::default()
    };
    eframe::run_native(
        "kinotrain",
        options,
        Box::new(move |_cc| {
            let app = match arg {
                Some(p) => KtApp::open(std::path::Path::new(&p)).unwrap_or_else(|e| KtApp {
                    player: {
                        let (pl, _ring) = Player::new(
                            Project::new(kt_model::ProjectSettings {
                                fps: 30,
                                width: 1280,
                                height: 720,
                                sample_rate: 48000,
                            }),
                            vec![],
                        );
                        pl
                    },
                    _audio: None,
                    project: Project::new(kt_model::ProjectSettings {
                        fps: 30,
                        width: 1280,
                        height: 720,
                        sample_rate: 48000,
                    }),
                    peaks: Arc::new(Mutex::new(HashMap::new())),
                    texture: None,
                    view_start: 0.0,
                    ticks_per_px: 900.0,
                    error: Some(e),
                }),
                None => {
                    let project = Project::new(kt_model::ProjectSettings {
                        fps: 30,
                        width: 1280,
                        height: 720,
                        sample_rate: 48000,
                    });
                    let (player, _ring) = Player::new(project.clone(), vec![]);
                    KtApp {
                        player,
                        _audio: None,
                        project,
                        peaks: Arc::new(Mutex::new(HashMap::new())),
                        texture: None,
                        view_start: 0.0,
                        ticks_per_px: 900.0,
                        error: Some("Запуск: kt-app <проект.ktproj>".into()),
                    }
                }
            };
            Ok(Box::new(app))
        }),
    )
}
