//! Where the FFmpeg libraries are looked for — a PURE plan (change windows-build, design D2).
//!
//! Linux has one system FFmpeg, and `ld.so` finds it by SONAME. Windows has none: the
//! program looks for a FOLDER holding the five DLLs of one major, in a fixed order, and
//! names every folder it rejected and why. The plan reads the machine only through
//! [`Env`], works on strings with the path rules of the OS it plans FOR, and so runs on
//! any OS in tests (t844, t845): the Windows plan is checked on Linux CI and the other way
//! round.
//!
//! Opening the libraries is not here — see `loader.rs`.

use std::fmt;

/// The OS whose rules the plan follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Windows,
}

impl Os {
    /// The OS this binary runs on.
    pub const NATIVE: Os = if cfg!(windows) { Os::Windows } else { Os::Linux };
}

/// The five libraries, in the order they are LOADED: every library after the first imports
/// only libraries before it, so each import binds to a module already loaded from the
/// chosen folder, never to a namesake found elsewhere (Windows matches imports by base
/// name against loaded modules; `ld.so` matches NEEDED by SONAME).
pub const LOAD_ORDER: [Lib; 5] =
    [Lib::Avutil, Lib::Swresample, Lib::Swscale, Lib::Avcodec, Lib::Avformat];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lib {
    Avutil,
    Swresample,
    Swscale,
    Avcodec,
    Avformat,
}

impl Lib {
    fn stem(self) -> &'static str {
        match self {
            Lib::Avutil => "avutil",
            Lib::Swresample => "swresample",
            Lib::Swscale => "swscale",
            Lib::Avcodec => "avcodec",
            Lib::Avformat => "avformat",
        }
    }
}

/// Library majors of one FFmpeg release, newest first (order = priority inside a folder and
/// on Linux). Every row has a version module in `lib.rs` built from the vendored headers of
/// that release; t862 checks each row against the headers' `LIB*_VERSION_MAJOR`.
pub const MAJORS: [(u32, Majors); 4] = [
    (9, Majors { avutil: 61, swresample: 7, swscale: 10, avcodec: 63, avformat: 63 }),
    (8, Majors { avutil: 60, swresample: 6, swscale: 9, avcodec: 62, avformat: 62 }),
    (7, Majors { avutil: 59, swresample: 5, swscale: 8, avcodec: 61, avformat: 61 }),
    (6, Majors { avutil: 58, swresample: 4, swscale: 7, avcodec: 60, avformat: 60 }),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Majors {
    pub avutil: u32,
    pub swresample: u32,
    pub swscale: u32,
    pub avcodec: u32,
    pub avformat: u32,
}

impl Majors {
    fn of(&self, lib: Lib) -> u32 {
        match lib {
            Lib::Avutil => self.avutil,
            Lib::Swresample => self.swresample,
            Lib::Swscale => self.swscale,
            Lib::Avcodec => self.avcodec,
            Lib::Avformat => self.avformat,
        }
    }
}

/// The avcodec library major expected for an FFmpeg release.
pub fn avcodec_major(ffmpeg: u32) -> Option<u32> {
    MAJORS.iter().find(|(m, _)| *m == ffmpeg).map(|(_, l)| l.avcodec)
}

/// File name of a library: `avcodec-62.dll` on Windows, `libavcodec.so.62` on Linux.
pub fn file_name(os: Os, lib: Lib, ffmpeg: u32) -> String {
    let m = MAJORS.iter().find(|(f, _)| *f == ffmpeg).expect("supported major").1.of(lib);
    match os {
        Os::Windows => format!("{}-{m}.dll", lib.stem()),
        Os::Linux => format!("lib{}.so.{m}", lib.stem()),
    }
}

/// What the plan may ask of the machine. Paths are strings in the planned OS's form.
pub trait Env {
    fn var(&self, name: &str) -> Option<String>;
    /// Folder of the running executable.
    fn exe_dir(&self) -> Option<String>;
    /// The folder the person chose (Locate…), if any.
    fn user_choice(&self) -> Option<String>;
    /// The ROOT of Kinetren's own downloads (Windows), if any: one folder per downloaded
    /// version (`9.0.2`, `8.1.2`, …) — see [`download_dirs`].
    fn download_root(&self) -> Option<String>;
    /// File names in a folder; `None` if it does not exist or cannot be read.
    fn list(&self, dir: &str) -> Option<Vec<String>>;
    /// Target of a symbolic link, `None` if `path` is not a link.
    fn read_link(&self, path: &str) -> Option<String>;
    /// May the Linux plan fall back to the system libraries by SONAME? (tests switch it off)
    fn system_search(&self) -> bool {
        true
    }
}

/// Where a candidate came from — for the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    UserChoice,
    EnvVar,
    NextToExe,
    Download,
    Path,
    /// The folder a `ffmpeg.exe` link on PATH points to (winget's `Links`).
    PathLink,
    Known,
    /// Linux: the system libraries by SONAME.
    System,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Source::UserChoice => "chosen folder",
            Source::EnvVar => "KINETREN_FFMPEG_DIR",
            Source::NextToExe => "next to the program",
            Source::Download => "Kinetren download",
            Source::Path => "PATH",
            Source::PathLink => "PATH (link)",
            Source::Known => "known location",
            Source::System => "system libraries",
        })
    }
}

/// Why a folder cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reject {
    /// The folder does not exist or cannot be read.
    Unreadable,
    /// No `avcodec-*.dll` / `libavcodec.so.*` at all.
    NoAvcodec,
    /// `ffmpeg.exe` without DLLs — a static build.
    StaticBuild,
    /// Only an avcodec of a major the loader does not know (FFmpeg 10 = avcodec 64).
    Unsupported { avcodec: u32 },
    /// avcodec of a supported release is there, a sibling library is not.
    Missing { ffmpeg: u32, file: String },
    /// Opening failed (set by the loader, not by the plan).
    Load(String),
}

impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reject::Unreadable => f.write_str("folder does not exist or cannot be read"),
            Reject::NoAvcodec => f.write_str("no avcodec-*.dll"),
            Reject::StaticBuild => f.write_str("static build — ffmpeg.exe without DLLs"),
            Reject::Unsupported { avcodec } => match ffmpeg_of_avcodec(*avcodec) {
                Some(n) => write!(f, "FFmpeg {n} (avcodec {avcodec}) is not supported yet"),
                None => write!(f, "avcodec {avcodec} is not supported"),
            },
            Reject::Missing { ffmpeg, file } => write!(f, "FFmpeg {ffmpeg}: no {file}"),
            Reject::Load(e) => write!(f, "could not be loaded: {e}"),
        }
    }
}

/// FFmpeg release of an avcodec major, including releases newer than the table
/// (avcodec 64 = FFmpeg 10, 65 = 11): the report names what the person installed.
fn ffmpeg_of_avcodec(avcodec: u32) -> Option<u32> {
    if let Some((f, _)) = MAJORS.iter().find(|(_, l)| l.avcodec == avcodec) {
        return Some(*f);
    }
    (avcodec >= 63).then(|| avcodec - 54)
}

/// One step of the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub source: Source,
    /// `None` = the system libraries by SONAME (Linux).
    pub dir: Option<String>,
    /// The FFmpeg release to load, or why the folder is rejected.
    pub verdict: Result<u32, Reject>,
}

/// The ordered plan: try the `Ok` candidates in order; the rejected ones are kept for the
/// report.
pub fn plan(os: Os, env: &dyn Env) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut push_dir = |out: &mut Vec<Candidate>, source: Source, dir: String| {
        let key = dir_key(os, &dir);
        if seen.contains(&key) {
            return;
        }
        seen.push(key);
        let verdict = judge(os, env.list(&dir).as_deref());
        out.push(Candidate { source, dir: Some(dir), verdict });
    };

    if let Some(d) = env.user_choice().filter(|d| !d.is_empty()) {
        push_dir(&mut out, Source::UserChoice, d);
    }
    if let Some(d) = env.var("KINETREN_FFMPEG_DIR").filter(|d| !d.is_empty()) {
        push_dir(&mut out, Source::EnvVar, d);
    }

    match os {
        Os::Linux => {
            if env.system_search() {
                for (f, _) in MAJORS {
                    out.push(Candidate { source: Source::System, dir: None, verdict: Ok(f) });
                }
            }
        }
        Os::Windows => {
            if let Some(exe) = env.exe_dir() {
                push_dir(&mut out, Source::NextToExe, join(os, &exe, "ffmpeg"));
                push_dir(&mut out, Source::NextToExe, join(os, &join(os, &exe, "ffmpeg"), "bin"));
            }
            if let Some(root) = env.download_root() {
                for d in download_dirs(os, env, &root) {
                    push_dir(&mut out, Source::Download, d);
                }
            }
            // PATH: only folders that look like FFmpeg — PATH has dozens of unrelated ones,
            // and listing them all as "no avcodec" would bury the one that matters.
            for dir in env.var("PATH").unwrap_or_default().split(';').map(str::trim) {
                if dir.is_empty() {
                    continue;
                }
                let Some(files) = env.list(dir) else { continue };
                let has_avcodec = files.iter().any(|f| avcodec_file_major(os, f).is_some());
                let exe = files.iter().find(|f| f.eq_ignore_ascii_case("ffmpeg.exe"));
                if has_avcodec {
                    push_dir(&mut out, Source::Path, dir.to_string());
                } else if let Some(exe) = exe {
                    // winget puts a LINK to ffmpeg.exe into ...\WinGet\Links; the DLLs are
                    // next to the link's target, not next to the link.
                    match env.read_link(&join(os, dir, exe)) {
                        Some(target) => {
                            // the link's own folder is not reported: "static build" would
                            // be a false reason for a folder of links
                            if let Some(parent) = parent(os, &target) {
                                push_dir(&mut out, Source::PathLink, parent);
                            }
                        }
                        None => push_dir(&mut out, Source::Path, dir.to_string()),
                    }
                }
            }
            for d in known_dirs(os, env) {
                if env.list(&d).is_some() {
                    push_dir(&mut out, Source::Known, d);
                }
            }
        }
    }
    out
}

/// Judge one folder's file list: the FFmpeg release it can load, or why not.
pub fn judge(os: Os, files: Option<&[String]>) -> Result<u32, Reject> {
    let Some(files) = files else { return Err(Reject::Unreadable) };
    let has = |name: &str| match os {
        Os::Windows => files.iter().any(|f| f.eq_ignore_ascii_case(name)),
        Os::Linux => files.iter().any(|f| f == name),
    };
    let mut first_missing: Option<Reject> = None;
    for (ffmpeg, _) in MAJORS {
        if !has(&file_name(os, Lib::Avcodec, ffmpeg)) {
            continue;
        }
        match LOAD_ORDER.iter().map(|l| file_name(os, *l, ffmpeg)).find(|n| !has(n)) {
            None => return Ok(ffmpeg),
            Some(file) => {
                first_missing.get_or_insert(Reject::Missing { ffmpeg, file });
            }
        }
    }
    if let Some(r) = first_missing {
        return Err(r);
    }
    if let Some(avcodec) = files.iter().filter_map(|f| avcodec_file_major(os, f)).max() {
        return Err(Reject::Unsupported { avcodec });
    }
    if files.iter().any(|f| f.eq_ignore_ascii_case("ffmpeg.exe")) {
        return Err(Reject::StaticBuild);
    }
    Err(Reject::NoAvcodec)
}

/// The avcodec major in a file name (`avcodec-63.dll`, `libavcodec.so.63`), if it is one.
fn avcodec_file_major(os: Os, f: &str) -> Option<u32> {
    match os {
        Os::Windows => {
            let lc = f.to_ascii_lowercase();
            lc.strip_prefix("avcodec-")?.strip_suffix(".dll")?.parse().ok()
        }
        Os::Linux => f.strip_prefix("libavcodec.so.")?.parse().ok(),
    }
}

/// The version folders under the download root, newest version first (ffmpeg-9, design D3).
/// The current pin is the newest, so it comes first; a folder an EARLIER pin left (`8.1.2`)
/// follows it — an update of Kinetren does not take away the FFmpeg the person downloaded.
/// Only names that are a dotted version count: the downloader's temporary `.partial-<pid>`
/// entries and anything else are neither candidates nor reported.
pub fn download_dirs(os: Os, env: &dyn Env, root: &str) -> Vec<String> {
    let version = |name: &str| -> Option<Vec<u32>> {
        let parts: Option<Vec<u32>> = name.split('.').map(|p| p.parse().ok()).collect();
        parts.filter(|p| p.len() >= 2)
    };
    let mut found: Vec<(Vec<u32>, String)> = env
        .list(root)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|name| version(&name).map(|v| (v, name)))
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().map(|(_, name)| join(os, root, &name)).collect()
}

/// Known install locations (Windows): scoop, winget packages, the folders tutorials use.
fn known_dirs(os: Os, env: &dyn Env) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(scoop) = env.var("SCOOP") {
        v.push(join(os, &scoop, r"apps\ffmpeg-shared\current\bin"));
    }
    if let Some(home) = env.var("USERPROFILE") {
        v.push(join(os, &home, r"scoop\apps\ffmpeg-shared\current\bin"));
    }
    if let Some(local) = env.var("LOCALAPPDATA") {
        // ...\WinGet\Packages\<Id>_<source hash>\<archive root>\bin
        let pkgs = join(os, &local, r"Microsoft\WinGet\Packages");
        let mut ids: Vec<String> = env
            .list(&pkgs)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| {
                let lc = p.to_ascii_lowercase();
                lc.starts_with("gyan.ffmpeg.shared_")
                    || (lc.starts_with("btbn.ffmpeg.") && lc.contains("shared"))
            })
            .collect();
        ids.sort();
        for id in ids {
            let pkg = join(os, &pkgs, &id);
            let mut roots = env.list(&pkg).unwrap_or_default();
            roots.sort();
            for root in roots {
                v.push(join(os, &join(os, &pkg, &root), "bin"));
            }
        }
    }
    v.push(r"C:\ffmpeg\bin".to_string());
    if let Some(pf) = env.var("ProgramFiles") {
        v.push(join(os, &pf, r"ffmpeg\bin"));
    }
    v
}

fn sep(os: Os) -> char {
    match os {
        Os::Windows => '\\',
        Os::Linux => '/',
    }
}

/// `dir` + `name` with the planned OS's separator.
pub fn join(os: Os, dir: &str, name: &str) -> String {
    let s = sep(os);
    let dir = dir.trim_end_matches(['/', '\\']);
    format!("{dir}{s}{name}")
}

fn parent(os: Os, path: &str) -> Option<String> {
    let i = match os {
        Os::Windows => path.rfind(['\\', '/'])?,
        Os::Linux => path.rfind('/')?,
    };
    Some(path[..i].to_string())
}

/// The same folder written two ways is one candidate (Windows: case and separators).
fn dir_key(os: Os, dir: &str) -> String {
    let d = dir.trim_end_matches(['/', '\\']);
    match os {
        Os::Windows => d.replace('/', "\\").to_lowercase(),
        Os::Linux => d.to_string(),
    }
}

/// The search result for people: what was loaded and what was rejected, in plan order.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub lines: Vec<ReportLine>,
    /// `Some((where, ffmpeg major, avcodec x.y.z))` after a successful load.
    pub loaded: Option<(String, u32, (u32, u32, u32))>,
}

#[derive(Debug, Clone)]
pub struct ReportLine {
    pub source: Source,
    pub place: String,
    pub reject: Option<Reject>,
}

impl Report {
    pub fn rejected(&self) -> impl Iterator<Item = &ReportLine> {
        self.lines.iter().filter(|l| l.reject.is_some())
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.loaded {
            Some((place, ffmpeg, (a, b, c))) => {
                writeln!(f, "loaded: FFmpeg {ffmpeg} (avcodec {a}.{b}.{c}) from {place}")?
            }
            None => writeln!(f, "FFmpeg not loaded")?,
        }
        for l in self.rejected() {
            writeln!(f, "  rejected [{}] {}: {}", l.source, l.place, l.reject.as_ref().unwrap())?;
        }
        Ok(())
    }
}

/// The hint to show when nothing was loaded — per OS (media-index, "the search report").
pub fn not_found_hint(os: Os) -> &'static str {
    match os {
        Os::Linux => {
            "Install FFmpeg 6, 7, 8 or 9 from your distribution (libavcodec, libavformat, \
             libswscale, libswresample)."
        }
        Os::Windows => {
            "Use View → FFmpeg… → Download, or install a SHARED build of FFmpeg 6, 7, 8 or 9 \
             (winget install Gyan.FFmpeg.Shared --version 9.0.2) and point Locate folder… at \
             its bin folder."
        }
    }
}
