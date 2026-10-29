//! Where the program looks for FFmpeg, as far as only the program knows it (change
//! windows-build, capability media-index): the folder the person chose with *Locate
//! folder…* lives in `<data_dir>/ffmpeg-dir` — one line, so kt-cli reads the same choice.
//!
//! The search itself is `kt_ffmpeg::search`; this module only hands it the program's own
//! folders before the first media open.

use std::path::PathBuf;

/// The file holding the person's choice.
pub fn user_choice_file() -> PathBuf {
    crate::journal::data_dir().join("ffmpeg-dir")
}

/// The chosen folder, if one is saved.
pub fn read_user_choice() -> Option<String> {
    let text = std::fs::read_to_string(user_choice_file()).ok()?;
    let line = text.lines().next()?.trim();
    (!line.is_empty()).then(|| line.to_string())
}

/// Give the search the program's folders. Called at startup, before any source opens,
/// and again after Locate… or a download.
pub fn install_search_dirs() {
    kt_ffmpeg::set_search_dirs(read_user_choice(), download_root_for_search(&download_root()));
}

/// The download root is a step of the Windows search only (Linux has system packages).
/// The ROOT, not the pin's folder (ffmpeg-9): a folder an earlier pin left keeps working.
pub fn download_root_for_search(root: &std::path::Path) -> Option<String> {
    cfg!(windows).then(|| root.to_string_lossy().into_owned())
}

// ---- the download of the pinned build (Windows; media-index) ------------------------------
//
// One PINNED archive, checked against a hash committed HERE (not the `.sha256` next to it on
// the same server — that would prove nothing). The "latest release" link is not used: it
// moves to the next major on its own, and a button that downloads an unsupported FFmpeg is
// worse than none (on 2026-10-22 it was already FFmpeg 9 while the loader knew 6/7/8). The
// GitHub mirror serves a byte-identical file (9.0.2 checked 2026-10-27), so one hash covers
// both URLs. `src/scripts/fetch-ffmpeg-win.sh` and the windows-latest CI job use the same pin
// — dist-check.py (t860) keeps the three equal.

/// The FFmpeg release the Download button installs.
pub const PINNED_VERSION: &str = kt_dirs::FFMPEG_DOWNLOAD_VERSION;
/// The archive's name as published — the cached copy the extraction test compares against.
#[cfg_attr(not(test), allow(dead_code))]
pub const PINNED_ARCHIVE: &str = "ffmpeg-9.0.2-full_build-shared.7z";
pub const PINNED_SHA256: &str = "4d2060a8b34a940aa47d785142055bb92a63053781e55f2ace4546edd519a8f5";
pub const PINNED_URLS: [&str; 2] = [
    "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-full_build-shared.7z",
    "https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-full_build-shared.7z",
];
/// Size shown before the download (bytes of the pinned archive).
pub const PINNED_SIZE: u64 = 59_758_994;
/// A guard against a server that never stops sending.
const MAX_ARCHIVE_BYTES: u64 = 200 * 1024 * 1024;
/// A temporary entry of another process untouched this long is a leftover of a killed one.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Where downloads live: `<local>/ffmpeg` (Windows: `%LOCALAPPDATA%\kinetren\ffmpeg`).
pub fn download_root() -> PathBuf {
    kt_dirs::ffmpeg_download_root()
}

/// What to download: the pinned archive, or (debug builds only) a test server's —
/// `KT_FFMPEG_DOWNLOAD_URL` + `KT_FFMPEG_DOWNLOAD_SHA256`, the pattern of `KT_TELEMETRY_URL`.
#[derive(Debug, Clone)]
pub struct DownloadSource {
    pub urls: Vec<String>,
    pub sha256: String,
    pub version: String,
    /// The archive's size, when it is known in advance (the pin): a mirror that announces
    /// another length is skipped before its body is read (ffmpeg-download-window-info, D3).
    pub size: Option<u64>,
    /// The archive's file name, for the window.
    pub archive: String,
}

impl DownloadSource {
    pub fn pinned() -> Self {
        if cfg!(debug_assertions)
            && let (Ok(url), Ok(sha)) =
                (std::env::var("KT_FFMPEG_DOWNLOAD_URL"), std::env::var("KT_FFMPEG_DOWNLOAD_SHA256"))
        {
            // several mirrors, space-separated: the live scenario's 404 and its wrong size
            let urls: Vec<String> = url.split_whitespace().map(str::to_string).collect();
            let size = std::env::var("KT_FFMPEG_DOWNLOAD_SIZE").ok().and_then(|v| v.parse().ok());
            let archive = urls.first().and_then(|u| u.rsplit('/').next()).unwrap_or_default().to_string();
            return DownloadSource { urls, sha256: sha, version: PINNED_VERSION.into(), size, archive };
        }
        DownloadSource {
            urls: PINNED_URLS.iter().map(|s| s.to_string()).collect(),
            sha256: PINNED_SHA256.into(),
            version: PINNED_VERSION.into(),
            size: Some(PINNED_SIZE),
            archive: PINNED_ARCHIVE.into(),
        }
    }
}

/// What the download is doing now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Connecting,
    Receiving,
    Verifying,
    Unpacking,
}

/// A mirror that did not deliver, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub url: String,
    pub error: String,
}

/// THE WHOLE STATE of the download at a moment (ffmpeg-download-window-info, D1): the background
/// job keeps only its latest progress value, so each value carries everything the window shows —
/// the mirror in use and its number, the bytes, the announced and the expected size, the
/// unpacking count and the mirrors that already failed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DownloadProgress {
    pub phase: Phase,
    /// 1-based number of the mirror in use, of `mirrors`.
    pub mirror: usize,
    pub mirrors: usize,
    pub url: String,
    pub done: u64,
    /// The size the server announced (Content-Length).
    pub announced: Option<u64>,
    /// The size known in advance (the pin).
    pub expected: Option<u64>,
    pub unpacked: usize,
    pub to_unpack: usize,
    pub attempts: Vec<Attempt>,
}

/// The window's own (time, bytes) samples of the download, for the speed and the time left
/// (ffmpeg-download-window-info, D2). Fixed size, no allocation; taken in `poll` when the bytes
/// change, and reset when the mirror changes, so a failed first mirror does not spoil the rate.
#[derive(Debug, Clone, Default)]
pub struct Samples {
    ring: [Option<(std::time::Instant, u64)>; 16],
    next: usize,
    mirror: usize,
}

impl Samples {
    pub fn push(&mut self, at: std::time::Instant, done: u64, mirror: usize) {
        let newest = self.newest();
        if mirror != self.mirror || newest.is_some_and(|(_, d)| done < d) {
            *self = Samples { mirror, ..Samples::default() };
        }
        if newest.is_some_and(|(_, d)| d == done) {
            return;
        }
        self.ring[self.next] = Some((at, done));
        self.next = (self.next + 1) % self.ring.len();
    }

    fn newest(&self) -> Option<(std::time::Instant, u64)> {
        self.ring.iter().flatten().copied().max_by_key(|(t, _)| *t)
    }

    /// Bytes per second over the last ≤ 5 s of samples; `None` with fewer than two samples or
    /// when nothing arrived for 2 s (a stalled mirror has no speed, not an old one).
    pub fn rate(&self, now: std::time::Instant) -> Option<f64> {
        let (t1, d1) = self.newest()?;
        if now.saturating_duration_since(t1) > std::time::Duration::from_secs(2) {
            return None;
        }
        let from = now.checked_sub(std::time::Duration::from_secs(5));
        let (t0, d0) = self
            .ring
            .iter()
            .flatten()
            .copied()
            .filter(|(t, _)| from.is_none_or(|f| *t >= f) && *t < t1)
            .min_by_key(|(t, _)| *t)?;
        let dt = t1.duration_since(t0).as_secs_f64();
        (dt > 0.0).then(|| (d1 - d0) as f64 / dt)
    }
}

/// The time left at `rate` bytes per second; `None` without a rate or a total.
pub fn eta(done: u64, total: Option<u64>, rate: Option<f64>) -> Option<std::time::Duration> {
    let (total, rate) = (total?, rate.filter(|r| *r > 0.0)?);
    Some(std::time::Duration::from_secs_f64(total.saturating_sub(done) as f64 / rate))
}

/// A finished download: where it went and what was verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub folder: PathBuf,
    pub sha256: String,
    pub archive: String,
}

/// Download, verify and install into `root/<version>`; the folder appears only whole.
/// Cancellation (`cancel`), a broken connection and a hash mismatch leave `root` without
/// traces of this attempt.
pub fn download_and_install(
    src: &DownloadSource,
    root: &std::path::Path,
    cancel: &dyn Fn() -> bool,
    step: &dyn Fn(DownloadProgress),
) -> Result<Installed, String> {
    std::fs::create_dir_all(root).map_err(|e| format!("{}: {e}", root.display()))?;
    remove_stale(root);
    let pid = std::process::id();
    let archive = root.join(format!(".download-{pid}.7z"));
    let partial = root.join(format!(".partial-{pid}"));
    let result = (|| {
        let mut p = fetch_verified(src, &archive, cancel, step)?;
        p.phase = Phase::Unpacking;
        step(p.clone());
        extract_dlls(&archive, &partial, cancel, &mut |unpacked, to_unpack| {
            p.unpacked = unpacked;
            p.to_unpack = to_unpack;
            step(p.clone());
        })?;
        let dest = root.join(&src.version);
        if dest.exists() {
            std::fs::remove_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        }
        std::fs::rename(&partial, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        Ok(Installed { folder: dest, sha256: src.sha256.to_ascii_lowercase(), archive: src.archive.clone() })
    })();
    let _ = std::fs::remove_file(&archive);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&partial);
    }
    result
}

/// Stream the archive to `out`, hashing as the bytes arrive; the first URL that delivers a
/// file with the pinned hash wins.
fn fetch_verified(
    src: &DownloadSource,
    out: &std::path::Path,
    cancel: &dyn Fn() -> bool,
    step: &dyn Fn(DownloadProgress),
) -> Result<DownloadProgress, String> {
    use sha2::Digest;
    use std::io::{Read, Write};
    // the proxy of the environment (HTTPS_PROXY) is used — corporate Windows needs it —
    // except for the machine itself: a proxy for loopback is never right
    let agent = |url: &str| -> ureq::Agent {
        let loopback = ["http://127.0.0.1", "http://localhost", "http://[::1]"]
            .iter()
            .any(|p| url.starts_with(p));
        let mut b = ureq::Agent::config_builder().timeout_connect(Some(std::time::Duration::from_secs(10)));
        if loopback {
            b = b.proxy(None);
        }
        b.build().into()
    };
    let mut p = DownloadProgress { mirrors: src.urls.len(), expected: src.size, ..Default::default() };
    for (i, url) in src.urls.iter().enumerate() {
        p = DownloadProgress {
            phase: Phase::Connecting,
            mirror: i + 1,
            url: url.clone(),
            done: 0,
            announced: None,
            ..p
        };
        step(p.clone());
        let agent = agent(url);
        let attempt = (|| -> Result<(), String> {
            let resp = agent.get(url).call().map_err(|e| e.to_string())?;
            let total = resp
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            p.announced = total;
            // A FILE OF ANOTHER LENGTH CANNOT HAVE THE PINNED HASH (D3): say so before reading
            // up to 57 MB of it
            if let (Some(got), Some(want)) = (total, src.size)
                && got != want
            {
                return Err(format!("announced {got} bytes, expected {want}"));
            }
            p.phase = Phase::Receiving;
            step(p.clone());
            let mut body = resp.into_body().into_with_config().limit(MAX_ARCHIVE_BYTES).reader();
            let mut file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
            let mut hasher = sha2::Sha256::new();
            let mut buf = vec![0u8; 256 * 1024];
            let mut done = 0u64;
            loop {
                if cancel() {
                    return Err("cancelled".into());
                }
                let n = body.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                done += n as u64;
                p.done = done;
                step(p.clone());
            }
            file.sync_all().map_err(|e| e.to_string())?;
            p.phase = Phase::Verifying;
            step(p.clone());
            let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
            if !got.eq_ignore_ascii_case(&src.sha256) {
                return Err(format!("checksum mismatch: expected {}, got {got}", src.sha256));
            }
            Ok(())
        })();
        match attempt {
            Ok(()) => return Ok(p),
            Err(e) if e == "cancelled" => {
                let _ = std::fs::remove_file(out);
                return Err(e);
            }
            Err(e) => {
                let _ = std::fs::remove_file(out);
                p.attempts.push(Attempt { url: url.clone(), error: e });
                step(p.clone());
            }
        }
    }
    Err(p.attempts.iter().map(|a| format!("{}: {}", a.url, a.error)).collect::<Vec<_>>().join("\n"))
}

/// Only `<root>/bin/*.dll` and `<root>/LICENSE*` of the archive, flat — the layout the
/// loader expects in a folder.
/// Is this archive entry one the loader needs (`<root>/bin/*.dll`, `<root>/LICENSE*`)?
fn wanted_entry(name: &str) -> bool {
    let name = name.replace('\\', "/");
    let parts: Vec<&str> = name.split('/').collect();
    match parts.as_slice() {
        [_, "bin", f] => f.to_ascii_lowercase().ends_with(".dll"),
        [_, f] => f.to_ascii_uppercase().starts_with("LICENSE"),
        _ => false,
    }
}

fn extract_dlls(
    archive: &std::path::Path,
    dest: &std::path::Path,
    cancel: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dest);
    std::fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut reader = sevenz_rust2::ArchiveReader::new(file, sevenz_rust2::Password::empty())
        .map_err(|e| format!("not a 7z archive: {e}"))?;
    // THE COUNT IS FREE (D4): the 7z header lists the entries before the solid stream
    let to_unpack = reader
        .archive()
        .files
        .iter()
        .filter(|e| !e.is_directory() && wanted_entry(e.name()))
        .count();
    progress(0, to_unpack);
    let mut count = 0usize;
    let mut cancelled = false;
    reader
        .for_each_entries(|entry, data| {
            if cancel() {
                cancelled = true;
                return Ok(false);
            }
            let name = entry.name().replace('\\', "/");
            let parts: Vec<&str> = name.split('/').collect();
            let wanted = wanted_entry(&name);
            if entry.is_directory() || !wanted {
                // the entry's data must still be consumed for the solid stream to go on
                std::io::copy(data, &mut std::io::sink())?;
                return Ok(true);
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            data.read_to_end(&mut bytes)?;
            std::fs::write(dest.join(parts[parts.len() - 1]), &bytes)?;
            count += 1;
            progress(count, to_unpack);
            Ok(true)
        })
        .map_err(|e| format!("extracting: {e}"))?;
    if cancelled {
        return Err("cancelled".into());
    }
    if count == 0 || !dest.read_dir().map(|mut d| d.any(|e| {
        e.map(|e| e.file_name().to_string_lossy().to_ascii_lowercase().starts_with("avcodec-")).unwrap_or(false)
    })).unwrap_or(false) {
        return Err("the archive holds no FFmpeg DLLs".into());
    }
    Ok(())
}

/// Remove `.download-*` / `.partial-*` entries of OTHER processes untouched for a while:
/// leftovers of a killed download. A live download writes continuously, so its entries are
/// never that old.
fn remove_stale(root: &std::path::Path) {
    let me = std::process::id().to_string();
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix(".download-").or_else(|| name.strip_prefix(".partial-")) else {
            continue;
        };
        let pid = rest.trim_end_matches(".7z");
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > STALE_AFTER);
        if pid != me && old {
            let p = e.path();
            let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        }
    }
}

// ---- the window: View → FFmpeg… (ui-reach) ---------------------------------------------
//
// One place where a person sees whether FFmpeg was found, where, which version, why every
// other candidate was not used — and fixes it with the mouse: Download (Windows), Locate
// folder…, Rescan. At startup without FFmpeg the same window opens once: a notice without
// buttons would be a dead end ("create, but can't manage", rounds 33–40). The source and
// the licence of the download are shown next to its button, before the click.

use crate::i18n::{k, tr};
use eframe::egui;

/// The state of the window.
pub struct FfmpegUi {
    pub open: bool,
    /// Where Locate… saves the choice (a test points it into its own folder).
    pub choice_file: PathBuf,
    /// Where Download installs (a test points it into its own folder).
    pub download_root: PathBuf,
    pub job: Option<crate::bg::BgJob<DownloadProgress, Result<Installed, String>>>,
    /// The outcome of the last action: (is an error, text).
    pub message: Option<(bool, String)>,
    /// A test's answer of the folder picker (the native dialog cannot be driven).
    pub test_pick: Option<PathBuf>,
    /// Does the pin's folder (`<download_root>/<PINNED_VERSION>`) exist? Asked of the disk
    /// once and after every search or download, not per frame (invariant 12); `None` = not
    /// asked yet.
    pin_present: Option<bool>,
    /// The last successful download: where it went and what was verified (the success block).
    pub installed: Option<Installed>,
    /// The progress the job reported last — kept after it ends, so a failure still shows the
    /// mirrors that were tried and why.
    pub last: Option<DownloadProgress>,
    /// The window's own samples of the bytes, for the speed and the time left (D2).
    pub samples: Samples,
    /// The OS whose rules the window follows (D6): native; a test sets Windows on any OS.
    pub os: kt_ffmpeg::search::Os,
    /// A test's record of the folders **Open folder** asked to show (D5), like `test_pick`.
    pub test_open: Option<Vec<PathBuf>>,
    /// What **Download** gets: the pin (a test points it at its own servers). Computed once —
    /// the window reads it every frame.
    pub source: DownloadSource,
}

impl Default for FfmpegUi {
    fn default() -> Self {
        FfmpegUi {
            open: false,
            choice_file: user_choice_file(),
            download_root: download_root(),
            job: None,
            message: None,
            test_pick: None,
            pin_present: None,
            installed: None,
            last: None,
            samples: Samples::default(),
            // the Windows rules on any OS, for the live scenario (debug builds only, D6)
            os: if cfg!(debug_assertions) && std::env::var("KT_FFMPEG_OS").as_deref() == Ok("windows") {
                kt_ffmpeg::search::Os::Windows
            } else {
                kt_ffmpeg::search::Os::NATIVE
            },
            test_open: None,
            source: DownloadSource::pinned(),
        }
    }
}

/// Is **Download FFmpeg <pin>** offered (ffmpeg-9, ui-reach, t867)? On Windows while the pin's
/// folder is absent — whatever is loaded: nothing, an EARLIER download (`8.1.2`) or a folder
/// the person chose. Linux has the distribution's packages and no button. Pure, any OS.
pub fn download_offered(os: kt_ffmpeg::search::Os, pin_present: bool) -> bool {
    os == kt_ffmpeg::search::Os::Windows && !pin_present
}

impl FfmpegUi {
    /// Does the pin's folder exist? (cached, see `pin_present`)
    pub fn pin_present(&mut self) -> bool {
        *self.pin_present.get_or_insert_with(|| self.download_root.join(PINNED_VERSION).is_dir())
    }

    fn read_choice(&self) -> Option<String> {
        let text = std::fs::read_to_string(&self.choice_file).ok()?;
        let line = text.lines().next()?.trim();
        (!line.is_empty()).then(|| line.to_string())
    }

    /// The folder **Open folder** shows (D5): the installed one after a success, else the folder
    /// of the loaded FFmpeg when it is a folder, else the download root. `None` on Linux with a
    /// system FFmpeg — a SONAME has no folder to open.
    pub fn folder_to_open(&self) -> Option<PathBuf> {
        if let Some(i) = &self.installed {
            return Some(i.folder.clone());
        }
        if let Some(place) = kt_ffmpeg::loaded_place() {
            let p = PathBuf::from(&place);
            if p.is_dir() {
                return Some(p);
            }
            if let Some(dir) = p.parent().filter(|d| d.is_dir() && !d.as_os_str().is_empty()) {
                return Some(dir.to_path_buf());
            }
            if self.os == kt_ffmpeg::search::Os::Linux {
                return None;
            }
        }
        (self.os == kt_ffmpeg::search::Os::Windows).then(|| self.download_root.clone())
    }

    /// **Open folder**: the test records it, the program asks the file manager (D5); the root is
    /// created when missing, so the person sees where a download will go.
    pub fn open_folder(&mut self) {
        let Some(dir) = self.folder_to_open() else { return };
        if dir == self.download_root {
            let _ = std::fs::create_dir_all(&dir);
        }
        if let Some(rec) = self.test_open.as_mut() {
            rec.push(dir);
            return;
        }
        if let Err(e) = crate::util::open_in_file_manager(&dir) {
            self.message = Some((true, e));
        }
    }

    /// Search again with the current folders; the message says what happened.
    pub fn rescan(&mut self) {
        self.pin_present = None;
        kt_ffmpeg::set_search_dirs(self.read_choice(), download_root_for_search(&self.download_root));
        self.message = match kt_ffmpeg::rescan() {
            Ok(info) => Some((
                false,
                crate::tf!(k::ffmpeg::INSTALLED, info.major, kt_ffmpeg::loaded_place().unwrap_or_default()),
            )),
            Err(_) => None, // the window shows the report itself
        };
    }

    /// The folder picked with Locate…: refused with the reason, or saved and searched.
    pub fn locate(&mut self, dir: &std::path::Path) {
        let d = dir.to_string_lossy().into_owned();
        match kt_ffmpeg::check_folder(&d) {
            Err(r) => self.message = Some((true, crate::tf!(k::ffmpeg::REFUSED, d, reject_text(&r)))),
            Ok(_) => {
                if let Err(e) = kt_otio::write_atomic(&self.choice_file, &format!("{d}\n")) {
                    self.message = Some((true, format!("{}: {e}", self.choice_file.display())));
                    return;
                }
                if kt_ffmpeg::backend_info().is_some() {
                    // a loaded FFmpeg stays for the process: its function pointers are in use
                    self.message = Some((false, tr(k::ffmpeg::RESTART).to_string()));
                } else {
                    self.rescan();
                }
            }
        }
    }

    pub fn start_download(&mut self) {
        if self.job.is_some() {
            return;
        }
        let root = self.download_root.clone();
        let src = self.source.clone();
        self.message = None;
        self.installed = None;
        self.last = None;
        self.samples = Samples::default();
        self.job = Some(crate::bg::BgJob::spawn("ffmpeg-download", move |ctl, send| {
            download_and_install(&src, &root, &|| ctl.should_stop(), &|st| send.send(st))
        }));
    }

    /// Collect the job; `true` — something changed (a reason to repaint).
    pub fn poll(&mut self) -> bool {
        let Some(job) = self.job.as_mut() else { return false };
        let changed = job.poll();
        if let Some(p) = &job.progress {
            if p.phase == Phase::Receiving {
                self.samples.push(std::time::Instant::now(), p.done, p.mirror);
            }
            self.last = Some(p.clone());
        }
        if let Some(done) = job.done.take() {
            self.job = None;
            match done {
                // a download over an already loaded FFmpeg (an earlier download, a chosen
                // folder): the loaded set stays for the process, so the new one is used after
                // a restart — "installed FFmpeg 8" would be a lie (ffmpeg-9)
                Ok(inst) if kt_ffmpeg::is_loaded() => {
                    self.pin_present = None;
                    self.installed = Some(inst);
                    self.message = Some((false, tr(k::ffmpeg::RESTART).to_string()));
                }
                Ok(inst) => {
                    self.installed = Some(inst);
                    self.rescan();
                }
                Err(e) if e == "cancelled" => self.message = None,
                Err(e) => self.message = Some((true, crate::tf!(k::ffmpeg::FAILED, e))),
            }
            return true;
        }
        changed
    }
}

/// The host of a URL, without the scheme, the port and the path (www.gyan.dev).
pub fn url_host(url: &str) -> &str {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = rest.split('/').next().unwrap_or(rest);
    host.rsplit_once(':').filter(|(_, port)| port.bytes().all(|b| b.is_ascii_digit())).map(|(h, _)| h).unwrap_or(host)
}

/// The first 16 hex digits of a SHA-256 and an ellipsis — the full value goes into the hover, so
/// the body does not widen the 560 px window (D7).
pub fn short_hash(sha: &str) -> String {
    format!("{}…", &sha[..sha.len().min(16)])
}

/// "about 11 s left" / "about 3 min left".
fn time_left(d: std::time::Duration) -> String {
    let s = d.as_secs().max(1);
    if s < 90 {
        crate::tf!(k::ffmpeg::LEFT_SECS, s)
    } else {
        crate::tf!(k::ffmpeg::LEFT_MINS, s.div_ceil(60))
    }
}

/// The text of the progress bar for a snapshot: the bytes, the speed (a dash when there is none)
/// and the time left (absent without a speed), or the phase.
pub fn progress_text(p: &DownloadProgress, rate: Option<f64>) -> (f32, String) {
    let mb = 1024 * 1024;
    match p.phase {
        Phase::Connecting => (0.0, crate::tf!(k::ffmpeg::CONNECTING_TO, url_host(&p.url), p.mirror, p.mirrors)),
        Phase::Receiving => {
            let total = p.announced.or(p.expected).unwrap_or(PINNED_SIZE).max(1);
            let speed = match rate {
                Some(r) => crate::tf!(k::ffmpeg::SPEED, format!("{:.1}", r / mb as f64)),
                None => "—".to_string(),
            };
            let mut text = format!(
                "{} · {speed}",
                crate::tf!(k::ffmpeg::PROGRESS, p.done / mb, total.div_ceil(mb))
            );
            if let Some(left) = eta(p.done, Some(total), rate) {
                text.push_str(" · ");
                text.push_str(&time_left(left));
            }
            (p.done as f32 / total as f32, text)
        }
        Phase::Verifying => (1.0, tr(k::ffmpeg::VERIFYING).to_string()),
        Phase::Unpacking => (
            p.unpacked as f32 / p.to_unpack.max(1) as f32,
            crate::tf!(k::ffmpeg::UNPACKING, p.unpacked, p.to_unpack),
        ),
    }
}

/// A rejection in the person's language (the loader's own text is English).
pub fn reject_text(r: &kt_ffmpeg::search::Reject) -> String {
    use kt_ffmpeg::search::Reject;
    match r {
        Reject::Unreadable => tr(k::ffmpeg::R_UNREADABLE).to_string(),
        Reject::NoAvcodec => tr(k::ffmpeg::R_NO_AVCODEC).to_string(),
        Reject::StaticBuild => tr(k::ffmpeg::R_STATIC).to_string(),
        Reject::Unsupported { avcodec } => {
            let ffmpeg = if *avcodec >= 63 { (avcodec - 54).to_string() } else { format!("avcodec {avcodec}") };
            crate::tf!(k::ffmpeg::R_UNSUPPORTED, ffmpeg)
        }
        Reject::Missing { ffmpeg, file } => crate::tf!(k::ffmpeg::R_MISSING, ffmpeg, file),
        Reject::Load(e) => crate::tf!(k::ffmpeg::R_LOAD, e),
    }
}

impl crate::app::KtApp {
    /// Startup: without FFmpeg the window opens once, with one notice (ui-reach).
    pub(crate) fn ffmpeg_startup_check(&mut self) {
        if kt_ffmpeg::backend_info().is_none() {
            self.ffmpeg_ui.open = true;
            self.warn(tr(k::ffmpeg::NOT_FOUND));
        }
    }

    pub(crate) fn draw_ffmpeg_window(&mut self, ctx: &egui::Context) {
        if self.ffmpeg_ui.poll() {
            ctx.request_repaint();
        }
        if self.ffmpeg_ui.job.is_some() {
            // the bar moves while bytes arrive; at rest nothing repaints (invariant 12)
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if !self.ffmpeg_ui.open {
            return;
        }
        let mut open = true;
        let mut act: Option<&'static str> = None;
        let info = kt_ffmpeg::backend_info();
        let report = kt_ffmpeg::search_report_data();
        let offer = download_offered(self.ffmpeg_ui.os, self.ffmpeg_ui.pin_present());
        let open_dir = self.ffmpeg_ui.folder_to_open();
        let rate = self.ffmpeg_ui.samples.rate(std::time::Instant::now());
        let win = crate::window_shell::Shell::new(
            egui::Id::new("kt_ffmpeg"),
            tr(k::ffmpeg::TITLE),
            crate::window_shell::Kind::Tool,
        )
            .default_width(560.0)
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                    match &info {
                        Some(i) => {
                            let (a, b, c) = i.avcodec_version;
                            let place = kt_ffmpeg::loaded_place().unwrap_or_default();
                            ui.strong(crate::tf!(k::ffmpeg::LOADED, i.major, format!("{a}.{b}.{c}"), place));
                        }
                        None => {
                            ui.colored_label(crate::app::HINT_ERROR, tr(k::ffmpeg::NOT_FOUND));
                        }
                    }
                    let rejected: Vec<_> = report
                        .as_ref()
                        .map(|r| r.rejected().cloned().collect())
                        .unwrap_or_default();
                    if !rejected.is_empty() {
                        ui.add_space(4.0);
                        ui.label(tr(k::ffmpeg::REJECTED));
                        for l in rejected {
                            let why = l.reject.as_ref().map(reject_text).unwrap_or_default();
                            ui.horizontal_wrapped(|ui| {
                                ui.weak(&l.place);
                                ui.label(format!("— {why}"));
                            });
                        }
                    }
                    if info.is_none() {
                        ui.add_space(4.0);
                        let hint = if cfg!(windows) { k::ffmpeg::HINT_WINDOWS } else { k::ffmpeg::HINT_LINUX };
                        ui.label(tr(hint));
                    }
                    if let Some((err, text)) = &self.ffmpeg_ui.message {
                        ui.add_space(4.0);
                        if *err {
                            ui.colored_label(crate::app::HINT_ERROR, text);
                        } else {
                            ui.label(text);
                        }
                    }
                    let ui_state = &self.ffmpeg_ui;
                    // BEFORE: what Download will do, where, and how to undo it (D7)
                    if offer && ui_state.job.is_none() && ui_state.installed.is_none() {
                        let src = &ui_state.source;
                        let mb = src.size.unwrap_or(PINNED_SIZE).div_ceil(1024 * 1024);
                        let host = src.urls.first().map(|u| url_host(u)).unwrap_or_default();
                        let mirror = src.urls.get(1).map(|u| url_host(u)).unwrap_or("—");
                        let dest = ui_state.download_root.join(&src.version);
                        ui.add_space(4.0);
                        ui.weak(crate::tf!(k::ffmpeg::DOWNLOAD_NOTE, mb));
                        let r = ui.label(crate::tf!(
                            k::ffmpeg::WHAT_HAPPENS,
                            &src.archive,
                            mb,
                            host,
                            mirror,
                            short_hash(&src.sha256)
                        ));
                        crate::widgets::field_probe::note("ffmpeg.what", r.rect);
                        r.on_hover_text(&src.sha256);
                        ui.label(crate::tf!(k::ffmpeg::WHAT_HAPPENS_WHERE, dest.display()));
                    }
                    // the mirrors that did not deliver, each with its reason (while running and
                    // after a failure)
                    if let Some(p) = &ui_state.last
                        && !p.attempts.is_empty()
                        && ui_state.installed.is_none()
                    {
                        ui.add_space(4.0);
                        let r = ui.label(tr(k::ffmpeg::TRIED));
                        crate::widgets::field_probe::note("ffmpeg.tried", r.rect);
                        for a in &p.attempts {
                            ui.horizontal_wrapped(|ui| {
                                ui.weak(url_host(&a.url));
                                ui.label(format!("— {}", a.error));
                            });
                        }
                    }
                    // AFTER a success: where it went and what was verified
                    if let Some(i) = &ui_state.installed {
                        ui.add_space(4.0);
                        ui.label(crate::tf!(k::ffmpeg::INSTALLED_INTO, i.folder.display()));
                        ui.label(crate::tf!(k::ffmpeg::SHA_MATCHES, short_hash(&i.sha256)))
                            .on_hover_text(&i.sha256);
                    }
                });
                crate::widgets::window_actions(ui, |ui| {
                    if self.ffmpeg_ui.job.is_some() {
                        let (frac, text) = match &self.ffmpeg_ui.last {
                            Some(p) => {
                                // the mirror in use, above the bar: which host and which of how many
                                if p.mirror > 0 {
                                    ui.weak(crate::tf!(k::ffmpeg::MIRROR, url_host(&p.url), p.mirror, p.mirrors));
                                }
                                progress_text(p, rate)
                            }
                            None => (0.0, tr(k::ffmpeg::CONNECTING).to_string()),
                        };
                        let r = ui.add(egui::ProgressBar::new(frac).text(&text).desired_width(ui.available_width()));
                        crate::widgets::field_probe::note("ffmpeg.progress", r.rect);
                    }
                    ui.horizontal(|ui| {
                        if self.ffmpeg_ui.job.is_some() {
                            if ui.button(tr(k::ffmpeg::CANCEL)).clicked() {
                                act = Some("cancel");
                            }
                        } else if offer {
                            let r = ui.button(crate::tf!(k::ffmpeg::DOWNLOAD, PINNED_VERSION));
                            crate::widgets::field_probe::note("ffmpeg.download", r.rect);
                            if r.clicked() {
                                act = Some("download");
                            }
                        }
                        if open_dir.is_some() {
                            let r = ui.button(tr(k::ffmpeg::OPEN_FOLDER));
                            crate::widgets::field_probe::note("ffmpeg.open_folder", r.rect);
                            if r.clicked() {
                                act = Some("open_folder");
                            }
                        }
                        if ui.button(tr(k::ffmpeg::LOCATE)).clicked() {
                            act = Some("locate");
                        }
                        if ui.button(tr(k::ffmpeg::RESCAN)).clicked() {
                            act = Some("rescan");
                        }
                    });
                });
            });
        if win.closed() {
            open = false;
        }
        if !open {
            self.ffmpeg_ui.open = false;
        }
        match act {
            Some("download") => self.ffmpeg_ui.start_download(),
            Some("cancel") => {
                if let Some(j) = &self.ffmpeg_ui.job {
                    j.cancel();
                }
            }
            Some("locate") => {
                let picked = match self.ffmpeg_ui.test_pick.take() {
                    Some(p) => Some(p),
                    None => rfd::FileDialog::new().set_title(tr(k::ffmpeg::LOCATE)).pick_folder(),
                };
                if let Some(dir) = picked {
                    self.ffmpeg_ui.locate(&dir);
                }
            }
            Some("rescan") => self.ffmpeg_ui.rescan(),
            Some("open_folder") => self.ffmpeg_ui.open_folder(),
            _ => {}
        }
        if act.is_some() {
            // a late load: the placeholders reopen on the next frames (poll_returned_sources)
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, Ordering};

    /// t867 (ffmpeg-9, ui-reach): **Download FFmpeg <pin>** stays offered over an earlier
    /// download and over a chosen folder, and goes away only when the pin's folder exists; the
    /// window's first line names the folder that was loaded. The search plan decides what loads
    /// (Windows rules, on any OS), and the offer is a pure function of the pin's folder.
    #[test]
    fn t867_download_stays_offered_over_an_earlier_download() {
        use kt_ffmpeg::search::{self, Env, LOAD_ORDER, Os};
        use std::collections::HashMap;

        struct Machine {
            dirs: HashMap<String, Vec<String>>,
            choice: Option<String>,
        }
        impl Env for Machine {
            fn var(&self, _: &str) -> Option<String> {
                None
            }
            fn exe_dir(&self) -> Option<String> {
                None
            }
            fn user_choice(&self) -> Option<String> {
                self.choice.clone()
            }
            fn download_root(&self) -> Option<String> {
                Some(ROOT.into())
            }
            fn list(&self, dir: &str) -> Option<Vec<String>> {
                self.dirs.get(dir).cloned()
            }
            fn read_link(&self, _: &str) -> Option<String> {
                None
            }
        }
        const ROOT: &str = r"C:\Users\u\AppData\Local\kinetren\ffmpeg";
        let set = |ffmpeg: u32| -> Vec<String> {
            LOAD_ORDER.iter().map(|l| search::file_name(Os::Windows, *l, ffmpeg)).collect()
        };
        let at = |v: &str| format!(r"{ROOT}\{v}");
        // what the window shows for a machine: (Download offered, the first line)
        let window = |m: &Machine| -> (bool, Option<String>) {
            let pin_present = m.list(ROOT).unwrap_or_default().iter().any(|e| e == PINNED_VERSION);
            let plan = search::plan(Os::Windows, m);
            let line = plan.iter().find(|c| c.verdict.is_ok()).map(|c| {
                let major = *c.verdict.as_ref().unwrap();
                // the avcodec version is not part of this scene (and a dotted literal here
                // would read as a probe name to scripts/live-coverage.py)
                crate::tf!(k::ffmpeg::LOADED, major, "-", c.dir.clone().unwrap())
            });
            (download_offered(Os::Windows, pin_present), line)
        };
        // an earlier pin's folder: older than any pin, so the scene does not depend on the pin
        const EARLIER: &str = "8.0.1";

        // 1. nothing downloaded, nothing found
        let none = Machine { dirs: HashMap::new(), choice: None };
        assert_eq!(window(&none), (true, None));

        // 2. a download from an earlier Kinetren, the current pin not yet
        let earlier = Machine {
            dirs: HashMap::from([(ROOT.into(), vec![EARLIER.into()]), (at(EARLIER), set(8))]),
            choice: None,
        };
        let (offered, line) = window(&earlier);
        assert!(offered, "Download is offered over an earlier download");
        let line = line.expect("FFmpeg 8 is loaded from the earlier download");
        assert!(line.contains(&at(EARLIER)) && line.starts_with("FFmpeg 8"), "{line}");

        // 3. the pin is downloaded: it loads first, and Download goes away
        let current = Machine {
            dirs: HashMap::from([
                (ROOT.into(), vec![EARLIER.into(), PINNED_VERSION.into()]),
                (at(EARLIER), set(8)),
                (at(PINNED_VERSION), set(9)),
            ]),
            choice: None,
        };
        let (offered, line) = window(&current);
        assert!(!offered, "the pin's folder exists — nothing to offer");
        assert!(line.unwrap().contains(&at(PINNED_VERSION)));

        // 4. FFmpeg 9 from a folder the person chose, nothing downloaded
        let chosen = Machine {
            dirs: HashMap::from([(r"D:\ff9\bin".into(), set(9))]),
            choice: Some(r"D:\ff9\bin".into()),
        };
        let (offered, line) = window(&chosen);
        assert!(offered, "a chosen folder does not hide the Download button");
        assert!(line.unwrap().contains(r"D:\ff9\bin"));

        // Linux: the distribution's packages, never a button
        assert!(!download_offered(Os::Linux, false) && !download_offered(Os::Linux, true));
    }

    /// A tiny archive with the structure of gyan.dev's: `<root>/bin/*.dll` (avcodec among
    /// them), `<root>/bin/ffmpeg.exe`, `<root>/include/…`, `<root>/LICENSE`.
    pub(crate) fn make_archive(path: &std::path::Path) -> Vec<u8> {
        let mut w = sevenz_rust2::ArchiveWriter::create(path).unwrap();
        let files: [(&str, &[u8]); 6] = [
            ("ffmpeg-9.0.2-full_build-shared/bin/avcodec-63.dll", b"codec"),
            ("ffmpeg-9.0.2-full_build-shared/bin/avutil-61.dll", b"util"),
            ("ffmpeg-9.0.2-full_build-shared/bin/ffmpeg.exe", b"exe"),
            ("ffmpeg-9.0.2-full_build-shared/include/libavcodec/avcodec.h", b"header"),
            ("ffmpeg-9.0.2-full_build-shared/LICENSE", b"GPLv3"),
            ("ffmpeg-9.0.2-full_build-shared/README.txt", b"readme"),
        ];
        for (name, data) in files {
            w.push_archive_entry(sevenz_rust2::ArchiveEntry::new_file(name), Some(data)).unwrap();
        }
        w.finish().unwrap();
        std::fs::read(path).unwrap()
    }

    /// A local HTTP server for one file. `cut` > 0: send only that many bytes, then hang
    /// up; `slow`: send in small pieces with pauses (so a cancel lands mid-way).
    pub(crate) fn serve(body: Vec<u8>, cut: usize, slow: bool) -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut req = [0u8; 4096];
                let _ = s.read(&mut req);
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let end = if cut > 0 { cut } else { body.len() };
                for chunk in body[..end].chunks(if slow { 64 } else { 64 * 1024 }) {
                    if s.write_all(chunk).is_err() {
                        break;
                    }
                    if slow {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                }
            }
        });
        format!("http://{addr}/ffmpeg.7z")
    }

    pub(crate) fn sha(bytes: &[u8]) -> String {
        use sha2::Digest;
        sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
    }

    pub(crate) fn entries(root: &std::path::Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(root)
            .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    /// t848 (windows-build, media-index): verify, extract only DLLs and the licence,
    /// install atomically; a wrong hash, a cancel and a broken connection leave nothing.
    #[test]
    fn t848_download_verifies_extracts_and_installs_atomically() {
        let dir = std::env::temp_dir().join(format!("kt-t848-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bytes = make_archive(&dir.join("ffmpeg-mini.7z"));
        let root = dir.join("ffmpeg");
        let never = || false;
        let quiet = |_: DownloadProgress| {};
        let src = |url: String, sha256: String| DownloadSource {
            urls: vec![url],
            sha256,
            version: "9.0.2".into(),
            size: None,
            archive: "ffmpeg-mini.7z".into(),
        };

        // wrong hash: named, nothing left
        let wrong = "0".repeat(64);
        let err = download_and_install(&src(serve(bytes.clone(), 0, false), wrong.clone()), &root, &never, &quiet)
            .expect_err("a wrong hash is refused");
        assert!(err.contains(&wrong) && err.contains(&sha(&bytes)), "expected and got are named: {err}");
        assert!(entries(&root).is_empty(), "nothing left: {:?}", entries(&root));

        // the right hash: only DLLs and the licence, no temporaries
        let steps = std::sync::Mutex::new(Vec::new());
        let dest = download_and_install(&src(serve(bytes.clone(), 0, false), sha(&bytes)), &root, &never, &|s| {
            steps.lock().unwrap().push(s)
        })
        .expect("the right hash installs")
        .folder;
        assert_eq!(dest, root.join("9.0.2"));
        assert_eq!(entries(&root), vec!["9.0.2".to_string()], "no temporary entries");
        assert_eq!(entries(&dest), vec!["LICENSE", "avcodec-63.dll", "avutil-61.dll"]);
        assert_eq!(std::fs::read(dest.join("avcodec-63.dll")).unwrap(), b"codec");
        let steps = steps.into_inner().unwrap();
        assert!(steps.iter().any(|s| s.phase == Phase::Verifying) && steps.iter().any(|s| s.phase == Phase::Unpacking));
        assert!(steps.iter().any(|s| s.phase == Phase::Receiving && s.announced == Some(bytes.len() as u64)));

        // cancel mid-way and a broken connection: the installed folder stays, nothing else
        let flag = AtomicBool::new(false);
        let url = serve(bytes.clone(), 0, true);
        let err = download_and_install(&src(url, sha(&bytes)), &root, &|| flag.load(Ordering::Relaxed), &|s| {
            if s.phase == Phase::Receiving && s.done > 128 {
                flag.store(true, Ordering::Relaxed);
            }
        })
        .expect_err("cancelled");
        assert_eq!(err, "cancelled");
        assert_eq!(entries(&root), vec!["9.0.2".to_string()]);

        let err = download_and_install(&src(serve(bytes.clone(), bytes.len() / 2, false), sha(&bytes)), &root, &never, &quiet)
            .expect_err("a broken connection fails");
        assert!(!err.is_empty());
        assert_eq!(entries(&root), vec!["9.0.2".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// t948 (ffmpeg-download-window-info, D2): the speed and the time left come from the window's
    /// own samples — over the last 5 s, none when the bytes stopped for 2 s, reset on a new mirror.
    #[test]
    fn t948_speed_and_time_left_from_samples() {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mb = 1024 * 1024;
        let mut s = Samples::default();
        assert_eq!(s.rate(at(0)), None, "no samples, no speed");
        s.push(at(0), 0, 1);
        assert_eq!(s.rate(at(100)), None, "one sample, no speed");
        // 1 MB a second for 8 s: the rate reads the last 5 s
        for i in 1..=8u64 {
            s.push(at(i * 1000), i * mb, 1);
        }
        let r = s.rate(at(8000)).expect("a steady rate");
        assert!((r - mb as f64).abs() < 1.0, "{r}");
        // the same bytes again are not a new sample, and 2 s without bytes is no speed
        s.push(at(9000), 8 * mb, 1);
        assert!(s.rate(at(9500)).is_some());
        assert_eq!(s.rate(at(10_500)), None, "stalled for 2.5 s");
        // a new mirror starts from scratch: the old one's bytes do not count
        s.push(at(11_000), 0, 2);
        assert_eq!(s.rate(at(11_000)), None, "one sample of the new mirror");
        s.push(at(12_000), mb / 2, 2);
        let r = s.rate(at(12_000)).unwrap();
        assert!((r - (mb / 2) as f64).abs() < 1.0, "{r}");
        // the time left: 10 MB to go at 1 MB/s is 10 s; nothing without a rate or a total
        assert_eq!(eta(5 * mb, Some(15 * mb), Some(mb as f64)), Some(Duration::from_secs(10)));
        assert_eq!(eta(5 * mb, None, Some(mb as f64)), None);
        assert_eq!(eta(5 * mb, Some(15 * mb), None), None);
        assert_eq!(eta(5 * mb, Some(15 * mb), Some(0.0)), None);
    }

    /// A local server that answers every request with `status` and an empty body.
    pub(crate) fn serve_status(status: &'static str) -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut req = [0u8; 4096];
                let _ = s.read(&mut req);
                let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            }
        });
        format!("http://{addr}/ffmpeg.7z")
    }

    /// t949 (ffmpeg-download-window-info, D1/D3/D4): every snapshot tells the whole state; a
    /// mirror announcing the wrong size is skipped before its body is read; failed mirrors are
    /// listed with their reasons; unpacking is counted and cancellable.
    #[test]
    fn t949_download_progress_tells_the_whole_state() {
        let dir = std::env::temp_dir().join(format!("kt-t949-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bytes = make_archive(&dir.join("ffmpeg-mini.7z"));
        let never = || false;
        let src = |urls: Vec<String>| DownloadSource {
            urls,
            sha256: sha(&bytes),
            version: "9.0.2".into(),
            size: Some(bytes.len() as u64),
            archive: "ffmpeg-mini.7z".into(),
        };

        // 1) the first mirror announces another size: skipped BEFORE the body, the second installs
        let mut longer = bytes.clone();
        longer.extend_from_slice(&[0u8; 10]);
        let (bad, good) = (serve(longer.clone(), 0, false), serve(bytes.clone(), 0, false));
        let steps = std::sync::Mutex::new(Vec::new());
        let root = dir.join("one");
        let got = download_and_install(&src(vec![bad.clone(), good.clone()]), &root, &never, &|s| {
            steps.lock().unwrap().push(s)
        })
        .expect("the second mirror installs");
        assert_eq!(got.folder, root.join("9.0.2"));
        assert_eq!((got.sha256.as_str(), got.archive.as_str()), (sha(&bytes).as_str(), "ffmpeg-mini.7z"));
        let steps = steps.into_inner().unwrap();
        assert!(
            !steps.iter().any(|s| s.mirror == 1 && s.phase == Phase::Receiving),
            "the wrong-size mirror's body was read"
        );
        let last = steps.last().unwrap();
        assert_eq!((last.mirror, last.mirrors, last.url.as_str()), (2, 2, good.as_str()));
        assert_eq!(last.attempts.len(), 1, "{:?}", last.attempts);
        assert_eq!(last.attempts[0].url, bad);
        let want = format!("announced {} bytes, expected {}", longer.len(), bytes.len());
        assert_eq!(last.attempts[0].error, want);
        // the unpacking is counted: avcodec, avutil and the licence, one by one
        let unpack: Vec<(usize, usize)> =
            steps.iter().filter(|s| s.phase == Phase::Unpacking).map(|s| (s.unpacked, s.to_unpack)).collect();
        assert_eq!(unpack.last(), Some(&(3, 3)), "{unpack:?}");
        assert!(unpack.contains(&(0, 3)) && unpack.contains(&(1, 3)), "{unpack:?}");

        // 2) a 404 and a broken connection: both listed with their reasons, nothing installed
        let (missing, broken) = (serve_status("404 Not Found"), serve(bytes.clone(), bytes.len() / 2, false));
        let last = std::sync::Mutex::new(DownloadProgress::default());
        let root = dir.join("two");
        let err = download_and_install(&src(vec![missing.clone(), broken.clone()]), &root, &never, &|s| {
            *last.lock().unwrap() = s
        })
        .expect_err("both mirrors fail");
        let last = last.into_inner().unwrap();
        assert_eq!(last.attempts.iter().map(|a| a.url.clone()).collect::<Vec<_>>(), [missing.clone(), broken]);
        assert!(last.attempts[0].error.contains("404"), "{:?}", last.attempts[0]);
        assert!(err.contains(&missing) && err.lines().count() == 2, "{err}");
        assert!(entries(&root).is_empty(), "nothing left: {:?}", entries(&root));

        // 3) a cancel during unpacking leaves nothing either
        let flag = AtomicBool::new(false);
        let root = dir.join("three");
        let err = download_and_install(&src(vec![serve(bytes.clone(), 0, false)]), &root, &|| {
            flag.load(Ordering::Relaxed)
        }, &|s| {
            if s.phase == Phase::Unpacking {
                flag.store(true, Ordering::Relaxed);
            }
        })
        .expect_err("cancelled while unpacking");
        assert_eq!(err, "cancelled");
        assert!(entries(&root).is_empty(), "nothing left: {:?}", entries(&root));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// windows-build task 5.2: the real pinned archive (cached by `scripts/fetch-ffmpeg-win.sh`)
    /// extracts to the same DLL bytes as 7-Zip. Skipped without the cache.
    #[test]
    fn pinned_archive_extracts_like_7zip() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ffmpeg-win");
        let archive = base.join(PINNED_ARCHIVE);
        let by_7z = base.join(PINNED_VERSION);
        if !archive.exists() || !by_7z.join("avcodec-63.dll").exists() {
            eprintln!("skipped: no cached pinned archive (scripts/fetch-ffmpeg-win.sh)");
            return;
        }
        let out = std::env::temp_dir().join(format!("kt-pinned-{}", std::process::id()));
        extract_dlls(&archive, &out, &|| false, &mut |_, _| {}).expect("extracts");
        assert_eq!(entries(&out), entries(&by_7z));
        for f in entries(&out) {
            assert!(std::fs::read(out.join(&f)).unwrap() == std::fs::read(by_7z.join(&f)).unwrap(), "{f} differs");
        }
        let _ = std::fs::remove_dir_all(&out);
    }
}
