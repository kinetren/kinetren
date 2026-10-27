//! GUI editor of Kinetren. Modules: app (state/panels), timeline
//! (the timeline), util (sources/index), audio_out (cpal).
//!
//! UI invariants — see the root CLAUDE.md (No. 3, No. 8).

// A release build on Windows is a GUI program: no console window next to it (windows-build).
// Debug builds keep the console for eprintln diagnostics.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod analysis;
mod app;
mod audio_out;
mod automation;
mod bench;
mod bg;
mod bin_panel;
mod camera_editor;
mod ctx_menu;
mod envclip;
mod eq_graph;
mod ffmpeg_setup;
#[cfg(test)]
mod font_tests;
mod fx_editor;
mod fx_registry;
mod fx_thumbs;
mod gen_ui;
mod geomctl;
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
mod pubparam;
#[cfg(test)]
mod reach_tests;
mod reel_preview;
mod render_dialog;
mod settings_window;
mod telemetry;
mod track_window;
mod theme;
mod thumbs;
mod timeline;
mod toolbar;
mod transition_editor;
mod user_store;
mod util;
mod widgets;
mod window_frame;

fn main() -> eframe::Result {
    app::run()
}
