//! The displayed version of the program (item 6 of round 12).
//!
//! `FULL` — `ktYY.MM.DD-<hash8>` by the commit date: by it one can tell unambiguously
//! which revision the binary was built from (the user twice launched an outdated
//! build and counted the defects as unfixed). `SHORT` — `ktYY.MM` for the
//! title of a release build, where the long string is not needed.
//!
//! In a debug build the FULL version is shown — there it is needed.

pub const FULL: &str = env!("KT_VERSION_FULL");
pub const SHORT: &str = env!("KT_VERSION_SHORT");

/// What to write in the title: debug — the full one, release — the short one.
pub fn display() -> &'static str {
    if cfg!(debug_assertions) { FULL } else { SHORT }
}

#[cfg(test)]
mod tests {
    /// The version must be non-empty and in the expected format: otherwise garbage
    /// would appear in the title, and “which build do I have” would again be unclear.
    #[test]
    fn version_strings_are_sane() {
        assert!(super::FULL.starts_with("kt"), "FULL = {}", super::FULL);
        assert!(super::SHORT.starts_with("kt"), "SHORT = {}", super::SHORT);
        assert!(super::FULL.len() >= 10, "FULL is too short: {}", super::FULL);
        assert!(super::display().len() >= 7);
    }
}
