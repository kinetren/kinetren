//! Родной формат проекта Kinotrain — OpenTimelineIO (.otio, JSON).
//!
//! Чистая Rust-реализация подмножества схем OTIO (эталон — C++/Python
//! библиотека 0.18.x) с полным passthrough непонятого: неизвестные ПОЛЯ
//! известных схем сохраняются на своих местах (`extra`), объекты неизвестных
//! схем/версий — как сырой JSON (`RawObject`), чужие неймспейсы `metadata` —
//! verbatim. Наши данные живут в `metadata["kt"]` (решение Р.24).
//!
//! Инвариант байт-стабильности — ИДЕМПОТЕНТНОСТЬ записи:
//! `write(parse(write(x))) == write(x)`. Первое сохранение чужого файла
//! нормализует форматирование (как делает и эталонная библиотека).

mod convert;
mod io;
mod legacy;
mod meta;
mod schema;
mod url;

pub use convert::{
    ClipExtra, MediaRefs, Notice, ProjectBundle, Retained, grid_fps, project_to_timeline,
    timeline_to_project,
};
pub use io::{
    AtomicFs, RealFs, Written, atomic_tmp_path, parse_document, write_atomic, write_atomic_with,
    write_document,
};
pub use legacy::import_legacy;
pub use meta::{
    KT_META_V6, KT_META_V8, KT_META_V9, KT_META_V10, KT_META_V11, KT_META_V12, KT_META_VERSION,
    KT_NS,
    check_kt_version, kt_entry_mut, kt_version,
};
pub use schema::{
    Clip, Composable, Effect, EffectNode, ExternalReference, Gap, LinearTimeWarp, Marker,
    MarkerNode, MediaRefNode, Metadata, MissingReference, RationalTime, RawObject, Stack,
    TimeRange, Timeline, Track, Transition,
};
/// Чужие имена генераторов, у которых есть наш эквивалент (импорт-алиас,
/// Р.52 С5): читаются первоклассно и при пересохранении канонизируются в
/// `kt.<tag>`. Список короткий и явный — незнакомое остаётся passthrough.
pub(crate) fn generator_alias(kind: &str) -> Option<&'static str> {
    match kind {
        "SMPTEBars" => Some("smpte"),
        _ => None,
    }
}

pub use url::{decode_file_url, encode_file_url, media_target_url, relativize, resolve_media_path};

/// Ключ единственной media-reference наших клипов (конвенция эталона).
pub const DEFAULT_MEDIA_KEY: &str = "DEFAULT_MEDIA";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a JSON object")]
    NotAnObject,
    #[error("missing OTIO_SCHEMA tag")]
    MissingSchemaTag,
    #[error("malformed OTIO_SCHEMA tag: {0}")]
    BadSchemaTag(String),
    /// Схема структурного хребта (Timeline/Stack/Track) новее поддерживаемой:
    /// «полупонимание» позиций детей небезопасно — честный отказ.
    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(String),
    #[error("root of an .otio file must be a Timeline, found {0}")]
    UnsupportedRoot(String),
    #[error("not an OpenTimelineIO file")]
    NotOtio,
    /// Файл написан более новым Kinotrain (metadata.kt.v выше нашей).
    #[error("project written by a newer Kinotrain (kt.v = {0})")]
    KtMetaTooNew(u32),
    /// Секция metadata.kt есть, но не читается. Молчать нельзя: с ней
    /// уехали бы настройки проекта, дорожки, морферы и раскладка, а файл
    /// открылся бы как чужой (Р.35 — нашлось живым сценарием).
    #[error("metadata.kt is unreadable: {0}")]
    KtMetaBroken(String),
    #[error("non-finite number in {0}")]
    NonFinite(&'static str),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Формат файла по СОДЕРЖИМОМУ (расширению не верим).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Otio,
    /// Старый .ktproj: как проект не открывается (решение Р.24 п.12) —
    /// только пункт «Импортировать .ktproj…».
    LegacyKtproj,
}

/// Загрузка .otio-проекта (файл .ktproj — честная ошибка: Р.24 п.12, «как
/// проект не открывается» — используйте import_legacy из пункта импорта).
pub fn load_project(
    text: &str,
    base_dir: Option<&std::path::Path>,
) -> Result<ProjectBundle, Error> {
    match detect(text)? {
        FileFormat::Otio => {
            let tl = parse_document(text)?;
            timeline_to_project(&tl, base_dir)
        }
        FileFormat::LegacyKtproj => Err(Error::NotOtio),
    }
}

/// Сериализация проекта в .otio (идемпотентная запись, тест 144).
pub fn save_project(
    p: &kt_model::Project,
    retained: &Retained,
    base_dir: Option<&std::path::Path>,
) -> String {
    write_document(&project_to_timeline(p, retained, base_dir))
}

pub fn detect(text: &str) -> Result<FileFormat, Error> {
    let v: serde_json::Value = serde_json::from_str(text)?;
    let serde_json::Value::Object(map) = v else {
        return Err(Error::NotAnObject);
    };
    if map.get("OTIO_SCHEMA").is_some() {
        return Ok(FileFormat::Otio);
    }
    if map.get("format_version").is_some() {
        return Ok(FileFormat::LegacyKtproj);
    }
    Err(Error::NotOtio)
}
