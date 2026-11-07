//! The telemetry journal (telemetry-journal, kkt-26.11.07b G6/G10): ONE local file of what the
//! program did and what the person did, each record stamped at the moment it happened.
//!
//! The owner: "агент который делал лог - не думал и сделал отложенную запись и я не могу тебе
//! сказать точное время", and "телеметрия != трекинг использования в posthog". Nothing here is
//! sent anywhere, and `telemetry.rs` never reads it.
//!
//! The rules, each with its test:
//! - **The caller stamps** (t1960). `record` takes the clock at the call — the local wall-clock time
//!   with milliseconds and its offset, and the seconds since the process started — and the line is
//!   formatted there. Writing it later changes nothing in it.
//! - **One writer, blocked while nothing comes** (t1964). A thread waits in `recv`: no timer, no
//!   poll, so at rest the journal wakes nobody. The flight sink calls `record` from the audio render
//!   thread, which must not wait on a disk (invariant 6), and the UI must not either.
//! - **Written means kept** (t1962). The writer appends with one `write_all` to a file opened for
//!   append, with no buffer of ours: once written, a SIGKILL loses nothing. A panic sends a flush
//!   barrier (`flush`).
//! - **Bounded** (t1963). Before a write that would pass `ROTATE_BYTES` the file moves to `.1`
//!   (`.1` to `.2`).
//!
//! The journal is NOT per run (unlike `kt-app.log`): a RUN line starts each run, so a crash and the
//! next start read in order. A short in-memory tail serves Diagnostics and the load report.

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};
// the type of the telemetry's one door, which the journal hears too (t887 counts the doors)
use crate::telemetry::Signal::{self};

pub const FILE: &str = "telemetry-journal.log";
/// Two megabytes is about eight hours of busy seconds; three files bound the journal at six.
pub const ROTATE_BYTES: u64 = 2 * 1024 * 1024;
/// Previous files kept next to the journal (`.1`, `.2`).
pub const KEEP_OLD: usize = 2;
/// The in-memory tail: what the load report exports and Diagnostics shows.
pub const TAIL_SPAN_MS: i64 = 15 * 60 * 1000;
const TAIL_MAX: usize = 4000;

/// What a record is about: the column after the time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A run started (version, OS, pid).
    Run,
    /// A busy second of the load watch, or the return to rest.
    Load,
    /// The playback black box and the summary of a playback.
    Play,
    /// A problem the program noticed (underruns, evictions, offline media).
    Note,
    /// What the person did: an edit, a window, a selection, a save.
    Act,
    /// The person's own mark of the moment.
    Mark,
    /// A line of `kt-app.log` or a notice.
    Log,
    /// A line of the render log.
    Export,
}

impl Kind {
    pub fn tag(self) -> &'static str {
        match self {
            Kind::Run => "RUN",
            Kind::Load => "LOAD",
            Kind::Play => "PLAY",
            Kind::Note => "NOTE",
            Kind::Act => "ACT",
            Kind::Mark => "MARK",
            Kind::Log => "LOG",
            Kind::Export => "EXPORT",
        }
    }
}

/// A moment: Unix milliseconds and milliseconds since the process started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Stamp {
    pub unix_ms: i64,
    pub mono_ms: u64,
}

impl Stamp {
    /// The same moment `ms` earlier (a run of repeats closed now, stamped at its last repeat).
    pub fn earlier(self, ms: u64) -> Stamp {
        Stamp { unix_ms: self.unix_ms - ms as i64, mono_ms: self.mono_ms.saturating_sub(ms) }
    }
}

/// Where time comes from: the system clock in the program, a hand-moved one in tests.
pub trait Clock: Send + Sync {
    fn now(&self) -> Stamp;
    /// Seconds east of UTC at that moment.
    fn offset(&self, unix_secs: i64) -> i64 {
        crate::journal::local_offset(unix_secs)
    }
}

struct SystemClock(Instant);

impl Clock for SystemClock {
    fn now(&self) -> Stamp {
        let unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        Stamp { unix_ms, mono_ms: self.0.elapsed().as_millis() as u64 }
    }
}

/// One record as Diagnostics and the report read it.
#[derive(Clone, Debug)]
pub struct Rec {
    pub at: Stamp,
    pub kind: Kind,
    /// The whole line as written, without the line break.
    pub line: String,
}

enum Msg {
    Line(String),
    Flush(mpsc::Sender<()>),
}

pub struct Journal {
    clock: Arc<dyn Clock>,
    #[cfg_attr(not(test), allow(dead_code))]
    path: PathBuf,
    /// The channel and the tail under one lock: the file and the tail keep one order.
    state: Mutex<State>,
    /// Records made / records the writer has put into the file.
    made: AtomicU64,
    #[cfg_attr(not(test), allow(dead_code))]
    written: Arc<AtomicU64>,
    /// The writer thread's kernel id (Linux), for the at-rest test.
    #[cfg_attr(not(test), allow(dead_code))]
    writer_tid: Arc<AtomicU64>,
    /// The gesture last journaled + 1: a drag is one record, not one per frame.
    last_gesture: AtomicU64,
}

struct State {
    tx: Option<mpsc::Sender<Msg>>,
    tail: VecDeque<Rec>,
}

impl Journal {
    /// A journal at `path`, rotating past `rotate` bytes. The writer thread starts here.
    pub fn open(path: PathBuf, clock: Arc<dyn Clock>, rotate: u64) -> Arc<Journal> {
        let (tx, rx) = mpsc::channel::<Msg>();
        let written = Arc::new(AtomicU64::new(0));
        let writer_tid = Arc::new(AtomicU64::new(0));
        let (w, t, p) = (written.clone(), writer_tid.clone(), path.clone());
        let spawned = std::thread::Builder::new()
            .name("tele-journal".into())
            .spawn(move || writer(&p, rotate, rx, &w, &t));
        Arc::new(Journal {
            clock,
            path,
            state: Mutex::new(State { tx: spawned.is_ok().then_some(tx), tail: VecDeque::new() }),
            made: AtomicU64::new(0),
            written,
            writer_tid,
            last_gesture: AtomicU64::new(0),
        })
    }

    #[cfg(test)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn now(&self) -> Stamp {
        self.clock.now()
    }

    /// Record an event that happens now.
    pub fn record(&self, kind: Kind, text: &str) {
        self.record_at(self.clock.now(), kind, text);
    }

    /// Record an event that happened at `at`.
    pub fn record_at(&self, at: Stamp, kind: Kind, text: &str) {
        let text = file_names_only(text);
        let line = format_line(at, kind, &text, self.clock.offset(at.unix_ms.div_euclid(1000)));
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(tx) = &st.tx {
            let _ = tx.send(Msg::Line(format!("{line}\n")));
        }
        st.tail.push_back(Rec { at, kind, line });
        while st.tail.len() > TAIL_MAX
            || st.tail.front().is_some_and(|f| at.unix_ms - f.at.unix_ms > TAIL_SPAN_MS)
        {
            st.tail.pop_front();
        }
        self.made.fetch_add(1, Ordering::Release);
    }

    /// How many records were made: Diagnostics refreshes its copy of the tail when it moves.
    pub fn generation(&self) -> u64 {
        self.made.load(Ordering::Acquire)
    }

    /// How many records the writer has put into the file.
    #[cfg(test)]
    pub fn written(&self) -> u64 {
        self.written.load(Ordering::Acquire)
    }

    /// The last `n` records, oldest first.
    pub fn tail(&self, n: usize) -> Vec<Rec> {
        let st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        st.tail.iter().skip(st.tail.len().saturating_sub(n)).cloned().collect()
    }

    /// The records from `from_unix_ms` on, oldest first (the load report).
    pub fn since(&self, from_unix_ms: i64) -> Vec<Rec> {
        let st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        st.tail.iter().filter(|r| r.at.unix_ms >= from_unix_ms).cloned().collect()
    }

    /// Wait until the writer has written everything recorded before this call, at most `limit`.
    pub fn flush(&self, limit: Duration) -> bool {
        let (tx, rx) = mpsc::channel();
        {
            let st = self.state.lock().unwrap_or_else(|p| p.into_inner());
            match &st.tx {
                Some(s) if s.send(Msg::Flush(tx)).is_ok() => {}
                _ => return false,
            }
        }
        rx.recv_timeout(limit).is_ok()
    }

    #[cfg(test)]
    pub(crate) fn writer_tid(&self) -> u64 {
        self.writer_tid.load(Ordering::Acquire)
    }
}

/// A path in a message becomes its file name: the journal holds no path beyond a file name (a
/// notice says "Saved: /home/…/haski.kt"). A word holding a `/` or a `\` keeps what follows the
/// last one; a path with spaces keeps its last word of each piece, never a directory.
pub(crate) fn file_names_only(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains(['/', '\\']) {
        return std::borrow::Cow::Borrowed(text);
    }
    let words: Vec<&str> = text
        .split(' ')
        .map(|w| match w.rfind(['/', '\\']) {
            // a URL-ish or a fraction like "1/2" stays readable: only a piece with a letter goes
            Some(i)
                if i + 1 < w.len()
                    && (w[..i].chars().any(|c| c.is_alphabetic()) || w.starts_with(['/', '\\', '~'])) =>
            {
                &w[i + 1..]
            }
            _ => w,
        })
        .collect();
    std::borrow::Cow::Owned(words.join(" "))
}

/// `2026-11-07 14:03:12.345+03:00  +123.456  ACT    text`
pub(crate) fn format_line(at: Stamp, kind: Kind, text: &str, offset_secs: i64) -> String {
    let secs = at.unix_ms.div_euclid(1000);
    let ms = at.unix_ms.rem_euclid(1000);
    let s = crate::journal::stamp_with(secs, offset_secs);
    format!(
        "{}.{ms:03}{}  +{}.{:03}  {:<6} {}",
        &s[..19],
        &s[19..],
        at.mono_ms / 1000,
        at.mono_ms % 1000,
        kind.tag(),
        text.replace(['\n', '\r'], " | ")
    )
}

/// The writer: blocked in `recv` until a record comes, then everything queued in one write.
fn writer(path: &Path, rotate: u64, rx: mpsc::Receiver<Msg>, written: &AtomicU64, tid: &AtomicU64) {
    #[cfg(target_os = "linux")]
    tid.store(unsafe { libc::gettid() } as u64, Ordering::Release);
    #[cfg(not(target_os = "linux"))]
    let _ = tid;
    let mut file: Option<std::fs::File> = None;
    let mut len = 0u64;
    while let Ok(first) = rx.recv() {
        let (mut buf, mut lines, mut barriers) = (String::new(), 0u64, Vec::new());
        for m in std::iter::once(first).chain(rx.try_iter()) {
            match m {
                Msg::Line(l) => {
                    buf.push_str(&l);
                    lines += 1;
                }
                Msg::Flush(b) => barriers.push(b),
            }
        }
        if lines > 0 {
            if file.is_none() {
                if let Some(d) = path.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                len = std::fs::metadata(path).map_or(0, |m| m.len());
            }
            if len > 0 && len + buf.len() as u64 > rotate {
                file = None;
                shift(path);
                len = 0;
            }
            if file.is_none() {
                file = std::fs::OpenOptions::new().create(true).append(true).open(path).ok();
            }
            if let Some(f) = &mut file
                && f.write_all(buf.as_bytes()).is_ok()
            {
                len += buf.len() as u64;
            }
            written.fetch_add(lines, Ordering::Release);
        }
        for b in barriers {
            let _ = b.send(());
        }
    }
}

/// `.1 → .2`, the journal → `.1`; the oldest falls off.
fn shift(path: &Path) {
    let nth = |i: usize| crate::journal::nth_run(path, i);
    for i in (1..KEEP_OLD).rev() {
        if nth(i).exists() {
            let _ = kt_otio::replace_file(&nth(i), &nth(i + 1));
        }
    }
    let _ = kt_otio::replace_file(path, &nth(1));
}

// ---- the process's journal ---------------------------------------------------------------------

static GLOBAL: OnceLock<Arc<Journal>> = OnceLock::new();

thread_local! {
    static BOUND: std::cell::RefCell<Option<Arc<Journal>>> = const { std::cell::RefCell::new(None) };
}

pub fn path() -> PathBuf {
    crate::journal::data_dir().join(FILE)
}

/// Open the program's journal and write the run's first line: once per process, before the
/// first window.
pub fn install() {
    let j = GLOBAL.get_or_init(|| {
        Journal::open(path(), Arc::new(SystemClock(Instant::now())), ROTATE_BYTES)
    });
    j.record(
        Kind::Run,
        &format!(
            "kinetren {} {} {} pid {}",
            kt_version::FULL,
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::process::id()
        ),
    );
}

/// The journal this thread writes to: a test's bound one, or the program's.
pub fn current() -> Option<Arc<Journal>> {
    BOUND.with(|b| b.borrow().clone()).or_else(|| GLOBAL.get().cloned())
}

/// The program's journal from any thread (the load watch, the flight sink): never a test's.
pub fn global() -> Option<Arc<Journal>> {
    GLOBAL.get().cloned()
}

/// Record an event that happens now. A no-op without a journal (kt-cli, most tests).
pub fn record(kind: Kind, text: &str) {
    if let Some(j) = current() {
        j.record(kind, text);
    }
}

/// Wait at most `limit` for the queued records (the panic hook, `on_exit`).
pub fn flush(limit: Duration) {
    if let Some(j) = current() {
        j.flush(limit);
    }
}

/// What the person did, from the door every edit already passes (`telemetry::signal`), whatever
/// the usage telemetry's consent: the journal is local. One record per batch, one per gesture.
pub fn on_signal(s: &Signal<'_>) {
    let Some(j) = current() else { return };
    match s {
        Signal::Commands(cmds, _) => {
            j.last_gesture.store(0, Ordering::Relaxed);
            j.record(Kind::Act, &format!("edit {}", commands_text(cmds)));
        }
        Signal::Command(cmd, _) => {
            j.last_gesture.store(0, Ordering::Relaxed);
            j.record(Kind::Act, &format!("edit {}", commands_text(std::slice::from_ref(*cmd))));
        }
        Signal::Gesture(cmd, g, _) => {
            if j.last_gesture.swap(g.wrapping_add(1), Ordering::Relaxed) != g.wrapping_add(1) {
                j.record(Kind::Act, &format!("edit {} (a live gesture)", commands_text(std::slice::from_ref(*cmd))));
            }
        }
        Signal::Undo => j.record(Kind::Act, "undo"),
        Signal::Redo => j.record(Kind::Act, "redo"),
        Signal::Gpu(info, software) => j.record(
            Kind::Log,
            &format!("GPU: {} / {:?} / {:?}{}", info.name, info.device_type, info.backend, if *software { ", software" } else { "" }),
        ),
        Signal::CrashReport(p) => j.record(
            Kind::Log,
            &format!("the previous run crashed: {}", p.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())),
        ),
        Signal::Exit => j.record(Kind::Act, "the program closes"),
        // named by the app's own call with the file name (`project_act`); the frame is no action
        Signal::Saved | Signal::ProjectOpened(..) | Signal::ExportEnded(_) | Signal::Frame { .. } => {}
    }
}

/// `SetFx event 7`, `MoveClip ×3, AddTransition`: the command names in order, a run counted.
pub(crate) fn commands_text(cmds: &[kt_model::Command]) -> String {
    let mut parts: Vec<(String, usize)> = Vec::new();
    for c in cmds {
        let name = variant_name(c);
        match parts.last_mut() {
            Some((n, k)) if *n == name => *k += 1,
            _ => parts.push((name, 1)),
        }
    }
    let mut out: Vec<String> = parts
        .into_iter()
        .map(|(n, k)| if k > 1 { format!("{n} ×{k}") } else { n })
        .collect();
    if let [one] = cmds
        && let Some(id) = one.target_clip()
    {
        out[0].push_str(&format!(" event {}", id.0));
    }
    if out.is_empty() { "(nothing)".into() } else { out.join(", ") }
}

/// A command's variant name: the first word its `Debug` writes, cut there (a `SetFx` carries
/// whole chains, and formatting them for a name would cost a copy of the chain).
fn variant_name(c: &impl std::fmt::Debug) -> String {
    struct First(String);
    impl std::fmt::Write for First {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.0.push_str(s);
            Err(std::fmt::Error)
        }
    }
    let mut w = First(String::new());
    let _ = std::fmt::write(&mut w, format_args!("{c:?}"));
    w.0.split(|ch: char| !ch.is_alphanumeric() && ch != '_').next().unwrap_or("").to_string()
}

/// A project opened or saved, by its file name only.
pub fn project_act(what: &str, path: Option<&Path>) {
    let name = path.and_then(|p| p.file_name()).map_or("(unnamed)".to_string(), |n| n.to_string_lossy().into_owned());
    record(Kind::Act, &format!("{what}: {name}"));
}

/// The playback black box's door (`FlightRecorder::set_sink`): each record as it is pushed, a run
/// of repeats once, stamped at its last repeat. Bound to the journal current where the player is
/// built; called from the render thread, so it only formats and sends.
pub fn flight_sink() -> Option<kt_engine::flight::FlightSink> {
    use kt_engine::flight::{Ev, Rec, Sunk, describe};
    let j = current()?;
    let text = |r: &Rec| match r.kind {
        Ev::Snap => format!(
            "underruns: render {} / callback {} frames, warm-up misses {}, head {:.3}",
            r.snap.render_underruns,
            r.snap.callback_underruns,
            r.snap.warmup_misses,
            r.snap.head_frames as f64 / kt_time::AUDIO_SAMPLE_RATE as f64
        ),
        _ => describe(r).trim_end().to_string(),
    };
    Some(Box::new(move |s| match s {
        Sunk::New(r) => j.record(Kind::Play, &text(&r)),
        Sunk::Repeats { rec, now_ms } => {
            let at = j.now().earlier(now_ms.saturating_sub(rec.at_ms));
            j.record_at(at, Kind::Play, &format!("{} ×{} (the last repeat)", text(&rec), rec.n));
        }
    }))
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// A clock moved by hand, in a fixed zone.
    pub struct FakeClock {
        pub at: Mutex<Stamp>,
        pub offset: i64,
    }

    impl FakeClock {
        pub fn new(unix_ms: i64, offset: i64) -> Arc<FakeClock> {
            Arc::new(FakeClock { at: Mutex::new(Stamp { unix_ms, mono_ms: 0 }), offset })
        }

        pub fn advance(&self, ms: u64) {
            let mut a = self.at.lock().unwrap();
            a.unix_ms += ms as i64;
            a.mono_ms += ms;
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Stamp {
            *self.at.lock().unwrap()
        }
        fn offset(&self, _unix_secs: i64) -> i64 {
            self.offset
        }
    }

    /// Bind `j` for this test thread until the guard drops.
    pub fn bind(j: Arc<Journal>) -> Bound {
        BOUND.with(|b| *b.borrow_mut() = Some(j));
        Bound
    }

    pub struct Bound;

    impl Drop for Bound {
        fn drop(&mut self) {
            BOUND.with(|b| *b.borrow_mut() = None);
        }
    }

    /// A journal in a fresh folder of the test's data directory.
    pub fn fresh(name: &str, clock: Arc<dyn Clock>, rotate: u64) -> Arc<Journal> {
        let dir = crate::journal::data_dir().join(format!("tj-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        Journal::open(dir.join(FILE), clock, rotate)
    }

    /// Wait until the writer has written `n` records.
    pub fn wait_written(j: &Journal, n: u64) {
        let deadline = kt_time::test_deadline();
        while j.written() < n {
            assert!(Instant::now() < deadline, "the journal's writer never wrote {n} records");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn read(j: &Journal) -> String {
        std::fs::read_to_string(j.path()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    /// 2025-11-07 07:00:00.250 UTC: 10:00:00.250 at +03:00.
    const T0: i64 = 1_762_498_800_250;

    /// Test t1960 (telemetry-journal): a record carries the time of its event, not of its
    /// writing or reading. The owner's report said "487 s ago" from the moment it was written.
    #[test]
    fn t1960_records_carry_the_event_time() {
        let clock = FakeClock::new(T0, 3 * 3600);
        let j = fresh("t1960", clock.clone(), ROTATE_BYTES);
        clock.advance(1_500);
        j.record(Kind::Act, "edit SetFx event 7");
        // the writer and the reader come an hour later: nothing in the line may move
        clock.advance(3_600_000);
        wait_written(&j, 1);
        let text = read(&j);
        let line = text.lines().next().expect("a line");
        assert!(line.starts_with("2025-11-07 10:00:01.750+03:00  +1.500  ACT"), "{line}");
        assert!(line.ends_with("edit SetFx event 7"), "{line}");
        assert_eq!(j.tail(5)[0].line, line, "the tail and the file say the same");
        // a moment handed in by the caller (a run of repeats closed later) keeps its own time
        let at = clock.now().earlier(3_600_000);
        j.record_at(at, Kind::Play, "WRAP ×100");
        wait_written(&j, 2);
        assert!(read(&j).lines().nth(1).unwrap().starts_with("2025-11-07 10:00:01.750+03:00"));
        // a path keeps its file name; a lone slash between words stays
        assert_eq!(file_names_only("Saved: /home/e/My Videos/haski.kt"), "Saved: My haski.kt");
        assert_eq!(file_names_only("render 0 / callback 2"), "render 0 / callback 2");
        assert_eq!(file_names_only("C:\\Users\\e\\a.kt ok"), "a.kt ok");
        // a line break of a message cannot split a record
        j.record(Kind::Log, "two\nlines");
        wait_written(&j, 3);
        assert_eq!(read(&j).lines().count(), 3);
    }

    /// Test t1962 (telemetry-journal): a killed process keeps every line it recorded — no buffer
    /// of ours sits between a record and the kernel. The child is this test binary.
    #[cfg(target_os = "linux")]
    #[test]
    fn t1962_journal_survives_a_kill() {
        use std::os::unix::process::ExitStatusExt;
        let dir = crate::journal::data_dir().join("tj-t1962");
        let _ = std::fs::remove_dir_all(&dir);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tele_journal::tests::t1962_child_records_and_dies", "--ignored", "--test-threads=1"])
            .env("KT_T1962_DIR", &dir)
            .env("KINETREN_DATA_DIR", &dir)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(libc::SIGKILL), "the child must die by SIGKILL: {status:?}");
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(text.ends_with('\n'), "a line was cut");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 50, "{text}");
        for (i, l) in lines.iter().enumerate() {
            assert!(l.ends_with(&format!("record {i}")), "{l}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "the child of t1962"]
    fn t1962_child_records_and_dies() {
        let Some(dir) = std::env::var_os("KT_T1962_DIR") else { return };
        let j = Journal::open(PathBuf::from(dir).join(FILE), Arc::new(SystemClock(Instant::now())), ROTATE_BYTES);
        for i in 0..50 {
            j.record(Kind::Act, &format!("record {i}"));
        }
        // written by the writer — not flushed, not closed, not dropped
        wait_written(&j, 50);
        unsafe { libc::kill(libc::getpid(), libc::SIGKILL) };
    }

    /// Test t1963 (telemetry-journal): rotation bounds the journal at three files.
    #[test]
    fn t1963_rotation_bounds_the_journal() {
        let clock = FakeClock::new(T0, 0);
        let j = fresh("t1963", clock.clone(), 1_000);
        let n = 120;
        for i in 0..n {
            j.record(Kind::Load, &format!("second {i:04} kinetren 100, prefetch 90"));
            clock.advance(1_000);
            // one write per record, so every rotation boundary is reached
            wait_written(&j, i + 1);
        }
        let line = read(&j).lines().next().map_or(0, |l| l.len() + 1) as u64;
        for i in 0..=KEEP_OLD {
            let p = if i == 0 { j.path().to_path_buf() } else { crate::journal::nth_run(j.path(), i) };
            let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or_else(|_| panic!("{} is missing", p.display()));
            assert!(len <= 1_000 + line, "{} holds {len} bytes", p.display());
        }
        assert!(!crate::journal::nth_run(j.path(), KEEP_OLD + 1).exists(), "a fourth file");
        assert!(read(&j).trim_end().ends_with(&format!("second {:04} kinetren 100, prefetch 90", n - 1)));
    }

    /// Test t1964 (telemetry-journal): with nothing to write, the journal's thread does not wake.
    #[cfg(target_os = "linux")]
    #[test]
    fn t1964_no_wakeups_at_rest() {
        let j = fresh("t1964", FakeClock::new(T0, 0), ROTATE_BYTES);
        j.record(Kind::Run, "start");
        wait_written(&j, 1);
        let tid = j.writer_tid();
        assert!(tid > 0);
        let switches = || {
            let s = std::fs::read_to_string(format!("/proc/self/task/{tid}/status")).unwrap();
            s.lines()
                .filter(|l| l.contains("ctxt_switches"))
                .filter_map(|l| l.split_whitespace().last()?.parse::<u64>().ok())
                .sum::<u64>()
        };
        let before = switches();
        std::thread::sleep(Duration::from_millis(1_500));
        assert_eq!(switches(), before, "the journal's writer woke with nothing to write");
        // and a record still wakes it at once
        j.record(Kind::Mark, "here");
        wait_written(&j, 2);
    }
}
