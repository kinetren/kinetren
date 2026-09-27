//! kt-ffmpeg: рантайм-слой над системным FFmpeg (мажоры 6/7/8).
//!
//! ИНВАРИАНТ: бинарь НЕ линкуется с libav* (`ldd kt-app | grep libav` пуст).
//! Библиотеки грузятся через dlopen при первом обращении, версия
//! определяется по `avcodec_version()`, а типы структур берутся из
//! соответствующего версионного модуля (bindgen по вендоренным заголовкам
//! `vendor/ffmpeg-N/include`). Благодаря этому один и тот же исполняемый
//! файл работает на системе с любым из поддерживаемых FFmpeg и использует
//! все его аппаратные ускорения.

pub mod api;
pub mod loader;

/// Кумулятивные счётчики активности FFmpeg-слоя (перф-панель; порт
/// avcpp-метрик v1: gAvcppOpenCalls/gAvcppPacketReads). Глобальные на
/// процесс, монотонные; UI дифференцирует по времени.
pub mod iostats {
    use std::sync::atomic::{AtomicU64, Ordering};
    /// Открытия demux-контекстов и декодеров (включая пересоздания на seek).
    pub static OPENS: AtomicU64 = AtomicU64::new(0);
    /// Прочитанные пакеты (av_read_frame по всем путям, включая скан).
    pub static PACKET_READS: AtomicU64 = AtomicU64::new(0);
    /// (opens, packet_reads)
    pub fn snapshot() -> (u64, u64) {
        (OPENS.load(Ordering::Relaxed), PACKET_READS.load(Ordering::Relaxed))
    }
}

use kt_time::Tick;

#[derive(Debug, thiserror::Error)]
pub enum FfError {
    #[error(
        "FFmpeg not found on this system. Install libavcodec/libavformat (FFmpeg 6, 7 or 8).\n{0}"
    )]
    NotFound(String),
    #[error("loading FFmpeg: {0}")]
    Load(String),
    #[error("{0}")]
    Av(String),
    #[error("no such stream or position")]
    OutOfRange,
}

/// Что удалось загрузить (для перф-панели и диагностики).
#[derive(Debug, Clone)]
pub struct BackendInfo {
    pub major: u32,
    pub avcodec_version: (u32, u32, u32),
}

/// Информация о медиафайле (дублирует kt_media::MediaInfo, но kt-ffmpeg
/// не зависит от kt-media — мост делает kt-media).
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub duration: Tick,
    pub has_video: bool,
    pub has_audio: bool,
    pub fps: Option<(u32, u32)>,
    /// Исходный размер кадра (0,0 если видео нет).
    pub width: u32,
    pub height: u32,
    /// Пиксельное соотношение сторон потока (num/den), 1:1 если не задано.
    pub sar: (u32, u32),
    /// ОДИНОЧНАЯ КАРТИНКА (Р.67): демуксер картинок, видео без звука и без
    /// своей длительности. У неё нет ни сетки кадров, ни конца материала —
    /// один кадр отдаётся на любой запрошенный тик, как у заглушки.
    pub still: bool,
    /// У формата пикселей потока ЕСТЬ АЛЬФА (Р.67): PNG с прозрачностью,
    /// ProRes 4444, WebM VP9 с alpha. Композит умеет её смешивать —
    /// раньше он просто не знал о ней (`has_alpha: false` прибито гвоздём).
    pub has_alpha: bool,
}

/// Конфигурация экспорта.
#[derive(Debug, Clone)]
pub struct MuxConfig {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// CRF/CQ/QP — смысл зависит от энкодера (23 ≈ визуально прозрачно).
    /// Игнорируется, если задан video_bitrate.
    pub quality: u32,
    /// Целевой битрейт видео, бит/с (None = режим качества CRF/CQ).
    pub video_bitrate: Option<u32>,
    pub audio_bitrate: u32,
    /// Принудительный энкодер (иначе автовыбор с hw-приоритетом).
    pub force_encoder: Option<String>,
}

/// Выбранный энкодер (для показа в UI).
#[derive(Debug, Clone)]
pub struct EncoderChoice {
    pub name: String,
    pub hardware: bool,
}

/// Готовый пакетный индекс (из sidecar-БД kt-media): pts аудио-пакетов и
/// pts видео-кейфреймов в timebase соответствующих потоков, отсортированы.
/// Валиден только для байт-в-байт того же файла (идентичность проверяет
/// вызывающая сторона по xxhash/size) — pts привязаны к контейнеру.
#[derive(Debug, Clone, Default)]
pub struct PrebuiltIndex {
    pub audio_pts: Vec<i64>,
    pub video_keyframes: Vec<i64>,
}

/// Кадр в RGBA.
pub struct RgbaFrame {
    pub pts: Tick,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Реализация декодера, общая для всех версий (включается в каждый
/// версионный модуль с собственными типами).
macro_rules! version_module {
    ($name:ident, $types:literal, $major:literal) => {
        pub mod $name {
            #![allow(
                non_upper_case_globals,
                non_camel_case_types,
                non_snake_case,
                dead_code,
                unsafe_op_in_unsafe_fn,
                clippy::all
            )]
            pub mod ffi {
                #![allow(warnings)]
                include!(concat!(env!("OUT_DIR"), "/", $types));
            }
            crate::ffmpeg_api!();
            include!("source_impl.rs");
            include!("muxer_impl.rs");
        }
    };
}

version_module!(v6, "types_v6.rs", 6);
version_module!(v7, "types_v7.rs", 7);
version_module!(v8, "types_v8.rs", 8);

/// Единая точка входа: загружает системный FFmpeg (один раз на процесс)
/// и открывает файл подходящей версией реализации.
pub enum Backend {
    V6(&'static v6::Api),
    V7(&'static v7::Api),
    V8(&'static v8::Api),
}

static BACKEND: std::sync::OnceLock<Result<(Backend, BackendInfo), String>> =
    std::sync::OnceLock::new();

fn init() -> &'static Result<(Backend, BackendInfo), String> {
    BACKEND.get_or_init(|| {
        let libs = loader::load().map_err(|e| e.to_string())?;
        let info = BackendInfo { major: libs.major, avcodec_version: libs.avcodec_version };
        // Библиотеки живут до конца процесса — утечка намеренная (иначе
        // указатели на функции повисли бы).
        let libs: &'static loader::Libs = Box::leak(Box::new(libs));
        let backend = unsafe {
            match libs.major {
                8 => Backend::V8(Box::leak(Box::new(
                    v8::Api::load(libs).map_err(|e| e.to_string())?,
                ))),
                7 => Backend::V7(Box::leak(Box::new(
                    v7::Api::load(libs).map_err(|e| e.to_string())?,
                ))),
                _ => Backend::V6(Box::leak(Box::new(
                    v6::Api::load(libs).map_err(|e| e.to_string())?,
                ))),
            }
        };
        Ok((backend, info))
    })
}

/// Версия загруженного FFmpeg (None, если не найден).
pub fn backend_info() -> Option<BackendInfo> {
    init().as_ref().ok().map(|(_, i)| i.clone())
}

pub fn backend_error() -> Option<String> {
    init().as_ref().err().cloned()
}

/// Список доступных в системе видео-энкодеров (для UI).
pub fn available_encoders() -> Vec<String> {
    match init().as_ref() {
        Ok((Backend::V6(api), _)) => v6::available_encoders(api),
        Ok((Backend::V7(api), _)) => v7::available_encoders(api),
        Ok((Backend::V8(api), _)) => v8::available_encoders(api),
        Err(_) => Vec::new(),
    }
}

/// Муксер MP4 — динамическая диспетчеризация по версии.
pub enum Muxer {
    V6(v6::Muxer),
    V7(v7::Muxer),
    V8(v8::Muxer),
}

impl Muxer {
    pub fn open(cfg: &MuxConfig) -> Result<Muxer, FfError> {
        let (backend, _) = init().as_ref().map_err(|e| FfError::NotFound(e.clone()))?;
        match backend {
            Backend::V6(api) => v6::Muxer::open(api, cfg).map(Muxer::V6),
            Backend::V7(api) => v7::Muxer::open(api, cfg).map(Muxer::V7),
            Backend::V8(api) => v8::Muxer::open(api, cfg).map(Muxer::V8),
        }
    }

    pub fn encoder_name(&self) -> &str {
        match self {
            Muxer::V6(m) => m.encoder_name(),
            Muxer::V7(m) => m.encoder_name(),
            Muxer::V8(m) => m.encoder_name(),
        }
    }

    /// Один кадр RGBA размера width×height из конфигурации.
    pub fn write_video(&mut self, rgba: &[u8]) -> Result<(), FfError> {
        match self {
            Muxer::V6(m) => m.write_video(rgba),
            Muxer::V7(m) => m.write_video(rgba),
            Muxer::V8(m) => m.write_video(rgba),
        }
    }

    /// Аудио 48 kHz stereo f32 interleaved (любой длины).
    pub fn write_audio(&mut self, samples: &[f32]) -> Result<(), FfError> {
        match self {
            Muxer::V6(m) => m.write_audio(samples),
            Muxer::V7(m) => m.write_audio(samples),
            Muxer::V8(m) => m.write_audio(samples),
        }
    }

    pub fn finish(self) -> Result<(), FfError> {
        match self {
            Muxer::V6(m) => m.finish(),
            Muxer::V7(m) => m.finish(),
            Muxer::V8(m) => m.finish(),
        }
    }
}

/// Открытый источник — динамическая диспетчеризация по версии.
pub enum Source {
    V6(v6::Source),
    V7(v7::Source),
    V8(v8::Source),
}

impl Source {
    pub fn open(path: &str) -> Result<Source, FfError> {
        Self::open_with_index(path, None)
    }

    /// Открытие с готовым пакетным индексом: полный скан-проход по файлу
    /// пропускается (остаётся только чтение заголовка) — быстрое открытие
    /// больших файлов. Индекс обязан соответствовать текущему содержимому
    /// файла (идентичность проверяет вызывающая сторона).
    pub fn open_with_index(path: &str, prebuilt: Option<PrebuiltIndex>) -> Result<Source, FfError> {
        let (backend, _) = init().as_ref().map_err(|e| FfError::NotFound(e.clone()))?;
        match backend {
            Backend::V6(api) => v6::Source::open_with_index(api, path, prebuilt).map(Source::V6),
            Backend::V7(api) => v7::Source::open_with_index(api, path, prebuilt).map(Source::V7),
            Backend::V8(api) => v8::Source::open_with_index(api, path, prebuilt).map(Source::V8),
        }
    }

    /// (pts аудио-пакетов, pts видео-кейфреймов) — для персиста в sidecar-БД.
    pub fn packet_index(&self) -> (&[i64], &[i64]) {
        match self {
            Source::V6(s) => s.packet_index(),
            Source::V7(s) => s.packet_index(),
            Source::V8(s) => s.packet_index(),
        }
    }

    /// Индекс пришёл готовым (скан не выполнялся) — диагностика/тест 34.
    pub fn index_was_prebuilt(&self) -> bool {
        match self {
            Source::V6(s) => s.index_was_prebuilt(),
            Source::V7(s) => s.index_was_prebuilt(),
            Source::V8(s) => s.index_was_prebuilt(),
        }
    }

    pub fn probe(&self) -> &Probe {
        match self {
            Source::V6(s) => s.probe(),
            Source::V7(s) => s.probe(),
            Source::V8(s) => s.probe(),
        }
    }

    /// Аудио-burst 48 kHz stereo f32 interleaved, ровно `out.len()` значений.
    pub fn read_audio_burst(&mut self, idx: i64, out: &mut [f32]) -> Result<(), FfError> {
        match self {
            Source::V6(s) => s.read_audio_burst(idx, out),
            Source::V7(s) => s.read_audio_burst(idx, out),
            Source::V8(s) => s.read_audio_burst(idx, out),
        }
    }

    pub fn read_video_frame(
        &mut self,
        pts: Tick,
        width: u32,
        height: u32,
    ) -> Result<RgbaFrame, FfError> {
        match self {
            Source::V6(s) => s.read_video_frame(pts, width, height),
            Source::V7(s) => s.read_video_frame(pts, width, height),
            Source::V8(s) => s.read_video_frame(pts, width, height),
        }
    }

    /// Спан-декод: см. source_impl::read_video_span.
    pub fn read_video_span(
        &mut self,
        keys: &[Tick],
        width: u32,
        height: u32,
        emit: &mut dyn FnMut(Tick, RgbaFrame),
    ) -> Result<(), FfError> {
        match self {
            Source::V6(s) => s.read_video_span(keys, width, height, emit),
            Source::V7(s) => s.read_video_span(keys, width, height, emit),
            Source::V8(s) => s.read_video_span(keys, width, height, emit),
        }
    }

    /// (backward_reopens, frames_decoded) — диагностика производительности.
    pub fn video_debug_stats(&self) -> (u64, u64) {
        match self {
            Source::V6(s) => s.video_debug_stats(),
            Source::V7(s) => s.video_debug_stats(),
            Source::V8(s) => s.video_debug_stats(),
        }
    }

    pub fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        match self {
            Source::V6(s) => s.keyframe_before(pts),
            Source::V7(s) => s.keyframe_before(pts),
            Source::V8(s) => s.keyframe_before(pts),
        }
    }
}
