//! The welcome window (welcome-window-and-tour, capability `ui-reach`).
//!
//! The owner (2026-10-28): "an intro window (as in programming IDEs) offering either to create
//! a project from a template or a list of recent projects (with thumbnails), and right there a
//! tile 'start the tour'… a main tile where the most likely action lands: for the first opening
//! it is the tour, for later openings the tile of the previous project. And in the same window
//! we can show what's new."
//!
//! * The window is a registered window INSIDE the main viewport (design D3): a second OS window
//!   brings the tiling-WM and Alt+Tab defects F-HLP-027 found.
//! * Exactly ONE tile is primary, and it is chosen by the pure `primary()` (D4); the start order
//!   of `run` asks it once, with the file-existence facts gathered before the call.
//! * Posters are rendered by the offline path at save/close (D5) into `<store>/posters/`, never
//!   next to the project; the window only READS finished PNGs, on a helper thread, and uploads a
//!   texture when one arrives (one repaint each, invariant 12).
//! * Nothing animates: the window repaints on input only.

use crate::user_store::{RecentEntry, TourProgress, UserTemplate};

/// A built-in template (D6). The label is a technical token, the same in every language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Template {
    pub id: &'static str,
    pub label: &'static str,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// The five built-ins, in the order the window shows them.
pub const BUILTIN: [Template; 5] = [
    Template { id: "1080p25", label: "1080p25", width: 1920, height: 1080, fps: 25 },
    Template { id: "1080p30", label: "1080p30", width: 1920, height: 1080, fps: 30 },
    Template { id: "4k30", label: "4K30", width: 3840, height: 2160, fps: 30 },
    Template { id: "v916", label: "9:16", width: 1080, height: 1920, fps: 30 },
    Template { id: "sq11", label: "1:1", width: 1080, height: 1080, fps: 30 },
];

/// The template a "new from the last-used template" falls back to when none was used yet.
pub const DEFAULT_TEMPLATE: &str = "1080p30";

/// A user template's id: `user:<name>`.
pub fn user_template_id(name: &str) -> String {
    format!("user:{name}")
}

/// Frame size and rate of a template id (built-in or the person's own).
pub fn template_by_id(id: &str, user: &[UserTemplate]) -> Option<(u32, u32, u32)> {
    if let Some(name) = id.strip_prefix("user:") {
        return user.iter().find(|t| t.name == name).map(|t| (t.width, t.height, t.fps));
    }
    BUILTIN.iter().find(|t| t.id == id).map(|t| (t.width, t.height, t.fps))
}

/// The project a template creates (D6): audio stays 48 kHz (invariant 1), and the settings
/// are explicit — the first imported clip does not resize the project.
pub fn project_for(width: u32, height: u32, fps: u32) -> kt_model::Project {
    kt_model::Project::new(kt_model::ProjectSettings {
        fps,
        width,
        height,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: true,
    })
}

/// What the primary tile does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tile {
    Tour,
    /// 1-based lesson and step, as the tile reads them.
    ContinueTour { lesson: u32, step: u32 },
    /// The row of the recent list.
    Recent(usize),
    /// A template id.
    NewFromTemplate(String),
}

/// What the start does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartAction {
    /// "Show at start" is off: the former start — the last project opens silently.
    NoWindowOpenLast,
    /// A project on the command line, or a recovery offer: the former start, no window.
    NoWindow,
    /// FFmpeg is missing: its window first, then this.
    AfterFfmpeg(Box<StartAction>),
    Window { primary: Tile },
}

/// The facts of the start, gathered BEFORE the call (one `exists()` per recent row).
#[derive(Debug, Clone)]
pub struct StartFacts<'a> {
    pub cli_project: bool,
    pub recovery: bool,
    pub ffmpeg_missing: bool,
    pub show_at_start: bool,
    pub first_start: bool,
    pub tour: &'a TourProgress,
    /// The first unfinished step of a STARTED tour; `None` — not started or finished.
    pub tour_next: Option<(u32, u32)>,
    /// The recent rows with "the file exists".
    pub recent: &'a [(RecentEntry, bool)],
    pub opened_since_tour: bool,
    pub last_template: Option<&'a str>,
}

/// The primary tile — the most likely action (D4). The order of the cases is the scenario of
/// t984 and lives in this one function.
pub fn primary(f: &StartFacts) -> StartAction {
    if f.cli_project || f.recovery {
        return StartAction::NoWindow;
    }
    if !f.show_at_start {
        return StartAction::NoWindowOpenLast;
    }
    let w = StartAction::Window { primary: window_tile(f) };
    if f.ffmpeg_missing { StartAction::AfterFfmpeg(Box::new(w)) } else { w }
}

/// The tile inside the window (also asked when the window is reopened from Help).
pub fn window_tile(f: &StartFacts) -> Tile {
    if f.first_start {
        return Tile::Tour;
    }
    let cont = f.tour_next.map(|(lesson, step)| Tile::ContinueTour { lesson, step });
    if let Some(c) = cont.clone()
        && !f.opened_since_tour
    {
        return c;
    }
    let template = || {
        let from_row = f.recent.first().and_then(|(e, _)| e.template.as_deref());
        Tile::NewFromTemplate(f.last_template.or(from_row).unwrap_or(DEFAULT_TEMPLATE).to_string())
    };
    match f.recent.first() {
        Some((_, true)) => Tile::Recent(0),
        // the previous project is missing: its row stays (flagged), and the most likely
        // action becomes a new project like the last one
        Some((_, false)) => template(),
        None if !f.tour.started() => Tile::Tour,
        None => cont.unwrap_or_else(template),
    }
}

/// What the start does (the result of `plan_start`).
#[derive(Debug, Clone, PartialEq)]
pub struct StartPlan {
    /// The welcome window with its primary tile.
    pub window: Option<Tile>,
    /// The welcome window waits for the FFmpeg window.
    pub after_ffmpeg: bool,
    /// The former start: the project to open silently.
    pub open: Option<std::path::PathBuf>,
    /// The first entry: the style question opens over the welcome window (L1). A start into a
    /// project passed by hand keeps it for the first start that has the window.
    pub offer_controls: bool,
}

/// The start order of `run` (D4): gather the facts — one `exists()` per recent row — and ask
/// `primary` once. `recovery(last)` says whether unsaved work waits.
pub fn plan_start(
    store: &mut crate::user_store::StartStore,
    last_project: Option<&str>,
    cli: Option<&str>,
    ffmpeg_missing: bool,
    recovery: impl Fn(Option<&std::path::Path>) -> bool,
) -> StartPlan {
    // a sandbox stored by an older build is not a project to reopen (render-crash-and-recovery D6)
    let last_project = last_project.filter(|p| !crate::lessons::is_sandbox(std::path::Path::new(p)));
    let first_start = store.begin_session(last_project);
    let last_path = store
        .recent
        .first()
        .map(|e| e.path.clone())
        .or_else(|| last_project.map(std::path::PathBuf::from))
        .filter(|p| p.exists());
    let rows: Vec<(RecentEntry, bool)> = store.recent.iter().map(|e| (e.clone(), e.path.exists())).collect();
    let decision = primary(&StartFacts {
        cli_project: cli.is_some(),
        recovery: recovery(last_path.as_deref()),
        ffmpeg_missing,
        show_at_start: store.start.show_at_start,
        first_start,
        tour: &store.tour,
        tour_next: crate::lessons::next_step(&store.tour),
        recent: &rows,
        opened_since_tour: store.opened_since_tour(),
        last_template: store.start.last_template.as_deref(),
    });
    let (window, after_ffmpeg) = match decision {
        StartAction::Window { primary } => (Some(primary), false),
        StartAction::AfterFfmpeg(inner) => match *inner {
            StartAction::Window { primary } => (Some(primary), true),
            _ => (None, false),
        },
        _ => (None, false),
    };
    let open = if window.is_some() {
        None
    } else {
        cli.map(std::path::PathBuf::from).or(last_path).filter(|p| p.exists())
    };
    let offer_controls = window.is_some() && !store.start.controls_offered;
    StartPlan { window, after_ffmpeg, open, offer_controls }
}

// ---- the state that lives between projects -------------------------------------------------

/// What a click on a start tile (or a Help item) goes to, once the unsaved-work question is
/// answered.
#[derive(Debug, Clone, PartialEq)]
pub enum StartGo {
    Template(String),
    Recent(std::path::PathBuf),
    /// A lesson (1-based); `fresh` — "Start over": a new copy and no progress.
    Lesson { lesson: u32, fresh: bool },
}

/// The welcome window's own state.
pub struct WelcomeUi {
    pub open: bool,
    pub primary: Tile,
    /// Open at the What's new panel (Help → What's new) — scrolled to once.
    pub at_whats_new: bool,
    pub posters: Posters,
}

impl Default for WelcomeUi {
    fn default() -> WelcomeUi {
        WelcomeUi { open: false, primary: Tile::Tour, at_whats_new: false, posters: Posters::default() }
    }
}

/// Everything of the start that must SURVIVE a project open: `KtApp` is rebuilt in place on
/// File → New/Open, and `KtApp::replace_with` moves this across.
pub struct Start {
    pub store: crate::user_store::StartStore,
    pub welcome: WelcomeUi,
    pub guide: crate::guide::GuideUi,
    pub lessons: crate::lessons::LessonsUi,
    /// FFmpeg was missing at start: the welcome window opens when its window closes (D4).
    pub after_ffmpeg: bool,
    /// What the unsaved-work question waits for.
    pub pending: Option<StartGo>,
    /// The template the current unnamed project was made from (its recent row remembers it).
    pub made_from: Option<String>,
    /// File › Preferences… is open (welcome-window-and-settings): here, so a project open from
    /// the welcome window does not close it.
    pub prefs_open: bool,
    /// The Layout & Controls style in force (control-layouts): here, so File → New keeps it.
    pub controls: crate::controls::Controls,
    /// The style window's own state.
    pub controls_ui: crate::prefs_window::ControlsUi,
}

impl Default for Start {
    /// A memory store: a `KtApp` built anywhere but `run` (harness, the in-place rebuild) does
    /// not read the person's files; `run` installs the real store, `replace_with` carries it.
    fn default() -> Start {
        Start {
            store: crate::user_store::StartStore::at(None),
            welcome: WelcomeUi::default(),
            guide: Default::default(),
            lessons: Default::default(),
            after_ffmpeg: false,
            pending: None,
            controls: Default::default(),
            controls_ui: Default::default(),
            made_from: None,
            prefs_open: false,
        }
    }
}

// ---- posters: the job (D5) and the loader -----------------------------------------------------

/// One poster to render.
pub struct PosterJob {
    pub out: std::path::PathBuf,
    pub project: kt_model::Project,
    pub base: Option<std::path::PathBuf>,
    pub at: kt_time::Tick,
}

#[derive(Default)]
struct PosterQueue {
    jobs: std::collections::VecDeque<PosterJob>,
    busy: bool,
    done: u64,
}

fn poster_queue() -> &'static (std::sync::Mutex<PosterQueue>, std::sync::Condvar) {
    static Q: std::sync::OnceLock<(std::sync::Mutex<PosterQueue>, std::sync::Condvar)> =
        std::sync::OnceLock::new();
    Q.get_or_init(|| {
        std::thread::Builder::new()
            .name("kt-posters".into())
            .spawn(poster_worker)
            .expect("spawn the poster worker");
        Default::default()
    })
}

fn poster_pause() -> &'static std::sync::Arc<std::sync::atomic::AtomicBool> {
    static P: std::sync::OnceLock<std::sync::Arc<std::sync::atomic::AtomicBool>> = std::sync::OnceLock::new();
    P.get_or_init(Default::default)
}

/// Queue a poster. The service outlives `KtApp` on purpose: a poster is queued when a project
/// CLOSES, i.e. exactly when its app is replaced. A pending job for the same file is replaced
/// (a series of Ctrl+S costs one frame).
pub fn request_poster(job: PosterJob) {
    let (m, cv) = poster_queue();
    let mut q = m.lock().unwrap_or_else(|e| e.into_inner());
    q.jobs.retain(|j| j.out != job.out);
    q.jobs.push_back(job);
    cv.notify_all();
}

/// Playback pauses the job (invariant 6: the offline path does not take time from real time).
pub fn set_posters_paused(on: bool) {
    poster_pause().store(on, std::sync::atomic::Ordering::Release);
}

/// Wait until the queue is empty and nothing renders; `false` on the timeout. On exit and in
/// tests.
pub fn wait_posters(timeout: std::time::Duration) -> bool {
    let (m, cv) = poster_queue();
    let deadline = std::time::Instant::now() + timeout;
    let mut q = m.lock().unwrap_or_else(|e| e.into_inner());
    while q.busy || !q.jobs.is_empty() {
        let now = std::time::Instant::now();
        if now >= deadline {
            return false;
        }
        q = cv.wait_timeout(q, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
    }
    true
}

/// How many posters the service has finished (tests count them).
#[cfg_attr(not(test), allow(dead_code))]
pub fn posters_done() -> u64 {
    poster_queue().0.lock().unwrap_or_else(|e| e.into_inner()).done
}

fn poster_worker() {
    // the queue's initializer spawned us and does not wait for us: this call blocks only
    // until that initializer returns
    let (m, cv) = poster_queue();
    loop {
        let job = {
            let mut q = m.lock().unwrap_or_else(|e| e.into_inner());
            while q.jobs.is_empty() {
                q = cv.wait(q).unwrap_or_else(|e| e.into_inner());
            }
            q.busy = true;
            q.jobs.pop_front()
        };
        if let Some(job) = job {
            let mut _span = kt_time::trace::span(kt_time::trace::Ev::Job);
            _span.aux(kt_time::trace::job::POSTER);
            let ctl = crate::bg::BgCtl::from_flags(Default::default(), poster_pause().clone());
            if let Some(rgba) = crate::reel_preview::render_poster(&ctl, &job.project, job.base.as_deref(), job.at) {
                write_poster(&job.out, rgba);
            }
        }
        let mut q = m.lock().unwrap_or_else(|e| e.into_inner());
        q.busy = false;
        q.done += 1;
        cv.notify_all();
    }
}

/// PNG by tmp + rename: the window never reads half a file.
fn write_poster(out: &std::path::Path, rgba: Vec<u8>) {
    use crate::reel_preview::{POSTER_H, POSTER_W};
    let Some(img) = image::RgbaImage::from_raw(POSTER_W, POSTER_H, rgba) else { return };
    if let Some(d) = out.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = out.with_extension("png.tmp");
    if img.save_with_format(&tmp, image::ImageFormat::Png).is_ok() {
        let _ = std::fs::rename(&tmp, out);
    }
}

/// Finished posters, read on a helper thread and uploaded as textures when they arrive (one
/// repaint each, invariant 12). The draw path only looks the texture up.
#[derive(Default)]
pub struct Posters {
    textures: std::collections::HashMap<std::path::PathBuf, egui::TextureHandle>,
    rx: Option<std::sync::mpsc::Receiver<(std::path::PathBuf, egui::ColorImage)>>,
}

impl Posters {
    /// Read the posters of these rows (key → PNG) afresh.
    pub fn load(&mut self, ctx: &egui::Context, rows: Vec<(std::path::PathBuf, std::path::PathBuf)>) {
        self.textures.clear();
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        let _ = std::thread::Builder::new().name("kt-poster-load".into()).spawn(move || {
            for (key, png) in rows {
                let Ok(bytes) = std::fs::read(&png) else { continue };
                let Ok(img) = image::load_from_memory(&bytes) else { continue };
                let img = img.to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let ci = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                if tx.send((key, ci)).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        });
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        while let Ok((key, img)) = rx.try_recv() {
            let name = format!("poster:{}", key.display());
            self.textures.insert(key, ctx.load_texture(name, img, egui::TextureOptions::LINEAR));
        }
    }

    pub fn get(&self, key: &std::path::Path) -> Option<&egui::TextureHandle> {
        self.textures.get(key)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn count(&self) -> usize {
        self.textures.len()
    }
}

// ---- the window --------------------------------------------------------------------------------

use crate::i18n::{k, tr};
use eframe::egui;

/// The initials of a project for its placeholder card.
fn initials(name: &str) -> String {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

/// What a click in the window asked for; applied after the window is drawn.
enum Act {
    Go(StartGo),
    Locate(usize),
    Forget(usize),
    Clear,
    DeleteTemplate(String),
    Guide,
    /// A link inside What's new: the Guide at that page and heading.
    GuideAt((String, String)),
    Prefs,
    Close,
}

/// The welcome window's default and smallest size (welcome-window-and-settings D1): a SIZED window
/// — a fitted one measured its content and once grew to 2010 points. The minimum holds the three
/// panes at their floors side by side.
const WELCOME_SIZE: [f32; 2] = [1180.0, 720.0];
const WELCOME_MIN: [f32; 2] = [820.0, 460.0];
/// The panes' floors and ceilings: Start a quarter, What's new a third, Recent the rest.
const START_W: (f32, f32) = (260.0, 340.0);
const NEWS_W: (f32, f32) = (300.0, 560.0);
const RECENT_MIN_W: f32 = 200.0;
/// A recent card's floor: the pane holds as many per row as fit, then stretches them (D4).
const CARD_MIN_W: f32 = 180.0;
const CARD_GAP: f32 = 10.0;
/// A start row (the tour, a template, a lesson): one click target across the pane.
const ROW_H: f32 = 40.0;

/// The panes' widths for the window's inner width: exact, or the window would widen itself by the
/// difference every frame (t918). Two separators sit between the three panes.
pub(crate) fn pane_widths(inner: f32, gap: f32) -> (f32, f32, f32) {
    let start = (inner * 0.25).clamp(START_W.0, START_W.1).floor();
    let news = (inner * 0.34).clamp(NEWS_W.0, NEWS_W.1).floor();
    let recent = (inner - start - news - 4.0 * gap - 12.0).max(RECENT_MIN_W).floor();
    (start, recent, news)
}

/// How many recent cards a row holds, and their stretched width.
pub(crate) fn card_columns(avail: f32) -> (usize, f32) {
    let cols = ((avail + CARD_GAP) / (CARD_MIN_W + CARD_GAP)).floor().max(1.0) as usize;
    let w = ((avail - CARD_GAP * (cols as f32 - 1.0)) / cols as f32).floor();
    (cols, w)
}

/// The gap between two panes: a separator's width, no height; the rule is painted over the row's
/// height once the row is laid out. Returns the rule's x.
fn pane_gap(ui: &mut egui::Ui) -> f32 {
    let (r, _) = ui.allocate_exact_size(egui::vec2(6.0, 0.0), egui::Sense::hover());
    r.center().x
}

/// A pane's title: strong, a little larger, with a rule under it — outside the pane's scroll, so it
/// stays when the pane scrolls.
fn pane_title(ui: &mut egui::Ui, text: &str, extra: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(text).size(15.0).strong());
        extra(ui);
    });
    ui.separator();
}

/// A row of the Start pane: a framed click target across the pane; the primary one has the accent
/// stroke (D3). `paint` draws the row's content into its rectangle; the row is a button named
/// `name` for screen readers and the harness.
fn start_row(
    ui: &mut egui::Ui,
    name: &str,
    primary: bool,
    probe: &str,
    paint: impl FnOnce(&egui::Painter, egui::Rect, &egui::Style),
) -> egui::Response {
    let h = if primary { ROW_H + 8.0 } else { ROW_H };
    let (rect, r) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    let painter = ui.painter_at(rect);
    let fill = if r.hovered() { crate::theme::HOVER } else { ui.visuals().faint_bg_color };
    let stroke = if primary {
        egui::Stroke::new(2.0, crate::theme::ACCENT)
    } else {
        egui::Stroke::new(1.0, crate::theme::OUTLINE)
    };
    painter.rect(rect, 3.0, fill, stroke, egui::StrokeKind::Inside);
    paint(&painter, rect.shrink2(egui::vec2(8.0, 4.0)), ui.style());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    crate::widgets::field_probe::note(probe, rect);
    ui.add_space(2.0);
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Two lines in a row's rectangle from `x`: the title strong, the subtitle weak, clipped.
fn row_text(p: &egui::Painter, rect: egui::Rect, x: f32, title: &str, sub: &str, style: &egui::Style) {
    let strong = style.visuals.strong_text_color();
    let weak = style.visuals.weak_text_color();
    let mid = rect.center().y;
    if sub.is_empty() {
        p.text(egui::pos2(x, mid), egui::Align2::LEFT_CENTER, title, egui::FontId::proportional(14.0), strong);
    } else {
        p.text(egui::pos2(x, mid - 1.0), egui::Align2::LEFT_BOTTOM, title, egui::FontId::proportional(14.0), strong);
        p.text(egui::pos2(x, mid + 1.0), egui::Align2::LEFT_TOP, sub, egui::FontId::proportional(12.0), weak);
    }
}

/// The frame of a template drawn in its aspect inside a 40×26 box (D3): the shape tells 16:9, 9:16
/// and 1:1 apart before the numbers are read. Returns the drawn rectangle.
pub(crate) fn frame_glyph(p: &egui::Painter, rect: egui::Rect, w: u32, h: u32, primary: bool) -> egui::Rect {
    let boxr = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - 13.0), egui::vec2(40.0, 26.0));
    let k = (boxr.width() / w as f32).min(boxr.height() / h as f32);
    let g = egui::Rect::from_center_size(boxr.center(), egui::vec2(w as f32 * k, h as f32 * k));
    let c = if primary { crate::theme::ACCENT } else { crate::theme::AXIS_ZERO };
    p.rect_stroke(g, 1.0, egui::Stroke::new(1.5, c), egui::StrokeKind::Inside);
    g
}

impl crate::app::KtApp {
    /// The facts of the start (D4) from the live store; the file existence of each recent row is
    /// asked here — at most 12 `exists()`, once per opening of the window.
    pub(crate) fn start_facts_tile(&self) -> Tile {
        let rows: Vec<(RecentEntry, bool)> =
            self.start.store.recent.iter().map(|e| (e.clone(), e.path.exists())).collect();
        let store = &self.start.store;
        let facts = StartFacts {
            cli_project: false,
            recovery: false,
            ffmpeg_missing: false,
            show_at_start: true,
            first_start: false,
            tour: &store.tour,
            tour_next: crate::lessons::next_step(&store.tour),
            recent: &rows,
            opened_since_tour: store.opened_since_tour(),
            last_template: store.start.last_template.as_deref(),
        };
        window_tile(&facts)
    }

    /// Open the welcome window (start, Help → Welcome window…, Help → What's new).
    pub(crate) fn open_welcome(&mut self, ctx: &egui::Context, primary: Option<Tile>, whats_new: bool) {
        let primary = primary.unwrap_or_else(|| self.start_facts_tile());
        let rows = self
            .start
            .store
            .recent
            .iter()
            .filter_map(|e| self.start.store.poster_path(&e.path).map(|p| (e.path.clone(), p)))
            .collect();
        let w = &mut self.start.welcome;
        w.open = true;
        w.primary = primary;
        w.at_whats_new = whats_new;
        w.posters.load(ctx, rows);
    }

    /// The welcome window (`kt_welcome`).
    pub(crate) fn draw_welcome(&mut self, ctx: &egui::Context) {
        if self.start.after_ffmpeg && !self.ffmpeg_ui.open {
            self.start.after_ffmpeg = false;
            let tile = self.start.welcome.primary.clone();
            self.open_welcome(ctx, Some(tile), false);
        }
        if !self.start.welcome.open || self.start.controls_ui.first_entry {
            return;
        }
        self.start.welcome.posters.poll(ctx);
        let mut act: Option<Act> = None;
        let primary = self.start.welcome.primary.clone();
        let store = &self.start.store;
        let posters = &self.start.welcome.posters;
        let scroll_news = std::mem::take(&mut self.start.welcome.at_whats_new);
        // Enter takes the primary tile (no text field has the keyboard here)
        let enter = !ctx.text_edit_focused()
            && !self.start.controls_ui.open
            && ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.any());
        let tile_go = |t: &Tile| -> Option<Act> {
            Some(match t {
                Tile::Tour => Act::Go(StartGo::Lesson { lesson: 1, fresh: false }),
                Tile::ContinueTour { lesson, .. } => Act::Go(StartGo::Lesson { lesson: *lesson, fresh: false }),
                Tile::Recent(i) => Act::Go(StartGo::Recent(store.recent.get(*i)?.path.clone())),
                Tile::NewFromTemplate(id) => Act::Go(StartGo::Template(id.clone())),
            })
        };
        if enter {
            act = tile_go(&primary);
        }
        // the update-check state already on disk (the window makes no request of its own)
        let outdated = self.update.as_ref().and_then(|u| u.outdated());
        // a sized window, never larger than the screen it opens on (D1)
        let screen = ctx.content_rect().size();
        let size = [WELCOME_SIZE[0].min(screen.x - 40.0), WELCOME_SIZE[1].min(screen.y - 60.0)];
        let win = crate::window_shell::Shell::new(
            egui::Id::new("kt_welcome"),
            tr(k::welcome::TITLE),
            crate::window_shell::Kind::Dialog,
        )
        .size(size, WELCOME_MIN)
        // centred ONCE, not anchored: an anchored window is re-centred every frame, so its corner
        // ran half as far as the pointer (the live `welcome` step: dragged 600, grew 260)
        .default_pos(ctx.content_rect().center() - egui::vec2(size[0], size[1]) / 2.0)
        .probe("welcome.window")
        .show(ctx, |ui| {
            // three panes side by side, each its own filling scroll down to the action row (D1):
            // scrolling What's new no longer scrolls the templates and the cards with it
            // (kkt-26.11.01 item 1)
            let gap = ui.spacing().item_spacing.x;
            let (start_w, recent_w, news_w) = pane_widths(ui.available_width(), gap);
            let mut seps = [0.0f32; 2];
            let row = ui.horizontal_top(|ui| {
                // ---- Start: the tour, the templates, the lessons ----
                ui.vertical(|ui| {
                    ui.set_width(start_w);
                    pane_title(ui, tr(k::welcome::START), |_| {});
                    ui.push_id("welcome_start", |ui| {
                        crate::widgets::window_body(ui, |ui| {
                            let next = crate::lessons::next_step(&store.tour);
                            let (text, is) = match (&primary, next) {
                                (Tile::ContinueTour { lesson, step }, _) => {
                                    (crate::tf!(k::welcome::CONTINUE, lesson, step), true)
                                }
                                (_, Some((l, s))) => (crate::tf!(k::welcome::CONTINUE, l, s), false),
                                (other, None) => (tr(k::welcome::TOUR).to_string(), *other == Tile::Tour),
                            };
                            let sub = tr(k::welcome::TOUR_SUB);
                            let r = start_row(ui, &text, is, "welcome.tour", |p, rect, st| {
                                let icon = egui::pos2(rect.left() + 12.0, rect.center().y);
                                p.text(icon, egui::Align2::CENTER_CENTER, crate::icons::PLAY, egui::FontId::proportional(18.0), crate::theme::ACCENT);
                                row_text(p, rect, rect.left() + 32.0, &text, sub, st);
                            });
                            if r.clicked() {
                                let lesson = next.map(|(l, _)| l).unwrap_or(1);
                                act = Some(Act::Go(StartGo::Lesson { lesson, fresh: false }));
                            }
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(tr(k::welcome::NEW_FROM_TEMPLATE)).strong());
                            let builtin = BUILTIN.iter().map(|t| (t.id.to_string(), t.label.to_string(), t.width, t.height, t.fps, false));
                            let own = store
                                .templates
                                .iter()
                                .map(|t| (user_template_id(&t.name), t.name.clone(), t.width, t.height, t.fps, true));
                            for (id, label, w, h, fps, mine) in builtin.chain(own).collect::<Vec<_>>() {
                                let is = primary == Tile::NewFromTemplate(id.clone());
                                let sub = format!("{w}×{h} · {fps} fps");
                                let name = format!("{label} {sub}");
                                let r = start_row(ui, &name, is, &format!("welcome.tpl.{id}"), |p, rect, st| {
                                    frame_glyph(p, rect, w, h, is);
                                    row_text(p, rect, rect.left() + 52.0, &label, &sub, st);
                                });
                                if r.clicked() {
                                    act = Some(Act::Go(StartGo::Template(id.clone())));
                                }
                                if mine {
                                    // a right click is never silent: the template's own menu
                                    let tname = label.clone();
                                    r.context_menu(|ui| {
                                        if ui.button(tr(k::welcome::DELETE_TEMPLATE)).clicked() {
                                            act = Some(Act::DeleteTemplate(tname.clone()));
                                            ui.close();
                                        }
                                    });
                                }
                            }
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(tr(k::welcome::LESSONS)).strong());
                            for (li, l) in crate::lessons::LESSONS.iter().enumerate() {
                                let n = li as u32 + 1;
                                let steps = l.steps.len() as u32;
                                let done = (1..=steps).filter(|s| store.tour.is_done(n, *s)).count() as u32;
                                let title = format!("{n} · {}", tr(l.title));
                                let mark = if done == steps {
                                    crate::icons::CHECK.to_string()
                                } else if done > 0 {
                                    format!("{done}/{steps}")
                                } else {
                                    String::new()
                                };
                                let name = format!("{title} {mark}");
                                let r = start_row(ui, &name, false, &format!("welcome.lesson.{n}"), |p, rect, st| {
                                    row_text(p, rect, rect.left(), &title, "", st);
                                    let c = if done == steps { crate::theme::AREA } else { st.visuals.weak_text_color() };
                                    p.text(rect.right_center(), egui::Align2::RIGHT_CENTER, &mark, egui::FontId::proportional(13.0), c);
                                });
                                if r.clicked() {
                                    act = Some(Act::Go(StartGo::Lesson { lesson: n, fresh: false }));
                                }
                            }
                        });
                    });
                });
                seps[0] = pane_gap(ui);
                // ---- Recent: poster cards that wrap and grow with the pane (D4) ----
                ui.vertical(|ui| {
                    ui.set_width(recent_w);
                    pane_title(ui, tr(k::welcome::RECENT), |ui| {
                        if !store.recent.is_empty() {
                            let r = ui.small_button(tr(k::welcome::CLEAR));
                            crate::widgets::hint(&r, tr(k::welcome::HINT_CLEAR));
                            if r.clicked() {
                                act = Some(Act::Clear);
                            }
                        }
                    });
                    ui.push_id("welcome_recent", |ui| {
                        crate::widgets::window_body(ui, |ui| {
                            if store.recent.is_empty() {
                                ui.weak(tr(k::welcome::NO_RECENT));
                            }
                            let (cols, card_w) = card_columns(ui.available_width());
                            let rows: Vec<_> = store.recent.iter().enumerate().collect();
                            for chunk in rows.chunks(cols) {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = CARD_GAP;
                                    for (i, e) in chunk {
                                        let i = *i;
                                        let is = primary == Tile::Recent(i);
                                        let exists = e.path.exists();
                                        let folder = e.path.parent().map(|p| p.display().to_string()).unwrap_or_default();
                                        let card = ui.vertical(|ui| {
                                            ui.set_width(card_w);
                                            let size = egui::vec2(card_w, (card_w * 9.0 / 16.0).round());
                                            let (rect, r) = ui.allocate_exact_size(size, egui::Sense::click());
                                            let painter = ui.painter_at(rect);
                                            match posters.get(&e.path) {
                                                Some(tex) => {
                                                    painter.image(
                                                        tex.id(),
                                                        rect,
                                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                                        egui::Color32::WHITE,
                                                    );
                                                }
                                                None => {
                                                    painter.rect_filled(rect, 3.0, crate::theme::HOVER);
                                                    painter.text(
                                                        rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        initials(&e.name),
                                                        egui::FontId::proportional(28.0),
                                                        crate::theme::AXIS_ZERO,
                                                    );
                                                }
                                            }
                                            let stroke = if is {
                                                egui::Stroke::new(2.0, crate::theme::ACCENT)
                                            } else if r.hovered() {
                                                egui::Stroke::new(1.0, crate::theme::SEAM)
                                            } else {
                                                egui::Stroke::new(1.0, crate::theme::OUTLINE)
                                            };
                                            painter.rect_stroke(rect, 3.0, stroke, egui::StrokeKind::Inside);
                                            let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
                                            // the card is a button named by its project (screen readers, the harness)
                                            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &e.name));
                                            crate::widgets::field_probe::note(&format!("welcome.recent.{i}"), r.rect);
                                            ui.add(egui::Label::new(egui::RichText::new(&e.name).strong()).truncate());
                                            if exists {
                                                ui.add(egui::Label::new(egui::RichText::new(&folder).weak().small()).truncate());
                                            } else {
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(crate::tf!(k::welcome::MISSING, &folder))
                                                            .color(crate::app::HINT_ERROR)
                                                            .small(),
                                                    )
                                                    .truncate(),
                                                );
                                                if ui.small_button(tr(k::welcome::LOCATE)).clicked() {
                                                    act = Some(Act::Locate(i));
                                                }
                                            }
                                            r
                                        });
                                        let r = card.inner;
                                        if r.clicked() {
                                            act = Some(if exists { Act::Go(StartGo::Recent(e.path.clone())) } else { Act::Locate(i) });
                                        }
                                        r.context_menu(|ui| {
                                            if ui.button(tr(k::welcome::FORGET)).clicked() {
                                                act = Some(Act::Forget(i));
                                                ui.close();
                                            }
                                        });
                                    }
                                });
                                ui.add_space(CARD_GAP);
                            }
                        });
                    });
                });
                seps[1] = pane_gap(ui);
                // ---- What's new: the page from the running version down, the Guide's renderer (D2) ----
                ui.vertical(|ui| {
                    ui.set_width(news_w);
                    let (label, blocks) = crate::guide::whats_new_from(kt_version::SHORT);
                    let head = if label == kt_version::SHORT {
                        crate::tf!(k::welcome::WHATS_NEW, &label)
                    } else {
                        crate::tf!(k::welcome::WHATS_NEW_OLDER, &label)
                    };
                    pane_title(ui, &head, |_| {});
                    ui.push_id("welcome_news", |ui| {
                        crate::widgets::window_body(ui, |ui| {
                            let top = ui.allocate_response(egui::vec2(0.0, 0.0), egui::Sense::hover());
                            if scroll_news {
                                top.scroll_to_me(Some(egui::Align::TOP));
                            }
                            if let Some(new) = &outdated {
                                let r = ui.add(
                                    egui::Label::new(egui::RichText::new(crate::update_check::title_hint(new)).color(crate::theme::BADGE_ON))
                                        .wrap(),
                                );
                                crate::widgets::field_probe::note("welcome.update", r.rect);
                            }
                            ui.weak(tr(k::guide::ENGLISH));
                            if let Some(link) = crate::guide::render(ui, blocks, None) {
                                act = Some(Act::GuideAt(link));
                            }
                        });
                    });
                });
            });
            // the rules between the panes, the row's height (a `Separator` in a horizontal row takes
            // ALL the height left, which pushed the action row down and grew the window by itself)
            let r = row.response.rect;
            for x in seps {
                ui.painter().vline(x, r.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
            }
            crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    let r = ui.button(tr(k::welcome::GUIDE));
                    crate::widgets::field_probe::note("welcome.guide", r.rect);
                    if r.clicked() {
                        act = Some(Act::Guide);
                    }
                    // whether this window opens at start is a preference: one switch, in one place
                    // (D6) — here only the door to it
                    let r = ui.button(tr(k::menu::file::PREFERENCES));
                    crate::widgets::field_probe::note("welcome.preferences", r.rect);
                    crate::widgets::hint(&r, tr(k::welcome::HINT_PREFERENCES));
                    if r.clicked() {
                        act = Some(Act::Prefs);
                    }
                    // a sized window: the right-to-left tail takes the row's room, not more
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let r = ui.button(tr(k::welcome::CLOSE));
                        crate::widgets::field_probe::note("welcome.close", r.rect);
                        if r.clicked() {
                            act = Some(Act::Close);
                        }
                    });
                });
            });
        });
        if win.closed() {
            act = act.or(Some(Act::Close));
        }
        match act {
            None => {}
            Some(Act::Close) => self.start.welcome.open = false,
            Some(Act::Prefs) => self.start.prefs_open = true,
            Some(Act::GuideAt((file, anchor))) => {
                let file = if file.is_empty() { "whats-new.md".to_string() } else { file };
                self.start.guide.open_at(&file, &anchor);
            }
            Some(Act::Guide) => {
                self.start.guide.open = true;
                self.start.guide.focus = true;
            }
            Some(Act::Clear) => {
                self.start.store.clear_recent();
                self.start.welcome.primary = self.start_facts_tile();
            }
            Some(Act::Forget(i)) => {
                if let Some(e) = self.start.store.recent.get(i).cloned() {
                    self.start.store.forget_recent(&e.path);
                    self.start.welcome.primary = self.start_facts_tile();
                }
            }
            Some(Act::DeleteTemplate(name)) => self.start.store.remove_template(&name),
            Some(Act::Locate(i)) => {
                let Some(old) = self.start.store.recent.get(i).cloned() else { return };
                if let Some(p) = crate::app::pick_file(
                    tr(k::dialog::OPEN_PROJECT),
                    "Kinetren project",
                    &[kt_model::project_file::EXTENSION],
                ) {
                    self.start.store.forget_recent(&old.path);
                    self.start_go(StartGo::Recent(p));
                }
            }
            Some(Act::Go(go)) => self.start_go(go),
        }
    }

    /// A start tile or a Help item: ask about unsaved work like any other open.
    pub(crate) fn start_go(&mut self, go: StartGo) {
        self.start.pending = Some(go);
        if self.guard_unsaved(crate::app::Pending::Start) {
            self.start_go_now();
        }
    }

    /// Do what `start_go` asked for (the question answered or not needed).
    pub(crate) fn start_go_now(&mut self) {
        let Some(go) = self.start.pending.take() else { return };
        match go {
            StartGo::Template(id) => {
                let Some((w, h, fps)) = template_by_id(&id, &self.start.store.templates) else { return };
                let app = crate::app::KtApp::new_with_project(None, project_for(w, h, fps));
                self.replace_with(app);
                self.start.store.set_last_template(&id);
                self.start.made_from = Some(id);
                self.start.welcome.open = false;
            }
            StartGo::Recent(path) => match crate::app::KtApp::open(&path) {
                Ok(mut app) => {
                    app.claim_project(&path);
                    self.replace_with(app);
                    self.start.welcome.open = false;
                }
                Err(e) => self.note(crate::tf!(k::msg::OPEN_FAILED, e)),
            },
            StartGo::Lesson { lesson, fresh } => {
                self.start.welcome.open = false;
                self.open_lesson(lesson, fresh);
            }
        }
    }

    /// The poster of the CURRENT project, if it is a named project of the person's (not a
    /// lesson sandbox): queued for the offline path (D5).
    pub(crate) fn queue_poster(&self) {
        let Some(path) = self.project_path.as_deref() else { return };
        if crate::lessons::is_sandbox(path) || self.model.project().clips.is_empty() {
            return;
        }
        let Some(out) = self.start.store.poster_path(path) else { return };
        let project = self.model.project().clone();
        let at = crate::reel_preview::poster_moment(&project, self.player.transport.playhead());
        request_poster(PosterJob { out, project, base: path.parent().map(|p| p.to_path_buf()), at });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, opened: u64) -> RecentEntry {
        RecentEntry {
            path: std::path::PathBuf::from(format!("/projects/{name}.kt")),
            name: name.into(),
            opened,
            template: Some("v916".into()),
        }
    }

    /// Test 984 (welcome-window-and-tour, ui-reach): the primary tile for each case of the spec,
    /// in its order — including FFmpeg missing and a recovery offer.
    #[test]
    fn t984_primary_tile_lands_on_the_most_likely_action() {
        let fresh = TourProgress::default();
        let finished = TourProgress { touched: 100, ..Default::default() };
        let mut stopped = TourProgress::default();
        stopped.done.entry(1).or_default().extend([1, 2, 3]);
        stopped.done.entry(2).or_default().insert(1);
        stopped.touched = 100;
        let alive = [(row("previous", 50), true)];
        let dead = [(row("previous", 50), false)];
        let newer = [(row("previous", 200), true)];
        let base = StartFacts {
            cli_project: false,
            recovery: false,
            ffmpeg_missing: false,
            show_at_start: true,
            first_start: false,
            tour: &fresh,
            tour_next: None,
            recent: &[],
            opened_since_tour: false,
            last_template: None,
        };
        let win = |t: Tile| StartAction::Window { primary: t };

        // 1. the first start (an empty store): the tour
        assert_eq!(primary(&StartFacts { first_start: true, ..base.clone() }), win(Tile::Tour));
        // 2. the tour finished, the previous project exists: the previous project
        let done = StartFacts { tour: &finished, recent: &alive, opened_since_tour: true, ..base.clone() };
        assert_eq!(primary(&done), win(Tile::Recent(0)));
        // 3. the same with the file deleted: a new project from the last-used template;
        //    the row is not removed (the window draws it flagged)
        let gone = StartFacts { recent: &dead, ..done.clone() };
        assert_eq!(primary(&gone), win(Tile::NewFromTemplate("v916".into())));
        assert_eq!(gone.recent.len(), 1, "the missing row stays");
        let gone_tpl = StartFacts { last_template: Some("4k30"), ..gone.clone() };
        assert_eq!(primary(&gone_tpl), win(Tile::NewFromTemplate("4k30".into())));
        // 4. a tour stopped at lesson 2, no project opened since: continue the tour
        let paused = StartFacts {
            tour: &stopped,
            tour_next: Some((2, 2)),
            recent: &alive,
            opened_since_tour: false,
            ..base.clone()
        };
        assert_eq!(primary(&paused), win(Tile::ContinueTour { lesson: 2, step: 2 }));
        // 5. the same, a project opened since: the previous project
        let moved_on = StartFacts { recent: &newer, opened_since_tour: true, ..paused.clone() };
        assert_eq!(primary(&moved_on), win(Tile::Recent(0)));
        // 6. a project on the command line: no welcome window
        assert_eq!(primary(&StartFacts { cli_project: true, ..done.clone() }), StartAction::NoWindow);
        // 7. a pending recovery offer: no welcome window (the recovery first)
        assert_eq!(primary(&StartFacts { recovery: true, ..done.clone() }), StartAction::NoWindow);
        // 8. FFmpeg missing: its window first, the welcome window when it closes
        assert_eq!(
            primary(&StartFacts { ffmpeg_missing: true, first_start: true, ..base.clone() }),
            StartAction::AfterFfmpeg(Box::new(win(Tile::Tour)))
        );
        // 9. "Show at start" off: no window, the last project opens as before
        assert_eq!(primary(&StartFacts { show_at_start: false, ..done.clone() }), StartAction::NoWindowOpenLast);
        // the first start beats every other fact but the command line / recovery / the switch
        assert_eq!(
            primary(&StartFacts { first_start: true, show_at_start: false, ..base.clone() }),
            StartAction::NoWindowOpenLast
        );
        // no recent and the tour never started — the tour; started and stopped — continue it
        assert_eq!(primary(&base), win(Tile::Tour));
        let no_recent = StartFacts { recent: &[], opened_since_tour: true, ..paused.clone() };
        assert_eq!(primary(&no_recent), win(Tile::ContinueTour { lesson: 2, step: 2 }));
        // finished tour, no recent: a new project from the default template
        let empty_done = StartFacts { tour: &finished, ..base.clone() };
        assert_eq!(primary(&empty_done), win(Tile::NewFromTemplate(DEFAULT_TEMPLATE.into())));
    }

    #[test]
    fn templates_resolve_builtin_and_user() {
        let user = [UserTemplate { name: "Shorts 60".into(), width: 1080, height: 1920, fps: 60 }];
        assert_eq!(template_by_id("v916", &user), Some((1080, 1920, 30)));
        assert_eq!(template_by_id(&user_template_id("Shorts 60"), &user), Some((1080, 1920, 60)));
        assert_eq!(template_by_id("nope", &user), None);
        let p = project_for(1080, 1920, 30);
        assert!(p.settings.settings_explicit && p.settings.sample_rate == 48000);
    }

    /// PROBE 1.2 (welcome-window-and-tour): the cost of one poster on the offline path. Judge it
    /// on release: `cargo test --release -p kt-app -- --ignored probe_poster_render_time --nocapture`.
    #[test]
    #[ignore]
    fn probe_poster_render_time() {
        crate::plugins::install();
        let p = crate::lessons::lesson_project();
        let ctl = crate::bg::BgCtl::from_flags(Default::default(), Default::default());
        let at = crate::reel_preview::poster_moment(&p, kt_time::Tick(0));
        let mut best = std::time::Duration::MAX;
        let mut cold = None;
        let mut colors = 0;
        for _ in 0..5 {
            let t0 = std::time::Instant::now();
            let px = crate::reel_preview::render_poster(&ctl, &p, None, at).expect("a poster");
            let dt = t0.elapsed();
            cold.get_or_insert(dt);
            best = best.min(dt);
            assert_eq!(px.len(), 320 * 180 * 4);
            colors = px.chunks(4).map(|c| (c[0], c[1], c[2])).collect::<std::collections::HashSet<_>>().len();
        }
        assert!(colors > 16, "the poster is (nearly) flat: {colors} colours — nothing was composed");
        eprintln!(
            "poster 320x180 of the lesson project at {at:?}: cold {:?}, best of 5 {best:?}, {colors} colours",
            cold.unwrap()
        );
    }
}
