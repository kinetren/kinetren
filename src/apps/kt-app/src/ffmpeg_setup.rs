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
    kt_ffmpeg::set_search_dirs(read_user_choice(), download_dir_for_search());
}

/// The download folder is a step of the Windows search only (Linux has system packages).
pub fn download_dir_for_search() -> Option<String> {
    cfg!(windows).then(|| kt_dirs::ffmpeg_download_dir().to_string_lossy().into_owned())
}

// ---- the download of the pinned build (Windows; media-index) ------------------------------
//
// One PINNED archive, checked against a hash committed HERE (not the `.sha256` next to it on
// the same server — that would prove nothing). The "latest release" link is not used: on
// 2026-10-22 it already served FFmpeg 9, which the loader does not support. The GitHub
// mirror serves a byte-identical file (checked 2026-10-24), so one hash covers both URLs.
// `src/scripts/fetch-ffmpeg-win.sh` (the Windows tests) uses the same pin — keep them equal.

/// The FFmpeg release the Download button installs.
pub const PINNED_VERSION: &str = kt_dirs::FFMPEG_DOWNLOAD_VERSION;
pub const PINNED_ARCHIVE: &str = "ffmpeg-8.1.2-full_build-shared.7z";
pub const PINNED_SHA256: &str = "cba748035c21ce1431d0823c7a3a711f38616f89f87a265dceddf9b7f6749d2d";
pub const PINNED_URLS: [&str; 2] = [
    "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.2-full_build-shared.7z",
    "https://github.com/GyanD/codexffmpeg/releases/download/8.1.2/ffmpeg-8.1.2-full_build-shared.7z",
];
/// Size shown before the download (bytes of the pinned archive).
pub const PINNED_SIZE: u64 = 59_459_100;
/// A guard against a server that never stops sending.
const MAX_ARCHIVE_BYTES: u64 = 200 * 1024 * 1024;
/// A temporary entry of another process untouched this long is a leftover of a killed one.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Where downloads live: `<local>/ffmpeg` (Windows: `%LOCALAPPDATA%\kinetren\ffmpeg`).
pub fn download_root() -> PathBuf {
    kt_dirs::local_dir().join("ffmpeg")
}

/// The folder of the installed pinned build — the fourth step of the Windows search.
pub fn installed_dir() -> PathBuf {
    kt_dirs::ffmpeg_download_dir()
}

/// What to download: the pinned archive, or (debug builds only) a test server's —
/// `KT_FFMPEG_DOWNLOAD_URL` + `KT_FFMPEG_DOWNLOAD_SHA256`, the pattern of `KT_TELEMETRY_URL`.
#[derive(Debug, Clone)]
pub struct DownloadSource {
    pub urls: Vec<String>,
    pub sha256: String,
    pub version: String,
}

impl DownloadSource {
    pub fn pinned() -> Self {
        if cfg!(debug_assertions)
            && let (Ok(url), Ok(sha)) =
                (std::env::var("KT_FFMPEG_DOWNLOAD_URL"), std::env::var("KT_FFMPEG_DOWNLOAD_SHA256"))
        {
            return DownloadSource { urls: vec![url], sha256: sha, version: PINNED_VERSION.into() };
        }
        DownloadSource {
            urls: PINNED_URLS.iter().map(|s| s.to_string()).collect(),
            sha256: PINNED_SHA256.into(),
            version: PINNED_VERSION.into(),
        }
    }
}

/// A step of the download for the window.
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStep {
    Connecting(String),
    Bytes { done: u64, total: Option<u64> },
    Verifying,
    Extracting,
}

/// Download, verify and install into `root/<version>`; the folder appears only whole.
/// Cancellation (`cancel`), a broken connection and a hash mismatch leave `root` without
/// traces of this attempt.
pub fn download_and_install(
    src: &DownloadSource,
    root: &std::path::Path,
    cancel: &dyn Fn() -> bool,
    step: &dyn Fn(DownloadStep),
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(root).map_err(|e| format!("{}: {e}", root.display()))?;
    remove_stale(root);
    let pid = std::process::id();
    let archive = root.join(format!(".download-{pid}.7z"));
    let partial = root.join(format!(".partial-{pid}"));
    let result = (|| {
        fetch_verified(src, &archive, cancel, step)?;
        step(DownloadStep::Extracting);
        extract_dlls(&archive, &partial, cancel)?;
        let dest = root.join(&src.version);
        if dest.exists() {
            std::fs::remove_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        }
        std::fs::rename(&partial, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        Ok(dest)
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
    step: &dyn Fn(DownloadStep),
) -> Result<(), String> {
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
    let mut errors = Vec::new();
    for url in &src.urls {
        step(DownloadStep::Connecting(url.clone()));
        let agent = agent(url);
        let attempt = (|| -> Result<(), String> {
            let resp = agent.get(url).call().map_err(|e| e.to_string())?;
            let total = resp
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
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
                step(DownloadStep::Bytes { done, total });
            }
            file.sync_all().map_err(|e| e.to_string())?;
            step(DownloadStep::Verifying);
            let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
            if !got.eq_ignore_ascii_case(&src.sha256) {
                return Err(format!("checksum mismatch: expected {}, got {got}", src.sha256));
            }
            Ok(())
        })();
        match attempt {
            Ok(()) => return Ok(()),
            Err(e) if e == "cancelled" => {
                let _ = std::fs::remove_file(out);
                return Err(e);
            }
            Err(e) => {
                let _ = std::fs::remove_file(out);
                errors.push(format!("{url}: {e}"));
            }
        }
    }
    Err(errors.join("\n"))
}

/// Only `<root>/bin/*.dll` and `<root>/LICENSE*` of the archive, flat — the layout the
/// loader expects in a folder.
fn extract_dlls(archive: &std::path::Path, dest: &std::path::Path, cancel: &dyn Fn() -> bool) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dest);
    std::fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut reader = sevenz_rust2::ArchiveReader::new(file, sevenz_rust2::Password::empty())
        .map_err(|e| format!("not a 7z archive: {e}"))?;
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
            let wanted = match parts.as_slice() {
                [_, "bin", f] => f.to_ascii_lowercase().ends_with(".dll"),
                [_, f] => f.to_ascii_uppercase().starts_with("LICENSE"),
                _ => false,
            };
            if entry.is_directory() || !wanted {
                // the entry's data must still be consumed for the solid stream to go on
                std::io::copy(data, &mut std::io::sink())?;
                return Ok(true);
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            data.read_to_end(&mut bytes)?;
            std::fs::write(dest.join(parts[parts.len() - 1]), &bytes)?;
            count += 1;
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
    pub job: Option<crate::bg::BgJob<DownloadStep, Result<PathBuf, String>>>,
    /// The outcome of the last action: (is an error, text).
    pub message: Option<(bool, String)>,
    /// A test's answer of the folder picker (the native dialog cannot be driven).
    pub test_pick: Option<PathBuf>,
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
        }
    }
}

impl FfmpegUi {
    fn read_choice(&self) -> Option<String> {
        let text = std::fs::read_to_string(&self.choice_file).ok()?;
        let line = text.lines().next()?.trim();
        (!line.is_empty()).then(|| line.to_string())
    }

    /// Search again with the current folders; the message says what happened.
    pub fn rescan(&mut self) {
        let download = cfg!(windows)
            .then(|| self.download_root.join(PINNED_VERSION).to_string_lossy().into_owned());
        kt_ffmpeg::set_search_dirs(self.read_choice(), download);
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
        let src = DownloadSource::pinned();
        self.message = None;
        self.job = Some(crate::bg::BgJob::spawn("ffmpeg-download", move |ctl, send| {
            download_and_install(&src, &root, &|| ctl.should_stop(), &|st| send.send(st))
        }));
    }

    /// Collect the job; `true` — something changed (a reason to repaint).
    pub fn poll(&mut self) -> bool {
        let Some(job) = self.job.as_mut() else { return false };
        let changed = job.poll();
        if let Some(done) = job.done.take() {
            self.job = None;
            match done {
                Ok(_) => self.rescan(),
                Err(e) if e == "cancelled" => self.message = None,
                Err(e) => self.message = Some((true, crate::tf!(k::ffmpeg::FAILED, e))),
            }
            return true;
        }
        changed
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
        egui::Window::new(tr(k::ffmpeg::TITLE))
            .id(egui::Id::new("kt_ffmpeg"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
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
                    if cfg!(windows) && info.is_none() {
                        ui.add_space(4.0);
                        ui.weak(crate::tf!(k::ffmpeg::DOWNLOAD_NOTE, PINNED_SIZE / (1024 * 1024)));
                    }
                });
                crate::widgets::window_actions(ui, |ui| {
                    if let Some(job) = &self.ffmpeg_ui.job {
                        let (frac, text) = match &job.progress {
                            Some(DownloadStep::Bytes { done, total }) => {
                                let t = total.unwrap_or(PINNED_SIZE).max(1);
                                (*done as f32 / t as f32, crate::tf!(k::ffmpeg::PROGRESS, done / (1024 * 1024), t / (1024 * 1024)))
                            }
                            Some(DownloadStep::Verifying) => (1.0, tr(k::ffmpeg::VERIFYING).to_string()),
                            Some(DownloadStep::Extracting) => (1.0, tr(k::ffmpeg::EXTRACTING).to_string()),
                            _ => (0.0, tr(k::ffmpeg::CONNECTING).to_string()),
                        };
                        ui.add(egui::ProgressBar::new(frac).text(text).desired_width(ui.available_width()));
                    }
                    ui.horizontal(|ui| {
                        if self.ffmpeg_ui.job.is_some() {
                            if ui.button(tr(k::ffmpeg::CANCEL)).clicked() {
                                act = Some("cancel");
                            }
                        } else if cfg!(windows)
                            && info.is_none()
                            && ui.button(crate::tf!(k::ffmpeg::DOWNLOAD, PINNED_VERSION)).clicked()
                        {
                            act = Some("download");
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
            _ => {}
        }
        if act.is_some() {
            // a late load: the placeholders reopen on the next frames (poll_returned_sources)
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A tiny archive with the structure of gyan.dev's: `<root>/bin/*.dll` (avcodec among
    /// them), `<root>/bin/ffmpeg.exe`, `<root>/include/…`, `<root>/LICENSE`.
    fn make_archive(path: &std::path::Path) -> Vec<u8> {
        let mut w = sevenz_rust2::ArchiveWriter::create(path).unwrap();
        let files: [(&str, &[u8]); 6] = [
            ("ffmpeg-8.1.2-full_build-shared/bin/avcodec-62.dll", b"codec"),
            ("ffmpeg-8.1.2-full_build-shared/bin/avutil-60.dll", b"util"),
            ("ffmpeg-8.1.2-full_build-shared/bin/ffmpeg.exe", b"exe"),
            ("ffmpeg-8.1.2-full_build-shared/include/libavcodec/avcodec.h", b"header"),
            ("ffmpeg-8.1.2-full_build-shared/LICENSE", b"GPLv3"),
            ("ffmpeg-8.1.2-full_build-shared/README.txt", b"readme"),
        ];
        for (name, data) in files {
            w.push_archive_entry(sevenz_rust2::ArchiveEntry::new_file(name), Some(data)).unwrap();
        }
        w.finish().unwrap();
        std::fs::read(path).unwrap()
    }

    /// A local HTTP server for one file. `cut` > 0: send only that many bytes, then hang
    /// up; `slow`: send in small pieces with pauses (so a cancel lands mid-way).
    fn serve(body: Vec<u8>, cut: usize, slow: bool) -> String {
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

    fn sha(bytes: &[u8]) -> String {
        use sha2::Digest;
        sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn entries(root: &std::path::Path) -> Vec<String> {
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
        let quiet = |_: DownloadStep| {};
        let src = |url: String, sha256: String| DownloadSource { urls: vec![url], sha256, version: "8.1.2".into() };

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
        .expect("the right hash installs");
        assert_eq!(dest, root.join("8.1.2"));
        assert_eq!(entries(&root), vec!["8.1.2".to_string()], "no temporary entries");
        assert_eq!(entries(&dest), vec!["LICENSE", "avcodec-62.dll", "avutil-60.dll"]);
        assert_eq!(std::fs::read(dest.join("avcodec-62.dll")).unwrap(), b"codec");
        let steps = steps.into_inner().unwrap();
        assert!(steps.contains(&DownloadStep::Verifying) && steps.contains(&DownloadStep::Extracting));
        assert!(steps.iter().any(|s| matches!(s, DownloadStep::Bytes { total: Some(n), .. } if *n == bytes.len() as u64)));

        // cancel mid-way and a broken connection: the installed folder stays, nothing else
        let flag = AtomicBool::new(false);
        let url = serve(bytes.clone(), 0, true);
        let err = download_and_install(&src(url, sha(&bytes)), &root, &|| flag.load(Ordering::Relaxed), &|s| {
            if matches!(s, DownloadStep::Bytes { done, .. } if done > 128) {
                flag.store(true, Ordering::Relaxed);
            }
        })
        .expect_err("cancelled");
        assert_eq!(err, "cancelled");
        assert_eq!(entries(&root), vec!["8.1.2".to_string()]);

        let err = download_and_install(&src(serve(bytes.clone(), bytes.len() / 2, false), sha(&bytes)), &root, &never, &quiet)
            .expect_err("a broken connection fails");
        assert!(!err.is_empty());
        assert_eq!(entries(&root), vec!["8.1.2".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// windows-build task 5.2: the real pinned archive (cached by `scripts/fetch-ffmpeg-win.sh`)
    /// extracts to the same DLL bytes as 7-Zip. Skipped without the cache.
    #[test]
    fn pinned_archive_extracts_like_7zip() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ffmpeg-win");
        let archive = base.join(PINNED_ARCHIVE);
        let by_7z = base.join(PINNED_VERSION);
        if !archive.exists() || !by_7z.join("avcodec-62.dll").exists() {
            eprintln!("skipped: no cached pinned archive (scripts/fetch-ffmpeg-win.sh)");
            return;
        }
        let out = std::env::temp_dir().join(format!("kt-pinned-{}", std::process::id()));
        extract_dlls(&archive, &out, &|| false).expect("extracts");
        assert_eq!(entries(&out), entries(&by_7z));
        for f in entries(&out) {
            assert!(std::fs::read(out.join(&f)).unwrap() == std::fs::read(by_7z.join(&f)).unwrap(), "{f} differs");
        }
        let _ = std::fs::remove_dir_all(&out);
    }
}
