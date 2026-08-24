//! GUI-редактор Kinotrain. Модули: app (состояние/панели), timeline
//! (таймлайн), util (источники/индекс), audio_out (cpal).
//!
//! Инварианты UI — см. корневой CLAUDE.md (№3, №8).

mod app;
mod audio_out;
mod bin_panel;
mod fx_editor;
mod fx_registry;
mod graphs;
mod icons;
mod meters;
mod morpher_editor;
mod render_dialog;
mod thumbs;
mod timeline;
mod toolbar;
mod util;

fn main() -> eframe::Result {
    app::run()
}
