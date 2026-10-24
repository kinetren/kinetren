//! Auxiliary: sources, paths, the background index worker.

use kt_media::MediaSource;
use kt_model::MediaId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// Parsing of pseudo-paths (`synth:`, `gen:`) moved to kt-media (R.52,
// `kt_media::factory`): before, it was written here and a SECOND time in
// kt-cli, and the copies drifted apart by case (footnote to item 211).

pub type Peaks = Vec<(Vec<(i16, i16)>, Vec<(i16, i16)>)>; // [level] -> (L, R) by blocks

pub fn open_source(path: &str) -> Result<Box<dyn MediaSource>, String> {
    open_source_indexed(path, None)
}

/// Opening via the project's sidecar index DB: if the file has not changed,
/// the batch index is taken from the DB and the full scan pass is skipped
/// (fast opening of large files, test 34). The connection is short-lived —
/// WAL allows parallel readers/writers from other threads.
/// Pseudo-paths (`synth:`, `gen:`) are resolved by the kt-media factory.
pub fn open_source_indexed(
    path: &str,
    db_path: Option<&Path>,
) -> Result<Box<dyn MediaSource>, String> {
    // A pseudo-path is cut off BEFORE the sidecar DB (R.53, item 413): opening an
    // sqlite (the connection + WAL pragma + a schema version query) for a source that has no file is
    // pure waste, and this is called on every edit of generator parameters and on every placement
    // of a clip.
    if let Some(r) = kt_media::factory::open_pseudo(path) {
        return r;
    }
    let mut db = db_path.and_then(|p| kt_media::index::MediaIndexDb::open(p).ok());
    kt_media::factory::open_indexed(path, db.as_mut())
}

/// Source or a PLACEHOLDER in its place (R.33 item 10): an unavailable file must not
/// prevent opening the project, playing and rendering. It returns the error text
/// as a second value — the caller decides how to report it.
///
/// `open_source_indexed` itself is untouched: its “error = error” is needed
/// where a placeholder is harmful — adding media to the project (a typo in the path
/// would silently turn into a stub) and polling the real permissions
/// of the sources for the render menu.
pub fn open_source_or_placeholder(
    path: &str,
    db_path: Option<&Path>,
    label: &str,
) -> (Box<dyn MediaSource>, Option<String>) {
    match open_source_indexed(path, db_path) {
        Ok(s) => (s, None),
        Err(e) => (Box::new(kt_media::placeholder::PlaceholderSource::new(label)), Some(e)),
    }
}

/// File name for the placeholder caption and notifications.
pub fn media_label(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// A media path of the model → the path to open. Absolute on EITHER OS stays as it is (a
/// path of the other OS becomes a placeholder showing it, windows-build); a relative one is
/// joined to the project directory with the native separators.
pub fn resolve_media_path(base: Option<&Path>, path: &str) -> String {
    if kt_model::is_pseudo_path(path) || kt_otio::is_absolute_anywhere(path) {
        return path.to_string();
    }
    match base {
        Some(b) => kt_otio::resolve_media_path(b, path).to_string_lossy().to_string(),
        None => path.to_string(),
    }
}

/// Index service: ONE thread per project, a task queue (R.49, item 368).
///
/// Previously this was `spawn_index_worker` — a TASK pretending to be a service.
/// Each call of it started a new thread: opened the project (once), saved
/// as… (twice), added media (three times) — and all three kept writing into a shared
/// `Arc<Mutex<..>>`, and the one opened first was writing already into a table that
/// nobody reads. The difference between task and service is now carried by the type: the service is
/// an endless thread with a queue, it lives as long as the project lives, and dies on
/// `Drop` (the `WatchdogHandle` disease).
///
/// Errors are now PUBLISHED: four `let _ =` were silent about the index not
/// being built, and the user saw placeholders instead of waveforms without
/// a single word about the cause.
/// Media loudness: (LUFS, dBTP, LRA).
pub type LoudnessMap = HashMap<MediaId, (f64, f64, f64)>;

pub struct IndexService {
    tx: std::sync::mpsc::Sender<IndexTask>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    /// What went wrong (read by the UI). Not `let _ =`.
    pub errors: Arc<Mutex<Vec<String>>>,
}

struct IndexTask {
    db_path: Option<PathBuf>,
    media: Vec<(MediaId, String)>,
}

impl Drop for IndexService {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
    }
}

impl IndexService {
    pub fn start(
        out: Arc<Mutex<HashMap<MediaId, Peaks>>>,
        loud: Arc<Mutex<LoudnessMap>>,
    ) -> IndexService {
        let (tx, rx) = std::sync::mpsc::channel::<IndexTask>();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let stop_t = stop.clone();
        let err_t = errors.clone();
        std::thread::Builder::new()
            .name("index-service".into())
            .spawn(move || {
                let mut db: Option<(PathBuf, kt_media::index::MediaIndexDb)> = None;
                while !stop_t.load(std::sync::atomic::Ordering::Acquire) {
                    // waiting for a job, but waking up to check the stop flag
                    let task = match rx.recv_timeout(std::time::Duration::from_millis(200)) {
                        Ok(t) => t,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                        // the channel is closed — the owner died, we leave
                        Err(_) => break,
                    };
                    // the DB is reopened only when the path has changed: on
                    // each added media there is no point in reopening
                    if db.as_ref().map(|(p, _)| p) != task.db_path.as_ref() {
                        db = task.db_path.as_ref().and_then(|p| {
                            match kt_media::index::MediaIndexDb::open(p) {
                                Ok(d) => Some((p.clone(), d)),
                                Err(e) => {
                                    err_t.lock().unwrap().push(format!("{}: {e}", p.display()));
                                    None
                                }
                            }
                        });
                    }
                    for (id, path) in task.media {
                        if stop_t.load(std::sync::atomic::Ordering::Acquire) {
                            return;
                        }
                        if out.lock().unwrap().contains_key(&id) {
                            continue;
                        }
                        match build_peaks(&path, db.as_mut().map(|(_, d)| d), &stop_t) {
                            Ok(Some((p, l))) => {
                                out.lock().unwrap().insert(id, p);
                                if let Some(l) = l {
                                    loud.lock().unwrap().insert(id, l);
                                }
                            }
                            Ok(None) => {}
                            Err(e) => err_t.lock().unwrap().push(format!("{path}: {e}")),
                        }
                    }
                }
            })
            .expect("spawn index service");
        IndexService { tx, stop, errors }
    }

    /// Queue the media. The service is dead — not a disaster: the waveforms
    /// will appear on the next opening of the project.
    pub fn enqueue(&self, db_path: Option<PathBuf>, media: Vec<(MediaId, String)>) {
        if media.is_empty() {
            return;
        }
        let _ = self.tx.send(IndexTask { db_path, media });
    }

    /// Take the accumulated errors (the UI shows them once).
    pub fn take_errors(&self) -> Vec<String> {
        std::mem::take(&mut *self.errors.lock().unwrap())
    }
}

/// The peak pyramid of one media. A separate function — it is called by the service, and
/// by tests, and by offline analysis.
///
/// It also MEASURES LOUDNESS (R.49, item 373) and puts it into the index DB: the pass over the
/// file already runs, and a second one for LUFS on a one-hour source would take minutes.
type PeaksAndLoudness = (Peaks, Option<(f64, f64, f64)>);

fn build_peaks(
    path: &str,
    db: Option<&mut kt_media::index::MediaIndexDb>,
    stop: &Arc<std::sync::atomic::AtomicBool>,
) -> Result<Option<PeaksAndLoudness>, String> {
    let from_source = |path: &str, measure: bool| -> Result<Option<PeaksAndLoudness>, String> {
        let mut s = open_source(path).map_err(|e| e.to_string())?;
        let mut meter = kt_fx::audio::loudness::LoudnessMeter::new();
        let pyr = if measure {
            kt_media::index::PeakPyramid::build_observed(s.as_mut(), &mut |b| {
                meter.feed(b.data.as_slice())
            })
        } else {
            kt_media::index::PeakPyramid::build(s.as_mut())
        };
        let l = meter.finish();
        let loud = measure.then(|| l.lufs.map(|lufs| (lufs, l.dbtp, l.lra))).flatten();
        Ok(Some((pyramid_to_peaks(pyr), loud)))
    };
    if kt_model::is_pseudo_path(path) {
        // Pseudo-sources do NOT measure loudness: they are a generator, not
        // material — their level is exactly what the parameters say, and
        // “normalising a sine to −23” is needed by nobody. Also, we do not pay
        // for a measurement where there is no decoder at all. For `gen:` without audio
        // (duration = 0) the pyramid comes out empty — and that is correct.
        return from_source(path, false);
    }
    let Some(db) = db else {
        // A project without a path → no DB, but peaks are needed (item 3: “the audio preview
        // disappeared” for an unnamed project): we build the pyramid in memory — on
        // saving the project the service gets the job already with a DB
        return from_source(path, true);
    };
    let (mid, status) = db.ensure_media(Path::new(path)).map_err(|e| e.to_string())?;
    let complete =
        db.is_complete(mid).unwrap_or(false) && status == kt_media::index::MediaEntryStatus::Reused;
    if !complete && !stop.load(std::sync::atomic::Ordering::Acquire) {
        // a separate short-lived source — not the playback decoders;
        // the batch index is persisted as a side effect (test 34)
        let mut src = kt_media::ffmpeg::FfmpegSource::open_indexed(path, Some(db))
            .map_err(|e| e.to_string())?;
        let mut meter = kt_fx::audio::loudness::LoudnessMeter::new();
        db.build_audio_peaks_observed(mid, &mut src, &mut |b| meter.feed(b.data.as_slice()))
            .map_err(|e| e.to_string())?;
        let l = meter.finish();
        // silence — not “−70”: a number that does not exist is not written to the DB
        if let Some(lufs) = l.lufs {
            let _ = db.set_loudness(mid, lufs, l.dbtp, l.lra);
        }
        db.mark_complete(mid).map_err(|e| e.to_string())?;
    }
    let mut levels = Vec::new();
    let mut level = 0u32;
    while let Ok(Some(l)) = db.audio_peaks(mid, level, 0) {
        let r = db.audio_peaks(mid, level, 1).ok().flatten().unwrap_or_else(|| l.clone());
        levels.push((l, r));
        level += 1;
    }
    // loudness is taken from the DB: the PREVIOUS launch could have computed it too — a second
    // full decode for an already known number is needed by nobody
    let loud = db.loudness(mid).ok().flatten();
    Ok(Some((levels, loud)))
}

pub(crate) fn pyramid_to_peaks(p: kt_media::index::PeakPyramid) -> Peaks {
    p.levels
        .into_iter()
        .map(|mut chs| {
            let l = if chs.is_empty() { Vec::new() } else { chs.remove(0) };
            let r = if chs.is_empty() { l.clone() } else { chs.remove(0) };
            (l, r)
        })
        .collect()
}

/// Watchdog of media file availability (port of mediaSourceProbeTimer 900 ms):
/// the offline set is read by the UI (clip tint); it runs in its own thread.
/// The watchdog handle: Drop stops the thread (before, when the project was reopened
/// the old watchdogs lived forever — a thread leak, item 1).
pub struct WatchdogHandle {
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for WatchdogHandle {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
    }
}

pub fn spawn_media_watchdog(
    project: &kt_model::Project,
    base: Option<&Path>,
) -> (Arc<Mutex<std::collections::HashSet<MediaId>>>, WatchdogHandle) {
    let set: Arc<Mutex<std::collections::HashSet<MediaId>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));
    let paths: Vec<(MediaId, String)> = project
        .media
        .iter()
        .filter(|m| !kt_model::is_pseudo_path(&m.path))
        .map(|m| (m.id, resolve_media_path(base, &m.path)))
        .collect();
    let out = set.clone();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_t = stop.clone();
    std::thread::Builder::new()
        .name("media-watchdog".into())
        .spawn(move || {
            while !stop_t.load(std::sync::atomic::Ordering::Acquire) {
                let mut offline = std::collections::HashSet::new();
                for (id, p) in &paths {
                    if !Path::new(p).exists() {
                        offline.insert(*id);
                    }
                }
                *out.lock().unwrap() = offline;
                std::thread::sleep(std::time::Duration::from_millis(900));
            }
        })
        .expect("spawn media watchdog");
    (set, WatchdogHandle { stop })
}

/// Timecode “hh:mm:ss.mls Fkk” (item 19 round 6, test 63) — the top line.
///
/// Negative time (the zone before zero inside the reel, R.58 item 462) is written
/// with a sign and fields from |t|: “-00:00:01.000 F00”, not a set of
/// negative fields.
pub fn format_timecode(t: kt_time::Tick, fps: u32) -> String {
    let sign = if t.0 < 0 { "-" } else { "" };
    let t = kt_time::Tick(t.0.abs());
    let ms = t.as_ms();
    let fps = fps.max(1) as i64;
    let frame = t.0.rem_euclid(90_000) / (90_000 / fps);
    format!(
        "{sign}{:02}:{:02}:{:02}.{:03} F{:02}",
        ms / 3_600_000,
        ms % 3_600_000 / 60_000,
        ms % 60_000 / 1000,
        ms % 1000,
        frame
    )
}

/// Parsing the entered timecode (item 4 round 10). Tolerant: “90” → 90 s,
/// “1:30” → mm:ss, “1:02:03” → hh:mm:ss, may include a fractional part
/// (“1:02:03.250”). The result is aligned to the frame grid — otherwise
/// after a transition the frame number jumps. Junk → None.
pub fn parse_timecode(s: &str, fps: u32) -> Option<kt_time::Tick> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = s.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    // the last group may carry a fractional part of seconds
    let last = parts.pop()?;
    let (sec_str, frac_str) = match last.split_once(['.', ',']) {
        Some((a, b)) => (a, Some(b)),
        None => (last, None),
    };
    let mut secs: f64 = 0.0;
    for p in &parts {
        let v: f64 = p.trim().parse().ok()?;
        if v < 0.0 {
            return None;
        }
        secs = secs * 60.0 + v;
    }
    let tail: f64 = if sec_str.trim().is_empty() && !parts.is_empty() {
        0.0 // “1:” — minutes without seconds
    } else {
        sec_str.trim().parse().ok()?
    };
    if tail < 0.0 {
        return None;
    }
    secs = secs * 60.0 + tail;
    if let Some(f) = frac_str {
        let f = f.trim();
        if !f.is_empty() {
            let digits: f64 = f.parse().ok()?;
            secs += digits / 10f64.powi(f.len() as i32);
        }
    }
    let grid = kt_time::FpsGrid::new(fps.max(1));
    Some(grid.align_floor(kt_time::Tick::from_secs_f64(secs)))
}

/// The input mask of the timecode (item 9 round 11): digits are typed RIGHT TO LEFT
/// (the user’s decision) into the HHMMSS places, and the unentered higher places are
/// zeros. So “130” is 00:01:30, not “something from the current position”: as in
/// editing suites, where a partial timecode is always padded with zeros.
pub const TC_MASK_DIGITS: usize = 6;

/// The places of the mask (exactly 6 characters) for the current input.
pub fn mask_digits(input: &str) -> String {
    let d: Vec<char> = input.chars().filter(|c| c.is_ascii_digit()).collect();
    let d: String = d[d.len().saturating_sub(TC_MASK_DIGITS)..].iter().collect();
    format!("{}{}", "0".repeat(TC_MASK_DIGITS - d.len()), d)
}

/// A tick from the mask. Minutes/seconds are deliberately not normalised to 0..59:
/// “00:99:00” = 99 minutes, typing it is faster this way and it is no surprise.
pub fn mask_to_tick(input: &str, fps: u32) -> kt_time::Tick {
    let d = mask_digits(input);
    let n = |a: usize, b: usize| -> i64 { d[a..b].parse::<i64>().unwrap_or(0) };
    let secs = n(0, 2) * 3600 + n(2, 4) * 60 + n(4, 6);
    let step = 90_000 / fps.max(1) as i64;
    kt_time::Tick(secs * 90_000 / step * step)
}

/// The HHMMSS places from the tick — for inserting a ready timecode into the mask.
pub fn tick_to_mask(t: kt_time::Tick) -> String {
    let ms = t.as_ms().max(0);
    format!(
        "{:02}{:02}{:02}",
        (ms / 3_600_000).min(99),
        ms % 3_600_000 / 60_000,
        ms % 60_000 / 1000
    )
}

/// How the mask looks on screen: (the unentered part, the entered part).
/// The first is drawn muted — it is immediately clear what is already fixed.
pub fn mask_text(input: &str) -> (String, String) {
    let d = mask_digits(input);
    let typed = input.chars().filter(|c| c.is_ascii_digit()).count().min(TC_MASK_DIGITS);
    let full = format!("{}:{}:{}", &d[0..2], &d[2..4], &d[4..6]);
    // indices of the first typed character in “hh:mm:ss” for 0..6 digits
    let cut = [8usize, 7, 6, 4, 3, 1, 0][typed];
    (full[..cut].to_string(), full[cut..].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        static GEN: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        GEN.get_or_init(|| {
            let st = std::process::Command::new("bash")
                .arg(dir.join("gen.sh"))
                .status()
                .expect("run fixtures/gen.sh");
            assert!(st.success());
        });
        dir.join("out").join(name).to_string_lossy().to_string()
    }

    /// Test 63 (item 19 round 6): the timecode of the top line.
    #[test]
    fn t63_format_timecode() {
        use kt_time::Tick;
        assert_eq!(format_timecode(Tick(0), 30), "00:00:00.000 F00");
        assert_eq!(format_timecode(Tick(90_000 - 3000), 30), "00:00:00.966 F29");
        assert_eq!(format_timecode(Tick(59 * 90_000), 30), "00:00:59.000 F00");
        assert_eq!(format_timecode(Tick(3600 * 90_000), 30), "01:00:00.000 F00");
        assert_eq!(format_timecode(Tick(90_000 / 24 * 5), 24), "00:00:00.208 F05");
    }

    /// Test 93 (item 4 round 10): parsing an entered timecode.
    #[test]
    fn t93_parse_timecode() {
        use kt_time::{FpsGrid, Tick};
        let g = FpsGrid::new(30);
        let p = |s: &str| parse_timecode(s, 30);
        // input forms
        assert_eq!(p("90"), Some(g.align_floor(Tick(90 * 90_000))));
        assert_eq!(p("1:30"), Some(g.align_floor(Tick(90 * 90_000))));
        assert_eq!(p("1:02:03"), Some(g.align_floor(Tick(3723 * 90_000))));
        assert_eq!(p("0:00:01.5"), Some(g.align_floor(Tick(90_000 + 45_000))));
        assert_eq!(p("  2:00  "), Some(g.align_floor(Tick(120 * 90_000))));
        // the result is always on a frame boundary
        let t = p("0:00:01.999").unwrap();
        assert_eq!(g.align_floor(t), t, "not aligned to the frame grid");
        // round-trip with the formatter: the string carries milliseconds, so
        // the tolerance is one frame (grid step)
        for ticks in [0i64, 90_000, 3_723_000, 12_345_678] {
            let s = format_timecode(Tick(ticks), 30);
            let head = s.split(' ').next().unwrap();
            let back = parse_timecode(head, 30).expect("round-trip");
            assert!(
                (back.0 - g.align_floor(Tick(ticks)).0).abs() <= g.frame_step(),
                "round-trip {s}: {} vs {}",
                back.0,
                ticks
            );
        }
        // junk
        assert_eq!(p(""), None);
        assert_eq!(p("abc"), None);
        assert_eq!(p("1:2:3:4"), None);
        assert_eq!(p("-5"), None);
    }

    /// Test 399 (R.49 B2): the service is ONE thread with a queue, not a task for
    /// each call. Two jobs in a row are served by both, and the second is
    /// not lost.
    #[test]
    fn t399_index_service_serves_a_queue() {
        let out: Arc<Mutex<HashMap<MediaId, Peaks>>> = Arc::new(Mutex::new(HashMap::new()));
        let loud: Arc<Mutex<LoudnessMap>> = Arc::new(Mutex::new(Default::default()));
        let svc = IndexService::start(out.clone(), loud.clone());
        // the synthetic source needs neither FFmpeg nor files on disk
        svc.enqueue(None, vec![(MediaId(1), "synth:sine:440".into())]);
        svc.enqueue(None, vec![(MediaId(2), "synth:ramp".into())]);
        // The deadline is deliberately GENEROUS: the `synth:` source lasts an HOUR
        // (the factory’s SYNTH_DURATION), and the peak pyramid for it in a
        // debug build takes seconds, and under a parallel run of the whole
        // workspace — tens of seconds. The test asserts “the queue does not lose
        // the work”, not “the service is fast”: SPEED is judged on the release build
        // (item 301), and mixing these two claims in one expectation is
        // exactly the flake caught before on t411/t476 (R.52).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if out.lock().unwrap().len() == 2 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the service served {} of two jobs — the queue loses work",
                out.lock().unwrap().len()
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        for id in [MediaId(1), MediaId(2)] {
            let g = out.lock().unwrap();
            let p = g.get(&id).expect("pyramid");
            assert!(!p.is_empty() && !p[0].0.is_empty(), "empty pyramid for {id:?}");
        }
        assert!(svc.take_errors().is_empty(), "healthy work must produce no errors");
        // the synthetic source does not measure loudness: it is a generator, not material
        assert!(loud.lock().unwrap().is_empty(), "there is no need to measure synthetics");
    }

    /// Test 400 (R.49 B2): the service dies together with its owner, and errors
    /// are PUBLISHED. Four `let _ =` were silent about the index not
    /// being built, and the person saw placeholders without a single word about the cause.
    #[test]
    fn t400_index_service_dies_with_owner_and_reports_errors() {
        // 1) the error gets through
        let out: Arc<Mutex<HashMap<MediaId, Peaks>>> = Arc::new(Mutex::new(HashMap::new()));
        let loud: Arc<Mutex<LoudnessMap>> = Arc::new(Mutex::new(Default::default()));
        let svc = IndexService::start(out.clone(), loud.clone());
        svc.enqueue(None, vec![(MediaId(9), "/proc/self/no-such-file.wav".into())]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut errs = Vec::new();
        while errs.is_empty() && std::time::Instant::now() < deadline {
            errs = svc.take_errors();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!errs.is_empty(), "the index failure is not mentioned at all");
        assert!(errs[0].contains("no-such-file"), "no address in the message: {errs:?}");
        assert!(svc.take_errors().is_empty(), "errors are reported once");

        // 2) Drop stops the thread: the next job is not served
        let out2: Arc<Mutex<HashMap<MediaId, Peaks>>> = Arc::new(Mutex::new(HashMap::new()));
        let tx = {
            let svc = IndexService::start(out2.clone(), Arc::new(Mutex::new(Default::default())));
            svc.tx.clone()
        }; // here Drop
        std::thread::sleep(std::time::Duration::from_millis(400));
        let _ =
            tx.send(IndexTask { db_path: None, media: vec![(MediaId(3), "synth:ramp".into())] });
        std::thread::sleep(std::time::Duration::from_millis(600));
        assert!(
            out2.lock().unwrap().is_empty(),
            "the service survived its owner and keeps working — exactly the thread leak \
             for which the service was created"
        );
    }

    /// Test 411 (R.49 B7): the loudness of the REAL material is computed in the same
    /// pass as the pyramid and goes into the index DB — the second launch takes
    /// it from there, without decoding the file again.
    #[test]
    fn t411_source_loudness_is_measured_on_the_pyramid_pass() {
        let dir = std::env::temp_dir().join(format!("kt-t411-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("p.ktindex.db");
        let file = fixture("sine48k_stereo.flac");

        let peaks: Arc<Mutex<HashMap<MediaId, Peaks>>> = Arc::new(Mutex::new(HashMap::new()));
        let loud: Arc<Mutex<LoudnessMap>> = Arc::new(Mutex::new(Default::default()));
        let svc = IndexService::start(peaks.clone(), loud.clone());
        svc.enqueue(Some(db_path.clone()), vec![(MediaId(5), file.clone())]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while loud.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let first = *loud.lock().unwrap().get(&MediaId(5)).expect("loudness not computed");
        assert!(first.0 < 0.0 && first.0 > -60.0, "loudness is suspicious: {first:?}");
        assert!(first.1 <= 0.5, "true peak above zero: {}", first.1);
        assert!(svc.take_errors().is_empty());
        drop(svc);

        // second launch: the number is taken from the DB, not computed again
        let peaks2: Arc<Mutex<HashMap<MediaId, Peaks>>> = Arc::new(Mutex::new(HashMap::new()));
        let loud2: Arc<Mutex<LoudnessMap>> = Arc::new(Mutex::new(Default::default()));
        let svc2 = IndexService::start(peaks2, loud2.clone());
        svc2.enqueue(Some(db_path), vec![(MediaId(5), file)]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while loud2.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let second = *loud2.lock().unwrap().get(&MediaId(5)).expect("loudness not read from the DB");
        assert_eq!(first, second, "the round trip through the DB changed the number");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Test 23 (item 3): peaks are built even WITHOUT an index DB (an unnamed project) —
    /// before, db_path=None meant “there will be no peaks at all”.
    #[test]
    fn t23_peaks_build_without_db() {
        let out: Arc<Mutex<HashMap<MediaId, Peaks>>> = Arc::new(Mutex::new(HashMap::new()));
        let loud: Arc<Mutex<LoudnessMap>> = Arc::new(Mutex::new(Default::default()));
        let svc = IndexService::start(out.clone(), loud.clone());
        svc.enqueue(None, vec![(MediaId(7), fixture("sine48k_stereo.flac"))]); // unnamed project
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(p) = out.lock().unwrap().get(&MediaId(7)) {
                assert!(!p.is_empty(), "the pyramid must have levels");
                let l0 = &p[0].0;
                assert!(!l0.is_empty(), "level 0 is not empty");
                assert!(
                    l0.iter().any(|(lo, hi)| *lo != 0 || *hi != 0),
                    "sine peaks must not be zeros"
                );
                break;
            }
            assert!(std::time::Instant::now() < deadline, "peaks did not build within 15 s");
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }

    /// Test 23 (item 3): thumbnails for media added AFTER the service started
    /// (before, the media map was frozen at start).
    #[test]
    fn t23_thumbs_for_media_added_after_start() {
        let project = kt_model::Project::new(kt_model::ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        });
        let svc = crate::thumbs::ThumbService::start(&project, None);
        svc.add_media(MediaId(3), fixture("testsrc_h264.mp4"));
        let key = crate::thumbs::ThumbKey { media: MediaId(3), t: 0 };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            svc.request(key);
            if svc.get(&key).is_some() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the thumbnail of the added media did not build within 15 s"
            );
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }
}
