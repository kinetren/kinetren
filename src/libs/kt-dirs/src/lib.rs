//! Per-user directories of Kinetren (change windows-build, design D4; capability
//! `durability`). See `CLAUDE.md` of the crate for the rules.

use std::ffi::OsString;
use std::path::PathBuf;

/// Name of the per-user directory.
pub const APP_DIR: &str = "kinetren";

/// eframe's settings file inside the data directory — the name eframe uses itself.
pub const EFRAME_SETTINGS_FILE: &str = "app.ron";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Windows,
}

impl Os {
    pub const NATIVE: Os = if cfg!(windows) { Os::Windows } else { Os::Linux };
}

/// Windows known folders the resolver may ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownFolder {
    RoamingAppData,
    LocalAppData,
}

/// What the resolver may ask of the machine.
pub trait Env {
    fn os(&self) -> Os;
    fn var_os(&self, name: &str) -> Option<OsString>;
    fn known_folder(&self, f: KnownFolder) -> Option<PathBuf>;
    fn temp_dir(&self) -> PathBuf;
}

/// The resolved directories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
    /// Recovery, journal, crash report, locks, presets, workspace, eframe's `app.ron`.
    pub data: PathBuf,
    /// Large regenerable downloads (Windows: Local, not Roaming). Equal to `data` on Linux.
    pub local: PathBuf,
    /// No per-user root was found: both live in the temp directory, and the caller says so.
    pub fallback: bool,
}

/// Resolve the directories. Pure: nothing is created.
///
/// `KINETREN_DATA_DIR` (absolute) replaces the data AND local directory on every OS: what
/// `XDG_DATA_HOME` gives tests and live runs on Linux, it gives them on Windows too (the
/// known folders cannot be redirected per process).
pub fn resolve(env: &dyn Env) -> Dirs {
    if let Some(d) = env.var_os("KINETREN_DATA_DIR").map(PathBuf::from).filter(|p| p.is_absolute()) {
        return Dirs { data: d.clone(), local: d, fallback: false };
    }
    match env.os() {
        Os::Linux => {
            // "absolute" by the rules of the OS planned FOR, not of the host (the Linux rules
            // are tested under wine too)
            let posix_abs = |p: &PathBuf| p.to_string_lossy().starts_with('/');
            let base = env
                .var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .filter(posix_abs)
                .or_else(|| {
                    env.var_os("HOME").map(PathBuf::from).filter(posix_abs).map(|h| h.join(".local/share"))
                });
            match base {
                Some(b) => {
                    let d = b.join(APP_DIR);
                    Dirs { data: d.clone(), local: d, fallback: false }
                }
                None => {
                    let d = env.temp_dir().join(APP_DIR);
                    Dirs { data: d.clone(), local: d, fallback: true }
                }
            }
        }
        Os::Windows => {
            let roaming = env.known_folder(KnownFolder::RoamingAppData);
            let local = env.known_folder(KnownFolder::LocalAppData);
            match roaming {
                Some(r) => Dirs {
                    data: r.join(APP_DIR).join("data"),
                    local: local.unwrap_or(r).join(APP_DIR),
                    fallback: false,
                },
                None => {
                    let t = env.temp_dir().join(APP_DIR);
                    Dirs { data: t.join("data"), local: t, fallback: true }
                }
            }
        }
    }
}

/// The machine as the resolver sees it.
pub struct SystemEnv;

impl Env for SystemEnv {
    fn os(&self) -> Os {
        Os::NATIVE
    }
    fn var_os(&self, name: &str) -> Option<OsString> {
        std::env::var_os(name)
    }
    fn known_folder(&self, f: KnownFolder) -> Option<PathBuf> {
        known_folder(f)
    }
    fn temp_dir(&self) -> PathBuf {
        std::env::temp_dir()
    }
}

#[cfg(windows)]
fn known_folder(f: KnownFolder) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{
        FOLDERID_LocalAppData, FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    };
    let id = match f {
        KnownFolder::RoamingAppData => &FOLDERID_RoamingAppData,
        KnownFolder::LocalAppData => &FOLDERID_LocalAppData,
    };
    let mut p: windows_sys::core::PWSTR = std::ptr::null_mut();
    let hr = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT as _, std::ptr::null_mut(), &mut p) };
    let out = if hr == 0 && !p.is_null() {
        let len = (0..).take_while(|&i| unsafe { *p.add(i) } != 0).count();
        let wide = unsafe { std::slice::from_raw_parts(p, len) };
        Some(PathBuf::from(OsString::from_wide(wide)))
    } else {
        None
    };
    unsafe { CoTaskMemFree(p as _) };
    out
}

#[cfg(not(windows))]
fn known_folder(_: KnownFolder) -> Option<PathBuf> {
    None
}

fn system() -> &'static Dirs {
    static DIRS: std::sync::OnceLock<Dirs> = std::sync::OnceLock::new();
    DIRS.get_or_init(|| resolve(&SystemEnv))
}

/// The data directory of this machine (resolved once per process).
pub fn data_dir() -> PathBuf {
    system().data.clone()
}

/// The sandbox the program runs in (change flatpak-flathub, D8): the ONE seam that every
/// sandbox-dependent behaviour reads (the update check, the FFmpeg hint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sandbox {
    None,
    /// Flatpak: `FLATPAK_ID` is set or `/.flatpak-info` exists.
    Flatpak,
}

/// Pure: the sandbox of a machine as `env` shows it, with whether `/.flatpak-info` exists.
pub fn sandbox_of(env: &dyn Env, flatpak_info: bool) -> Sandbox {
    if flatpak_info || env.var_os("FLATPAK_ID").is_some_and(|v| !v.is_empty()) {
        Sandbox::Flatpak
    } else {
        Sandbox::None
    }
}

/// The sandbox of this process (asked once).
pub fn sandbox() -> Sandbox {
    static SANDBOX: std::sync::OnceLock<Sandbox> = std::sync::OnceLock::new();
    *SANDBOX.get_or_init(|| sandbox_of(&SystemEnv, std::path::Path::new("/.flatpak-info").exists()))
}

/// The local directory of this machine (downloads).
pub fn local_dir() -> PathBuf {
    system().local.clone()
}

/// The FFmpeg release the program's Download button installs (Windows). kt-app downloads it
/// (`ffmpeg_setup.rs`, with the pinned hash); kt-app and kt-cli both search its folder.
pub const FFMPEG_DOWNLOAD_VERSION: &str = "9.0.2";

/// Where the downloaded FFmpeg lives: `<local>/ffmpeg/<version>`.
pub fn ffmpeg_download_dir() -> PathBuf {
    ffmpeg_download_root().join(FFMPEG_DOWNLOAD_VERSION)
}

/// The root of all downloads, `<local>/ffmpeg`: the search looks at every version folder in
/// it, newest first, so a folder an earlier pin left keeps working (ffmpeg-9).
pub fn ffmpeg_download_root() -> PathBuf {
    local_dir().join("ffmpeg")
}

/// No per-user root was found; the program should say so once.
pub fn is_fallback() -> bool {
    system().fallback
}

/// A Windows path without the verbatim prefix `\\?\` that `std::fs::canonicalize` adds:
/// `\\?\C:\x` → `C:\x`, `\\?\UNC\h\s\x` → `\\h\s\x` (the `dunce` rule). The prefix is kept
/// when the plain spelling would mean something else or not open: longer than `MAX_PATH`, or not
/// a drive / UNC path (`\\?\Volume{…}`). Linux paths come back as they are. Pure over `os`, so
/// the Windows rule is tested on Linux (t1062).
pub fn plain_path(s: &str, os: Os) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    if os == Os::Linux {
        return Cow::Borrowed(s);
    }
    const MAX_PATH: usize = 260;
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        let plain = format!(r"\\{rest}");
        if plain.len() < MAX_PATH {
            return Cow::Owned(plain);
        }
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\';
        if drive && rest.len() < MAX_PATH {
            return Cow::Borrowed(rest);
        }
    }
    Cow::Borrowed(s)
}

/// `std::fs::canonicalize` as a person reads the path: the verbatim prefix removed where the
/// plain spelling names the same file (`plain_path`). A path shown in the interface or used as a
/// key next to `std::path::absolute` spellings goes through this, never through the bare call:
/// on Windows the bare one gave `\\?\C:\…` and a welcome card named a folder nobody had typed
/// (t986 on windows-latest, change ci-green-again).
pub fn canonical(p: &std::path::Path) -> std::io::Result<PathBuf> {
    let c = std::fs::canonicalize(p)?;
    // a path that is not UTF-8 is left as the system gave it: a lossy copy would name another file
    let plain = c.to_str().map(|s| plain_path(s, Os::NATIVE).into_owned());
    Ok(plain.map(PathBuf::from).unwrap_or(c))
}

/// Project locks (capability `durability`): the lock of a project is a file in
/// `<data>/locks` named after the project's absolute path, shared by kt-app (which takes it)
/// and kt-cli (which reports its holder).
pub mod locks {
    use super::Os;
    use std::path::{Path, PathBuf};

    /// The string a lock name is computed from. Linux: the absolute path as is (the names
    /// of existing locks stay the same). Windows: one file has many spellings —
    /// `C:\Proj\a.kt`, `c:\proj\A.kt`, `\\?\C:\Proj\a.kt`, `C:/Proj/a.kt` — so the verbatim
    /// prefix is removed, separators unified and case folded (t852).
    pub fn key(abs: &str, os: Os) -> String {
        match os {
            Os::Linux => abs.to_string(),
            Os::Windows => {
                let s = abs.replace('/', "\\");
                let s = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
                    format!(r"\\{rest}")
                } else if let Some(rest) = s.strip_prefix(r"\\?\") {
                    rest.to_string()
                } else {
                    s
                };
                s.to_lowercase()
            }
        }
    }

    /// File name stem of the lock (`<stem>.lock`, `<stem>.pid`).
    pub fn stem(abs: &str, os: Os) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in key(abs, os).as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        format!("{h:016x}")
    }

    /// `<data>/locks/<stem>.lock` for a project path (made absolute, not canonical: the
    /// lock is needed for "Save as…" to a file that does not exist yet).
    pub fn lock_file(data_dir: &Path, project: &Path) -> PathBuf {
        let abs = std::path::absolute(project).unwrap_or_else(|_| project.to_path_buf());
        data_dir.join("locks").join(format!("{}.lock", stem(&abs.to_string_lossy(), Os::NATIVE)))
    }

    /// The holder's pid written next to the lock: `<stem>.pid`. A Windows lock is
    /// mandatory — the locked file cannot be read by the second copy — so the pid lives in a
    /// file of its own. The kernel lock is what holds; this file is only information.
    pub fn pid_file(lock_file: &Path) -> PathBuf {
        lock_file.with_extension("pid")
    }

    /// The pid of the holder: from the pid file, else from the first line of the lock file
    /// (locks written before windows-build, and readable on Linux).
    pub fn read_holder(lock_file: &Path) -> Option<u32> {
        let parse = |p: &Path| -> Option<u32> {
            std::fs::read_to_string(p).ok()?.lines().next()?.trim().parse().ok()
        };
        parse(&pid_file(lock_file)).or_else(|| parse(lock_file))
    }
}
