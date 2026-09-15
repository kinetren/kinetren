//! GUI-редактор Kinotrain. Модули: app (состояние/панели), timeline
//! (таймлайн), util (источники/индекс), audio_out (cpal).
//!
//! Инварианты UI — см. корневой CLAUDE.md (№3, №8).

mod analysis;
mod app;
mod audio_out;
mod automation;
mod bg;
mod bin_panel;
mod camera_editor;
mod ctx_menu;
mod eq_graph;
#[cfg(test)]
mod font_tests;
mod fx_editor;
mod fx_registry;
mod graphs;
mod i18n;
#[cfg(test)]
mod i18n_tests;
mod icons;
mod journal;
#[cfg(test)]
mod layout_tests;
mod meters;
mod morpher_editor;
mod project_lock;
#[cfg(test)]
mod reach_tests;
mod render_dialog;
mod theme;
mod thumbs;
mod timeline;
mod toolbar;
mod transition_editor;
mod user_store;
mod util;
mod widgets;

fn main() -> eframe::Result {
    app::run()
}
