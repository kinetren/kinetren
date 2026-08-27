//! Отображаемая версия программы (п.6 р.12).
//!
//! `FULL` — `ktYY.MM.DD-<hash8>` по дате коммита: по ней однозначно видно,
//! из какой ревизии собран бинарь (пользователь дважды запускал устаревшую
//! сборку и считал дефекты неисправленными). `SHORT` — `ktYY.MM` для
//! заголовка релизной сборки, где длинная строка не нужна.
//!
//! В debug-сборке показывается ПОЛНАЯ версия — там она и нужна.

pub const FULL: &str = env!("KT_VERSION_FULL");
pub const SHORT: &str = env!("KT_VERSION_SHORT");

/// Что писать в заголовке: debug — полную, релиз — короткую.
pub fn display() -> &'static str {
    if cfg!(debug_assertions) { FULL } else { SHORT }
}

#[cfg(test)]
mod tests {
    /// Версия обязана быть непустой и в ожидаемом формате: иначе в
    /// заголовке появится мусор, и «какая у меня сборка» снова неясно.
    #[test]
    fn version_strings_are_sane() {
        assert!(super::FULL.starts_with("kt"), "FULL = {}", super::FULL);
        assert!(super::SHORT.starts_with("kt"), "SHORT = {}", super::SHORT);
        assert!(super::FULL.len() >= 10, "FULL слишком короткая: {}", super::FULL);
        assert!(super::display().len() >= 7);
    }
}
