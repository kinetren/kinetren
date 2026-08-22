//! Рантайм-загрузка libav*: перебор SONAME от новых мажоров к старым,
//! определение версии через avcodec_version() и выбор согласованного
//! набора библиотек (ld.so сам подтягивает зависимости той же мажорной
//! версии по NEEDED, поэтому смешать 6 и 7 невозможно).

use crate::FfError;
use libloading::os::unix::{Library, RTLD_LAZY, RTLD_LOCAL, Symbol};

/// Набор SONAME одного релиза FFmpeg.
struct Soname {
    major: u32,
    avutil: &'static str,
    avcodec: &'static str,
    avformat: &'static str,
    swscale: &'static str,
    swresample: &'static str,
}

/// Кандидаты — от новых к старым (порядок = приоритет).
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
    fn open(name: &str) -> Result<Lib, FfError> {
        // RTLD_LOCAL: не засоряем глобальную область — символы берём
        // адресно из своих хэндлов.
        unsafe { Library::open(Some(name), RTLD_LAZY | RTLD_LOCAL) }
            .map(Lib)
            .map_err(|e| FfError::Load(format!("{name}: {e}")))
    }

    /// # Safety
    /// Тип `T` должен соответствовать реальной сигнатуре функции.
    pub unsafe fn sym<T: Copy>(&self, name: &str) -> Result<T, FfError> {
        let sym: Symbol<T> = unsafe { self.0.get(name.as_bytes()) }
            .map_err(|e| FfError::Load(format!("символ {}: {e}", name.trim_end_matches('\0'))))?;
        Ok(*sym)
    }
}

/// Пять загруженных библиотек одной мажорной версии.
pub struct Libs {
    pub avutil: Lib,
    pub avcodec: Lib,
    pub avformat: Lib,
    pub swscale: Lib,
    pub swresample: Lib,
    pub major: u32,
    /// Полная версия avcodec (major.minor.micro) для диагностики.
    pub avcodec_version: (u32, u32, u32),
}

/// Найти и загрузить установленный в системе FFmpeg.
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

fn try_candidate(c: &Soname) -> Result<Libs, FfError> {
    let avcodec = Lib::open(c.avcodec)?;
    // сверяем, что мажор библиотеки совпадает с ожидаемым набором типов
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
            "{}: avcodec_version() = {maj}.{min}.{mic}, ожидался мажор {expected_avcodec_major}",
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
