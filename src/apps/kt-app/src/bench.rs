//! LIVE BENCH (R.68): one kt-app process per worker, projects in succession.
//!
//! The reason, verbatim (the owner): “reduce the restarts of kt (you can open projects
//! one after another)”. The R.68 measurement: 50 live scenarios cost 1012 s, and
//! each paid 3–5 s for the cold start of the program, while 96 % of the remaining
//! time the harness simply SLEPT — fixed pauses for the software renderer
//! (0.35 s per click, 0.65 s for EACH typed character), because there was no one
//! to ask about a drawn frame.
//!
//! The bench closes both:
//!
//! * `frames()` — how many frames the program has DRAWN. The harness waits for a frame, not for
//!   the clock: under load the wait stretches together with the frame — exactly
//!   what the fixed pause could not do, and false reds grew from it
//!   (§468: a second run under load turned ~14 green scenarios red).
//! * a command channel: “open a project” instead of restarting the process. Opening
//!   goes through the same code as `File → Open` (`KtApp::open` + `*self`), so
//!   the new state of the program is built from scratch — cleanliness is not
//!   promised, it is obtained by construction.
//!
//! The channel is enabled ONLY by `KT_LIVE_CTL=<file>`; without the variable there is
//! just one `OnceLock` check here and not a single system operation. The protocol is a line
//! “<number> <command> [argument]”: the program executes a command with a number
//! greater than the one already executed and publishes it in the geometry dump
//! (`bench.done`), from which the harness learns that the command passed. The number is not
//! “there is a new line”: the file is rewritten whole, and without a counter two
//! identical commands in a row (“open the same project”) would be indistinguishable.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Drawn frames. The counter of the PROCESS, not of the application: `File → Open`
/// recreates `KtApp` in place (`*self = app`), and a counter inside the struct would
/// reset on every open — the harness would wait for a frame that had “already been”
/// drawn.
static FRAMES: AtomicU64 = AtomicU64::new(0);
/// number of the last EXECUTED command (for the same reason — a static)
static DONE: AtomicU64 = AtomicU64::new(0);
/// How many INPUT EVENTS the program has read. A frame is not enough: a frame may have started
/// before the event arrived from the X server, and “frame drawn” does not yet mean
/// “gesture seen”. The event counter answers exactly this question.
static INPUTS: AtomicU64 = AtomicU64::new(0);
/// timestamp of the command file: there is no need to read it every frame
static LAST_CTL: AtomicU64 = AtomicU64::new(0);

fn ctl_path() -> Option<&'static PathBuf> {
    static P: OnceLock<Option<PathBuf>> = OnceLock::new();
    P.get_or_init(|| std::env::var_os("KT_LIVE_CTL").map(PathBuf::from)).as_ref()
}

/// Is the bench on? In the product — no, and none of the branches below work.
pub(crate) fn enabled() -> bool {
    ctl_path().is_some()
}

/// Frame drawn. Called from `eframe::App::ui` — that is, the counter counts
/// exactly the SHOWN frames, not layout passes.
pub(crate) fn tick() {
    FRAMES.fetch_add(1, Ordering::Relaxed);
}

/// The program has read the input events of this frame.
pub(crate) fn note_inputs(n: u64) {
    if n > 0 {
        INPUTS.fetch_add(n, Ordering::Relaxed);
    }
}

pub(crate) fn inputs() -> u64 {
    INPUTS.load(Ordering::Relaxed)
}

pub(crate) fn frames() -> u64 {
    FRAMES.load(Ordering::Relaxed)
}

pub(crate) fn done() -> u64 {
    DONE.load(Ordering::Relaxed)
}

/// A bench command.
pub(crate) enum Cmd {
    /// Open a project by path (without the question about unsaved work: the bench is not
    /// a person, and a modal window left by the previous scenario would break the
    /// next one).
    Open(PathBuf),
    /// The user store for the NEXT scenario. The workspace,
    /// presets and log of the previous scenario have no right to influence the
    /// verdict of the next one: `nestpreview` was green alone and red
    /// in a full run for exactly this reason.
    Store(PathBuf),
    /// Do nothing — needed so that the harness can wait for a frame and
    /// make sure the program is alive and listening.
    Noop,
}

/// The store assigned by the bench (see `Cmd::Store`). `user_store::store_dir`
/// asks it first: the environment variable of a live process cannot be changed from outside,
/// and the store must be its own for each scenario.
static STORE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

pub(crate) fn set_store(dir: PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut g) = STORE.lock() {
        *g = Some(dir);
    }
}

/// The store directory assigned by the bench (or `None` in a normal launch).
pub(crate) fn store_dir() -> Option<PathBuf> {
    if !enabled() {
        return None;
    }
    STORE.lock().ok().and_then(|g| g.clone())
}

/// Read the command if it is new. A parse error is NOT a panic: the bench
/// reads a file that another process is writing, and catching it mid-write is legitimate
/// (the next frame will read the whole thing).
pub(crate) fn poll() -> Option<(u64, Cmd)> {
    let path = ctl_path()?;
    // WE READ ONLY A CHANGED FILE: `poll` is called every frame, and reading the
    // file within a frame is I/O in the draw path. This round had already broken fine gestures
    // with the same class (writing a dump every frame), so first a cheap `stat`, and only on a
    // new timestamp — the read.
    let stamp = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    if stamp == LAST_CTL.load(Ordering::Relaxed) {
        return None;
    }
    LAST_CTL.store(stamp, Ordering::Relaxed);
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().next()?.trim();
    let mut it = line.splitn(3, ' ');
    let seq: u64 = it.next()?.parse().ok()?;
    if seq <= DONE.load(Ordering::Relaxed) {
        return None;
    }
    let cmd = match it.next()? {
        "open" => Cmd::Open(PathBuf::from(it.next()?.trim())),
        "store" => Cmd::Store(PathBuf::from(it.next()?.trim())),
        "noop" => Cmd::Noop,
        _ => return None,
    };
    Some((seq, cmd))
}

/// The command is executed — the harness will see the number in the dump and move on.
pub(crate) fn finished(seq: u64) {
    DONE.store(seq, Ordering::Relaxed);
}

/// How many program windows are open right now.
///
/// Proof of CLEANLINESS between scenarios: the bench opens projects in
/// succession, and a scenario must start with a program without anyone else's windows. We count
/// the visible egui layers above the content — WINDOWS (`Order::Middle`).
///
/// `Order::Foreground` does not fit here, although modal windows live there:
/// the status line is drawn there too (`Area(Foreground)`, the rule “one hint in one place”),
/// and “clean” turned into “one window open” out of nowhere. The program names
/// modal windows itself —
/// `ask`/`recovery_open` are added to this number at the call site.
pub(crate) fn open_windows(ctx: &eframe::egui::Context) -> usize {
    ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|l| l.order == eframe::egui::Order::Middle)
            .count()
    })
}
