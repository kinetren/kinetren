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
    pub fn start_render(&mut self) {
        if self.render_job.is_some() {
            self.note("рендер уже идёт");
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
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let range = project.region.map(|r| (r.start, r.end));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
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
                        &cancel,
                        move |p| {
                            let _ = txp.send(RenderMsg::Progress(p));
                        },
                    );
                    let _ = tx.send(RenderMsg::Done(
                        res.map(|s| {
                            format!(
                                "{} кадров видео, {} кадров аудио, underruns {}",
                                s.video_frames, s.audio_frames, s.underruns
                            )
                        })
                        .map_err(|e| e.to_string()),
                    ));
                })
                .expect("spawn render");
        }
        self.render_job = Some(RenderJob {
            rx,
            cancel,
            started: std::time::Instant::now(),
            progress: ExportProgress::default(),
            out_path,
            done: None,
        });
    }

    pub fn draw_render_dialog(&mut self, ctx: &egui::Context) {
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
                let frac = if job.progress.total_video_frames > 0 {
                    job.progress.video_frames as f32 / job.progress.total_video_frames as f32
                } else {
                    0.0
                };
                ui.add(egui::ProgressBar::new(frac).show_percentage());
                let elapsed = job.started.elapsed().as_secs_f64();
                let speed = if elapsed > 0.5 && job.progress.total_video_frames > 0 {
                    let fps_done = job.progress.video_frames as f64 / elapsed;
                    let proj_fps = self.model.project().settings.fps as f64;
                    fps_done / proj_fps
                } else {
                    0.0
                };
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
