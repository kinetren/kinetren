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
//! Format — one line per record: `YYYY-MM-DD HH:MM:SS level text`. Rotation
//! at 1 MiB into `kt-app.log.1` (one step, no fan of numbers): the journal is needed for
//! analysis of the last session, not for history.

use std::io::Write;
use std::path::{Path, PathBuf};

/// The rotation threshold. A megabyte is about ten thousand lines: more than the
/// program produces in a day of work, and less than is noticeable on disk.
pub const ROTATE_BYTES: u64 = 1024 * 1024;

/// Name of the per-user data directory under `$XDG_DATA_HOME`.
pub const DATA_DIR_NAME: &str = "kinetren";
/// The directory of the program's former name (before 2026-10-21). It is
/// moved into place once (`data_dir_in`); nothing reads it afterwards.
pub const FORMER_DATA_DIR_NAME: &str = "kinotrain";

/// The program's data directory (`$XDG_DATA_HOME/kinetren`). Shared with the
/// autosave — there is no need for a separate home for the journal. The single resolver:
/// recovery, journal, locks, presets, workspace and eframe's own storage
/// (its app id is the same name) all live here.
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    data_dir_in(&base)
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

/// The body of the report. Taken out of the hook so that it can be checked without
/// replacing the global hook in a test (the replacement would affect neighboring tests).
pub(crate) fn crash_report(panic_text: &str) -> String {
    let dump = flight_slot().lock().ok().and_then(|s| s.as_ref().map(|f| f.dump()));
    crash_report_with(panic_text, dump.as_deref())
}

pub(crate) fn crash_report_with(panic_text: &str, flight: Option<&str>) -> String {
    let mut out = String::with_capacity(8 * 1024);
    out.push_str(&format!("kinetren {}\n", kt_version::display()));
    out.push_str(&format!("time: {}\n", stamp()));
    let t = std::thread::current();
    out.push_str(&format!("thread: {}\n", t.name().unwrap_or("<unnamed>")));
    out.push_str(&format!("panic: {panic_text}\n\n"));
    out.push_str(&format!("{}\n", std::backtrace::Backtrace::force_capture()));
    // dump of the “black box”: what the program was doing in the last seconds
    if let Some(f) = flight {
        out.push_str("\n--- flight recorder ---\n");
        out.push_str(f);
    }
    out
}

/// If the previous session left a report — return its path ONCE and
/// set the report itself aside.
pub fn take_crash_report() -> Option<PathBuf> {
    let p = crash_path();
    if !p.exists() {
        return None;
    }
    let seen = crash_seen_path();
    match std::fs::rename(&p, &seen) {
        Ok(()) => Some(seen),
        // renaming did not work — still must say so, the report is in place
        Err(_) => Some(p),
    }
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
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
}

/// Date and time without external dependencies: the program does not pull in chrono for the
/// sake of one line. The algorithm is civil_from_days by Howard Hinnant.
fn stamp() -> String {
    stamp_of(std::time::SystemTime::now())
}

/// The same label for an arbitrary moment — the recovery dialog uses it to show the time
/// of each candidate (item 353).
pub fn stamp_of(t: std::time::SystemTime) -> String {
    let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
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
        write_to(Path::new("/proc/self/no-such-dir/kt.log"), "WARN", "ignored");
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
        let r =
            crash_report_with("assertion failed: everything broke for the user", Some(&f.dump()));
        assert!(r.contains(kt_version::display()), "no version: a complaint without it is useless");
        assert!(r.contains("thread:"), "no thread");
        assert!(r.contains("assertion failed"), "the panic message itself is missing");
        assert!(r.contains("crash_report_with"), "the backtrace has no names — the report is made of bare addresses");
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
}
