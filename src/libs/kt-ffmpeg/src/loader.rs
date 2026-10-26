//! Runtime loading of libav*: walks the plan of `search.rs`, opens the first candidate that
//! loads, and checks the version with `avcodec_version()`.
//!
//! Linux: by SONAME (`ld.so` pulls the dependencies of the same major by NEEDED, so 6 and
//! 7 cannot mix), or by full paths from `KINETREN_FFMPEG_DIR`. Windows (windows-build,
//! design D2): by full paths from ONE folder, in `search::LOAD_ORDER`, with
//! `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS` (no PATH, no current
//! folder); afterwards every module must lie in that folder, or the set is rejected as mixed.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::FfError;
use crate::search::{self, Candidate, Env, LOAD_ORDER, Lib as LibId, Os, Reject, Report, ReportLine};
use libloading::{Library, Symbol};

/// Libraries opened by this process — t847 proves a remembered success opens nothing again.
pub(crate) static LIB_OPENS: AtomicU64 = AtomicU64::new(0);

pub struct Lib(Library, String);

impl Lib {
    /// Open by SONAME (Linux system search).
    #[cfg(unix)]
    fn open_name(name: &str) -> Result<Lib, String> {
        use libloading::os::unix::{Library as UnixLibrary, RTLD_LAZY, RTLD_LOCAL};
        LIB_OPENS.fetch_add(1, Ordering::Relaxed);
        // RTLD_LOCAL: we do not pollute the global namespace — symbols are taken
        // by address from our own handles.
        unsafe { UnixLibrary::open(Some(name), RTLD_LAZY | RTLD_LOCAL) }
            .map(|l| Lib(l.into(), name.to_string()))
            // dlerror() already names the file
            .map_err(|e| e.to_string())
    }

    /// Open by full path from a chosen folder.
    #[cfg(unix)]
    fn open_path(path: &str, _dir: &str) -> Result<Lib, String> {
        Self::open_name(path)
    }

    #[cfg(windows)]
    fn open_path(path: &str, dir: &str) -> Result<Lib, String> {
        use libloading::os::windows::{
            LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, Library as WinLibrary,
        };
        LIB_OPENS.fetch_add(1, Ordering::Relaxed);
        let lib = unsafe {
            WinLibrary::load_with_flags(
                path,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
        }
        .map_err(|e| format!("{path}: {e}"))?;
        // The module must be the file we asked for: a namesake already loaded by someone
        // else in the process would be returned instead, silently.
        let raw = lib.into_raw();
        let actual = module_path(raw);
        let lib = unsafe { WinLibrary::from_raw(raw) };
        match actual {
            Some(p) if in_dir(&p, dir) => Ok(Lib(lib.into(), p)),
            Some(p) => Err(format!("mixed set: {path} resolved to {p}")),
            None => Err(format!("{path}: module path unknown")),
        }
    }

    /// # Safety
    /// The type `T` must match the real signature of the function.
    pub unsafe fn sym<T: Copy>(&self, name: &str) -> Result<T, FfError> {
        let sym: Symbol<T> = unsafe { self.0.get(name.as_bytes()) }
            .map_err(|e| FfError::Load(format!("symbol {}: {e}", name.trim_end_matches('\0'))))?;
        Ok(*sym)
    }
}

#[cfg(windows)]
fn module_path(h: isize) -> Option<String> {
    use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
    let mut buf = vec![0u16; 32768];
    let n = unsafe { GetModuleFileNameW(h as _, buf.as_mut_ptr(), buf.len() as u32) } as usize;
    (n > 0).then(|| String::from_utf16_lossy(&buf[..n]))
}

#[cfg(windows)]
fn in_dir(file: &str, dir: &str) -> bool {
    let norm = |s: &str| {
        let s = s.strip_prefix(r"\\?\").unwrap_or(s);
        s.replace('/', "\\").trim_end_matches('\\').to_lowercase()
    };
    let (f, d) = (norm(file), norm(dir));
    f.rsplit_once('\\').map(|(parent, _)| parent == d).unwrap_or(false)
}

/// Five loaded libraries of one major version.
pub struct Libs {
    pub avutil: Lib,
    pub avcodec: Lib,
    pub avformat: Lib,
    pub swscale: Lib,
    pub swresample: Lib,
    pub major: u32,
    /// Full avcodec version (major.minor.micro) for diagnostics.
    pub avcodec_version: (u32, u32, u32),
    /// Where they came from (folder or "system libraries"), for the report.
    pub place: String,
}

impl Libs {
    /// Path (Windows: as the loader resolved it) or SONAME of each module, in load order.
    pub fn module_paths(&self) -> Vec<String> {
        [&self.avutil, &self.swresample, &self.swscale, &self.avcodec, &self.avformat]
            .iter()
            .map(|l| l.1.clone())
            .collect()
    }
}

/// Walk the plan; the first candidate that opens wins. The report lists every rejection.
pub fn load(env: &dyn Env) -> (Result<Libs, FfError>, Report) {
    let mut report = Report::default();
    for cand in search::plan(Os::NATIVE, env) {
        let place = place_of(&cand);
        let reject = match &cand.verdict {
            Err(r) => Some(r.clone()),
            Ok(_) => match open(&cand) {
                Ok(libs) => {
                    report.loaded = Some((place.clone(), libs.major, libs.avcodec_version));
                    report.lines.push(ReportLine { source: cand.source, place, reject: None });
                    return (Ok(libs), report);
                }
                Err(e) => Some(Reject::Load(e)),
            },
        };
        report.lines.push(ReportLine { source: cand.source, place, reject });
    }
    let text = format!("{report}{}", search::not_found_hint(Os::NATIVE));
    (Err(FfError::NotFound(text)), report)
}

fn open(c: &Candidate) -> Result<Libs, String> {
    let ffmpeg = *c.verdict.as_ref().expect("an accepted candidate");
    let mut opened: Vec<Lib> = Vec::with_capacity(5);
    for id in LOAD_ORDER {
        let name = search::file_name(Os::NATIVE, id, ffmpeg);
        let lib = match &c.dir {
            Some(dir) => Lib::open_path(&search::join(Os::NATIVE, dir, &name), dir)?,
            #[cfg(unix)]
            None => Lib::open_name(&name)?,
            #[cfg(windows)]
            None => return Err("no system FFmpeg on Windows".into()),
        };
        opened.push(lib);
    }
    let mut it = opened.into_iter();
    let (avutil, swresample, swscale, avcodec, avformat) = (
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
    );
    debug_assert_eq!(LOAD_ORDER[3], LibId::Avcodec);
    // the library major must match the set of types we are about to use
    let version: unsafe extern "C" fn() -> u32 =
        unsafe { avcodec.sym("avcodec_version\0") }.map_err(|e| e.to_string())?;
    let v = unsafe { version() };
    let (maj, min, mic) = ((v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
    let expected = search::avcodec_major(ffmpeg).expect("supported");
    if maj != expected {
        return Err(format!("avcodec_version() = {maj}.{min}.{mic}, expected major {expected}"));
    }
    Ok(Libs {
        avutil,
        avcodec,
        avformat,
        swscale,
        swresample,
        major: ffmpeg,
        avcodec_version: (maj, min, mic),
        place: place_of(c),
    })
}

/// How a candidate is named in the report: its folder, or the FFmpeg release asked of the
/// system by SONAME.
fn place_of(c: &Candidate) -> String {
    match (&c.dir, &c.verdict) {
        (Some(d), _) => d.clone(),
        (None, Ok(f)) => format!("system {}", search::file_name(Os::NATIVE, LibId::Avcodec, *f)),
        (None, Err(_)) => "system".into(),
    }
}

/// The machine as the plan sees it.
pub struct SystemEnv {
    pub user_choice: Option<String>,
    /// The root of Kinetren's downloads (one folder per version).
    pub download_root: Option<String>,
}

impl Env for SystemEnv {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
    fn exe_dir(&self) -> Option<String> {
        let exe = std::env::current_exe().ok()?;
        Some(exe.parent()?.to_string_lossy().into_owned())
    }
    fn user_choice(&self) -> Option<String> {
        self.user_choice.clone()
    }
    fn download_root(&self) -> Option<String> {
        self.download_root.clone()
    }
    fn list(&self, dir: &str) -> Option<Vec<String>> {
        let rd = std::fs::read_dir(dir).ok()?;
        Some(rd.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect())
    }
    /// `KINETREN_FFMPEG_SYSTEM=0` switches the Linux fallback to the system libraries off:
    /// a diagnostic for "which folder would be used", and the way tests get "no FFmpeg".
    fn system_search(&self) -> bool {
        std::env::var("KINETREN_FFMPEG_SYSTEM").map(|v| v != "0").unwrap_or(true)
    }
    fn read_link(&self, path: &str) -> Option<String> {
        let t = std::fs::read_link(path).ok()?;
        let t = if t.is_absolute() {
            t
        } else {
            std::path::Path::new(path).parent()?.join(t)
        };
        Some(t.to_string_lossy().into_owned())
    }
}
