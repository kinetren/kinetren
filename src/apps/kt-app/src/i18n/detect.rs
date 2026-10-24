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
/// variable is not considered set. When none is set — the user's locale name of the OS
/// (Windows: `ru-RU`, `zh-Hans-CN`; windows-build: these variables do not exist there, and
/// every Windows user started in English).
pub fn from_env() -> Option<Lang> {
    detect(&|v| std::env::var(v).ok(), os_locale_name())
}

/// The rule over injected sources (t861).
pub fn detect(var: &dyn Fn(&str) -> Option<String>, os_locale: Option<String>) -> Option<Lang> {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(v) = var(name)
            && !v.is_empty()
        {
            return from_locale(&v);
        }
    }
    os_locale.as_deref().and_then(from_locale)
}

/// The user's default locale name of Windows (`GetUserDefaultLocaleName`).
#[cfg(windows)]
fn os_locale_name() -> Option<String> {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
    let mut buf = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH
    let n = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    (n > 1).then(|| String::from_utf16_lossy(&buf[..(n - 1) as usize]))
}

#[cfg(not(windows))]
fn os_locale_name() -> Option<String> {
    None
}
