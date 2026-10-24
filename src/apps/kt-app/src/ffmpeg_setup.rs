//! Where the program looks for FFmpeg, as far as only the program knows it (change
//! windows-build, capability media-index): the folder the person chose with *Locate
//! folder…* lives in `<data_dir>/ffmpeg-dir` — one line, so kt-cli reads the same choice.
//!
//! The search itself is `kt_ffmpeg::search`; this module only hands it the program's own
//! folders before the first media open.

use std::path::PathBuf;

/// The file holding the person's choice.
pub fn user_choice_file() -> PathBuf {
    crate::journal::data_dir().join("ffmpeg-dir")
}

/// The chosen folder, if one is saved.
pub fn read_user_choice() -> Option<String> {
    let text = std::fs::read_to_string(user_choice_file()).ok()?;
    let line = text.lines().next()?.trim();
    (!line.is_empty()).then(|| line.to_string())
}

/// Give the search the program's folders. Called at startup, before any source opens.
pub fn install_search_dirs() {
    kt_ffmpeg::set_search_dirs(read_user_choice(), None);
}
