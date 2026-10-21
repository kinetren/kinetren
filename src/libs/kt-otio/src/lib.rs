//! Родной формат проекта Kinetren — OpenTimelineIO (.otio, JSON).
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
mod legacy_import;
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
    /// Это файл ПРОЕКТА своего формата (Р.70), а не файл обмена: дверь
    /// импорта ему не нужна — его открывает обычное «Открыть».
    #[error("a Kinetren project file, not an interchange file — use File → Open")]
    ProjectFile,
    /// A project of the former name (`kinotrain` root key): Open refuses it,
    /// the import door repairs it (`kt_model::project_file::from_former_text`).
    #[error("a project of Kinotrain (the former name of Kinetren) — use File → Import older Kinetren project…")]
    FormerProject,
    /// Файл написан более новым Kinetren (metadata.kt.v выше нашей).
    #[error("project written by a newer Kinetren (kt.v = {0})")]
    KtMetaTooNew(u32),
    /// Секция metadata.kt есть, но не читается. Молчать нельзя: с ней
    /// уехали бы настройки проекта, дорожки, морферы и раскладка, а файл
    /// открылся бы как чужой (Р.35 — нашлось живым сценарием).
    #[error("metadata.kt is unreadable: {0}")]
    KtMetaBroken(String),
    /// Файл ПРЕЖНЕЙ версии конверта, который ридер не читает (Р.67):
    /// его чинит импорт (`import_older`), а не ридер. Текст называет
    /// версию и дверь — kt-app подставляет имя пункта меню.
    #[error("project of an older Kinetren version (kt.v = {version}) — use File → Import older Kinetren project… ({cause})")]
    NeedsImport { version: u32, cause: String },
    #[error("non-finite number in {0}")]
    NonFinite(&'static str),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Формат файла по СОДЕРЖИМОМУ (расширению не верим).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Файл ПРОЕКТА своего формата (Р.70): признак `kinetren` на корне.
    /// Читает его `kt_model::project_file`; этот крейт лишь опознаёт.
    Project,
    /// A project file of the former name (`kinotrain` at the root).
    FormerProject,
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
        FileFormat::Project => Err(Error::ProjectFile),
        FileFormat::FormerProject => Err(Error::FormerProject),
        FileFormat::Otio => {
            let tl = parse_document(text)?;
            match timeline_to_project(&tl, base_dir) {
                // наша секция старой версии не читается — это работа
                // импорта, а не ридера (Р.67): называем версию и дверь
                Err(Error::KtMetaBroken(cause)) => match meta::kt_version(&tl) {
                    Some(v) if v < meta::KT_META_VERSION => {
                        Err(Error::NeedsImport { version: v, cause })
                    }
                    _ => Err(Error::KtMetaBroken(cause)),
                },
                other => other,
            }
        }
        FileFormat::LegacyKtproj => Err(Error::NotOtio),
    }
}

/// Импорт проекта СТАРОЙ версии конверта (Р.67): починки старых форм, затем
/// обычное чтение. См. `legacy_import`.
pub use legacy_import::{FIXUPS, Fixup, import_older};

/// ДВЕРЬ ИМПОРТА (Р.70) — одна на три входа: легаси `.ktproj`, наш `.otio`
/// любой версии конверта и чужой `.otio`. Правило выбора пути живёт ЗДЕСЬ,
/// и приложение с kt-cli зовут его одинаково: чужой файл без нашего конверта
/// читается обычным ридером (ему починки не нужны и `import_older` его
/// отверг бы), наш старой версии — починками, легаси — своим ридером.
/// Файл проекта в эту дверь не идёт: `Error::ProjectFile` называет «Открыть».
/// Пути медиа в результате АБСОЛЮТНЫ (разрешены от `base_dir`), как у
/// `load_project`.
pub fn import_interchange(
    text: &str,
    base_dir: Option<&std::path::Path>,
) -> Result<ProjectBundle, Error> {
    match detect(text)? {
        FileFormat::Project => Err(Error::ProjectFile),
        // a project of the former name: the key is renamed, the rest is read
        // by the normal reader (owner's rule R.67 — repairs live here)
        FileFormat::FormerProject => {
            let f = kt_model::project_file::from_former_text(text)
                .map_err(|e| Error::KtMetaBroken(e.to_string()))?;
            let retained = match f.otio {
                Some(v) => Retained::from_json(&v)?,
                None => Retained::default(),
            };
            let mut project = f.project;
            resolve_project_media(&mut project, base_dir);
            let notices = vec![format!(
                "import fixup: project marker `{}` → `{}` (renamed to Kinetren)",
                kt_model::project_file::FORMER_MARKER,
                kt_model::project_file::MARKER
            )];
            Ok(ProjectBundle { project, retained, notices })
        }
        FileFormat::LegacyKtproj => {
            let (mut project, notices) = import_legacy(text)?;
            resolve_project_media(&mut project, base_dir);
            Ok(ProjectBundle { project, retained: Retained::default(), notices })
        }
        FileFormat::Otio => match load_project(text, base_dir) {
            Err(Error::NeedsImport { .. }) => import_older(text, base_dir),
            other => other,
        },
    }
}

/// Относительные пути медиа → абсолютные от каталога файла. Псевдо-пути
/// (`synth:`, `gen:`) и абсолютные не трогаются. Зовётся при ЧТЕНИИ файла
/// проекта: модель в памяти держит абсолютные пути, как после `load_project`.
pub fn resolve_project_media(p: &mut kt_model::Project, base_dir: Option<&std::path::Path>) {
    let Some(base) = base_dir else { return };
    for m in &mut p.media {
        if kt_model::is_pseudo_path(&m.path) || std::path::Path::new(&m.path).is_absolute() {
            continue;
        }
        m.path = resolve_media_path(base, &m.path).to_string_lossy().into_owned();
    }
}

/// Абсолютные пути медиа → относительные к каталогу файла, где это
/// выразимо (иначе остаются абсолютными). Зовётся при ЗАПИСИ файла проекта:
/// проект, переехавший вместе с материалом, обязан открыться (т797).
pub fn relativize_project_media(p: &mut kt_model::Project, base_dir: Option<&std::path::Path>) {
    let Some(base) = base_dir else { return };
    for m in &mut p.media {
        if kt_model::is_pseudo_path(&m.path) || !std::path::Path::new(&m.path).is_absolute() {
            continue;
        }
        if let Some(rel) = relativize(base, std::path::Path::new(&m.path)) {
            m.path = rel;
        }
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
    if map.contains_key(kt_model::project_file::MARKER) {
        return Ok(FileFormat::Project);
    }
    if map.contains_key(kt_model::project_file::FORMER_MARKER) {
        return Ok(FileFormat::FormerProject);
    }
    if map.get("OTIO_SCHEMA").is_some() {
        return Ok(FileFormat::Otio);
    }
    if map.get("format_version").is_some() {
        return Ok(FileFormat::LegacyKtproj);
    }
    Err(Error::NotOtio)
}
