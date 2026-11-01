//! The program journal — the only channel when there is no one to show it to.
//!
//! Why it exists at all (R.49, item 350). Everything the program wants to say goes into
//! the bell. But the window's `on_exit` is ALREADY GONE: egui is closed, there will be no more
//! frames, and our `.desktop` has `Terminal=false` — output to stderr goes
//! nowhere (at best into journald, where the editor will never look). The project's
//! final record lives exactly there: a failure on it is the worst possible, and before
//! R.49 it left no trace. The same goes for the panic hook (item 355): there is no one to show
//! the report to, the window is already dead.
//!
//! Therefore the journal does NOT duplicate the bell: it records what the
//! person would otherwise never see, plus warnings — so that the complaint
//! “it said something and it vanished” has an address.
//!
//! Format — one line per record: `YYYY-MM-DD HH:MM:SS+HH:MM level text`, LOCAL time with its
//! offset (render-crash-and-recovery D5: the journal said UTC without saying so). ONE FILE PER
//! RUN: `start_run` moves the previous run to `kt-app.log.1` (a fan of four previous runs) and
//! opens the new file with a header; past 1 MiB a run shifts the same fan. The owner's crash
//! (kkt-26.11.01 item 9) came with a journal of several runs glued together: "it seems the log
//! from an old run stayed".

use std::io::Write;
use std::path::{Path, PathBuf};

/// The rotation threshold. A megabyte is about ten thousand lines: more than the
/// program produces in a day of work, and less than is noticeable on disk.
pub const ROTATE_BYTES: u64 = 1024 * 1024;
/// Previous runs kept next to the current journal (`kt-app.log.1` … `.4`).
pub const KEEP_RUNS: usize = 4;
/// The start of a run's header line — what `last_run_tail` cuts at.
pub const RUN_HEADER: &str = "==== kinetren ";
/// The line the supervisor appends to a crash report it showed in its window (D4).
pub const SHOWN_MARK: &str = "shown: ";

/// Name of the per-user data directory under `$XDG_DATA_HOME`.
pub const DATA_DIR_NAME: &str = "kinetren";
/// The directory of the program's former name (before 2026-10-21). It is
/// moved into place once (`data_dir_in`); nothing reads it afterwards.
pub const FORMER_DATA_DIR_NAME: &str = "kinotrain";

/// The program's data directory (`$XDG_DATA_HOME/kinetren`, `%APPDATA%\kinetren\data`).
/// Shared with the autosave — there is no need for a separate home for the journal.
/// Recovery, journal, locks, presets, workspace and eframe's own storage (`app.ron`,
/// `NativeOptions::persistence_path`) all live here. The resolver is `kt-dirs`, shared with
/// kt-cli; it never answers with the working directory (windows-build).
pub fn data_dir() -> PathBuf {
    let dir = kt_dirs::data_dir();
    // The former name existed only on Linux, and only a REAL per-user root may hold it:
    // migrating inside a fallback (temp) directory would move someone else's folder.
    #[cfg(target_os = "linux")]
    if !kt_dirs::is_fallback()
        && let Some(base) = dir.parent()
    {
        return data_dir_in(base);
    }
    dir
}

/// `base/kinetren`, after moving the former `base/kinotrain` into place if
/// only that one exists (kinetren-rename). A rename is atomic on one
/// filesystem; if it fails, the content is copied into a staging directory
/// that is then renamed, and the former directory is left as it was. When
/// both exist the new one wins and nothing is merged: a merge could bring
/// back deleted presets or stale locks.
pub fn data_dir_in(base: &Path) -> PathBuf {
    let new = base.join(DATA_DIR_NAME);
    let former = base.join(FORMER_DATA_DIR_NAME);
    if !new.exists() && former.is_dir() && std::fs::rename(&former, &new).is_err() {
        let staging = base.join(format!("{DATA_DIR_NAME}.moving"));
        let _ = std::fs::remove_dir_all(&staging);
        if copy_tree(&former, &staging).is_ok() {
            let _ = std::fs::rename(&staging, &new);
        } else {
            let _ = std::fs::remove_dir_all(&staging);
        }
    }
    new
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dst = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &dst)?;
        } else {
            std::fs::copy(e.path(), dst)?;
        }
    }
    Ok(())
}

pub fn path() -> PathBuf {
    data_dir().join("kt-app.log")
}

/// Report about the last crash. One file: it is overwritten by each new
/// crash, and the read one moves to `.seen` — so the warning appears
/// exactly once per crash, while the last report is always in place.
pub fn crash_path() -> PathBuf {
    data_dir().join("kt-app-crash.txt")
}

pub fn crash_seen_path() -> PathBuf {
    data_dir().join("kt-app-crash.seen.txt")
}

/// “Black box” for the panic hook. Stored SEPARATELY from the application:
/// the hook has no right to touch the model (invariant 2 — the FX state belongs to the
/// render thread; invariant 6 — in a callback, no locks and no allocations),
/// while the event ring is self-sufficient and can be read from any thread.
static FLIGHT: std::sync::OnceLock<
    std::sync::Mutex<Option<std::sync::Arc<kt_engine::flight::FlightRecorder>>>,
> = std::sync::OnceLock::new();

fn flight_slot()
-> &'static std::sync::Mutex<Option<std::sync::Arc<kt_engine::flight::FlightRecorder>>> {
    FLIGHT.get_or_init(|| std::sync::Mutex::new(None))
}

pub fn set_flight(f: std::sync::Arc<kt_engine::flight::FlightRecorder>) {
    if let Ok(mut slot) = flight_slot().lock() {
        *slot = Some(f);
    }
}

/// Install the panic hook. The division of labor with `Drop for KtApp` (item 354)
/// is direct: **the hook is diagnostics, `Drop` is data**. The hook does not save the project
/// (one may not reach into the model from an arbitrary panicking thread) and does not interfere
/// with unwinding: the previous hook is called next, and `Drop` runs its own course.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let out = crash_report(&info.to_string());
        let p = crash_path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(&p, &out);
        log("CRASH", &format!("panic report written to {}", p.display()));
        prev(info);
    }));
}

/// The session the report will name (telemetry's `$session_id`, set once at start): the next
/// start's `app_crashed` says which session ended in the crash (telemetry D8).
static SESSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_session(id: &str) {
    let _ = SESSION.set(id.to_string());
}

/// The body of the report. Taken out of the hook so that it can be checked without
/// replacing the global hook in a test (the replacement would affect neighboring tests).
pub(crate) fn crash_report(panic_text: &str) -> String {
    let dump = flight_slot().lock().ok().and_then(|s| s.as_ref().map(|f| f.dump()));
    crash_report_with(panic_text, dump.as_deref(), SESSION.get().map(String::as_str))
}

pub(crate) fn crash_report_with(
    panic_text: &str,
    flight: Option<&str>,
    session: Option<&str>,
) -> String {
    let mut out = String::with_capacity(8 * 1024);
    // the FULL version even in a release build: "which exact build crashed"
    out.push_str(&format!("kinetren {}\n", kt_version::FULL));
    out.push_str(&format!("time: {}\n", stamp()));
    let t = std::thread::current();
    out.push_str(&format!("thread: {}\n", t.name().unwrap_or("<unnamed>")));
    if let Some(s) = session {
        out.push_str(&format!("session: {s}\n"));
    }
    out.push_str(&format!("panic: {panic_text}\n\n"));
    out.push_str(&format!("{}\n", std::backtrace::Backtrace::force_capture()));
    // dump of the “black box”: what the program was doing in the last seconds
    if let Some(f) = flight {
        out.push_str("\n--- flight recorder ---\n");
        out.push_str(f);
    }
    out
}

/// A report taken at start: where it lies now, and whether the crash window already showed it.
#[derive(Debug, Clone, PartialEq)]
pub struct TakenReport {
    pub path: PathBuf,
    pub shown: bool,
}

/// If the previous session left a report — return it ONCE and set the report itself aside.
pub fn take_crash_report() -> Option<TakenReport> {
    take_crash_report_at(&crash_path(), &crash_seen_path())
}

pub(crate) fn take_crash_report_at(p: &Path, seen: &Path) -> Option<TakenReport> {
    if !p.exists() {
        return None;
    }
    let shown = std::fs::read_to_string(p)
        .map(|t| t.lines().any(|l| l.starts_with(SHOWN_MARK)))
        .unwrap_or(false);
    let path = match kt_otio::replace_file(p, seen) {
        Ok(()) => seen.to_path_buf(),
        // renaming did not work — still must say so, the report is in place
        Err(_) => p.to_path_buf(),
    };
    Some(TakenReport { path, shown })
}

/// The crash window showed this report (D4): the next start takes it without a second bell line.
pub fn mark_shown(report: &Path) {
    let _ = std::fs::OpenOptions::new()
        .append(true)
        .open(report)
        .and_then(|mut f| f.write_all(format!("\n{SHOWN_MARK}{}\n", stamp()).as_bytes()));
}

/// Open this run's journal (D5): the previous run moves into the fan, the new file starts with
/// the header. Called once, at the editor's start.
pub fn start_run() {
    let ffmpeg = match kt_ffmpeg::backend_info() {
        Some(i) => format!("FFmpeg {}", i.major),
        None => "FFmpeg missing".to_string(),
    };
    start_run_at(&path(), &ffmpeg);
}

pub(crate) fn start_run_at(path: &Path, ffmpeg: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false) {
        shift(path);
    }
    let header = format!(
        "{RUN_HEADER}{} · run {} · pid {} · {} {} · {ffmpeg} ====\n",
        kt_version::FULL,
        stamp(),
        std::process::id(),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(header.as_bytes()));
}

/// The last `n` lines of the LAST run in the journal (after its header) — what the crash window
/// shows. A journal without a header (an older build) gives its last lines.
pub fn last_run_tail(path: &Path, n: usize) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let lines: Vec<&str> = text.lines().collect();
    let from = lines.iter().rposition(|l| l.starts_with(RUN_HEADER)).unwrap_or(0);
    let run = &lines[from..];
    run[run.len().saturating_sub(n)..].iter().map(|l| l.to_string()).collect()
}

/// Append a line. Journal write errors are SWALLOWED on purpose: the journal is
/// an auxiliary channel, and crashing or making noise because of it, when the user
/// is just losing a project, would be mockery.
pub fn log(level: &str, text: &str) {
    write_to(&path(), level, text);
}

pub(crate) fn write_to(path: &Path, level: &str, text: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    rotate(path);
    let line = format!("{} {:<5} {}\n", stamp(), level, text.replace('\n', " | "));
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

fn rotate(path: &Path) {
    let big = std::fs::metadata(path).map(|m| m.len() >= ROTATE_BYTES).unwrap_or(false);
    if big {
        shift(path);
    }
}

/// Move the journal into the fan: `.3 → .4`, …, `kt-app.log → .1`; the oldest falls off. On
/// Windows a held file is retried (kt-otio).
fn shift(path: &Path) {
    let nth = |i: usize| nth_run(path, i);
    for i in (1..KEEP_RUNS).rev() {
        if nth(i).exists() {
            let _ = kt_otio::replace_file(&nth(i), &nth(i + 1));
        }
    }
    let _ = kt_otio::replace_file(path, &nth(1));
}

/// The journal of the `i`-th previous run: `kt-app.log` → `kt-app.log.<i>`.
pub(crate) fn nth_run(path: &Path, i: usize) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(format!(".{i}"));
    PathBuf::from(s)
}

/// Date and time without external dependencies: the program does not pull in chrono for the
/// sake of one line. The algorithm is civil_from_days by Howard Hinnant.
fn stamp() -> String {
    stamp_of(std::time::SystemTime::now())
}

/// The same label for an arbitrary moment, in local time with its offset — the recovery dialog
/// uses it to show the time of each candidate (item 353; its first 16 characters are the
/// minute).
pub fn stamp_of(t: std::time::SystemTime) -> String {
    let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    stamp_with(secs, local_offset(secs))
}

/// `YYYY-MM-DD HH:MM:SS±HH:MM` for a Unix second and an offset in seconds east of UTC.
pub(crate) fn stamp_with(unix_secs: i64, offset_secs: i64) -> String {
    let local = unix_secs + offset_secs;
    let (days, rem) = (local.div_euclid(86_400), local.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    let sign = if offset_secs < 0 { '-' } else { '+' };
    let off = offset_secs.unsigned_abs() / 60;
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}{sign}{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        off / 60,
        off % 60
    )
}

/// The local offset from UTC at that moment, in seconds east (D5): `localtime_r`'s `tm_gmtoff`.
#[cfg(target_os = "linux")]
fn local_offset(unix_secs: i64) -> i64 {
    let t = unix_secs as libc::time_t;
    // SAFETY: a zeroed plain-data `tm` filled by the reentrant call; `t` outlives the call
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        return 0;
    }
    tm.tm_gmtoff as i64
}

/// Windows: the zone's CURRENT bias (a moment across a daylight change is an hour off; the journal
/// stamps "now", the recovery dialog a recent file).
#[cfg(windows)]
fn local_offset(_unix_secs: i64) -> i64 {
    use windows_sys::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
    const TIME_ZONE_ID_DAYLIGHT: u32 = 2;
    const TIME_ZONE_ID_INVALID: u32 = u32::MAX;
    // SAFETY: a zeroed plain-data struct filled by the call
    let mut tz: TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
    let r = unsafe { GetTimeZoneInformation(&mut tz) };
    if r == TIME_ZONE_ID_INVALID {
        return 0;
    }
    let bias = tz.Bias + if r == TIME_ZONE_ID_DAYLIGHT { tz.DaylightBias } else { tz.StandardBias };
    -(bias as i64) * 60
}

#[cfg(not(any(target_os = "linux", windows)))]
fn local_offset(_unix_secs: i64) -> i64 {
    0
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kt-jr-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("kt-app.log")
    }

    /// t354: the journal appends and does not overwrite, and rotates at the threshold,
    /// without losing the old. A failure to write the journal must not be noticeable.
    #[test]
    fn t354_journal_appends_and_rotates() {
        let p = tmp("rot");
        write_to(&p, "WARN", "first line");
        write_to(&p, "INFO", "second line");
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("first line") && body.contains("second line"), "{body}");
        assert_eq!(body.lines().count(), 2, "appending overwrote the log");
        assert!(body.starts_with("20"), "no date at the start of the line: {body}");

        // the threshold crossed — the past moves into .log.1, the new one starts from zero
        std::fs::write(&p, "x".repeat(ROTATE_BYTES as usize + 1)).unwrap();
        write_to(&p, "WARN", "after rotate");
        let fresh = std::fs::read_to_string(&p).unwrap();
        assert_eq!(fresh.lines().count(), 1, "rotation did not reset the log");
        assert!(fresh.contains("after rotate"));
        let old = std::fs::read_to_string(p.with_extension("log.1")).unwrap();
        assert_eq!(old.len(), ROTATE_BYTES as usize + 1, "old data lost on rotation");

        // an unreachable path — silently past it: the journal does not dare to bring the program down
        write_to(
            &crate::layout_tests::unwritable_path("journal").with_file_name("kt-test.log"),
            "WARN",
            "ignored",
        );
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    /// t375: the crash report names the version, the thread and the place, carries the backtrace
    /// and the dump of the “black box” — otherwise the complaint “the program closed” remains
    /// without a single clue. The symmetric guard against poisoning of the ring — t373
    /// in kt-engine: the hook reads the box exactly when someone has already panicked.
    #[test]
    fn t375_crash_report_names_version_thread_and_flight() {
        use kt_engine::flight::{Ev, FlightRecorder};
        let f = FlightRecorder::new();
        f.push(Ev::Play, 48_000, 0);
        // the dump is passed explicitly: the “black box” slot is global, and tests
        // run in parallel — a neighboring harness manages to put its own there
        let r = crash_report_with(
            "assertion failed: everything broke for the user",
            Some(&f.dump()),
            None,
        );
        assert!(r.contains(kt_version::display()), "no version: a complaint without it is useless");
        assert!(r.contains("thread:"), "no thread");
        assert!(r.contains("assertion failed"), "the panic message itself is missing");
        assert!(
            r.contains("crash_report_with"),
            "the backtrace has no names — the report is made of bare addresses"
        );
        assert!(r.contains("--- flight recorder ---"), "no black box dump");
        assert!(r.to_lowercase().contains("play"), "black box is empty: {r}");
    }

    /// t374: the previous session's report is handed out exactly ONCE and is not lost.
    #[test]
    fn t374_crash_report_is_reported_once() {
        let dir = std::env::temp_dir().join(format!("kt-t374-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let crash = dir.join("kt-app-crash.txt");
        let seen = dir.join("kt-app-crash.seen.txt");
        std::fs::write(&crash, "report").unwrap();

        // The same logic as in `take_crash_report`, on explicit paths: the data directory
        // is one per process, and tests run in parallel
        let take = |c: &Path, s: &Path| -> Option<PathBuf> {
            if !c.exists() {
                return None;
            }
            match std::fs::rename(c, s) {
                Ok(()) => Some(s.to_path_buf()),
                Err(_) => Some(c.to_path_buf()),
            }
        };
        assert_eq!(take(&crash, &seen), Some(seen.clone()), "the crash is not mentioned");
        assert_eq!(std::fs::read_to_string(&seen).unwrap(), "report", "report lost");
        assert_eq!(take(&crash, &seen), None, "one crash is reported twice");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// t1075 (render-crash-and-recovery D5): each run has its own journal with a header, the
    /// previous runs live in a fan of four, and the tail is the last run's.
    #[test]
    fn t1075_journal_is_per_run_with_a_header() {
        let p = tmp("runs");
        start_run_at(&p, "FFmpeg 7");
        write_to(&p, "INFO", "first run, line one");
        write_to(&p, "WARN", "first run, line two");
        start_run_at(&p, "FFmpeg 7");
        write_to(&p, "INFO", "second run");
        let cur = std::fs::read_to_string(&p).unwrap();
        let first = cur.lines().next().unwrap();
        assert!(first.starts_with(RUN_HEADER), "no header: {cur}");
        for part in [kt_version::FULL, std::env::consts::OS, "FFmpeg 7", &format!("pid {}", std::process::id())] {
            assert!(first.contains(part), "the header lacks {part}: {first}");
        }
        assert!(
            first.split(" · ").nth(1).is_some_and(|r| r.len() == "run 2026-11-01 18:10:12+03:00".len()),
            "the header's time has no offset: {first}"
        );
        assert_eq!(cur.lines().count(), 2, "the second run's journal holds another run: {cur}");
        assert!(cur.contains("second run") && !cur.contains("first run"), "{cur}");
        let prev = std::fs::read_to_string(nth_run(&p, 1)).unwrap();
        assert!(prev.contains("first run, line one") && prev.contains("first run, line two"), "{prev}");

        let tail = last_run_tail(&p, 10);
        assert_eq!(tail.len(), 2, "{tail:?}");
        assert!(tail[1].contains("second run"));
        // a journal of glued runs (an older build's): the tail is the last run's only
        std::fs::write(&p, format!("{prev}{cur}")).unwrap();
        let tail = last_run_tail(&p, 10);
        assert!(tail.iter().all(|l| !l.contains("first run")), "{tail:?}");

        for _ in 0..5 {
            start_run_at(&p, "FFmpeg missing");
            write_to(&p, "INFO", "later");
        }
        assert!(nth_run(&p, KEEP_RUNS).exists(), "the fan lost a run");
        assert!(!nth_run(&p, KEEP_RUNS + 1).exists(), "the fan grows without bound");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    /// t1079 (D5): the stamp is local wall-clock time followed by its offset.
    #[test]
    fn t1079_stamp_is_local_time_with_offset() {
        // 2026-10-31 18:10:12 UTC
        let t = 1_793_470_212;
        assert_eq!(stamp_with(t, 3 * 3600), "2026-10-31 21:10:12+03:00");
        assert_eq!(stamp_with(t, -(5 * 3600 + 1800)), "2026-10-31 12:40:12-05:30");
        assert_eq!(stamp_with(t, 0), "2026-10-31 18:10:12+00:00");
        // past midnight the date rolls over
        assert_eq!(stamp_with(t, 6 * 3600), "2026-11-01 00:10:12+06:00");
        // the journal's own lines carry an offset
        let p = tmp("offset");
        write_to(&p, "INFO", "x");
        let line = std::fs::read_to_string(&p).unwrap();
        let stamp = &line[..25];
        assert!(matches!(stamp.as_bytes()[19], b'+' | b'-') && &stamp[22..23] == ":", "{line}");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    /// t1078 (D4): a report the crash window showed is taken without a second announcement; an
    /// unmarked one is announced. Both are taken (moved aside, handed to telemetry).
    #[test]
    fn t1078_shown_report_is_taken_but_not_announced() {
        let dir = tmp("shown").parent().unwrap().to_path_buf();
        let (crash, seen) = (dir.join("kt-app-crash.txt"), dir.join("kt-app-crash.seen.txt"));
        std::fs::write(&crash, "kinetren x\npanic: boom\n").unwrap();
        mark_shown(&crash);
        let r = take_crash_report_at(&crash, &seen).expect("the shown report is not taken");
        assert!(r.shown, "the shown mark is not seen");
        assert_eq!(r.path, seen);
        assert!(!crash.exists(), "the report was not moved aside");
        std::fs::write(&crash, "kinetren x\npanic: boom\n").unwrap();
        let r = take_crash_report_at(&crash, &seen).unwrap();
        assert!(!r.shown, "an unshown report reads as shown");
        assert_eq!(take_crash_report_at(&crash, &seen), None, "one crash is reported twice");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The date is assembled by hand — a guard on known values.
    #[test]
    fn t354b_civil_from_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1)); // leap year
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(20_713), (2026, 9, 17));
    }

    /// t830 (kinetren-rename): the first resolve moves the former directory
    /// with everything in it; the next resolve changes nothing; with both
    /// present the new one wins and the former one is not touched.
    #[test]
    fn t830_former_data_dir_is_moved_once() {
        let base = std::env::temp_dir().join(format!("kt-t830-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let former = base.join(FORMER_DATA_DIR_NAME);
        std::fs::create_dir_all(former.join("locks")).unwrap();
        std::fs::write(former.join("fx-presets.json"), "[1]").unwrap();
        std::fs::write(former.join("locks/0123.lock"), "4242\n").unwrap();

        let dir = data_dir_in(&base);
        assert_eq!(dir, base.join(DATA_DIR_NAME));
        assert_eq!(std::fs::read_to_string(dir.join("fx-presets.json")).unwrap(), "[1]");
        assert_eq!(std::fs::read_to_string(dir.join("locks/0123.lock")).unwrap(), "4242\n");
        assert!(!former.exists(), "the former directory is moved, not duplicated");

        // a second resolve changes nothing
        std::fs::write(dir.join("workspace.json"), "{}").unwrap();
        assert_eq!(data_dir_in(&base), dir);
        assert!(dir.join("workspace.json").exists() && !former.exists());

        // both present: the new one wins, the former one is untouched
        std::fs::create_dir_all(&former).unwrap();
        std::fs::write(former.join("fx-presets.json"), "[2]").unwrap();
        assert_eq!(data_dir_in(&base), dir);
        assert_eq!(std::fs::read_to_string(dir.join("fx-presets.json")).unwrap(), "[1]");
        assert_eq!(std::fs::read_to_string(former.join("fx-presets.json")).unwrap(), "[2]");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// windows-build task 3.2: the settings file passed to eframe explicitly
    /// (`persistence_path`) is the one eframe picks by itself, so no settings move.
    #[test]
    fn eframe_settings_file_is_where_eframe_puts_it() {
        let ours = data_dir().join(kt_dirs::EFRAME_SETTINGS_FILE);
        let eframe = eframe::storage_dir("kinetren").map(|d| d.join(kt_dirs::EFRAME_SETTINGS_FILE));
        assert_eq!(Some(ours), eframe);
    }
}
