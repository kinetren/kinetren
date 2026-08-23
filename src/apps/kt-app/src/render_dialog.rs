//! Рендер-диалог (порт RenderProgressDialog): прогресс/фаза/ETA/скорость,
//! лог, Cancel. Экспорт в фоне через export_mp4_with со СВЕЖИМИ
//! FfmpegSource (не декодеры плеера); диапазон = регион, иначе весь проект.

use crate::app::KtApp;
use crate::util::{resolve_media_path, synth_pattern};
use eframe::egui;
use kt_engine::export::ExportProgress;
use kt_media::MediaSource;
use kt_media::synthetic::SyntheticSource;
use kt_model::{MediaId, Project};
use kt_time::Tick;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub enum RenderMsg {
    Progress(ExportProgress),
    Done(Result<String, String>),
}

pub struct RenderJob {
    pub rx: std::sync::mpsc::Receiver<RenderMsg>,
    pub cancel: Arc<AtomicBool>,
    pub started: std::time::Instant,
    pub progress: ExportProgress,
    pub out_path: String,
    pub done: Option<Result<String, String>>,
    /// Настройки запуска (для панели Output).
    pub opts: kt_engine::export::ExportOptions,
    pub out_res: (u32, u32),
    pub fps: u32,
    /// Скорость ×realtime: min/max/сумма/число замеров (панель Run, v1).
    pub speed_min: f64,
    pub speed_max: f64,
    pub speed_sum: f64,
    pub speed_n: u64,
    pub last_frames: u64,
    pub last_tick: std::time::Instant,
}

/// Диалог НАСТРОЕК рендера (п.5): кодек, качество, битрейт, разрешение.
pub struct RenderSetup {
    pub encoder: Option<String>, // None = Auto
    pub quality: u32,
    pub audio_bitrate: u32,
    pub resolution: Option<(u32, u32)>,
    pub open: bool,
}

impl Default for RenderSetup {
    fn default() -> RenderSetup {
        RenderSetup {
            encoder: None,
            quality: 23,
            audio_bitrate: 192_000,
            resolution: None,
            open: false,
        }
    }
}

type Sources = Vec<(MediaId, Box<dyn MediaSource>)>;

fn open_all(project: &Project, base: Option<&std::path::Path>) -> Result<Sources, String> {
    let mut out = Vec::new();
    for m in &project.media {
        let src: Box<dyn MediaSource> = if let Some(p) = synth_pattern(&m.path) {
            Box::new(SyntheticSource::new(Tick(90_000 * 3600), p))
        } else {
            let resolved = resolve_media_path(base, &m.path);
            Box::new(
                kt_media::ffmpeg::FfmpegSource::open(&resolved)
                    .map_err(|e| format!("{resolved}: {e}"))?,
            )
        };
        out.push((m.id, src));
    }
    Ok(out)
}

impl KtApp {
    /// Кнопка «Рендер»: открыть панель настроек (п.5 — выбор кодека и пр.).
    pub fn start_render(&mut self) {
        if self.render_job.is_some() {
            self.note("рендер уже идёт");
            return;
        }
        self.render_setup.open = true;
    }

    /// Запуск после подтверждения настроек.
    pub fn launch_render(&mut self) {
        if self.render_job.is_some() {
            return;
        }
        let default_name = format!(
            "kp-render-{}.mp4",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter("MP4 Video", &["mp4"])
            .save_file()
        else {
            return;
        };
        let out_path = path.to_string_lossy().to_string();
        let project = self.model.project().clone();
        let proj_res = (project.settings.width, project.settings.height);
        let proj_fps = project.settings.fps;
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let range = project.region.map(|r| (r.start, r.end));
        let cancel = Arc::new(AtomicBool::new(false));
        let opts = kt_engine::export::ExportOptions {
            quality: self.render_setup.quality,
            audio_bitrate: self.render_setup.audio_bitrate,
            force_encoder: self.render_setup.encoder.clone(),
            resolution: self.render_setup.resolution,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let opts_thread = opts.clone();
        {
            let cancel = cancel.clone();
            let out_path = out_path.clone();
            std::thread::Builder::new()
                .name("render".into())
                .spawn(move || {
                    let sources = || open_all(&project, base.as_deref());
                    let (a, v) = match (sources(), sources()) {
                        (Ok(a), Ok(v)) => (a, v),
                        (Err(e), _) | (_, Err(e)) => {
                            let _ = tx.send(RenderMsg::Done(Err(e)));
                            return;
                        }
                    };
                    let txp = tx.clone();
                    let res = kt_engine::export::export_mp4_with(
                        &project,
                        a,
                        v,
                        &out_path,
                        range,
                        &opts_thread,
                        &cancel,
                        move |p| {
                            let _ = txp.send(RenderMsg::Progress(p));
                        },
                    );
                    let _ = tx.send(RenderMsg::Done(
                        res.map(|s| {
                            format!(
                                "{} кадров видео, {} кадров аудио, underruns {}, энкодер {}",
                                s.video_frames, s.audio_frames, s.underruns, s.encoder
                            )
                        })
                        .map_err(|e| e.to_string()),
                    ));
                })
                .expect("spawn render");
        }
        let out_res = opts.resolution.unwrap_or(proj_res);
        self.render_job = Some(RenderJob {
            rx,
            cancel,
            started: std::time::Instant::now(),
            progress: ExportProgress::default(),
            out_path,
            done: None,
            opts,
            out_res,
            fps: proj_fps,
            speed_min: f64::MAX,
            speed_max: 0.0,
            speed_sum: 0.0,
            speed_n: 0,
            last_frames: 0,
            last_tick: std::time::Instant::now(),
        });
        self.render_setup.open = false;
    }

    /// Панель настроек рендера (до запуска).
    pub fn draw_render_setup(&mut self, ctx: &egui::Context) {
        if !self.render_setup.open || self.render_job.is_some() {
            return;
        }
        let mut open = true;
        let mut launch = false;
        let proj = self.model.project().settings.clone();
        let max_src = self.max_source_resolution();
        egui::Window::new("Настройки рендера")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Grid::new("render_setup").num_columns(2).spacing([12.0, 8.0]).show(
                    ui,
                    |ui| {
                        ui.label("Видео-кодек:");
                        let encoders = kt_media::muxer::available_encoders();
                        let sel = self
                            .render_setup
                            .encoder
                            .clone()
                            .unwrap_or_else(|| "Авто (hw-приоритет)".into());
                        egui::ComboBox::from_id_salt("enc").selected_text(&sel).show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.render_setup.encoder,
                                None,
                                "Авто (hw-приоритет)",
                            );
                            for e in &encoders {
                                ui.selectable_value(
                                    &mut self.render_setup.encoder,
                                    Some(e.clone()),
                                    e,
                                );
                            }
                        });
                        ui.end_row();

                        ui.label("Качество (CRF/CQ):");
                        ui.add(egui::Slider::new(&mut self.render_setup.quality, 14..=32));
                        ui.end_row();

                        ui.label("Аудио-битрейт:");
                        egui::ComboBox::from_id_salt("abr")
                            .selected_text(format!(
                                "{} kbps",
                                self.render_setup.audio_bitrate / 1000
                            ))
                            .show_ui(ui, |ui| {
                                for b in [128_000u32, 192_000, 256_000, 320_000] {
                                    ui.selectable_value(
                                        &mut self.render_setup.audio_bitrate,
                                        b,
                                        format!("{} kbps", b / 1000),
                                    );
                                }
                            });
                        ui.end_row();

                        ui.label("Разрешение:");
                        let cur = self
                            .render_setup
                            .resolution
                            .map(|(w, h)| format!("{w}×{h}"))
                            .unwrap_or_else(|| format!("Проект ({}×{})", proj.width, proj.height));
                        egui::ComboBox::from_id_salt("rres").selected_text(cur).show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.render_setup.resolution,
                                None,
                                format!("Проект ({}×{})", proj.width, proj.height),
                            );
                            for r in [(1280u32, 720u32), (1920, 1080), (2560, 1440), (3840, 2160)] {
                                ui.selectable_value(
                                    &mut self.render_setup.resolution,
                                    Some(r),
                                    format!("{}×{}", r.0, r.1),
                                );
                            }
                            if let Some((mw, mh)) = max_src {
                                ui.selectable_value(
                                    &mut self.render_setup.resolution,
                                    Some((mw, mh)),
                                    format!("Max ({mw}×{mh})"),
                                );
                            }
                        });
                        ui.end_row();

                        ui.label("Аудио:");
                        ui.weak("AAC, 48 kHz stereo");
                        ui.end_row();
                        ui.label("Контейнер:");
                        ui.weak("MP4 (+faststart)");
                        ui.end_row();
                    },
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Рендер…").clicked() {
                        launch = true;
                    }
                    if ui.button("Отмена").clicked() {
                        self.render_setup.open = false;
                    }
                });
            });
        if !open {
            self.render_setup.open = false;
        }
        if launch {
            self.launch_render();
        }
    }

    pub fn draw_render_dialog(&mut self, ctx: &egui::Context) {
        self.draw_render_setup(ctx);
        let Some(job) = &mut self.render_job else { return };
        while let Ok(msg) = job.rx.try_recv() {
            match msg {
                RenderMsg::Progress(p) => job.progress = p,
                RenderMsg::Done(r) => job.done = Some(r),
            }
        }
        let mut close = false;
        egui::Window::new("Рендер в MP4")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Файл: {}", job.out_path));
                ui.weak(format!(
                    "{}×{} @ {} fps  |  {}  CRF {}  |  AAC {} kbps  |  MP4 +faststart",
                    job.out_res.0,
                    job.out_res.1,
                    job.fps,
                    job.opts.force_encoder.as_deref().unwrap_or("авто-энкодер"),
                    job.opts.quality,
                    job.opts.audio_bitrate / 1000,
                ));
                let frac = if job.progress.total_video_frames > 0 {
                    job.progress.video_frames as f32 / job.progress.total_video_frames as f32
                } else {
                    0.0
                };
                ui.add(egui::ProgressBar::new(frac).show_percentage());
                let elapsed = job.started.elapsed().as_secs_f64();
                let proj_fps = job.fps.max(1) as f64;
                let speed = if elapsed > 0.5 && job.progress.total_video_frames > 0 {
                    job.progress.video_frames as f64 / elapsed / proj_fps
                } else {
                    0.0
                };
                // мгновенная скорость по дельте кадров (панель Speed v1)
                let dt = job.last_tick.elapsed().as_secs_f64();
                if dt >= 0.5 && job.done.is_none() {
                    let cur =
                        (job.progress.video_frames - job.last_frames) as f64 / dt / proj_fps;
                    job.last_frames = job.progress.video_frames;
                    job.last_tick = std::time::Instant::now();
                    if cur > 0.0 {
                        job.speed_min = job.speed_min.min(cur);
                        job.speed_max = job.speed_max.max(cur);
                        job.speed_sum += cur;
                        job.speed_n += 1;
                    }
                }
                if job.speed_n > 0 {
                    ui.weak(format!(
                        "Скорость: min {:.2}×  avg {:.2}×  cur {:.2}×  max {:.2}×",
                        if job.speed_min == f64::MAX { 0.0 } else { job.speed_min },
                        job.speed_sum / job.speed_n as f64,
                        speed,
                        job.speed_max,
                    ));
                }
                let eta = if frac > 0.01 && job.done.is_none() {
                    elapsed / frac as f64 * (1.0 - frac as f64)
                } else {
                    0.0
                };
                ui.label(format!(
                    "Видео {}/{}  Аудио {}/{}  |  {:.2}× realtime  |  прошло {:.0}с, осталось ~{:.0}с",
                    job.progress.video_frames,
                    job.progress.total_video_frames,
                    job.progress.audio_frames,
                    job.progress.total_audio_frames,
                    speed,
                    elapsed,
                    eta
                ));
                ui.add_space(4.0);
                match &job.done {
                    None => {
                        if ui.button("Отмена").clicked() {
                            job.cancel.store(true, std::sync::atomic::Ordering::Release);
                        }
                    }
                    Some(Ok(msg)) => {
                        ui.colored_label(egui::Color32::from_rgb(120, 230, 120), format!("Готово: {msg}"));
                        if ui.button("OK").clicked() {
                            close = true;
                        }
                    }
                    Some(Err(e)) => {
                        ui.colored_label(egui::Color32::from_rgb(255, 120, 120), format!("Ошибка: {e}"));
                        if ui.button("Закрыть").clicked() {
                            close = true;
                        }
                    }
                }
            });
        if close {
            self.render_job = None;
        }
    }
}
