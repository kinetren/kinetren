//! Язык интерфейса при первом запуске — из локали ОС (решение п.3 р.19).
//! Выбор пользователя перекрывает автоопределение и живёт в eframe Storage.

use super::Lang;

/// `ru_RU.UTF-8`, `zh_CN`, `es-MX@euro` → язык. `C`, `POSIX` и всё
/// неизвестное — `None`, вызывающий берёт английский.
pub fn from_locale(s: &str) -> Option<Lang> {
    let s = s.split(['.', '@']).next().unwrap_or("");
    let base = s.split(['_', '-']).next().unwrap_or("").to_ascii_lowercase();
    match base.as_str() {
        // zh_TW/zh_HK тоже сюда: каталог один, упрощённый (см. i18n/README.md)
        "zh" => Some(Lang::Zh),
        "ar" => Some(Lang::Ar),
        "es" => Some(Lang::Es),
        "ru" => Some(Lang::Ru),
        "en" => Some(Lang::En),
        _ => None,
    }
}

/// Приоритет POSIX: `LC_ALL` → `LC_MESSAGES` → `LANG`. Пустое значение
/// переменной не считается заданным.
pub fn from_env() -> Option<Lang> {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var)
            && !v.is_empty()
        {
            return from_locale(&v);
        }
    }
    None
}
