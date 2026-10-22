//! The UI language on first launch — from the OS locale (decision of item 3, round 19).
//! The user's choice overrides auto-detection and lives in eframe Storage.

use super::Lang;

/// `ru_RU.UTF-8`, `zh_CN`, `es-MX@euro` → a language. `C`, `POSIX` and everything
/// unknown — `None`; the caller takes English.
pub fn from_locale(s: &str) -> Option<Lang> {
    let s = s.split(['.', '@']).next().unwrap_or("");
    let base = s.split(['_', '-']).next().unwrap_or("").to_ascii_lowercase();
    match base.as_str() {
        // zh_TW/zh_HK also go here: the catalog is one, simplified (see i18n/README.md)
        "zh" => Some(Lang::Zh),
        "ar" => Some(Lang::Ar),
        "es" => Some(Lang::Es),
        "ru" => Some(Lang::Ru),
        "en" => Some(Lang::En),
        _ => None,
    }
}

/// POSIX priority: `LC_ALL` → `LC_MESSAGES` → `LANG`. An empty value of a
/// variable is not considered set.
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
