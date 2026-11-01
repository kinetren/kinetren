//! Render dialog (port of RenderProgressDialog): progress/phase/ETA/speed,
//! log, Cancel. Export in the background via export_mp4_with with FRESH
//! FfmpegSource (not the player's decoders); range = region, otherwise the whole project.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::util::resolve_media_path;
use eframe::egui;
use kt_engine::export::ExportProgress;
use kt_media::MediaSource;
use kt_model::{MediaId, Project};
use kt_time::Tick;

/// What the render job's thread sends (render-window-v1-parity, D4): the engine's events and the
/// backend the job chose, in arrival order.
pub enum RenderMsg {
    Engine(kt_engine::export::ExportEvent),
    Backend(BackendLine),
}

/// The video backend of the run: the GPU with the adapter's name, or the CPU with the reason.
#[derive(Debug, Clone, PartialEq)]
pub enum BackendLine {
    Gpu(String),
    Cpu(String),
}

/// One line of the render log (D9): a fact, written in English to the file and shown translated.
#[derive(Debug, Clone, PartialEq)]
pub enum LogLine {
    Engine(kt_engine::export::ExportLog),
    Opened(String),
    Backend(BackendLine),
    Pause,
    Resume,
    Cancel,
    Failed(String),
}

impl LogLine {
    /// For the file (the journal's rule: the file is for a report — English).
    pub fn english(&self) -> String {
        use kt_engine::export::{ExportLog, ExportPhase};
        match self {
            LogLine::Engine(ExportLog::NotOpened { name, code }) => {
                format!("encoder {name} did not open (code {code})")
            }
            LogLine::Engine(ExportLog::Skipped { name, reason }) => format!("encoder {name} skipped: {reason}"),
            LogLine::Engine(ExportLog::Placeholder { label }) => format!("placeholder rendered for {label}"),
            LogLine::Engine(ExportLog::Phase(ExportPhase::Video)) => "phase: video".into(),
            LogLine::Engine(ExportLog::Phase(ExportPhase::Audio)) => "phase: audio".into(),
            LogLine::Engine(ExportLog::Phase(ExportPhase::Finalize) | ExportLog::Finalizing) => {
                "finalizing the file".into()
            }
            LogLine::Engine(ExportLog::Phase(ExportPhase::Done)) => "done".into(),
            LogLine::Engine(ExportLog::Error(e)) => format!("error: {e}"),
            LogLine::Opened(n) => format!("encoder {n} opened"),
            LogLine::Backend(BackendLine::Gpu(a)) => format!("video on the GPU: {a}"),
            LogLine::Backend(BackendLine::Cpu(r)) => format!("video on the CPU: {r}"),
            LogLine::Pause => "paused".into(),
            LogLine::Resume => "resumed".into(),
            LogLine::Cancel => "cancel requested".into(),
            LogLine::Failed(e) => format!("render failed: {e}"),
        }
    }

    /// For the window, in the person's language.
    pub fn shown(&self) -> String {
        use kt_engine::export::{ExportLog, ExportPhase};
        match self {
            LogLine::Engine(ExportLog::NotOpened { name, code }) => {
                crate::tf!(k::render::LOG_NOT_OPENED, name, code)
            }
            // the reason is the export's own fact, in English like an error text
            LogLine::Engine(ExportLog::Skipped { name, reason }) => crate::tf!(k::render::LOG_SKIPPED, name, reason),
            LogLine::Engine(ExportLog::Placeholder { label }) => crate::tf!(k::render::LOG_PLACEHOLDER, label),
            LogLine::Engine(ExportLog::Phase(ExportPhase::Video)) => tr(k::render::PHASE_VIDEO).into(),
            LogLine::Engine(ExportLog::Phase(ExportPhase::Audio)) => tr(k::render::PHASE_AUDIO).into(),
            LogLine::Engine(ExportLog::Phase(ExportPhase::Finalize) | ExportLog::Finalizing) => {
                tr(k::render::PHASE_FINALIZE).into()
            }
            LogLine::Engine(ExportLog::Phase(ExportPhase::Done)) => tr(k::render::PHASE_DONE).into(),
            LogLine::Engine(ExportLog::Error(e)) => crate::tf!(k::render::ERROR_MSG, e),
            LogLine::Opened(n) => crate::tf!(k::render::LOG_OPENED, n),
            LogLine::Backend(b) => backend_text(b),
            LogLine::Pause => tr(k::render::PAUSED).into(),
            LogLine::Resume => tr(k::render::LOG_RESUMED).into(),
            LogLine::Cancel => tr(k::render::LOG_CANCEL).into(),
            LogLine::Failed(e) => crate::tf!(k::render::ERROR_MSG, e),
        }
    }
}

fn backend_text(b: &BackendLine) -> String {
    match b {
        BackendLine::Gpu(a) => crate::tf!(k::render::BACKEND_GPU, a),
        BackendLine::Cpu(r) => crate::tf!(k::render::BACKEND_CPU, r),
    }
}

/// How many lines the window keeps (v1's `setMaximumBlockCount(800)`).
pub const LOG_LINES: usize = 800;

/// "0:42" / "1:02:03".
pub fn clock_text(d: std::time::Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// A size the way a person reads it: KB under a megabyte ("0.0 MB" said nothing about a small file,
/// live scenario `render`), MB with one decimal above.
pub fn size_text(bytes: u64) -> String {
    if bytes < 1024 * 1024 {
        crate::tf!(k::render::SIZE_KB, bytes.div_ceil(1024))
    } else {
        crate::tf!(k::render::SIZE_MB, format!("{:.1}", bytes as f64 / (1024.0 * 1024.0)))
    }
}

/// A bitrate: kbit/s under a megabit, Mbit/s with one decimal above.
pub fn bitrate_text(bps: f64) -> String {
    if bps < 1e6 {
        crate::tf!(k::render::KBPS_VALUE, format!("{:.0}", bps / 1e3))
    } else {
        crate::tf!(k::render::BITRATE_VALUE, format!("{:.1}", bps / 1e6))
    }
}

/// The quality setting under the name the opened encoder uses (F-EXP-004 trick 3): CRF for x264,
/// CQ for NVENC, QP for AMF/VAAPI, global quality for QSV; the target bitrate when one was set.
pub fn quality_text(r: &kt_media::muxer::OpenReport, opts: &kt_engine::export::ExportOptions) -> String {
    if let Some(bps) = opts.video_bitrate {
        return bitrate_text(bps as f64);
    }
    for (key, label) in [("crf", "CRF"), ("cq", "CQ"), ("qp", "QP"), ("qp_i", "QP"), ("global_quality", "global quality"), ("quality", "quality")] {
        if let Some((_, v)) = r.options.iter().find(|(k, _)| k == key) {
            return format!("{label} {v}");
        }
    }
    String::new()
}

/// The render log file (D9): `render.log`, the previous run's moved to `render.log.1`.
pub struct LogFile {
    pub path: std::path::PathBuf,
    out: Option<std::io::BufWriter<std::fs::File>>,
    /// The file could not be written: the log is only in the window.
    pub failed: bool,
}

impl LogFile {
    pub fn open(dir: &std::path::Path) -> LogFile {
        let path = dir.join("render.log");
        let out = (|| {
            std::fs::create_dir_all(dir)?;
            if path.exists() {
                std::fs::rename(&path, dir.join("render.log.1"))?;
            }
            std::fs::File::create(&path).map(std::io::BufWriter::new)
        })();
        match out {
            Ok(f) => LogFile { path, out: Some(f), failed: false },
            Err(e) => {
                crate::journal::log("RENDER", &format!("render log not written ({}): {e}", path.display()));
                LogFile { path, out: None, failed: true }
            }
        }
    }

    fn write(&mut self, stamp: std::time::Duration, line: &str, flush: bool) {
        use std::io::Write;
        let Some(out) = self.out.as_mut() else { return };
        let ok = writeln!(out, "[{}] {line}", clock_text(stamp)).is_ok() && (!flush || out.flush().is_ok());
        if !ok {
            self.out = None;
            self.failed = true;
            crate::journal::log("RENDER", &format!("render log write failed: {}", self.path.display()));
        }
    }
}

pub struct RenderJob {
    /// Common frame of a background TASK (R.49, item 367): channel, cancel, pause and
    /// stopping the thread on `Drop`. Render is the reference from which the frame was
    /// extracted; everything below is its own numbers for the Run panel.
    pub job: crate::bg::BgJob<RenderMsg, Result<kt_engine::export::ExportStats, String>>,
    /// The result, moved here when it arrives (the moment the clocks stop).
    pub result: Option<Result<kt_engine::export::ExportStats, String>>,
    pub progress: ExportProgress,
    /// The encoder that opened (before the first frame) — `None` while it is being chosen.
    pub report: Option<kt_media::muxer::OpenReport>,
    pub backend: Option<BackendLine>,
    pub out_path: String,
    /// Launch settings (for the Output panel).
    pub opts: kt_engine::export::ExportOptions,
    pub out_res: (u32, u32),
    pub fps: u32,
    /// Sources rendered as a stub (R.33 item 10).
    pub missing: Vec<String>,
    /// THE UI OWNS EVERY CLOCK (D3): `clock` is the seam a test drives; `finished_at` stops
    /// elapsed, the speed and the final block at the moment the result arrived (T12).
    pub clock: fn() -> std::time::Instant,
    pub started: std::time::Instant,
    pub finished_at: Option<std::time::Instant>,
    /// The last speed sample: (time, video frames, video bytes).
    sample: (std::time::Instant, u64, u64),
    pub speed_min: f64,
    pub speed_max: f64,
    pub speed_sum: f64,
    pub speed_n: u64,
    pub speed_cur: f64,
    /// The video bitrate of the last sample, bit/s of OUTPUT (a ratio of counts, no clock).
    pub bitrate_cur: Option<f64>,
    pub graph: crate::graphs::RollingGraph,
    /// The last `LOG_LINES` lines with their elapsed stamps (D9).
    pub log: std::collections::VecDeque<(std::time::Duration, LogLine)>,
    pub log_file: LogFile,
    /// The file's size, read once at the finish.
    pub final_size: Option<u64>,
    /// Where Cancel stood during the run: the closing buttons must not land there (trick 3).
    pub cancel_rect: Option<egui::Rect>,
}

impl RenderJob {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        job: crate::bg::BgJob<RenderMsg, Result<kt_engine::export::ExportStats, String>>,
        out_path: String,
        opts: kt_engine::export::ExportOptions,
        out_res: (u32, u32),
        fps: u32,
        missing: Vec<String>,
        log_dir: &std::path::Path,
        clock: fn() -> std::time::Instant,
    ) -> RenderJob {
        let now = clock();
        RenderJob {
            job,
            result: None,
            progress: ExportProgress::default(),
            report: None,
            backend: None,
            out_path,
            opts,
            out_res,
            fps: fps.max(1),
            missing,
            clock,
            started: now,
            finished_at: None,
            sample: (now, 0, 0),
            speed_min: f64::MAX,
            speed_max: 0.0,
            speed_sum: 0.0,
            speed_n: 0,
            speed_cur: 0.0,
            bitrate_cur: None,
            graph: crate::graphs::RollingGraph::new(
                k::render::SPEED_GRAPH,
                crate::graphs::Axis::LinearAuto,
                vec![("×", egui::Color32::from_rgb(120, 200, 255), 4.0)],
            ),
            log: std::collections::VecDeque::with_capacity(LOG_LINES),
            log_file: LogFile::open(log_dir),
            final_size: None,
            cancel_rect: None,
        }
    }

    /// Elapsed time of the run — stopped at the finish.
    pub fn elapsed(&self) -> std::time::Duration {
        self.finished_at.unwrap_or_else(self.clock).saturating_duration_since(self.started)
    }

    pub fn is_running(&self) -> bool {
        self.result.is_none()
    }

    pub fn push_log(&mut self, line: LogLine) {
        let stamp = self.elapsed();
        // flushed at phase changes and at the end: a crash in libav keeps the lines before it
        let flush = matches!(
            line,
            LogLine::Engine(kt_engine::export::ExportLog::Phase(_) | kt_engine::export::ExportLog::Finalizing)
                | LogLine::Failed(_)
                | LogLine::Engine(kt_engine::export::ExportLog::Error(_))
        );
        self.log_file.write(stamp, &line.english(), flush);
        if self.log.len() == LOG_LINES {
            self.log.pop_front();
        }
        self.log.push_back((stamp, line));
    }

    /// One message of the job (the same path for the background thread and a test's script).
    pub fn on_msg(&mut self, m: RenderMsg) {
        use kt_engine::export::ExportEvent;
        match m {
            RenderMsg::Engine(ExportEvent::Opened(r)) => {
                self.push_log(LogLine::Opened(r.encoder.clone()));
                self.report = Some(r);
            }
            RenderMsg::Engine(ExportEvent::Progress(p)) => {
                self.progress = p;
                self.sample_speed();
            }
            RenderMsg::Engine(ExportEvent::Log(l)) => self.push_log(LogLine::Engine(l)),
            RenderMsg::Backend(b) => {
                self.push_log(LogLine::Backend(b.clone()));
                self.backend = Some(b);
            }
        }
    }

    /// Speed (× real time of OUTPUT) and the video bitrate over ≥ 0.5 s between samples (D3).
    fn sample_speed(&mut self) {
        let now = (self.clock)();
        let (t0, f0, b0) = self.sample;
        let dt = now.saturating_duration_since(t0).as_secs_f64();
        let (f1, b1) = (self.progress.video_frames, self.progress.video_bytes);
        if dt < 0.5 || f1 <= f0 {
            return;
        }
        let out_s = (f1 - f0) as f64 / self.fps as f64;
        let cur = out_s / dt;
        self.speed_cur = cur;
        self.speed_min = self.speed_min.min(cur);
        self.speed_max = self.speed_max.max(cur);
        self.speed_sum += cur;
        self.speed_n += 1;
        self.bitrate_cur = Some(b1.saturating_sub(b0) as f64 * 8.0 / out_s);
        self.graph.push(&[cur as f32]);
        self.sample = (now, f1, b1);
    }

    /// The result arrived: the clocks stop here, the file's size is read once.
    pub fn on_result(&mut self, r: Result<kt_engine::export::ExportStats, String>) {
        self.finished_at = Some((self.clock)());
        match &r {
            Ok(_) => {
                self.final_size = std::fs::metadata(&self.out_path).ok().map(|m| m.len());
                // the engine's own "done" line is the result: a second one only repeated it
            }
            Err(e) => self.push_log(LogLine::Failed(e.clone())),
        }
        self.result = Some(r);
    }

    /// Collect what the thread sent; `true` — something changed.
    pub fn poll(&mut self) -> bool {
        let mut msgs = Vec::new();
        let any = self.job.poll_with(|m| msgs.push(m));
        for m in msgs {
            self.on_msg(m);
        }
        if let Some(r) = self.job.done.take() {
            self.on_result(r);
        }
        any
    }

    /// Output seconds rendered and in total.
    fn output_secs(&self) -> (f64, f64) {
        let fps = self.fps as f64;
        (self.progress.video_frames as f64 / fps, self.progress.total_video_frames as f64 / fps)
    }
}

/// A duration for a display row: a negative, NaN or overflowing number of seconds is zero, never a
/// panic (render-crash-and-recovery D1).
pub(crate) fn secs(s: f64) -> std::time::Duration {
    std::time::Duration::try_from_secs_f64(s.max(0.0)).unwrap_or_default()
}

/// Render SETTINGS dialog (item 5): codec, quality, bitrate, resolution.
pub struct RenderSetup {
    pub encoder: Option<String>, // None = Auto
    pub quality: u32,
    /// Some = target video bitrate mode (bit/s) instead of CRF (item 9).
    pub video_bitrate: Option<u32>,
    pub audio_bitrate: u32,
    pub resolution: Option<(u32, u32)>,
    pub open: bool,
    /// Sources unavailable at the moment the settings are opened: shown as a
    /// warning BEFORE the launch, not a surprise in the finished file.
    pub missing: Vec<String>,
    /// Where `render.log` is written (D9): the data directory; a test points it at a temp folder.
    pub log_dir: std::path::PathBuf,
    /// A test's recorder of Open file / Open folder / Open log (D7): `Some` — the path is pushed
    /// here instead of starting the system's program.
    pub test_open: Option<Vec<std::path::PathBuf>>,
    /// A test's answer of the save dialog: the harness runs many tests in one process, and
    /// `KT_FILE_PICK` — an environment variable — would answer for all of them at once.
    pub test_pick: Option<std::path::PathBuf>,
    /// The encoders the settings offer, asked when the window opens (`start_render`).
    pub encoders: Vec<kt_media::muxer::AvailableEncoder>,
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
            // a test never rotates the developer's own render.log
            log_dir: if cfg!(test) {
                std::env::temp_dir().join(format!("kt-render-log-{}", std::process::id()))
            } else {
                crate::journal::data_dir()
            },
            test_open: None,
            test_pick: None,
            encoders: Vec::new(),
        }
    }
}

type Sources = Vec<(MediaId, Box<dyn MediaSource>)>;

/// Render sources. An unavailable file does NOT cancel the export (R.33
/// item 10): a placeholder takes its place, and the name goes into the list —
/// the user is warned before the start and sees the count afterwards.
pub(crate) fn open_all(
    project: &Project,
    base: Option<&std::path::Path>,
) -> (Sources, Vec<String>) {
    let mut out = Vec::new();
    let mut missing = Vec::new();
    for m in &project.media {
        let src: Box<dyn MediaSource> = match kt_media::factory::open_pseudo(&m.path) {
            Some(Ok(s)) => s,
            // a broken pseudo-path (a generator newer than this build) — the same
            // fate as a missing file: a visible placeholder + a line
            // in the list; the export is not cancelled
            Some(Err(e)) => {
                let label = crate::util::media_label(&m.path);
                missing.push(format!("{label}: {e}"));
                Box::new(kt_media::placeholder::PlaceholderSource::new(label))
            }
            None => {
                let resolved = resolve_media_path(base, &m.path);
                match kt_media::ffmpeg::FfmpegSource::open(&resolved) {
                    Ok(s) => Box::new(s),
                    Err(e) => {
                        let label = crate::util::media_label(&m.path);
                        missing.push(format!("{label}: {e}"));
                        Box::new(kt_media::placeholder::PlaceholderSource::new(label))
                    }
                }
            }
        };
        out.push((m.id, src));
    }
    (out, missing)
}

/// Unavailable project sources — for the warning BEFORE the render starts.
pub(crate) fn missing_sources(project: &Project, base: Option<&std::path::Path>) -> Vec<String> {
    project
        .media
        .iter()
        .filter(|m| match kt_media::factory::open_pseudo(&m.path) {
            // pseudo-source “unavailable” only if the path is broken
            Some(r) => r.is_err(),
            None => !std::path::Path::new(&resolve_media_path(base, &m.path)).exists(),
        })
        .map(|m| crate::util::media_label(&m.path))
        .collect()
}

/// A piece of a file name from user text: spaces become hyphens,
/// characters forbidden by the file system become underscores (R.33 item 9).
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

/// Output file name from the user's template (R.33 item 9):
/// `kt-<project>-<region>-<length in seconds>s.mp4`. No project — `untitled`;
/// the region did not match a named marker (or the name is empty) — the region
/// segment is omitted so that the name does not misstate the content.
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
    /// Name substituted into the render save dialog.
    pub(crate) fn default_render_name(&self) -> String {
        let project = self.model.project();
        let (start, end) = project.region.map_or_else(
            || (Tick::ZERO, kt_engine::export::project_range(project)),
            |r| (r.start, r.end),
        );
        // the name is taken only from a range marker MATCHING the region: otherwise
        // the file name would contain a foreign piece of the timeline.
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

    /// “Render” button: opens the settings panel (item 5 — choice of codec etc.).
    pub fn start_render(&mut self) {
        if self.render_job.is_some() {
            self.note(tr(k::render::ALREADY_RUNNING));
            return;
        }
        let base = self.project_path.as_deref().and_then(|p| p.parent());
        self.render_setup.missing = missing_sources(&self.model.document(), base);
        self.render_setup.encoders = kt_media::muxer::available_encoders();
        // The mismatch must be NAMED (rule 16, R.55 C1): from this
        // round on, INSIDE a reel the reel PLAYS, but the export still covers
        // the whole DOCUMENT. A person who pressed “Render” from inside a composition
        // would otherwise get something other than what they heard, and would learn of it
        // only from the finished file.
        if self.model.editing().is_some() {
            self.warn(tr(k::render::WHOLE_PROJECT_NOT_REEL));
        }
        self.render_setup.open = true;
    }

    /// Start after the settings are confirmed.
    pub fn launch_render(&mut self) {
        if self.render_job.is_some() {
            return;
        }
        let default_name = self.default_render_name();
        // via the shared save-dialog seam (`KT_FILE_PICK`): a direct rfd
        // in the virtual screen silently returns “nothing”, and a render from a live
        // scenario (landing screenshots) did not start at all
        let picked = match self.render_setup.test_pick.clone() {
            Some(p) => Some(p),
            None => crate::app::pick_save(tr(k::render::SETTINGS_TITLE), "MP4 Video", &["mp4"], None, &default_name),
        };
        let Some(path) = picked else { return };
        // AN EXISTING FILE IS ASKED ABOUT FIRST (D8): the muxer would truncate it at once, and a
        // cancelled or failed render would then delete it. Asked always — the native dialog's own
        // question is not guaranteed (the `KT_FILE_PICK` seam, portals that do not confirm).
        if path.exists() {
            self.render_setup.open = false;
            self.ask = Some(crate::app::Ask::OverwriteRender { path });
            return;
        }
        self.start_render_to(path);
    }

    /// Start the render into `path` (asked about already, or new).
    pub fn start_render_to(&mut self, path: std::path::PathBuf) {
        if self.render_job.is_some() {
            return;
        }
        let out_path = path.to_string_lossy().to_string();
        let project = self.model.document();
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
            placeholders: self.render_setup.missing.clone(),
        };
        let opts_thread = opts.clone();
        let out_path_thread = out_path.clone();
        let job = crate::bg::BgJob::spawn("render", move |ctl, send| {
            let (a, _) = open_all(&project, base.as_deref());
            let (v, _) = open_all(&project, base.as_deref());
            // R.29: GPU render by default — its own headless device per
            // job (the UI queue does not share time with the export); if there is no engine
            // → CPU (for 2D this is a full-fledged path), and the window says why (D4)
            let video_gpu = match crate::app::export_video_gpu() {
                Ok(e) => {
                    send.send(RenderMsg::Backend(BackendLine::Gpu(e.info().name.clone())));
                    Some(e)
                }
                Err(reason) => {
                    send.send(RenderMsg::Backend(BackendLine::Cpu(reason)));
                    None
                }
            };
            let backends = kt_engine::ExportBackends { video_gpu };
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
                move |e| send.send(RenderMsg::Engine(e)),
            )
            .map_err(|e| e.to_string())
        });
        let out_res = opts.resolution.unwrap_or(proj_res);
        self.render_job = Some(RenderJob::new(
            job,
            out_path,
            opts,
            out_res,
            proj_fps,
            self.render_setup.missing.clone(),
            &self.render_setup.log_dir,
            std::time::Instant::now,
        ));
        self.render_setup.open = false;
    }

    /// Render settings panel (before the launch).
    pub fn draw_render_setup(&mut self, ctx: &egui::Context) {
        if !self.render_setup.open || self.render_job.is_some() {
            return;
        }
        let mut open = true;
        let mut launch = false;
        let proj = self.model.project().settings.clone();
        let max_src = self.max_source_resolution_cached();
        let shown = crate::window_shell::Shell::new(
            egui::Id::new("kt_render_settings"),
            tr(k::render::SETTINGS_TITLE),
            crate::window_shell::Kind::Dialog,
        )
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                // GROUPS modeled on “Transform” (R.66): video, audio, file
                crate::widgets::group(ui, "render.video", tr(k::grp::VIDEO), |ui| {
                egui::Grid::new("render_setup").num_columns(2).spacing([12.0, 8.0]).show(
                    ui,
                    |ui| {
                        ui.label(tr(k::render::VIDEO_CODEC));
                        // asked once when the window opens (`start_render`): a device candidate
                        // opens its device to be listed, which is not a per-frame cost
                        let encoders = self.render_setup.encoders.clone();
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
                                // a device encoder names the device it will use (hw-encode-context D6)
                                let label = match &e.device {
                                    Some(d) => format!("{} ({d})", e.name),
                                    None => e.name.clone(),
                                };
                                ui.selectable_value(&mut self.render_setup.encoder, Some(e.name.clone()), label);
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

                        ui.end_row();
                    },
                );
                });
                crate::widgets::group(ui, "render.audio", tr(k::grp::AUDIO), |ui| {
                egui::Grid::new("render_audio").num_columns(2).spacing([12.0, 8.0]).show(
                    ui,
                    |ui| {
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
                        ui.end_row();
                    },
                );
                });
                crate::widgets::group(ui, "render.file", tr(k::grp::FILE), |ui| {
                egui::Grid::new("render_file").num_columns(2).spacing([12.0, 8.0]).show(
                    ui,
                    |ui| {
                        ui.label(tr(k::render::CONTAINER));
                        ui.weak("MP4 (+faststart)");
                        ui.end_row();
                    },
                );
                });
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
                // WHAT WILL NOT GET INTO THE FILE (R.62): mute and solo — are the
                // project's content and affect the export (owner's decision),
                // so they must be mentioned BEFORE the start and in the same place as
                // the unavailable sources (rules 16, 19).
                // The “where to change it” address is the track header, and it is named
                // by the track's own name.
                let doc = self.model.document();
                let silenced = kt_model::silenced_tracks(&doc);
                if !silenced.is_empty() {
                    ui.add_space(4.0);
                    let col = egui::Color32::from_rgb(255, 190, 120);
                    if !silenced.muted.is_empty() {
                        let r = ui.colored_label(
                            col,
                            crate::tf!(
                                k::render::MUTED_WARN,
                                silenced.muted.len(),
                                silenced.muted.join(", ")
                            ),
                        );
                        crate::widgets::field_probe::note("render.muted", r.rect);
                    }
                    if !silenced.solo.is_empty() {
                        let r = ui.colored_label(
                            col,
                            crate::tf!(
                                k::render::SOLO_WARN,
                                silenced.solo.len(),
                                silenced.solo.join(", ")
                            ),
                        );
                        crate::widgets::field_probe::note("render.solo", r.rect);
                    }
                }
                });
                crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    let start = ui.button(tr(k::render::START));
                    crate::widgets::field_probe::note("render.start", start.rect);
                    if start.clicked() {
                        launch = true;
                    }
                    if ui.button(tr(k::render::CANCEL)).clicked() {
                        self.render_setup.open = false;
                    }
                });
                });
            });
        if shown.closed() {
            open = false;
        }
        // the window frame as a whole — the landing screenshots are cropped by it (site/screenshots.py)
        if let Some(r) = &shown.response {
            crate::widgets::field_probe::note("win.render", r.rect);
        }
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
        if job.poll() {
            ctx.request_repaint();
        }
        // the window moves only while the job runs (invariant 12, the t38 family)
        if job.is_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        #[derive(Clone, Copy, PartialEq)]
        enum Act {
            Pause,
            Cancel,
            OpenFile,
            OpenFolder,
            OpenLog,
            Close,
        }
        let mut act: Option<Act> = None;
        let mut hover: Option<String> = None;
        const FORM_W: f32 = 380.0;
        let running = job.is_running();
        let phase = match job.progress.phase {
            kt_engine::export::ExportPhase::Video => tr(k::render::PHASE_VIDEO),
            kt_engine::export::ExportPhase::Audio => tr(k::render::PHASE_AUDIO),
            kt_engine::export::ExportPhase::Finalize => tr(k::render::PHASE_FINALIZE),
            kt_engine::export::ExportPhase::Done => tr(k::render::PHASE_DONE),
        };
        let phase = match &job.result {
            Some(Ok(_)) => tr(k::render::PHASE_DONE),
            _ => phase,
        };
        // THE ROWS COME FROM THE RUN (D5): the encoder that opened, never a guessed name
        let video = match &job.report {
            Some(r) => {
                let kind = tr(if r.hardware { k::render::HARDWARE } else { k::render::SOFTWARE });
                // a device encoder names its device (hw-encode-context D6)
                let kind = match &r.device {
                    Some(d) => format!("{kind} · {d}"),
                    None => kind.to_string(),
                };
                format!("{} ({kind}) · {} · {}", r.encoder, r.pix_fmt, quality_text(r, &job.opts))
            }
            None => tr(k::render::CHOOSING).to_string(),
        };
        let out_rows: Vec<(&str, String)> = vec![
            (tr(k::render::ROW_RESOLUTION), format!("{}×{}", job.out_res.0, job.out_res.1)),
            (tr(k::render::ROW_FPS), crate::tf!(k::render::FPS_VALUE, job.fps)),
            (tr(k::render::ROW_VIDEO), video),
            (tr(k::render::ROW_AUDIO), crate::tf!(k::render::AUDIO_LINE, job.opts.audio_bitrate / 1000)),
            (tr(k::render::ROW_MUX), "MP4 (+faststart)".to_string()),
            (tr(k::render::ROW_BACKEND), job.backend.as_ref().map(backend_text).unwrap_or_else(|| "—".into())),
            (tr(k::render::ROW_FILE), job.out_path.clone()),
        ];
        let p = job.progress;
        // CLAMPED (render-crash-and-recovery D1): a counter past its total once made the
        // remaining time negative, and `Duration::from_secs_f64` panicked on the UI thread
        let frac = match &job.result {
            Some(Ok(_)) => 1.0,
            _ if p.total_video_frames > 0 => {
                (p.video_frames as f64 / p.total_video_frames as f64).clamp(0.0, 1.0)
            }
            _ => 0.0,
        };
        let elapsed = job.elapsed();
        let (done_s, total_s) = job.output_secs();
        let so_far = p.video_bytes + p.audio_bytes;
        let mut run_rows: Vec<(&str, String)> = vec![
            (tr(k::render::ROW_PHASE), phase.to_string()),
            (tr(k::render::ROW_FRAMES), format!("{} / {}", p.video_frames, p.total_video_frames)),
            (tr(k::render::ROW_ELAPSED), clock_text(elapsed)),
        ];
        if running && frac > 0.01 {
            let left = elapsed.as_secs_f64() / frac * (1.0 - frac);
            run_rows.push((tr(k::render::ROW_REMAINING), clock_text(secs(left))));
        }
        run_rows.push((
            tr(k::render::ROW_LENGTH),
            format!(
                "{} / {}",
                clock_text(secs(done_s)),
                clock_text(secs(total_s))
            ),
        ));
        let size = match (job.final_size, running && frac >= 0.02) {
            (Some(f), _) => size_text(f),
            (None, true) => crate::tf!(
                k::render::SIZE_VALUE,
                size_text(so_far),
                size_text(((so_far as f64 / frac) as u64).max(so_far))
            ),
            (None, false) => size_text(so_far),
        };
        run_rows.push((tr(k::render::ROW_SIZE), size));
        if let Some(b) = job.bitrate_cur {
            run_rows.push((tr(k::render::ROW_BITRATE), bitrate_text(b)));
        }
        let form = |ui: &mut egui::Ui, id: &str, title: &str, rows: &[(&str, String)]| {
            ui.strong(title);
            egui::Grid::new(id).num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
                for (key, value) in rows {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak(*key);
                    });
                    // truncated, not stretched: a long output path widened the window past the
                    // screen and took Close with it (live scenario `render`)
                    ui.add(egui::Label::new(egui::RichText::new(value).monospace()).truncate());
                    ui.end_row();
                }
            });
        };
        let shown = crate::window_shell::Shell::new(
            egui::Id::new("kt_render_progress"),
            tr(k::render::PROGRESS_TITLE),
            crate::window_shell::Kind::Progress {
                why: "an export runs for minutes; an Esc meant for the timeline must not kill it, and the window has its own Cancel",
            },
        )
            .resizable()
            .default_size([2.0 * FORM_W + 40.0, 520.0])
            .default_pos(ctx.content_rect().center() - egui::vec2(FORM_W + 20.0, 260.0))
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                    // the forms side by side when there is room, else one under the other (rule 3)
                    if ui.available_width() >= 2.0 * FORM_W {
                        ui.columns(2, |c| {
                            form(&mut c[0], "render_out", tr(k::render::OUT_TITLE), &out_rows);
                            form(&mut c[1], "render_run", tr(k::render::RUN_TITLE), &run_rows);
                        });
                    } else {
                        form(ui, "render_out", tr(k::render::OUT_TITLE), &out_rows);
                        ui.add_space(6.0);
                        form(ui, "render_run", tr(k::render::RUN_TITLE), &run_rows);
                    }
                    ui.add_space(6.0);
                    if job.speed_n > 0 {
                        ui.weak(crate::tf!(
                            k::render::SPEED,
                            format!("{:.2}", if job.speed_min == f64::MAX { 0.0 } else { job.speed_min }),
                            format!("{:.2}", job.speed_sum / job.speed_n as f64),
                            format!("{:.2}", if running { job.speed_cur } else { 0.0 }),
                            format!("{:.2}", job.speed_max),
                        ));
                    }
                    let chart = ui.scope(|ui| job.graph.draw(ui, 120.0)).response.rect;
                    crate::widgets::field_probe::note("render.chart", chart);
                    // THE FINISHED RUN IS FROZEN AND SUMMARISED (D6)
                    match &job.result {
                        Some(Ok(stats)) => {
                            let secs = elapsed.as_secs_f64().max(1e-3);
                            let backend = match &job.backend {
                                Some(BackendLine::Gpu(_)) => "GPU",
                                Some(BackendLine::Cpu(_)) => "CPU",
                                None => "—",
                            };
                            let avg_bitrate = p.video_bytes as f64 * 8.0 / done_s.max(1e-3);
                            ui.add_space(4.0);
                            ui.colored_label(
                                egui::Color32::from_rgb(120, 230, 120),
                                crate::tf!(
                                    k::render::FINAL,
                                    clock_text(elapsed),
                                    format!("{:.1}", done_s / secs),
                                    stats.video_frames,
                                    &stats.encoder,
                                    backend,
                                    size_text(job.final_size.unwrap_or(so_far)),
                                    bitrate_text(avg_bitrate)
                                ),
                            );
                            ui.weak(crate::tf!(k::render::FINAL_AUDIO, stats.audio_frames));
                            // an incomplete render no longer looks like a success (R.33
                            // item 10): stubs and lost layers are named directly
                            if !job.missing.is_empty() {
                                ui.colored_label(
                                    egui::Color32::from_rgb(255, 190, 120),
                                    crate::tf!(k::render::MISSING_AFTER, job.missing.len(), job.missing.join(", ")),
                                );
                            }
                            // Loudness is computed on the same pass (R.49, item 371), FOR
                            // REFERENCE: the normalization target is a person's choice
                            if let Some(l) = stats.loudness
                                && let Some(lufs) = l.lufs
                            {
                                ui.label(crate::tf!(
                                    k::render::LOUDNESS,
                                    format!("{lufs:.1}"),
                                    format!("{:.1}", l.dbtp),
                                    format!("{:.1}", l.lra)
                                ));
                                ui.weak(crate::tf!(k::render::LOUDNESS_TO_TARGET, format!("{:+.1}", -23.0 - lufs)));
                            }
                            if stats.missing_video_layers > 0 {
                                ui.colored_label(
                                    egui::Color32::from_rgb(255, 190, 120),
                                    crate::tf!(k::render::MISSING_LAYERS, stats.missing_video_layers),
                                );
                            }
                        }
                        Some(Err(e)) => {
                            ui.colored_label(egui::Color32::from_rgb(255, 120, 120), crate::tf!(k::render::ERROR_MSG, e));
                        }
                        None => {}
                    }
                    // THE LOG (D9): closed while healthy, opened by itself after an error
                    let failed = matches!(job.result, Some(Err(_)));
                    let r = egui::CollapsingHeader::new(tr(k::render::LOG_TITLE))
                        .id_salt("render_log")
                        .open(failed.then_some(true))
                        .show(ui, |ui| {
                            if job.log_file.failed {
                                ui.colored_label(
                                    egui::Color32::from_rgb(255, 190, 120),
                                    crate::tf!(k::render::LOG_ONLY_WINDOW, job.log_file.path.display()),
                                );
                            } else {
                                ui.weak(crate::tf!(k::render::LOG_PATH, job.log_file.path.display()));
                            }
                            egui::ScrollArea::vertical().max_height(160.0).stick_to_bottom(true).show(ui, |ui| {
                                for (stamp, line) in &job.log {
                                    ui.monospace(format!("[{}] {}", clock_text(*stamp), line.shown()));
                                }
                            });
                        });
                    crate::widgets::field_probe::note("render.log", r.header_response.rect);
                });
                crate::widgets::window_actions(ui, |ui| {
                    ui.add(egui::ProgressBar::new(frac as f32).show_percentage());
                    ui.horizontal(|ui| {
                        if running {
                            let paused = job.job.is_paused();
                            let r = ui.button(if paused { tr(k::render::RESUME) } else { tr(k::render::PAUSE) });
                            crate::widgets::field_probe::note("render.pause", r.rect);
                            if r.clicked() {
                                act = Some(Act::Pause);
                            }
                            if paused {
                                ui.colored_label(egui::Color32::from_rgb(255, 200, 100), tr(k::render::PAUSED));
                            }
                            // a cancel during the final mux would throw a finished file away
                            let finishing = p.phase == kt_engine::export::ExportPhase::Finalize;
                            let r = ui.add_enabled(!finishing, egui::Button::new(tr(k::render::CANCEL)));
                            crate::widgets::field_probe::note("render.cancel", r.rect);
                            job.cancel_rect = Some(r.rect);
                            // `hovered()` is always false on a disabled widget: the pointer itself is asked
                            if finishing && r.contains_pointer() {
                                hover = Some(tr(k::render::CANCEL_FINALIZING).to_string());
                            }
                            if r.clicked() {
                                act = Some(Act::Cancel);
                            }
                        } else {
                            if matches!(job.result, Some(Ok(_))) {
                                let r = ui.button(tr(k::render::OPEN_FILE));
                                crate::widgets::field_probe::note("render.open_file", r.rect);
                                if r.clicked() {
                                    act = Some(Act::OpenFile);
                                }
                                let r = ui.button(tr(k::render::OPEN_FOLDER));
                                crate::widgets::field_probe::note("render.open_folder", r.rect);
                                if r.clicked() {
                                    act = Some(Act::OpenFolder);
                                }
                            } else {
                                let r = ui.button(tr(k::render::OPEN_LOG));
                                crate::widgets::field_probe::note("render.open_log", r.rect);
                                if r.clicked() {
                                    act = Some(Act::OpenLog);
                                }
                            }
                            // Close at the far right, away from where Cancel stood (trick 3)
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let r = ui.button(tr(k::render::CLOSE));
                                crate::widgets::field_probe::note("render.close", r.rect);
                                if r.clicked() {
                                    act = Some(Act::Close);
                                }
                            });
                        }
                    });
                });
            });
        // the window frame as a whole — the landing screenshots are cropped by it (site/screenshots.py)
        if let Some(r) = &shown.response {
            crate::widgets::field_probe::note("win.render_progress", r.rect);
        }
        if let Some(h) = hover {
            self.hover_hint = Some(h);
        }
        let Some(job) = &mut self.render_job else { return };
        match act {
            Some(Act::Pause) => {
                let paused = job.job.is_paused();
                job.job.set_paused(!paused);
                job.push_log(if paused { LogLine::Resume } else { LogLine::Pause });
            }
            Some(Act::Cancel) => {
                job.job.cancel();
                job.push_log(LogLine::Cancel);
            }
            Some(Act::OpenFile) => {
                let p = std::path::PathBuf::from(&job.out_path);
                self.open_render_path(p);
            }
            Some(Act::OpenFolder) => {
                let p = std::path::PathBuf::from(&job.out_path);
                let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or(p);
                self.open_render_path(dir);
            }
            Some(Act::OpenLog) => {
                let p = job.log_file.path.clone();
                self.open_render_path(p);
            }
            Some(Act::Close) => {
                // the dialog lets the job go: the export's facts (telemetry-identify-activity)
                if let Some(job) = self.render_job.take() {
                    crate::telemetry::signal(crate::telemetry::Signal::ExportEnded(&job));
                }
            }
            None => {}
        }
    }

    /// Open a file or a folder of the render in the system (D7): the test records it; a spawn
    /// failure is an error notice naming the path (rule 16).
    fn open_render_path(&mut self, p: std::path::PathBuf) {
        if let Some(rec) = self.render_setup.test_open.as_mut() {
            rec.push(p);
            return;
        }
        if crate::util::open_in_file_manager(&p).is_err() {
            self.warn(crate::tf!(k::render::OPEN_FAILED, p.display()));
        }
    }
}
