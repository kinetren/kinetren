//! GUI-редактор Kinotrain. Модули: app (состояние/панели), timeline
//! (таймлайн), util (источники/индекс), audio_out (cpal).
//!
//! Инварианты UI — см. корневой CLAUDE.md (№3, №8).

mod app;
mod audio_out;
mod automation;
mod bin_panel;
mod camera_editor;
mod fx_editor;
#[cfg(test)]
mod font_tests;
mod fx_registry;
mod graphs;
mod i18n;
#[cfg(test)]
mod i18n_tests;
mod icons;
#[cfg(test)]
mod layout_tests;
mod meters;
mod morpher_editor;
mod render_dialog;
mod theme;
mod thumbs;
mod timeline;
mod transition_editor;
mod toolbar;
mod util;
mod widgets;

fn main() -> eframe::Result {
    app::run()
}
