//! Рендер-диалог (порт RenderProgressDialog): прогресс/фаза/ETA/скорость,
//! лог, Cancel. Экспорт в фоне через export_mp4_with со СВЕЖИМИ
//! FfmpegSource (не декодеры плеера); диапазон = регион, иначе весь проект.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::util::{resolve_media_path, synth_pattern};
use eframe::egui;
use kt_engine::export::ExportProgress;
use kt_media::MediaSource;
use kt_media::synthetic::SyntheticSource;
use kt_model::{MediaId, Project};
use kt_time::Tick;

pub struct RenderJob {
    /// Общий каркас фоновой ЗАДАЧИ (Р.49, п.367): канал, отмена, пауза и
    /// гашение потока при `Drop`. Рендер — эталон, из которого каркас и
    /// выделен; всё, что ниже, — его собственные числа для панели Run.
    pub job: crate::bg::BgJob<ExportProgress, Result<kt_engine::export::ExportStats, String>>,
    pub progress: ExportProgress,
    pub out_path: String,
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
    /// Исходники, отрендеренные заглушкой (Р.33 п.10).
    pub missing: Vec<String>,
}

/// Диалог НАСТРОЕК рендера (п.5): кодек, качество, битрейт, разрешение.
pub struct RenderSetup {
    pub encoder: Option<String>, // None = Auto
    pub quality: u32,
    /// Some = режим целевого битрейта видео (бит/с) вместо CRF (п.9).
    pub video_bitrate: Option<u32>,
    pub audio_bitrate: u32,
    pub resolution: Option<(u32, u32)>,
    pub open: bool,
    /// Недоступные исходники на момент открытия настроек: показываются
    /// предупреждением ДО запуска, а не сюрпризом в готовом файле.
    pub missing: Vec<String>,
}

impl Default for RenderSetup {
    fn default() -> RenderSetup {
        RenderSetup {
            encoder: None,
            quality: 23,
            video_bitrate: None,
            audio_bitrate: 192_000,
            resolution: None,
            open: false,
            missing: Vec::new(),
        }
    }
}

type Sources = Vec<(MediaId, Box<dyn MediaSource>)>;

/// Источники для рендера. Недоступный файл НЕ отменяет экспорт (Р.33
/// п.10): вместо него встаёт плейсхолдер, а имя попадает в список —
/// пользователь предупреждён до старта и видит счёт после.
pub(crate) fn open_all(
    project: &Project,
    base: Option<&std::path::Path>,
) -> (Sources, Vec<String>) {
    let mut out = Vec::new();
    let mut missing = Vec::new();
    for m in &project.media {
        let src: Box<dyn MediaSource> = if let Some(p) = synth_pattern(&m.path) {
            Box::new(SyntheticSource::new(Tick(90_000 * 3600), p))
        } else {
            let resolved = resolve_media_path(base, &m.path);
            match kt_media::ffmpeg::FfmpegSource::open(&resolved) {
                Ok(s) => Box::new(s),
                Err(e) => {
                    let label = crate::util::media_label(&m.path);
                    missing.push(format!("{label}: {e}"));
                    Box::new(kt_media::placeholder::PlaceholderSource::new(label))
                }
            }
        };
        out.push((m.id, src));
    }
    (out, missing)
}

/// Недоступные исходники проекта — для предупреждения ДО старта рендера.
pub(crate) fn missing_sources(project: &Project, base: Option<&std::path::Path>) -> Vec<String> {
    project
        .media
        .iter()
        .filter(|m| synth_pattern(&m.path).is_none())
        .filter(|m| !std::path::Path::new(&resolve_media_path(base, &m.path)).exists())
        .map(|m| crate::util::media_label(&m.path))
        .collect()
}

/// Кусок имени файла из пользовательского текста: пробелы — дефисом,
/// запрещённые файловой системой символы — подчёркиванием (Р.33 п.9).
fn sanitize_name_part(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            c if c.is_whitespace() => '-',
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect()
}

/// Имя выходного файла по шаблону пользователя (Р.33 п.9):
/// `kt-<проект>-<регион>-<длина в секундах>s.mp4`. Проекта нет — `untitled`;
/// регион не совпал с именованной меткой (или имя пустое) — сегмент региона
/// опускается, чтобы имя не врало про содержимое.
pub(crate) fn render_file_name(
    project_stem: Option<&str>,
    region_name: Option<&str>,
    len_ticks: i64,
) -> String {
    let mut name = String::from("kt-");
    let proj = project_stem.map(sanitize_name_part).filter(|s| !s.is_empty());
    name.push_str(&proj.unwrap_or_else(|| "untitled".into()));
    if let Some(r) = region_name.map(sanitize_name_part).filter(|s| !s.is_empty()) {
        name.push('-');
        name.push_str(&r);
    }
    let secs = (len_ticks.max(0) as f64 / 90_000.0).round() as i64;
    format!("{name}-{secs}s.mp4")
}

impl KtApp {
    /// Имя, подставляемое в диалог сохранения рендера.
    pub(crate) fn default_render_name(&self) -> String {
        let project = self.model.project();
        let (start, end) = project.region.map_or_else(
            || (Tick::ZERO, kt_engine::export::project_range(project)),
            |r| (r.start, r.end),
        );
        // имя берём только у метки-диапазона, СОВПАДАЮЩЕЙ с регионом: иначе
        // в имени файла оказался бы чужой кусок таймлайна
        let region_name = project
            .markers
            .iter()
            .find(|m| m.is_range() && m.t == start && m.end() == end && !m.name.is_empty())
            .map(|m| m.name.clone());
        let stem = self
            .project_path
            .as_deref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string());
        render_file_name(stem.as_deref(), region_name.as_deref(), (end - start).0)
    }

    /// Кнопка «Рендер»: открыть панель настроек (п.5 — выбор кодека и пр.).
    pub fn start_render(&mut self) {
        if self.render_job.is_some() {
            self.note(tr(k::render::ALREADY_RUNNING));
            return;
        }
        let base = self.project_path.as_deref().and_then(|p| p.parent());
        self.render_setup.missing = missing_sources(self.model.project(), base);
        self.render_setup.open = true;
    }

    /// Запуск после подтверждения настроек.
    pub fn launch_render(&mut self) {
        if self.render_job.is_some() {
            return;
        }
        let default_name = self.default_render_name();
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
        let opts = kt_engine::export::ExportOptions {
            quality: self.render_setup.quality,
            video_bitrate: self.render_setup.video_bitrate,
            audio_bitrate: self.render_setup.audio_bitrate,
            force_encoder: self.render_setup.encoder.clone(),
            resolution: self.render_setup.resolution,
        };
        let opts_thread = opts.clone();
        let out_path_thread = out_path.clone();
        let job = crate::bg::BgJob::spawn("render", move |ctl, send| {
            let send_p = move |p| send.send(p);
            let (a, _) = open_all(&project, base.as_deref());
            let (v, _) = open_all(&project, base.as_deref());
            // Р.29: GPU-рендер по умолчанию — свой headless-девайс на
            // джоб (очередь UI не делит время с экспортом); движка нет
            // → CPU (для 2D это полноценный путь)
            let backends = kt_engine::ExportBackends { video_gpu: crate::app::export_video_gpu() };
            kt_engine::export::export_mp4_with_backends(
                &project,
                a,
                v,
                &out_path_thread,
                range,
                &opts_thread,
                &backends,
                ctl.cancel_flag(),
                ctl.pause_flag(),
                send_p,
            )
            .map_err(|e| e.to_string())
        });
        let out_res = opts.resolution.unwrap_or(proj_res);
        self.render_job = Some(RenderJob {
            job,
            progress: ExportProgress::default(),
            out_path,
            opts,
            out_res,
            fps: proj_fps,
            speed_min: f64::MAX,
            speed_max: 0.0,
            speed_sum: 0.0,
            speed_n: 0,
            last_frames: 0,
            last_tick: std::time::Instant::now(),
            missing: self.render_setup.missing.clone(),
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
        let max_src = self.max_source_resolution_cached();
        egui::Window::new(tr(k::render::SETTINGS_TITLE))
            .id(egui::Id::new("kt_render_settings"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Grid::new("render_setup").num_columns(2).spacing([12.0, 8.0]).show(
                    ui,
                    |ui| {
                        ui.label(tr(k::render::VIDEO_CODEC));
                        let encoders = kt_media::muxer::available_encoders();
                        let sel = self
                            .render_setup
                            .encoder
                            .clone()
                            .unwrap_or_else(|| tr(k::render::ENCODER_AUTO).to_string());
                        egui::ComboBox::from_id_salt("enc").selected_text(&sel).show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.render_setup.encoder,
                                None,
                                tr(k::render::ENCODER_AUTO),
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

                        ui.label(tr(k::render::QUALITY));
                        ui.horizontal(|ui| {
                            let mut by_bitrate = self.render_setup.video_bitrate.is_some();
                            if ui.selectable_label(!by_bitrate, "CRF").clicked() {
                                by_bitrate = false;
                                self.render_setup.video_bitrate = None;
                            }
                            if ui.selectable_label(by_bitrate, tr(k::render::BITRATE)).clicked()
                                && self.render_setup.video_bitrate.is_none()
                            {
                                self.render_setup.video_bitrate = Some(8_000_000);
                            }
                            match &mut self.render_setup.video_bitrate {
                                Some(bps) => {
                                    let mut mbps = *bps as f32 / 1_000_000.0;
                                    if ui
                                        .add(
                                            egui::Slider::new(&mut mbps, 0.5..=80.0)
                                                .suffix(tr(k::render::MBPS_SUFFIX))
                                                .logarithmic(true),
                                        )
                                        .changed()
                                    {
                                        *bps = (mbps * 1_000_000.0) as u32;
                                    }
                                }
                                None => {
                                    ui.add(egui::Slider::new(
                                        &mut self.render_setup.quality,
                                        14..=32,
                                    ));
                                }
                            }
                        });
                        ui.end_row();

                        ui.label(tr(k::render::AUDIO_BITRATE));
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

                        ui.label(tr(k::render::RESOLUTION));
                        let cur = self
                            .render_setup
                            .resolution
                            .map(|(w, h)| format!("{w}×{h}"))
                            .unwrap_or_else(|| {
                                crate::tf!(k::render::RES_PROJECT, proj.width, proj.height)
                            });
                        egui::ComboBox::from_id_salt("rres").selected_text(cur).show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.render_setup.resolution,
                                None,
                                crate::tf!(k::render::RES_PROJECT, proj.width, proj.height),
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

                        ui.label(tr(k::render::AUDIO));
                        ui.weak("AAC, 48 kHz stereo");
                        ui.end_row();
                        ui.label(tr(k::render::CONTAINER));
                        ui.weak("MP4 (+faststart)");
                        ui.end_row();
                    },
                );
                if !self.render_setup.missing.is_empty() {
                    ui.add_space(4.0);
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 190, 120),
                        crate::tf!(
                            k::render::MISSING_WARN,
                            self.render_setup.missing.len(),
                            self.render_setup.missing.join(", ")
                        ),
                    );
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(tr(k::render::START)).clicked() {
                        launch = true;
                    }
                    if ui.button(tr(k::render::CANCEL)).clicked() {
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
        if job.job.poll()
            && let Some(p) = job.job.progress
        {
            job.progress = p;
        }
        let mut close = false;
        egui::Window::new(tr(k::render::PROGRESS_TITLE))
            .id(egui::Id::new("kt_render_progress"))
            .collapsible(false)
            .resizable(true)
            .default_size([560.0, 300.0])
            .default_pos(ctx.content_rect().center() - egui::vec2(280.0, 150.0))
            .show(ctx, |ui| {
                ui.label(crate::tf!(k::render::FILE, job.out_path));
                ui.weak(format!(
                    "{}×{} @ {} fps  |  {}  CRF {}  |  AAC {} kbps  |  MP4 +faststart",
                    job.out_res.0,
                    job.out_res.1,
                    job.fps,
                    job.opts.force_encoder.as_deref().unwrap_or(tr(k::render::ENCODER_DEFAULT)),
                    job.opts.quality,
                    job.opts.audio_bitrate / 1000,
                ));
                let frac = if job.progress.total_video_frames > 0 {
                    job.progress.video_frames as f32 / job.progress.total_video_frames as f32
                } else {
                    0.0
                };
                let phase = match job.progress.phase {
                    kt_engine::export::ExportPhase::Video => tr(k::render::PHASE_VIDEO),
                    kt_engine::export::ExportPhase::Audio => tr(k::render::PHASE_AUDIO),
                    kt_engine::export::ExportPhase::Finalize => tr(k::render::PHASE_FINALIZE),
                    kt_engine::export::ExportPhase::Done => tr(k::render::PHASE_DONE),
                };
                ui.horizontal(|ui| {
                    ui.label(tr(k::render::PHASE));
                    ui.strong(phase);
                });
                ui.add(egui::ProgressBar::new(frac).show_percentage());
                let elapsed = job.job.started.elapsed().as_secs_f64();
                let proj_fps = job.fps.max(1) as f64;
                let speed = if elapsed > 0.5 && job.progress.total_video_frames > 0 {
                    job.progress.video_frames as f64 / elapsed / proj_fps
                } else {
                    0.0
                };
                // мгновенная скорость по дельте кадров (панель Speed v1)
                let dt = job.last_tick.elapsed().as_secs_f64();
                if dt >= 0.5 && job.job.done.is_none() {
                    let cur = (job.progress.video_frames - job.last_frames) as f64 / dt / proj_fps;
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
                    ui.weak(crate::tf!(
                        k::render::SPEED,
                        format!(
                            "{:.2}",
                            if job.speed_min == f64::MAX { 0.0 } else { job.speed_min }
                        ),
                        format!("{:.2}", job.speed_sum / job.speed_n as f64),
                        format!("{speed:.2}"),
                        format!("{:.2}", job.speed_max),
                    ));
                }
                let eta = if frac > 0.01 && job.job.done.is_none() {
                    elapsed / frac as f64 * (1.0 - frac as f64)
                } else {
                    0.0
                };
                ui.label(crate::tf!(
                    k::render::COUNTERS,
                    job.progress.video_frames,
                    job.progress.total_video_frames,
                    job.progress.audio_frames,
                    job.progress.total_audio_frames,
                    format!("{speed:.2}"),
                    format!("{elapsed:.0}"),
                    format!("{eta:.0}"),
                ));
                ui.add_space(4.0);
                match &job.job.done {
                    None => {
                        ui.horizontal(|ui| {
                            let paused = job.job.is_paused();
                            if ui
                                .button(if paused {
                                    tr(k::render::RESUME)
                                } else {
                                    tr(k::render::PAUSE)
                                })
                                .clicked()
                            {
                                job.job.set_paused(!paused);
                            }
                            if paused {
                                ui.colored_label(
                                    egui::Color32::from_rgb(255, 200, 100),
                                    tr(k::render::PAUSED),
                                );
                            }
                            if ui.button(tr(k::render::CANCEL)).clicked() {
                                job.job.cancel();
                            }
                        });
                    }
                    Some(Ok(stats)) => {
                        let msg = crate::tf!(
                            k::render::SUMMARY,
                            stats.video_frames,
                            stats.audio_frames,
                            stats.underruns,
                            stats.encoder
                        );
                        ui.colored_label(
                            egui::Color32::from_rgb(120, 230, 120),
                            crate::tf!(k::render::DONE_MSG, msg),
                        );
                        // неполный рендер больше не выглядит успехом (Р.33
                        // п.10): заглушки и потерянные слои названы прямо
                        // в сводке, а не только в выводе kt-cli
                        if !job.missing.is_empty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 190, 120),
                                crate::tf!(
                                    k::render::MISSING_AFTER,
                                    job.missing.len(),
                                    job.missing.join(", ")
                                ),
                            );
                        }
                        // Громкость посчитана на том же проходе (Р.49,
                        // п.371). Показывается СПРАВОЧНО: кнопки
                        // «нормализовать» тут нет намеренно — цель
                        // нормализации (вещание −23, стриминг −14, кино
                        // −27) выбирает человек, а не программа, и
                        // применять её задним числом к уже записанному
                        // файлу значило бы перекодировать его ещё раз.
                        if let Some(l) = stats.loudness
                            && let Some(lufs) = l.lufs
                        {
                            ui.label(crate::tf!(
                                k::render::LOUDNESS,
                                format!("{lufs:.1}"),
                                format!("{:.1}", l.dbtp),
                                format!("{:.1}", l.lra)
                            ));
                            let delta = -23.0 - lufs;
                            ui.weak(crate::tf!(
                                k::render::LOUDNESS_TO_TARGET,
                                format!("{delta:+.1}")
                            ));
                        }
                        if stats.missing_video_layers > 0 {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 190, 120),
                                crate::tf!(k::render::MISSING_LAYERS, stats.missing_video_layers),
                            );
                        }
                        if ui.button("OK").clicked() {
                            close = true;
                        }
                    }
                    Some(Err(e)) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 120, 120),
                            crate::tf!(k::render::ERROR_MSG, e),
                        );
                        if ui.button(tr(k::render::CLOSE)).clicked() {
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
