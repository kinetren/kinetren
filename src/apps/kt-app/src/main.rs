//! GUI-редактор Kinotrain. Модули: app (состояние/панели), timeline
//! (таймлайн), util (источники/индекс), audio_out (cpal).
//!
//! Инварианты UI — см. корневой CLAUDE.md (№3, №8).

mod app;
mod audio_out;
mod bin_panel;
mod fx_editor;
mod meters;
mod render_dialog;
mod thumbs;
mod timeline;
mod util;

fn main() -> eframe::Result {
    app::run()
}
