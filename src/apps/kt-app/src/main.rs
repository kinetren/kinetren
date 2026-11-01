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
mod guide;
mod i18n;
#[cfg(test)]
mod i18n_tests;
mod icons;
mod journal;
#[cfg(test)]
mod layout_tests;
mod lessons;
mod meters;
mod morpher_editor;
mod project_lock;
mod plugins;
mod pubparam;
#[cfg(test)]
mod reach_tests;
#[cfg(test)]
mod gen_audio_tests;
#[cfg(test)]
mod segfield_tests;
mod reel_preview;
mod render_dialog;
mod settings_window;
mod supervisor;
mod telemetry;
mod track_window;
mod theme;
mod thumbs;
mod timeline;
mod toolbar;
mod transition_editor;
mod update_check;
mod user_store;
mod welcome;
mod util;
mod widgets;
mod window_frame;
mod window_shell;

fn main() -> eframe::Result {
    // the first process watches the editor and shows the crash window (render-crash-and-recovery)
    if let Some(code) = supervisor::run() {
        std::process::exit(code);
    }
    app::run()
}
