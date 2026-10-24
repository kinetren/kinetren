//! Runtime loading of libav*: iteration over SONAMEs from newer majors to older,
//! determining the version via avcodec_version(), and choosing a consistent
//! set of libraries (ld.so itself pulls in the dependencies of the same major
//! version via NEEDED, so mixing 6 and 7 is impossible).

use crate::FfError;
use libloading::{Library, Symbol};

/// SONAME set of one FFmpeg release.
#[cfg(unix)]
struct Soname {
    major: u32,
    avutil: &'static str,
    avcodec: &'static str,
    avformat: &'static str,
    swscale: &'static str,
    swresample: &'static str,
}

/// Candidates — from newer to older (order = priority).
#[cfg(unix)]
const CANDIDATES: &[Soname] = &[
    Soname {
        major: 8,
        avutil: "libavutil.so.60",
        avcodec: "libavcodec.so.62",
        avformat: "libavformat.so.62",
        swscale: "libswscale.so.9",
        swresample: "libswresample.so.6",
    },
    Soname {
        major: 7,
        avutil: "libavutil.so.59",
        avcodec: "libavcodec.so.61",
        avformat: "libavformat.so.61",
        swscale: "libswscale.so.8",
        swresample: "libswresample.so.5",
    },
    Soname {
        major: 6,
        avutil: "libavutil.so.58",
        avcodec: "libavcodec.so.60",
        avformat: "libavformat.so.60",
        swscale: "libswscale.so.7",
        swresample: "libswresample.so.4",
    },
];

pub struct Lib(Library);

impl Lib {
    #[cfg(unix)]
    fn open(name: &str) -> Result<Lib, FfError> {
        use libloading::os::unix::{Library as UnixLibrary, RTLD_LAZY, RTLD_LOCAL};
        // RTLD_LOCAL: we do not pollute the global namespace — symbols are taken
        // by address from our own handles.
        unsafe { UnixLibrary::open(Some(name), RTLD_LAZY | RTLD_LOCAL) }
            .map(|l| Lib(l.into()))
            .map_err(|e| FfError::Load(format!("{name}: {e}")))
    }

    /// # Safety
    /// Type `T` must match the real signature of the function.
    pub unsafe fn sym<T: Copy>(&self, name: &str) -> Result<T, FfError> {
        let sym: Symbol<T> = unsafe { self.0.get(name.as_bytes()) }
            .map_err(|e| FfError::Load(format!("symbol {}: {e}", name.trim_end_matches('\0'))))?;
        Ok(*sym)
    }
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
}

/// Find and load the FFmpeg installed in the system.
#[cfg(windows)]
pub fn load() -> Result<Libs, FfError> {
    // The Windows search (directories, one-folder load) comes with stage 2 of
    // windows-build; until then the build links and reports FFmpeg as absent.
    Err(FfError::NotFound("the Windows FFmpeg search is not implemented yet".into()))
}

/// Find and load the FFmpeg installed in the system.
#[cfg(unix)]
pub fn load() -> Result<Libs, FfError> {
    let mut tried = Vec::new();
    for cand in CANDIDATES {
        match try_candidate(cand) {
            Ok(libs) => return Ok(libs),
            Err(e) => tried.push(format!("  FFmpeg {}: {e}", cand.major)),
        }
    }
    Err(FfError::NotFound(tried.join("\n")))
}

#[cfg(unix)]
fn try_candidate(c: &Soname) -> Result<Libs, FfError> {
    let avcodec = Lib::open(c.avcodec)?;
    // we check that the library’s major matches the expected set of types
    let version: unsafe extern "C" fn() -> u32 = unsafe { avcodec.sym("avcodec_version\0")? };
    let v = unsafe { version() };
    let (maj, min, mic) = ((v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
    let expected_avcodec_major = match c.major {
        8 => 62,
        7 => 61,
        _ => 60,
    };
    if maj != expected_avcodec_major {
        return Err(FfError::Load(format!(
            "{}: avcodec_version() = {maj}.{min}.{mic}, expected major {expected_avcodec_major}",
            c.avcodec
        )));
    }
    Ok(Libs {
        avutil: Lib::open(c.avutil)?,
        avformat: Lib::open(c.avformat)?,
        swscale: Lib::open(c.swscale)?,
        swresample: Lib::open(c.swresample)?,
        avcodec,
        major: c.major,
        avcodec_version: (maj, min, mic),
    })
}
