//! The supervisor and the crash window (render-crash-and-recovery D2/D3).
//!
//! The owner, 2026-11-01: "I want the crash window right after the crash, not who knows when".
//! A dying process cannot show a window reliably: the panic hook runs on whatever thread failed,
//! inside the event loop that is unwinding, and a segfault, an abort or the OOM killer's SIGKILL
//! never reach a hook at all. So `kinetren` runs itself twice: the first process is a thin
//! SUPERVISOR that starts the editor as its child (the same binary, `KT_SUPERVISED=1`) and waits;
//! it initialises no window, GPU, FFmpeg or telemetry until the child ends. On an abnormal end it
//! opens the crash window itself — in a process that did not crash.
//!
//! `KT_NO_SUPERVISOR=1` runs the editor in the first process (a debugger would otherwise attach to
//! the supervisor). On Linux the child dies with the supervisor (`PR_SET_PDEATHSIG`): a `kill` of
//! what the person sees as "the program" must not leave the editor running unwatched.

use crate::i18n::{k, tr};
use eframe::egui;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Set in the child's environment: this process is the editor.
pub const ENV_CHILD: &str = "KT_SUPERVISED";
/// Run the editor without a supervisor.
pub const ENV_OFF: &str = "KT_NO_SUPERVISOR";
/// Set when the crash window's "Restore the project" started the editor again (D4).
pub const ENV_AFTER_CRASH: &str = "KT_AFTER_CRASH";

/// Lines of the crashed run's journal the window shows.
const TAIL_LINES: usize = 40;
/// Lines of the crashed run's telemetry journal the report and the window carry (segv-crashes).
const EVENT_LINES: usize = 60;

/// The editor was started by "Restore the project" in the crash window.
pub fn after_crash() -> bool {
    std::env::var_os(ENV_AFTER_CRASH).is_some()
}

/// How the child ended, as the OS reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Death {
    /// An exit code (101 is a Rust panic; on Windows the NTSTATUS of an exception).
    Code(i32),
    /// A signal (Linux).
    Signal(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Code 0: the person closed the program.
    Clean,
    /// SIGTERM, SIGINT, SIGHUP: someone asked the program to stop (a logout, `kill`, Ctrl+C).
    Requested(i32),
    /// Everything else: the crash window opens.
    Crash(Death),
}

const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

/// The pure classification of an exit (D2).
pub fn classify(code: Option<i32>, signal: Option<i32>) -> Verdict {
    match (code, signal) {
        (_, Some(s)) if [SIGTERM, SIGINT, SIGHUP].contains(&s) => Verdict::Requested(s),
        (_, Some(s)) => Verdict::Crash(Death::Signal(s)),
        (Some(0), None) => Verdict::Clean,
        (Some(c), None) => Verdict::Crash(Death::Code(c)),
        (None, None) => Verdict::Crash(Death::Code(-1)),
    }
}

/// The supervisor's own exit code: the child's, a signal as the shell writes it (128 + n).
pub fn exit_code(v: Verdict) -> i32 {
    match v {
        Verdict::Clean => 0,
        Verdict::Requested(s) | Verdict::Crash(Death::Signal(s)) => 128 + s,
        Verdict::Crash(Death::Code(c)) => c,
    }
}

fn status_parts(st: &std::process::ExitStatus) -> (Option<i32>, Option<i32>) {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        (st.code(), st.signal())
    }
    #[cfg(not(unix))]
    {
        (st.code(), None)
    }
}

/// The signal's name with what it usually means. Linux numbering (the only Unix we ship).
pub fn signal_name(s: i32) -> String {
    let n = match s {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        4 => "SIGILL (illegal instruction)",
        5 => "SIGTRAP",
        6 => "SIGABRT (abort)",
        7 => "SIGBUS (bus error)",
        8 => "SIGFPE (arithmetic error)",
        9 => "SIGKILL",
        11 => "SIGSEGV (segmentation fault)",
        13 => "SIGPIPE",
        15 => "SIGTERM",
        _ => return format!("signal {s}"),
    };
    format!("{n}, {s}")
}

/// A Windows exception code by name; `None` for an ordinary exit code.
pub fn windows_exception(c: i32) -> Option<&'static str> {
    Some(match c as u32 {
        0xC000_0005 => "access violation (0xC0000005)",
        0xC000_00FD => "stack overflow (0xC00000FD)",
        0xC000_0409 => "fast fail / abort (0xC0000409)",
        0xC000_001D => "illegal instruction (0xC000001D)",
        0xC000_0094 => "integer division by zero (0xC0000094)",
        0xC000_0017 => "out of memory (0xC0000017)",
        _ => return None,
    })
}

/// The death in plain English — the report file and stderr (not translated: the report is
/// for whoever reads the file, and every other line of it is English).
pub fn death_plain(d: Death) -> String {
    match d {
        Death::Signal(s) => format!("was ended by {}", signal_name(s)),
        Death::Code(101) => "stopped on an internal error (panic, exit code 101)".to_string(),
        Death::Code(c) => match windows_exception(c) {
            Some(e) => format!("was ended by Windows: {e}"),
            None => format!("exited with code {c}"),
        },
    }
}

/// What the crash window shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Crash {
    pub death: Death,
    /// The panic message with its place, from a FRESH report.
    pub panic: Option<String>,
    /// A native crash in one line: the signal, the thread and where (segv-crashes), from the
    /// handler's fresh raw report.
    pub fault: Option<String>,
    /// The report: the child's own (a panic) or the one the supervisor wrote.
    pub report: PathBuf,
    /// The journal lines of the crashed run.
    pub tail: Vec<String>,
    pub version: String,
}

/// The message of a panic report: `panic: panicked at <place>:` and the message on the next
/// line become "<message> — <place>".
pub fn panic_message(report: &str) -> Option<String> {
    let mut lines = report.lines();
    let first = lines.find_map(|l| l.strip_prefix("panic: "))?.trim();
    match first.strip_prefix("panicked at ") {
        Some(place) => {
            let msg = lines.next().map(str::trim).filter(|m| !m.is_empty()).unwrap_or("");
            let place = place.trim_end_matches(':');
            Some(if msg.is_empty() { place.to_string() } else { format!("{msg} — {place}") })
        }
        None => Some(first.to_string()),
    }
}

/// Gather what happened (D2): the child's FRESH report (written after it started), or a report
/// the supervisor writes itself naming the exit — "the last report is always in place" holds
/// for signals too — and the journal lines of the crashed run.
///
/// A native crash (segv-crashes): the handler's fresh raw report is symbolized HERE, in a healthy
/// process, into the report — the faulting thread and its stack, the other threads — beside the
/// last lines of the run's telemetry journal (`pid`: the child's, to find its RUN line). The raw
/// file then moves to `.seen`, so the next start does not take it again.
pub fn gather(death: Death, started: SystemTime, data_dir: &Path, pid: Option<u32>) -> Crash {
    let report = data_dir.join("kt-app-crash.txt");
    let raw = data_dir.join(crate::journal::NATIVE_RAW);
    let tail = crate::journal::last_run_tail(&data_dir.join("kt-app.log"), TAIL_LINES);
    let events = crate::tele_journal::run_tail(&data_dir.join(crate::tele_journal::FILE), pid, EVENT_LINES);
    // a second of slack: the mtime of some file systems is whole seconds
    let since = started.checked_sub(Duration::from_secs(2)).unwrap_or(started);
    let is_fresh = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).is_ok_and(|t| t >= since);
    let fresh = is_fresh(&report);
    let native = if is_fresh(&raw) { std::fs::read_to_string(&raw).ok() } else { None };
    let fault = native.as_deref().and_then(kt_crash::summary);
    let native_text = native.as_deref().map(kt_crash::symbolize);
    let panic = if fresh { std::fs::read_to_string(&report).ok().and_then(|t| panic_message(&t)) } else { None };
    let events_text = if events.is_empty() {
        String::new()
    } else {
        format!("\n--- telemetry journal, the run's last lines ---\n{}\n", events.join("\n"))
    };
    if fresh {
        // the child's own panic report; an abort after it (a panic while panicking) adds its stack
        if let Some(n) = &native_text {
            let _ = std::fs::OpenOptions::new()
                .append(true)
                .open(&report)
                .and_then(|mut f| f.write_all(format!("\n--- the native crash that followed ---\n{n}").as_bytes()));
        }
    } else {
        let _ = std::fs::create_dir_all(data_dir);
        let text = format!(
            "kinetren {}\ntime: {}\nexit: the editor {}\n{}{}{}\n--- log tail ---\n{}\n",
            kt_version::FULL,
            crate::journal::stamp_of(SystemTime::now()),
            death_plain(death),
            fault.as_deref().map_or(String::new(), |f| format!("{}{f}\n", crate::journal::SUMMARY_MARK)),
            native_text.as_deref().map_or(String::new(), |n| format!("\n--- native crash ---\n{n}")),
            events_text,
            tail.join("\n"),
        );
        let _ = std::fs::write(&report, text);
    }
    if native.is_some() {
        let _ = kt_otio::replace_file(&raw, &data_dir.join(crate::journal::NATIVE_RAW_SEEN));
    }
    // the window shows what the person did and what the program saw; the log when there is no journal
    let tail = if events.is_empty() { tail } else { events };
    Crash { death, panic, fault, report, tail, version: kt_version::FULL.to_string() }
}

/// Run the editor as a watched child. `Some(code)` — this process was the supervisor and is done;
/// `None` — run the editor here (the child itself, `KT_NO_SUPERVISOR`, or a child that could not
/// be started at all).
pub fn run() -> Option<i32> {
    if std::env::var_os(ENV_CHILD).is_some() || std::env::var_os(ENV_OFF).is_some() {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let mut after = false;
    loop {
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(&args);
        if after {
            cmd.env(ENV_AFTER_CRASH, "1");
        }
        let started = SystemTime::now();
        let (status, pid) = match spawn_watched(cmd) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("kinetren: could not start the editor as a child ({e}); running it here");
                return None;
            }
        };
        let (code, signal) = status_parts(&status);
        let v = classify(code, signal);
        let Verdict::Crash(death) = v else { return Some(exit_code(v)) };
        let crash = gather(death, started, &crate::journal::data_dir(), Some(pid));
        eprintln!("kinetren: the editor {}; the report is in {}", death_plain(death), crash.report.display());
        match show(&crash) {
            Choice::Restart => after = true,
            Choice::Close => return Some(exit_code(v)),
        }
    }
}

/// Start the child and wait for it; its exit and its pid. On Linux it gets SIGTERM when the
/// supervisor dies.
pub fn spawn_watched(mut cmd: std::process::Command) -> std::io::Result<(std::process::ExitStatus, u32)> {
    cmd.env(ENV_CHILD, "1");
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        let parent = std::process::id() as libc::pid_t;
        // SAFETY: only async-signal-safe calls between fork and exec
        unsafe {
            cmd.pre_exec(move || {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                // the supervisor died before the line above: follow it at once
                if libc::getppid() != parent {
                    libc::_exit(0);
                }
                Ok(())
            });
        }
    }
    let mut child = cmd.spawn()?;
    let pid = child.id();
    child.wait().map(|st| (st, pid))
}

/// What the person chose in the crash window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Restart,
    Close,
}

/// The death in the person's words (the window).
pub fn death_words(c: &Crash) -> String {
    match (&c.panic, c.death) {
        (Some(_), _) | (None, Death::Code(101)) => tr(k::crash::PANIC).to_string(),
        (None, Death::Signal(SIGKILL)) => tr(k::crash::KILLED).to_string(),
        (None, Death::Signal(s)) => crate::tf!(k::crash::SIGNAL, signal_name(s)),
        (None, Death::Code(code)) => match windows_exception(code) {
            Some(e) => crate::tf!(k::crash::EXCEPTION, e),
            None => crate::tf!(k::crash::CODE, code),
        },
    }
}

/// The window's body, shared by the real window and the harness (t1074). The buttons stay
/// outside the scroll (the form contract of R.66). Returns the button pressed this frame.
pub fn draw(ui: &mut egui::Ui, c: &Crash, rects: &mut Vec<(&'static str, egui::Rect)>) -> Option<Choice> {
    let mut choice = None;
    egui::Panel::bottom("kt_crash_buttons").show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let r = ui.add(egui::Button::new(egui::RichText::new(tr(k::crash::RESTORE)).strong()));
            rects.push(("crash.restore", r.rect));
            if r.clicked() {
                choice = Some(Choice::Restart);
            }
            // only the button the live scenario presses is published (t804 counts the rest)
            let r = ui.button(tr(k::crash::LOGS));
            if r.clicked()
                && let Some(dir) = c.report.parent()
                && let Err(e) = crate::util::open_in_file_manager(dir)
            {
                eprintln!("kinetren: {e}");
            }
            if ui.button(tr(k::crash::CLOSE)).clicked() {
                choice = Some(Choice::Close);
            }
        });
        ui.add_space(4.0);
    });
    egui::CentralPanel::default().show(ui, |ui| {
        ui.heading(tr(k::crash::HEADLINE));
        ui.add_space(4.0);
        ui.label(death_words(c));
        if let Some(p) = &c.panic {
            ui.label(egui::RichText::new(p).monospace());
        }
        // the signal, the thread and the function (segv-crashes): data, drawn as it is
        if let Some(f) = &c.fault {
            ui.label(egui::RichText::new(f).monospace());
        }
        ui.add_space(4.0);
        ui.label(crate::tf!(k::crash::VERSION, &c.version));
        ui.label(crate::tf!(k::crash::REPORT, c.report.display()));
        ui.label(tr(k::crash::NOTE));
        ui.separator();
        ui.label(if c.tail.is_empty() { tr(k::crash::LOG_EMPTY) } else { tr(k::crash::LOG_TAIL) });
        egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            for l in &c.tail {
                ui.label(egui::RichText::new(l).monospace().small());
            }
        });
    });
    choice
}

struct CrashWindow {
    crash: Crash,
    choice: std::rc::Rc<std::cell::Cell<Choice>>,
    marked: bool,
    dump: Option<PathBuf>,
    frames: u64,
}

impl eframe::App for CrashWindow {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // the report is marked once the window is ON SCREEN: a window that never opened leaves
        // it unmarked, and the next start announces the crash in the bell (D4)
        if !self.marked {
            crate::journal::mark_shown(&self.crash.report);
            self.marked = true;
        }
        let mut rects = Vec::new();
        if let Some(c) = draw(ui, &self.crash, &mut rects) {
            self.choice.set(c);
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if let Some(p) = &self.dump {
            self.frames += 1;
            write_dump(p, &self.crash, &rects, self.frames);
        }
    }
}

/// The live scenario's view of the window (`<KT_GEOM_DUMP>.crash.json`): the button rectangles
/// and the texts, the way the editor's dump gives `fields`.
fn write_dump(path: &Path, c: &Crash, rects: &[(&'static str, egui::Rect)], frame: u64) {
    let esc = |s: &str| s.replace('\\', "/").replace('"', "'").replace(['\n', '\r', '\t'], " ");
    let fields: Vec<String> = rects
        .iter()
        .map(|(n, r)| {
            format!(
                "{{\"param\":\"{n}\",\"x\":{:.1},\"y\":{:.1},\"w\":{:.1},\"h\":{:.1}}}",
                r.left(),
                r.top(),
                r.width(),
                r.height()
            )
        })
        .collect();
    let json = format!(
        "{{\"crash\":true,\"frame\":{frame},\"what\":\"{}\",\"panic\":\"{}\",\"version\":\"{}\",\"report\":\"{}\",\"tail\":{},\"fields\":[{}]}}\n",
        esc(&death_words(c)),
        esc(c.panic.as_deref().unwrap_or("")),
        esc(&c.version),
        esc(&c.report.to_string_lossy()),
        c.tail.len(),
        fields.join(","),
    );
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// The editor's language for the window: `KT_LANG`, else the `lang` entry the editor stored in
/// `app.ron` (read as text: the window must not write eframe's storage — it would overwrite the
/// editor's window geometry), else the system's.
fn language() -> crate::i18n::Lang {
    let stored = || {
        let ron = std::fs::read_to_string(crate::journal::data_dir().join(kt_dirs::EFRAME_SETTINGS_FILE)).ok()?;
        stored_lang(&ron)
    };
    let code = std::env::var("KT_LANG").ok().or_else(stored);
    crate::i18n::resolve(code.as_deref().and_then(crate::i18n::choice_from_code))
}

/// The `"lang"` value of eframe's RON map (`{"lang":"ru", …}`).
pub(crate) fn stored_lang(ron: &str) -> Option<String> {
    let at = ron.find("\"lang\"")? + "\"lang\"".len();
    let rest = ron[at..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

/// Open the crash window and wait for the person's choice. A window that cannot open (no
/// display) is a `Close`: the report path is already on stderr.
fn show(crash: &Crash) -> Choice {
    let lang = language();
    crate::i18n::set_lang(lang);
    let choice = std::rc::Rc::new(std::cell::Cell::new(Choice::Close));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([680.0, 480.0])
            .with_min_inner_size([480.0, 320.0])
            .with_icon(crate::app::app_icon())
            .with_app_id(crate::app::APP_ID)
            .with_title(tr(k::crash::TITLE)),
        persist_window: false,
        persistence_path: None,
        ..Default::default()
    };
    let dump = std::env::var_os("KT_GEOM_DUMP").map(|p| {
        let mut p = p;
        p.push(".crash.json");
        PathBuf::from(p)
    });
    let app = CrashWindow { crash: crash.clone(), choice: choice.clone(), marked: false, dump, frames: 0 };
    let res = eframe::run_native(
        "kinetren-crash",
        options,
        Box::new(move |cc| {
            crate::theme::apply(&cc.egui_ctx);
            crate::i18n::set_lang_ctx(&cc.egui_ctx, lang);
            crate::icons::install_fonts(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    );
    if let Err(e) = res {
        eprintln!("kinetren: the crash window could not open: {e}");
    }
    choice.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    const CHILD: &str = "KT_T1072_CHILD";

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kt-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// This test binary run as the child: only the named test, its data directory a temporary one.
    fn child(test: &str, mode: &str, data: &Path) -> Command {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args(["--exact", test, "--nocapture", "--test-threads=1"])
            .env(CHILD, mode)
            .env("KINETREN_DATA_DIR", data)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        c
    }

    fn run_child(test: &str, mode: &str, data: &Path) -> (Verdict, SystemTime) {
        let started = SystemTime::now();
        let (st, _) = spawn_watched(child(test, mode, data)).unwrap();
        let (code, signal) = status_parts(&st);
        (classify(code, signal), started)
    }

    /// The child's body: does something only when a test above started it.
    #[test]
    fn t1072_child_body() {
        let Ok(mode) = std::env::var(CHILD) else { return };
        assert_eq!(std::env::var(ENV_CHILD).as_deref(), Ok("1"), "the child is not marked supervised");
        crate::journal::start_run_at(&crate::journal::path(), "FFmpeg test");
        crate::journal::log("INFO", "t1072 child: about to end");
        match mode.as_str() {
            "panic" => {
                crate::journal::install_panic_hook();
                // the panic is the test failing: the harness exits with 101, as a panicking
                // `main` does
                panic!("t1072: a deliberate panic in the child");
            }
            #[cfg(target_os = "linux")]
            "abort" => std::process::abort(),
            #[cfg(target_os = "linux")]
            "segv" => {
                // the editor's own start: the telemetry journal's RUN line, then the handler
                crate::tele_journal::install();
                crate::tele_journal::record(crate::tele_journal::Kind::Act, "t2084 child: about to fault");
                crate::tele_journal::flush(std::time::Duration::from_secs(5));
                crate::journal::install_crash_handler();
                let t = std::thread::Builder::new().name("t2084-fault".into());
                let _ = t.spawn(kt_crash::testing::kt_crash_probe_segv).map(|h| h.join());
            }
            #[cfg(target_os = "linux")]
            "kill" => unsafe {
                libc::raise(libc::SIGKILL);
            },
            #[cfg(target_os = "linux")]
            "term" => unsafe {
                libc::raise(libc::SIGTERM);
            },
            _ => std::process::exit(0),
        }
    }

    /// Test 1072 (D2): a child that panics is detected, its own report is taken (and keeps its
    /// backtrace), and the journal tail is the child's run.
    #[test]
    fn t1072_child_panic_is_detected() {
        let dir = scratch("t1072");
        let data = dir.join("kinetren");
        let (v, started) = run_child("supervisor::tests::t1072_child_body", "panic", &data);
        assert_eq!(v, Verdict::Crash(Death::Code(101)), "the child's panic is not a crash");
        let c = gather(Death::Code(101), started, &data, None);
        let msg = c.panic.as_deref().unwrap_or_default();
        assert!(msg.contains("t1072: a deliberate panic in the child"), "the panic message: {c:?}");
        assert!(msg.contains("supervisor.rs"), "the panic's place is lost: {msg}");
        let report = std::fs::read_to_string(&c.report).unwrap();
        assert!(report.contains("install_panic_hook"), "the report is not the child's own: {report}");
        assert!(c.tail.iter().any(|l| l.contains("t1072 child: about to end")), "{:?}", c.tail);
        assert!(c.tail[0].starts_with(crate::journal::RUN_HEADER), "the tail is not this run's: {:?}", c.tail);
        assert_eq!(c.version, kt_version::FULL);
        // the window marks the report; the next start takes it without a second bell line
        crate::journal::mark_shown(&c.report);
        let taken =
            crate::journal::take_crash_report_at(&c.report, &data.join("kt-app-crash.seen.txt")).unwrap();
        assert!(taken.shown);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Test 2084 (segv-crashes): a native crash of the editor leaves a report that names the
    /// signal, the faulting thread and its function with file and line, the other threads, and the
    /// run's last telemetry journal lines; the window gets the one-line summary and those lines; the
    /// raw report is taken once.
    #[cfg(target_os = "linux")]
    #[test]
    fn t2084_native_crash_report_names_thread_stack_and_journal() {
        let dir = scratch("t2084");
        let data = dir.join("kinetren");
        let started = SystemTime::now();
        let (st, pid) = spawn_watched(child("supervisor::tests::t1072_child_body", "segv", &data)).unwrap();
        let (code, signal) = status_parts(&st);
        assert_eq!(classify(code, signal), Verdict::Crash(Death::Signal(11)), "the editor dies by SIGSEGV");
        assert!(data.join(crate::journal::NATIVE_RAW).exists(), "the handler wrote no raw report");
        let c = gather(Death::Signal(11), started, &data, Some(pid));
        let fault = c.fault.clone().unwrap_or_default();
        assert!(fault.starts_with("SIGSEGV in thread \"t2084-fault\" at kt_crash::testing::kt_crash_probe_segv"), "{fault}");
        let report = std::fs::read_to_string(&c.report).unwrap();
        for want in [
            "exit: the editor was ended by SIGSEGV".to_string(),
            format!("{}{fault}", crate::journal::SUMMARY_MARK),
            "thread: t2084-fault".to_string(),
            "kt_crash::testing::kt_crash_probe_segv (libs/kt-crash/src/lib.rs:".to_string(),
            "t2084-fault  ← faulted".to_string(),
            "--- telemetry journal, the run's last lines ---".to_string(),
            "t2084 child: about to fault".to_string(),
        ] {
            assert!(report.contains(&want), "the report misses “{want}”:\n{report}");
        }
        assert!(c.tail.iter().any(|l| l.contains("t2084 child: about to fault")), "the window's lines: {:?}", c.tail);
        assert!(c.tail[0].contains(&format!(" pid {pid}")), "the lines are not this run's: {:?}", c.tail);
        // taken once: the raw report moved aside, a second gather finds no native crash
        assert!(!data.join(crate::journal::NATIVE_RAW).exists() && data.join(crate::journal::NATIVE_RAW_SEEN).exists());
        assert_eq!(crate::journal::report_summary(&c.report).as_deref(), Some(fault.as_str()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Test 2085 (segv-crashes): with no supervisor (`KT_NO_SUPERVISOR`, a supervisor killed
    /// with its child) the next start symbolizes the raw report into the crash report, which the
    /// bell then announces.
    #[cfg(target_os = "linux")]
    #[test]
    fn t2085_a_raw_report_is_absorbed_at_the_next_start() {
        let dir = scratch("t2085");
        let data = dir.join("kinetren");
        let (st, _) = spawn_watched(child("supervisor::tests::t1072_child_body", "segv", &data)).unwrap();
        assert_eq!(status_parts(&st).1, Some(11));
        crate::journal::absorb_native_report_in(&data);
        let report = data.join("kt-app-crash.txt");
        let summary = crate::journal::report_summary(&report).expect("no summary in the absorbed report");
        assert!(summary.starts_with("SIGSEGV in thread \"t2084-fault\""), "{summary}");
        assert!(std::fs::read_to_string(&report).unwrap().contains("t2084 child: about to fault"));
        assert!(!data.join(crate::journal::NATIVE_RAW).exists(), "taken twice");
        let taken = crate::journal::take_crash_report_at(&report, &data.join("kt-app-crash.seen.txt")).unwrap();
        assert!(!taken.shown, "no window showed it: the bell announces it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Test 1073 (D2): an abort and a SIGKILL are crashes with a report the supervisor wrote;
    /// SIGTERM and a clean exit open nothing; the Windows codes have names.
    #[test]
    fn t1073_exits_are_classified() {
        #[cfg(target_os = "linux")]
        {
            let dir = scratch("t1073");
            let data = dir.join("kinetren");
            let body = "supervisor::tests::t1072_child_body";
            let (v, started) = run_child(body, "abort", &data);
            assert_eq!(v, Verdict::Crash(Death::Signal(6)), "abort");
            let c = gather(Death::Signal(6), started, &data, None);
            assert_eq!(c.panic, None);
            let report = std::fs::read_to_string(&c.report).unwrap();
            assert!(report.contains("SIGABRT") && report.contains(kt_version::FULL), "{report}");
            assert!(report.contains("t1072 child: about to end"), "the report has no log tail: {report}");
            assert!(death_words(&c).contains("SIGABRT"), "{}", death_words(&c));

            let (v, _) = run_child(body, "kill", &data);
            assert_eq!(v, Verdict::Crash(Death::Signal(9)), "SIGKILL");
            crate::i18n::set_lang(crate::i18n::Lang::En);
            let c = Crash { death: Death::Signal(9), panic: None, ..c };
            assert!(death_words(&c).contains("memory"), "{}", death_words(&c));

            assert_eq!(run_child(body, "term", &data).0, Verdict::Requested(15), "SIGTERM opens a window");
            assert_eq!(run_child(body, "clean", &data).0, Verdict::Clean);
            let _ = std::fs::remove_dir_all(&dir);
        }
        assert_eq!(classify(Some(0), None), Verdict::Clean);
        assert_eq!(classify(None, Some(2)), Verdict::Requested(2));
        assert_eq!(classify(None, Some(1)), Verdict::Requested(1));
        assert_eq!(classify(None, Some(11)), Verdict::Crash(Death::Signal(11)));
        assert_eq!(classify(Some(101), None), Verdict::Crash(Death::Code(101)));
        assert_eq!(exit_code(Verdict::Crash(Death::Signal(11))), 139);
        assert_eq!(exit_code(Verdict::Requested(15)), 143);
        for (code, word) in [(0xC000_0005u32, "access violation"), (0xC000_00FD, "stack overflow"), (0xC000_0409, "abort")] {
            let c = code as i32;
            assert!(windows_exception(c).is_some_and(|w| w.contains(word)), "{code:#x}");
            assert!(death_plain(Death::Code(c)).contains(word));
        }
        assert_eq!(windows_exception(1), None);
        assert_eq!(death_plain(Death::Code(3)), "exited with code 3");
    }

    /// Test 1074 (D3): the window says what happened, names the version, the report and the log,
    /// and its buttons below the scrolling body give the choice.
    #[test]
    fn t1074_crash_window_shows_and_offers() {
        use egui_kittest::kittest::Queryable;
        crate::i18n::set_lang(crate::i18n::Lang::En);
        let crash = Crash {
            death: Death::Code(101),
            panic: Some("cannot convert float seconds to Duration: value is negative — render_dialog.rs:937:52".into()),
            fault: None,
            report: PathBuf::from("/tmp/kt-t1074/kt-app-crash.txt"),
            tail: (0..60).map(|i| format!("2026-11-01 18:10:{:02}+03:00 INFO  line {i}", i % 60)).collect(),
            version: kt_version::FULL.to_string(),
        };
        struct St {
            crash: Crash,
            choice: Option<Choice>,
        }
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(560.0, 360.0)).build_ui_state(
            |ui, st: &mut St| {
                let mut rects = Vec::new();
                if let Some(c) = draw(ui, &st.crash, &mut rects) {
                    st.choice = Some(c);
                }
            },
            St { crash, choice: None },
        );
        crate::icons::install_fonts(&h.ctx);
        h.step();
        h.step();
        for want in [
            tr(k::crash::HEADLINE).to_string(),
            tr(k::crash::PANIC).to_string(),
            "cannot convert float seconds to Duration: value is negative — render_dialog.rs:937:52".to_string(),
            crate::tf!(k::crash::VERSION, kt_version::FULL),
            crate::tf!(k::crash::REPORT, "/tmp/kt-t1074/kt-app-crash.txt"),
            tr(k::crash::LOG_TAIL).to_string(),
        ] {
            assert!(h.query_by_label(&want).is_some(), "the window does not show “{want}”");
        }
        // the log sticks to its END: the last line is what the person sees first
        assert!(h.query_by_label_contains("line 59").is_some(), "the last log line is not shown");
        let screen = h.ctx.content_rect();
        for b in [k::crash::RESTORE, k::crash::LOGS, k::crash::CLOSE] {
            let r = h.get_by_label(tr(b)).rect();
            assert!(screen.contains_rect(r), "“{}” left a low window: {r:?}", tr(b));
        }
        h.get_by_label(tr(k::crash::RESTORE)).click();
        h.step();
        h.step();
        assert_eq!(h.state().choice, Some(Choice::Restart));
        h.state_mut().choice = None;
        h.get_by_label(tr(k::crash::CLOSE)).click();
        h.step();
        h.step();
        assert_eq!(h.state().choice, Some(Choice::Close));
        // the language the window takes from the editor's stored settings
        assert_eq!(stored_lang("{\"last_project\":\"/x.kt\",\"lang\": \"ru\"}").as_deref(), Some("ru"));
        assert_eq!(stored_lang("{}"), None);
    }
}
