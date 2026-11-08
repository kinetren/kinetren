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
mod controls;
mod ctx_menu;
#[cfg(test)]
mod controls_tests;
#[cfg(test)]
mod diag_tests;
#[cfg(test)]
mod idle_tests;
#[cfg(test)]
mod ui_wait_tests;
#[cfg(test)]
mod input_work_tests;
#[cfg(test)]
mod frame_cost_tests;
mod envclip;
mod deck_face;
mod eq_graph;
mod ffmpeg_setup;
#[cfg(test)]
mod font_tests;
mod fx_editor;
mod fx_registry;
mod frame_cost;
mod fx_thumbs;
mod gen_ui;
mod geomctl;
mod graphs;
mod guide;
mod i18n;
#[cfg(test)]
mod i18n_tests;
mod icons;
mod isp_panel;
mod journal;
#[cfg(test)]
mod layout_tests;
mod lessons;
mod load_watch;
#[cfg(test)]
mod mask_tests;
mod maskctl;
mod masks;
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
#[cfg(test)]
mod window_fill_tests;
#[cfg(test)]
mod welcome_prefs_tests;
#[cfg(test)]
mod vhs_tape_tests;
#[cfg(test)]
mod pitch_tail_tests;
mod reel_preview;
mod render_dialog;
mod settings_window;
mod prefs_window;
mod preview_display;
mod supervisor;
mod tele_journal;
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
mod wake;
mod window_frame;
mod window_shell;

fn main() -> eframe::Result {
    // frame-sized buffers go back to the system when freed (export-throughput D11)
    kt_media::frame_alloc::return_large_frames_to_os();
    // a throwaway process that meets a hardware driver for the render (hw-encode-context D10)
    kt_media::muxer::hw_trial::serve();
    // the first process watches the editor and shows the crash window (render-crash-and-recovery)
    if let Some(code) = supervisor::run() {
        std::process::exit(code);
    }
    app::run()
}
