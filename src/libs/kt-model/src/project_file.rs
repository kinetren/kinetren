//! ФАЙЛ ПРОЕКТА — свой формат (Р.70).
//!
//! Решение владельца (02.10.2026): «мы уходим от otio-формата, оставляем
//! otio только на экспорт и делаем breaking changes чтобы наш формат стал в
//! сто раз лучше» и «я против бинарщины — у нас файл должен быть
//! индексируемым в git».
//!
//! Повод. В JSON OpenTimelineIO нет ОБЪЕКТНОЙ ИДЕНТИЧНОСТИ: сказать «это
//! та же лента, что выше» нечем, выражается только вложенное содержимое,
//! поэтому тело ленты писалось заново в каждый её экземпляр. У владельца три
//! ленты дали 15 220 клипов и 115 МБ, которые автосейв писал регулярно.
//! Замер против сериализации модели: 17× на плоском проекте, 501× и 929×
//! на вложенных сценах.
//!
//! Что здесь:
//!
//! * файл — сериализация МОДЕЛИ. «Каждая сущность один раз» — свойство самой
//!   модели (`Project.sequences` держит тело ленты, экземпляр ссылается
//!   адресом), и формат его просто не портит;
//! * текст, ПОСТРОЧНЫЙ и детерминированный: правка одного события меняет
//!   единицы строк, одна модель даёт те же байты. Компактный JSON отвергнут —
//!   одна строка на весь файл диффа не имеет; двоичный и сжатый запрещены
//!   владельцем;
//! * на корне свой признак (`kinotrain`) и ВЕРСИЯ; вид файла определяется по
//!   содержимому, не по расширению. Версия больше нынешней — честный отказ с
//!   номером, частичное чтение запрещено («открылось, и части монтажа нет» —
//!   худший исход из трёх, F-RES-003). Ломающие изменения разрешены, каскада
//!   совместимости с чужой схемой здесь нет;
//! * чужой passthrough импортированного файла обмена едет отдельным полем
//!   (`otio`) как непрозрачный JSON с отсортированными ключами. Сортировка
//!   ЯВНАЯ: `kt-otio` включает `serde_json/preserve_order`, а фичи cargo едины
//!   на весь workspace, так что `Map` везде insertion-ordered — положиться на
//!   «ключи сортируются сами» нельзя.
//!
//! Чего здесь НЕТ: записи на диск. Единственный дом атомарной записи — `kt-otio`
//! (`write_atomic`, Р.49: «второго писателя файла проекта в дереве нет»), и
//! сторож ревизии приложения (`write_project_guarded`) зовёт его с текстом
//! отсюда. Этот модуль отвечает за ТЕКСТ.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::project::{Project, ProjectError};

/// Ключ-признак на корне файла. По нему файл опознаётся как наш; его
/// значение — версия формата.
pub const MARKER: &str = "kinotrain";

/// Версия СВОЕГО формата. Не путать с `project::FORMAT_VERSION` — это версия
/// легаси-формата C++ (`.ktproj`, ключ `format_version`), которую читает
/// только импорт.
pub const PROJECT_FILE_VERSION: u32 = 1;

/// Расширение файла проекта. `.ktproj` НЕ переиспользуется: два разных
/// формата под одним суффиксом — ловушка для человека и скриптов, а
/// легаси-`.ktproj` жив в импорте.
pub const EXTENSION: &str = "kt";

/// Содержимое файла проекта: модель и непрозрачные остатки чужого файла
/// обмена, если проект был из него импортирован.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectFile {
    pub project: Project,
    /// Passthrough импортированного `.otio` (чужие неймспейсы, маркеры,
    /// эффекты) — чтобы экспорт вернул их на место. `None` у проекта, который
    /// никогда не был файлом обмена. Содержимое непрозрачно для модели.
    pub otio: Option<Value>,
}

/// Почему файл не прочитался. Каждый исход назван: читатель обязан сказать
/// человеку, что это за файл и какой дверью его открыть (правило 19).
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ProjectFileError {
    /// Это не файл проекта. `looks_like` — на что он похож по содержимому
    /// (файл обмена, легаси), чтобы ошибка назвала дверь импорта.
    #[error("not a Kinotrain project file{}", looks_like_suffix(.looks_like))]
    NotOurs { looks_like: Option<FileKind> },
    /// Файл написан более новой программой. Частичное чтение запрещено.
    #[error(
        "project written by a newer Kinotrain (format {version}, this build reads up to {PROJECT_FILE_VERSION})"
    )]
    TooNew { version: u32 },
    #[error("invalid json: {0}")]
    Json(String),
    #[error("project invalid: {0}")]
    Model(#[from] ProjectError),
}

/// На что похож файл, который не наш. Различается по СОДЕРЖИМОМУ: расширению
/// верить нельзя, recovery- и tmp-файлы носят чужие суффиксы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// Файл обмена OpenTimelineIO (`OTIO_SCHEMA` на корне).
    Otio,
    /// Легаси-проект C++ (`format_version` на корне).
    LegacyKtproj,
}

fn looks_like_suffix(kind: &Option<FileKind>) -> String {
    match kind {
        Some(FileKind::Otio) => {
            " — it is an OpenTimelineIO file; use File → Import older Kinotrain project…".into()
        }
        Some(FileKind::LegacyKtproj) => {
            " — it is a legacy .ktproj; use File → Import older Kinotrain project…".into()
        }
        None => String::new(),
    }
}

/// Корневой конверт файла. Порядок полей — порядок строк в файле: версия
/// первой, чтобы незнакомую отвергнуть, не читая остального.
#[derive(Serialize, Deserialize)]
struct Envelope {
    #[serde(rename = "kinotrain")]
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    otio: Option<Value>,
    project: Project,
}

/// Текст файла проекта: построчный, детерминированный, с завершающим
/// переводом строки (так файл дружит с `git diff` и `cat`).
pub fn to_text(project: &Project, otio: Option<&Value>) -> String {
    let env = Envelope {
        version: PROJECT_FILE_VERSION,
        otio: otio.map(sorted),
        project: project.clone(),
    };
    let mut s = serde_json::to_string_pretty(&env)
        .expect("модель сериализуется всегда: ни карт с нестроковыми ключами, ни NaN");
    s.push('\n');
    s
}

/// Прочитать файл проекта. Чужой файл и файл будущей версии — названный
/// отказ; прочитанная модель нормализована и проверена, как при любом
/// чтении с диска.
pub fn from_text(text: &str) -> Result<ProjectFile, ProjectFileError> {
    let peek: Value = serde_json::from_str(text).map_err(|e| ProjectFileError::Json(e.to_string()))?;
    let Some(root) = peek.as_object() else {
        return Err(ProjectFileError::Json("root is not an object".into()));
    };
    let Some(version) = root.get(MARKER) else {
        return Err(ProjectFileError::NotOurs { looks_like: looks_like(root) });
    };
    let version = version
        .as_u64()
        .ok_or_else(|| ProjectFileError::Json(format!("`{MARKER}` is not a version number")))?;
    if version > u64::from(PROJECT_FILE_VERSION) {
        return Err(ProjectFileError::TooNew { version: version as u32 });
    }
    // версии НИЖЕ нынешней читаются миграцией на чтении, односторонне; пока
    // версия одна, мигрировать нечего — место для будущих починок здесь
    let env: Envelope =
        serde_json::from_value(peek).map_err(|e| ProjectFileError::Json(e.to_string()))?;
    let mut project = env.project;
    project.normalize();
    project.validate()?;
    Ok(ProjectFile { project, otio: env.otio })
}

/// На что похож JSON-объект без нашего признака.
pub fn looks_like(root: &serde_json::Map<String, Value>) -> Option<FileKind> {
    if root.contains_key("OTIO_SCHEMA") {
        Some(FileKind::Otio)
    } else if root.contains_key("format_version") {
        Some(FileKind::LegacyKtproj)
    } else {
        None
    }
}

/// Это наш файл? Только по признаку на корне — для детекта вида файла до
/// полного чтения.
pub fn is_project_file(text: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| v.as_object().map(|o| o.contains_key(MARKER)))
        .unwrap_or(false)
}

/// Глубокая сортировка ключей объекта: единственный источник
/// неупорядоченности в файле — чужой passthrough, и без сортировки каждое
/// сохранение давало бы ложный дифф.
fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                out.insert(k.clone(), sorted(&map[k]));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}
