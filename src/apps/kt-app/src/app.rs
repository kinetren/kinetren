//! KtApp: application state, panels, keys, drops, lifecycle.

use crate::audio_out;
use crate::i18n::{k, tr};
use crate::timeline::{CtxTarget, Drag};
use crate::util::*;
use eframe::egui;
use kt_engine::Player;
use kt_model::*;
use kt_time::Tick;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Notification importance (item 18).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
}

/// A question the program must get answered BEFORE an action (R.49).
/// A separate type, not a flag per case: there will be several questions
/// (a busy project, unsaved work), and they are all drawn in one place —
/// otherwise each would get its own “Cancel” button with its own behaviour.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    /// The project is busy in another copy of the program — write anyway?
    SaveOverLock { pid: Option<u32> },
    /// There is unsaved work in the editor, and the user is leaving.
    Unsaved(Pending),
    /// A FOREIGN PROJECT was dropped onto the window (R.55, C6): open it or insert it
    /// as a composition. The owner’s words: “I drag a project file onto the timeline
    /// and a new timeline opens, but I want it to give me a choice.” Until this round
    /// there was no choice — and, worse, no question about unsaved work either: it
    /// vanished silently. Since R.70 a project is `.kt`;
    /// a file dropped onto the window is not a project,
    /// but a hint about the import door.
    DropProject,
}

/// What the program was going to do when it asked about unsaved changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pending {
    New,
    Quit,
    Open,
    /// IMPORT DOOR (R.67, R.70): legacy `.ktproj`, our old `.otio`,
    /// a foreign `.otio` — the result opens as a document of its own format.
    ImportOlder,
    /// Open a PROJECT DROPPED ONTO THE WINDOW (R.55, C6). The path lives in a separate
    /// field `KtApp.pending_drop`: `Pending` is `Copy`, and a `PathBuf` is not put
    /// into it.
    DropOpen,
}

/// The answer to the question. Three states, not a `bool`: “do not save” and “cancel” are
/// DIFFERENT answers (the first continues the action, the second cancels it), and confusing
/// them means losing work on pressing the “Cancel” button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Yes,
    No,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct Notice {
    pub at: String,
    pub severity: Severity,
    pub text: String,
    /// How many times in a row this same message came (shown as “×N”).
    pub repeats: u32,
}

/// The result of the background reopening: media, a name for the message and the source
/// (None — the file did not open again, the placeholder stays).
pub type ReopenedMedia = (MediaId, String, Option<Box<dyn kt_media::MediaSource>>);

/// Window “what to name the inserted thing” (R.35 item 4).
/// Auto-ducking dialog (R.48, item 341): on which track to measure pauses and
/// how much to attenuate. A snapshot, not a live sidechain — therefore this is an
/// ordinary model edit with a single undo step.
/// Dialog “save the chain as a preset” (R.48, item 344). A preset is a file of the
/// USER, not of the model: project undo does not roll it back.
pub struct PresetSaveDialog {
    /// What we save (the chain at the moment of the press).
    pub block: kt_model::ops::FxChainBlock,
    pub name: String,
    /// Renaming an existing item instead of creating a new one.
    pub rename_of: Option<String>,
    pub focus: bool,
}

/// Level of the path inside nested items (R.50, B1).
///
/// Stores the WHOLE VIEW from the outside: exiting must mean “came back to the same place”,
/// not “ended up at the start of the timeline with an empty selection” (rule 14 —
/// the feature works repeatedly, and rule 5 — an action does not wipe the neighbouring
/// state).
#[derive(Clone)]
pub struct NestStep {
    /// Where we stood BEFORE entering (`None` — the root).
    pub parent: Option<kt_model::MediaId>,
    /// Through which instance we entered: the cursor is translated back by it.
    pub inst: ClipId,
    /// Reel name — for breadcrumbs.
    pub name: String,
    pub view_start: f64,
    pub ticks_per_px: f64,
    pub tl_scroll: f32,
    pub cursor: Tick,
    pub region: Option<kt_model::Region>,
    pub selected: std::collections::BTreeSet<ClipId>,
    /// Selection of service rows. Stored for exactly the same reason as
    /// `selected`: `enter_nest` clears it (inside a reel it is meaningless),
    /// and without restoring, exit stopped being an identity — the rule “a gesture
    /// works repeatedly” broke on the morph layer and the camera.
    pub selected_morph: std::collections::BTreeSet<MorphId>,
    pub selected_camera: std::collections::BTreeSet<kt_model::CameraId>,
    /// An instance EXPANDED in the context being left at the moment of entry (R.58,
    /// item 461) — including `inst` itself if entered through its band with a
    /// second Enter. The owner’s decision of R.58 (revision of R.56 item 5):
    /// restore EVERYTHING, including the entry band — the Enter/Esc cascade
    /// is symmetric. The id is stored, not the `Band`: on return the band is
    /// RE-computed by `expand_band` (the reel’s content could have changed
    /// inside — bases and length differ); if the instance disappeared or stopped being
    /// a nested item — silently without a band.
    pub band: Option<ClipId>,
}

/// Reel name window (R.50). The same form as the preset name window: live
/// preview, Enter/Esc, focus is returned on close (rule 14).
pub struct SeqNameDialog {
    pub seq: kt_model::MediaId,
    pub name: String,
    pub focus: bool,
}

pub struct PasteNamesDialog {
    /// Insert position, frozen at the moment of Ctrl+V.
    pub at: kt_time::Tick,
    /// And the ROW of the insert — for the same reason: under the window the person
    /// may move both the cursor and the mouse (R.56, item 450).
    pub row: Option<(LaneKind, u32)>,
    /// Prefix word and side (R.38 item 19: default is SUFFIX).
    pub affix: String,
    pub before: bool,
    /// Take focus into the field on the first frame — and RETURN it on close
    /// (rule 14: the feature must work repeatedly).
    pub focus: bool,
}

/// What Enter does on the selected nested item (R.56, item 451).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterOnNest {
    /// Collapsed — expand IN PLACE (the default mode).
    Expand,
    /// Already expanded — FULL ENTRY: the reel plays from its zero.
    Enter,
    /// Not a reel — the key has nothing to do here.
    NotANest,
}

/// Selection frame in the model domain: [start, end) and the touched tracks.
pub type Band = (kt_time::Tick, kt_time::Tick, Vec<(LaneKind, u32)>);

pub struct KtApp {
    pub model: TimelineModel,
    pub project_path: Option<PathBuf>,
    /// Edits of CONTENT (clips, FX, morpher, markers, tracks, media):
    /// only they give the asterisk and trigger autosave (item 5 round 11).
    pub dirty_content: bool,
    /// CONVENIENCE edits (region/loop, bin layout, track names,
    /// row order): the user changed nothing — no asterisk,
    /// they reach the disk incidentally on save/exit.
    pub dirty_ui: bool,
    pub player: Player,
    /// Keep-alive handle of the cpal stream (Drop = stops the output).
    #[allow(dead_code)]
    pub audio: Option<audio_out::AudioOutput>,
    pub peaks: Arc<Mutex<HashMap<MediaId, Peaks>>>,
    /// Index service: ONE thread per project with a queue (R.49, item 368).
    /// Previously three tasks pretending to be a service stood in its place.
    pub index: crate::util::IndexService,
    /// Index errors are reported (once per address).
    pub index_errs_said: std::collections::HashSet<String>,
    /// Media loudness: (LUFS, dBTP, LRA). Read from the index DB as it
    /// becomes ready (R.49, item 373).
    pub loudness: Arc<Mutex<crate::util::LoudnessMap>>,
    /// Offline analysis of material (item 372).
    pub analysis: crate::analysis::Analysis,
    /// Ducking intervals computed in the background, and the number of unavailable media.
    /// Ducking settings waiting for their intervals.
    pub texture: Option<egui::TextureHandle>,
    pub view_start: f64, // ticks
    pub ticks_per_px: f64,
    /// Selected events (R.33 item 7): a set, not one — the frame,
    /// Ctrl+click and double-click on a range marker select in batches.
    pub selected: std::collections::BTreeSet<ClipId>,
    /// Selection tool mode (toolbar button, as in Vegas): a plain
    /// drag on empty space draws a frame instead of a region. Session-only —
    /// not written into the project.
    pub sel_tool: bool,
    /// Lock of the automation graph axis — ONE FOR THE WHOLE PROGRAM and
    /// remembered in the project (R.40 item 15, the user’s decision verbatim:
    /// “One for the whole program, remembered in the project”). Until now it lived in
    /// `AutoView` of each window and died with the window.
    pub auto_axis_linked: bool,
    /// Selected morph blocks (H1; priority over the clip on Del).
    ///
    /// A SET, per R.40: it was `Option`, and the selection frame physically did not
    /// fit more than one morpher — and caught zero, because its own catch was a
    /// `Vec<ClipId>`.
    pub selected_morph: BTreeSet<MorphId>,
    /// Selected cameras (R.38 item 5, “I do not understand how the camera track works”):
    /// until that round the camera could be neither selected nor deleted
    /// from the keyboard — only through the context menu. A set — per R.40.
    pub selected_camera: BTreeSet<kt_model::CameraId>,
    /// Placement mode of a morpher (port of pendingMorpherPlacement v1):
    /// frozen targets + view; Esc — cancel.
    pub drag: Option<Drag>,
    /// Main (fixed) playback speed (items 4-5 v1.4).
    pub base_rate: f32,
    /// Position of the shuttle slider while it is held (None = not held).
    pub shuttle_override: Option<f32>,
    /// Which speed-control band is grabbed by a drag: Some(true) = upper
    /// (persistent), Some(false) = lower (shuttle). Items 18-19 round 7.
    pub speed_drag_upper: Option<bool>,
    /// Virtual cursor of the speed band in pixels from its left edge
    /// (R.57): the drag accumulates INCREMENTS, so Shift slows the motion rather than
    /// teleporting the value (`drag_advance`).
    pub speed_drag_vx: f32,
    /// Open speed input field — IN PLACE of the band, not a window (R.57).
    pub speed_entry: Option<SpeedEntry>,
    /// Rectangle of the speed control of the last frame (live `rate.py`).
    pub speed_rect: egui::Rect,
    /// Deferred single click on the preview (item 11 round 7): we wait for the
    /// double-click window, then play/pause.
    pub preview_click_at: Option<std::time::Instant>,
    /// Mouse position during file DnD (X11 polling; None on Wayland).
    pub dnd_hover: Option<egui::Pos2>,
    /// Timeline geometry cache of the last frame (drop, item 2 v1.4).
    pub tl_geom: Option<crate::timeline::TlGeom>,
    /// KT_GEOM_DUMP — path for the geometry dump to live scenarios (R.27).
    pub(crate) geom_dump_path: Option<String>,
    pub(crate) geom_dumped_at: Option<std::time::Instant>,
    /// Display order of tracks (item 6 v1.4); normalized each frame,
    /// stored in the project’s UiState.
    /// The preview is split into its own window (item 7 v1.4, second monitor).
    pub preview_detached: bool,
    /// Economical UI: 30 fps during playback (item 1 round 5, weak GPUs).
    pub ui_economy: bool,
    /// Selected interface language; `None` — “auto” (by OS locale, item 3 round 19).
    /// This is a USER setting: it lives in the eframe Storage, not in
    /// `.ktproj`, and it does not raise the unsaved-changes asterisk.
    pub lang_choice: Option<crate::i18n::Lang>,
    /// Anonymous usage telemetry (change telemetry-posthog). One per PROCESS: File → New/Open
    /// rebuild `KtApp` in place and pick the same value up from `telemetry::global()`, so a
    /// new document never sends a second `app_started`. `None` in the harness.
    pub telemetry: Option<std::sync::Arc<crate::telemetry::Telemetry>>,
    /// Moment of the last pause (double Space = stop, item 15 round 6).
    pub paused_at: Option<std::time::Instant>,
    /// Space held down since this moment (item 3 round 16). None = not held.
    pub space_down_at: Option<std::time::Instant>,
    /// Current speed-up multiplier from holding Space (None = not speeding up).
    pub space_boost: Option<f32>,
    /// The window follows the cursor during play; fades on manual pan/zoom
    /// (item 14 round 6) — used in L2, set in on_play.
    pub follow_playhead: bool,
    /// Moment of the last MANUAL scroll/zoom (item 30 round 7: grace for follow_rearm).
    pub last_user_scroll: Option<std::time::Instant>,
    /// Window drag started from the expanded state: first restore the
    /// window, StartDrag — on the NEXT frame (item 10 round 8, X11).
    pub pending_unmax_drag: bool,
    /// Where to scroll the timeline window on the next frame (item 11 round 9:
    /// jump to the stop point must show the cursor).
    pub pending_reveal: Option<i64>,

    /// Individual row heights (R.38 item 11) — a mirror of

    /// Active inline input for the marker name (item 1 round 10).
    pub marker_edit: Option<MarkerEdit>,
    /// Request focus for the marker name field (once, on open).
    pub marker_edit_focus: bool,
    /// Active inline input for the track name (item 2 round 10).
    pub lane_edit: Option<LaneEdit>,
    pub area_edit: Option<AreaEdit>,
    pub area_edit_focus: bool,
    /// Request focus for the track name field.
    pub lane_edit_focus: bool,
    /// Timecode input in the toolbar (item 4 round 10): Some — the field is open.
    pub tc_edit: Option<String>,
    pub tc_edit_focus: bool,
    /// Project search palette (item 3 round 10).
    pub search_open: bool,
    pub search_focus: bool,
    pub search_query: String,
    pub search_sel: usize,
    /// Vertical scroll of the track zone, px (item 12 round 6).
    pub tl_scroll: f32,
    /// Explicit row height (Shift+Ctrl+wheel); None = auto-fit.
    pub tl_v_zoom: Option<f32>,
    /// Last successful preview bucket in the bin (scrubbing without flicker, item 11 round 6).
    pub bin_last_good: HashMap<MediaId, crate::thumbs::ThumbKey>,
    /// Previous offline snapshot of the bin: detection of offline→online (item 1 round 7).
    pub bin_last_offline: std::collections::HashSet<kt_model::MediaId>,
    /// Media bin: free layout (item 7 round 8) instead of the grid.
    pub bin_free: bool,
    /// Positions of cards in the free layout (live in the project, UiState).
    pub bin_layout: std::collections::HashMap<kt_model::MediaId, egui::Pos2>,
    /// Which card is being dragged inside the bin (free mode).
    pub bin_drag: Option<crate::bin_panel::BinDragState>,
    /// Card groups of the bin (item 13 round 11): they live here; they go into
    /// the project together with the rest of UiState.
    pub bin_groups: Vec<kt_model::BinGroup>,
    /// Card selection (by frame or Ctrl+click) — in this session only.
    pub bin_sel: std::collections::BTreeSet<kt_model::MediaId>,
    /// Start of the selection frame (bin area coordinates).
    pub bin_rubber: Option<egui::Pos2>,
    /// Group rename: index + buffer.
    pub bin_group_edit: Option<(usize, String)>,
    /// The focus in the group name field is requested ONCE (as for markers):
    /// an unconditional request_focus every frame kept the field from losing focus,
    /// and Enter did not save the name at all (complaint of round 13).
    pub bin_group_focus: bool,
    /// Zones where window resize does NOT start (item 9 round 12): window buttons, menu,
    /// bell. The resize zone reads the raw pointer at the end of the frame and
    /// bypasses egui’s hit test, taking the press for itself, so buttons 16 px
    /// from the edge were practically unclickable. The buffer is reused.
    pub no_resize_rects: Vec<egui::Rect>,
    /// Rectangles of the cards of this frame (free mode): needed by the
    /// selection frame and group frames, and also give the harness test a place to aim
    /// the mouse at. The buffer is reused, not allocated per frame.
    pub bin_card_rects: Vec<(kt_model::MediaId, egui::Rect)>,
    /// Fader handle grab: the cursor offset relative to the handle —
    /// we drag “relatively”, without jumping to the position (item 2 round 8).
    pub fader_grab: Option<f32>,
    /// Last sent window title (we send Title only on change).
    pub(crate) last_title: String,
    /// Reusable buffer of the preview composition (no per-frame allocations).
    pub(crate) preview_buf: Vec<u8>,
    /// Throttling of the glow mip: every N new frames (ambient does not flicker).
    glow_skip: u8,
    /// Name of the wgpu adapter/backend (perf panel diagnostics).
    pub gpu_info: Option<String>,
    /// Notice about loss of GPU video rendering was shown (once per session, R.29).
    pub gpu_fx_lost_notified: bool,
    /// Cache of max_source_resolution by model_rev (R.31 item 3: the function opens
    /// ALL media files — it must not be called per frame from the render window).
    pub max_src_cache: Option<(u64, Option<(u32, u32)>)>,
    /// Background autosave (R.31 item 2): (snapshot rev, result). The UI thread
    /// no longer writes the file itself — it only serializes the snapshot and waits for the result.
    pub autosave_rx: Option<(u64, std::sync::mpsc::Receiver<std::io::Result<SaveOutcome>>)>,
    /// Project write guard (R.32): the model revision WRITTEN to disk
    /// last. The background autosave must check against it before replacing the
    /// file — otherwise it would overwrite a newer Ctrl+S with its stale snapshot
    /// (a race found by live scenarios).
    /// The largest event number born in THIS session (item 356).
    pub clip_id_hw: std::cell::Cell<u64>,
    pub saved_rev: std::sync::Arc<std::sync::Mutex<u64>>,
    /// Guard of the RECOVERY file — separate from the project guard (R.49,
    /// item 350). One mutex for two different targets began to cancel Ctrl+S
    /// because of another revision: a background recovery write raised the bar, and
    /// the next explicit project write turned out to be “stale”.
    pub recovery_rev: std::sync::Arc<std::sync::Mutex<u64>>,
    /// Lock on the project file (item 352). `None` — the project is unnamed or
    /// busy with another copy: then `lock_holder` carries its process number.
    pub project_lock: Option<crate::project_lock::ProjectLock>,
    /// Process number of the holder, if the project is busy not by us.
    pub lock_holder: Option<Option<u32>>,
    /// The question “the project is busy — save anyway?” is asked ONCE per
    /// session: afterwards the user has already decided.
    pub lock_override: bool,
    /// The asked and not yet answered question.
    pub ask: Option<Ask>,
    /// Exit is already allowed by the user: the next `close_requested` is not
    /// intercepted, otherwise the window would never close.
    pub quit_confirmed: bool,
    /// Last line of defence is enabled (item 354). Disabled in the harness: a thousand
    /// apps created and destroyed must not scatter recovery files
    /// into the shared data directory.
    pub recovery_on_drop: bool,
    /// Target of the PANEL context menu (R.49, item 363). Mutually cancels with
    /// `ctx_target`: there is one menu on screen.
    pub ctx_panel: Option<crate::ctx_menu::PanelTarget>,
    /// Found unsaved work and the chosen candidate (item 353).
    pub recovery_offer: Vec<RecoveryCandidate>,
    pub recovery_pick: usize,
    pub recovery_open: bool,
    /// Recovery file written by THIS copy of the program. We delete only it on
    /// save: otherwise Ctrl+S in one copy would kill the recovery of another
    /// (R.49, item 351).
    pub recovery_written: Option<PathBuf>,
    pub snap_line: Option<i64>,
    /// (transport generation, number of loop jumps) on the previous frame of the
    /// preview. A change of either is a legitimate reason for the display key to go back:
    /// seek bumps the generation, while loop-wrap deliberately does NOT bump it (audio
    /// and FX tails continue), so it has its own counter — without it, on the second lap of the
    /// loop the picture froze (item 14 round 14, round 15 item 2).
    pub preview_gen: (u64, u64),
    /// Magnet temporarily off (Ctrl held during dragging, item 2 round 11).
    /// Useful behaviour is the default; the modifier TURNS IT OFF.
    pub snap_off: bool,
    /// Last playback direction (item 5 round 13): the cursor trail
    /// glows behind it, and on pause it must not jump from side to side —
    /// we keep the sign of the last non-zero speed.
    pub play_dir_back: bool,
    /// Width of the meters panel (dragged by its own strip, item 6 round 13). “Convenience”:
    /// goes into UiState, does not light the asterisk.
    pub meters_w: f32,
    /// The user’s workspace (R.49, item 360): meter width and view by
    /// projects live HERE, not in someone else’s project file.
    pub workspace: crate::user_store::Workspace,
    /// Interval of the background recovery write. A separate field for the sake of live
    /// scenarios (`KT_AUTOSAVE_SECS`): waiting half a minute in each run
    /// would be dishonest toward the check, while faking the timer from inside would mean
    /// checking not the program the user runs.
    pub autosave_secs: u64,
    /// Where to write the workspace. `None` — the user’s shared file; in the
    /// harness and live scenarios it is substituted, otherwise a hundred and fifty tests
    /// would write into the developer’s home directory.
    pub workspace_file: Option<PathBuf>,
    /// View, READ from the project file. Written back as is:
    /// byte-stability of someone else’s file is intact, and our view edits go to the
    /// workspace.
    pub ui_passthrough: Option<kt_model::UiState>,
    pub ctx_target: Option<CtxTarget>,
    /// Timeline row under the right click (item 5 round 16): a new track from the
    /// context menu is placed RIGHT BELOW it, not at the very bottom.
    pub ctx_row: Option<(LaneKind, u32)>,
    /// TIME of the right click (R.56, item 450): “Paste” from the menu puts the block
    /// where the person pointed, not where the cursor is.
    pub ctx_at: Option<kt_time::Tick>,
    /// Live volume of the volume rail until release (id, dB).
    pub live_gain: Option<(ClipId, f32)>,
    /// Live preview of video level during line drag (R.28, item 12).
    pub live_opacity: Option<(ClipId, f32)>,
    pub status: Option<(String, std::time::Instant)>,
    /// Media for which a placeholder now stands (R.33 item 10): the file did not
    /// open when the project loaded. Empties when the file returns.
    pub placeholder_media: std::collections::HashSet<MediaId>,
    /// Reopening of a returned file runs in the background (FFmpeg opening
    /// is blocking — in the UI thread it is a noticeable pause).
    pub reopen_rx: Option<std::sync::mpsc::Receiver<ReopenedMedia>>,
    /// Internal clipboard (block of clips+transitions, rebased to zero).
    pub clipboard: kt_model::ops::ClipBlock,
    /// Passthrough remainders of an imported exchange file (foreign fields/schemas/
    /// metadata): they travel in the project file in the `otio` field and come back on
    /// export, outside undo (R.24, R.70).
    pub retained: kt_otio::Retained,
    /// Where to offer to save for an unnamed document (R.70): after
    /// importing `film.otio` — `film.kt` next to the source. The project path itself
    /// stays empty: writing `.kt` under a foreign name is not allowed, and silently placing
    /// the file next to it is no better; the person chooses the name in the dialog.
    pub save_hint: Option<PathBuf>,
    /// Requested READ of the system envelope buffer (R.61).
    ///
    /// There is NO LONGER a mirror of the buffer inside the program: it blocked the button on
    /// content copied in another window, and forbade pasting into a foreign effect view.
    /// What lives here is not the content but the final task “go read it” — on X11 a
    /// synchronous read of one’s own buffer would hang in the event loop
    /// that is drawing at that moment.
    pub env_read: Option<crate::envclip::ClipJob>,
    /// The remainder of matching that the person decides: curves whose names the
    /// target lacks, and the target’s free parameters.
    pub env_fit: Option<crate::envclip::FitDialog>,
    /// FX chain/slot buffer.
    pub fx_clipboard: Option<kt_model::ops::FxChainBlock>,
    /// Media whose files are now unavailable (watchdog, 900 ms).
    pub offline: Arc<Mutex<std::collections::HashSet<MediaId>>>,
    /// Drop stops the watchdog thread (reopening without leaks).
    #[allow(dead_code)]
    pub watchdog: Option<crate::util::WatchdogHandle>,
    pub meter: crate::meters::MeterState,
    pub smooth_scaling: bool,
    pub fx_editors: Vec<crate::fx_editor::FxEditor>,
    /// Transition windows (R.33 C4) — one per transition.
    pub transition_editors: Vec<crate::transition_editor::TransitionEditor>,
    /// Media paths for which the player has raised sources — a snapshot for
    /// `sync_media_sources` (R.52): a change of path (generator window,
    /// undo/redo) is visible as a diff against it.
    pub(crate) media_paths: HashMap<MediaId, String>,
    /// Generator values for which EVENT SOURCES have been raised
    /// (R.59, stage 2). From this round the parameters live on the event, and
    /// the source address is computed from it (`kt_engine::clip_source_addr`),
    /// so the snapshot is keyed by the event, not by the media.
    pub(crate) gen_states: HashMap<ClipId, kt_model::GenState>,
    /// The first sync call takes the build_player state as given.
    pub(crate) media_synced: bool,
    /// Model revision at which the sync ran (R.53, item 413): the path map
    /// is not rebuilt while the model has not changed.
    pub(crate) media_synced_rev: u64,
    /// How many times the path map was actually rebuilt (guard t503).
    pub(crate) media_sync_builds: u64,
    /// Font families whose substitution has already been announced (R.53, item 413):
    /// otherwise, with a missing font, each edit of a title’s path gave its own
    /// notice — during a drag that is a storm of identical messages.
    pub(crate) font_warned: std::collections::HashSet<String>,
    /// Camera windows (R.34 K3).
    pub camera_editors: Vec<crate::camera_editor::CameraEditor>,
    pub morph_editors: Vec<crate::morpher_editor::MorphEditor>,
    pub gesture_counter: u64,
    pub render_job: Option<crate::render_dialog::RenderJob>,
    pub render_setup: crate::render_dialog::RenderSetup,
    pub last_autosave: std::time::Instant,
    /// Hint of the hovered toolbar button — duplicated in the status bar (port).
    pub hover_hint: Option<String>,
    /// Refusal of an action outside the timeline (R.62): shown as an ERROR in the
    /// status line, like the gesture’s stop (`blocked`), with the same TTL as the status.
    pub error_status: Option<(String, std::time::Instant)>,
    /// Obstacle of the current gesture (item 5 R.28): highlighted on the timeline,
    /// the hint is in the ERROR style and says where to look.
    pub blocked: Option<crate::timeline::Blocked>,
    /// END OF MATERIAL that the trim ran into (R.63 item 2): the tick where
    /// the file ends. An obstacle here is not an event but the edge of the source —
    /// there is nothing to highlight, but we MUST say it (rule 16), otherwise
    /// the edge simply stops moving and this looks like the mouse lagging.
    pub blocked_material: Option<i64>,
    /// A LOCKED track that the gesture ran into (R.62): highlighted in red for exactly one frame,
    /// like any other obstacle.
    /// The locked track that the gesture ran into, and the MOMENT of refusal. Time is not
    /// for beauty here: refusal also comes from a KEY (delete, cut), and `handle_keys` runs before
    /// the timeline is drawn — if the highlight went out every frame, the person would see it
    /// for exactly one frame, that is, never.
    /// It lives as long as the refusal text in the status line
    /// (`STATUS_TTL_MS`).
    pub blocked_track: Option<(kt_model::TrackId, std::time::Instant)>,
    /// The last selection frame in the MODEL DOMAIN (time × track keys)
    /// — from it a named area is made (R.35 item 2). Session-only:
    /// it has no need to survive a restart.
    pub last_band: Option<Band>,
    /// Insert prefix window (R.35 item 4). Non-blocking, with a FROZEN
    /// insert position: the cursor under the window does not move it.
    pub paste_names: Option<PasteNamesDialog>,
    /// Timeline row UNDER THE MOUSE (R.56, item 450) — only while the pointer is
    /// over the timeline. Ctrl+V puts the block here: keyboard paste
    /// must land in the same place the menu paste would.
    pub tl_hover_row: Option<(LaneKind, u32)>,
    pub preset_save: Option<PresetSaveDialog>,
    /// “Project settings” window (R.62).
    pub settings_win: Option<crate::settings_window::SettingsWindow>,
    /// TRACK window (R.62): properties and chain. One per program — like the
    /// settings window; a second window for the same track is not opened.
    pub track_win: Option<crate::track_window::TrackWindow>,
    /// Rename window of a nested reel (R.50).
    pub seq_name: Option<SeqNameDialog>,
    /// Dialog for the name of the published handle (R.59, stage 3).
    pub publish_dialog: Option<crate::pubparam::PublishDialog>,
    /// “Public parameters” window of a reel — the AUTHOR side. One per
    /// program: the set is declared at the reel you are standing in.
    pub published_win: Option<crate::pubparam::PublishedWin>,
    /// “Composition parameters” windows of instances — the CONSUMER side.
    /// A list, not one: comparing two releases side by side is a normal task.
    /// Path inside nested items: one level per input (R.50, B1).
    pub nest_path: Vec<NestStep>,
    /// Band expanded INSIDE the reel at the moment of leaving it (R.58,
    /// item 461): returned to the same reel — expansion in place. A pair for
    /// `NestStep::band` (that one remembers the band of the LEAVING context at
    /// entry, this one — at exit). The id is stored: the band is recomputed.
    pub reel_bands: std::collections::HashMap<kt_model::MediaId, ClipId>,
    /// Reels about which “stands in N places” has already been said (R.55, C5).
    /// PER REEL, not for the whole session: two different shared reels — two
    /// different warnings, and that is honest. Not written to the file.
    pub shared_warned: std::collections::HashSet<kt_model::MediaId>,
    /// Project dropped onto the window: path and drop location (R.55, C6).
    pub pending_drop: Option<(PathBuf, Option<(Tick, (ClipKind, u32))>)>,
    /// FX preset library (R.48): a user file shared by all
    /// projects. Loaded lazily on first access.
    pub presets: crate::user_store::FxPresetStore,
    /// State of the preset library already shown (once per session).
    pub presets_reported: bool,
    /// The impossibility of resetting the catalog was reported (once per session).
    pub dir_sync_warned: bool,
    /// Rectangle of the status line (filled when it is drawn).
    pub statusbar_rect: egui::Rect,
    /// What is actually shown in the hint line (for the harness test).
    pub status_line_shown: (String, HintKind),
    /// Timeline rect of the previous frame (for the floating toolbar).
    pub timeline_rect: egui::Rect,
    /// Notification retrospective (item 18): ring + unread counter;
    /// the window opens with the bell in the status bar.
    pub notices: std::collections::VecDeque<Notice>,
    pub notices_unread: usize,
    pub notices_open: bool,
    /// View → FFmpeg… (windows-build): where FFmpeg was found, Download/Locate/Rescan.
    pub ffmpeg_ui: crate::ffmpeg_setup::FfmpegUi,
    /// Soft renderer (llvmpipe/Cpu) — detected once (items 17, 19).
    pub sw_render: Option<bool>,
    pub perf: PerfState,
    /// Key of the last preview frame UPLOADED to the texture: (playhead tick,
    /// quality, whether complete). The texture is updated only on change —
    /// earlier the frame was pushed to the GPU 60 times/s even when paused (item 1).
    pub preview_key: Option<(i64, (u32, u32), bool)>,
    /// Ongoing gesture of the on-screen geometry controls (R.64).
    pub geom_drag: Option<crate::geomctl::GeomDrag>,
    /// Active set of on-screen controls (mirror of `UiState.geom_controls`).
    pub geom_ctl: crate::geomctl::GeomMode,
    /// The controls reached gesture parsing in this frame (the scene is not degenerate).
    pub geom_ctl_drawn: bool,
    /// Version of the CONTENT of the sources at the last composite
    /// (R.53, item 412): the preview key must account for it, otherwise an edit of
    /// parameters with a still cursor does not reach the screen.
    pub preview_content: u64,
    /// PROJECT version in the player at the last composite (R.62,
    /// t722b): a model edit while paused — a conform rule, an FX parameter,
    /// a level — moves neither the cursor, nor the quality, nor the content of the
    /// sources, and without this field the preview hung with the old frame until the
    /// first movement of the playhead (found by the live scenario `conform`).
    pub preview_project: u64,
    /// When the preview first became incomplete (the “Loading” scrim — after 180 ms,
    /// as in v1, so it does not flicker on quick misses; item 14).
    pub incomplete_since: Option<std::time::Instant>,
    /// Last attempt of a composite of an incomplete frame (retry throttling, item 1).
    pub last_compose_retry: std::time::Instant,
    /// Blurred mini-copy of the frame for the ambient glow (item 18): 32×18,
    /// updated together with the frame, drawn by LINEAR stretching.
    pub glow_texture: Option<egui::TextureHandle>,
    /// Rect of the preview of the previous frame (for the glow overlay above the UI, item 8).
    pub preview_rect: egui::Rect,
    /// Rectangle of the PICTURE ITSELF inside the preview (letterbox margins already cut):
    /// the “canvas → screen” scale is computed from it, not from the panel.
    pub preview_img_rect: egui::Rect,
    /// Rectangle of the media bin of the previous frame (R.49, item 365):
    /// it catches a right click on the free background.
    pub bin_rect: egui::Rect,
    /// Rect of the meters of the previous frame + colour of the dominant zone.
    /// (meters panel, intensity 0..1, dB level) — the overlay paints the halo
    /// by height ZONES (item 8 v1.4), not in a single colour.
    pub meter_glow: Option<(egui::Rect, f32, f32)>,
    pub thumbs: crate::thumbs::ThumbService,
    /// Effect thumbnails for the picker (R.54): their own service, because
    /// it computes not the source frame but the chain ON the frame (item 430).
    pub fx_thumbs: crate::fx_thumbs::FxThumbService,
    /// Filmstrip textures (LRU is held by ThumbService, here are egui handles).
    pub thumb_textures: HashMap<crate::thumbs::ThumbKey, egui::TextureHandle>,
    /// Texture insertion order — LRU eviction instead of clear() (item 2).
    pub thumb_lru: std::collections::VecDeque<crate::thumbs::ThumbKey>,
    /// Model version (bumped in apply/undo/redo) — the key of the drawing caches (item 1).
    pub model_rev: u64,
    /// Cache of coverage polls: (cache.epoch, model_rev, quality) → per-clip
    /// runs. Previously draw_timeline took the cache lock TWICE per clip per frame.
    pub coverage_cache: CoverageCache,
    /// Cache of clip names/name suppression by model_rev (there were 2 HashMaps plus
    /// O(n²) per frame).
    pub names_cache: NamesCache,
    /// “Places in the edit” per media, by model revision (R.53, item 416).
    uses_cache: (Option<u64>, Arc<HashMap<MediaId, usize>>),
    /// Time-stretch maps per clip, by model_rev (R.37 item 3).
    /// The preview must compute the position with the SAME map as the render, but
    /// building the profile per pixel column — an allocation in the draw path
    /// (invariant #12), so once per revision.
    pub warp_cache: WarpCache,
    /// Preview of nested reels: frames and envelope (R.56, item 452).
    pub reel_previews: crate::reel_preview::ReelPreviews,
    /// Frame textures of the band: (reel key, frame index).
    pub reel_textures: HashMap<(crate::reel_preview::ReelKey, usize), egui::TextureHandle>,
    /// Reel REVISIONS on the current model (preview key). Computed by
    /// `model_rev`, like the warp maps: `content_hash` walks the reel content, and
    /// doing that every UI frame is pointless.
    pub reel_hash_cache: (Option<u64>, Arc<HashMap<ClipId, crate::reel_preview::ReelKey>>),
}

impl KtApp {
    fn empty_project() -> Project {
        Project::new(ProjectSettings {
            fps: 30,
            width: 1280,
            height: 720,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        })
    }

    /// R.29: assemble the GPU video-FX engine on the live eframe device and
    /// inject it into the player. Called from the first frame of ui() — and therefore
    /// repeated for each new KtApp (opening a project recreates both the player and
    /// the real detection). `KT_VIDEO_GPU=0` — forced CPU (debugging/comparison);
    /// a creation failure — notice + CPU.
    fn init_video_gpu(&mut self, frame: &eframe::Frame) {
        if std::env::var("KT_VIDEO_GPU").as_deref() == Ok("0") {
            return;
        }
        let built = match frame.wgpu_render_state() {
            None => Err("no wgpu render state".to_string()),
            Some(rs) => kt_gpu::GpuFxEngine::new(kt_gpu::GpuContext::from_parts(
                std::sync::Arc::new(rs.device.clone()),
                std::sync::Arc::new(rs.queue.clone()),
                rs.adapter.get_info(),
            ))
            .map_err(|e| e.to_string()),
        };
        match built {
            Ok(engine) => self.player.set_video_gpu(Some(engine)),
            Err(e) => self.warn(crate::tf!(k::msg::VIDEO_FX_CPU, e)),
        }
    }

    /// Player + list of UNAVAILABLE media (id, name, error). An unavailable
    /// file gets a placeholder and moves on: the project opens, the preview
    /// and render work (R.33 item 10). Previously all errors were lost here
    /// except the last one, and the clip stayed with no source at all — the preview
    /// showed an eternal “Loading”.
    fn build_player(
        project: &Project,
        base: Option<&Path>,
        index_db: Option<&Path>,
    ) -> (
        Player,
        Option<audio_out::AudioOutput>,
        Vec<(MediaId, String, String)>,
        Vec<(MediaId, kt_time::Tick)>,
    ) {
        let mut sources = Vec::new();
        let mut missing = Vec::new();
        // MEDIA LENGTHS (R.63 item 2): only the one who opened the source knows them, and at START
        // that is here. Without this the model would learn them only at the next path change —
        // that is, for an open project never, and the tail past the end of the file
        // would not differ from anything else.
        let mut lens: Vec<(MediaId, kt_time::Tick)> = Vec::new();
        for m in &project.media {
            let label = crate::util::media_label(&m.path);
            let (src, err) = crate::util::open_source_or_placeholder(
                &resolve_media_path(base, &m.path),
                index_db,
                &label,
            );
            if let Some(e) = err {
                missing.push((m.id, label, e));
            }
            lens.push((m.id, src.info().duration));
            sources.push((m.id, src));
        }
        let (player, ring) = Player::new(project.clone(), sources);
        // “black box” of the panic hook (item 355). Reassigned on every recreation of the application:
        // the dump must be from the LIVE player.
        crate::journal::set_flight(player.flight.clone());
        let audio = match audio_out::start(ring, player.shared.clone(), player.transport.clone()) {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("audio output unavailable: {e}");
                None
            }
        };
        (player, audio, missing, lens)
    }

    pub(crate) fn new_empty(error: Option<String>) -> KtApp {
        let startup_error = error;
        let project = Self::empty_project();
        let project2 = project.clone();
        let (player, audio, _, _) = Self::build_player(&project, None, None);
        let peaks: Arc<Mutex<HashMap<MediaId, Peaks>>> = Arc::new(Mutex::new(HashMap::new()));
        let loudness_map: Arc<Mutex<crate::util::LoudnessMap>> =
            Arc::new(Mutex::new(Default::default()));
        let index = crate::util::IndexService::start(peaks.clone(), loudness_map.clone());
        let mut app = KtApp {
            model: TimelineModel::new(project),
            project_path: None,
            dirty_content: false,
            dirty_ui: false,
            player,
            audio,
            peaks,
            index,
            index_errs_said: Default::default(),
            loudness: loudness_map,
            analysis: Default::default(),
            texture: None,
            view_start: 0.0,
            ticks_per_px: 900.0,
            selected: std::collections::BTreeSet::new(),
            sel_tool: false,
            auto_axis_linked: Self::default_axis_linked(),
            selected_morph: BTreeSet::new(),
            selected_camera: BTreeSet::new(),
            drag: None,
            base_rate: 1.0,
            shuttle_override: None,
            speed_drag_upper: None,
            speed_drag_vx: 0.0,
            speed_entry: None,
            speed_rect: egui::Rect::NOTHING,
            preview_click_at: None,
            dnd_hover: None,
            tl_geom: None,
            geom_dump_path: std::env::var("KT_GEOM_DUMP").ok(),
            geom_dumped_at: None,
            // KT_DETACH_PREVIEW=1 — start with the preview detached (debugging)
            preview_detached: std::env::var_os("KT_DETACH_PREVIEW").is_some(),
            ui_economy: false,
            lang_choice: None,
            telemetry: crate::telemetry::global(),
            paused_at: None,
            space_down_at: None,
            space_boost: None,
            follow_playhead: false,
            last_user_scroll: None,
            pending_unmax_drag: false,
            pending_reveal: None,
            marker_edit: None,
            marker_edit_focus: false,
            lane_edit: None,
            area_edit: None,
            area_edit_focus: false,
            lane_edit_focus: false,
            tc_edit: None,
            tc_edit_focus: false,
            search_open: false,
            search_focus: false,
            search_query: String::new(),
            search_sel: 0,
            tl_scroll: 0.0,
            tl_v_zoom: None,
            bin_last_good: HashMap::new(),
            bin_last_offline: std::collections::HashSet::new(),
            bin_free: true,
            bin_layout: std::collections::HashMap::new(),
            bin_drag: None,
            bin_groups: Vec::new(),
            bin_sel: Default::default(),
            bin_rubber: None,
            bin_group_edit: None,
            bin_group_focus: false,
            no_resize_rects: Vec::new(),
            bin_card_rects: Vec::new(),
            fader_grab: None,
            last_title: String::new(),
            preview_buf: Vec::new(),
            glow_skip: 0,
            gpu_info: None,
            gpu_fx_lost_notified: false,
            max_src_cache: None,
            autosave_rx: None,
            clip_id_hw: std::cell::Cell::new(0),
            saved_rev: std::sync::Arc::new(std::sync::Mutex::new(0)),
            recovery_rev: std::sync::Arc::new(std::sync::Mutex::new(0)),
            project_lock: None,
            lock_holder: None,
            lock_override: false,
            ask: None,
            quit_confirmed: false,
            recovery_on_drop: true,
            ctx_panel: None,
            recovery_offer: Vec::new(),
            recovery_pick: 0,
            recovery_open: false,
            recovery_written: None,
            save_hint: None,
            snap_line: None,
            preview_gen: (0, 0),
            snap_off: false,
            play_dir_back: false,
            meters_w: 70.0,
            workspace: crate::user_store::Workspace::default(),
            workspace_file: None,
            autosave_secs: std::env::var("KT_AUTOSAVE_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v| *v > 0)
                .unwrap_or(AUTOSAVE_SECS),
            ui_passthrough: None,
            ctx_target: None,
            ctx_row: None,
            ctx_at: None,
            live_gain: None,
            live_opacity: None,
            status: None,
            placeholder_media: Default::default(),
            reopen_rx: None,
            clipboard: kt_model::ops::ClipBlock::default(),
            retained: kt_otio::Retained::default(),
            env_read: None,
            env_fit: None,
            fx_clipboard: None,
            offline: Arc::new(Mutex::new(std::collections::HashSet::new())),
            watchdog: None,
            meter: Default::default(),
            smooth_scaling: true,
            glow_texture: None,
            preview_rect: egui::Rect::NOTHING,
            preview_img_rect: egui::Rect::NOTHING,
            bin_rect: egui::Rect::NOTHING,
            meter_glow: None,
            thumbs: crate::thumbs::ThumbService::start(&project2, None),
            fx_thumbs: crate::fx_thumbs::FxThumbService::start(),
            fx_editors: Vec::new(),
            transition_editors: Vec::new(),
            media_paths: HashMap::new(),
            gen_states: HashMap::new(),
            media_synced: false,
            media_synced_rev: 0,
            media_sync_builds: 0,
            font_warned: std::collections::HashSet::new(),
            camera_editors: Vec::new(),
            morph_editors: Vec::new(),
            gesture_counter: 0,
            render_job: None,
            render_setup: Default::default(),
            last_autosave: std::time::Instant::now(),
            hover_hint: None,
            error_status: None,
            blocked: None,
            blocked_material: None,
            blocked_track: None,
            last_band: None,
            paste_names: None,
            tl_hover_row: None,
            preset_save: None,
            settings_win: None,
            track_win: None,
            seq_name: None,
            publish_dialog: None,
            published_win: None,
            nest_path: Vec::new(),
            reel_bands: std::collections::HashMap::new(),
            shared_warned: Default::default(),
            pending_drop: None,
            presets: Default::default(),
            presets_reported: false,
            dir_sync_warned: false,
            statusbar_rect: egui::Rect::NOTHING,
            status_line_shown: (String::new(), HintKind::Idle),
            timeline_rect: egui::Rect::NOTHING,
            notices: std::collections::VecDeque::new(),
            notices_unread: 0,
            notices_open: false,
            ffmpeg_ui: Default::default(),
            sw_render: None,
            perf: PerfState::new(),
            preview_key: None,
            geom_drag: None,
            geom_ctl: Self::default_geom_mode(None),
            geom_ctl_drawn: false,
            preview_content: 0,
            preview_project: 0,
            incomplete_since: None,
            last_compose_retry: std::time::Instant::now(),
            thumb_textures: HashMap::new(),
            thumb_lru: std::collections::VecDeque::new(),
            model_rev: 0,
            coverage_cache: Default::default(),
            names_cache: Default::default(),
            uses_cache: Default::default(),
            warp_cache: Default::default(),
            reel_previews: Default::default(),
            reel_textures: HashMap::new(),
            reel_hash_cache: (None, Arc::new(HashMap::new())),
        };
        // failure to open a file at startup is an ERROR, not a line for five
        // seconds (rule 16): the person reads it in the bell, when the hover hint
        // has already covered the status line (R.67)
        if let Some(e) = startup_error {
            app.refuse(e);
        }
        app
    }

    pub(crate) fn open(path: &Path) -> Result<KtApp, String> {
        let (project, retained, notices) = load_project_file(path)?;
        Self::open_loaded(path, project, retained, notices)
    }

    /// IMPORT DOOR (R.67, R.70): legacy `.ktproj`, our `.otio` of any
    /// envelope version (repairs of old forms are done by `kt_otio::import_older`)
    /// and a foreign `.otio`. The result is a document of ITS OWN format without a name, with
    /// the edit flag raised and a hint “save as `<name>.kt` next to it”;
    /// the source file is never touched by the import. A project file does not go here —
    /// it has the “Open” door.
    pub(crate) fn open_older(path: &Path) -> Result<KtApp, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        // version — for the notification: the legacy file carries its own, our envelope — kt.v,
        // a foreign file — 0
        let version = match kt_otio::detect(&text) {
            Ok(kt_otio::FileFormat::LegacyKtproj) => kt_model::FORMAT_VERSION,
            Ok(kt_otio::FileFormat::FormerProject) => kt_model::project_file::PROJECT_FILE_VERSION,
            _ => kt_otio::parse_document(&text)
                .ok()
                .and_then(|tl| kt_otio::kt_version(&tl))
                .unwrap_or(0),
        };
        let b = match kt_otio::import_interchange(&text, path.parent()) {
            Ok(b) => b,
            Err(kt_otio::Error::ProjectFile) => {
                return Err(crate::tf!(k::msg::ALREADY_PROJECT, tr(k::menu::file::OPEN)));
            }
            Err(e) => return Err(e.to_string()),
        };
        let mut app = Self::open_loaded(path, b.project, b.retained, b.notices)?;
        app.project_path = None;
        app.save_hint = Some(path.with_extension(kt_model::project_file::EXTENSION));
        app.dirty_content = true;
        app.note(crate::tf!(k::msg::IMPORT_OLDER_DONE, version));
        Ok(app)
    }

    /// Text of the project file for this document: the model plus the foreign
    /// passthrough, if there is one. ONE body for Ctrl+S, autosave and `Drop`.
    /// `base_dir` — the file’s directory, relative to which media paths are written.
    pub(crate) fn project_text(&self, doc: &Project, base_dir: Option<&Path>) -> String {
        project_text_of(doc, &self.retained, base_dir)
    }

    /// Common body of `open`/`open_older`: the project has already been read.
    fn open_loaded(
        path: &Path,
        project: Project,
        retained: kt_otio::Retained,
        notices: Vec<String>,
    ) -> Result<KtApp, String> {
        let base = path.parent();
        let (player, audio, err, media_lens) =
            Self::build_player(&project, base, Some(&path.with_extension("ktindex.db")));
        let (offline, watchdog) = spawn_media_watchdog(&project, base);
        let thumbs_svc = crate::thumbs::ThumbService::start(&project, Some(path.to_path_buf()));
        let peaks = Arc::new(Mutex::new(HashMap::new()));
        let media: Vec<_> =
            project.media.iter().map(|m| (m.id, resolve_media_path(base, &m.path))).collect();
        let loudness_map: Arc<Mutex<crate::util::LoudnessMap>> =
            Arc::new(Mutex::new(Default::default()));
        let index = crate::util::IndexService::start(peaks.clone(), loudness_map.clone());
        index.enqueue(Some(path.with_extension("ktindex.db")), media);
        let ui_state = project.ui.clone();
        let mut app = KtApp {
            model: TimelineModel::new(project),
            project_path: Some(path.to_path_buf()),
            dirty_content: false,
            dirty_ui: false,
            player,
            audio,
            peaks,
            index,
            index_errs_said: Default::default(),
            loudness: loudness_map,
            analysis: Default::default(),
            texture: None,
            view_start: 0.0,
            ticks_per_px: 900.0,
            selected: std::collections::BTreeSet::new(),
            sel_tool: false,
            auto_axis_linked: Self::default_axis_linked(),
            selected_morph: BTreeSet::new(),
            selected_camera: BTreeSet::new(),
            drag: None,
            base_rate: 1.0,
            shuttle_override: None,
            speed_drag_upper: None,
            speed_drag_vx: 0.0,
            speed_entry: None,
            speed_rect: egui::Rect::NOTHING,
            preview_click_at: None,
            dnd_hover: None,
            tl_geom: None,
            geom_dump_path: std::env::var("KT_GEOM_DUMP").ok(),
            geom_dumped_at: None,
            // KT_DETACH_PREVIEW=1 — start with the preview detached (debugging)
            preview_detached: std::env::var_os("KT_DETACH_PREVIEW").is_some(),
            ui_economy: false,
            lang_choice: None,
            telemetry: crate::telemetry::global(),
            paused_at: None,
            space_down_at: None,
            space_boost: None,
            follow_playhead: false,
            last_user_scroll: None,
            pending_unmax_drag: false,
            pending_reveal: None,
            marker_edit: None,
            marker_edit_focus: false,
            lane_edit: None,
            area_edit: None,
            area_edit_focus: false,
            lane_edit_focus: false,
            tc_edit: None,
            tc_edit_focus: false,
            search_open: false,
            search_focus: false,
            search_query: String::new(),
            search_sel: 0,
            tl_scroll: 0.0,
            tl_v_zoom: None,
            bin_last_good: HashMap::new(),
            bin_last_offline: std::collections::HashSet::new(),
            bin_free: true,
            bin_layout: std::collections::HashMap::new(),
            bin_drag: None,
            bin_groups: Vec::new(),
            bin_sel: Default::default(),
            bin_rubber: None,
            bin_group_edit: None,
            bin_group_focus: false,
            no_resize_rects: Vec::new(),
            bin_card_rects: Vec::new(),
            fader_grab: None,
            last_title: String::new(),
            preview_buf: Vec::new(),
            glow_skip: 0,
            gpu_info: None,
            gpu_fx_lost_notified: false,
            max_src_cache: None,
            autosave_rx: None,
            clip_id_hw: std::cell::Cell::new(0),
            saved_rev: std::sync::Arc::new(std::sync::Mutex::new(0)),
            recovery_rev: std::sync::Arc::new(std::sync::Mutex::new(0)),
            project_lock: None,
            lock_holder: None,
            lock_override: false,
            ask: None,
            quit_confirmed: false,
            recovery_on_drop: true,
            ctx_panel: None,
            recovery_offer: Vec::new(),
            recovery_pick: 0,
            recovery_open: false,
            recovery_written: None,
            save_hint: None,
            snap_line: None,
            preview_gen: (0, 0),
            snap_off: false,
            play_dir_back: false,
            meters_w: 70.0,
            workspace: crate::user_store::Workspace::default(),
            workspace_file: None,
            autosave_secs: std::env::var("KT_AUTOSAVE_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v| *v > 0)
                .unwrap_or(AUTOSAVE_SECS),
            ui_passthrough: None,
            ctx_target: None,
            ctx_row: None,
            ctx_at: None,
            live_gain: None,
            live_opacity: None,
            status: None,
            placeholder_media: err.iter().map(|(id, _, _)| *id).collect(),
            reopen_rx: None,
            clipboard: kt_model::ops::ClipBlock::default(),
            retained,
            env_read: None,
            env_fit: None,
            fx_clipboard: None,
            offline,
            watchdog: Some(watchdog),
            meter: Default::default(),
            smooth_scaling: true,
            glow_texture: None,
            preview_rect: egui::Rect::NOTHING,
            preview_img_rect: egui::Rect::NOTHING,
            bin_rect: egui::Rect::NOTHING,
            meter_glow: None,
            thumbs: thumbs_svc,
            fx_thumbs: crate::fx_thumbs::FxThumbService::start(),
            fx_editors: Vec::new(),
            transition_editors: Vec::new(),
            media_paths: HashMap::new(),
            gen_states: HashMap::new(),
            media_synced: false,
            media_synced_rev: 0,
            media_sync_builds: 0,
            font_warned: std::collections::HashSet::new(),
            camera_editors: Vec::new(),
            morph_editors: Vec::new(),
            gesture_counter: 0,
            render_job: None,
            render_setup: Default::default(),
            last_autosave: std::time::Instant::now(),
            hover_hint: None,
            error_status: None,
            blocked: None,
            blocked_material: None,
            blocked_track: None,
            last_band: None,
            paste_names: None,
            tl_hover_row: None,
            preset_save: None,
            settings_win: None,
            track_win: None,
            seq_name: None,
            publish_dialog: None,
            published_win: None,
            nest_path: Vec::new(),
            reel_bands: std::collections::HashMap::new(),
            shared_warned: Default::default(),
            pending_drop: None,
            presets: Default::default(),
            presets_reported: false,
            dir_sync_warned: false,
            statusbar_rect: egui::Rect::NOTHING,
            status_line_shown: (String::new(), HintKind::Idle),
            timeline_rect: egui::Rect::NOTHING,
            notices: std::collections::VecDeque::new(),
            notices_unread: 0,
            notices_open: false,
            ffmpeg_ui: Default::default(),
            sw_render: None,
            perf: PerfState::new(),
            preview_key: None,
            geom_drag: None,
            geom_ctl: Self::default_geom_mode(None),
            geom_ctl_drawn: false,
            preview_content: 0,
            preview_project: 0,
            incomplete_since: None,
            last_compose_retry: std::time::Instant::now(),
            thumb_textures: HashMap::new(),
            thumb_lru: std::collections::VecDeque::new(),
            model_rev: 0,
            coverage_cache: Default::default(),
            names_cache: Default::default(),
            uses_cache: Default::default(),
            warp_cache: Default::default(),
            reel_previews: Default::default(),
            reel_textures: HashMap::new(),
            reel_hash_cache: (None, Arc::new(HashMap::new())),
        };
        // loop region of a loaded project — into the player at once (previously
        // sync happened only in on_region_changed, i.e. only on an EDIT of the region —
        // a loaded loop did not loop at all)
        {
            let p = app.model.project();
            let region = p
                .region
                .filter(|_| p.loop_enabled)
                .map(|r| kt_time::TickRange::new(r.start, r.end));
            app.player.set_loop_region(region);
        }
        // import/conversion remarks — to the user (bell)
        for n in notices {
            app.note(n);
        }
        // unavailable sources: EACH on a separate line in the bell and a
        // counter in the status. The project is open meanwhile, the edit is intact; instead of
        // frames there are placeholders with file names (R.33 item 10)
        for (_, label, e) in &err {
            app.warn(crate::tf!(k::msg::MEDIA_MISSING, label, e));
        }
        if !err.is_empty() {
            app.note(crate::tf!(k::msg::MEDIA_MISSING_COUNT, err.len()));
        }
        // KT_AUTOPLAY=1 — play at once (perf debugging without input)
        if std::env::var_os("KT_AUTOPLAY").is_some() {
            app.follow_playhead = true;
            app.player.play();
        }
        // restoration of viewport/cursor (item 13, v1: fromJson)
        app.ui_passthrough = ui_state.clone();
        if let Some(ui) = ui_state {
            // first tpp (unified limits, item 17), then view_start — previously
            // view_start was clamped by the still-default tpp
            app.ticks_per_px =
                ui.ticks_per_px.clamp(crate::timeline::TPP_MIN, crate::timeline::TPP_MAX);
            app.view_start = ui.view_start.max(-40.0 * app.ticks_per_px);
            app.player.seek(Tick(ui.cursor.max(0)));
            // lane order from R.35 comes in the model; ui.lane_order is a
            // legacy input, carried over by the format reader
            app.bin_free = ui.bin_free; // item 7 round 8
            app.bin_layout =
                ui.bin_layout.iter().map(|(id, x, y)| (*id, egui::pos2(*x, *y))).collect();
            app.bin_groups = ui.bin_groups;
            // axis lock of the graphs (R.40 item 15): the file holds the INVERSE, so that
            // the serde default `false` means the lock is on
            app.auto_axis_linked = !ui.auto_axis_unlinked;
            app.geom_ctl = crate::geomctl::GeomMode::from_u32(ui.geom_controls);
            if ui.meters_w > 0.0 {
                app.meters_w = ui.meters_w.clamp(METERS_W_MIN, METERS_W_MAX);
            }
        }
        // R.49 item 360: the user’s workspace is stronger than what lies in a
        // foreign file. An empty store (first launch after the move)
        // keeps the values from the file — migration without a single line of code.
        // measured media lengths — INTO THE MODEL (R.63 item 2): the trim’s stop and the
        // tail hatching depend on them. Outside undo and without an asterisk: this is a
        // fact about the file, not an edit.
        for (id, d) in media_lens {
            app.model.note_media_length(id, d);
        }
        app.workspace = crate::user_store::load_workspace();
        // showing the frame — a property of the workspace (R.67): the default is on,
        // switching it off is remembered between launches.
        app.geom_ctl = Self::default_geom_mode(app.workspace.geom_controls);
        if app.workspace.meters_w > 0.0 {
            app.meters_w = app.workspace.meters_w.clamp(METERS_W_MIN, METERS_W_MAX);
        }
        if let Some(v) = app.workspace.view_of(Some(path)) {
            app.ticks_per_px =
                v.ticks_per_px.clamp(crate::timeline::TPP_MIN, crate::timeline::TPP_MAX);
            app.view_start = v.view_start.max(-40.0 * app.ticks_per_px);
            app.player.seek(Tick(v.cursor.max(0)));
        }
        Ok(app)
    }

    /// Remember the workspace. Called where the program already writes:
    /// on project save and on exit — it does not need its own schedule.
    pub(crate) fn save_workspace(&mut self) {
        // a file newer than our build — we do not touch it, but we must not stay silent:
        // otherwise panel widths “are not remembered” without any explanation
        if self.workspace.is_readonly() {
            let p = self
                .workspace_file
                .clone()
                .unwrap_or_else(|| crate::user_store::store_dir().join("workspace.json"));
            self.warn(crate::tf!(k::msg::WORKSPACE_READONLY, p.display()));
            return;
        }
        self.workspace.meters_w = self.meters_w;
        self.workspace.set_view(
            self.project_path.as_deref(),
            crate::user_store::ProjectView {
                view_start: self.view_start,
                ticks_per_px: self.ticks_per_px,
                cursor: self.player.transport.playhead().0,
            },
        );
        match &self.workspace_file {
            Some(p) => crate::user_store::save_workspace_at(p, &self.workspace),
            None => crate::user_store::save_workspace(&self.workspace),
        }
    }

    #[cfg(test)]
    pub(crate) fn ui_state_for_test(&self) -> kt_model::UiState {
        self.ui_state()
    }

    /// Individual row height (R.38 item 11). `None` — reset to the
    /// computed one. From R.62 the height lives ON THE TRACK and goes through undo
    /// together with the name: there are no longer three parallel property vectors.
    pub fn set_row_height(&mut self, key: (kt_model::LaneKind, u32), h: Option<f32>) {
        let Some(t) = self.model.project().track_at(key.0, key.1) else { return };
        let (id, mut props) = (t.id, t.props());
        props.height = h.unwrap_or(0.0);
        if props != t.props() {
            self.apply(Command::SetTrackProps { id, props });
        }
    }

    /// Row heights for the frame geometry: `(kind, number, px)`.
    pub fn row_heights(&self) -> Vec<(kt_model::LaneKind, u32, f32)> {
        let p = self.model.project();
        p.rows()
            .into_iter()
            .filter_map(|(k, l)| {
                p.track_at(k, l).filter(|t| t.height > 0.0).map(|t| (k, l, t.height))
            })
            .collect()
    }

    /// REFUSAL OF A COMMAND to the person (rule 16): impossibility is an error with a
    /// reason and an address, not a faded hint.
    ///
    /// The track lock NAMES the TRACK and the place where to remove it: before R.62
    /// such a refusal would come as English text of the model's error in a
    /// routine status line — “pressed and nothing happened”.
    pub(crate) fn refuse_command(&mut self, e: &kt_model::CommandError) {
        if let kt_model::CommandError::TrackLocked(id) = e {
            let name = self
                .model
                .project()
                .rows()
                .into_iter()
                .find(|(k, l)| self.model.project().track_at(*k, *l).map(|t| t.id) == Some(*id))
                .and_then(|(k, l)| {
                    self.model.project().track_at(k, l).map(|t| t.label(l))
                })
                .unwrap_or_default();
            self.blocked_track = Some((*id, std::time::Instant::now()));
            self.refuse(crate::tf!(k::track::LOCKED, name));
            return;
        }
        self.note(e.to_string());
    }

    /// Toggle the track badge (R.62) — ONE body for the badge, menu item
    /// and hotkey (rule 17). FX opens the chain editor:
    /// “enable the chain” without a chain is pointless.
    pub fn toggle_track_badge(&mut self, id: kt_model::TrackId, badge: crate::timeline::TrackBadge) {
        use crate::timeline::TrackBadge as B;
        let Some(t) = self.model.project().track(id) else { return };
        if badge == B::Fx {
            self.open_track_fx(id);
            return;
        }
        let mut props = t.props();
        match badge {
            B::Mute => props.mute = !props.mute,
            B::Solo => props.solo = !props.solo,
            B::Lock => props.lock = !props.lock,
            B::Fx => unreachable!("FX opens the editor above"),
        }
        self.apply(Command::SetTrackProps { id, props });
    }

    /// The track that the key looks at (R.62). We have no “selected track”:
    /// EVENTS are selected, so the target is the track of the selected event, and without
    /// a selection — the row under the pointer (the one the badges highlight).
    /// The body is one, shared with the menu and the badge:
    /// `toggle_track_badge` (UI rule 17).
    pub fn key_target_track(&self) -> Option<kt_model::TrackId> {
        let (kind, lane) = self
            .selected
            .iter()
            .copied()
            .find_map(|id| self.model.clip(id).map(kt_model::lane_key))
            .or(self.tl_hover_row)?;
        self.model.project().track_at(kind, lane).map(|t| t.id)
    }

    /// Row caption: the track name or the formula V1/A1/M1/C1.
    pub fn row_label(&self, kind: kt_model::LaneKind, lane: u32) -> String {
        self.model.project().track_label(kind, lane)
    }

    /// Visible window of the main timeline in ticks (R.38 item 14): by it the
    /// automation graph builds its axis while the lock is not removed. One
    /// function for all windows with curves — otherwise the slot’s, the camera’s and the transition’s
    /// zoom would drift apart, as their key editors once did.
    pub fn tl_view(&self) -> (kt_time::Tick, kt_time::Tick) {
        let w =
            self.tl_geom.as_ref().map(|g| g.rect.width() as f64 * g.tpp).unwrap_or(10.0 * 90_000.0);
        let s = self.view_start;
        (kt_time::Tick(s.max(0.0) as i64), kt_time::Tick((s + w) as i64))
    }

    /// Viewport snapshot for writing into the project (item 13).
    fn ui_state(&self) -> kt_model::UiState {
        // Fields of the WORKSPACE (view and meter width) return to the file
        // as they came (R.49, item 360): their own live in user_store.
        let keep = self.ui_passthrough.clone().unwrap_or_default();
        kt_model::UiState {
            view_start: keep.view_start,
            ticks_per_px: keep.ticks_per_px,
            cursor: keep.cursor,
            lane_order: Vec::new(), // row order lives in the model (R.35)
            bin_free: self.bin_free,
            bin_layout: self.bin_layout.iter().map(|(id, p)| (*id, p.x, p.y)).collect(),
            bin_groups: self.bin_groups.clone(),
            meters_w: keep.meters_w,
            // names and heights of tracks since R.62 live on object tracks
            // (model, undo); only legacy read inputs remained in `ui`
            lane_names: Vec::new(),
            lane_heights: Vec::new(),
            auto_axis_unlinked: !self.auto_axis_linked,
            geom_controls: self.geom_ctl.as_u32(),
        }
    }

    /// Full frame drawing. Moved out of `eframe::App::ui` so that the
    /// layout harness test can call it (rule item 9 round 8: layout and
    /// interactions are checked by egui_kittest, not only by a screenshot).
    pub(crate) fn draw_all(&mut self, ui: &mut egui::Ui) {
        // LIVE BENCH COUNTERS (R.68) sit here, not in `ui()`: a frame is
        // the drawing, and the harness draws it with the same body, bypassing
        // `eframe::App::ui`. Counting in `ui()` would mean “in the product there are frames,
        // in the test there are not” — the counter guard would check the wrong thing.
        crate::bench::tick();
        if crate::bench::enabled() {
            crate::bench::note_inputs(ui.ctx().input(|i| i.events.len()) as u64);
        }
        // Telemetry (telemetry-posthog): `app_started` once per process on the first frame,
        // and the first-start notice once — it lists what is sent and what is NOT sent and
        // says where it is switched off (rule 19). After the first frame this is two atomics.
        if let Some(t) = self.telemetry.clone()
            && t.frame(crate::i18n::current().code())
        {
            self.push_notice(Severity::Info, crate::telemetry::notice_text());
            self.status = Some((tr(k::telemetry::HEADLINE).to_string(), std::time::Instant::now()));
        }
        let ctx = ui.ctx().clone();
        // Loss of the video GPU executor in the middle of a session (DeviceLost) —
        // the preview is already on the CPU fallback, notice once (item 2 R.29, test 185)
        if !self.gpu_fx_lost_notified
            && let kt_engine::VideoBackendStatus::Poisoned { reason, .. } =
                self.player.video_backend()
        {
            self.gpu_fx_lost_notified = true;
            self.warn(crate::tf!(k::msg::VIDEO_FX_GPU_LOST, reason));
        }
        // The root Ui that eframe hands out is framed by default 8 px —
        // because of them the WHOLE interface did not reach the window edges (complaint, item 3
        // round 9, about the “meters’ indent on the right” — in fact each
        // panel was indenting). We work on the full content_rect.
        let mut root =
            ui.new_child(egui::UiBuilder::new().max_rect(ctx.content_rect()).layout(*ui.layout()));
        let ui = &mut root;
        // project name — in the window title (item 19 round 6), not every frame
        // name WITHOUT extension (item 5 round 12: “.ktproj” is clear anyway) and build
        // version: in debug the full vYY.MM.DD-hash, in release the short ktYY.MM
        let title = format!(
            "{}{} — {}",
            self.project_path
                .as_ref()
                .and_then(|p| p.file_stem().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| tr(k::win::UNTITLED).to_string()),
            if self.dirty_content { " *" } else { "" },
            kt_version::display()
        );
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        // A CLICK OUTSIDE A FIELD GIVES UP FOCUS — one rule for the whole program
        // (R.63 item 4), and it stands BEFORE the keys are read.
        //
        // egui gives up focus from a `TextEdit` only on Esc, Tab, another focusable widget,
        // or its disappearance. The timeline, the preview and the toolbar are not focusable
        // (`Sense::click_and_drag`), so a click on them did NOT drop focus, and
        // `handle_keys` entirely returned on `egui_wants_keyboard_input()`. In the owner’s words:
        // “however I poke at the rest of the interface, pressing Space does not lead to pause…
        // until I close the effect window.”
        // The rule is one for the whole program on purpose:
        // in each window separately the next agent would forget it
        // (rule 17).
        self.surrender_focus_on_outside_click(&ctx);
        self.handle_keys(&ctx);
        // Speed-up by holding Space — EXACTLY once per frame (handle_keys when
        // the preview is detached is called twice, item 3 round 16).
        self.tick_space_boost(&ctx);
        self.handle_drops(&ctx);
        self.hover_hint = None;
        self.no_resize_rects.clear();
        let rate = self.player.playback_rate();
        if rate.abs() > 0.01 {
            self.play_dir_back = rate < 0.0;
        }

        // Item 10 round 8: the window was restored from maximized last frame —
        // now dragging may begin
        if self.pending_unmax_drag {
            self.pending_unmax_drag = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            Self::release_pointer_after_wm_grab(&ctx);
        }
        // Status bar (bottom)
        egui::Panel::bottom("status")
            .resizable(false)
            .frame(egui::Frame::new().fill(crate::theme::PANEL).inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 0,
                bottom: 1,
            }))
            .exact_size(20.0)
            .show(ui, |ui| {
                self.draw_statusbar(ui);
            });
        // Timeline (bottom, full width) + floating toolbar on top
        // Item 10 round 9: the timeline needs a CEILING, otherwise at small height it
        // takes everything and pushes out the header with the logo/menu and the window buttons (they
        // live inside the side columns). The smart shrinking of egui panels does not
        // break from this — only an upper limit is added.
        let tl_max = (ui.available_height() - 140.0).max(80.0);
        egui::Panel::bottom("timeline")
            .frame(egui::Frame::new().fill(crate::theme::PANEL))
            .resizable(true)
            .default_size(300.0)
            .max_size(tl_max)
            .show(ui, |ui| {
                let full = ui.available_rect_before_wrap();
                self.timeline_rect = full;
                let strip_h = 34.0;
                let tl_rect = egui::Rect::from_min_max(
                    full.min,
                    egui::pos2(full.right(), full.bottom() - strip_h),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(tl_rect), |ui| {
                    self.draw_timeline(ui);
                });
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                        egui::pos2(full.left(), full.bottom() - strip_h),
                        full.max,
                    )),
                    |ui| {
                        self.draw_toolbar_strip(ui);
                    },
                );
            });
        // Time strip above the timeline (port of the Timeline time strip)
        // left column: media bin + performance box (port of widget_3)
        // LIMIT SET ON COMPLAINT of item 4 round 13 — do not squeeze without a new
        // request from the user: 420 px were insurance of round 7.1, in round 9 raised to
        // half the window by his direct request, and in round 11 I cut them
        // back “by analogy” with the meters and broke the work for the third time.
        egui::Panel::left("left_col")
            .frame(crate::theme::panel_frame())
            .resizable(true)
            .default_size(232.0)
            .min_size(72.0)
            .max_size((ui.max_rect().width() * 0.7).max(420.0))
            .show(ui, |ui| {
                // header: logo + menu + project name (items 15, 6, 7 round 8)
                self.draw_column_header(ui);
                ui.add_space(2.0);
                // Round 7 item 27: content in a ScrollArea — the panel frame no longer outgrows its
                // height, and the egui separator no longer climbs onto the timeline
                // (panel.rs takes the ACTUAL rect of the frame); the column also
                // scrolls at a small window height.
                // auto_shrink by WIDTH is mandatory: [false] made the content
                // demand the whole available width, the panel swelled to the content
                // and ate the preview (regression of item 3 round 7.1). For height
                // false — for the sake of item 27 (the frame does not outgrow the panel).
                egui::ScrollArea::vertical().auto_shrink([true, false]).show(ui, |ui| {
                    egui::Frame::group(ui.style())
                        .fill(crate::theme::WINDOW)
                        .corner_radius(1.0)
                        .show(ui, |ui| {
                            self.draw_bin(ui);
                        });
                    ui.add_space(8.0);
                    egui::Frame::group(ui.style())
                        .fill(crate::theme::WINDOW)
                        .corner_radius(1.0)
                        .show(ui, |ui| {
                            self.draw_perf_box(ui);
                        });
                });
            });
        // centre: preview + meters on the right, controls below them.
        // Frame without air: the default 8 px around the perimeter ate the space and
        // pushed the window buttons away from the corner (item 15 round 8).
        egui::CentralPanel::default().frame(egui::Frame::new().fill(crate::theme::PANEL)).show(
            ui,
            |ui| {
                let p = self.model.project();
                if let Some(r) = p.region
                    && !p.loop_enabled
                    && self.player.transport.is_playing()
                    && self.player.transport.playhead() >= r.end
                    && self.player.transport.playhead() < r.end + Tick(9000)
                {
                    self.player.pause();
                    // like a manual pause: double Space after an auto-pause = stop
                    // to the start of the region (item 29 round 7)
                    self.paused_at = Some(std::time::Instant::now());
                }
                egui::Panel::bottom("prev_controls").show(ui, |ui| {
                    ui.add_space(3.0);
                    self.draw_preview_controls(ui);
                    ui.add_space(3.0);
                });
                // We hold the WIDTH ourselves, the standard resizable is off (item 6 round 13):
                // the egui grip is ±8 px over the WHOLE height of the panel and is registered
                // AFTER its content (as designed in egui), so it won over
                // the window buttons, and “minimize” was pressed halfway. Our own strip
                // lies to the LEFT of the panel and does not go inside it.
                let mw = self.meters_w.clamp(METERS_W_MIN, METERS_W_MAX);
                egui::Panel::right("meters_col")
                    .resizable(false)
                    .exact_size(mw)
                    .frame(egui::Frame::new().fill(crate::theme::PANEL))
                    .show(ui, |ui| {
                        // window buttons — here, in the layout (item 15 round 8)
                        self.draw_window_buttons(ui);
                        self.draw_meters(ui);
                    });
                // the drag strip — already OUTSIDE the panel: the panel itself took the space for it,
                // so the right edge of the remaining space is its left edge.
                // Below the window drag strip, so as not to contend with it.
                {
                    // the panel’s edge is computed from the window (ui.max_rect(): the panel
                    // does not shrink it), the strip lies to the LEFT of it and does not go into the panel
                    let win = ui.ctx().content_rect();
                    let edge = win.right() - mw;
                    let sep = egui::Rect::from_min_max(
                        egui::pos2(edge - METERS_SEP_W, win.top() + TOP_DRAG_H),
                        egui::pos2(edge, win.bottom()),
                    );
                    let r = ui.interact(sep, egui::Id::new("meters-sep"), egui::Sense::drag());
                    if r.hovered() || r.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    if r.dragged()
                        && let Some(pos) = r.interact_pointer_pos()
                    {
                        self.meters_w = (win.right() - pos.x).clamp(METERS_W_MIN, METERS_W_MAX);
                        self.dirty_ui = true;
                    }
                }
                egui::CentralPanel::default().frame(egui::Frame::new()).show(ui, |ui| {
                    // Item 2 round 9: a CONTINUOUS window-drag zone across the full width.
                    // The strip is invisible and lies between the resize edge (WIN_EDGE)
                    // and the bottom of the column headers, so it takes no space and
                    // joins the header on the left and the window buttons on the right.
                    if std::env::var_os("KT_DECORATIONS").is_none() {
                        let full = ui.available_rect_before_wrap();
                        // In the maximized window there is NO resize (the block below is disabled
                        // by the maximized guard), so the top WIN_EDGE pixels
                        // remained DEAD: you drag at the very edge — nothing, a little lower — it works
                        // (item 11 round 13). We hand them over to the drag.
                        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                        let top = if maximized { full.top() } else { full.top() + WIN_EDGE };
                        let strip = egui::Rect::from_min_max(
                            egui::pos2(full.left() - 8.0, top),
                            egui::pos2(full.right() + 8.0, full.top() + TOP_DRAG_H),
                        );
                        let bg = ui.interact(
                            strip,
                            ui.id().with("top-drag"),
                            egui::Sense::click_and_drag(),
                        );
                        self.window_drag_from(&bg, ui.ctx());
                        // The strip takes NO SPACE (item 1 round 11): previously here stood
                        // advance_cursor_after_rect, and the preview lost 20 px of height for nothing.
                        // The strip does not steal clicks — egui looks up the click target and
                        // the drag target independently, and the preview itself does not accept
                        // clicks above TOP_DRAG_H.
                    }
                    if self.preview_detached {
                        ui.centered_and_justified(|ui| {
                            ui.weak(tr(k::hint::PREVIEW_DETACHED));
                        });
                    } else {
                        self.draw_preview(ui);
                    }
                });
            },
        );

        // ---- detached preview window (item 7 v1.4): second monitor ------------
        // show_viewport_immediate: one egui::Context for all viewports →
        // the preview texture works in a foreign window without a move; deferred is not
        // suitable (it requires Send+Sync with &mut self).
        if self.preview_detached {
            let vp = egui::ViewportId::from_hash_of("kt-preview-window");
            ctx.show_viewport_immediate(
                vp,
                egui::ViewportBuilder::default()
                    .with_title(tr(k::win::PREVIEW_TITLE))
                    .with_inner_size([960.0, 560.0]),
                |ui, _class| {
                    // input of this viewport: space/shortcuts work from the
                    // window; Esc here ONLY returns the preview (does not touch the
                    // global cascade — the main window’s region/windows do not suffer)
                    let child_ctx = ui.ctx().clone();
                    let esc = child_ctx.input(|i| i.key_pressed(egui::Key::Escape));
                    if !esc {
                        self.handle_keys(&child_ctx);
                    }
                    // without margins, black background — only the video (item 7 round 5)
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(8, 8, 10)))
                        .show(ui, |ui| {
                            self.draw_preview(ui);
                        });
                    // Esc in the preview window returns it to the main window
                    if esc || child_ctx.input(|i| i.viewport().close_requested()) {
                        self.preview_detached =
                            preview_detached_next(self.preview_detached, false, true);
                    }
                },
            );
        }

        self.sample_perf();
        self.draw_search(&ctx);
        self.draw_fx_editors(&ctx);
        self.draw_transition_editors(&ctx);
        self.draw_camera_editors(&ctx);
        self.draw_morph_editors(&ctx);
        self.draw_paste_names(&ctx);
        self.draw_preset_save(&ctx);
        self.draw_settings_window(&ctx);
        self.draw_track_window(&ctx);
        self.draw_seq_name(&ctx);
        // Public parameters of the composition (R.59, stage 3): name dialog,
        // the author window and the consumer windows.
        self.draw_publish_dialog(&ctx);
        self.draw_published_win(&ctx);
        self.draw_env_fit(&ctx);
        // the field’s signal request is taken AFTER the windows are drawn — inside
        // `Window::show` the model cannot be reached (the same mechanism as
        // `hint_sink`)
        self.drain_publish_req();
        self.draw_render_dialog(&ctx);
        self.draw_diagnostics(&ctx);

        // autosave (port of autosaveTimer): an unnamed project — into the
        // recovery file ~/.local/share/kinetren/ (item 20, v1).
        // item 4 round 11: it was every 4 s and on ANY edit, including region movement
        // and zoom — that is, a constant write of the whole project in the
        // UI thread. Now: once per 30 s, only from edits of CONTENT and
        // not during a gesture/playback (insurance — if quiet for more than two
        // intervals, we write anyway: losing edits is worse than a micro-freeze).
        // R.31 item 2: serialization and writing — IN THE BACKGROUND (checklist item 12
        // “background work does not get in the way”): on a big project/slow disk
        // autosave in the UI thread dropped the FPS to zero every 30 s.
        // dirty_ui/viewport shift are saved along the way too (item 5 round 11) —
        // previously this was done by a synchronous save from eframe::save every 30 s.
        // R.49 item 351: the target of the background write is the RECOVERY FILE, not the
        // user’s project (the owner’s direct decision). Only an explicit save changes the project;
        // therefore the asterisk here no longer goes out either.
        // Viewport drift (zoom/scroll) is NO LONGER a reason to write: previously it
        // rewrote the whole project every 30 seconds without a single edit.
        let due = self.autosave_rx.is_none() && self.autosave_due();
        if (self.dirty_content || self.dirty_ui) && due {
            self.last_autosave = std::time::Instant::now();
            let ui = Some(self.ui_state());
            self.model.set_ui_state(ui);
            let target = self.recovery_target();
            let project = self.model.document();
            let retained = self.retained.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            let snap_rev = self.model_rev;
            self.autosave_rx = Some((snap_rev, rx));
            let guard = self.recovery_rev.clone();
            self.recovery_written = Some(target.clone());
            std::thread::Builder::new()
                .name("autosave".into())
                .spawn(move || {
                    let out = project_text_of(&project, &retained, target.parent());
                    // replacing the file — under the guard: if while we
                    // were serializing a MORE RECENT snapshot landed on disk
                    // (Ctrl+S), we do not write our stale one at all
                    let _ = tx.send(write_project_guarded(&guard, &target, snap_rev, &out));
                })
                .ok();
        }
        self.poll_returned_sources();
        self.poll_env_clipboard();
        // Offline analysis: take the finished, start the next one (item 372).
        // `animating` depends on `is_running()`, not on “window open” —
        // otherwise repainting would not stop after finish (repeat of t38).
        if let Some((kind, rev, res)) = self.analysis.poll(crate::analysis::spawn_default) {
            self.on_analysis_done(kind, rev, res);
        }
        if self.analysis.is_running() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
        // Preview of reels (R.56, item 452). Pause is not courtesy: an offline render of the
        // reel decodes files, and during playback it would take the processor
        // away from real time (invariant 6).
        self.reel_previews
            .set_paused(self.player.transport.is_playing() || self.render_job.is_some());
        if self.reel_previews.poll() {
            self.dirty_ui = true;
            // Textures live exactly as long as their band: a reel evicted from the
            // LRU or made stale by an edit has no right to hold tens of megabytes in
            // video memory
            let live = &self.reel_previews;
            self.reel_textures.retain(|(k, _), _| live.get(k).is_some());
        }
        if self.reel_previews.is_running() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
        // Index errors — ALOUD (R.49, item 368). Four `let _ =` were silent about
        // the index not having been built, and the person saw placeholders
        // instead of waveforms without a single word about the cause. Once per address:
        // the service may retry, and the message need not be repeated.
        for e in self.index.take_errors() {
            if self.index_errs_said.insert(e.clone()) {
                self.warn(crate::tf!(k::msg::INDEX_FAILED, e));
            }
        }
        if let Some((snap_rev, rx)) = &self.autosave_rx
            && let Ok(res) = rx.try_recv()
        {
            let snap_rev = *snap_rev;
            self.autosave_rx = None;
            let _ = snap_rev;
            match res {
                // The asterisk does NOT go out: the user’s work went into the recovery file,
                // while its project is still without these edits.
                Ok(SaveOutcome::Wrote { dir_synced }) => self.warn_if_dir_unsynced(dir_synced),
                // routinely and silently: so a newer snapshot has already been written
                Ok(SaveOutcome::SkippedNewerOnDisk) => {}
                Err(e) => {
                    self.recovery_written = None;
                    self.warn(crate::tf!(k::msg::AUTOSAVE_FAILED, e))
                }
            }
        }

        // offer to restore unsaved work (item 353)
        if self.recovery_open {
            let mut act: Option<u8> = None;
            egui::Modal::new(egui::Id::new("kt_recovery")).show(ui.ctx(), |ui| {
                ui.set_max_width(560.0);
                crate::widgets::window_body(ui, |ui| {
                ui.heading(tr(k::ask::RECOVERY_TITLE));
                ui.add_space(6.0);
                ui.label(tr(k::ask::RECOVERY_INTRO));
                ui.add_space(6.0);
                for (i, c) in self.recovery_offer.iter().enumerate() {
                    let owner = match &c.for_project {
                        Some(p) => p.display().to_string(),
                        None => tr(k::ask::RECOVERY_UNNAMED).to_string(),
                    };
                    let label = format!("{}  —  {owner}", crate::journal::stamp_of(c.mtime));
                    if ui.radio(self.recovery_pick == i, label).clicked() {
                        self.recovery_pick = i;
                    }
                    ui.weak(c.path.display().to_string());
                }
                ui.add_space(8.0);
                // rule 19: the dialog says that the decision breaks nothing
                ui.weak(tr(k::ask::RECOVERY_NOTE));
                });
                crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    // button coordinates — outward for live scenarios (R.27):
                    // the egui modal window is not searched for in the accessibility tree
                    let r = ui.button(tr(k::ask::RECOVERY_RESTORE));
                    crate::widgets::field_probe::note("ask.restore", r.rect);
                    if r.clicked() {
                        act = Some(0);
                    }
                    let r = ui.button(tr(k::ask::RECOVERY_FORGET));
                    crate::widgets::field_probe::note("ask.forget", r.rect);
                    if r.clicked() {
                        act = Some(1);
                    }
                    let r = ui.button(tr(k::ask::RECOVERY_LATER));
                    crate::widgets::field_probe::note("ask.later", r.rect);
                    if r.clicked() {
                        act = Some(2);
                    }
                });
                });
            });
            match act {
                Some(0) => self.restore_recovery(self.recovery_pick),
                Some(1) => self.forget_recovery(self.recovery_pick),
                Some(2) => self.recovery_open = false,
                _ => {}
            }
        }

        // a question the program waits for an answer to (item 352). Modal window:
        // until it is answered, saving does not continue.
        if let Some(ask) = self.ask.clone() {
            let mut answer: Option<Answer> = None;
            egui::Modal::new(egui::Id::new("kt_ask")).show(ui.ctx(), |ui| {
                ui.set_max_width(440.0);
                match &ask {
                    Ask::SaveOverLock { pid } => {
                        ui.heading(tr(k::ask::LOCKED_TITLE));
                        ui.add_space(6.0);
                        ui.label(match pid {
                            Some(n) => crate::tf!(k::msg::PROJECT_LOCKED_BY, n),
                            None => tr(k::msg::PROJECT_LOCKED).to_string(),
                        });
                        ui.add_space(4.0);
                        ui.weak(tr(k::ask::LOCKED_BODY));
                        // rule 19: where to replay it
                        ui.weak(tr(k::ask::LOCKED_WHERE));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button(tr(k::ask::SAVE_ANYWAY)).clicked() {
                                answer = Some(Answer::Yes);
                            }
                            if ui.button(tr(k::ask::DONT_SAVE)).clicked() {
                                answer = Some(Answer::No);
                            }
                        });
                    }
                    Ask::Unsaved(p) => {
                        ui.heading(tr(k::ask::UNSAVED_TITLE));
                        ui.add_space(6.0);
                        ui.label(tr(match p {
                            Pending::New => k::ask::UNSAVED_NEW,
                            Pending::Open | Pending::DropOpen | Pending::ImportOlder => {
                                k::ask::UNSAVED_OPEN
                            }
                            Pending::Quit => k::ask::UNSAVED_QUIT,
                        }));
                        ui.add_space(4.0);
                        // rule 19: the decision commits to nothing
                        ui.weak(tr(k::ask::UNSAVED_WHERE));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            let r = ui.button(tr(k::ask::SAVE_AND_GO));
                            crate::widgets::field_probe::note("ask.save_and_go", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::Yes);
                            }
                            let r = ui.button(tr(k::ask::DISCARD));
                            crate::widgets::field_probe::note("ask.discard", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::No);
                            }
                            let r = ui.button(tr(k::ask::STAY));
                            crate::widgets::field_probe::note("ask.stay", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::Cancel);
                            }
                        });
                    }
                    Ask::DropProject => {
                        ui.heading(tr(k::ask::DROP_PROJECT_TITLE));
                        ui.add_space(6.0);
                        let what = self
                            .pending_drop
                            .as_ref()
                            .map(|(p, _)| p.display().to_string())
                            .unwrap_or_default();
                        ui.label(crate::tf!(k::ask::DROP_PROJECT_BODY, what));
                        ui.add_space(4.0);
                        ui.weak(tr(k::ask::DROP_PROJECT_WHERE));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            let r = ui.button(tr(k::ask::DROP_OPEN));
                            crate::widgets::field_probe::note("ask.drop_open", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::Yes);
                            }
                            let r = ui.button(tr(k::ask::DROP_INSERT));
                            crate::widgets::field_probe::note("ask.drop_insert", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::No);
                            }
                            let r = ui.button(tr(k::ask::STAY_HERE));
                            crate::widgets::field_probe::note("ask.drop_cancel", r.rect);
                            if r.clicked() {
                                answer = Some(Answer::Cancel);
                            }
                        });
                    }
                }
            });
            if let Some(a) = answer {
                self.answer_ask(a);
            }
        }

        // analysis progress window (item 372). ONE for all tasks: a window for each
        // would mean a palisade of windows with a queue of a dozen files.
        if self.analysis.open {
            let mut cancel = false;
            egui::Window::new(tr(k::win::ANALYSIS_TITLE))
                .id(egui::Id::new("kt_analysis"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
                .show(ui.ctx(), |ui| {
                    ui.add(egui::ProgressBar::new(self.analysis.progress()).desired_width(220.0));
                    if self.analysis.queued() > 0 {
                        ui.weak(crate::tf!(k::win::ANALYSIS_QUEUED, self.analysis.queued()));
                    }
                    if ui.button(tr(k::win::ANALYSIS_CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            if cancel {
                self.analysis.cancel_all();
            }
        }

        // FFmpeg window (windows-build): found or not, why, Download / Locate / Rescan
        self.draw_ffmpeg_window(ui.ctx());

        // notifications window (item 18)
        if self.notices_open {
            let mut open = self.notices_open;
            egui::Window::new(tr(k::win::NOTICES_TITLE))
                .id(egui::Id::new("kt_notices"))
                .open(&mut open)
                .default_size([460.0, 340.0])
                .min_size([320.0, 200.0])
                .resizable(true)
                .vscroll(true)
                .show(ui.ctx(), |ui| {
                    if self.notices.is_empty() {
                        ui.weak(tr(k::win::NOTICES_EMPTY));
                    }
                    for n in self.notices.iter().rev() {
                        let col = match n.severity {
                            Severity::Warn => egui::Color32::from_rgb(255, 190, 120),
                            Severity::Info => egui::Color32::from_gray(200),
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.weak(&n.at);
                            ui.colored_label(col, &n.text);
                            if n.repeats > 1 {
                                ui.weak(format!("×{}", n.repeats));
                            }
                        });
                    }
                });
            self.notices_open = open;
        }

        // ---- resize of a borderless window from the edges and corners (items 11-13 round 8) ----
        // IMPORTANT: not a widget and not at the start of the frame. A widget lost the hit test
        // to all panels (egui at equal distance gives the click to the widget registered LATER),
        // and the `drag_started` of click_and_drag came only after a shift of >6 px —
        // by then the cursor had already left the zone, and BeginResize NEVER went out.
        // Now we read the pointer directly at the end of the frame:
        // the cursor is set last
        // (it wins), and the command is sent on the press itself.
        if !ctx.input(|i| i.viewport().maximized.unwrap_or(false))
            && std::env::var_os("KT_DECORATIONS").is_none()
        {
            let win = ctx.content_rect();
            // buttons at the edges (window, menu, bell) take the press for
            // themselves — except the 2 px EDGE itself, where the resize remains,
            // including the diagonal one at the corner (item 9 round 12)
            let guarded = |slf: &Self, pos: egui::Pos2| {
                win.shrink(WIN_BORDER).contains(pos)
                    && slf.no_resize_rects.iter().any(|r| r.contains(pos))
            };
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos())
                && !guarded(self, pos)
                && let Some(dir) = resize_zone(win, pos, WIN_EDGE, WIN_CORNER)
            {
                ctx.set_cursor_icon(resize_cursor(dir));
                if ctx.input(|i| i.pointer.primary_pressed()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                    Self::release_pointer_after_wm_grab(&ctx);
                }
            }
        }

        // hint — last, over the status bar (item 1 round 9)
        self.draw_status_hint(&ctx);

        // Glow overlay above the interface (item 8): the preview and the meters softly
        // “glow” beyond the borders of their panels, without hindering reading of the UI.
        if self.sw_render == Some(false) {
            let fg = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("ambient-overlay"),
            ));
            // preview: a blurred texture slightly wider than the panel, very low alpha
            if let (Some(glow), true, false) =
                (&self.glow_texture, self.preview_rect.is_finite(), self.preview_detached)
            {
                let r = self.preview_rect.expand(28.0);
                fg.image(
                    glow.id(),
                    r,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14),
                );
            }
            // meters: halo to the left of the panel, colour by height ZONES (item 8);
            // the right edge — exactly the left edge of the panel (item 10: previously +8 px
            // ran onto the first bar by the dB label)
            if let Some((r, _k, loud_db)) = self.meter_glow
                && loud_db > crate::meters::METER_MIN_DB + 0.5
            {
                {
                    let glow_rect = crate::meters::overlay_glow_rect(r);
                    // rows on zone boundaries: zone colour, alpha by k;
                    // above the level — fade into transparency
                    let mut mesh = egui::Mesh::default();
                    let y_of = |db: f32| -> f32 {
                        let t = (db - crate::meters::METER_MIN_DB)
                            / (crate::meters::METER_MAX_DB - crate::meters::METER_MIN_DB);
                        glow_rect.bottom() - t.clamp(0.0, 1.0) * glow_rect.height()
                    };
                    let mut push_row = |y: f32, col: egui::Color32, alpha: u8| {
                        for (x, aa) in [(glow_rect.left(), 0u8), (glow_rect.right(), alpha)] {
                            mesh.vertices.push(egui::epaint::Vertex {
                                pos: egui::pos2(x, y),
                                uv: egui::epaint::WHITE_UV,
                                color: egui::Color32::from_rgba_unmultiplied(
                                    col.r(),
                                    col.g(),
                                    col.b(),
                                    aa,
                                ),
                            });
                        }
                    };
                    // the alpha of a row — from ITS dB, not from the total loudness
                    // (item 4 round 16): a single common `k` made the whole
                    // halo flicker along the height, including the always-lit bottom
                    let stops = crate::meters::glow_stops(loud_db);
                    for &db in &stops {
                        push_row(
                            y_of(db),
                            crate::meters::zone_color(db),
                            crate::meters::glow_row_alpha(
                                db,
                                loud_db,
                                crate::meters::GLOW_OVERLAY_A,
                            ),
                        );
                    }
                    let top_col = crate::meters::zone_color(loud_db);
                    push_row((y_of(loud_db) - 30.0).max(glow_rect.top()), top_col, 0);
                    let rows = stops.len() + 1;
                    for row in 0..rows - 1 {
                        let b = (row * 2) as u32;
                        mesh.indices.extend_from_slice(&[b, b + 1, b + 3, b, b + 3, b + 2]);
                    }
                    fg.add(egui::Shape::mesh(mesh));
                }
            }
        }

        // Redraw ON DEMAND (item 1: unconditional 60 fps + filling the
        // texture every frame heated the system even with an empty project; egui
        // already redraws instantly on any input).
        let animating = self.player.transport.is_playing()
            || self.space_down_at.is_some()
            || self.drag.is_some()
            || self.render_job.is_some()
            || self.meter.is_active();
        // “slow down when busy” (rule 12): picker thumbnails are
        // computed in their own thread, and during playback/export it must yield
        self.fx_thumbs.set_busy(self.player.transport.is_playing() || self.render_job.is_some());
        // ...and while it has unfinished requests — we ask for frames, otherwise the
        // ready cell appears only on the next input. The gate hangs on
        // “work is ALIVE”, not on “picker is open”: an open menu has
        // no right to hold 60 fps in idle (defect t38).
        if self.fx_thumbs.pending() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
        let delay = if animating {
            std::time::Duration::from_millis(if self.ui_economy { 33 } else { 16 })
        } else if self.incomplete_since.is_some() {
            std::time::Duration::from_millis(100) // retries of an incomplete frame
        } else {
            repaint_delay(false)
        };
        ctx.request_repaint_after(delay);
    }

    /// After a system drag/resize of the window (X11 `_NET_WM_MOVERESIZE`) the WM
    /// takes the pointer and ButtonReleased does NOT come to the application — egui
    /// forever considers the button pressed, `dragged` holds the drag widget, and the whole
    /// rest of the UI loses hovered (timeline scroll died until a click,
    /// item 28 round 7). Call right after send_viewport_cmd(StartDrag/BeginResize).
    pub(crate) fn release_pointer_after_wm_grab(ctx: &egui::Context) {
        ctx.stop_dragging();
        ctx.input_mut(|i| i.pointer = Default::default());
    }

    /// The hint line — drawn LAST in the frame, over the status bar
    /// (item 1 round 9). The status bar, drawn as a panel second, long before the buttons,
    /// so reading `hover_hint` inside it is pointless — there it is always
    /// None. Layer Foreground + interactable(false): a window drag from the status bar
    /// is not intercepted.
    fn draw_status_hint(&mut self, ctx: &egui::Context) {
        let strip = self.statusbar_rect;
        if !strip.is_finite() || strip.width() < 1.0 {
            return;
        }
        // hints of shared widgets come from the window closures (the sink
        // `widgets::hint_sink`, item 415): we take them exactly here, at the end of the
        // frame, so that the display place stays ONE (rule 10)
        if self.hover_hint.is_none() {
            self.hover_hint = crate::widgets::hint_sink::take();
        } else {
            let _ = crate::widgets::hint_sink::take();
        }
        let mode = if self.dnd_hover.is_some() { Some(tr(k::hint::DROP_HERE)) } else { None };
        let status =
            self.status.as_ref().map(|(m, at)| (m.as_str(), at.elapsed().as_millis() as u64));
        let blocked = self
            .blocked
            .map(|b| self.blocked_text(b))
            .or_else(|| {
                self.blocked_material
                    .map(|t| crate::tf!(k::tl::hint::TRIM_MATERIAL_END, format!("{:.1}", t as f64 / 90_000.0)))
            })
            .or_else(|| {
            self.error_status
                .as_ref()
                .filter(|(_, at)| (at.elapsed().as_millis() as u64) < STATUS_TTL_MS)
                .map(|(t, _)| t.clone())
        });
        let (text, kind) = status_line(
            blocked.as_deref(),
            self.hover_hint.as_deref(),
            mode,
            status,
            STATUS_TTL_MS,
            tr(k::hint::IDLE),
        );
        if kind != HintKind::Status
            && self
                .status
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed().as_millis() as u64 >= STATUS_TTL_MS)
        {
            self.status = None;
        }
        let col = match kind {
            HintKind::Error => HINT_ERROR,
            HintKind::Status => egui::Color32::from_rgb(255, 210, 120),
            HintKind::Mode => egui::Color32::from_rgb(150, 215, 175),
            HintKind::Hover => egui::Color32::from_gray(205),
            HintKind::Idle => egui::Color32::from_gray(120),
        };
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("status-hint"),
        ));
        let galley = painter.layout_no_wrap(text.clone(), egui::FontId::proportional(12.0), col);
        let x1 = strip.right() - 6.0;
        // a long hint quietly overwrites the metrics on the left — it is more important
        let plate = egui::Rect::from_min_max(
            egui::pos2((x1 - galley.size().x - 8.0).max(strip.left()), strip.top()),
            egui::pos2(x1, strip.bottom()),
        );
        painter.rect_filled(plate, 0.0, crate::theme::PANEL);
        painter.galley(
            egui::pos2(plate.left() + 4.0, strip.center().y - galley.size().y * 0.5),
            galley,
            col,
        );
        // what is ACTUALLY drawn — for the harness test (checking the internal
        // field hover_hint is useless: it is filled after the status bar)
        self.status_line_shown = (text, kind);
    }

    /// Project search palette (item 3 round 10): Ctrl+F, arrows, Enter.
    /// Esc is handled HERE: while focus is in the field, handle_keys
    /// is not run at all, and the common Esc cascade is never reached.
    fn draw_search(&mut self, ctx: &egui::Context) {
        if !self.search_open {
            return;
        }
        let hits = search_hits(self.model.project(), &self.bin_groups, &self.search_query);
        let mut open = true;
        let mut go: Option<Hit> = None;
        let shown = egui::Window::new(tr(k::win::SEARCH_TITLE))
            .id(egui::Id::new("kt_search"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -80.0])
            .default_width(420.0)
            .show(ctx, |ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text(tr(k::search::FIELD))
                        .desired_width(400.0),
                );
                if !r.has_focus() && self.search_focus {
                    r.request_focus();
                    self.search_focus = false;
                }
                let (up, down, enter, esc) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowUp),
                        i.key_pressed(egui::Key::ArrowDown),
                        i.key_pressed(egui::Key::Enter),
                        i.key_pressed(egui::Key::Escape),
                    )
                });
                if down {
                    self.search_sel = (self.search_sel + 1).min(hits.len().saturating_sub(1));
                }
                if up {
                    self.search_sel = self.search_sel.saturating_sub(1);
                }
                if esc {
                    self.search_open = false;
                }
                ui.separator();
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (i, h) in hits.iter().enumerate() {
                        let tag = match h.kind {
                            HitKind::Marker => tr(k::search::MARKER),
                            HitKind::Media => tr(k::search::MEDIA),
                            HitKind::Track => tr(k::search::TRACK),
                            HitKind::Clip => tr(k::search::CLIP),
                            HitKind::Group => tr(k::search::GROUP),
                        };
                        let sel = i == self.search_sel;
                        let text = format!("{tag}  ·  {}", h.label);
                        if ui.selectable_label(sel, text).clicked() {
                            go = Some(h.clone());
                        }
                    }
                    if hits.is_empty() && !self.search_query.trim().is_empty() {
                        ui.weak(tr(k::search::NOTHING));
                    }
                });
                if enter && let Some(h) = hits.get(self.search_sel) {
                    go = Some(h.clone());
                }
            });
        // the whole window frame — the README screenshots crop by it
        if let Some(r) = &shown {
            crate::widgets::field_probe::note("win.search", r.response.rect);
        }
        if !open {
            self.search_open = false;
        }
        if let Some(h) = go {
            self.search_open = false;
            match h.kind {
                HitKind::Media => {
                    if let Some(id) = h.media {
                        self.place_media_at_cursor(id);
                    }
                }
                // group: show it in the bin — free mode and
                // the whole selection (a jump “nowhere” is useless)
                HitKind::Group => {
                    if let Some(first) = h.media
                        && let Some(gi) = self.bin_group_of(first)
                    {
                        self.bin_free = true;
                        self.bin_sel = self.bin_groups[gi].members.iter().copied().collect();
                    }
                }
                _ => {
                    self.player.seek(Tick(h.at));
                    self.pending_reveal = Some(h.at);
                    match h.clip {
                        Some(id) => self.select_only(id),
                        None => self.selected.clear(),
                    }
                }
            }
        }
    }

    /// Jump to the previous/next stop point (item 16 round 8).
    /// `far` (Shift) — at once to the start/end of the project.
    pub(crate) fn jump_stop(&mut self, forward: bool, far: bool) {
        let p = self.model.project();
        let pts = stop_points(p);
        // “end” — not only the end of the material: a marker or region boundary
        // may lie further right, and then the forward jump would drag the cursor
        // BACK (item 7 round 10)
        let end = kt_engine::export::project_range(p).0.max(pts.last().copied().unwrap_or(0));
        let cursor = self.player.transport.playhead().0;
        let at = if far {
            if forward { end } else { 0 }
        } else if forward {
            next_stop(cursor, &pts).unwrap_or(end)
        } else {
            prev_stop(cursor, &pts).unwrap_or(0)
        };
        let at = Tick(at.max(0)).align_audio_up();
        self.player.seek(at);
        // the timeline window must show the cursor (item 11 round 9); applied in
        // draw_timeline, where the current rect and zoom are known
        self.pending_reveal = Some(self.player.transport.playhead().0);
    }

    /// Navigation “Back”/“Forward” with Shift = DRAG the region (item 2 round 16).
    ///
    /// Shift is the same “extend selection” modifier as in Premiere,
    /// Resolve and DAWs. Shift+A places the LEFT edge of the region at the new position,
    /// Shift+D — the right one; if there is no region yet, the second edge becomes the
    /// position BEFORE the jump, so the very first Shift+D selects the traversed
    /// stretch. The region is a “convenience”, so only `dirty_ui`.
    pub(crate) fn nav_jump(&mut self, forward: bool, far: bool, drag_region: bool) {
        let before = self.player.transport.playhead();
        self.jump_stop(forward, far);
        if !drag_region {
            return;
        }
        if self.model.project().region.is_none() {
            let at = self.player.transport.playhead();
            self.model.set_region(Region::normalized(before, at));
        }
        // the edge is placed at the NEW cursor position; the opposite edge
        // is clamped inside set_region_edge_from_cursor
        self.set_region_edge_from_cursor(!forward);
    }

    /// Zoom with buttons — anchored to the PLAYHEAD (item 26 round 7: previously it anchored
    /// the left edge of the window). The cursor stays at the same screen position; if it is outside
    /// the window — we centre it.
    pub(crate) fn zoom_at_playhead(&mut self, factor: f64) {
        self.follow_playhead = false; // manual zoom (item 14 round 6)
        self.last_user_scroll = Some(std::time::Instant::now());
        let ph = self.player.transport.playhead().0 as f64;
        let old_tpp = self.ticks_per_px;
        let new_tpp = (old_tpp * factor).clamp(crate::timeline::TPP_MIN, crate::timeline::TPP_MAX);
        let w = (self.timeline_rect.width() as f64 - crate::timeline::GUTTER_W as f64).max(1.0);
        let px = (ph - self.view_start) / old_tpp; // screen position of the cursor
        let px = if (0.0..=w).contains(&px) { px } else { w * 0.5 };
        self.view_start = ph - px * new_tpp;
        self.ticks_per_px = new_tpp;
    }

    /// Hint: a popup + a line in the bottom right of the status bar (item 16 round 7 —
    /// hover_hint is read by draw_statusbar). All on_hover_text calls go here.
    pub(crate) fn hint(&mut self, r: egui::Response, text: &str) -> egui::Response {
        if r.hovered() {
            self.hover_hint = Some(text.to_string());
        }
        // item 14 round 8: NO popups at all — the hint lives in exactly
        // one place, at the bottom right of the status line
        r
    }

    /// Transport machine Space/buttons (item 15 round 6, test 58; replaces the
    /// former rule “from pause at a region — always to the start”,
    /// by the user’s sanction):
    /// - playing → pause, cursor in place;
    /// - pause + repeat Space within DOUBLE_PAUSE_MS → “stop”:
    ///   cursor to the start of the region (or 0);
    /// - pause + Space later → resume FROM THE SAME PLACE; but with loop on and
    ///   the cursor OUTSIDE the region — from the start of the region (transport wrap
    ///   works only from inside the region).
    pub fn transport_toggle(
        playing: bool,
        pause_recent: bool,
        cursor: i64,
        region: Option<(i64, i64)>,
        loop_enabled: bool,
    ) -> TransportAction {
        if playing {
            return TransportAction::Pause;
        }
        let region = region.filter(|(s, e)| e > s);
        if pause_recent {
            return TransportAction::SeekStart(region.map(|(s, _)| s).unwrap_or(0));
        }
        match region {
            Some((s, e)) if loop_enabled && !(cursor >= s && cursor < e) => {
                TransportAction::Resume { seek: Some(s) }
            }
            // item 29 round 7: the region has been played through (auto-pause left the cursor at or past the
            // end) → Play plays the region again; otherwise resume fell into the
            // auto-pause window in the same frame and “did nothing”. Only
            // cursor >= e: playback BEFORE the region is not touched.
            Some((s, e)) if !loop_enabled && cursor >= e => {
                TransportAction::Resume { seek: Some(s) }
            }
            _ => TransportAction::Resume { seek: None },
        }
    }

    /// Speed-up by holding Space (item 3 round 16, as on YouTube): per frame dt
    /// the multiplier approaches `SPACE_BOOST_MAX`, up to a ceiling of ~2 seconds.
    ///
    /// The scheme is ANALYTIC (exponential), not explicit: `stable_dt` after an
    /// idle period comes equal to the idle redraw delay (up to 250 ms), and
    /// explicit integration overshoots the target on such steps (item 7 round 9 —
    /// exactly so the fader flew off to 400 dB).
    pub fn space_boost_step(cur: f32, dt: f32) -> f32 {
        // in SPACE_BOOST_SECS we cover 95% of the path to the ceiling
        let k = -(0.05f32.ln()) / SPACE_BOOST_SECS;
        let a = (-k * dt.clamp(0.0, 0.25)).exp();
        let next = SPACE_BOOST_MAX + (cur - SPACE_BOOST_MAX) * a;
        next.clamp(1.0, SPACE_BOOST_MAX)
    }

    /// The speed-up frame: while space is held longer than the threshold — the speed creeps
    /// up. Called EXACTLY once per frame (not from handle_keys: that one
    /// is called twice with a detached preview and would speed up twice as fast).
    fn tick_space_boost(&mut self, ctx: &egui::Context) {
        let Some(t0) = self.space_down_at else { return };
        // shuttle — a more explicit intent of the user, we do not hinder it
        if self.shuttle_override.is_some() {
            return;
        }
        if t0.elapsed().as_millis() < SPACE_HOLD_MS {
            return;
        }
        if self.space_boost.is_none() {
            self.space_boost = Some(1.0);
            // stood still — holding starts playback at once (otherwise “speeding up”
            // has nothing to do), but on release we no longer toggle play/pause
            if !self.player.transport.is_playing() {
                self.on_play();
            }
        }
        let dt = ctx.input(|i| i.stable_dt);
        let b = Self::space_boost_step(self.space_boost.unwrap_or(1.0), dt);
        self.space_boost = Some(b);
        // hysteresis: each set_playback_rate resets the extrapolation window
        // of the display clock (item 14 round 14) — it must not be jerked on every
        // hundredth
        let want = self.base_rate * b;
        if boost_needs_update(want, self.player.playback_rate(), self.base_rate) {
            self.player.set_playback_rate(want);
        }
        ctx.request_repaint();
    }

    /// What to show on the LOWER (time) strip of the speed control:
    /// shuttle, and while speeding up by holding Space — the actual speed
    /// (item 3 round 16). Otherwise the speed-up goes on while the widget silently shows 1×.
    pub fn speed_temp_slider(&self) -> Option<f32> {
        if self.shuttle_override.is_some() {
            return self.shuttle_override;
        }
        self.space_boost.map(|_| rate_to_slider(self.player.playback_rate()))
    }

    /// Space released: a short press = play/pause, a long one = the end of
    /// the speed-up (transport is NOT toggled in this case).
    fn on_space_released(&mut self) {
        let held = self.space_down_at.take().map(|t| t.elapsed().as_millis()).unwrap_or(0);
        if self.space_boost.take().is_some() {
            self.player.set_playback_rate(self.base_rate);
            return;
        }
        if held < SPACE_HOLD_MS {
            self.on_transport_toggle();
        }
    }

    /// Single point Space/Play-Pause button (both go here).
    pub fn on_transport_toggle(&mut self) {
        let p = self.model.project();
        let region = p.region.map(|r| (r.start.0, r.end.0));
        let pause_recent =
            self.paused_at.map(|t| t.elapsed().as_millis() < DOUBLE_PAUSE_MS).unwrap_or(false);
        match Self::transport_toggle(
            self.player.transport.is_playing(),
            pause_recent,
            self.player.transport.playhead().0,
            region,
            p.loop_enabled,
        ) {
            TransportAction::Pause => {
                self.player.pause();
                self.paused_at = Some(std::time::Instant::now());
            }
            TransportAction::SeekStart(at) => {
                self.player.seek(Tick(at));
                self.paused_at = None;
            }
            TransportAction::Resume { seek } => {
                if let Some(at) = seek {
                    self.player.seek(Tick(at));
                }
                self.paused_at = None;
                self.on_play();
            }
        }
    }

    /// Start playback from the current position (resume semantics, item 15).
    pub fn on_play(&mut self) {
        self.follow_playhead = true;
        self.player.play();
    }

    /// Region released by the mouse (end of drag). During playback the cursor
    /// is immediately moved to the start of the new region (rule “selected during
    /// play — released — we play the region”, item 4/v1-clamp).
    pub fn on_region_released(&mut self) {
        if self.player.transport.is_playing()
            && let Some(r) = self.model.project().region
            && r.end > r.start
        {
            self.player.seek(r.start);
        }
    }

    /// Path of the sidecar index DB of the current project (None for unnamed).
    fn index_db_path(&self) -> Option<std::path::PathBuf> {
        self.project_path.as_ref().map(|p| p.with_extension("ktindex.db"))
    }

    /// Per-component maximum resolution over the project’s video sources
    /// (port of computeMaxSourceVideoResolution; short-lived probes).
    /// Cached variant (R.31 item 3): recomputed only on a model change.
    pub fn max_source_resolution_cached(&mut self) -> Option<(u32, u32)> {
        if let Some((rev, v)) = self.max_src_cache
            && rev == self.model_rev
        {
            return v;
        }
        let v = self.max_source_resolution();
        self.max_src_cache = Some((self.model_rev, v));
        v
    }

    pub fn max_source_resolution(&self) -> Option<(u32, u32)> {
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let db = self.index_db_path();
        let (mut mw, mut mh) = (0u32, 0u32);
        for m in &self.model.project().media {
            // Pseudo-sources do not take part: a generator’s “native” is the
            // reserve of detail for the decode step (GEN_NATIVE, R.52), not the
            // resolution of the material, and it has no place in “Max (by sources)”.
            if kt_model::is_pseudo_path(&m.path) {
                continue;
            }
            if let Ok(src) =
                open_source_indexed(&resolve_media_path(base.as_deref(), &m.path), db.as_deref())
            {
                let i = src.info();
                if i.has_video {
                    mw = mw.max(i.width);
                    mh = mh.max(i.height);
                }
            }
        }
        (mw >= 2 && mh >= 2).then_some((mw & !1, mh & !1))
    }

    /// Esc cascade (item 11): windows → active drag → region+selection.
    pub fn escape_action(&mut self) {
        // 0) number input of speed (R.57) — it lives IN PLACE of the band, and
        // it must be closed before anything else; focus is handed over by
        // `draw_speed_control` on the next frame
        if self.speed_entry.take().is_some() {
            return;
        }
        if self.search_open {
            self.search_open = false;
            return;
        }
        // 0) insert prefix window (R.35 item 4)
        if self.preset_save.take().is_some() {
            return;
        }
        if self.paste_names.take().is_some() {
            return;
        }
        // 1) close the top window of ours. The public-parameter windows (R.59,
        // stage 3) go BEFORE the effect windows: they are opened on top — and
        // the person expects that Esc removes what they see last.
        // The handle-name dialog is deliberately NOT in the cascade: it swallows Esc
        // itself, like the reel name window — while focus is in it, it never
        // reaches here (`egui_wants_keyboard_input`).
        if let Some(w) = self.published_win.as_mut().filter(|w| w.open) {
            w.open = false;
            return;
        }
        if self.render_setup.open {
            self.render_setup.open = false;
            return;
        }
        // track window (R.62) — the same “our window” as the effect window
        if let Some(w) = self.track_win.as_mut().filter(|w| w.open) {
            w.open = false;
            return;
        }
        if let Some(ed) = self.fx_editors.iter_mut().rev().find(|e| e.open) {
            ed.open = false;
            return;
        }
        if let Some(ed) = self.morph_editors.iter_mut().rev().find(|e| e.open) {
            ed.open = false;
            return;
        }
        if self.notices_open {
            self.notices_open = false;
            return;
        }
        if self.ffmpeg_ui.open {
            self.ffmpeg_ui.open = false;
            return;
        }
        if self.perf.diag_open {
            self.perf.diag_open = false;
            return;
        }
        // 2) cancel the active drag
        if self.drag.is_some() {
            self.drag = None;
            self.snap_line = None;
            return;
        }
        // 3) clear the region and selection (loop stays armed — item 3 of round 11)
        let had = self.model.project().region.is_some()
            || !self.selected.is_empty()
            || !self.selected_morph.is_empty()
            || !self.selected_camera.is_empty();
        if self.model.project().region.is_some() {
            self.clear_region_keep_loop();
        }
        self.selected.clear();
        self.selected_morph.clear();
        self.selected_camera.clear();
        // 4) COLLAPSE THE DISCLOSURE — before leaving the reel (R.55, C5):
        // a collapsed strip costs the person less attention than leaving outward,
        // and restoring it with one gesture is cheaper.
        if !had && self.collapse_all_in_place() {
            return;
        }
        // 5) EXIT THE REEL — LAST (R.50, B3). Were it earlier,
        // “clear selection” would throw the person outward: one key
        // would do two different things, and the second one is irreversible in attention terms.
        if !had && !self.nest_path.is_empty() {
            self.leave_nest();
        }
    }

    /// Reordering of track display rows — ONE command for any kind
    /// (R.56, item 449).
    ///
    /// Before R.56 there were two: between video rows the CONTENT was moved
    /// (`reorder_lane`, batch `MoveClip`), and any other case
    /// was a “purely visual vector shift”. Because of that second branch,
    /// a video row dragged next to a morph or camera row changed
    /// its place on screen but NOT its number — that is, it stayed the same
    /// composite layer. Now the track number = the row’s rank,
    /// and the model itself renumbers the content: drag a track — you changed
    /// the edit (owner’s decision R.35), and an empty row moves
    /// just like a full one.
    pub fn reorder_rows(&mut self, from: usize, to: usize) {
        let mut rows = self.model.project().rows();
        if from == to || from >= rows.len() || to >= rows.len() {
            return;
        }
        // Rows are addressed by KEY, not by index: collapsing a band
        // (below) changes their count, and gesture indices after it would lie.
        let (a, b) = (rows[from], rows[to]);
        self.collapse_bands_for_rows();
        rows = self.model.project().rows();
        let (Some(from), Some(to)) =
            (rows.iter().position(|&k| k == a), rows.iter().position(|&k| k == b))
        else {
            // a row that belonged to a collapsed band — there is nothing left to
            // reorder, and that is more honest than moving the wrong one
            return;
        };
        let e = rows.remove(from);
        rows.insert(to, e);
        let perm = kt_model::LanePerm::from_rows(
            &rows,
            kt_model::LaneBases::of_root(self.model.project()),
        );
        if self.apply(Command::SetLaneOrder { order: rows }) {
            // names and heights live OUTSIDE undo and are addressed by the pair (kind,
            // number) — so they move with the same mapping as the content;
            // the name and height live ON THE TRACK and travel with the object —
            // there is no longer any need to move them separately (R.62)
            let _ = perm;
        }
    }

    /// Collapse the disclosure before editing ROWS, and say so
    /// (R.56, item 449).
    ///
    /// With a band open the model does not accept the order or the number of rows
    /// (`CommandError::BandsOpen`): the band lives at the tail of the numeric
    /// space, and renumbering would put its events into the wrong
    /// reels. Collapsing silently would also be wrong — the person looks for their
    /// disclosure where they left it (rule 16).
    pub fn collapse_bands_for_rows(&mut self) {
        if self.collapse_all_in_place() {
            self.note(tr(k::msg::ROWS_COLLAPSED_BAND));
        }
    }

    /// Inserting a track at a SPECIFIC place: model command, display row
    /// and NAMES at once (item 5 of round 16).
    ///
    /// Names previously did not move at all: the functions `lane_names_insert` and
    /// its adjacent ones existed and were covered by t90, but were not called
    /// from the app — after insertion the label ended up on someone else’s track.
    pub fn insert_lane_at(&mut self, kind: LaneKind, at: u32) -> bool {
        let ok = self.apply(Command::InsertTrack { kind, at });
        ok
    }

    /// Add a track RIGHT BELOW the clicked row (item 5 of round 16: “so that
    /// the addition happens not at the very bottom, but after the track that
    /// was clicked”).
    ///
    /// The new track’s number is its RANK among the rows of its own kind (R.56,
    /// item 449). Before R.56 a row of another kind gave `at = tail`, that is
    /// the LARGEST number: the track landed at the ceiling of the screen and
    /// ended up as the BOTTOM layer of the composite — “I put video there and
    /// can’t see it” (the owner’s complaint, item 2). The order and the number are set
    /// in ONE batch, that is one undo step: there used to be two.
    pub fn insert_lane_after_row(&mut self, kind: LaneKind, after: Option<(LaneKind, u32)>) {
        self.collapse_bands_for_rows();
        let rows = self.model.project().rows();
        // rank = how many rows of this kind sit NOT BELOW the clicked one
        let at = match after.and_then(|row| rows.iter().position(|&x| x == row)) {
            Some(pos) => rows[..=pos].iter().filter(|e| e.0 == kind).count() as u32,
            None => rows.iter().filter(|e| e.0 == kind).count() as u32,
        };
        let insert = Command::InsertTrack { kind, at };
        // The order is computed OURSELVES and put into the same batch: `lane_order_insert`
        // places the row before the next track OF ITS OWN kind, and if there is none —
        // at the very end of the list. Live check: a click on V2 with the order
        // V1,V2,A1,A2 gave V1,V2,A1,A2,V3 — exactly the “to the bottom”
        // that had been complained about.
        let mut order = rows;
        kt_model::lane_order_insert(&mut order, kind, at);
        if let Some(row) = after
            && let Some(f) = order.iter().position(|&e| e == (kind, at))
        {
            let e = order.remove(f);
            let to = order.iter().position(|&x| x == row).map(|i| i + 1).unwrap_or(order.len());
            order.insert(to, e);
        }
        self.apply_batch(vec![insert, Command::SetLaneOrder { order }]);
    }

    /// Deleting a track (item 8, the user’s decision: together with its events,
    /// one undo step). Since R.62 this is ONE model command: it removes the
    /// track’s events, shifts higher numbers down, and takes its name, height and
    /// properties with it — there are no longer three parallel tables.
    pub fn delete_lane(&mut self, kind: ClipKind, lane: u32) {
        // the model does not accept the EDIT of ROWS while a band is open (R.56)
        self.collapse_bands_for_rows();
        let lk = kt_model::lane_key_of(kind);
        if self.model.project().lanes(lk) <= 1 {
            self.note(tr(k::msg::LAST_LANE));
            return;
        }
        self.apply(Command::RemoveTrack { kind: lk, at: lane });
    }

    /// The visible timeline range in ticks (for calculating “where to look”).
    pub(crate) fn view_range_ticks(&self) -> (i64, i64) {
        let w = self.tl_geom.as_ref().map(|g| g.rect.width() as f64).unwrap_or(0.0);
        (self.view_start as i64, (self.view_start + w * self.ticks_per_px) as i64)
    }

    /// Hint text about an obstacle: inside the kind — simply “an event is in the way
    /// (highlighted)”, outside it — with the side and the distance,
    /// so that the person does not have to “scroll there by hand” (item 5 of R.28).
    pub(crate) fn blocked_text(&self, b: crate::timeline::Blocked) -> String {
        use crate::timeline::{BlockView, block_view};
        let view = self.view_range_ticks();
        let secs = |ticks: i64| format!("{:.1}", ticks as f64 / 90_000.0);
        match block_view(b.span, view) {
            BlockView::Visible => tr(k::tl::hint::CLAMPED).to_string(),
            BlockView::Left(d) => crate::tf!(k::tl::hint::CLAMPED_FAR_LEFT, secs(d)),
            BlockView::Right(d) => crate::tf!(k::tl::hint::CLAMPED_FAR_RIGHT, secs(d)),
        }
    }

    /// Dump the playback “black box” to a file next to the project
    /// (an unnamed project — into the working directory). R.28, item 10.
    pub(crate) fn save_flight_log(&self) -> std::io::Result<String> {
        let path = match &self.project_path {
            Some(p) => p.with_extension("ktflight.log"),
            None => std::path::PathBuf::from("kt-flight.log"),
        };
        std::fs::write(&path, self.player.flight.dump())?;
        Ok(path.display().to_string())
    }

    pub fn note(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        self.push_notice(Severity::Info, text.clone());
        self.status = Some((text, std::time::Instant::now()));
    }

    /// Refusal: an impossible action is shown as an ERROR, not as a faded
    /// hint (rule 16) — in red in the status line, into the log and into the
    /// retrospective.
    pub fn refuse(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        crate::journal::log("REFUSE", &text);
        self.push_notice(Severity::Warn, text.clone());
        self.error_status = Some((text, std::time::Instant::now()));
    }

    /// Warning notification: to the status bar and to the retrospective (item 18).
    pub fn warn(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        // to the log — so that the complaint “it said something and then vanished” has an address
        crate::journal::log("WARN", &text);
        self.push_notice(Severity::Warn, text.clone());
        self.status = Some((text, std::time::Instant::now()));
    }

    /// Say ONCE per session that this file system’s directory is not flushed:
    /// the file is in place, but on power loss the rename itself may be lost
    /// (exFAT, network shares, non-unix).
    /// Write the recovery file RIGHT NOW, synchronously. Called where
    /// there will be no background thread anymore: `on_exit` and (item 354) `Drop`. The outcome goes
    /// to the log: there is no window, and there is no one to show it to.
    pub(crate) fn write_recovery_now(&mut self) {
        let target = self.recovery_target();
        let json = self.project_text(&self.model.document(), target.parent());
        match write_project_guarded(&self.recovery_rev, &target, self.model_rev, &json) {
            Ok(SaveOutcome::Wrote { dir_synced }) => {
                self.recovery_written = Some(target);
                if !dir_synced {
                    crate::journal::log("WARN", "recovery write: directory sync unsupported");
                }
            }
            Ok(SaveOutcome::SkippedNewerOnDisk) => {}
            Err(e) => crate::journal::log(
                "WARN",
                &format!("recovery write failed: {} ({e})", target.display()),
            ),
        }
    }

    /// Look for unsaved work and offer it. Called at startup and
    /// from the menu item: the mechanism must work AGAIN (rule 14), not
    /// only once at launch.
    pub fn offer_recovery(&mut self, quiet: bool) {
        let list = recovery_candidates(self.project_path.as_deref());
        self.offer_recovery_list(list, quiet);
    }

    pub(crate) fn offer_recovery_list(&mut self, list: Vec<RecoveryCandidate>, quiet: bool) {
        self.recovery_offer = list;
        self.recovery_pick = 0;
        self.recovery_open = !self.recovery_offer.is_empty();
        if self.recovery_offer.is_empty() && !quiet {
            self.note(tr(k::msg::RECOVERY_NONE));
        }
    }

    /// Take the work from the recovery file INTO THE EDITOR. The source file
    /// is not touched: recovery can be repeated (changed your mind, cancelled, opened
    /// the wrong candidate), and only the user has the right to destroy the source of
    /// unsaved work.
    pub fn restore_recovery(&mut self, i: usize) {
        let Some(c) = self.recovery_offer.get(i).cloned() else { return };
        match load_project_file(&c.path) {
            Ok((project, retained, notices)) => {
                self.model = kt_model::TimelineModel::new(project);
                self.retained = retained;
                self.model_rev += 1;
                self.push_project_to_player();
                self.sync_media_sources();
                // the work is IN THE EDITOR and nowhere else: the asterisk stays lit until
                // the user presses Ctrl+S
                self.dirty_content = true;
                self.recovery_open = false;
                for n in notices {
                    self.warn(n);
                }
                self.note(crate::tf!(k::msg::RECOVERY_RESTORED, c.path.display()));
            }
            Err(e) => self.warn(crate::tf!(k::msg::RECOVERY_FAILED, c.path.display(), e)),
        }
    }

    /// Discard the recovery file: the work from it is not needed.
    pub fn forget_recovery(&mut self, i: usize) {
        if i >= self.recovery_offer.len() {
            return;
        }
        let c = self.recovery_offer.remove(i);
        let _ = std::fs::remove_file(&c.path);
        if self.recovery_written.as_deref() == Some(c.path.as_path()) {
            self.recovery_written = None;
        }
        self.recovery_pick = self.recovery_pick.min(self.recovery_offer.len().saturating_sub(1));
        self.recovery_open = !self.recovery_offer.is_empty();
        self.note(crate::tf!(k::msg::RECOVERY_FORGOTTEN, c.path.display()));
    }

    /// Sync two selected events by sound (R.49, item 375).
    ///
    /// ONE BODY for both paths — the event menu item and the main menu item
    /// (rule 17: paired ways behave the same). Impossibility is shown as an ERROR,
    /// not as a faded hint (rule 16): “the index is still being built” and
    /// “the records do not match” are different causes, and both are
    /// named aloud.
    pub fn sync_selected_by_audio(&mut self) {
        let ids: Vec<kt_model::ClipId> = self.selected.iter().copied().collect();
        if ids.len() != 2 {
            self.warn(tr(k::msg::SYNC_NEED_TWO));
            return;
        }
        let (a, b) = (ids[0], ids[1]);
        let (Some(ca), Some(cb)) = (self.model.clip(a).cloned(), self.model.clip(b).cloned())
        else {
            return;
        };
        // The peak pyramids of both files must be ready. “Not ready yet” is an
        // honest refusal, not a “zero shift”: zero is indistinguishable from “they
        // already match anyway”, and the editor would believe it.
        let peaks = self.peaks.lock().unwrap();
        let (Some(pa), Some(pb)) = (peaks.get(&ca.media), peaks.get(&cb.media)) else {
            drop(peaks);
            self.warn(tr(k::msg::SYNC_PENDING));
            return;
        };
        let la: Vec<Vec<(i16, i16)>> = pa.iter().map(|(l, _)| l.clone()).collect();
        let lb: Vec<Vec<(i16, i16)>> = pb.iter().map(|(l, _)| l.clone()).collect();
        drop(peaks);
        let Some(off) = kt_media::sync::offset_from_pyramids(&la, &lb, kt_media::PEAK_BLOCK_FRAMES)
        else {
            self.warn(tr(k::msg::SYNC_NO_MATCH));
            return;
        };
        let cmds =
            match kt_model::ops::sync_by_audio_commands(self.model.project(), a, b, off.ticks) {
                kt_model::ops::SyncPlan::Move(c) => c,
                kt_model::ops::SyncPlan::AlreadyAligned => {
                    self.note(crate::tf!(k::msg::SYNC_DONE, 0, (off.confidence * 100.0).round()));
                    return;
                }
                kt_model::ops::SyncPlan::OutOfRange => {
                    self.warn(tr(k::msg::SYNC_OUT_OF_RANGE));
                    return;
                }
            };
        if self.apply_batch(cmds) {
            self.note(crate::tf!(
                k::msg::SYNC_DONE,
                off.ticks.as_ms(),
                (off.confidence * 100.0).round()
            ));
        }
    }

    // ---- nested reels (R.50, item 381) ------------------------------------

    /// Collapse the selection into a reel. ONE BODY for all call paths (rule 17):
    /// the event menu, the main menu.
    pub fn collapse_selection(&mut self) {
        let ids = self.selected_with_groups();
        if ids.is_empty() {
            self.warn(tr(k::msg::NEST_NEED_SELECTION));
            return;
        }
        let mut n = kt_model::ops::NestCounters::of(self.model.project());
        let taken: Vec<String> =
            self.model.project().sequences.iter().map(|s| s.name.clone()).collect();
        let name = kt_model::unique_name(&taken, tr(k::msg::NEST_DEFAULT_NAME), "copy");
        let (cmds, rep) =
            kt_model::ops::collapse_into_sequence(self.model.project(), &ids, &name, n.ids());
        if cmds.is_empty() {
            self.warn(tr(k::msg::NEST_NEED_SELECTION));
            return;
        }
        // Material is NOT cut: the events moved into the reel whole, so there must be
        // no handle restore (apply_batch_restoring) here — it would
        // hand the material back to clips that are no longer at the root.
        if !self.apply_batch(cmds) {
            return;
        }
        if let Some(seq) = rep.seq
            && let Some(inst) = self.model.project().clips.iter().find(|c| c.media == seq)
        {
            let id = inst.id;
            self.select_only(id);
        }
        self.note(crate::tf!(k::msg::NEST_COLLAPSED, name.clone()));
        // A morpher layer that covered the collapsed material FROM OUTSIDE now hits
        // the assembled picture, not each piece: the picture has legitimately
        // changed, and the person must learn of it IMMEDIATELY, not go looking
        // for the cause later.
        if rep.outer_layers > 0 {
            self.warn(crate::tf!(k::msg::NEST_OUTER_LAYERS, rep.outer_layers));
        }
        // What MOVED INSIDE must also be named (rule 16):
        // the person looks for their layer or camera where they used to be, and they are
        // no longer there. Before R.55 the layer moved and silently died inside (the order
        // of the reel’s rows was the default, and nothing ended up
        // under the morph row), and the camera did not move at all.
        if rep.moved_layers > 0 || rep.moved_cameras > 0 {
            self.note(crate::tf!(k::msg::NEST_MOVED_EXTRAS, rep.moved_layers, rep.moved_cameras));
        }
        if rep.kept_ref_layers > 0 {
            self.warn(crate::tf!(k::msg::NEST_REF_LAYERS_STAY, rep.kept_ref_layers));
        }
    }

    /// Unfold the instance back onto tracks.
    pub fn expand_nest(&mut self, id: ClipId) {
        let mut n = kt_model::ops::NestCounters::of(self.model.project());
        let (cmds, rep) = kt_model::ops::expand_instance(self.model.project(), id, n.ids());
        if cmds.is_empty() {
            self.warn(tr(k::msg::NEST_NOT_A_NEST));
            return;
        }
        if !self.apply_batch(cmds) {
            return;
        }
        // rule 16: what was removed must be NAMED, not discovered later
        if rep.chain_dropped || rep.level_dropped {
            self.warn(tr(k::msg::NEST_EXPAND_DROPPED));
        } else {
            self.note(tr(k::msg::NEST_EXPANDED));
        }
    }

    /// Detach the instance from the other places: the content is copied into its own reel.
    pub fn nest_make_independent(&mut self, id: ClipId) {
        let p = self.model.project();
        let Some(seq) = p.clips.iter().find(|c| c.id == id).and_then(|c| p.sequence(c.media))
        else {
            self.warn(tr(k::msg::NEST_NOT_A_NEST));
            return;
        };
        let taken: Vec<String> = p.sequences.iter().map(|s| s.name.clone()).collect();
        let name = kt_model::unique_name(&taken, &seq.name, "copy");
        let mut n = kt_model::ops::NestCounters::of(p);
        let cmds = kt_model::ops::make_independent(self.model.project(), id, &name, n.ids());
        if self.apply_batch(cmds) {
            self.note(crate::tf!(k::msg::NEST_INDEPENDENT, name));
        }
    }

    /// Jump to the NEXT place where the same reel stands (R.50, B3).
    ///
    /// The “×N” badge says there are several places; without this verb it would be
    /// a message without a continuation — “there are three, find them yourself”.
    pub fn goto_next_place(&mut self, id: ClipId) {
        let Some(c) = self.model.clip(id).cloned() else { return };
        let here = c.start;
        let mut places: Vec<kt_model::ClipEvent> =
            self.model.project().clips.iter().filter(|x| x.media == c.media).cloned().collect();
        if places.len() < 2 {
            self.warn(tr(k::msg::NEST_ONE_PLACE));
            return;
        }
        places.sort_by_key(|x| (x.start, x.id));
        // in a cycle: after the last one — the first, otherwise the verb would hit
        // a dead end exactly where it is most needed
        let next =
            places.iter().find(|x| x.start > here).or_else(|| places.first()).cloned().unwrap();
        self.select_only(next.id);
        self.player.seek(next.start);
        self.center_view_on(next.start);
    }

    /// Bring the moment into view (a crumb of common arithmetic: we do not create
    /// our own scroll formula at the call sites).
    fn center_view_on(&mut self, t: Tick) {
        let half = self.timeline_rect.width().max(1.0) as f64 * self.ticks_per_px / 2.0;
        self.view_start = (t.0 as f64 - half).max(self.view_floor(self.ticks_per_px));
    }

    /// LOWER TIME BOUND FOR GESTURES over events (R.58, item 462; owner’s decision):
    /// at the root zero is a wall; inside a full entry the screen axis = the reel’s
    /// axis, and its zero is NOT a wall — an event legitimately moves
    /// left of zero (there its instance window does not play anyway). The cursor and
    /// playback do not go here: they have their own floor,
    /// `timeline::CURSOR_FLOOR`. One function for all gestures; guard over the
    /// codebase — t637c.
    pub fn time_floor(&self) -> i64 {
        if self.model.editing().is_some() { -(i64::MAX / 4) } else { 0 }
    }

    /// Lower bound of SCROLLING (`view_start`), in ticks: at the root the old
    /// −40 px, inside a reel — up to the earliest event of the content (it
    /// may stand left of zero), and the same −40 px margin.
    pub fn view_floor(&self, tpp: f64) -> f64 {
        let earliest = if self.model.editing().is_some() {
            self.model.project().clips.iter().map(|c| c.start.0).min().unwrap_or(0).min(0)
        } else {
            0
        };
        earliest as f64 - 40.0 * tpp
    }

    /// Create an EMPTY reel (R.55, C6).
    ///
    /// It is immediately visible in the bin, and you can already work with it: put it
    /// on the timeline, unfold it, rename it, remove it. Until this round
    /// `AddSequence` was constructed only inside collapsing and detaching — there was
    /// nothing to create a composition from scratch with.
    pub fn create_empty_reel(&mut self) {
        let taken: Vec<String> =
            self.model.document().sequences.iter().map(|s| s.name.clone()).collect();
        let name = kt_model::unique_name(&taken, tr(k::msg::NEST_DEFAULT_NAME), "copy");
        let id = kt_model::MediaId(kt_model::ops::NestCounters::of(&self.model.document()).media);
        if self.apply(Command::AddSequence { id, name: name.clone() }) {
            self.note(crate::tf!(k::msg::NEST_CREATED, name));
        }
    }

    /// Remove reels that nobody is looking at any more.
    pub fn remove_unused_seqs(&mut self) {
        let cmds = kt_model::ops::remove_unused_sequences(self.model.project());
        let n = cmds.len();
        if n == 0 {
            self.note(tr(k::msg::NEST_NONE_UNUSED));
            return;
        }
        if self.apply_batch(cmds) {
            self.note(crate::tf!(k::msg::NEST_REMOVED_UNUSED, n));
        }
    }

    /// Unfold an event’s reel IN PLACE (R.55, C5) — the default mode.
    ///
    /// We do NOT leave the root: the whole program plays, the cursor is on its
    /// axis, neighbours and transitions stay in place. This is exactly what the owner asked for —
    /// “the main timeline gives playback and the composition
    /// plays at the same time”. The price: there are two coordinate systems on screen, and
    /// the band’s rows must be visibly distinguishable.
    ///
    /// It also works INSIDE a full entry (R.58, item 460): the band is counted from
    /// the canon, and after `swap_reel` the canon is the current reel — its bases are
    /// its own, and the application has nothing to do here. The earlier guard
    /// “can unfold only at the root” was an error in thinking, not in the model.
    pub fn expand_in_place(&mut self, id: ClipId) {
        if self.model.bands().iter().any(|b| b.inst == id) {
            self.collapse_in_place(id);
            return; // the gesture works AGAIN (rule 14): the second time it collapses
        }
        if !self.is_nest(id) {
            self.warn(tr(k::msg::NEST_NOT_A_NEST));
            return;
        }
        // ONE band at a time (R.55, C5). The limit stands as a constant, not
        // scattered: at a depth of four unfoldings these would be four interdependent
        // offsets for each lane command and four dimming levels across two dozen rows —
        // unreadable. It is lifted by one line plus a test.
        let had = !self.model.bands().is_empty();
        self.model.collapse_all_bands();
        if !self.model.expand_band(id) {
            self.warn(tr(k::msg::NEST_NOT_A_NEST));
            return;
        }
        if had {
            self.note(tr(k::msg::NEST_ONE_BAND));
        }
        // the instance stays SELECTED: the next Enter must find the target
        self.after_bands_changed(Some(id));
    }

    /// Collapse the disclosure back. No history step and no trace in the file:
    /// a disclosure is a workplace, not work.
    pub fn collapse_in_place(&mut self, id: ClipId) {
        if self.model.collapse_band(id) {
            self.after_bands_changed(Some(id));
        }
    }

    pub fn collapse_all_in_place(&mut self) -> bool {
        if self.model.bands().is_empty() {
            return false;
        }
        self.model.collapse_all_bands();
        self.after_bands_changed(None);
        true
    }

    /// Common to expanding and collapsing: the project has NOT changed (the same document
    /// goes to the player and to the file), but the view, the selection and the row caches have.
    ///
    /// `keep` — the instance whose selection survives a change of rows. It is a ROOT
    /// event, and the rule “the selection lives in exactly one reel” protects against
    /// addresses of the BAND; holding it is legitimate and necessary: without this,
    /// the second Enter found no target and hit empty space (R.56, item 451).
    fn after_bands_changed(&mut self, keep: Option<ClipId>) {
        // the selection lives in EXACTLY ONE reel: after the set of rows changes,
        // keeping foreign addresses in it means promising a gesture that will not come
        self.selected.retain(|id| Some(*id) == keep);
        self.selected_morph.clear();
        self.selected_camera.clear();
        self.model_rev += 1;
        self.dirty_ui = true;
    }

    /// What Enter does on the selected nested item (R.56, item 451).
    ///
    /// Owner’s decision: the quick gesture gives the NEXT mode, not a toggle.
    /// The first Enter unfolds in place (the default mode), the second one on an already
    /// unfolded item — full entry. Collapsing is still done by a repeated double
    /// click and Esc: the key has one direction, the mouse has another, and both are
    /// named in the menu.
    pub fn enter_pressed_on_nest(&self, id: ClipId) -> EnterOnNest {
        if !self.is_nest(id) {
            return EnterOnNest::NotANest;
        }
        // decided ONLY by the band, the context is irrelevant (R.58, item 460):
        // inside a reel the cascade is the same — the first Enter unfolds, the second enters
        if self.model.bands().iter().any(|b| b.inst == id) {
            EnterOnNest::Enter
        } else {
            EnterOnNest::Expand
        }
    }

    /// Enter the reel of event `id` (R.50, B1).
    ///
    /// The cursor is TRANSLATED, not reset: `t_inner = source_in + (t_outer −
    /// start)`. The person entered rather than jumped — the preview shows the same
    /// frame, and this is the only sign that they landed where they
    /// had been aiming.
    pub fn enter_nest(&mut self, id: ClipId) {
        let Some(c) = self.model.clip(id).cloned() else { return };
        let Some(seq) = self.model.project().sequence(c.media).map(|s| (s.id, s.name.clone()))
        else {
            self.warn(tr(k::msg::NEST_NOT_A_NEST));
            return;
        };
        if self.nest_path.len() >= kt_model::MAX_NEST_DEPTH {
            self.warn(crate::tf!(k::msg::NEST_TOO_DEEP, kt_model::MAX_NEST_DEPTH));
            return;
        }
        let here = self.player.transport.playhead();
        // the band of the context being left is remembered BEFORE `set_editing`: it
        // clears the bands unconditionally (a model invariant), and restoring it on exit
        // is the application’s job (R.58, item 461)
        let band = self.model.bands().first().map(|b| b.inst);
        self.nest_path.push(NestStep {
            parent: self.model.editing(),
            inst: id,
            name: seq.1,
            view_start: self.view_start,
            ticks_per_px: self.ticks_per_px,
            tl_scroll: self.tl_scroll,
            cursor: here,
            region: self.model.project().region,
            selected: std::mem::take(&mut self.selected),
            selected_morph: std::mem::take(&mut self.selected_morph),
            selected_camera: std::mem::take(&mut self.selected_camera),
            band,
        });
        self.model.set_editing(Some(seq.0));
        // returned to the reel where the disclosure was already — it is in place (R.58)
        if let Some(inst) = self.reel_bands.get(&seq.0).copied() {
            self.model.expand_band(inst);
        }
        // the region lives in the TIME AXIS of the reel, and outside it was in the root’s axis:
        // carrying it inside would cover the wrong place
        self.model.set_region(None);
        // a selection outside, inside is meaningless: there are other events there
        // (the sets themselves are already taken into the snapshot above)
        self.selected.clear();
        // The cursor is TRANSLATED, not reset — but only while there is somewhere
        // to translate it to. If it stood OUTSIDE the instance body (the person entered
        // from the menu without placing the cursor), the translated position ends up
        // beyond the reel’s content, and “entered” would mean “staring at the void”.
        // Then the cursor settles at the nearest edge of the material: that is the
        // nearest frame the reel shows.
        // The end of the content is taken AFTER the swap: `set_editing` has already
        // swapped the root and the reel, and `sequence(c.media)` now gives the record
        // that holds the ROOT’s content, not the reel’s.
        let end = self
            .model
            .project()
            .clips
            .iter()
            .map(|x| x.start + x.duration)
            .max()
            .unwrap_or(c.duration);
        let inner = (c.source_in + (here - c.start)).clamp(Tick::ZERO, end.max(Tick::ZERO));
        self.on_context_changed(inner);
        // The VIEW is brought to the translated cursor. Without this the scroll
        // stayed in the ROOT’s axis: entering a nested item sitting at the tenth
        // minute, the person saw an empty screen, because the reel’s content
        // lies near its zero. Exiting does not touch the view — there it is RESTORED
        // from the snapshot as a whole (t456).
        self.center_view_on(inner);
    }

    /// Go up one level. `None` in the path — we are already at the root.
    pub fn leave_nest(&mut self) {
        let Some(step) = self.nest_path.pop() else { return };
        let inner = self.player.transport.playhead();
        // the disclosure of THIS reel is remembered before exit: `set_editing` would
        // wipe it, and the person may come back here (R.58, item 461)
        if let Some(cur) = self.model.editing() {
            match self.model.bands().first().map(|b| b.inst) {
                Some(inst) => {
                    self.reel_bands.insert(cur, inst);
                }
                None => {
                    self.reel_bands.remove(&cur);
                }
            }
        }
        self.model.set_editing(step.parent);
        self.model.set_region(step.region);
        self.view_start = step.view_start;
        self.ticks_per_px = step.ticks_per_px;
        self.tl_scroll = step.tl_scroll;
        self.selected = step.selected;
        self.selected_morph = step.selected_morph;
        self.selected_camera = step.selected_camera;
        // the cursor is translated back by the SAME rule; the event could have
        // moved or vanished under undo — then we return to where they stood
        // before entering
        let back = match self.model.clip(step.inst).cloned() {
            Some(c) => (c.start + (inner - c.source_in)).max(Tick::ZERO),
            None => step.cursor,
        };
        self.on_context_changed(back);
        // THE BAND IS RESTORED (R.58, item 461; owner’s decision — a revision of
        // R.56 item 5): a disclosure is a workplace, and leaving the reel does not
        // take it away, including the band entered via the second Enter.
        // It is recalculated afresh: the content could have changed inside.
        // If the instance vanished or stopped being a nested item — silently, without a band.
        // The selection has already returned from the snapshot — `after_bands_changed`
        // holds exactly it, if it is a single one (the instance).
        if let Some(inst) = step.band
            && self.model.expand_band(inst)
        {
            let keep = (self.selected.len() == 1).then(|| *self.selected.iter().next().unwrap());
            self.after_bands_changed(keep);
        }
    }

    /// Common to entry and exit: the frame caches are rebuilt, the cursor lands on
    /// the translated spot, and the player gets what it must play HERE
    /// (`player_project`: at the root — the document, inside a reel — the reel itself).
    fn on_context_changed(&mut self, cursor: Tick) {
        self.player.seek(cursor);
        self.blocked = None;
        // revision — what the frame caches hang on (coverage, names, paths) and
        // the renderer build: a context change alters EVERYTHING they hold
        self.model_rev += 1;
        self.push_project_to_player();
        self.sync_media_sources();
    }

    /// Path crumbs: the root and each reel on the way inside.
    pub fn nest_crumbs(&self) -> Vec<String> {
        self.nest_path.iter().map(|s| s.name.clone()).collect()
    }

    /// Reel name window (reference — the preset name window, t345).
    fn draw_seq_name(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.seq_name.take() else { return };
        let mut go = false;
        let mut cancel = false;
        let mut open = true;
        let taken: Vec<String> = self
            .model
            .project()
            .sequences
            .iter()
            .filter(|s| s.id != d.seq)
            .map(|s| s.name.clone())
            .collect();
        egui::Window::new(tr(k::msg::NEST_RENAME_TITLE))
            .id(egui::Id::new("seq_name"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::msg::NEST_NAME_LABEL));
                    let resp = ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(200.0));
                    if d.focus {
                        resp.request_focus();
                        d.focus = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                });
                // how many places change together with this — our only answer to the common
                // affliction of Premiere/FCP/VEGAS, and it is free
                ui.label(
                    egui::RichText::new(crate::tf!(k::msg::NEST_USES, self.nest_uses(d.seq)))
                        .weak()
                        .small(),
                );
                ui.horizontal(|ui| {
                    if ui.button(tr(k::msg::NEST_RENAME_DO)).clicked() {
                        go = true;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if go && !d.name.trim().is_empty() {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("seq_name")));
            let name = kt_model::unique_name(&taken, d.name.trim(), "copy");
            self.apply(Command::RenameSequence { id: d.seq, name });
            return;
        }
        if cancel || !open {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("seq_name")));
            return;
        }
        self.seq_name = Some(d);
    }

    /// Is the event a NESTED item? (R.50)
    /// The rectangle of the media card in the bin (frame back) — where the person
    /// acts. The list of this frame is short (the bin’s cards), a linear search is cheaper
    /// than a map. It is called by the harness tests and live scenarios: the product path
    /// does not need the card’s coordinate.
    #[allow(dead_code)]
    pub fn bin_card_rect(&self, id: MediaId) -> Option<egui::Rect> {
        self.bin_card_rects.iter().find(|(m, _)| *m == id).map(|(_, r)| *r)
    }

    pub fn is_nest(&self, id: ClipId) -> bool {
        self.model.clip(id).is_some_and(|c| self.model.project().sequence(c.media).is_some())
    }

    /// How many places in the tree look at this reel.
    pub fn nest_uses(&self, seq: kt_model::MediaId) -> usize {
        self.model.project().sequence_uses(seq)
    }

    /// Answer the given question. Moved out of drawing so that the test
    /// answers with exactly the same code as the button (rule 17).
    pub fn answer_ask(&mut self, ans: Answer) {
        let Some(a) = self.ask.take() else { return };
        match (a, ans) {
            (Ask::SaveOverLock { .. }, Answer::Yes) => {
                self.lock_override = true;
                self.save();
            }
            (Ask::SaveOverLock { .. }, _) => {}
            (Ask::Unsaved(_), Answer::Cancel) => {}
            (Ask::Unsaved(p), Answer::Yes) => {
                self.save();
                // the save may not have happened (Save-As was cancelled, the write was refused,
                // the question about a busy project) — then we must not leave:
                // exactly this is how “save and exit” differs from “exit”
                if !self.dirty_content && !self.dirty_ui {
                    self.proceed(p);
                }
            }
            // ABANDONED PROJECT: three answers, and all three DIFFERENT in meaning —
            // exactly why `Answer` has three of them rather than a `bool`
            (Ask::DropProject, Answer::Yes) => {
                // “Open as project” — through the same door as the menu item:
                // the question about unsaved work and the lock on the file (R.49).
                if self.guard_unsaved(Pending::DropOpen) {
                    self.open_dropped_now();
                }
            }
            (Ask::DropProject, Answer::No) => self.insert_dropped_as_sequence(),
            (Ask::DropProject, Answer::Cancel) => {
                self.pending_drop = None;
            }
            (Ask::Unsaved(p), Answer::No) => {
                // the work is declared unneeded: we clear the flags BEFORE the transition, otherwise
                // `Drop` would revive it from the recovery file
                self.dirty_content = false;
                self.dirty_ui = false;
                self.drop_own_recovery();
                self.proceed(p);
            }
        }
    }

    /// Do what the question was asked for.
    fn proceed(&mut self, p: Pending) {
        match p {
            Pending::New => *self = Self::new_empty(None),
            Pending::Open => self.open_project_dialog_now(),
            Pending::ImportOlder => self.import_older_dialog_now(),
            Pending::Quit => self.quit_confirmed = true,
            Pending::DropOpen => self.open_dropped_now(),
        }
    }

    /// Open the abandoned project — by the SAME path as the “Open” menu item
    /// (R.55, C6).
    ///
    /// Before this round the drop did `*self = app` bypassing both the question about
    /// unsaved work and the file lock: the work disappeared without a single question, and
    /// a second copy of the program on the same file was not detected.
    fn open_dropped_now(&mut self) {
        let Some((path, _)) = self.pending_drop.take() else { return };
        match KtApp::open(&path) {
            Ok(mut app) => {
                app.claim_project(&path);
                *self = app;
            }
            Err(e) => self.warn(format!("{}: {e}", path.display())),
        }
    }

    /// Insert the abandoned project as a COMPOSITION into the current edit (R.55, C6).
    fn insert_dropped_as_sequence(&mut self) {
        let Some((path, target)) = self.pending_drop.take() else { return };
        let src = match load_project_file(&path) {
            Ok((p, _, notices)) => {
                for n in notices {
                    self.warn(n);
                }
                p
            }
            Err(e) => {
                self.warn(format!("{}: {e}", path.display()));
                return;
            }
        };
        let name = kt_model::unique_name(
            &self.model.project().sequences.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default().trim(),
            "copy",
        );
        let (at, lane) = match target {
            Some((at, (_, lane))) => (at, lane),
            None => (self.player.transport.playhead(), 0),
        };
        let doc = self.model.document();
        let mut n = kt_model::ops::NestCounters::of(&doc);
        let (cmds, rep) = kt_model::ops::import_as_sequences(
            self.model.project(),
            &src,
            &name,
            at,
            lane,
            n.ids(),
        );
        if cmds.is_empty() {
            self.warn(tr(k::msg::IMPORT_EMPTY));
            return;
        }
        if !self.apply_batch(cmds) {
            return;
        }
        self.note(crate::tf!(k::msg::IMPORT_DONE, name.clone(), rep.media_added));
        // What did NOT arrive must be NAMED (rule 16): the areas,
        // markers and settings of the other project belong to the DOCUMENT, not the
        // edit, and they must not replace one’s own.
        if rep.dropped_extras > 0 {
            self.warn(crate::tf!(k::msg::IMPORT_DROPPED, rep.dropped_extras));
        }
    }

    /// Ask if there is something to lose. `true` — one may act at once.
    ///
    /// Before R.49 the program asked NOTHING, and that was a legitimate choice
    /// (verdict F-RES-005): the background autosave wrote straight into the user’s file,
    /// so there was nothing to lose. The owner removed this (item 351) — together with
    /// the right not to ask.
    pub fn guard_unsaved(&mut self, p: Pending) -> bool {
        if !self.dirty_content && !self.dirty_ui {
            return true;
        }
        self.ask = Some(Ask::Unsaved(p));
        false
    }

    /// Take the project (item 352). A second copy is not forbidden — it degrades
    /// honestly: a warning with the number of the other process, its own
    /// recovery, its own temporary file and a question at the first save.
    pub(crate) fn claim_project(&mut self, path: &Path) {
        // the old lock is released BEFORE taking the new one: otherwise “Save as…”
        // to the same file would refuse against itself
        self.project_lock = None;
        self.lock_holder = None;
        self.lock_override = false;
        match crate::project_lock::acquire(path) {
            crate::project_lock::LockState::Owned(l) => self.project_lock = Some(l),
            crate::project_lock::LockState::Busy { pid } => {
                self.lock_holder = Some(pid);
                match pid {
                    Some(n) => self.warn(crate::tf!(k::msg::PROJECT_LOCKED_BY, n)),
                    None => self.warn(tr(k::msg::PROJECT_LOCKED)),
                }
            }
            // failing to take the lock at all — silently behave as the owner:
            // refusing to work because the data directory is missing is not allowed
            crate::project_lock::LockState::Unavailable => {}
        }
    }

    /// The address of its own recovery file. The second copy’s has the process number:
    /// otherwise two copies of one project would write to one file and
    /// overwrite each other’s unsaved work — exactly what this whole
    /// wave is about.
    pub(crate) fn recovery_target(&self) -> PathBuf {
        let base = recovery_path_for(self.project_path.as_deref());
        match self.lock_holder {
            None => base,
            Some(_) => {
                let name =
                    base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                base.with_file_name(format!(
                    "{name}.{}.{}",
                    std::process::id(),
                    kt_model::project_file::EXTENSION
                ))
            }
        }
    }

    /// Remove the recovery file written by THIS copy.
    pub(crate) fn drop_own_recovery(&mut self) {
        if let Some(p) = self.recovery_written.take() {
            let _ = std::fs::remove_file(p);
        }
    }

    fn warn_if_dir_unsynced(&mut self, dir_synced: bool) {
        if !dir_synced && !self.dir_sync_warned {
            self.dir_sync_warned = true;
            self.warn(tr(k::msg::DIR_SYNC_UNSUPPORTED));
        }
    }

    fn push_notice(&mut self, severity: Severity, text: String) {
        let at = chrono_like_now();
        // Consecutive duplicates collapse into “text ×N”, but the counter of
        // unread items ALWAYS grows (R.49, item 350): earlier the early return
        // suppressed it too, and a second “save failed” in a row did not colour
        // the bell — the only channel for reporting a write error
        // died exactly when the error repeated.
        if let Some(last) = self.notices.back_mut()
            && last.text == text
        {
            last.repeats += 1;
            last.at = at;
            last.severity = severity;
            self.notices_unread += 1;
            return;
        }
        self.notices.push_back(Notice { at, severity, text, repeats: 1 });
        if self.notices.len() > 200 {
            self.notices.pop_front();
        }
        self.notices_unread += 1;
    }

    /// The project that the PLAYER plays is the ONLY place where it is decided
    /// what exactly is heard and seen (R.55, C1).
    ///
    /// Standing at the root, the document plays. Standing INSIDE a reel, the REEL ITSELF plays
    /// from its own zero: before R.55 `document()` always went here, and the person,
    /// having entered the composition, saw its events on the timeline, but heard and saw
    /// the ROOT, taken at the moment numerically equal to the reel’s local time.
    /// This coincided exactly when the instance stood at zero — which is why
    /// no test and no live scenario ever saw the defect.
    ///
    /// The FILE and the EXPORT still take `document()`: a mismatch there would mean
    /// writing to disk something other than what the person edited (t456b).
    pub(crate) fn player_project(&self) -> kt_model::Project {
        match self.model.editing() {
            Some(_) => self.model.reel_document(),
            None => self.model.document(),
        }
    }

    /// Give the player what it must play. There must be no `set_project` of its own
    /// in the application: there were eight of them, and the first edit
    /// inside a reel returned the root to the player (guard — t456c).
    pub(crate) fn push_project_to_player(&mut self) {
        let p = self.player_project();
        self.player.set_project(p);
    }

    pub fn apply(&mut self, cmd: Command) -> bool {
        self.model_rev += 1;
        // through `apply_batch`, not `apply`: the kinship of born events
        // arrives by ONE path (item 357), and a single cut is no exception
        match self.model.apply_batch(vec![cmd]) {
            Ok(births) => {
                self.push_project_to_player();
                self.sync_media_sources();
                self.dirty_content = true;
                self.bump_clip_hw();
                self.inherit_extras(&births);
                true
            }
            Err(e) => {
                self.refuse_command(&e);
                false
            }
        }
    }

    // ---- event selection (R.33 item 7) ---------------------------------------

    /// Whose reel this is: `None` — the root, `Some` — an unfolded band (R.55, C5).
    pub fn reel_of_clip(&self, id: ClipId) -> Option<kt_model::MediaId> {
        let c = self.model.clip(id)?;
        self.model.band_of_row(kt_model::lane_key(c).0, c.lane).map(|b| b.seq)
    }

    /// Select exactly this event (a plain click).
    pub fn select_only(&mut self, id: ClipId) {
        self.selected.clear();
        self.selected.insert(id);
    }

    /// Ctrl+click: add to / remove from the selection.
    ///
    /// **The selection lives in EXACTLY ONE reel** (R.55, C5). A mixed
    /// selection would look ordinary, and a gesture on it would go only
    /// halfway: `selected_with_groups` collects groups within one
    /// space, and commands on events of different reels cannot both survive a single batch
    /// together. So a click in another band REPLACES the selection and
    /// says so — silently dropping half would be worse.
    pub fn toggle_selected(&mut self, id: ClipId) {
        if self.selected.remove(&id) {
            return;
        }
        let here = self.reel_of_clip(id);
        if self.selected.iter().any(|o| self.reel_of_clip(*o) != here) {
            self.selected.clear();
            self.note(tr(k::msg::NEST_SELECTION_MOVED));
        }
        self.selected.insert(id);
    }

    pub fn is_selected(&self, id: ClipId) -> bool {
        self.selected.contains(&id)
    }

    /// The single selected event — for operations for which a set has no defined
    /// meaning (A/V magnet, breaking a link,
    /// moving to the adjacent track, trim to the cursor).
    pub fn selected_one(&self) -> Option<ClipId> {
        (self.selected.len() == 1).then(|| self.selected.iter().copied().next()).flatten()
    }

    /// The same, but the silence is replaced by a clear message: “choose an event”
    /// (item 16 of the checklist — an impossible action is visible, not swallowed).
    fn require_one_selected(&mut self) -> Option<ClipId> {
        match self.selected_one() {
            Some(id) => Some(id),
            None => {
                self.note(tr(k::msg::PICK_CLIP));
                None
            }
        }
    }

    /// The selected events together with their link groups (move, delete,
    /// copy, morpher targets — everything works on this set).
    pub fn selected_with_groups(&self) -> Vec<ClipId> {
        let p = self.model.project();
        let groups: Vec<GroupId> = p
            .clips
            .iter()
            .filter(|c| self.selected.contains(&c.id))
            .filter_map(|c| c.link_group)
            .collect();
        let mut ids: Vec<ClipId> = p
            .clips
            .iter()
            .filter(|c| {
                self.selected.contains(&c.id) || c.link_group.is_some_and(|g| groups.contains(&g))
            })
            .map(|c| c.id)
            .collect();
        ids.sort_by_key(|i| i.0);
        ids.dedup();
        ids
    }

    /// A live edit INSIDE a gesture: commands with one `gesture` collapse into
    /// one undo step (dragging a transition edge, a session of the FX/morpher window).
    ///
    /// All such edits must go through this wrapper. While each one called
    /// `model.apply_coalesced` itself, `model_rev` did not grow — and the background autosave,
    /// whose gate was “`model_rev` == the captured revision”, cleared the asterisk, although
    /// the edits were made AFTER the snapshot: the changes looked saved and
    /// were lost on exit (item 200).
    ///
    /// We do not show an error: the path is live, the command is sent every drag frame, and
    /// a badge on every frame would be noise — the gesture simply does not move further.
    pub fn apply_coalesced(&mut self, cmd: Command, gesture: u64) -> bool {
        // Law P (R.53, item 412): a command that does not touch the plan has no
        // right to clone the project and rebuild the graphs. An edit of
        // generator parameters is `SetMediaPath`, whose addressee is the worker
        // of this media, not the renderers (otherwise every keypress in the
        // title field would stop the audio-render thread).
        let touches_plan = cmd.touches_render_plan();
        if self.model.apply_coalesced(cmd, gesture).is_err() {
            return false;
        }
        self.model_rev += 1;
        if touches_plan {
            self.push_project_to_player();
        }
        self.sync_media_sources();
        self.dirty_content = true;
        true
    }

    /// A command batch + restoration of the crossfade “handles” that this gesture kills
    /// (R.26): pulled clips apart or deleted a neighbour — the material cut off by an overlap
    /// is restored by the same undo step. All new paths for deleting and
    /// moving clips must go through this wrapper, not through
    /// apply_batch, otherwise material will again start to be silently lost.
    pub fn apply_batch_restoring(&mut self, mut cmds: Vec<Command>) -> bool {
        if let Ok(after) = self.model.preview(&cmds) {
            cmds.extend(kt_model::ops::restore_handles(self.model.project(), &after));
        }
        self.apply_batch(cmds)
    }

    /// A reel stands in SEVERAL places, and the person edits it in the unfolded view —
    /// say so once (R.55, C5, owner’s decision).
    ///
    /// The edit changes ALL places: it is a reference, which is why one creates it. But
    /// it looks like an ordinary drag on the timeline, and staying silent means
    /// letting the person change three places while thinking they changed one
    /// (rule 16). Beyond this we stay silent: a warning on every edit
    /// would turn into noise that people stop reading.
    fn warn_shared_band(&mut self, cmds: &[Command]) {
        let touched: Vec<kt_model::MediaId> = cmds
            .iter()
            .filter_map(|c| c.target_clip())
            .filter_map(|id| self.reel_of_clip(id))
            .collect();
        for seq in touched {
            if self.shared_warned.contains(&seq) {
                continue;
            }
            let uses = self.model.document().sequence_uses(seq);
            if uses > 1 {
                self.shared_warned.insert(seq);
                self.warn(crate::tf!(k::msg::NEST_SHARED_EDIT, uses));
            }
        }
    }

    /// A command batch = one undo snapshot per gesture (atomic).
    pub fn apply_batch(&mut self, cmds: Vec<Command>) -> bool {
        self.model_rev += 1;
        self.warn_shared_band(&cmds);
        match self.model.apply_batch(cmds) {
            Ok(births) => {
                self.push_project_to_player();
                self.sync_media_sources();
                self.dirty_content = true;
                self.bump_clip_hw();
                self.inherit_extras(&births);
                true
            }
            Err(e) => {
                self.refuse_command(&e);
                false
            }
        }
    }

    // ---- parity operations (region/markers/clipboard/tracks) ----------------

    pub fn marker_toggle_at_cursor(&mut self) {
        let at = self.player.transport.playhead();
        self.marker_here(at);
    }

    /// The region by the span of an event’s link group — ONE body for Shift+double
    /// click and the menu item (item 17 of the checklist: paired ways behave the
    /// same).
    pub fn region_from_event(&mut self, id: ClipId) {
        let span = {
            let p = self.model.project();
            let group = p.clips.iter().find(|c| c.id == id).and_then(|c| c.link_group);
            let members: Vec<&ClipEvent> = p
                .clips
                .iter()
                .filter(|c| c.id == id || (group.is_some() && c.link_group == group))
                .collect();
            members
                .iter()
                .map(|c| c.start)
                .min()
                .zip(members.iter().map(|c| c.start + c.duration).max())
        };
        if let Some((start, end)) = span {
            self.model.set_region(Region::normalized(start, end));
            self.on_region_changed();
        }
    }

    /// A source file that came back to disk replaces its placeholder (R.33 item 10).
    /// The watchdog already watches whether files exist, so here there is only the reaction:
    /// the file is no longer offline, and a placeholder stands in its place —
    /// we reopen it IN THE BACKGROUND and swap the player’s source. The position,
    /// transport and the cache of other sources are not touched.
    /// The player’s sources catch up with the model (R.52): the diff of paths
    /// `document().media` against the snapshot of the last sync. A path change
    /// (generator window, undo/redo) = recreating the source via the factory
    /// (`replace_media` stops the worker and clears its cache); a disappearance of
    /// media = `remove_media` (at the same time an old debt is closed: “Remove from
    /// project” used to leave the worker alive); appearance WITHOUT manual registration
    /// (redo of an addition) = raising the source. Called from all edit wrappers
    /// and from undo/redo — otherwise an undo of a colour change would not reach
    /// the picture: the source lives outside the project→player sync.
    /// A substituted title font is a VISIBLE notification (R.52 C4).
    ///
    /// R.59, stage 2: the family lives on the EVENT, not in the media path —
    /// so it is asked here with the ready view, not with the path.
    fn note_missing_title_font(&mut self, kind: &kt_model::GeneratorKind) {
        let kt_model::GeneratorKind::Text { family, .. } = kind else { return };
        // about each family we speak ONCE per session (item 413): a title
        // changes on every character, and without dedup this is a storm
        if self.font_warned.contains(family) {
            return;
        }
        if let Some(warn) = crate::gen_ui::font_warning(kind) {
            self.font_warned.insert(family.clone());
            self.note(warn);
        }
    }

    /// GENERATOR VALUES reach the live sources (R.59, stage 2).
    ///
    /// Structural edits (an event was born, died, or changed kind) travel
    /// by themselves: their commands touch the plan, so `set_project` has already called
    /// `Player::sync_generators`. What remains here is exactly what the plan does NOT
    /// touch (law P) — the values and curves: their addressee is the worker of this
    /// event, not the renderers. Otherwise every keypress in a title
    /// would stop the audio-render thread, and this is exactly the owner’s complaint
    /// for which the `retune` lane was set up in R.53.
    fn sync_gen_sources(&mut self) {
        let mut cur: HashMap<ClipId, kt_model::GenState> = HashMap::new();
        {
            let doc = self.model.document();
            for c in doc.clips.iter().chain(doc.sequences.iter().flat_map(|s| s.clips.iter())) {
                if let Some(k) = &c.r#gen {
                    cur.insert(
                        c.id,
                        kt_model::GenState { kind: k.clone(), auto: c.gen_auto.clone() },
                    );
                }
            }
        }
        if cur == self.gen_states {
            return;
        }
        // the source’s passport may have changed (SMPTE with tone on —
        // is already SOUNDING media, a different set of clips): then the
        // retune honestly refuses, and the source is recreated
        let mut respawn = false;
        let mut fonts: Vec<kt_model::GeneratorKind> = Vec::new();
        for (id, st) in &cur {
            let old = self.gen_states.get(id);
            if old == Some(st) {
                continue;
            }
            fonts.push(st.kind.clone());
            let addr = kt_engine::gen_source_addr(*id);
            // Thumbnails of the event: the thumbnail service has ITS OWN source, and it needs
            // a frame, not an address — so the path is complete, with values.
            let path = kt_model::format_gen_path(&st.kind);
            match old {
                Some(prev) => {
                    if !self.player.update_gen(*id, st.kind.clone(), st.auto.clone()) {
                        respawn = true;
                    }
                    let prev_path = kt_model::format_gen_path(&prev.kind);
                    if kt_media::factory::can_retune(&prev_path, &path) {
                        self.thumbs.retune_media(addr, path);
                    } else {
                        self.thumbs.add_media(addr, path);
                        self.thumbs.invalidate_media(addr);
                    }
                }
                None => self.thumbs.add_media(addr, path),
            }
            // picker cells remember the SAME ThumbKey, and it does not depend on the
            // generator values (a twin defect of item 412)
            self.fx_thumbs.invalidate_media(addr);
        }
        if respawn {
            let p = self.player_project();
            self.player.sync_generators(&p);
        }
        for k in fonts {
            self.note_missing_title_font(&k);
        }
        self.gen_states = cur;
    }

    /// Dragging a title DIRECTLY IN THE PREVIEW (R.52 C6): the frame of the text block from the same
    /// layout that draws the frame; dragging writes pos_x/pos_y with THE SAME
    /// command and the same gesture as the window — both ways behave
    /// identically by construction (rule 17). The grab zone is wider than the frame
    /// (rule 1); click-to-play outside the frame still works.
    fn title_drag_in_preview(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        img: egui::Rect,
    ) {
        let Some((clip, gesture)) = self.edited_title() else { return };
        let Some(mut kind) = self.model.clip(clip).and_then(|c| c.r#gen.clone()) else { return };
        let before = kind.clone();
        let (iw, ih) = (img.width().max(1.0) as u32, img.height().max(1.0) as u32);
        let Some(info) = kt_media::generator::title_layout(&kind, iw, ih) else { return };
        let b = info.block;
        let frame = egui::Rect::from_min_size(
            img.min + egui::vec2(b.x as f32, b.y as f32),
            egui::vec2(b.w.max(8.0) as f32, b.h.max(8.0) as f32),
        )
        .intersect(img);
        painter.rect(
            frame,
            0.0,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(120, 190, 255, 200)),
            egui::StrokeKind::Outside,
        );
        let resp =
            ui.interact(frame.expand(8.0), ui.id().with(("title-drag", clip)), egui::Sense::drag());
        if resp.dragged() {
            let d = resp.drag_delta();
            if d != egui::Vec2::ZERO
                && let kt_model::GeneratorKind::Text { pos_x, pos_y, .. } = &mut kind
            {
                *pos_x = (*pos_x + d.x as f64 / img.width().max(1.0) as f64).clamp(0.0, 1.0);
                *pos_y = (*pos_y + d.y as f64 / img.height().max(1.0) as f64).clamp(0.0, 1.0);
                // THE SAME command and the same gesture as the form in the window
                // (rule 17 by construction); the values live on the EVENT
                if kind != before {
                    self.apply_coalesced(
                        kt_model::Command::SetClipGen { id: clip, kind: Some(kind) },
                        gesture,
                    );
                }
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    /// An event whose TITLE is being edited now (R.59, stage 2): the open effects window
    /// in which the HEAD of the band is selected, and the event’s source is
    /// the text. A title no longer has its own window, so both the drag frame and
    /// the safe zones ask the same place.
    pub(crate) fn edited_title(&self) -> Option<(ClipId, u64)> {
        for ed in self.fx_editors.iter().rev().filter(|e| e.open) {
            for id in &ed.members {
                if ed.selected.get(id) != Some(&crate::widgets::ChainSel::Head) {
                    continue;
                }
                let is_text = self
                    .model
                    .clip(*id)
                    .and_then(|c| c.r#gen.as_ref())
                    .is_some_and(|g| matches!(g, kt_model::GeneratorKind::Text { .. }));
                if is_text {
                    return Some((*id, ed.gesture));
                }
            }
        }
        None
    }

    /// Safe zones are drawn while the title is being edited (R.52 C3).
    pub(crate) fn show_safe_zones(&self) -> bool {
        self.edited_title().is_some()
    }

    pub(crate) fn sync_media_sources(&mut self) {
        // A cheap check BEFORE building the map (R.53, item 413): sync is called from
        // every edit wrapper, that is, on EVERY frame of any
        // coalesced gesture (dragging a transition edge, a level, an eyedropper). While
        // the model revision has not moved, the media set could not have changed —
        // and building a HashMap with a clone of each path per frame is forbidden by
        // invariant 12 (no allocations in the frame paths).
        if self.media_synced && self.media_synced_rev == self.model_rev {
            return;
        }
        self.media_synced_rev = self.model_rev;
        self.media_sync_builds += 1;
        let cur: HashMap<MediaId, String> =
            self.model.document().media.iter().map(|m| (m.id, m.path.clone())).collect();
        if !self.media_synced {
            // first call: the sources have already been raised by build_player
            self.media_paths = cur;
            self.media_synced = true;
            self.sync_gen_sources();
            return;
        }
        if self.media_paths == cur {
            // the media set is the same, but the generator VALUES could have
            // changed: they live on the events (R.59, stage 2)
            self.sync_gen_sources();
            return;
        }
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let db = self.index_db_path();
        for (id, path) in &cur {
            let known = self.media_paths.get(id).cloned();
            if known.as_deref() == Some(path.as_str()) {
                continue;
            }
            let resolved = resolve_media_path(base.as_deref(), path);
            // RETUNING IN PLACE (R.53, item 412): for a generator, a change of
            // parameters is not a file substitution. The decoder thread remains
            // alive, the renderers are intact; the frames of the old parameters are discarded
            // by the worker itself. The source’s passport cannot change in this case —
            // the factory predicate watches over this (t499).
            if let Some(old) = known.as_deref()
                && kt_media::factory::can_retune(old, path)
                && self.player.retune_media(*id, path)
            {
                self.player.set_media_path(*id, path);
                self.thumbs.retune_media(*id, resolved);
                // picker cells remember the SAME ThumbKey, and it does not depend on the
                // generator parameters: without this line the strip would show
                // the effect on a frame of the OLD parameters — a twin defect of
                // item 412, only in the picker
                self.fx_thumbs.invalidate_media(*id);
                continue;
            }
            let label = crate::util::media_label(path);
            let (src, err) =
                crate::util::open_source_or_placeholder(&resolved, db.as_deref(), &label);
            let opened = err.is_none();
            if let Some(e) = err {
                self.note(e);
            }
            // MATERIAL LENGTH — comes from whoever opened the source (R.63 item 2).
            // The model does not obtain it (law P), and without it the right trim
            // has no stop: this is exactly how an event slides past the end of the file and
            // “sounds like silence”. A placeholder does not know its own length — its
            // passport is empty, and no limit will appear.
            if opened {
                let d = src.info().duration;
                if d.0 > 0 {
                    self.model.note_media_length(*id, d);
                }
            }
            if known.is_some() {
                self.player.replace_media(*id, src);
            } else {
                self.player.add_media(*id, src);
            }
            self.thumbs.add_media(*id, resolved);
            self.thumbs.invalidate_media(*id);
        }
        let gone: Vec<MediaId> =
            self.media_paths.keys().filter(|id| !cur.contains_key(id)).copied().collect();
        for id in gone {
            self.player.remove_media(id);
        }
        self.media_paths = cur;
        self.sync_gen_sources();
    }

    /// “Create generator” from the bin menu (R.52): media + source +
    /// card + the parameters window at once. The clip is NOT placed — the card is
    /// dragged to the timeline or “Place at cursor” is pressed, as for a file.
    /// New generator: the media AND the CLIP under the cursor — in one batch, one
    /// history step (R.53, item 416).
    ///
    /// Before this item the menu entry created only the media in the bin: the window
    /// of parameters opened, but there was nothing in the preview — a title had to be typed
    /// blind, then go to the bin and place the card by hand.
    /// Ctrl+Z returns everything at once, because the batch is one.
    pub fn create_generator(&mut self, kind: kt_model::GeneratorKind) {
        // R.59, stage 2: in the media path — the KIND; values travel to the EVENT.
        let path = kt_model::gen_kind_path(&kind);
        let id = MediaId(self.next_media_id());
        // the source is raised by us (the sync snapshot — BEFORE apply); it also gives
        // the passport by which the set of clips is computed
        let src = match crate::util::open_source(&path) {
            Ok(s) => s,
            Err(e) => {
                self.note(e);
                return;
            }
        };
        let info = src.info().clone();
        self.media_paths.insert(id, path.clone());
        let at = self.player.transport.playhead();
        let mut cmds = vec![Command::AddMedia { id, path: path.clone() }];
        cmds.extend(self.clip_cmds_for(id, &info, at, None));
        // the values chosen in the menu — onto the events of this same batch: in one
        // history step, like all the rest of the creation (R.53, item 416)
        for c in cmds.iter_mut() {
            if let Command::AddClip(ev) = c {
                ev.r#gen = Some(kind.clone());
            }
        }
        if !self.apply_batch(cmds) {
            self.media_paths.remove(&id);
            return;
        }
        self.player.add_media(id, src);
        self.thumbs.add_media(id, path);
        self.open_generator_editor(id);
    }

    pub(crate) fn poll_returned_sources(&mut self) {
        if let Some(rx) = self.reopen_rx.take() {
            let mut ready = Vec::new();
            while let Ok(msg) = rx.try_recv() {
                ready.push(msg);
            }
            if ready.is_empty() {
                self.reopen_rx = Some(rx); // still opening — we wait
            }
            for (id, label, src) in ready {
                // did not open again — the placeholder stays, there is nothing to complain about
                if let Some(s) = src {
                    self.player.replace_media(id, s);
                    self.placeholder_media.remove(&id);
                    self.note(crate::tf!(k::msg::MEDIA_ONLINE, label));
                }
            }
        }
        if self.placeholder_media.is_empty() || self.reopen_rx.is_some() {
            return;
        }
        let offline = self.offline.lock().unwrap().clone();
        let base = self.project_path.as_deref().and_then(|p| p.parent());
        let db = self.project_path.as_deref().map(|p| p.with_extension("ktindex.db"));
        let back: Vec<(MediaId, String, String)> = self
            .model
            .project()
            .media
            .iter()
            .filter(|m| self.placeholder_media.contains(&m.id) && !offline.contains(&m.id))
            .map(|m| (m.id, crate::util::media_label(&m.path), resolve_media_path(base, &m.path)))
            .collect();
        if back.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.reopen_rx = Some(rx);
        std::thread::Builder::new()
            .name("reopen-media".into())
            .spawn(move || {
                for (id, label, path) in back {
                    let src = open_source_indexed(&path, db.as_deref()).ok();
                    let _ = tx.send((id, label, src));
                }
            })
            .ok();
    }

    /// Region by a range marker + selection of its events (R.33 item 8): ONE
    /// body for a double click on the reel and any future menu item.
    pub fn region_from_marked_range(&mut self, marker_start: i64) {
        let Some(m) =
            self.model.project().markers.iter().find(|m| m.t.0 == marker_start && m.is_range())
        else {
            return;
        };
        let Some(r) = Region::normalized(m.t, m.end()) else { return };
        let ids: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| r.intersects_clip(c))
            .map(|c| c.id)
            .collect();
        self.model.set_region(Some(r));
        self.on_region_changed();
        self.selected.clear();
        self.selected.extend(ids);
        if !self.selected.is_empty() {
            self.selected_morph.clear();
            self.selected_camera.clear();
        }
    }

    /// A marker at point `at` — ONE body for the M gesture and the “Marker
    /// here” menu item (item 11 from the user: “marker here with a selected region
    /// must produce a region marker… obviously there are paired ways to do something”).
    /// Paired ways must take the context into account identically —
    /// item 17 of the checklist in apps/kt-app/CLAUDE.md.
    pub fn marker_here(&mut self, at: Tick) {
        // R.24 (the user’s gesture, verbatim): “with a selected region and
        // the cursor inside it — m gives marked_range, with the cursor outside the region —
        // a single marker at the cursor”. A repeated M on the same region is a toggle.
        // The region’s edges count as “inside”: a right click on the edge is
        // a click ON the region, not beside it.
        if let Some(r) = self.model.project().region
            && (r.contains(at) || at == r.start || at == r.end)
        {
            let dur = r.end - r.start;
            let at_start = self.model.project().markers.iter().find(|m| m.t == r.start).cloned();
            match at_start {
                Some(m) if m.duration == dur => {
                    self.apply(Command::RemoveMarker { at: r.start });
                }
                Some(_) => {
                    // a point marker at the start of a region — stretch it into a range
                    self.apply(Command::SetMarkerDuration { at: r.start, duration: dur });
                }
                None => {
                    if self.apply(Command::AddMarker { at: r.start, duration: dur }) {
                        self.marker_edit =
                            Some(MarkerEdit { at: r.start.0, buf: String::new(), is_new: true });
                        self.marker_edit_focus = true;
                    }
                }
            }
            return;
        }
        // magnet as in the old one: remove the nearest within the threshold, otherwise add
        let threshold = ((crate::timeline::SNAP_PX * self.ticks_per_px) as i64).max(900);
        let near = self
            .model
            .project()
            .markers
            .iter()
            .map(|m| m.t)
            .min_by_key(|t| (t.0 - at.0).abs())
            .filter(|t| (t.0 - at.0).abs() <= threshold);
        match near {
            Some(t) => {
                self.apply(Command::RemoveMarker { at: t });
            }
            None => {
                // new marker: immediately open the name field (item 1 of round 10);
                // a click elsewhere / Esc leaves the marker without a name
                if self.apply(Command::AddMarker { at, duration: Tick::ZERO }) {
                    self.marker_edit =
                        Some(MarkerEdit { at: at.0, buf: String::new(), is_new: true });
                    self.marker_edit_focus = true;
                }
            }
        };
    }

    /// Region in/out from the cursor; without a region — a seed [cursor, cursor+1 s].
    pub fn set_region_edge_from_cursor(&mut self, set_start: bool) {
        let at = self.player.transport.playhead();
        let cur =
            self.model.project().region.unwrap_or(Region { start: at, end: Tick(at.0 + 90_000) });
        let r = if set_start {
            Region::normalized(at, cur.end.max(Tick(at.0 + 90)))
        } else {
            Region::normalized(cur.start.min(at), at)
        };
        self.model.set_region(r);
        self.on_region_changed();
    }

    /// Is it time to autosave: every 30 s, not during a gesture and not while
    /// playing — but if there is no quiet moment for longer than two intervals, we write
    /// anyway (losing edits is worse than a one-off flicker, item 4 of round 11).
    pub fn autosave_due(&self) -> bool {
        let idle = self.last_autosave.elapsed().as_secs();
        if idle < self.autosave_secs {
            return false;
        }
        let busy =
            self.drag.is_some() || self.bin_drag.is_some() || self.player.transport.is_playing();
        !busy || idle >= self.autosave_secs * 2
    }

    pub fn clear_region(&mut self) {
        self.model.set_region(None);
        // item 16 of round 6: without a region an armed loop — “the lamp is on,
        // it does not loop” (and survived a restart) — we switch it off along with it
        self.model.set_loop_enabled(false);
        self.on_region_changed();
    }

    /// Esc: clear the region, WITHOUT touching the loop (item 3 of round 11). The loop is a
    /// separate intent of the user; only the explicit “Clear region” command used to switch it
    /// off, and Esc switched it off as well.
    pub fn clear_region_keep_loop(&mut self) {
        self.model.set_region(None);
        self.on_region_changed();
    }

    pub fn toggle_loop(&mut self) {
        let on = !self.model.project().loop_enabled;
        self.model.set_loop_enabled(on);
        self.on_region_changed();
    }

    /// Syncing the region/loop with the player.
    ///
    /// `set_project` is NOT called here (R.43, item 296): the region and `loop_enabled`
    /// live in the “convenience” state; the engine does not read them from the project
    /// at all — the loop arrives via a separate `set_loop_region`.
    /// And `set_project` is a clone of the whole project plus a rebuild of both
    /// renderers: on the user’s edit (1961 events) every shift of the region border
    /// and every press of Loop stopped the audio-render thread, the ring emptied,
    /// and callback underruns began — “the region is somehow broken during playback”.
    /// Edits of the CONTENT go through `apply`/`apply_batch`, and they are what bring
    /// the project to the player.
    pub fn on_region_changed(&mut self) {
        let p = self.model.project();
        let region =
            p.region.filter(|_| p.loop_enabled).map(|r| kt_time::TickRange::new(r.start, r.end));
        self.player.set_loop_region(region);
        self.dirty_ui = true;
    }

    pub fn copy_selection(&mut self) {
        let mut block = match self.model.project().region {
            Some(r) => kt_model::ops::copy_region_block(self.model.project(), r),
            // without a region — everything selected (each with its own link group)
            None => kt_model::ops::copy_clips(self.model.project(), &self.selected_with_groups()),
        };
        // THE CLIPBOARD LIVES IN THE COORDINATES OF THE OWNING REEL (R.56, item 450).
        // We copy FROM the WORKING view, and in it the unfolded band sits with an offset
        // `base[kind]`, and the offsets of video, audio and morph rows are DIFFERENT.
        // A repeated paste would put these numbers as they are — video would land at the
        // ceiling, audio at the bottom, and the morpher at the very bottom, where a new row
        // would be started for it. This is exactly what the owner complained about.
        // The block’s reel is decided by the FIRST event, not by “at least one band found”:
        // a region touching both the root and a band, otherwise
        // it would be localised into the reel and throw out the root majority.
        let band = block
            .clips
            .first()
            .and_then(|c| self.model.band_of_row(kt_model::lane_key(c).0, c.lane))
            .map(|b| (b.base, b.len));
        let root = kt_model::LaneBases::default();
        let (base, len) = band.unwrap_or((
            root,
            // root: “own” = everything BELOW the start of any band
            kt_model::LaneBases::of_root(self.model.project()),
        ));
        let dropped = block.localize(base, len);
        if dropped > 0 {
            self.note(crate::tf!(k::msg::COPY_ONE_REEL, dropped));
        }
        if block.clips.is_empty() {
            self.note(tr(k::msg::NOTHING_TO_COPY));
        } else {
            self.note(crate::tf!(k::msg::CLIPS_COPIED, block.clips.len()));
            self.clipboard = block;
        }
    }

    /// The paste prefix window (R.35 item 4). Non-blocking: the paste position is
    /// frozen, Enter pastes, Esc cancels (it is also in the Esc cascade),
    /// on closing focus is RETURNED — otherwise hotkeys would die until
    /// a click elsewhere (rule 14, defect of item 8 of round 12).
    fn draw_paste_names(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.paste_names.take() else { return };
        let mut go = false;
        let mut cancel = false;
        let mut open = true;
        egui::Window::new(tr(k::msg::PASTE_PREFIX_TITLE))
            .id(egui::Id::new("paste_names"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::msg::PASTE_PREFIX_LABEL));
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut d.affix)
                            .desired_width(180.0)
                            .hint_text(tr(k::msg::PASTE_AFFIX_DEFAULT)),
                    );
                    if d.focus {
                        resp.request_focus();
                        d.focus = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                    // the side — a toggle next to the field, not a second window:
                    // it is the same choice, not a separate setting
                    if ui.selectable_label(d.before, tr(k::msg::PASTE_AFFIX_BEFORE)).clicked() {
                        d.before = !d.before;
                    }
                });
                // live preview: the user sees the name BEFORE pasting
                let sample = self
                    .clipboard
                    .areas
                    .first()
                    .map(|a| a.name.clone())
                    .or_else(|| {
                        self.clipboard
                            .markers
                            .iter()
                            .find(|m| !m.name.is_empty())
                            .map(|m| m.name.clone())
                    })
                    .unwrap_or_default();
                if !sample.is_empty() {
                    let a = d.affix.trim();
                    let shown = if a.is_empty() {
                        sample.clone()
                    } else if d.before {
                        format!("{a} {sample}")
                    } else {
                        format!("{sample} {a}")
                    };
                    ui.label(
                        egui::RichText::new(kt_model::unique_name(
                            &self.model.project().taken_names(),
                            &shown,
                            "copy",
                        ))
                        .strong(),
                    );
                }
                ui.label(egui::RichText::new(tr(k::msg::PASTE_PREFIX_NOTE)).weak().small());
                ui.horizontal(|ui| {
                    if ui.button(tr(k::msg::PASTE_DO)).clicked() {
                        go = true;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if go {
            let (at, row, affix, before) = (d.at, d.row, d.affix.trim().to_string(), d.before);
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("paste_names")));
            self.paste_here(at, row, affix, before);
            return;
        }
        if cancel || !open {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("paste_names")));
            return;
        }
        self.paste_names = Some(d);
    }

    /// Preset name window (reference — the paste prefix window, t250):
    /// live preview of the final name, Enter/Esc, focus is returned on
    /// closing. The model is NOT touched — this is a write to the user’s file.
    fn draw_preset_save(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.preset_save.take() else { return };
        let mut go = false;
        let mut cancel = false;
        let mut open = true;
        let taken = self.presets.taken_names();
        egui::Window::new(tr(k::msg::PRESET_SAVE_TITLE))
            .id(egui::Id::new("preset_save"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::msg::PRESET_NAME_LABEL));
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut d.name)
                            .desired_width(200.0)
                            .hint_text(tr(k::msg::PRESET_NAME_HINT)),
                    );
                    if d.focus {
                        resp.request_focus();
                        d.focus = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                });
                // live preview: the user sees the FINAL name before saving
                // (a taken one gets a suffix)
                let wanted = d.name.trim();
                if !wanted.is_empty() {
                    let taken_now: Vec<String> = taken
                        .iter()
                        .filter(|n| Some(n.as_str()) != d.rename_of.as_deref())
                        .cloned()
                        .collect();
                    let shown = kt_model::unique_name(&taken_now, wanted, "copy");
                    ui.label(egui::RichText::new(shown).strong());
                }
                ui.label(egui::RichText::new(tr(k::msg::PRESET_SAVE_NOTE)).weak().small());
                ui.horizontal(|ui| {
                    if ui.button(tr(k::msg::PRESET_SAVE_DO)).clicked() {
                        go = true;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if go && !d.name.trim().is_empty() {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("preset_save")));
            let taken_now: Vec<String> = taken
                .iter()
                .filter(|n| Some(n.as_str()) != d.rename_of.as_deref())
                .cloned()
                .collect();
            let name = kt_model::unique_name(&taken_now, d.name.trim(), "copy");
            let res = match &d.rename_of {
                Some(old) => self.presets.rename(old, &name),
                None => self.presets.add(&name, &d.block),
            };
            match res {
                Ok(()) => self.note(format!("{} {name}", tr(k::msg::PRESET_SAVED))),
                Err(e) => self.note(format!("{}: {e}", tr(k::msg::PRESET_SAVE_FAILED))),
            }
            return;
        }
        if cancel || !open {
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("preset_save")));
            return;
        }
        self.preset_save = Some(d);
    }
    /// Measure the media loudness by direct request (R.49, item 373).
    ///
    /// Exactly ON REQUEST: the display itself starts nothing (invariant 8),
    /// and the index service measures only what it already indexes.
    pub fn measure_loudness(&mut self, id: MediaId) {
        let Some(m) = self.model.project().media.iter().find(|m| m.id == id).cloned() else {
            return;
        };
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let index = self.index_db_path();
        let path = resolve_media_path(base.as_deref(), &m.path);
        let rev = self.model_rev;
        let kind = crate::analysis::AnalysisKind::Loudness(id);
        // “try again” is exactly the direct request: the negative cache
        // is cleared only here
        let retry = self.analysis.failure(kind).is_some();
        self.analysis.request_with(
            kind,
            rev,
            retry,
            move |ctl: crate::bg::BgCtl, send: crate::bg::BgSend<f32>| {
                let mut src = open_source_indexed(&path, index.as_deref())?;
                let total =
                    src.info().duration.to_audio_frames() / kt_time::AUDIO_BURST_FRAMES as i64;
                let mut meter = kt_fx::audio::loudness::LoudnessMeter::new();
                for b in 0..total.max(1) {
                    if ctl.should_stop() {
                        return Err("cancelled".into());
                    }
                    let Ok(burst) = src.read_audio_burst(b) else { break };
                    meter.feed(burst.data.as_slice());
                    if b % 64 == 0 {
                        send.send(b as f32 / total.max(1) as f32);
                    }
                }
                let l = meter.finish();
                Ok(crate::analysis::AnalysisOut::Loudness {
                    media: id,
                    lufs: l.lufs,
                    dbtp: l.dbtp,
                    lra: l.lra,
                })
            },
        );
    }

    /// The analysis task result has arrived.
    ///
    /// Stale protection by revision: while the task was computing, the user could
    /// have swapped the source or rebuilt the edit — a number computed on the old
    /// model must not be applied to the other one.
    fn on_analysis_done(
        &mut self,
        kind: crate::analysis::AnalysisKind,
        rev: u64,
        res: Result<crate::analysis::AnalysisOut, String>,
    ) {
        use crate::analysis::AnalysisOut as O;
        let out = match res {
            Ok(o) => o,
            Err(e) => {
                if e != "cancelled" {
                    self.warn(crate::tf!(k::msg::ANALYSIS_FAILED, e));
                }
                return;
            }
        };
        // stale protection: the revision AND the presence of the media. If the source was swapped —
        // a number computed on the previous file does not apply to it.
        let alive = |m: MediaId| self.model.project().media.iter().any(|x| x.id == m);
        let stale = self.model_rev != rev || !kind.media().into_iter().all(alive);
        match (kind, out) {
            (_, O::Loudness { media, lufs, dbtp, lra }) => {
                // a number for a vanished or swapped media does not belong to
                // this project: applying it would be lying about the material
                if stale {
                    return;
                }
                if let Some(l) = lufs {
                    self.loudness.lock().unwrap().insert(media, (l, dbtp, lra));
                }
            }
        }
    }

    /// The same result trick as in the frame — for the stale-protection test.
    #[cfg(test)]
    pub(crate) fn on_analysis_done_for_test(
        &mut self,
        kind: crate::analysis::AnalysisKind,
        rev: u64,
        res: Result<crate::analysis::AnalysisOut, String>,
    ) {
        self.on_analysis_done(kind, rev, res);
    }

    /// Is there a NAMED item in the buffer — only then do we ask for the prefix
    /// (otherwise the paste goes silently, as before).
    pub(crate) fn clipboard_has_names(&self) -> bool {
        !self.clipboard.areas.is_empty()
            || self.clipboard.markers.iter().any(|m| !m.name.is_empty())
    }

    /// Ctrl+V: the time is the CURSOR, the row is under the mouse, if the mouse is over
    /// the timeline (R.56, item 450).
    pub fn paste_at_cursor(&mut self) {
        let (at, row) = (self.player.transport.playhead(), self.tl_hover_row);
        self.paste_at(at, row)
    }

    /// The “Paste” item of the right-hand menu: TIME AND ROW OF THE CLICK (R.56, item 450,
    /// the owner’s decision). By clicking on a specific place, the person names the
    /// place — they need not search for the cursor.
    pub fn paste_at_click(&mut self) {
        let at = self.ctx_at.unwrap_or_else(|| self.player.transport.playhead());
        let row = self.ctx_row;
        self.paste_at(at, row)
    }

    /// Paste into a NAMED place: the time and row of the click (the “Paste”
    /// item of the right menu) or the cursor and the row under the mouse (Ctrl+V).
    pub fn paste_at(&mut self, at: Tick, row: Option<(LaneKind, u32)>) {
        if self.clipboard.clips.is_empty() {
            self.note(tr(k::msg::CLIPBOARD_EMPTY));
            return;
        }
        // a named item in the buffer — we ask for the prefix with a window (R.35 item 4);
        // the paste PLACE is FROZEN, so that the cursor and the mouse under the window
        // do not move it away.
        if self.clipboard_has_names() && self.paste_names.is_none() {
            self.paste_names = Some(crate::app::PasteNamesDialog {
                at,
                row,
                affix: tr(k::msg::PASTE_AFFIX_DEFAULT).to_string(),
                before: false,
                focus: true,
            });
            return;
        }
        let (at, row) = match self.paste_names.take() {
            Some(d) => (d.at, d.row),
            None => (at, row),
        };
        self.paste_here(at, row, String::new(), false)
    }

    /// The paste itself: ONE `paste_block` → ONE `apply_batch`.
    ///
    /// `row` — the row of the WORKING VIEW that was clicked. The block is stored in its own
    /// reel’s coordinates (see `copy_selection`), so here it is first brought to the row,
    /// and then, if the target row belongs to an unfolded band, it is translated
    /// into that band’s coordinates.
    pub(crate) fn paste_here(
        &mut self,
        at: Tick,
        row: Option<(LaneKind, u32)>,
        affix: String,
        before: bool,
    ) {
        let target = row.and_then(|(k, l)| self.model.band_of_row(k, l)).map(|b| (b.base, b.len));
        let local_row = row.map(|(k, l)| (k, l - target.map_or(0, |(b, _)| b.get(k))));
        let (block, clamped) = self.clipboard.clone().shifted_to_row(local_row);
        if !clamped.is_empty() {
            self.note(tr(k::msg::PASTE_LANES_CLAMPED));
        }
        let mut block = block;
        // A PASTE INTO THE ROOT DOES NOT CLIMB INTO AN UNFOLDED BAND (R.62). Before change B
        // the band lay at the TAIL of the numeric space, and low numbers
        // always belonged to the root; now the band’s rows stand UNDER their own
        // instance, that is in the middle, and half of a pair (audio to the
        // clicked video) could silently move inside the reel. The row is
        // clicked for ONE kind — the other kinds step over the band.
        if target.is_none() && !self.model.bands().is_empty() {
            let mut push = kt_model::LaneBases::default();
            for k in [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera] {
                let Some((first, _)) = block.lane_span(k) else { continue };
                if let Some(b) = self.model.band_of_row(k, first) {
                    *push.get_mut(k) = b.base.get(k) + b.len.get(k) - first;
                }
            }
            block.offset_lanes(push);
        }
        // Tracks for the block are created EXPLICITLY. A silent growth in `AddClip`
        // with an unfolded band would put the event into the wrong reel: the number
        // `base` belongs to the band, and `base + len` — to nobody any more.
        let need = block.lanes_needed();
        let mut grow = Vec::new();
        let have = match target {
            Some((_, len)) => len,
            None => kt_model::LaneBases::of_root(self.model.project()),
        };
        for k in [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera] {
            let tail = match target {
                Some((base, len)) => base.get(k) + len.get(k),
                None => have.get(k),
            };
            for i in have.get(k)..need.get(k) {
                let at = tail + (i - have.get(k));
                grow.push(Command::InsertTrack { kind: k, at });
            }
        }
        if let Some((base, _)) = target {
            block.offset_lanes(base);
        }
        let mut next_clip = self.next_clip_id();
        let mut next_group = self.next_group_id();
        let mut next_transition = self.next_transition_id();
        let mut next_morph = self.next_morph_id();
        let mut next_camera = self.next_camera_id();
        let names =
            kt_model::ops::PasteNames { affix, before, taken: self.model.project().taken_names() };
        let (cmds, end, skipped) = kt_model::ops::paste_block(
            &block,
            at,
            kt_model::ops::PasteIds {
                clip: &mut next_clip,
                group: &mut next_group,
                transition: &mut next_transition,
                morph: &mut next_morph,
                camera: &mut next_camera,
            },
            &names,
        );
        if skipped.morphers > 0 {
            self.note(crate::tf!(k::msg::PASTE_SKIPPED_MORPHERS, skipped.morphers));
        }
        if skipped.markers > 0 {
            self.note(crate::tf!(k::msg::PASTE_SKIPPED_MARKERS, skipped.markers));
        }
        let first = cmds.iter().find_map(|c| match c {
            Command::AddClip(ev) => Some(ev.id),
            _ => None,
        });
        // tracks are created BEFORE the content and in the same batch — one history
        // step for the whole paste
        let cmds = grow.into_iter().chain(cmds).collect::<Vec<_>>();
        if self.apply_batch(cmds) {
            match first {
                Some(id) => self.select_only(id),
                None => self.selected.clear(),
            }
            // the cursor to the end of the block — chain-paste (semantics of the old one)
            self.player.seek(end);
        }
    }

    /// S: ALWAYS a cut at the cursor (item 5): the selection + group; without a
    /// selection — all events under the cursor. Cutting out a region — only from the
    /// context menu (punch_out_action).
    pub fn cut_action(&mut self) {
        if !self.selected.is_empty() {
            self.split_selected();
        } else {
            self.split_all_at_cursor();
        }
    }

    /// Cut ALL clips crossing the cursor (S without a selection).
    pub fn split_all_at_cursor(&mut self) {
        let at = self.player.transport.playhead();
        let targets: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| {
                c.start + MIN_CLIP_DURATION <= at && at + MIN_CLIP_DURATION <= c.start + c.duration
            })
            .map(|c| c.id)
            .collect();
        if targets.is_empty() {
            self.note(tr(k::msg::NO_EVENTS_HERE));
            return;
        }
        let mut next_id = self.next_clip_id();
        let mut next_group = self.next_group_id();
        // one new group per old one (the right halves of pairs remain pairs)
        let mut group_map: std::collections::HashMap<GroupId, GroupId> =
            std::collections::HashMap::new();
        let mut cmds = Vec::new();
        #[allow(clippy::explicit_counter_loop)]
        for id in targets {
            let old_group = self.model.clip(id).and_then(|c| c.link_group);
            let new_group = old_group.map(|g| {
                *group_map.entry(g).or_insert_with(|| {
                    let ng = GroupId(next_group);
                    next_group += 1;
                    ng
                })
            });
            cmds.push(Command::Split { id, at, new_id: ClipId(next_id), new_group });
            next_id += 1;
        }
        self.apply_batch(cmds);
    }

    /// Cutting the interior of a region (context menu).
    pub fn punch_out_action(&mut self) {
        if let Some(r) = self.model.project().region {
            let mut next = self.next_clip_id();
            let cmds = kt_model::ops::punch_out_region(self.model.project(), r, &mut next);
            if cmds.is_empty() {
                self.note(tr(k::msg::REGION_MISSES));
            } else {
                self.apply_batch_restoring(cmds);
                self.selected.clear();
            }
        }
    }

    /// Del: with a region = delete the crossing ones (+ partners), without — the selection.
    pub fn delete_action(&mut self) {
        // a selected morph block — priority (fix D6 of v1: with a drawn region
        // the morpher could not be deleted from the keyboard)
        if !self.selected_morph.is_empty() || !self.selected_camera.is_empty() {
            // in bulk: the frame now catches service blocks, and deletion must
            // remove EXACTLY what is highlighted (R.40)
            let cmds: Vec<Command> = self
                .selected_morph
                .iter()
                .map(|id| Command::RemoveMorpher { id: *id })
                .chain(self.selected_camera.iter().map(|id| Command::RemoveCamera { id: *id }))
                .collect();
            if self.apply_batch(cmds) {
                self.selected_morph.clear();
                self.selected_camera.clear();
            }
            return;
        }
        if let Some(r) = self.model.project().region {
            let cmds = kt_model::ops::delete_region(self.model.project(), r);
            if cmds.is_empty() {
                self.note(tr(k::msg::REGION_MISSES));
            } else {
                self.apply_batch_restoring(cmds);
                self.selected.clear();
            }
        } else {
            self.delete_selected();
        }
    }

    pub fn trim_selected_start_to_cursor(&mut self) {
        let Some(sel) = self.require_one_selected() else { return };
        let at = self.player.transport.playhead();
        let cmds = kt_model::ops::trim_group_start_to(self.model.project(), sel, at);
        if cmds.is_empty() {
            self.note(tr(k::msg::CURSOR_OUTSIDE));
        } else {
            self.apply_batch(cmds);
        }
    }

    pub fn trim_selected_end_to_cursor(&mut self) {
        let Some(sel) = self.require_one_selected() else { return };
        let at = self.player.transport.playhead();
        let cmds = kt_model::ops::trim_group_end_to(self.model.project(), sel, at);
        if cmds.is_empty() {
            self.note(tr(k::msg::CURSOR_OUTSIDE));
        } else {
            self.apply_batch(cmds);
        }
    }

    /// Move the selection to the adjacent track of its own kind (the partner stays in place).
    pub fn move_selected_lane(&mut self, delta: i32) {
        let Some(sel) = self.require_one_selected() else { return };
        let Some(c) = self.model.clip(sel).cloned() else { return };
        let lanes = match c.kind {
            ClipKind::Video => self.model.project().lanes(kt_model::LaneKind::Video),
            ClipKind::Audio => self.model.project().lanes(kt_model::LaneKind::Audio),
        };
        let new_lane = (c.lane as i64 + delta as i64).clamp(0, lanes.saturating_sub(1) as i64);
        if new_lane as u32 != c.lane {
            // MoveClip moves the whole group in time; here the time does not change,
            // and the lane changes only for the selected one — semantics of the old one
            self.apply(Command::MoveClip {
                id: sel,
                new_start: c.start,
                new_lane: new_lane as u32,
            });
        }
    }

    /// “Add track” from the menu: to the end of its own kind. Goes through the same
    /// point as the paste from the context menu — otherwise the names and the
    /// display order diverge (item 5 of round 16).
    pub fn add_track(&mut self, kind: ClipKind) {
        let lk = match kind {
            ClipKind::Video => LaneKind::Video,
            ClipKind::Audio => LaneKind::Audio,
        };
        self.insert_lane_after_row(lk, None);
    }

    /// A/V magnet: the nearest clip of the opposite type of the same media
    /// within 2×threshold → common start/duration/link (the old magnetize).
    pub fn magnetize_selected(&mut self) {
        let Some(sel) = self.require_one_selected() else { return };
        let mut next_group = self.next_group_id();
        match kt_model::ops::magnetize_commands(
            self.model.project(),
            sel,
            Tick(90_000 * 5), // partner within 5 s
            &mut next_group,
        ) {
            None => self.note(tr(k::msg::NO_OTHER_KIND)),
            Some(cmds) if cmds.is_empty() => self.note(tr(k::msg::ALREADY_LINKED)),
            Some(cmds) => {
                if self.apply_batch(cmds) {
                    self.note(tr(k::msg::AV_LINKED));
                }
            }
        }
    }

    pub fn unlink_selected(&mut self) {
        let Some(sel) = self.require_one_selected() else { return };
        let group = self.model.clip(sel).and_then(|c| c.link_group);
        let ids: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| c.id == sel || (group.is_some() && c.link_group == group))
            .map(|c| c.id)
            .collect();
        self.apply_batch(ids.into_iter().map(|id| Command::UnlinkClip { id }).collect());
    }

    pub fn undo(&mut self) {
        self.model_rev += 1;
        if self.model.undo().is_ok() {
            self.push_project_to_player();
            self.sync_media_sources();
            self.dirty_content = true;
            self.bump_clip_hw();
        }
    }

    pub fn redo(&mut self) {
        self.model_rev += 1;
        if self.model.redo().is_ok() {
            self.push_project_to_player();
            self.sync_media_sources();
            self.dirty_content = true;
            self.bump_clip_hw();
        }
    }

    /// File → New menu (item 3 of round 7): a fresh unnamed project in place.
    /// The old state (player/streams/watchdog) is dropped correctly AFTER the
    /// construction of the new one.
    pub fn new_project(&mut self) {
        if self.guard_unsaved(Pending::New) {
            *self = Self::new_empty(None);
        }
    }

    /// File → Open…: a dialog + recreation of the application in place.
    pub fn open_project_dialog(&mut self) {
        if self.guard_unsaved(Pending::Open) {
            self.open_project_dialog_now();
        }
    }

    /// Open a project at the request of the LIVE RIG (R.68).
    ///
    /// The same code as `File → Open`, but without a dialog and without the question about
    /// unsaved work: the rig opens projects one after another, and the modal window
    /// of the previous scenario would break the next one. The rig does not leave its own
    /// recovery behind it — otherwise the recovery file of scenario A would surface as a
    /// question for scenario B, which starts in its own process.
    fn bench_open(&mut self, ctx: &egui::Context, path: &Path) {
        // The egui MEMORY LIVES OUTSIDE KtApp. `*self = Self::open(..)` rebuilds the
        // program’s state from scratch, but the state of the WIDGETS (scroll positions,
        // collapses, window positions, focus and an unfinished drag) lies in
        // `egui::Context`, which belongs to eframe. Without a reset the scenario
        // inherited the previous one’s leftovers: on the third scenario in a row
        // a drag of an event simply did not start.
        // This is the “reset workplace” from the rig’s contract.
        ctx.memory_mut(|m| {
            m.data.clear();
            m.reset_areas();
        });
        ctx.stop_dragging();
        match Self::open(path) {
            Ok(mut app) => {
                self.recovery_on_drop = false;
                self.drop_own_recovery();
                app.claim_project(path);
                *self = app;
            }
            Err(e) => self.note(crate::tf!(k::msg::OPEN_FAILED, e)),
        }
    }

    fn open_project_dialog_now(&mut self) {
        let Some(p) = pick_file(
            tr(k::dialog::OPEN_PROJECT),
            "Kinetren project",
            &[kt_model::project_file::EXTENSION],
        ) else {
            return;
        };
        match Self::open(&p) {
            Ok(mut app) => {
                app.claim_project(&p);
                *self = app;
            }
            Err(e) => self.note(crate::tf!(k::msg::OPEN_FAILED, e)),
        }
    }

    /// Menu File → Save As…: the dialog is forced (save() without
    /// project_path opens it itself); cancel returns the old path.
    pub fn save_as(&mut self) {
        let old = self.project_path.take();
        self.save();
        if self.project_path.is_none() {
            self.project_path = old; // the user changed their mind
        }
    }

    pub fn save(&mut self) {
        let path = match self.project_path.clone() {
            Some(p) => p,
            // an unnamed project: Save-As dialog (item 12 — refusal here was
            // “this must not be a problem”)
            None => {
                let Some(p) = pick_save(
                    tr(k::dialog::SAVE_PROJECT),
                    "Kinetren project",
                    &[kt_model::project_file::EXTENSION],
                    self.save_hint.as_deref(),
                    UNTITLED_PROJECT,
                ) else {
                    return; // the user changed their mind
                };
                let p = if p.extension().is_none() {
                    p.with_extension(kt_model::project_file::EXTENSION)
                } else {
                    p
                };
                self.project_path = Some(p.clone());
                self.claim_project(&p);
                // the index DB and thumbnails can now live next to the project
                self.index.enqueue(Some(p.with_extension("ktindex.db")), {
                    let base = p.parent();
                    self.model
                        .project()
                        .media
                        .iter()
                        .map(|m| (m.id, resolve_media_path(base, &m.path)))
                        .collect()
                });
                p
            }
        };
        // The project is occupied by another copy — we ask ONCE per session (item 352).
        // We do not forbid it: a second copy is legitimate, and its work lies in its own
        // recovery file. But the program has no right to silently overwrite someone else’s Ctrl+S.
        if let Some(pid) = self.lock_holder
            && !self.lock_override
        {
            self.ask = Some(Ask::SaveOverLock { pid });
            return;
        }
        let ui = Some(self.ui_state());
        self.model.set_ui_state(ui);
        let out = self.project_text(&self.model.document(), path.parent());
        // under the watchdog: the background autosave must not overwrite our record
        // with its older snapshot (R.32)
        match write_project_guarded(&self.saved_rev, &path, self.model_rev, &out) {
            Ok(SaveOutcome::Wrote { dir_synced }) => {
                self.dirty_content = false;
                self.dirty_ui = false;
                self.note(crate::tf!(k::msg::SAVED, path.display()));
                self.warn_if_dir_unsynced(dir_synced);
                self.save_workspace();
                // the work went into the project — OUR OWN recovery is no longer
                // needed. We remove exactly our own (R.49, item 351): someone else’s nearby —
                // is the unsaved work of another copy of the program.
                self.drop_own_recovery();
            }
            // NOT written: the asterisk stays, the recovery file is intact —
            // otherwise the person would consider the work saved (item 350)
            Ok(SaveOutcome::SkippedNewerOnDisk) => {
                self.note(tr(k::msg::SAVE_SKIPPED_NEWER));
            }
            Err(e) => self.warn(crate::tf!(k::msg::SAVE_FAILED, e)),
        }
    }

    pub fn next_morph_id(&self) -> u64 {
        self.model.project().max_morph_id() + 1
    }

    pub fn next_camera_id(&self) -> u64 {
        self.model.project().max_camera_id() + 1
    }

    /// The camera under the playback cursor (R.29): default “50 mm”,
    /// 5 seconds. Parameters and curves — in the camera window (double click on the
    /// block; before R.34 this comment lied — there was no window at all).
    pub fn add_camera_at_playhead(&mut self, lane: u32) {
        let id = kt_model::CameraId(
            self.model.project().cameras.iter().map(|c| c.id.0).max().unwrap_or(0) + 1,
        );
        let start = self.player.transport.playhead();
        let cam = kt_model::CameraEvent {
            id,
            lane,
            start,
            duration: Tick(5 * 90_000),
            name: String::new(),
            cam: kt_model::CameraParams::default(),
            keyframes: vec![],
            auto: vec![],
            ease: true,
        };
        if self.apply(Command::AddCamera(cam)) {
            self.dirty_content = true;
        }
    }

    /// The FIRST FREE service row of kind `kind` over the interval
    /// (R.40 item 4). Order — display, top to bottom: a layer acts “on everything
    /// below”, and the topmost free row covers the most.
    /// `None` — no free row; the caller creates a new one.
    ///
    /// One function for the morpher and the camera: “put it on top of what is already there”
    /// is one defect, not two (rule “a remark = a CLASS of problems”).
    pub fn free_service_lane(&self, kind: LaneKind, start: Tick, duration: Tick) -> Option<u32> {
        let (s0, e0) = (start.0, (start + duration).0);
        let p = self.model.project();
        p.rows().into_iter().filter(|(k, _)| *k == kind).map(|(_, l)| l).find(|lane| match kind {
            LaneKind::Morph => !p
                .morphers
                .iter()
                .any(|m| m.lane == *lane && m.start.0 < e0 && (m.start + m.duration).0 > s0),
            LaneKind::Camera => !p
                .cameras
                .iter()
                .any(|c| c.lane == *lane && c.start.0 < e0 && (c.start + c.duration).0 > s0),
            _ => false,
        })
    }

    /// “+C”: camera into the project (R.40 item 5, verbatim: “I do not understand how
    /// to add something to the camera track — what is it for?????”).
    ///
    /// Before this round the only entry was RMB on the HEADER of the camera row — a strip
    /// 34 px wide — and the row itself could be absent altogether: there was nothing to create it
    /// with, except the same menu of another row.
    /// Symmetric with “+M” (rule 17: paired ways of one action).
    pub fn add_camera_layer(&mut self) {
        let start = Tick(self.player.transport.playhead().0.max(0));
        let duration = Tick(5 * 90_000);
        if let Some(lane) = self.free_service_lane(LaneKind::Camera, start, duration) {
            self.add_camera_at_playhead(lane);
            return;
        }
        // there is no free row — we create a new one. The place in the display order
        // is chosen by `InsertCameraLane` itself (`lane_order_insert`): next to
        // the other camera rows. We do not compute our own layout here —
        // it would diverge from the row’s menu, which calls the same command.
        let lane = self.model.project().lanes(kt_model::LaneKind::Camera);
        if self.apply(Command::InsertTrack { kind: LaneKind::Camera, at: lane }) {
            self.add_camera_at_playhead(lane);
        }
    }

    /// Select EXACTLY this morpher (and deselect everything else). A separate
    /// function, because “select” is three fields, and a forgotten fourth place
    /// has already cost a round.
    pub fn select_morph_only(&mut self, id: MorphId) {
        self.selected.clear();
        self.selected_camera.clear();
        self.selected_morph.clear();
        self.selected_morph.insert(id);
    }

    /// The same for a camera.
    pub fn select_camera_only(&mut self, id: kt_model::CameraId) {
        self.selected.clear();
        self.selected_morph.clear();
        self.selected_camera.clear();
        self.selected_camera.insert(id);
    }

    /// The default of the axis lock: ENGAGED (rule 4 of the checklist — a useful
    /// mode by default; the user should not have to configure the program for normal
    /// behaviour). One place per product, and a test.
    pub fn default_axis_linked() -> bool {
        true
    }

    /// Apply the automation graph’s requests about the AXIS (R.40 item 15).
    ///
    /// The lock is one for the whole program, and there are several graph windows — so
    /// ONE place must apply it, not each window in its own way.
    /// `set_view` arrives while the lock is ON: the wheel over a graph zooms the
    /// main timeline, and the axes stay in sync.
    pub fn apply_axis_req(&mut self, req: crate::automation::AxisReq) {
        if let Some(v) = req.set_linked {
            self.auto_axis_linked = v;
            self.dirty_ui = true;
        }
        if let Some((a, b)) = req.set_view {
            let w = (b.0 - a.0).max(1) as f64;
            // the width of the track band — from the frame geometry; we do not create our own
            let px = self.tl_geom.as_ref().map(|g| g.rect.width() as f64).unwrap_or(800.0).max(1.0);
            self.ticks_per_px = (w / px).clamp(crate::timeline::TPP_MIN, crate::timeline::TPP_MAX);
            self.view_start = (a.0 as f64).max(self.view_floor(self.ticks_per_px));
            self.dirty_ui = true;
        }
    }

    /// Collecting the morpher targets (port of collectMorpherTargetClipIds v1): the region
    /// takes priority over the selection (clips of all tracks crossing the region),
    /// otherwise the selected clip; in both cases the targets are widened by link groups.
    pub fn morph_target_ids(&self) -> Vec<ClipId> {
        let p = self.model.project();
        let mut ids: Vec<ClipId> = match p.region {
            Some(r) => p.clips.iter().filter(|c| r.intersects_clip(c)).map(|c| c.id).collect(),
            None => self.selected.iter().copied().collect(),
        };
        let groups: Vec<GroupId> =
            p.clips.iter().filter(|c| ids.contains(&c.id)).filter_map(|c| c.link_group).collect();
        for c in &p.clips {
            if let Some(g) = c.link_group
                && groups.contains(&g)
                && !ids.contains(&c.id)
            {
                ids.push(c.id);
            }
        }
        ids.sort_by_key(|i| i.0);
        ids.dedup();
        ids
    }

    /// Add a morpher LAYER (R.35 items 3 and 5). Four menu items (“Add
    /// morpher”, “Add FX batch”, the same by region) collapsed into one, and
    /// it asks neither for the kind (there are no kinds any more) nor for targets:
    /// the layer acts on everything BELOW.
    ///
    /// Two things are done here in ONE history step, because separately
    /// they make no sense: a morph track is created AND raised to row 0.
    /// The default `normalize_lane_order` puts morph rows UNDER video and
    /// audio, that is, “everything below” would be empty — while the user
    /// asked for exactly the opposite: “we create morpher tracks
    /// by default above all of them.”
    /// Where the new morph block will land: IF THERE IS A REGION — by the region; if not — from
    /// the cursor, 1.5 s (decision of item 3 of R.38, verbatim: “if there is a region — by the
    /// region; if not — from the cursor”). One rule for all creation methods,
    /// otherwise the menu and the button would diverge, like the M gesture with the “Marker here” item
    /// (rule 17 of the checklist).
    pub fn morph_span(&self) -> (Tick, Tick) {
        match self.model.project().region {
            Some(r) if (r.end - r.start).0 >= kt_model::MIN_CLIP_DURATION.0 => {
                (r.start, r.end - r.start)
            }
            _ => (Tick(self.player.transport.playhead().0.max(0)), Tick(90 * 1500)),
        }
    }

    /// A morpher on an ALREADY EXISTING morph track — symmetric to
    /// `add_camera_at_playhead` (the claim of item 2 of R.38: “I cannot add a morpher
    /// event onto a morpher track in any way”).
    pub fn add_morpher_at(&mut self, lane: u32) {
        let id = MorphId(self.next_morph_id());
        let (start, duration) = self.morph_span();
        let ev = MorpherEvent {
            id,
            lane,
            start,
            duration,
            targets: kt_model::MorphTargets::Below,
            patch: kt_model::MorphPatch::default(),
            timed: true,
            strength: 1.0,
            fade_in: Tick::ZERO,
            fade_out: Tick::ZERO,
            enabled: true,
        };
        if self.apply(Command::AddMorpher(ev)) {
            self.select_morph_only(id);
            self.selected.clear();
            self.open_morph_editor(id);
        }
    }

    /// “+M”: morph layer into the project.
    ///
    /// The track is REUSED (R.38 item 4, verbatim: “if I press the same
    /// add morpher layer in another area — it creates one more
    /// track”), but only a track FREE IN THIS INTERVAL (R.40, verbatim:
    /// “you can add a morpher layer a hundred times, and it will each time create it
    /// on the same track — so there may end up being a heap of
    /// morphers hidden behind one and the same place”). R.38 always reused
    /// the FIRST row — and put the block on top of what already lay there, where
    /// there was nothing to grab it with.
    ///
    /// The search order is top to bottom over the display rows: a layer acts
    /// “on everything below”, and the topmost free row covers the most.
    /// If none is free, a new one is created at the top.
    pub fn add_morph_layer(&mut self) {
        let (start, duration) = self.morph_span();
        let reuse = self.free_service_lane(LaneKind::Morph, start, duration);
        if let Some(lane) = reuse {
            self.add_morpher_at(lane);
            return;
        }
        let id = MorphId(self.next_morph_id());
        let mut cmds = vec![Command::InsertTrack { kind: LaneKind::Morph, at: 0 }];
        // order of rows: the new morph row goes to the top, the others stay as they were
        let mut order: Vec<(LaneKind, u32)> = self
            .model
            .project()
            .rows()
            .into_iter()
            .map(|(k, l)| if k == LaneKind::Morph { (k, l + 1) } else { (k, l) })
            .collect();
        order.insert(0, (LaneKind::Morph, 0));
        cmds.push(Command::SetLaneOrder { order });
        cmds.push(Command::AddMorpher(MorpherEvent {
            id,
            lane: 0,
            start,
            duration,
            targets: kt_model::MorphTargets::Below,
            patch: kt_model::MorphPatch::default(),
            timed: true,
            strength: 1.0,
            fade_in: Tick::ZERO,
            fade_out: Tick::ZERO,
            enabled: true,
        }));
        if self.apply_batch(cmds) {
            self.select_morph_only(id);
            self.selected.clear();
            // v1: the editor window opens immediately after placement
            self.open_morph_editor(id);
        }
    }

    /// Save the area (R.35 item 2). ONE function for all three ways — releasing the frame,
    /// the menu item, the toolbar button (rule 17): otherwise
    /// they would diverge on what exactly falls into the area.
    ///
    /// The geometry is taken from the last selection frame, and if there was none
    /// — from the region on all tracks. The name is generated unique RIGHT HERE,
    /// so that the user sees a ready name, not a refusal.
    pub fn save_marked_area(&mut self) {
        let p = self.model.project();
        let (start, end, lanes) = match self.last_band.clone() {
            Some((a, b, lanes)) if b.0 > a.0 => (a, b, lanes),
            _ => match p.region {
                Some(r) => (r.start, r.end, Vec::new()),
                None => {
                    self.note(tr(k::msg::AREA_NEEDS_BAND));
                    return;
                }
            },
        };
        if (end - start).0 < kt_model::MIN_CLIP_DURATION.0 {
            self.note(tr(k::msg::AREA_NEEDS_BAND));
            return;
        }
        let name = kt_model::unique_name(&p.taken_names(), "", tr(k::msg::AREA_STEM));
        let area = kt_model::MarkedArea {
            name: name.clone(),
            start,
            duration: end - start,
            lanes,
            color: None,
        };
        if self.apply(Command::AddArea(area)) {
            self.note(crate::tf!(k::msg::AREA_SAVED, name));
        }
    }

    /// A free event number. Numbers are NOT REUSED within the
    /// session (R.49, item 356): the earlier formula “maximum over current clips + 1”
    /// after deleting the last event gave its very number again, and
    /// the newborn clip inherited a FOREIGN envelope from `Retained.clips` —
    /// the name, markers and material links of the deleted one. The same number
    /// is used by link groups and morpher links.
    ///
    /// The file format does NOT TOUCH this: the class lives inside the session — on reopening
    /// the file, the envelopes are rebuilt from it. The variant “clean `Retained`
    /// on deletion” was rejected: deletion is rolled back via undo, and
    /// a cleaned envelope is not.
    pub fn next_clip_id(&self) -> u64 {
        let live = self.model.project().max_clip_id();
        let hw = self.clip_id_hw.get().max(live);
        self.clip_id_hw.set(hw);
        hw + 1
    }

    /// A derived event inherits its parent’s envelope (R.49, item 357).
    ///
    /// Half of a cut, the remainder of a crop, an inserted copy, a piece of bake — all of it
    /// is “the same event, only shorter”. Before R.49 they lost the NAME,
    /// foreign effects (including unrecognised time warps), markers and foreign
    /// metadata: the file received a nameless clip without anything that a foreign
    /// editor had put there.
    ///
    /// Markers are copied WHOLE, and the WRITER distributes them between the halves by
    /// intersection with the material window. To distribute them here would mean losing
    /// them on undo: an undone cut would return the parent without markers that had
    /// moved to the children.
    fn inherit_extras(&mut self, births: &kt_model::Births) {
        for (child, parent) in &births.pairs {
            if self.retained.clips.contains_key(child) {
                continue; // a child already has its own envelope — we do not touch it
            }
            if let Some(ex) = self.retained.clips.get(parent).cloned() {
                self.retained.clips.insert(*child, ex);
            }
        }
    }

    /// Remember the numbers that have already been born. Called AFTER each edit
    /// of the model: only there are the new clips already in the project, and
    /// they could not yet have been deleted.
    fn bump_clip_hw(&mut self) {
        let live = self.model.project().clips.iter().map(|c| c.id.0).max().unwrap_or(0);
        self.clip_id_hw.set(self.clip_id_hw.get().max(live));
    }

    /// Commit the fade handle gesture (R.28, item 12). A length below the minimum
    /// of the window — there is no fade: the corner is pushed back, and the transition is removed. Otherwise
    /// we create/edit a fade lying ENTIRELY INSIDE the event (external offset
    /// zero) — exactly as in Vegas, with no second event next to it.
    pub(crate) fn commit_fade_handle(&mut self, id: ClipId, left: bool, len: i64) {
        let existing = self
            .model
            .project()
            .transitions
            .iter()
            .find(|t| {
                t.is_fade()
                    && match (left, t.a, t.b) {
                        (true, None, Some(b)) => b == id,
                        (false, Some(a), None) => a == id,
                        _ => false,
                    }
            })
            .cloned();
        if len < kt_model::MIN_TRANSITION_DURATION.0 {
            if let Some(t) = existing {
                self.apply(Command::RemoveTransition { id: t.id });
            }
            return;
        }
        let (a, b) = if left { (None, Some(id)) } else { (Some(id), None) };
        let (in_offset, out_offset) =
            if left { (Tick::ZERO, Tick(len)) } else { (Tick(len), Tick::ZERO) };
        let had = existing.is_some();
        let ev = match existing {
            // a repeated drag of the corner edits ONLY the window length: the selected
            // kind, the curves and the audio curve are not switched off by themselves (rule 20,
            // item 397 — before, a fresh Crossfade was assembled here and
            // silently erased the fade settings)
            Some(mut t) => {
                t.in_offset = in_offset;
                t.out_offset = out_offset;
                t
            }
            None => TransitionEvent {
                id: TransitionId(self.next_transition_id()),
                a,
                b,
                in_offset,
                out_offset,
                kind: TransitionKind::Crossfade,
                audio: Default::default(),
                auto: vec![],
                invert: false,
            },
        };
        self.apply(if had { Command::SetTransition(ev) } else { Command::AddTransition(ev) });
    }

    pub fn next_transition_id(&self) -> u64 {
        self.model.project().max_transition_id() + 1
    }

    pub fn next_group_id(&self) -> u64 {
        self.model.project().max_group_id() + 1
    }

    /// The fresh media address — across the WHOLE space (R.50): the reels live in
    /// one address space with the files.
    pub fn next_media_id(&self) -> u64 {
        let p = self.model.project();
        p.media.iter().map(|m| m.id.0).chain(p.sequences.iter().map(|s| s.id.0)).max().unwrap_or(0)
            + 1
    }

    pub fn split_selected(&mut self) {
        if self.selected.is_empty() {
            self.note(tr(k::msg::NOTHING_TO_CUT));
            return;
        }
        let at = self.player.transport.playhead();
        // all selected ones (and their link partners) are cut, which the cursor really
        // crosses; we do not impose a cut on those it does not cross
        let targets: Vec<ClipId> = {
            let ids = self.selected_with_groups();
            self.model
                .project()
                .clips
                .iter()
                .filter(|c| ids.contains(&c.id) && c.start < at && at < c.start + c.duration)
                .map(|c| c.id)
                .collect()
        };
        if targets.is_empty() {
            self.note(tr(k::msg::CURSOR_OUTSIDE));
            return;
        }
        let mut next_id = self.next_clip_id();
        let mut next_group = self.next_group_id();
        // one new group per old one — the right halves of pairs remain
        // pairs (the same scheme as the cut without a selection)
        let mut group_map: std::collections::HashMap<GroupId, GroupId> =
            std::collections::HashMap::new();
        let mut cmds = Vec::new();
        #[allow(clippy::explicit_counter_loop)]
        for id in targets {
            let new_group = self.model.clip(id).and_then(|c| c.link_group).map(|g| {
                *group_map.entry(g).or_insert_with(|| {
                    let ng = GroupId(next_group);
                    next_group += 1;
                    ng
                })
            });
            cmds.push(Command::Split { id, at, new_id: ClipId(next_id), new_group });
            next_id += 1;
        }
        self.apply_batch(cmds);
    }

    pub fn delete_selected(&mut self) {
        // we delete the selection together with link groups (as Del on an A/V pair
        // in the old editor, only now in bulk)
        let ids = self.selected_with_groups();
        if ids.is_empty() {
            return;
        }
        self.apply_batch_restoring(ids.into_iter().map(|id| Command::RemoveClip { id }).collect());
        self.selected.clear();
    }

    pub fn add_media_clip(&mut self, path: &str) {
        let at = self.player.transport.playhead();
        self.add_media_clip_at(path, at, None);
    }

    /// Media + clip(s) at a concrete position (drop at the mouse position, item 2 of v1.4).
    /// The path of the ADDED media is absolute (R.67).
    ///
    /// The model has no relative file paths: relativity appears only on WRITE, and only
    /// relative to the project file.
    /// Otherwise `sources/logo.jpg`, given relative to the launch directory,
    /// on saving to `sources/p.otio` turned into
    /// `sources/sources/logo.jpg` — the file “vanished” without moving from its
    /// place (exactly what happened with the owner’s picture). Pseudo-paths
    /// (`synth:`, `gen:`) remain as they are — they are not files.
    pub(crate) fn absolute_media_path(path: &str) -> String {
        if kt_model::is_pseudo_path(path) {
            return path.to_string();
        }
        let p = std::path::Path::new(path);
        if p.is_absolute() {
            return path.to_string();
        }
        std::path::absolute(p)
            .map(|a| a.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string())
    }

    pub fn add_media_clip_at(&mut self, path: &str, at: Tick, lane_hint: Option<(ClipKind, u32)>) {
        let path = &Self::absolute_media_path(path);
        let media_id = MediaId(self.next_media_id());
        // the sync snapshot — BEFORE apply: we raise the source ourselves below (R.52)
        self.media_paths.insert(media_id, path.to_string());
        if !self.apply(Command::AddMedia { id: media_id, path: path.to_string() }) {
            self.media_paths.remove(&media_id);
            return;
        }
        self.place_media_at(media_id, at, lane_hint);
        // Incrementally: a new worker on the shared cache. Recreating the player
        // here reset the WHOLE warmed cache (item 27) and broke the deltas of the
        // perf counters (item 24: “10^19 evictions/s”).
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let resolved = resolve_media_path(base.as_deref(), path);
        match open_source_indexed(&resolved, self.index_db_path().as_deref()) {
            Ok(src) => self.player.add_media(media_id, src),
            Err(e) => self.note(format!("{path}: {e}")),
        }
        self.thumbs.add_media(media_id, resolved.clone());
        // in the index DB — the resolved path (item 150: a raw relative path
        // gave the file a SECOND media_id in the cache next to the resolved one)
        self.index.enqueue(
            self.project_path.as_ref().map(|p| p.with_extension("ktindex.db")),
            vec![(media_id, resolved.clone())],
        );
        self.note(crate::tf!(k::msg::ADDED, path));
    }

    /// Clip(s) of existing media at the cursor (a click in the bin).
    pub fn place_media_at_cursor(&mut self, media_id: MediaId) {
        let at = self.player.transport.playhead();
        self.place_media_at(media_id, at, None);
    }

    /// An instance of a REEL at position `at` (R.55, C6).
    ///
    /// A video+audio pair in one link group — as in collapsing
    /// (`collapse_into_sequence`): then “placed it, then slowed the whole thing down”
    /// comes for free via `sync_warp_to_link_group`.
    fn place_reel_at(&mut self, seq: MediaId, at: Tick, lane_hint: Option<(ClipKind, u32)>) {
        let doc = self.model.document();
        let Some(s) = doc.sequence(seq) else { return };
        let len = s.end().max(kt_model::MIN_CLIP_DURATION);
        let kinds: Vec<ClipKind> = [ClipKind::Video, ClipKind::Audio]
            .into_iter()
            .filter(|k| s.clips.iter().any(|c| c.kind == *k))
            .collect();
        if kinds.is_empty() {
            self.warn(tr(k::msg::NEST_EMPTY_REEL));
            return;
        }
        let mut next_clip = self.next_clip_id();
        let group = (kinds.len() > 1).then(|| {
            let g = kt_model::GroupId(self.next_group_id());
            g
        });
        let mut cmds = Vec::new();
        let mut first = None;
        for k in kinds {
            let id = ClipId(next_clip);
            next_clip += 1;
            first.get_or_insert(id);
            cmds.push(Command::AddClip(kt_model::ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id,
                link_group: group,
                kind: k,
                lane: match lane_hint {
                    Some((hk, l)) if hk == k => l,
                    _ => 0,
                },
                start: at,
                duration: len,
                source_in: Tick::ZERO,
                media: seq,
                gain_db: 0.0,
                reversed: false,
                fx: Vec::new(),
                fx_bypass: Vec::new(),
                fx_auto: Vec::new(),
                opacity: 1.0,
                fx_off: false,
            }));
        }
        if self.apply_batch(cmds)
            && let Some(id) = first
        {
            self.select_only(id);
        }
    }

    /// Clip(s) of media at position `at`; `lane_hint` — the track under the mouse on
    /// the drop (item 2 of v1.4): its kind gets this lane, the paired half — lane 0.
    pub fn place_media_at(
        &mut self,
        media_id: MediaId,
        at: Tick,
        lane_hint: Option<(ClipKind, u32)>,
    ) {
        // A REEL is placed as an INSTANCE (R.55, C6): it has neither a file nor
        // a duration from a source — the length is taken from its content.
        // Before, a reel could be placed in a second place ONLY
        // by copy-pasting an already placed instance.
        if self.model.document().sequence(media_id).is_some() {
            self.place_reel_at(media_id, at, lane_hint);
            return;
        }
        let Some(path) =
            self.model.project().media.iter().find(|m| m.id == media_id).map(|m| m.path.clone())
        else {
            return;
        };
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let resolved = resolve_media_path(base.as_deref(), &path);
        let info = match open_source_indexed(&resolved, self.index_db_path().as_deref()) {
            Ok(s) => s.info().clone(),
            Err(e) => {
                self.note(format!("{path}: {e}"));
                return;
            }
        };
        let cmds = self.clip_cmds_for(media_id, &info, at, lane_hint);
        self.apply_batch(cmds);
    }

    /// Commands “put clip(s) of this media at the point”. Moved out of
    /// `place_media_at` (R.53, item 416): creating a generator must put the
    /// clip in THE SAME batch as the media — otherwise the new kind appears
    /// only in the bin, and the user types the title blind (rule 4:
    /// a useful default).
    pub(crate) fn clip_cmds_for(
        &mut self,
        media_id: MediaId,
        info: &kt_media::MediaInfo,
        at: Tick,
        lane_hint: Option<(ClipKind, u32)>,
    ) -> Vec<Command> {
        let dur = if info.duration.0 > 0 { info.duration } else { Tick(90_000 * 5) };
        let group = (info.has_video && info.has_audio).then(|| GroupId(self.next_group_id()));
        let mut next_id = self.next_clip_id();
        let mut cmds = Vec::new();
        // R.62: the FIRST clip into an empty project with implicit settings sets
        // the raster and fps (as in Premiere: “change the settings to match the clip”); after that
        // a mismatch of proportions does NOT ask (rule 4), but reports, with
        // an address of “where to change it” (rules 16, 19). Procedural sources
        // (fps = None) have no proportions and do not set the project.
        if info.has_video && info.fps.is_some() {
            let settings = self.model.document().settings.clone();
            let (vw, vh) = info.view_size();
            let even = |v: f64| ((v.round() as u32).max(16)) & !1;
            let (w, h) = (even(vw), even(vh));
            let empty_root =
                self.model.editing().is_none() && self.model.project().clips.is_empty();
            if empty_root && !settings.settings_explicit {
                let fps = info
                    .fps
                    .filter(|(n, d)| *d > 0 && n % d == 0)
                    .map(|(n, d)| n / d)
                    .filter(|f| (1..=120).contains(f) && 90_000 % f == 0);
                let mut st = settings.clone();
                st.width = w;
                st.height = h;
                if let Some(f) = fps {
                    st.fps = f;
                }
                if st != settings {
                    // the preview share is kept (item 20 of round 7)
                    let pq = self.player.preview_quality();
                    let frac = pq.width as f32 / settings.width.max(1) as f32;
                    self.player.set_preview_quality(preview_quality_for(w, h, frac));
                    cmds.push(Command::SetSettings { settings: st });
                }
                match fps {
                    Some(f) => self.note(crate::tf!(k::msg::FIRST_CLIP_SET_PROJECT, w, h, f)),
                    None => {
                        let (n, d) = info.fps.unwrap_or((0, 1));
                        let shown = format!("{:.3}", n as f64 / d.max(1) as f64);
                        self.note(crate::tf!(
                            k::msg::FIRST_CLIP_FPS_KEPT,
                            w,
                            h,
                            shown,
                            settings.fps
                        ));
                    }
                }
            } else {
                let pa = settings.width as f64 / settings.height.max(1) as f64;
                let va = vw / vh.max(1.0);
                if ((va - pa) / pa).abs() > 0.01 {
                    let name = self
                        .model
                        .project()
                        .media
                        .iter()
                        .find(|m| m.id == media_id)
                        .map(|m| {
                            std::path::Path::new(&m.path)
                                .file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_else(|| m.path.clone())
                        })
                        .unwrap_or_default();
                    let rule = crate::settings_window::rule_label(settings.conform);
                    self.note(crate::tf!(
                        k::msg::CONFORM_MISMATCH,
                        name,
                        w,
                        h,
                        settings.width,
                        settings.height,
                        rule
                    ));
                }
            }
        }
        for (kind, present) in
            [(ClipKind::Video, info.has_video), (ClipKind::Audio, info.has_audio)]
        {
            if !present {
                continue;
            }
            let lane = match lane_hint {
                Some((k, l)) if k == kind => l,
                _ => 0,
            };
            cmds.push(Command::AddClip(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(next_id),
                link_group: group,
                kind,
                lane,
                start: at,
                duration: dur,
                source_in: Tick(0),
                media: media_id,
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
            }));
            next_id += 1;
        }
        cmds
    }

    // -- snap ---------------------------------------------------------------

    fn draw_preview(&mut self, ui: &mut egui::Ui) {
        let t = self.player.video_playhead();
        let q = self.player.preview_quality();
        // The key is a tick, ALIGNED to the fps grid (test 39): previously the key
        // was the raw tick — during playback it changed every UI frame and the texture
        // was re-uploaded to the GPU 60 times/s regardless of the project fps
        // (up to ~500 MB/s at 1080p — the main source of “visually <30 fps”).
        let grid = self.player.video_grid();
        let raw_key = preview_key_tick(grid, t);
        // The key does NOT go back AT ALL, except on seek/loop-wrap (item 14 of round 14):
        // the earlier tolerance “rollback up to 2 cells” was blind to speed and at
        // 0.25× let through picture rollbacks by a couple of frames back.
        let tgen = (self.player.transport.generation(), self.player.transport.wraps());
        let reanchor = !self.player.transport.is_playing() || tgen != self.preview_gen;
        let (key_tick, skipped, rolled) = kt_engine::player::preview_key_step(
            self.preview_key.map(|k| k.0),
            raw_key,
            grid.frame_step(),
            if self.play_dir_back { -1 } else { 1 },
            reanchor,
        );
        self.preview_gen = tgen;
        self.perf.video_cells_skipped += skipped;
        if rolled {
            self.perf.video_rollbacks += 1;
        }
        let key_now = (key_tick, (q.width, q.height));
        // The CONTENT of the sources is part of the preview key (R.53, item 412).
        //
        // An edit of generator parameters moves neither the cursor, nor the quality,
        // nor the plan: the key stays the same, the composite counts as “already computed”,
        // and the OLD frame hangs on screen —
        // until the first movement of the playhead.
        // The harness did not see this: t478 asks the player’s cache directly,
        // bypassing the preview memoisation, and it was green on a program where a colour edit
        // did not reach the picture (the live scenario `generator` showed a black
        // preview at an orange value in the model).
        let content = self.player.content_version();
        // …and the PROJECT is part of the key too (R.62, t722b): a change of the conform
        // rule in the settings window while paused touches none of the above, and the composite
        // counted as “already computed”
        let project = self.player.project_version();
        let need = match self.preview_key {
            Some((pt, pq, was_complete)) => {
                pt != key_now.0
                    || pq != key_now.1
                    || !was_complete
                    || self.preview_content != content
                    || self.preview_project != project
            }
            None => true,
        };
        self.preview_content = content;
        self.preview_project = project;
        let new_frame = self.preview_key.map(|k| k.0) != Some(key_now.0);
        let mut complete = self.preview_key.map(|k| k.2).unwrap_or(false);
        if need {
            self.last_compose_retry = std::time::Instant::now();
            let mut rgba = std::mem::take(&mut self.preview_buf);
            // composition by the ratcheted key, not the raw t:
            // otherwise on a retry (the frame is still loading) the picture was rebuilt
            // for a position 1–2 cells behind the key, and an earlier frame
            // went on screen (item 14 of round 14)
            let (cq, c) = self.player.compose_preview_into(Tick(key_tick), &mut rgba);
            complete = c;
            // we hold the last FULL frame: an incomplete composite does not overwrite the
            // picture (the flicker “black frame ↔ Loading”, item 14/15)
            if c || self.texture.is_none() {
                // glow mip: a coarse box downscale 32×18; during playback — every 4th new frame
                // (ambient glow flicker will not notice)
                self.glow_skip = self.glow_skip.wrapping_add(1);
                if self.sw_render == Some(false) && self.glow_skip % 4 == 1 {
                    let (gw, gh) = (32usize, 18usize);
                    let mut small = vec![0u8; gw * gh * 4];
                    let (sw, sh) = (cq.width as usize, cq.height as usize);
                    for gy in 0..gh {
                        let sy = gy * sh / gh;
                        for gx in 0..gw {
                            let sx = gx * sw / gw;
                            let si = (sy * sw + sx) * 4;
                            let di = (gy * gw + gx) * 4;
                            small[di..di + 4].copy_from_slice(&rgba[si..si + 4]);
                        }
                    }
                    let gimg = egui::ColorImage::from_rgba_unmultiplied([gw, gh], &small);
                    match &mut self.glow_texture {
                        Some(t) => t.set(gimg, egui::TextureOptions::LINEAR),
                        None => {
                            self.glow_texture = Some(ui.ctx().load_texture(
                                "preview-glow",
                                gimg,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                    }
                }
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [cq.width as usize, cq.height as usize],
                    &rgba,
                );
                let opts = if self.smooth_scaling {
                    egui::TextureOptions::LINEAR
                } else {
                    egui::TextureOptions::NEAREST
                };
                match &mut self.texture {
                    Some(tex) => tex.set(image, opts),
                    None => {
                        self.texture = Some(ui.ctx().load_texture("preview", image, opts));
                    }
                }
                // honest V-fps metric: real displays of NEW frames
                if new_frame {
                    self.perf.video_frames_shown += 1;
                }
            }
            self.preview_key = Some((key_now.0, key_now.1, c));
            self.preview_buf = rgba; // buffer back (reuse)
        }
        if complete {
            self.incomplete_since = None;
        } else if self.incomplete_since.is_none() {
            self.incomplete_since = Some(std::time::Instant::now());
        }
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        self.preview_rect = rect;
        // the picture is drawn from the very top of the window, and clicks are accepted
        // BELOW the window-drag strip: otherwise a press at the edge would be
        // at once a resize, a window move and play/pause (item 1 of round 11)
        let hit =
            egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + TOP_DRAG_H), rect.max);
        let resp = ui.interact(hit, ui.id().with("preview-click"), egui::Sense::click());
        let resp = self.hint(resp, tr(k::hint::PREVIEW_CLICK));
        // Right click on the preview (R.49, item 365). A double click already knows how to
        // detach and re-attach the window, but this could be learned only
        // from a hint — the preview was silent on the right button. The item is exactly one
        // and exactly the one the gesture performs: zero new keys.
        {
            let detached = self.preview_detached;
            let mut toggle = false;
            let mut picked = false;
            let mut cycle = false;
            let mode_now = self.geom_ctl;
            resp.clone().context_menu(|ui| {
                picked = true;
                // FRAME CONTROLS (R.64) — as the first item: this is the thing
                // for which people most often come to the preview with the right button.
                // One item for all three states: exactly one set is active
                // (the Resolve rule), and it has one switch.
                let r = ui.button(crate::tf!(
                    k::geomctl::TOGGLE,
                    crate::geomctl::mode_name(mode_now.next())
                ));
                crate::widgets::field_probe::note("geom.toggle", r.rect);
                if r.clicked() {
                    cycle = true;
                    ui.close();
                }
                ui.separator();
                let label = if detached {
                    tr(k::menu::view::ATTACH_PREVIEW)
                } else {
                    tr(k::menu::view::DETACH_PREVIEW)
                };
                if ui.button(label).clicked() {
                    toggle = true;
                    ui.close();
                }
            });
            if cycle {
                self.cycle_geom_mode();
            }
            if picked {
                self.set_panel_target(crate::ctx_menu::PanelTarget::Preview);
            }
            if toggle {
                self.preview_detached = !detached;
                self.dirty_ui = true;
            }
        }
        // items 10–11 of round 7: a single click (deferred for the double-click window) =
        // play/pause; a double = detach/attach. It also works in the separate preview
        // window (a double click there returns the preview back).
        {
            let pending_ms = self.preview_click_at.map(|t| t.elapsed().as_millis());
            let (action, op) = preview_click_step(
                pending_ms,
                resp.clicked(),
                resp.double_clicked(),
                PREVIEW_CLICK_DELAY_MS,
            );
            match op {
                PendingOp::Arm => self.preview_click_at = Some(std::time::Instant::now()),
                PendingOp::Clear => self.preview_click_at = None,
                PendingOp::Keep => {}
            }
            match action {
                PreviewClick::TogglePlay => self.on_transport_toggle(),
                PreviewClick::ToggleDetach => {
                    self.preview_detached =
                        preview_detached_next(self.preview_detached, true, false);
                }
                PreviewClick::None => {}
            }
            if let Some(t0) = self.preview_click_at {
                // wait for the window to expire without input
                let left = PREVIEW_CLICK_DELAY_MS.saturating_sub(t0.elapsed().as_millis());
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(left as u64 + 5));
            }
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(10, 10, 12));
        if let Some(tex) = &self.texture {
            let scale =
                (rect.width() / q.width as f32).min(rect.height() / q.height as f32).max(0.05);
            let size = egui::vec2(q.width as f32 * scale, q.height as f32 * scale);
            let img_rect = egui::Rect::from_center_size(rect.center(), size);
            // ambient glow as on YouTube (item 18): a BLURRED mini-copy
            // (32×18, LINEAR stretch = strong blur) + vignette to the background
            if self.sw_render == Some(false)
                && let Some(glow) = &self.glow_texture
            {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                for (expand, alpha) in [(0.35f32, 40u8), (0.18, 60)] {
                    let glow_rect = egui::Rect::from_center_size(
                        img_rect.center(),
                        img_rect.size() * (1.0 + expand),
                    )
                    .intersect(rect);
                    painter.image(
                        glow.id(),
                        glow_rect,
                        uv,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                    );
                }
                // vignette: the edges of the area fade into the background colour
                let bg = egui::Color32::from_rgb(10, 10, 12);
                let fade = 36.0f32;
                let mut vin = egui::Mesh::default();
                let mut quad = |a: egui::Pos2,
                                b: egui::Pos2,
                                ca: egui::Color32,
                                cb: egui::Color32,
                                horiz: bool| {
                    let i = vin.vertices.len() as u32;
                    let pts = if horiz {
                        [
                            (egui::pos2(a.x, a.y), ca),
                            (egui::pos2(b.x, a.y), cb),
                            (egui::pos2(b.x, b.y), cb),
                            (egui::pos2(a.x, b.y), ca),
                        ]
                    } else {
                        [
                            (egui::pos2(a.x, a.y), ca),
                            (egui::pos2(b.x, a.y), ca),
                            (egui::pos2(b.x, b.y), cb),
                            (egui::pos2(a.x, b.y), cb),
                        ]
                    };
                    for (p, c) in pts {
                        vin.vertices.push(egui::epaint::Vertex {
                            pos: p,
                            uv: egui::epaint::WHITE_UV,
                            color: c,
                        });
                    }
                    vin.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                };
                let t = egui::Color32::from_rgba_unmultiplied(10, 10, 12, 0);
                quad(rect.left_top(), egui::pos2(rect.left() + fade, rect.bottom()), bg, t, true);
                quad(egui::pos2(rect.right() - fade, rect.top()), rect.right_bottom(), t, bg, true);
                quad(rect.left_top(), egui::pos2(rect.right(), rect.top() + fade), bg, t, false);
                quad(
                    egui::pos2(rect.left(), rect.bottom() - fade),
                    rect.right_bottom(),
                    t,
                    bg,
                    false,
                );
                painter.add(egui::Shape::mesh(vin));
            }
            painter.image(
                tex.id(),
                img_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            // a thin frame outline
            painter.rect(
                img_rect,
                2.0,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 24)),
                egui::StrokeKind::Outside,
            );
            // safe zones (R.52 C3, F-TXT-001): while the title window is open —
            // action 5% and title 10% (a legacy of CRT televisions, still alive today)
            if self.show_safe_zones() {
                for r in safe_zone_rects(img_rect) {
                    painter.rect(
                        r,
                        0.0,
                        egui::Color32::TRANSPARENT,
                        egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 70),
                        ),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            // GEOMETRY CONTROLS — before the title drag: the smaller and more specific
            // target (the title block) has priority, egui gives the pointer to the
            // last registered interactor
            self.preview_img_rect = img_rect;
            // the picture itself — the README's before/after crops use it
            crate::widgets::field_probe::note("preview.frame", img_rect);
            self.geometry_controls(ui, &painter, img_rect, (q.width, q.height));
            // the show button — AFTER the frame: egui gives the pointer to the last
            // registered interactor, and the frame body (moving) covers the whole frame —
            // register the button earlier, and a click on it would go to the move
            // (caught by the live scenario `geomctl`)
            self.geom_toggle_button(ui, img_rect);
            self.title_drag_in_preview(ui, &painter, img_rect);
            // scrim — only if the incompleteness lasts >180 ms (v1)
            let show_overlay = self
                .incomplete_since
                .map(|t0| t0.elapsed() > std::time::Duration::from_millis(180))
                .unwrap_or(false);
            if show_overlay {
                painter.rect_filled(img_rect, 2.0, egui::Color32::from_black_alpha(140));
                painter.text(
                    img_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    tr(k::perf::LOADING),
                    egui::FontId::proportional(16.0),
                    egui::Color32::from_gray(230),
                );
            }
        }
    }
    /// Compact speed control (items 4–5 of round 5): an exact field for the main
    /// speed + ONE strip — dragging on it = shuttle (springy: released, it returns
    /// to the main speed, Vegas), double-click = 1×; the main speed marker is
    /// visible on the strip. The strip mapping is slider_to_rate (test 46).
    /// Speed control (items 18–19 of round 7, the user’s decision): TWO
    /// parallel strips in the former 170×20 footprint — the upper one moves the
    /// CONSTANT speed (it stays), the lower one is a shuttle with auto-return.
    /// The value while dragging — an overlay at the cursor; the input field and the
    /// right-hand label “+1.00×” are removed (we do not make obvious labels).
    pub(crate) fn draw_speed_control(&mut self, ui: &mut egui::Ui) {
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(170.0, 20.0), egui::Sense::click_and_drag());
        self.speed_rect = rect;
        crate::widgets::field_probe::note("speed", rect);
        // Numeric input lives IN THE SAME rectangle (R.57): no window, no
        // extra strip — rules 3, 8 and 13.
        if self.speed_entry.is_some() {
            self.draw_speed_entry(ui, rect, resp);
            return;
        }
        // the field was closed FROM THE OUTSIDE (Esc cascade) — focus must still
        // return, otherwise the hotkeys are dead until a click elsewhere (rule 14)
        if resp.has_focus() {
            ui.memory_mut(|m| m.surrender_focus(resp.id));
        }
        let p = ui.painter_at(rect);
        // the temporary speed on the lower strip is the shuttle OR acceleration by
        // holding space: without this, during acceleration the widget lied “1×”
        // (item 3 of round 16, found by a live check)
        let temp_t = self.speed_temp_slider();
        let overriding = temp_t.is_some();
        let mid = rect.center().y;
        let upper = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), mid));
        let lower = egui::Rect::from_min_max(egui::pos2(rect.left(), mid), rect.max);
        p.rect_filled(rect, 2.0, egui::Color32::from_gray(24));
        p.line_segment(
            [egui::pos2(rect.left(), mid), egui::pos2(rect.right(), mid)],
            egui::Stroke::new(1.0, crate::theme::SEAM),
        );
        let to_x = |t: f32| rect.left() + (t + 1.0) * 0.5 * rect.width();
        // notches: centre (0/stop) and ±1×, ±2× — across the full height
        for (t, strong) in [
            (0.0f32, true),
            (rate_to_slider(1.0), true),
            (rate_to_slider(-1.0), true),
            (1.0, false),
            (-1.0, false),
        ] {
            let x = to_x(t);
            p.line_segment(
                [
                    egui::pos2(x, rect.top() + if strong { 2.0 } else { 5.0 }),
                    egui::pos2(x, rect.bottom() - if strong { 2.0 } else { 5.0 }),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_gray(if strong { 70 } else { 48 })),
            );
        }
        // the strip is selected in the frame of the press and held until the end of the drag
        if resp.drag_started()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            self.speed_drag_upper = Some(pos.y < mid);
            self.speed_drag_vx = pos.x - rect.left();
        } else if resp.dragged() {
            // Shift — FINE adjustment (R.57): we accumulate INCREMENTS into our own
            // virtual cursor. The absolute pointer position must not be divided —
            // Shift pressed in the middle of a gesture would drag the value
            // to the centre with a jump.
            let fine = ui.input(|i| i.modifiers.shift);
            self.speed_drag_vx = drag_advance(self.speed_drag_vx, resp.drag_delta().x, fine);
        }
        if resp.dragged() || resp.drag_started() {
            let raw = self.speed_drag_vx / rect.width() * 2.0 - 1.0;
            let t = raw.clamp(-1.0, 1.0);
            // went beyond the widget — a “second wind” (item 4 of round 8)
            let over_px = ((raw.abs() - 1.0) * rect.width() * 0.5).max(0.0);
            let rate_of =
                |t: f32| overshoot_rate(slider_to_rate(t), over_px, OVERSHOOT_PX_PER_DOUBLE);
            if self.speed_drag_upper == Some(true) {
                // upper strip: constant speed (it stays)
                self.base_rate = rate_of(t);
                if self.shuttle_override.is_none() {
                    self.player.set_playback_rate(self.base_rate);
                }
            } else {
                // lower: temporary shuttle with auto-return
                self.shuttle_override = Some(t);
                self.player.set_playback_rate(rate_of(t));
            }
        }
        if resp.drag_stopped() {
            self.speed_drag_upper = None;
            if self.shuttle_override.take().is_some() {
                self.player.set_playback_rate(self.base_rate);
            }
        }
        if resp.double_clicked() {
            self.base_rate = 1.0;
            self.shuttle_override = None;
            self.player.set_playback_rate(1.0);
        }
        let base_t = rate_to_slider(self.base_rate);
        let cur_t = temp_t.unwrap_or(base_t);
        // upper strip: fill from the centre to base + marker
        p.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(to_x(0.0).min(to_x(base_t)), upper.top() + 3.0),
                egui::pos2(to_x(0.0).max(to_x(base_t)), upper.bottom() - 1.0),
            ),
            1.0,
            egui::Color32::from_rgba_unmultiplied(120, 160, 255, 70),
        );
        p.line_segment(
            [egui::pos2(to_x(base_t), upper.top() + 1.0), egui::pos2(to_x(base_t), upper.bottom())],
            egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(150, 180, 255, 220)),
        );
        // lower strip: shuttle fill + a dot of the current speed
        let (x0, x1) = (to_x(0.0).min(to_x(cur_t)), to_x(0.0).max(to_x(cur_t)));
        if overriding {
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x0, lower.top() + 1.0),
                    egui::pos2(x1, lower.bottom() - 3.0),
                ),
                1.0,
                egui::Color32::from_rgba_unmultiplied(255, 190, 90, 90),
            );
        }
        p.circle_filled(
            egui::pos2(to_x(cur_t), lower.center().y),
            4.5,
            if overriding {
                egui::Color32::from_rgb(255, 200, 100)
            } else {
                egui::Color32::from_gray(210)
            },
        );
        // value — overlay at the cursor ONLY while dragging (item 19 of round 7)
        if self.speed_drag_upper.is_some()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let cur = self.player.playback_rate();
            let shown = if self.speed_drag_upper == Some(true) { self.base_rate } else { cur };
            egui::Area::new(egui::Id::new("speed-drag-overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos + egui::vec2(14.0, -26.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.monospace(format_rate(shown));
                    });
                });
        }
        // NUMERIC INPUT (R.57, the owner’s request “must be able to
        // set 0.05”): right click or Ctrl+click on the strip. Both entries
        // lead into one body (rule 17).
        if resp.secondary_clicked() || (resp.clicked() && ui.input(|i| i.modifiers.command)) {
            self.speed_entry = Some(SpeedEntry { text: String::new(), focus: true, bad: false });
        }
        self.hint(resp, tr(k::hint::SPEED));
    }

    /// The speed input field IN PLACE of the strip (R.57). The form is the same as the
    /// timecode mask in the toolbar (`toolbar.rs`): the same rectangle, the
    /// same monospaced font, an accent underline; events are consumed so that
    /// digits and Enter do not leak into the hotkeys. A bluntly inserted
    /// `TextEdit` is forbidden here by rule 8.
    fn draw_speed_entry(&mut self, ui: &mut egui::Ui, rect: egui::Rect, resp: egui::Response) {
        let Some(mut st) = self.speed_entry.clone() else { return };
        if st.focus {
            resp.request_focus();
            st.focus = false;
        }
        // arrows/Tab must not move the focus out of the field
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                resp.id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: false,
                },
            )
        });
        let mut apply = false;
        let mut cancel = false;
        ui.input_mut(|i| {
            i.events.retain(|e| match e {
                egui::Event::Text(t)
                    if t.chars().all(|c| {
                        c.is_ascii_digit()
                            || matches!(c, '.' | ',' | '-' | '+' | '\u{2212}' | 'x' | '\u{00d7}')
                    }) =>
                {
                    st.text.push_str(t);
                    st.bad = false;
                    false
                }
                egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                    st.text.pop();
                    st.bad = false;
                    false
                }
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                    apply = true;
                    false
                }
                egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                    cancel = true;
                    false
                }
                _ => true,
            })
        });
        let font = egui::FontId::monospace(13.0);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(22, 24, 28));
        painter.line_segment(
            [
                egui::pos2(rect.left() + 2.0, rect.bottom() - 1.0),
                egui::pos2(rect.right() - 2.0, rect.bottom() - 1.0),
            ],
            egui::Stroke::new(
                1.0,
                // rule 16: an unreadable value — an ERROR, not a faded hint; the field
                // stays open in this case
                if st.bad { HINT_ERROR } else { crate::theme::ACCENT },
            ),
        );
        // empty — we show the current speed muted: the first digit replaces it
        // entirely (the same as “opened with everything selected”)
        let (txt, col) = if st.text.is_empty() {
            (format_rate(self.base_rate), egui::Color32::from_gray(120))
        } else {
            (st.text.clone(), egui::Color32::from_rgb(235, 235, 240))
        };
        painter.text(
            egui::pos2(rect.left() + 5.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            txt,
            font,
            col,
        );
        if apply {
            match parse_rate_text(&st.text).map(normalize_rate) {
                Some(r) => {
                    self.base_rate = r;
                    if self.shuttle_override.is_none() {
                        self.player.set_playback_rate(r);
                    }
                    self.speed_entry = None;
                }
                None => {
                    st.bad = true;
                    self.speed_entry = Some(st);
                }
            }
        } else if cancel || (resp.lost_focus() && !resp.has_focus()) {
            self.speed_entry = None;
        } else {
            self.speed_entry = Some(st);
        }
        // FOCUS MUST RETURN (rule 14). The guard stands on EVERY closing path
        // at once — including the external one (the Esc cascade clears `speed_entry`
        // without our knowledge): the widget lives every frame with the same id, and
        // “simply close” is not enough — egui would consider it focused, and
        // `handle_keys` is gated by `egui_wants_keyboard_input()`.
        if self.speed_entry.is_none() {
            ui.memory_mut(|m| m.surrender_focus(resp.id));
        }
        self.hint(resp, tr(k::hint::SPEED_ENTRY));
    }

    /// Preview controls (item 17 of round 7: without obvious labels — the values
    /// are self-sufficient; item 20: Resolution BEFORE Quality — quality is the
    /// fraction of the project resolution, the preview engine sees only the product).
    fn draw_preview_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let proj = self.model.project().settings.clone();
            let cur_q = self.player.preview_quality();
            let fracs = [1.0f32 / 16.0, 1.0 / 8.0, 1.0 / 4.0, 1.0 / 2.0, 1.0];
            let mut frac_idx = {
                let ratio = cur_q.width as f32 / proj.width.max(1) as f32;
                fracs
                    .iter()
                    .enumerate()
                    .min_by(|a, b| (a.1 - ratio).abs().partial_cmp(&(b.1 - ratio).abs()).unwrap())
                    .map(|(i, _)| i)
                    .unwrap_or(2)
            };
            // the fraction size — SIMILAR to the project frame (R.64)
            let q_for = preview_quality_for;
            let mut fps = proj.fps;
            let before_fps = fps;
            let r = egui::ComboBox::from_id_salt("fps")
                .selected_text(fps.to_string())
                .show_ui(ui, |ui| {
                    for f in [24u32, 25, 30, 60] {
                        ui.selectable_value(&mut fps, f, f.to_string());
                    }
                })
                .response;
            self.hint(r, tr(k::hint::FPS));
            if fps != before_fps {
                let mut st = proj.clone();
                st.fps = fps;
                self.apply(Command::SetSettings { settings: st });
            }
            ui.separator();
            let mut res = (proj.width, proj.height);
            let before_res = res;
            let res_resp = egui::ComboBox::from_id_salt("res")
                .selected_text(format!("{}×{}", res.0, res.1))
                .show_ui(ui, |ui| {
                    for r in
                        [(640u32, 360u32), (1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)]
                    {
                        ui.selectable_value(&mut res, r, format!("{}×{}", r.0, r.1));
                    }
                    // “Max (W×H)” — component-wise maximum over the sources (v1)
                    if let Some((mw, mh)) = self.max_source_resolution_cached() {
                        ui.selectable_value(&mut res, (mw, mh), format!("Max ({mw}×{mh})"));
                    }
                })
                .response;
            self.hint(res_resp, tr(k::hint::RESOLUTION));
            if res != before_res {
                let mut st = proj.clone();
                st.width = res.0;
                st.height = res.1;
                self.apply(Command::SetSettings { settings: st });
                // quality — a fraction of the resolution: keep the fraction, not the pixels
                // (previously the shown fraction silently drifted, item 20 of round 7)
                self.player.set_preview_quality(q_for(res.0, res.1, fracs[frac_idx]));
            }
            ui.separator();
            // The pitch engine — a PROJECT setting (R.48): one choice for the whole
            // edit, so it sits next to fps and the resolution, not in the
            // parameters of the effect.
            let mut engine = proj.pitch_engine;
            let before_engine = engine;
            let eng_resp = egui::ComboBox::from_id_salt("pitch_engine")
                .selected_text(match engine {
                    kt_model::PitchEngine::Wsola => tr(k::fx::PITCH_WSOLA),
                    kt_model::PitchEngine::Vocoder => tr(k::fx::PITCH_VOCODER),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut engine,
                        kt_model::PitchEngine::Wsola,
                        tr(k::fx::PITCH_WSOLA),
                    );
                    ui.selectable_value(
                        &mut engine,
                        kt_model::PitchEngine::Vocoder,
                        tr(k::fx::PITCH_VOCODER),
                    );
                })
                .response;
            self.hint(eng_resp, tr(k::hint::PITCH_ENGINE));
            if engine != before_engine {
                let mut st = proj.clone();
                st.pitch_engine = engine;
                self.apply(Command::SetSettings { settings: st });
            }
            ui.separator();
            let labels = ["1/16", "1/8", "1/4", "1/2", "1/1"];
            let before = frac_idx;
            let q_resp = egui::ComboBox::from_id_salt("quality")
                .selected_text(labels[frac_idx])
                .show_ui(ui, |ui| {
                    for (i, l) in labels.iter().enumerate() {
                        ui.selectable_value(&mut frac_idx, i, *l);
                    }
                })
                .response;
            self.hint(q_resp, tr(k::hint::PREVIEW_QUALITY));
            if frac_idx != before {
                self.player.set_preview_quality(q_for(proj.width, proj.height, fracs[frac_idx]));
            }
            ui.separator();
            let sm = ui.checkbox(&mut self.smooth_scaling, tr(k::perf::SMOOTHING));
            self.hint(sm, tr(k::hint::SMOOTHING));
        });
    }

    fn draw_perf_box(&mut self, ui: &mut egui::Ui) {
        // long lines must not set the minimum width of the panel (item 18 of round 6)
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        ui.label(egui::RichText::new(tr(k::perf::TITLE)).strong());
        ui.add_space(2.0);
        let ord = std::sync::atomic::Ordering::Relaxed;
        let q = self.player.preview_quality();
        // the largest decode step on the edit (R.46, item 322)
        let max_stage = self
            .player
            .video_decode_qualities()
            .into_iter()
            .map(|(_, dq)| (dq.width / q.width.max(1)).max(1))
            .max()
            .unwrap_or(1);
        // item 15 of round 7: short metrics in two columns (saving height)
        let s = self.model.project().settings.clone();
        let rows: [(String, String); 4] = [
            (
                crate::tf!(
                    k::perf::CACHE_MB,
                    format!("{:.0}", self.player.cache.audio_bytes() as f64 / 1e6),
                    format!("{:.0}", self.player.cache.video_bytes() as f64 / 1e6)
                ),
                // WHAT THESE TWO NUMBERS ARE (R.43, item 289): the left — misses of the
                // RENDERER (the cache did not give a burst), the right — starvation of the
                // CALLBACK (the ring is empty). The user read them as
                // “left/right channel” — and was entitled to: there was no caption.
                crate::tf!(
                    k::perf::UNDERRUNS,
                    self.player.underruns.load(ord),
                    self.player.shared.callback_underruns.load(ord)
                ),
            ),
            (
                crate::tf!(k::perf::PREVIEW_RES, q.width, q.height),
                // decode step (R.46): shown only when there is one — on an ordinary edit the
                // line takes no space
                if max_stage > 1 {
                    crate::tf!(k::perf::DECODE_SCALE, max_stage)
                } else {
                    format!("{} fps, {}×{}", s.fps, s.width, s.height)
                },
            ),
            (
                crate::tf!(k::perf::CLIPS_N, self.model.project().clips.len()),
                crate::tf!(k::perf::MEDIA_N, self.model.project().media.len()),
            ),
            (
                crate::tf!(k::perf::V_SHOWN, self.perf.video_frames_shown),
                // short caption: in a narrow column the long line was cut with an ellipsis
                // and the rollback counter could not be seen
                crate::tf!(
                    k::perf::V_SKIPPED,
                    self.perf.video_cells_skipped,
                    self.perf.video_rollbacks
                ),
            ),
        ];
        // ui.columns divides the AVAILABLE width in half and requires no minimum —
        // Grid with Extend inflated the panel by content and ate the preview
        // (a regression of item 3 of round 7.1; the same defect that item 18 of round 6 fixed)
        ui.columns(2, |cols| {
            for c in cols.iter_mut() {
                c.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                c.spacing_mut().item_spacing.y = 1.0;
            }
            for (a, b) in rows {
                cols[0].weak(a);
                cols[1].weak(b);
            }
        });
        if let Some(g) = &self.gpu_info {
            ui.weak(format!("GPU: {g}"));
        }
        // video FX backend (R.29): GPU by default, CPU — fallback with a notice
        match self.player.video_backend() {
            kt_engine::VideoBackendStatus::Gpu(info) => {
                ui.weak(crate::tf!(k::perf::VIDEO_FX_GPU, info.name));
            }
            _ => {
                ui.weak(tr(k::perf::VIDEO_FX_CPU));
            }
        }
        let r = ui.checkbox(&mut self.ui_economy, tr(k::perf::ECONOMY));
        self.hint(r, tr(k::hint::ECONOMY));
        // “Black box” of playback (R.28, item 10): the ring of transport events
        // and counters lives always, the button dumps it to a file — floating bugs are
        // caught by it without making the user catch the moment. A sticking is visible
        // right here.
        {
            let recs = self.player.flight.records();
            let stalls = recs.iter().filter(|r| r.kind == kt_engine::flight::Ev::Stall).count();
            let btn = ui.button(tr(k::perf::FLIGHT_SAVE));
            let btn = self.hint(btn, tr(k::hint::FLIGHT));
            if btn.clicked() {
                match self.save_flight_log() {
                    Ok(path) => self.note(crate::tf!(k::msg::FLIGHT_SAVED, path)),
                    Err(e) => self.warn(e.to_string()),
                }
            }
            if stalls > 0 {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 105, 97),
                    crate::tf!(k::perf::FLIGHT_STALLS, stalls),
                );
            }
        }
        // cumulative counters of the FFmpeg layer (port of the avcpp open/read/vdec/adec line v1)
        let (opens, reads) = kt_media::ffmpeg::io_stats();
        ui.weak(format!(
            "FFmpeg: open {opens} | read {reads} | vdec {} | adec {}",
            self.player.decode_stats.video_decodes.load(ord),
            self.player.decode_stats.audio_decodes.load(ord)
        ));
        ui.add_space(4.0);
        self.perf.quality.draw(ui, 96.0);
        ui.add_space(4.0);
        self.perf.decode.draw(ui, 96.0);
    }

    fn draw_statusbar(&mut self, ui: &mut egui::Ui) {
        // item 5 of round 7: the window is DRAGGABLE by the status bar (widgets on top will catch
        // their own clicks; empty zones — drag)
        let strip = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), 22.0),
        );
        // the status line rectangle is needed at the END of the frame: the hint is drawn
        // there (item 1 of round 9 — otherwise the status bar sees the hover_hint of the previous
        // frame, or rather nothing at all: it is drawn BEFORE all buttons)
        self.statusbar_rect = strip;
        let bg = ui.interact(strip, ui.id().with("status-drag"), egui::Sense::click_and_drag());
        if bg.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            Self::release_pointer_after_wm_grab(ui.ctx());
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let ord = std::sync::atomic::Ordering::Relaxed;
            ui.add_space(6.0); // item 6 of round 7: the bell does not stick to the edge
            // notification bell (item 18): badge = unread count
            let bell = if self.notices_unread > 0 {
                format!("{} {}", crate::icons::BELL, self.notices_unread)
            } else {
                crate::icons::BELL.to_string()
            };
            let has_warn = self
                .notices
                .iter()
                .rev()
                .take(self.notices_unread)
                .any(|n| n.severity == Severity::Warn);
            let bell_btn = egui::Button::new(egui::RichText::new(bell).color(if has_warn {
                egui::Color32::from_rgb(255, 170, 80)
            } else if self.notices_unread > 0 {
                egui::Color32::from_rgb(160, 200, 255)
            } else {
                egui::Color32::from_gray(140)
            }))
            .frame(false);
            let bell_resp = ui.add(bell_btn);
            self.no_resize_rects.push(bell_resp.rect.expand(2.0));
            if self.hint(bell_resp, tr(k::win::NOTICES_TITLE)).clicked() {
                self.notices_open = !self.notices_open;
                if self.notices_open {
                    self.notices_unread = 0;
                }
            }
            ui.separator();
            // items 21, 24, 25 of round 7: “zoom:” erased (a value with a unit explains itself),
            // “layers” removed, “(loop)” is not written — the button lights up
            ui.weak(format!("{:.0} px/s", 90_000.0 / self.ticks_per_px));
            if let Some(r) = self.model.project().region {
                ui.separator();
                ui.weak(crate::tf!(
                    k::perf::REGION_S,
                    format!("{:.2}", r.start.as_secs_f64()),
                    format!("{:.2}", r.end.as_secs_f64()),
                ));
            }
            // item 22 of round 7: cache/underruns after the region
            ui.separator();
            ui.weak(crate::tf!(
                k::perf::CACHE_SHORT,
                format!("{:.0}", self.player.cache.audio_bytes() as f64 / 1e6),
                format!("{:.0}", self.player.cache.video_bytes() as f64 / 1e6),
            ));
            ui.separator();
            // callback starvation — these are FRAMES that were not there at the output;
            // in milliseconds one can see how much silence accumulated (R.43 item 290)
            let cu = self.player.shared.callback_underruns.load(ord);
            ui.weak(crate::tf!(
                k::perf::UNDERRUNS_SHORT,
                self.player.underruns.load(ord),
                cu,
                cu / (kt_time::AUDIO_SAMPLE_RATE as u64 / 1000).max(1)
            ));
            ui.separator();
            // item 1 of round 7.1: V-fps (the new frames shown) — here, not in the toolbar
            ui.weak(format!("{:.0} vfps", self.perf.vfps));
        });
    }

    /// Take the focus off the input field if the press fell OUTSIDE its
    /// rectangle (R.63 item 4). The rectangle is taken from egui itself
    /// (`read_response`) — we do not keep our own registry of fields: it would diverge
    /// from the real layout at the first new window.
    fn surrender_focus_on_outside_click(&mut self, ctx: &egui::Context) {
        let Some(id) = ctx.memory(|m| m.focused()) else { return };
        let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) else { return };
        if !ctx.input(|i| i.pointer.primary_pressed() || i.pointer.secondary_pressed()) {
            return;
        }
        // no rectangle (the widget disappeared) — the focus has all the less reason
        // to hold on
        let inside = ctx.read_response(id).is_some_and(|r| r.rect.contains(pos));
        if !inside {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        // GATE — TEXT INPUT, not “something is focused” (R.63 item 4).
        //
        // `egui_wants_keyboard_input()` is true when ANY widget has focus, and in egui
        // buttons, checkboxes and sliders are all focusable. One click on a button
        // in the effects window — and all hotkeys are dead until the window is closed:
        // “however I poke at the rest of the interface, space does not pause… probably it
        // is the same with all other fields and buttons” (the owner, R.63 item 4).
        // Only TEXT ENTRY takes the keyboard from us.
        // Our own masks (the timecode in the toolbar, the speed on the strip) are drawn NOT
        // by a `TextEdit`, but by a field in place of the value (rule 8), and egui knows
        // nothing about them — we name them one by one. A new text field must appear
        // in this list;
        // the guards are t98 and t627.
        if ctx.text_edit_focused() || self.tc_edit.is_some() || self.speed_entry.is_some() {
            return;
        }
        struct Keys {
            cut: bool,
            del: bool,
            undo: bool,
            redo: bool,
            save: bool,
            marker: bool,
            track_mute: bool,
            track_solo: bool,
            region_in: bool,
            region_out: bool,
            loop_toggle: bool,
            esc: bool,
            enter_nest: bool,
            copy: bool,
            paste: bool,
            nav_prev: bool,
            nav_next: bool,
            nav_far: bool,
            nav_region: bool,
            goto_tc: bool,
            find: bool,
        }
        // We read SPACE from EVENTS, not key_pressed: the latter turns on
        // keyboard auto-repeat, and a held space sent a series of play/pause
        // (item 3 of round 16). We only remember the press — the decision is made on
        // RELEASE: a short press = play/pause, a long one = acceleration.
        let (space_down, space_up) = ctx.input(|i| {
            let mut down = false;
            let mut up = false;
            for e in &i.events {
                if let egui::Event::Key { key: egui::Key::Space, pressed, repeat, .. } = e {
                    if *pressed {
                        down |= !*repeat;
                    } else {
                        up = true;
                    }
                }
            }
            (down, up)
        });
        if space_down && self.space_down_at.is_none() {
            self.space_down_at = Some(std::time::Instant::now());
        }
        if space_up {
            self.on_space_released();
        }
        let k = ctx.input(|i| Keys {
            cut: i.key_pressed(egui::Key::S) && !i.modifiers.ctrl,
            del: i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            undo: i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
            redo: i.modifiers.ctrl
                && (i.modifiers.shift && i.key_pressed(egui::Key::Z)
                    || i.key_pressed(egui::Key::Y)),
            save: i.modifiers.ctrl && i.key_pressed(egui::Key::S),
            marker: i.key_pressed(egui::Key::M) && !i.modifiers.ctrl && !i.modifiers.alt,
            // TRACK (R.62): the bare M and S are already taken by the marker and the cut —
            // both were set by the owner’s direct request and do not move.
            // Mute and solo take ALT, and act on the track
            // of the SELECTED event (otherwise we have no “selected track”).
            track_mute: i.modifiers.alt && i.key_pressed(egui::Key::M),
            track_solo: i.modifiers.alt && i.key_pressed(egui::Key::S),
            region_in: i.key_pressed(egui::Key::I) && !i.modifiers.ctrl && !i.modifiers.alt,
            region_out: i.key_pressed(egui::Key::O) && !i.modifiers.ctrl && !i.modifiers.alt,
            loop_toggle: i.key_pressed(egui::Key::L) && !i.modifiers.ctrl && !i.modifiers.alt,
            // Ctrl+C/Ctrl+V do NOT arrive as key_pressed: egui-winit
            // intercepts them and sends Event::Copy / Event::Paste(..)
            // (regression of item 21 — “Ctrl+V does not work”)
            esc: i.key_pressed(egui::Key::Escape),
            enter_nest: i.key_pressed(egui::Key::Enter) && !i.modifiers.any(),
            copy: i.events.iter().any(|e| matches!(e, egui::Event::Copy))
                || (i.modifiers.ctrl && i.key_pressed(egui::Key::C)),
            paste: i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
                || (i.modifiers.ctrl && i.key_pressed(egui::Key::V)),
            // item 16 of round 8: Home/End — to the stop points; with Shift — to the
            // very start/end of the project
            // item 8 of round 10: A/D — the same jumps as the navigation buttons.
            // “To the very end” remains only on Shift+Home/End: Shift+D
            // must not suddenly fly to the end of the project.
            nav_prev: i.key_pressed(egui::Key::Home)
                || (i.key_pressed(egui::Key::A) && !i.modifiers.ctrl && !i.modifiers.alt),
            nav_next: i.key_pressed(egui::Key::End)
                || (i.key_pressed(egui::Key::D) && !i.modifiers.ctrl && !i.modifiers.alt),
            goto_tc: i.modifiers.ctrl && i.key_pressed(egui::Key::L),
            find: i.modifiers.ctrl && i.key_pressed(egui::Key::F),
            nav_far: i.modifiers.shift
                && (i.key_pressed(egui::Key::Home) || i.key_pressed(egui::Key::End)),
            // item 2 of round 16: Shift+A/D — the jump DRAGS the region edge (Shift as
            // “extend the selection” in pro programs). Home/End has Shift
            // taken for “to the very start/end”, so we look at A/D.
            nav_region: i.modifiers.shift
                && !i.modifiers.ctrl
                && !i.modifiers.alt
                && (i.key_pressed(egui::Key::A) || i.key_pressed(egui::Key::D)),
        });
        // item 9 of round 11: it also works during playback (seek on the fly
        // the player can do; the “only on pause” gate was over-caution)
        if k.goto_tc {
            self.tc_edit = Some(String::new());
            self.tc_edit_focus = true;
        }
        if k.find {
            self.search_open = true;
            self.search_focus = true;
        }
        if k.nav_prev {
            self.nav_jump(false, k.nav_far, k.nav_region);
        }
        if k.nav_next {
            self.nav_jump(true, k.nav_far, k.nav_region);
        }
        if k.cut {
            self.cut_action();
        }
        if k.del {
            self.delete_action();
        }
        if k.undo {
            self.undo();
        }
        if k.redo {
            self.redo();
        }
        if k.save {
            self.save();
        }
        if k.marker {
            self.marker_toggle_at_cursor();
        }
        if k.track_mute
            && let Some(id) = self.key_target_track()
        {
            self.toggle_track_badge(id, crate::timeline::TrackBadge::Mute);
        }
        if k.track_solo
            && let Some(id) = self.key_target_track()
        {
            self.toggle_track_badge(id, crate::timeline::TrackBadge::Solo);
        }
        if k.region_in {
            self.set_region_edge_from_cursor(true);
        }
        if k.region_out {
            self.set_region_edge_from_cursor(false);
        }
        if k.loop_toggle {
            self.toggle_loop();
        }
        if k.esc {
            self.escape_action();
        }
        // Enter — a CASCADE on the selected nested item (R.56, item 451): the first
        // unfolds in place (the default mode, the keyboard twin
        // of a double click), the second on an already unfolded one — FULL ENTRY.
        // The quick gesture leads DEEPER, not a toggle: back — Esc,
        // which already cascades in the reverse direction anyway.
        if k.enter_nest
            && let Some(id) = self.selected.iter().copied().find(|id| self.is_nest(*id))
        {
            match self.enter_pressed_on_nest(id) {
                EnterOnNest::Expand => self.expand_in_place(id),
                EnterOnNest::Enter => self.enter_nest(id),
                EnterOnNest::NotANest => {}
            }
        }
        if k.copy {
            self.copy_selection();
        }
        if k.paste {
            self.paste_at_cursor();
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        // during DnD-hover winit/X11 does not send mouse movement (the coordinates
        // of XdndPosition are discarded in winit) — we read the pointer directly;
        // on Wayland it returns None → an honest fallback to the playhead
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if hovering && let Some(p) = dnd_pointer_pos(ctx) {
            self.dnd_hover = Some(p);
        }
        if hovering {
            ctx.request_repaint(); // mouse movement during DnD is not sent — polling
        }
        // The position lives WHILE the hover goes on, and is taken ONLY on drop
        // (dnd_keep_hover, test 47): the earlier take() on every frame erased the
        // position in the very frame where it was set — by the drop it was always None
        // (the drop fell to the playhead) and the silhouette was not drawn at all.
        if !dnd_keep_hover(hovering, !dropped.is_empty()) {
            self.dnd_hover = None;
        }
        // the drop position is needed both by the morpher’s source field and by the timeline:
        // we take it BEFORE the target computation on the track takes it
        let drop_pos = if dropped.is_empty() { None } else { self.dnd_hover };
        let target = if dropped.is_empty() {
            None
        } else {
            self.dnd_hover.take().and_then(|pos| {
                let g = self.tl_geom.clone()?;
                if !g.in_lanes(pos) {
                    return None;
                }
                let lane = g.lane_at_y(pos.y)?;
                let at = self.snap(g.x_to_tick(pos.x).max(0), None);
                Some((Tick(at), lane))
            })
        };
        for path in dropped {
            self.route_dropped(path, target, drop_pos);
        }
    }

    /// Where a DROPPED file leads (R.70): a project file — the question “open or
    /// insert”, an exchange file and legacy — a hint about the import door (each
    /// with its own text), the rest — material for the edit. Separate from reading
    /// the input, so that the three outcomes are checked without a real drop (t821).
    pub(crate) fn route_dropped(
        &mut self,
        path: PathBuf,
        target: Option<(Tick, (ClipKind, u32))>,
        drop_pos: Option<egui::Pos2>,
    ) {
        {
            let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
            if ext.as_deref() == Some("ktproj") {
                // R.24 item 12: .ktproj does not open as a project — the import door
                self.note(crate::tf!(k::msg::KTPROJ_IMPORT_ONLY, tr(k::menu::file::IMPORT_OLDER)));
            } else if ext.as_deref() == Some("otio") {
                // R.70: an exchange file is not a project; its door is import, and
                // the notification names it by the menu item name (rule 19)
                self.note(crate::tf!(
                    k::msg::OTIO_IMPORT_ONLY,
                    path.display().to_string(),
                    tr(k::menu::file::IMPORT_OLDER)
                ));
            } else if ext.as_deref() == Some(kt_model::project_file::EXTENSION) {
                // A PROJECT WAS DROPPED — WE ASK (R.55, C6). Before this round
                // here stood `*self = app`: no choice of “open or
                // insert”, no question about unsaved work, no lock
                // on the file. The work disappeared silently — bypassing everything
                // built in R.49.
                self.pending_drop = Some((path.clone(), target));
                self.ask = Some(Ask::DropProject);
            } else if let Some(id) = self.morph_editors.iter().find_map(|e| {
                let r = e.src_rect?;
                let pos = drop_pos?;
                (e.open && r.contains(pos)).then_some(e.id)
            }) {
                // the file was dropped ON THE SOURCE FIELD of an open morpher (R.33
                // item 2): it becomes the new source, not a new
                // clip on the timeline
                self.set_morph_source_file(id, &path.to_string_lossy());
            } else if let Some((at, lane)) = target {
                self.add_media_clip_at(&path.to_string_lossy(), at, Some(lane));
            } else {
                self.add_media_clip(&path.to_string_lossy());
            }
        }
    }
}
/// The action of the Space transport machine (item 15 of round 6, test 58).
#[derive(Debug, PartialEq, Eq)]
pub enum TransportAction {
    Pause,
    Resume { seek: Option<i64> },
    SeekStart(i64),
}

/// The “double Space” window = stop (ms).
/// The autosave interval (item 4 of round 11): it was 4 s — a constant write of the whole
/// project on the UI thread at any sneeze, like moving a region.
pub const AUTOSAVE_SECS: u64 = 30;
pub const STATUS_TTL_MS: u64 = 6000;
pub const DOUBLE_PAUSE_MS: u128 = 400;
/// How long to hold space before it becomes “acceleration”, not
/// play/pause (item 3 of round 16). Noticeably less than the double-press window, so
/// “stop at the region start” with two quick presses does not suffer.
pub const SPACE_HOLD_MS: u128 = 220;
/// The acceleration ceiling by holding (the user’s answer of round 16: “up to 4×”).
pub const SPACE_BOOST_MAX: f32 = 4.0;
/// How many seconds of holding it takes acceleration to reach the ceiling. 5 s by
/// the direct request of item 3 of round 17 (“it must be 5, not 2, otherwise it is very fast”) —
/// do NOT speed it up without a new request.
pub const SPACE_BOOST_SECS: f32 = 5.0;

/// State of the perf panel: rolling graphs, counter deltas, the problem log.
/// The coverage-poll cache of the timeline (item 1).
#[derive(Default)]
pub struct CoverageCache {
    stamp: Option<(u64, u64, (u32, u32))>,
    /// Arc: a snapshot per frame — a clone of the pointer, not of the maps (item 1 of v1.4).
    pub per_clip: Arc<HashMap<ClipId, Vec<std::ops::Range<i64>>>>,
    /// Throttling during playback (item 1 of round 5): the cache epoch changes on EVERY
    /// insert → recalculation ran 60 times/s with the cache mutex (contention with
    /// the decoders and the audio render). The strips are refreshed no more often than 4 Hz.
    last_refresh: Option<std::time::Instant>,
}

/// Clip-name cache (item 1: the HashMaps and the O(n²) suppress were rebuilt every frame).
/// Time stretches of clips, computed once per model version.
#[derive(Default)]
pub struct WarpCache {
    /// The key — (model version, number of clips): the model can be swapped
    /// WHOLESALE without bumping `model_rev` (opening a project, harness tests),
    /// and then the second number saves — the same safeguard as for `names_cache`.
    rev: Option<(u64, usize)>,
    /// Only clips WITH a warp: for the rest the mapping is linear, and there is
    /// nothing to store about them.
    maps: Arc<HashMap<ClipId, kt_engine::ClipWarp>>,
}

/// The media caption for the user: for a file — the file name, for a generator —
/// the KIND NAME (R.53, item 416). Until this item the clip-generator was captioned
/// with the raw path `gen:solid:c=ff8800`, which moreover changed on every
/// keypress in the parameters window: the caption jerked under one’s hands. There is one place —
/// the bin card calls the same one.
pub fn media_display_name(path: &str) -> String {
    if let Some(kd) = kt_model::parse_gen_path(path) {
        return crate::gen_ui::kind_name(&kd).to_string();
    }
    std::path::Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

#[derive(Default)]
pub struct NamesCache {
    rev: Option<u64>,
    /// The language in which the names were computed (R.53, item 416). The caption
    /// of a generator clip is the KIND NAME, that is, a catalogue string; a change of language
    /// does not touch the model, and without this stamp the timeline would stay in the old
    /// language until the first edit of the montage.
    lang: Option<crate::i18n::Lang>,
    /// The clip drawing order (indices into `project().clips`) — the same
    /// back-to-front as the engine’s composition: the rear track first, and
    /// inside a track the one that started earlier first, so that a later one lands ON TOP
    /// (Vegas rule, item 177). It is computed once per model version: per frame
    /// sorting thousands of clips is not allowed (item 1 of v1.4).
    pub draw_order: Arc<Vec<usize>>,
    /// Arc: snapshots per frame — a clone of the pointer, not of structures (item 1 of v1.4).
    pub names: Arc<HashMap<ClipId, String>>,
    pub suppress: Arc<std::collections::HashSet<ClipId>>,
    pub media_paths: Arc<HashMap<MediaId, String>>,
    /// Zones of BARE overlaps (R.51, item 399): overlaps of pairs of the same
    /// (kind, lane) that have no transition. The strip marks them with hatching —
    /// the target of the RMB gesture “Transition from…” must be visible (rule 16;
    /// the minus of “overlapped and did not notice” from F-TL-010).
    pub overlaps: Arc<Vec<(ClipKind, u32, i64, i64)>>,
}

impl KtApp {
    /// Time-stretch maps of clips (R.37 item 3): the preview computes the position
    /// of the material with THE SAME map as the render. Recalculation — once per model
    /// version; a clone of the pointer is handed out per frame.
    pub fn warp_maps(&mut self) -> Arc<HashMap<ClipId, kt_engine::ClipWarp>> {
        let stamp = (self.model_rev, self.model.project().clips.len());
        if self.warp_cache.rev != Some(stamp) {
            self.warp_cache.rev = Some(stamp);
            let maps: HashMap<ClipId, kt_engine::ClipWarp> = self
                .model
                .project()
                .clips
                .iter()
                .filter_map(|c| kt_engine::clip_warp_map(c).map(|w| (c.id, w)))
                .collect();
            self.warp_cache.maps = Arc::new(maps);
        }
        self.warp_cache.maps.clone()
    }

    /// Preview keys per INSTANCE: `ClipId` → (reel, revision)
    /// (R.56 item 452, rewritten in R.59 stage 3 — t680).
    ///
    /// The reel’s revision has become a function of the PAIR (reel, the set of
    /// instance overrides), so the preview key is also per instance: two releases with
    /// different titles must show THEIR OWN frames on the timeline, not the frames
    /// of the template. For an instance without overrides
    /// `instance_content` EQUALS `content_hash`, so earlier keys, the disk tier and
    /// all projects without publications do not move by a single bit.
    ///
    /// Computed by `model_rev`, like the warp maps: the hash walks all the reel’s
    /// content, and there is no point doing that on every UI frame.
    pub fn reel_keys(&mut self) -> Arc<HashMap<ClipId, crate::reel_preview::ReelKey>> {
        if self.reel_hash_cache.0 != Some(self.model_rev) {
            let doc = self.model.document();
            let mut map: HashMap<ClipId, crate::reel_preview::ReelKey> = HashMap::new();
            // A SINGLE reel now has SEVERAL live revisions at once, and
            // invalidation must keep them all: if it threw out “someone else’s”,
            // two instances would recalculate each other in a circle.
            let mut live: HashMap<MediaId, std::collections::HashSet<u64>> = HashMap::new();
            for s in &doc.sequences {
                live.entry(s.id).or_default();
            }
            for c in &self.model.project().clips {
                if !live.contains_key(&c.media) {
                    continue;
                }
                let hash = kt_engine::instance_content(&doc, c.media, &c.overrides);
                map.insert(c.id, crate::reel_preview::ReelKey { seq: c.media, hash });
                live.entry(c.media).or_default().insert(hash);
            }
            for (id, hs) in &live {
                self.reel_previews.invalidate_seq(*id, hs);
            }
            self.reel_hash_cache = (Some(self.model_rev), Arc::new(map));
        }
        self.reel_hash_cache.1.clone()
    }

    /// The preview request for a reel — the ONLY entry (called from drawing).
    ///
    /// The key does not depend on zoom or scrolling, so scrolling and zooming
    /// do not generate requests (invariant 8): a repeated request for the same
    /// key is discarded by dedup.
    pub fn request_reel_preview(
        &mut self,
        key: crate::reel_preview::ReelKey,
        overrides: &[kt_model::Override],
    ) {
        if self.reel_previews.get(&key).is_some() {
            return;
        }
        let Some(mut p) = self.model.document().reel_as_root(key.seq) else { return };
        // Overrides are BAKED into the document that the preview counts:
        // the key is already per instance, and if we counted the template, the strip
        // would show someone else’s title under its own key (t680).
        // The rule is one for the program: the set lands BEFORE the morpher layers, by
        // the same `apply_overrides` that `resolve_reel` calls.
        if !overrides.is_empty() {
            // `swap_reel` has already put the reel’s set into the root’s `published`
            let published = p.published.clone();
            let frame_step = 90_000 / p.settings.fps.max(1) as i64;
            let mut clips = std::mem::take(&mut p.clips);
            let mut trans = std::mem::take(&mut p.transitions);
            let mut cams = std::mem::take(&mut p.cameras);
            kt_model::apply_overrides(
                &published,
                overrides,
                &mut clips,
                &mut trans,
                &mut cams,
                frame_step,
                &|m| p.default_gen_for(m),
            );
            p.clips = clips;
            p.transitions = trans;
            p.cameras = cams;
        }
        let base = self.project_path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf());
        let db = self.index_db_path();
        self.reel_previews.request(key, p, db, base);
    }

    /// Runs of coverage per clip; recalculation only when the cache epoch,
    /// the model version or the preview quality changes.
    pub fn coverage_runs(&mut self) -> Arc<HashMap<ClipId, Vec<std::ops::Range<i64>>>> {
        use kt_time::AUDIO_BURST_FRAMES;
        let q = self.player.preview_quality();
        let stamp = (self.player.cache.epoch(), self.model_rev, (q.width, q.height));
        let throttled = self.player.transport.is_playing()
            && self
                .coverage_cache
                .last_refresh
                .map(|t| t.elapsed().as_millis() < 250)
                .unwrap_or(false);
        if self.coverage_cache.stamp != Some(stamp) && !throttled {
            self.coverage_cache.stamp = Some(stamp);
            self.coverage_cache.last_refresh = Some(std::time::Instant::now());
            // The clip DECODE quality (R.46, item 322): a clip with a zooming chain
            // warms up larger than Quality, and coverage by Quality would show a
            // warm clip as cold. A morpher layer could cut a clip into
            // pieces with different chains — we take the union of their qualities.
            let decode_q = self.player.video_decode_qualities();
            // A nested item: the source reel has none, and the strip asks about its
            // CONTENT — the intersection of the children’s warmth (R.50, B6).
            let nest_cov: HashMap<ClipId, Vec<std::ops::Range<i64>>> =
                self.player.nest_coverage().into_iter().collect();
            let mut per_clip = HashMap::new();
            for c in &self.model.project().clips {
                // the ADDRESS of the event’s SOURCE (R.59, stage 2): for a generator the values
                // live on the event, and its own source is what warms up.
                // One rule for the program — the request, the read and the strip must
                // all ask `clip_source_addr` (invariant 4).
                let src = kt_engine::clip_source_addr(c);
                let runs = match c.kind {
                    ClipKind::Audio => {
                        let f0 = c.source_in.to_audio_frames();
                        let f1 = (c.source_in + c.duration).to_audio_frames();
                        self.player.cache.coverage(
                            kt_engine::source_id(src),
                            kt_cache::BurstKind::Audio,
                            f0.div_euclid(AUDIO_BURST_FRAMES as i64)
                                ..((f1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1),
                        )
                    }
                    ClipKind::Video if nest_cov.contains_key(&c.id) => {
                        nest_cov.get(&c.id).cloned().unwrap_or_default()
                    }
                    ClipKind::Video => {
                        let (b0, b1) = (c.source_in.0, c.source_in.0 + c.duration.0);
                        let mut qs: Vec<kt_media::Quality> = decode_q
                            .iter()
                            .filter(|(id, _)| kt_model::same_clip(*id, c.id))
                            .map(|(_, dq)| *dq)
                            .collect();
                        qs.sort();
                        qs.dedup();
                        if qs.is_empty() {
                            qs.push(q);
                        }
                        let mut runs: Vec<std::ops::Range<i64>> = Vec::new();
                        for dq in qs {
                            runs.extend(self.player.cache.coverage(
                                kt_engine::source_id(src),
                                kt_cache::BurstKind::video(Some(dq)),
                                b0..b1.max(b0 + 1),
                            ));
                        }
                        merge_runs(runs)
                    }
                };
                per_clip.insert(c.id, runs);
            }
            self.coverage_cache.per_clip = Arc::new(per_clip);
        }
        self.coverage_cache.per_clip.clone()
    }

    /// Clip names and the “suppress name” set (the pair partner) by model_rev.
    #[allow(clippy::type_complexity)]
    pub fn names(
        &mut self,
    ) -> (
        Arc<HashMap<ClipId, String>>,
        Arc<std::collections::HashSet<ClipId>>,
        Arc<HashMap<MediaId, String>>,
        Arc<Vec<usize>>,
    ) {
        // The length check is a safeguard — the drawing order now AFFECTS THE HIT TEST
        // (item 177), and a stale cache would mean “no clips”.
        // The model can be swapped wholesale without bumping model_rev (opening a
        // project, harness tests) — then this check saves.
        if self.names_cache.rev != Some(self.model_rev)
            || self.names_cache.lang != Some(crate::i18n::current())
            || self.names_cache.draw_order.len() != self.model.project().clips.len()
        {
            self.names_cache.rev = Some(self.model_rev);
            self.names_cache.lang = Some(crate::i18n::current());
            let mut names = HashMap::new();
            let mut suppress = std::collections::HashSet::new();
            let p = self.model.project();
            let media_names: HashMap<MediaId, String> =
                p.media.iter().map(|m| (m.id, media_display_name(&m.path))).collect();
            for c in &p.clips {
                if let Some(n) = media_names.get(&c.media) {
                    names.insert(c.id, n.clone());
                }
                // suppress the name of the AUDIO half of an A/V pair (drawn on the video)
                if c.kind == ClipKind::Audio
                    && let Some(g) = c.link_group
                    && p.clips.iter().any(|o| {
                        o.kind == ClipKind::Video && o.link_group == Some(g) && o.media == c.media
                    })
                {
                    suppress.insert(c.id);
                }
            }
            self.names_cache.names = Arc::new(names);
            self.names_cache.suppress = Arc::new(suppress);
            self.names_cache.media_paths =
                Arc::new(p.media.iter().map(|m| (m.id, m.path.clone())).collect());
            let mut order: Vec<usize> = (0..p.clips.len()).collect();
            order.sort_by_key(|&i| {
                let c = &p.clips[i];
                (std::cmp::Reverse(c.lane), c.start.0)
            });
            self.names_cache.draw_order = Arc::new(order);
            // bare overlaps: sorting by track + scanning neighbours,
            // O(n log n + intersections) — once per model version
            let mut sorted: Vec<&ClipEvent> = p.clips.iter().collect();
            sorted.sort_by_key(|c| (c.kind as u8, c.lane, c.start.0));
            let mut ov: Vec<(ClipKind, u32, i64, i64)> = Vec::new();
            for (i, a) in sorted.iter().enumerate() {
                let a_end = a.start.0 + a.duration.0;
                for b in sorted[i + 1..]
                    .iter()
                    .take_while(|b| b.kind == a.kind && b.lane == a.lane && b.start.0 < a_end)
                {
                    let covered = p.transitions.iter().any(|t| {
                        (t.a, t.b) == (Some(a.id), Some(b.id))
                            || (t.a, t.b) == (Some(b.id), Some(a.id))
                    });
                    if !covered {
                        ov.push((a.kind, a.lane, b.start.0, a_end.min(b.start.0 + b.duration.0)));
                    }
                }
            }
            self.names_cache.overlaps = Arc::new(ov);
        }
        (
            self.names_cache.names.clone(),
            self.names_cache.suppress.clone(),
            self.names_cache.media_paths.clone(),
            self.names_cache.draw_order.clone(),
        )
    }

    /// How many TIMES each media stands on the edit (R.53, item 416). It is computed
    /// once per model revision: the bin has dozens of cards, the edit has events by
    /// the thousands, and polling `Project::sequence_uses` per card per frame is
    /// a linear walk of the whole document multiplied by the number of cards
    /// (invariant 12).
    pub fn media_uses(&mut self) -> Arc<HashMap<MediaId, usize>> {
        if self.uses_cache.0 != Some(self.model_rev) {
            let p = self.model.project();
            let mut m: HashMap<MediaId, usize> = HashMap::new();
            for c in p.clips.iter().chain(p.sequences.iter().flat_map(|s| s.clips.iter())) {
                *m.entry(c.media).or_default() += 1;
            }
            self.uses_cache = (Some(self.model_rev), Arc::new(m));
        }
        self.uses_cache.1.clone()
    }

    /// Zones of bare overlaps (R.51). Freshness is ensured by `names()` —
    /// call it after that in the same frame.
    pub fn overlap_zones(&self) -> Arc<Vec<(ClipKind, u32, i64, i64)>> {
        self.names_cache.overlaps.clone()
    }
}

/// The delay before the next redraw: 60 fps only when there is something to
/// animate, at rest — a rare tick (status bar/watchdog).
/// A pure function — test 20-repaint (item 1).
pub fn repaint_delay(animating: bool) -> std::time::Duration {
    if animating {
        std::time::Duration::from_millis(16)
    } else {
        std::time::Duration::from_millis(250)
    }
}

/// The mouse position in window coordinates (points) during a DnD of a file.
/// winit 0.30/X11 discards the XdndPosition coordinates and does not send mouse
/// movement while dragging a file (item 2 of v1.4) — we poll the X server directly
/// (x11rb). On Wayland/without X11 → None (fallback: drop on the playhead).
#[cfg(target_os = "linux")]
fn dnd_pointer_pos(ctx: &egui::Context) -> Option<egui::Pos2> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;
    static X11: std::sync::OnceLock<Option<(x11rb::rust_connection::RustConnection, u32)>> =
        std::sync::OnceLock::new();
    let conn = X11
        .get_or_init(|| {
            x11rb::connect(None).ok().map(|(c, screen)| {
                let root = c.setup().roots[screen].root;
                (c, root)
            })
        })
        .as_ref()?;
    let reply = conn.0.query_pointer(conn.1).ok()?.reply().ok()?;
    let inner = ctx.input(|i| i.viewport().inner_rect)?;
    let ppp = ctx.pixels_per_point();
    let p = egui::pos2(
        reply.root_x as f32 / ppp - inner.min.x,
        reply.root_y as f32 / ppp - inner.min.y,
    );
    (p.x >= 0.0 && p.y >= 0.0).then_some(p)
}

/// Windows (windows-build): winit does not report the pointer during an OLE drag either —
/// the same reading of the pointer in screen pixels, from `GetCursorPos`. Before, every drop
/// landed at the playhead and the silhouette did not follow the pointer.
#[cfg(windows)]
fn dnd_pointer_pos(ctx: &egui::Context) -> Option<egui::Pos2> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut pt = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut pt) } == 0 {
        return None;
    }
    let inner = ctx.input(|i| i.viewport().inner_rect)?;
    let ppp = ctx.pixels_per_point();
    let p = egui::pos2(pt.x as f32 / ppp - inner.min.x, pt.y as f32 / ppp - inner.min.y);
    (p.x >= 0.0 && p.y >= 0.0).then_some(p)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn dnd_pointer_pos(_ctx: &egui::Context) -> Option<egui::Pos2> {
    None
}

/// The life of the dnd position (item 2 of v1.4, test 47): it is held while there is a hover
/// AND on the drop frame (hovered_files is already empty for that frame); it goes out
/// only when there is neither.
pub fn dnd_keep_hover(hovering: bool, dropping: bool) -> bool {
    hovering || dropping
}

/// The resize zone of a borderless window by position at the edge (item 22 of round 6, test 64):
/// corners take priority over edges, centre → None.
/// The window resize zone: TWO radii (items 11–13 of round 8) — a thin edge and
/// a noticeably larger corner, otherwise corners cannot be hit with a mouse.
pub fn resize_zone(
    win: egui::Rect,
    pos: egui::Pos2,
    edge: f32,
    corner: f32,
) -> Option<egui::ResizeDirection> {
    use egui::ResizeDirection::*;
    if !win.contains(pos) {
        return None;
    }
    let (dl, dr) = (pos.x - win.left(), win.right() - pos.x);
    let (dt, db) = (pos.y - win.top(), win.bottom() - pos.y);
    // corners take priority over edges and are “caught” at a greater distance
    let (cl, cr, ct, cb) = (dl <= corner, dr <= corner, dt <= corner, db <= corner);
    if cl && ct {
        return Some(NorthWest);
    }
    if cr && ct {
        return Some(NorthEast);
    }
    if cl && cb {
        return Some(SouthWest);
    }
    if cr && cb {
        return Some(SouthEast);
    }
    Some(match (dl <= edge, dr <= edge, dt <= edge, db <= edge) {
        (true, ..) => West,
        (_, true, ..) => East,
        (_, _, true, _) => North,
        (_, _, _, true) => South,
        _ => return None,
    })
}

/// Cursor for the resize direction.
pub fn resize_cursor(dir: egui::ResizeDirection) -> egui::CursorIcon {
    use egui::ResizeDirection::*;
    match dir {
        North | South => egui::CursorIcon::ResizeVertical,
        East | West => egui::CursorIcon::ResizeHorizontal,
        NorthEast | SouthWest => egui::CursorIcon::ResizeNeSw,
        NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
    }
}

/// Width of the “grabbable” window edge and the corner radius (item 12 of round 8: 4 px required
/// pixel-precise aiming).
/// Height of the invisible window-drag strip along the top edge
/// (item 2 of round 9). It takes no space — simply the first pixels of the central zone.
pub const TOP_DRAG_H: f32 = 20.0;
/// The very edge of the window: here resize wins even over buttons, so that the corner
/// of the window stays “pullable” (item 9 of round 12).
/// The meter panel: width limits and its own drag strip to the LEFT of the
/// panel (item 6 of round 13). The maximum was set at the request of item 2 of round 12 — do not change
/// without a new user request.
pub const METERS_W_MIN: f32 = 64.0;
pub const METERS_W_MAX: f32 = 90.0;
pub const METERS_SEP_W: f32 = 10.0;
pub const WIN_BORDER: f32 = 4.0;
pub const WIN_EDGE: f32 = 8.0;
pub const WIN_CORNER: f32 = 16.0;

/// State machine of the preview detach (item 7 of v1.4, test 51): closing the window always
/// returns the preview; otherwise the button toggles.
/// What to show at the bottom right of the status line (item 14 of round 8). Priority:
/// hover → active mode → temporary message → routine line.
/// All in one place: no pop-ups, no overlays on top of the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintKind {
    /// Action impossible: red, overrides everything (item 5 of R.28).
    Error,
    Hover,
    Mode,
    Status,
    Idle,
}

/// Write a hint to the frame accumulator where `&mut self` is not available
/// (inside the ui closures of panels). It does NOT create a pop-up (item 14 of round 8).
pub fn hint_into(out: &mut Option<String>, r: egui::Response, text: &str) -> egui::Response {
    if r.hovered() {
        *out = Some(text.to_string());
    }
    r
}

pub fn status_line(
    error: Option<&str>,
    hover: Option<&str>,
    mode: Option<&str>,
    status: Option<(&str, u64)>, // (text, age in ms)
    status_ttl_ms: u64,
    idle: &str,
) -> (String, HintKind) {
    // “everywhere in the hints such situations are made as errors” (item 5):
    // the impossibility of an action outweighs any hover and any status
    if let Some(e) = error {
        return (e.to_string(), HintKind::Error);
    }
    if let Some(h) = hover {
        return (h.to_string(), HintKind::Hover);
    }
    if let Some(m) = mode {
        return (m.to_string(), HintKind::Mode);
    }
    if let Some((s, age)) = status
        && age < status_ttl_ms
    {
        return (s.to_string(), HintKind::Status);
    }
    (idle.to_string(), HintKind::Idle)
}

/// The deferred single click on the preview (items 10–11 of round 7): a single =
/// play/pause (waits for the double-click window), a double = detach/attach.
/// A pure machine — test 74.
#[derive(Debug, PartialEq)]
pub enum PreviewClick {
    None,
    TogglePlay,
    ToggleDetach,
}

#[derive(Debug, PartialEq)]
pub enum PendingOp {
    Keep,
    Arm,
    Clear,
}

pub const PREVIEW_CLICK_DELAY_MS: u128 = 300;

pub fn preview_click_step(
    pending_ms: Option<u128>,
    clicked: bool,
    double_clicked: bool,
    delay_ms: u128,
) -> (PreviewClick, PendingOp) {
    if double_clicked {
        // a double cancels the deferred single (play does not jerk)
        return (PreviewClick::ToggleDetach, PendingOp::Clear);
    }
    if clicked {
        return (PreviewClick::None, PendingOp::Arm);
    }
    match pending_ms {
        Some(ms) if ms >= delay_ms => (PreviewClick::TogglePlay, PendingOp::Clear),
        _ => (PreviewClick::None, PendingOp::Keep),
    }
}

pub fn preview_detached_next(detached: bool, toggle: bool, close_requested: bool) -> bool {
    if close_requested {
        false
    } else if toggle {
        !detached
    } else {
        detached
    }
}

// ---- transport speed scale (R.57, item 455) ----------------------------
// THREE different “0.25”s lived in the program, and the coincidence of numbers was taken for a rule:
// the edge of the slider’s dead zone, `RATE_MIN` right there, and `RATE_CLAMP` of the warp map.
// The owner has no decision beyond 0.25 — there is only the ENGINE’s limit
// (|rate| >= 0.001 in callback_step). So the scale goes down to 0.01, and exactly
// zero is given only by a narrow zone of 1.5 px around the centre.
/// The share of the slider stroke around the centre that gives EXACTLY 0 (a still frame).
/// 0.018 · 85 px ≈ 1.5 px — the “narrow zone” from the owner’s decision.
pub const RATE_STOP: f32 = 0.018;
/// The smallest representable speed: below it the sound no longer holds
/// (`kt_model::warp::RATE_AUDIO_FLOOR`), and the scale has no reason to go further.
pub const RATE_FLOOR: f32 = 0.01;
/// The knee: up to it the stroke is LINEAR in speed (otherwise 0.01…0.25 would be squeezed
/// into a pixel), after it — logarithmic in magnitude.
pub const RATE_KNEE: f32 = 0.25;
/// The share of the stroke up to the knee.
pub const RATE_KNEE_T: f32 = 0.30;
/// |rate| at the edge of the slider (beyond it — only the “second wind” overshoot).
pub const RATE_MAX: f32 = 2.0;
/// How many times Shift slows the movement when dragging the strip.
pub const SPEED_FINE_DIV: f32 = 8.0;

/// The colour of “cannot be done” (rule 16): the status line and the underline of the
/// input field with an unreadable value must speak of the error IDENTICALLY.
pub const HINT_ERROR: egui::Color32 = egui::Color32::from_rgb(255, 105, 97);

/// Speed input as a number (R.57). It lives IN THE SAME 170×20 rectangle as
/// the strip — the widget’s height was set by the owner’s request (item 18 of round 7) and
/// does not change; the input has no window of its own (rule 3 “nothing jumps”).
#[derive(Debug, Clone, Default)]
pub struct SpeedEntry {
    /// What is typed. Empty — we show the current speed muted:
    /// the first digit replaces it entirely (the same as “select all”).
    pub text: String,
    /// Take the focus on the nearest frame.
    pub focus: bool,
    /// Typed text is unreadable — underline in the error style
    /// (rule 16: inability is shown as an ERROR), the field is left open.
    pub bad: bool,
}

/// Window icon (item 6, round 9): without it the taskbar showed a placeholder. The raster —
/// the same silhouette that is drawn in the header and lies in packaging/.
fn app_icon() -> egui::IconData {
    let img = image::load_from_memory_with_format(
        include_bytes!("../assets/kinetren-256.png"),
        image::ImageFormat::Png,
    )
    .expect("application icon")
    .to_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData { rgba: img.into_raw(), width, height }
}

/// What we found by project search (item 3, round 10).
#[derive(Debug, Clone, PartialEq)]
pub enum HitKind {
    Marker,
    Media,
    Track,
    Clip,
    /// Group of bin cards (item 13, round 11).
    Group,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub kind: HitKind,
    pub label: String,
    /// Where to go (ticks); for media — 0.
    pub at: i64,
    pub media: Option<kt_model::MediaId>,
    pub clip: Option<kt_model::ClipId>,
}

/// Search by markers, media, tracks and clips. A pure function — exactly like
/// the FX registry filter: tested without UI. Empty query — empty result.
pub fn search_hits(
    p: &kt_model::Project,
    // groups live in the app and get into the project only on
    // save — search must see the LIVE ones (item 13, round 11)
    groups: &[kt_model::BinGroup],
    query: &str,
) -> Vec<Hit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(u8, Hit)> = Vec::new();
    // rank 0 — exact match, 1 — starts with the query, 2 — substring
    let rank = |s: &str| -> Option<u8> {
        let l = s.to_lowercase();
        if l == q {
            Some(0)
        } else if l.starts_with(&q) {
            Some(1)
        } else if l.contains(&q) {
            Some(2)
        } else {
            None
        }
    };
    for m in &p.markers {
        let label = if m.name.is_empty() {
            crate::tf!(k::search::MARKER_AT, format!("{:.2}", m.t.as_secs_f64()))
        } else {
            m.name.clone()
        };
        if let Some(r) = rank(&label) {
            out.push((r, Hit { kind: HitKind::Marker, label, at: m.t.0, media: None, clip: None }));
        }
    }
    for g in groups {
        if g.name.is_empty() {
            continue;
        }
        if let Some(r) = rank(&g.name) {
            out.push((
                r,
                Hit {
                    kind: HitKind::Group,
                    label: format!("{} ({})", g.name, g.members.len()),
                    at: 0,
                    media: g.members.first().copied(),
                    clip: None,
                },
            ));
        }
    }
    for m in &p.media {
        let name = std::path::Path::new(&m.path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| m.path.clone());
        let r = rank(&name).or_else(|| rank(&m.path));
        if let Some(r) = r {
            out.push((
                r,
                Hit { kind: HitKind::Media, label: name, at: 0, media: Some(m.id), clip: None },
            ));
        }
    }
    let (v, a) =
        (p.lanes(kt_model::LaneKind::Video).max(1), p.lanes(kt_model::LaneKind::Audio).max(1));
    for (kind, count) in [
        (kt_model::LaneKind::Video, v),
        (kt_model::LaneKind::Audio, a),
        (kt_model::LaneKind::Morph, p.lanes(kt_model::LaneKind::Morph)),
    ] {
        for lane in 0..count {
            let label = p.track_label(kind, lane);
            if let Some(r) = rank(&label) {
                let at =
                    p.clips.iter().filter(|c| c.lane == lane).map(|c| c.start.0).min().unwrap_or(0);
                out.push((r, Hit { kind: HitKind::Track, label, at, media: None, clip: None }));
            }
        }
    }
    for c in &p.clips {
        let name = p
            .media
            .iter()
            .find(|m| m.id == c.media)
            .map(|m| {
                std::path::Path::new(&m.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| m.path.clone())
            })
            .unwrap_or_default();
        if let Some(r) = rank(&name) {
            out.push((
                r + 1, // clips below media with the same name
                Hit {
                    kind: HitKind::Clip,
                    label: crate::tf!(
                        k::search::CLIP_AT,
                        name,
                        format!("{:.2}", c.start.as_secs_f64())
                    ),
                    at: c.start.0,
                    media: Some(c.media),
                    clip: Some(c.id),
                },
            ));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.at.cmp(&b.1.at)));
    out.into_iter().map(|(_, h)| h).collect()
}

/// Inline editing of a marker name (item 1, round 10).
#[derive(Debug, Clone)]
pub struct MarkerEdit {
    pub at: i64,
    pub buf: String,
    /// Marker just created: closing without Enter leaves it unnamed.
    pub is_new: bool,
}

/// Inline editing of a track name (item 2, round 10).
#[derive(Debug, Clone)]
pub struct LaneEdit {
    pub kind: kt_model::LaneKind,
    pub lane: u32,
    pub buf: String,
}

/// Renaming a named region (R.38 item 1). Separate from `LaneEdit`:
/// for a region the key is the NAME, and the old name must be remembered so the command knows,
/// exactly what is being renamed.
#[derive(Clone)]
pub struct AreaEdit {
    pub name: String,
    pub buf: String,
}

/// Stop points for navigation (item 16, round 8): zero, region bounds, all
/// markers, bounds of MERGED clips (where emptiness gives way to
/// material) and the end of the project. Sorted, no duplicates. Built
/// only on a keypress, not every frame.
pub fn stop_points(p: &kt_model::Project) -> Vec<i64> {
    let mut v: Vec<i64> = vec![0];
    if let Some(r) = p.region {
        v.push(r.start.0);
        v.push(r.end.0);
    }
    v.extend(p.markers.iter().map(|m| m.t.0));
    // end of a range marker is also a stop point (R.24)
    v.extend(p.markers.iter().filter(|m| m.is_range()).map(|m| m.end().0));
    // we merge overlapping clips: a boundary is a change of
    // “empty ↔ has material”, not every joint inside dense editing
    let mut spans: Vec<(i64, i64)> =
        p.clips.iter().map(|c| (c.start.0, (c.start + c.duration).0)).collect();
    spans.sort_unstable();
    let mut merged: Vec<(i64, i64)> = Vec::with_capacity(spans.len());
    for (s, e) in spans {
        match merged.last_mut() {
            Some((_, pe)) if s <= *pe => *pe = (*pe).max(e),
            _ => merged.push((s, e)),
        }
    }
    for (s, e) in merged {
        v.push(s);
        v.push(e);
    }
    // Points are aligned to the audio grid (item 7, round 10): the playhead is stored in
    // audio frames, and the round-trip “tick → frame → tick” truncates downward. Without
    // alignment the cursor after a jump landed on the tick LEFT of the target, and
    // “forward” kept finding the same point endlessly.
    let mut v: Vec<i64> = v.into_iter().map(|t| Tick(t).align_audio_up().0).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// The nearest stop point STRICTLY left of the cursor. The strict inequality
/// itself gives “pressing again — next point”, without storing
/// direction and without sticking on the current position.
pub fn prev_stop(cursor: i64, points: &[i64]) -> Option<i64> {
    points.iter().copied().filter(|&p| p < cursor).max()
}

/// The nearest stop point strictly right of the cursor.
pub fn next_stop(cursor: i64, points: &[i64]) -> Option<i64> {
    points.iter().copied().filter(|&p| p > cursor).min()
}

/// “Second wind” of the shuttle (item 4, round 8): if during a drag the mouse left
/// the widget's area, the speed keeps growing past the edge of the scale — up to ±8×. The widget
/// does not change visually. `over_px` — by how much the cursor went past the edge
/// (>=0), `k` — how many pixels give a doubling.
pub fn overshoot_rate(base: f32, over_px: f32, k: f32) -> f32 {
    if over_px <= 0.0 || base == 0.0 {
        return base;
    }
    let mult = 2f32.powf(over_px / k.max(1.0));
    let out = base.abs() * mult;
    out.min(ENGINE_RATE_MAX) * base.signum()
}

/// Engine speed ceiling (the ring and look-ahead are computed for it).
pub const ENGINE_RATE_MAX: f32 = kt_engine::player::ENGINE_RATE_MAX;
/// Pixels of departure from the widget per speed doubling.
pub const OVERSHOOT_PX_PER_DOUBLE: f32 = 60.0;

/// Slider [−1..1] → speed [−2..2] (R.57, test 46).
///
/// Linear `RATE_FLOOR → RATE_KNEE` over the first `RATE_KNEE_T` of travel, then
/// logarithm `RATE_KNEE → RATE_MAX`. Exactly 0 is produced only for `|t| < RATE_STOP`.
/// Before R.57 the edge of the 7 % dead zone immediately equalled 0.25×, and the interval
/// (0, 0.25) was unrepresentable BY CONSTRUCTION — the owner's complaint: “should
/// be able to set 0.05”. Symmetric for reverse.
pub fn slider_to_rate(t: f32) -> f32 {
    let a = t.abs();
    if a < RATE_STOP {
        return 0.0;
    }
    let r = if a < RATE_KNEE_T {
        RATE_FLOOR + (RATE_KNEE - RATE_FLOOR) * (a - RATE_STOP) / (RATE_KNEE_T - RATE_STOP)
    } else {
        let x = ((a - RATE_KNEE_T) / (1.0 - RATE_KNEE_T)).clamp(0.0, 1.0);
        RATE_KNEE * (RATE_MAX / RATE_KNEE).powf(x)
    };
    r.copysign(t)
}

/// Inverse mapping (test 46): rate → slider position. |rate| > 2
/// sits on the edge of the scale — the label stays visible (as before R.57).
pub fn rate_to_slider(rate: f32) -> f32 {
    let a = rate.abs();
    if a < RATE_FLOOR * 0.5 {
        return 0.0;
    }
    let t = if a < RATE_KNEE {
        RATE_STOP
            + (a.max(RATE_FLOOR) - RATE_FLOOR) / (RATE_KNEE - RATE_FLOOR)
                * (RATE_KNEE_T - RATE_STOP)
    } else {
        let x = (a.min(RATE_MAX) / RATE_KNEE).ln() / (RATE_MAX / RATE_KNEE).ln();
        RATE_KNEE_T + x * (1.0 - RATE_KNEE_T)
    };
    t.copysign(rate)
}

/// A typed (or code-supplied) speed value → what is allowed to reach the player:
/// near-zero becomes a STOP frame, the rest
/// is clamped to the scale floor and the engine ceiling, with sign.
pub fn normalize_rate(v: f32) -> f32 {
    if !v.is_finite() || v.abs() < 0.005 {
        return 0.0;
    }
    v.abs().clamp(RATE_FLOOR, ENGINE_RATE_MAX).copysign(v)
}

/// Parsing of a typed speed. Tolerates what a person actually types: a typographic minus (“−”),
/// a comma instead of a dot, a “×”/“x” suffix,
/// a leading “+”. Clamps nothing — that is done by `normalize_rate`.
pub fn parse_rate_text(s: &str) -> Option<f32> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '\u{2212}' | '\u{2013}' | '\u{2014}' => '-',
            ',' => '.',
            c => c,
        })
        .collect();
    let body = cleaned.trim_end_matches(['\u{00d7}', 'x', 'X']).trim_start_matches('+');
    if body.is_empty() {
        return None;
    }
    body.parse::<f32>().ok().filter(|v| v.is_finite())
}

/// Travel of the virtual cursor of the speed strip per drag frame.
///
/// The INCREMENT is what accumulates: dividing the absolute pointer position by
/// `SPEED_FINE_DIV` is not allowed — Shift pressed mid-drag would instantly
/// pull the value toward the center.
pub fn drag_advance(vx: f32, dx: f32, fine: bool) -> f32 {
    vx + if fine { dx / SPEED_FINE_DIV } else { dx }
}

/// Whether the speed accelerated by Space should be passed on to the player now. The hysteresis is
/// RELATIVE: the former absolute 0.02 at base 0.05 is 40 % of the base,
/// meaning the acceleration never reached the engine at all. Each `set_playback_rate`
/// resets the extrapolation window of the display clock (item 14, round 14), so the threshold
/// is still needed; a lower floor keeps it off zero at base 0.01.
pub fn boost_needs_update(want: f32, cur: f32, base: f32) -> bool {
    (want - cur).abs() > (base.abs() * 0.02).max(0.0005)
}

pub fn format_rate(r: f32) -> String {
    if r == 0.0 { tr(k::perf::RATE_STOP).to_string() } else { format!("{r:+.2}×") }
}

/// Key frame of the preview: a tick aligned to the project fps grid (test 39).
/// Preview size by fraction is SIMILAR to the project frame (R.64).
///
/// Independent even rounding of the axes was misleading: 1920×1080 at 1/8 gave
/// 240×134 (1.791), at 1/16 — 120×66 (1.818) instead of 1.778. “Fit”
/// computes the frame FROM THE CANVAS, so preview and export got different
/// frames, and the on-screen control would show something other than what goes into the file.
///
/// Evenness remains a decoder requirement (4:2:0), so exact similarity
/// does not exist at small fractions — we take the nearest EVEN PAIR from
/// the neighborhood of ±2 on each axis, minimizing the ratio error; on
/// a tie — the larger area.
pub fn preview_quality_for(w: u32, h: u32, f: f32) -> kt_media::Quality {
    let even = |v: f32| ((v.max(2.0) as u32).max(2)) & !1;
    let (w0, h0) = (even(w as f32 * f), even(h as f32 * f));
    let want = w.max(1) as f64 / h.max(1) as f64;
    let mut best = (w0, h0);
    let mut best_err = f64::MAX;
    for dw in [-2i64, 0, 2] {
        for dh in [-2i64, 0, 2] {
            let cw = (w0 as i64 + dw).max(2) as u32 & !1;
            let ch = (h0 as i64 + dh).max(2) as u32 & !1;
            let err = (cw as f64 / ch as f64 - want).abs();
            let better = err < best_err - 1e-12
                || ((err - best_err).abs() <= 1e-12
                    && (cw as u64 * ch as u64) > (best.0 as u64 * best.1 as u64));
            if better {
                best = (cw, ch);
                best_err = err;
            }
        }
    }
    kt_media::Quality { width: best.0, height: best.1 }
}

pub fn preview_key_tick(grid: kt_time::FpsGrid, t: Tick) -> i64 {
    grid.align_floor(t).0
}

pub struct PerfState {
    pub quality: crate::graphs::RollingGraph,
    pub decode: crate::graphs::RollingGraph,
    pub churn: crate::graphs::RollingGraph,
    last_sample: std::time::Instant,
    // previous cumulative values for deltas
    last_cache: kt_cache::CacheStats,
    last_decode: (u64, u64),
    last_io: (u64, u64),
    last_underruns: (u64, u64),
    frames: u32,
    /// Cumulative of actually shown NEW video frames (aligned key).
    pub video_frames_shown: u64,
    /// Smoothed V-fps of the last measurement window (item 23, round 7:
    /// shown in the timecode of the toolbar).
    pub vfps: f32,
    last_video_shown: u64,
    /// Skipped cells of the frame grid during playback (jitter diagnostics).
    pub video_cells_skipped: u64,
    /// Backward rollbacks of the display key (item 14, round 14): the counter must not be
    /// one-directional — with slow playback there are no forward skips at all,
    /// and the panel stayed silent exactly in the case people complained about.
    pub video_rollbacks: u64,
    pub diag_open: bool,
    pub log: std::collections::VecDeque<String>,
    /// Dedup of consecutive identical messages: (text, repeat count).
    last_msg: Option<(String, u32)>,
    last_offline: usize,
    last_hit_warn: Option<std::time::Instant>,
}

impl PerfState {
    fn new() -> PerfState {
        use crate::graphs::{Axis, RollingGraph};
        PerfState {
            quality: RollingGraph::new(
                k::perf::GRAPH_QUALITY,
                Axis::Percent,
                vec![
                    ("FPS", egui::Color32::from_rgb(120, 230, 90), 120.0),
                    // honest video fps: the NEW frames shown (item 1, v1.4;
                    // “FPS” is the UI frame rate and says nothing about the video)
                    ("V-fps", egui::Color32::from_rgb(255, 145, 80), 120.0),
                    ("A-hit", egui::Color32::from_rgb(114, 214, 255), 100.0),
                    ("V-hit", egui::Color32::from_rgb(240, 190, 70), 100.0),
                ],
            ),
            // FFmpeg activity: a port of v1's “avcpp Activity” (the same 5 series
            // and colors), but driven by LIVE counters of kt-ffmpeg::iostats
            decode: RollingGraph::new(
                k::perf::GRAPH_DECODE,
                Axis::LinearAuto,
                vec![
                    ("Total", egui::Color32::from_rgb(255, 145, 80), 10_000.0),
                    ("Open", egui::Color32::from_rgb(255, 205, 105), 500.0),
                    ("Read", egui::Color32::from_rgb(244, 99, 99), 10_000.0),
                    ("V-dec", egui::Color32::from_rgb(95, 209, 155), 1500.0),
                    ("A-dec", egui::Color32::from_rgb(130, 173, 255), 1500.0),
                ],
            ),
            churn: RollingGraph::new(
                k::perf::GRAPH_CHURN,
                Axis::LinearAuto,
                vec![
                    ("evict", egui::Color32::from_rgb(255, 160, 120), 2000.0),
                    ("miss", egui::Color32::from_rgb(200, 160, 255), 2000.0),
                    // the most valuable v1 metric: evicted BEFORE the first read
                    ("no-read", egui::Color32::from_rgb(255, 90, 90), 2000.0),
                ],
            ),
            last_sample: std::time::Instant::now(),
            last_cache: Default::default(),
            last_decode: (0, 0),
            last_io: (0, 0),
            last_underruns: (0, 0),
            frames: 0,
            video_frames_shown: 0,
            vfps: 0.0,
            last_video_shown: 0,
            video_cells_skipped: 0,
            video_rollbacks: 0,
            diag_open: false,
            log: std::collections::VecDeque::new(),
            last_msg: None,
            last_offline: 0,
            last_hit_warn: None,
        }
    }

    fn note(&mut self, msg: String) {
        // consecutive identical messages collapse into “×N”
        // (in v1 a burst flooded the whole log within seconds)
        if let Some((last, n)) = &mut self.last_msg
            && *last == msg
        {
            *n += 1;
            if let Some(line) = self.log.back_mut() {
                *line = format!("[{}] {} ×{}", chrono_like_now(), msg, n);
            }
            return;
        }
        self.last_msg = Some((msg.clone(), 1));
        let now = chrono_like_now();
        self.log.push_back(format!("[{now}] {msg}"));
        while self.log.len() > 800 {
            self.log.pop_front();
        }
    }
}

/// hh:mm:ss without a chrono dependency.
/// Autosave file of an unnamed project (port of v1
/// defaultAutosaveProjectPath: ~/.local/share/kinetren/).
/// Found unsaved work: the file, the time, and the project it
/// belongs to (`None` — work whose project had no name).
#[derive(Clone, Debug)]
pub struct RecoveryCandidate {
    pub path: PathBuf,
    pub mtime: std::time::SystemTime,
    pub for_project: Option<PathBuf>,
}

/// Collect everything the program did not manage to hand to the user (item 353).
///
/// Selection is by TIME, not by fact of existence. A scenario that before R.49
/// was not covered by anything: open project A → “New” → an hour of work → crash.
/// `last_project` pointed to A, the program obediently opened A, and said nothing about the hour
/// — because it looked only at `last_project` and
/// only at the moment when it was absent.
///
/// Project recovery is offered only if it is STRICTLY newer than the project itself
/// otherwise, after a normal Ctrl+S, the program would offer to “restore”
/// what already lies in the file. Unnamed work has nothing to compare against —
/// it is always offered until the user takes or deletes it.
pub fn recovery_candidates(project: Option<&Path>) -> Vec<RecoveryCandidate> {
    recovery_candidates_at(project, &autosave_path())
}

/// The same with an explicitly named file of unnamed work. A seam for the sake of tests: the data directory
/// is one per process, and tests run in parallel — swapping the environment variable
/// would break neighbours.
pub fn recovery_candidates_at(project: Option<&Path>, anon: &Path) -> Vec<RecoveryCandidate> {
    let mut out: Vec<RecoveryCandidate> = Vec::new();
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();

    if let Some(proj) = project {
        let base = recovery_path_for(Some(proj));
        let stem = base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let proj_time = mtime(proj);
        if let Some(dir) = base.parent()
            && let Ok(rd) = std::fs::read_dir(dir)
        {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                // both `<name>.recovery.kt` itself and the files of second copies
                // `<name>.recovery.kt.<pid>.kt`
                if !name.starts_with(&stem) || !name.ends_with(RECOVERY_EXT) {
                    continue;
                }
                let path = e.path();
                let Some(t) = mtime(&path) else { continue };
                if proj_time.is_some_and(|pt| t <= pt) {
                    continue; // project is newer — nothing to restore
                }
                out.push(RecoveryCandidate { path, mtime: t, for_project: Some(proj.into()) });
            }
        }
    }

    // unnamed work — in the data directory
    let stem = anon.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(dir) = anon.parent()
        && let Ok(rd) = std::fs::read_dir(dir)
    {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with(&stem) || !name.ends_with(RECOVERY_EXT) {
                continue;
            }
            let path = e.path();
            let Some(t) = mtime(&path) else { continue };
            out.push(RecoveryCandidate { path, mtime: t, for_project: None });
        }
    }

    // newer — on top: the one worked on last is offered first
    out.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.path.cmp(&b.path)));
    out
}

/// The project file extension, with the dot — recovery files end with it too.
const RECOVERY_EXT: &str = ".kt";
/// Default name in the save dialog of an unnamed project.
const UNTITLED_PROJECT: &str = "untitled.kt";
const _: () = assert!(RECOVERY_EXT.len() == kt_model::project_file::EXTENSION.len() + 1);
const _: () = assert!(UNTITLED_PROJECT.len() == "untitled".len() + RECOVERY_EXT.len());

/// Recovery file for this project: `<full name>.recovery.kt`
/// NEXT TO the project, of the SAME format as the project (R.70: writing megabytes
/// of the exchange format every half minute — both a delay and a loss window).
/// The suffix is appended to the FULL name, rather than replacing the extension (lesson
/// t150: `film.kt` and `film.mp4.kt` must give different names, otherwise one
/// project overwrites another's recovery).
/// An unnamed project is recovered in the data directory: there is no
/// directory next to it yet.
pub fn recovery_path_for(project: Option<&Path>) -> PathBuf {
    match project {
        Some(p) => {
            let mut name = p.file_name().unwrap_or_default().to_os_string();
            name.push(".recovery");
            name.push(RECOVERY_EXT);
            p.with_file_name(name)
        }
        None => autosave_path(),
    }
}

pub fn autosave_path() -> PathBuf {
    crate::journal::data_dir().join(format!("autosave-project{RECOVERY_EXT}"))
}

/// Project write: first to a temporary file next to it, then a rename.
/// A direct overwrite of the live file on a power loss or error midway
/// left the user with a truncated project.
/// Project write UNDER A GUARD (R.32): `rev` — the model revision carried by
/// `json`. If a snapshot NEWER than it (`*guard > rev`) has already landed on disk —
/// we do not write at all and return `Ok(false)`: otherwise the background autosave
/// would overwrite a more recent Ctrl+S with its stale snapshot, and the user's
/// edits were lost (the race was found by the live scenarios).
/// What the write attempt ended with (R.49, item 350).
///
/// Previously this returned a `bool`, and all three callers did
/// `.map(|_| ())` — “already newer on disk, I did not write” became
/// indistinguishable from “written”. The consequence for Ctrl+S: the asterisk went out,
/// “saved” was printed, and the recovery file was DELETED. The type removes this
/// branch: the three callers have three different answers, and one cannot be forgotten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutcome {
    /// Written. `dir_synced == false` — the file is in place, but the directory
    /// could not be synced (exFAT, network share, non-unix).
    Wrote { dir_synced: bool },
    /// A newer snapshot is already on disk — did not write at all.
    SkippedNewerOnDisk,
}

pub fn write_project_guarded(
    guard: &std::sync::Mutex<u64>,
    path: &std::path::Path,
    rev: u64,
    json: &str,
) -> std::io::Result<SaveOutcome> {
    // A poisoned mutex is no reason not to save: the thread that panicked under it
    // left only a number (the last written revision), and it is intact
    // (item 354 — in `Drop` any panic would mean an abort).
    let mut last = guard.lock().unwrap_or_else(|e| e.into_inner());
    if *last > rev {
        return Ok(SaveOutcome::SkippedNewerOnDisk);
    }
    let w = kt_otio::write_atomic(path, json)?;
    *last = rev;
    Ok(SaveOutcome::Wrote { dir_synced: w.dir_synced })
}

/// A thin wrapper over the single home of atomic writes (R.49, item 349):
/// the write itself, the fsync of the file and of the directory live in kt-otio, because kt-cli
/// goes there too — there is no second writer of the project file in the tree.
pub fn write_project_atomic(path: &std::path::Path, json: &str) -> std::io::Result<()> {
    kt_otio::write_atomic(path, json).map(|_| ())
}

/// The system file-picker dialog — ONE place (R.67). In the virtual
/// screen of live scenarios there is no portable dialog (rfd on Linux goes to
/// xdg-portal, which does not exist in Xvfb, and silently returns “nothing”), so
/// `KT_FILE_PICK=<path>` substitutes the dialog's answer: the menu item is
/// still clicked for real, and the file choice itself is the only thing that
/// a scenario cannot do by hand as a person would.
pub(crate) fn pick_file(title: &str, filter: &str, exts: &[&str]) -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("KT_FILE_PICK") {
        return Some(std::path::PathBuf::from(p));
    }
    rfd::FileDialog::new().set_title(title).add_filter(filter, exts).pick_file()
}

/// The SAVE dialog — the same `KT_FILE_PICK` seam as `pick_file`:
/// the virtual screen of live scenarios has no dialog, while the menu item
/// is clicked for real. `hint` — where and under what name to suggest it
/// (after importing `film.otio` — `film.kt` next to it).
pub(crate) fn pick_save(
    title: &str,
    filter: &str,
    exts: &[&str],
    hint: Option<&Path>,
    default_name: &str,
) -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("KT_FILE_PICK") {
        return Some(std::path::PathBuf::from(p));
    }
    let mut d = rfd::FileDialog::new().set_title(title).add_filter(filter, exts);
    match hint {
        Some(h) => {
            if let Some(dir) = h.parent() {
                d = d.set_directory(dir);
            }
            d = d.set_file_name(h.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| default_name.into()));
        }
        None => d = d.set_file_name(default_name),
    }
    d.save_file()
}

/// The text of the project file (R.70): the model as line-by-line JSON plus foreign
/// passthrough as a separate field, when there is one. The only place where
/// a document turns into the bytes of the project file: Ctrl+S, autosave and `Drop`
/// go here; kt-cli goes to the same function in kt-model.
pub fn project_text_of(
    project: &Project,
    retained: &kt_otio::Retained,
    base_dir: Option<&Path>,
) -> String {
    let otio = (!retained.is_empty()).then(|| retained.to_json());
    // in the file, media paths are RELATIVE to it (t797): the project moves
    // together with its material; in memory they are absolute (see `load_project_file`)
    let mut project = project.clone();
    kt_otio::relativize_project_media(&mut project, base_dir);
    kt_model::project_file::to_text(&project, otio.as_ref())
}

/// Reading a PROJECT file from disk for open() (R.70): only its own format.
/// Exchange and legacy files are rejected with the name of the import item (rule 19),
/// a file of a future version — with its version number and without a partial read.
pub(crate) fn load_project_file(
    path: &Path,
) -> Result<(Project, kt_otio::Retained, Vec<String>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    // An empty file is not “broken JSON” but a known outcome of a power loss before
    // R.49: the rename reached the journal before the data. The raw serde string
    // (“EOF while parsing a value at line 1 column 0”) says nothing about this,
    // while the user needs to understand what to look for nearby — the recovery file —
    // rather than fix the syntax.
    if text.trim().is_empty() {
        return Err(crate::tf!(k::msg::PROJECT_FILE_EMPTY, path.display().to_string()));
    }
    use kt_model::project_file::{FileKind, ProjectFileError, from_text};
    match from_text(&text) {
        Ok(file) => {
            let retained = match file.otio {
                Some(v) => kt_otio::Retained::from_json(&v).map_err(|e| e.to_string())?,
                None => kt_otio::Retained::default(),
            };
            let mut project = file.project;
            kt_otio::resolve_project_media(&mut project, path.parent());
            Ok((project, retained, Vec::new()))
        }
        Err(ProjectFileError::NotOurs { looks_like: Some(FileKind::Otio) }) => {
            // an exchange file is not a project; its door is import, by the item name
            Err(crate::tf!(
                k::msg::OTIO_IMPORT_ONLY,
                path.display().to_string(),
                tr(k::menu::file::IMPORT_OLDER)
            ))
        }
        Err(ProjectFileError::NotOurs { looks_like: Some(FileKind::LegacyKtproj) }) => {
            Err(crate::tf!(k::msg::KTPROJ_IMPORT_ONLY, tr(k::menu::file::IMPORT_OLDER)))
        }
        Err(ProjectFileError::NotOurs { looks_like: Some(FileKind::FormerName) }) => {
            // saved before the rename to Kinetren: the import door renames
            // the root key (R.67 — the reader is not taught old forms)
            Err(crate::tf!(
                k::msg::FORMER_NAME_IMPORT_ONLY,
                path.display().to_string(),
                tr(k::menu::file::IMPORT_OLDER)
            ))
        }
        Err(ProjectFileError::TooNew { version }) => {
            Err(crate::tf!(k::msg::PROJECT_TOO_NEW, version))
        }
        Err(e) => Err(e.to_string()),
    }
}

fn chrono_like_now() -> String {
    let secs =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let s = secs % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

impl KtApp {
    /// Metrics sampler every 500 ms (port of performanceTimer).
    fn sample_perf(&mut self) {
        self.perf.frames += 1;
        let dt = self.perf.last_sample.elapsed().as_secs_f32();
        if dt < 0.5 {
            return;
        }
        self.perf.last_sample = std::time::Instant::now();
        let ord = std::sync::atomic::Ordering::Relaxed;

        // saturating: recreating the player zeroes the counters of the new cache,
        // while perf.last_* holds the cumulatives of the old one — a bare subtraction gave
        // a u64 wrap and “10870945131100372992 evictions/s” (item 24)
        let cache = self.player.cache.stats();
        let lc = self.perf.last_cache;
        let d_ah = cache.audio_hits.saturating_sub(lc.audio_hits);
        let d_am = cache.audio_misses.saturating_sub(lc.audio_misses);
        let d_vh = cache.video_hits.saturating_sub(lc.video_hits);
        let d_vm = cache.video_misses.saturating_sub(lc.video_misses);
        let d_ev = cache.evictions.saturating_sub(lc.evictions);
        self.perf.last_cache = cache;

        let a_dec = self.player.decode_stats.audio_decodes.load(ord);
        let v_dec = self.player.decode_stats.video_decodes.load(ord);
        let (la, lv) = self.perf.last_decode;
        self.perf.last_decode = (a_dec, v_dec);
        // FFmpeg activity: opens (demux + decoders, incl. reopen on seek) and
        // packets read (port of v1's avcpp open/read)
        let (opens, reads) = kt_media::ffmpeg::io_stats();
        let (lo, lr) = self.perf.last_io;
        self.perf.last_io = (opens, reads);

        let fps = self.perf.frames as f32 / dt;
        self.perf.frames = 0;
        let a_hit =
            if d_ah + d_am > 0 { d_ah as f32 / (d_ah + d_am) as f32 * 100.0 } else { 100.0 };
        let v_hit =
            if d_vh + d_vm > 0 { d_vh as f32 / (d_vh + d_vm) as f32 * 100.0 } else { 100.0 };
        let v_shown = self.perf.video_frames_shown;
        let v_fps = v_shown.saturating_sub(self.perf.last_video_shown) as f32 / dt;
        self.perf.last_video_shown = v_shown;
        self.perf.vfps = v_fps; // item 23, round 7: the toolbar timecode
        self.perf.quality.push(&[fps.min(120.0), v_fps.min(120.0), a_hit, v_hit]);
        let (r_open, r_read) =
            (opens.saturating_sub(lo) as f32 / dt, reads.saturating_sub(lr) as f32 / dt);
        let (r_vdec, r_adec) =
            (v_dec.saturating_sub(lv) as f32 / dt, a_dec.saturating_sub(la) as f32 / dt);
        self.perf.decode.push(&[r_open + r_read + r_vdec + r_adec, r_open, r_read, r_vdec, r_adec]);
        let d_unused = cache.unused_evictions.saturating_sub(lc.unused_evictions);
        self.perf.churn.push(&[d_ev as f32 / dt, (d_am + d_vm) as f32 / dt, d_unused as f32 / dt]);

        // problem log
        let ur = self.player.underruns.load(ord);
        let uc = self.player.shared.callback_underruns.load(ord);
        let (lur, luc) = self.perf.last_underruns;
        if ur > lur {
            self.perf.note(crate::tf!(k::perf::note::RENDER_UNDERRUNS, ur - lur, ur));
        }
        if uc > luc {
            self.perf.note(crate::tf!(k::perf::note::CALLBACK_UNDERRUNS, uc - luc, uc));
        }
        self.perf.last_underruns = (ur, uc);
        if d_ev as f32 / dt > 500.0 {
            self.perf.note(crate::tf!(
                k::perf::note::EVICTION_SPIKE,
                format!("{:.0}", d_ev as f32 / dt)
            ));
        }
        // port of the main v1 warning: frames evicted BEFORE the first read
        if d_unused > 0 {
            self.perf.note(crate::tf!(k::perf::note::PREFETCH_THRASH, d_unused));
        }
        // threshold (v1 had no thresholds at all): low video hit during playback
        if v_hit < 80.0
            && self.player.transport.is_playing()
            && self.perf.last_hit_warn.map(|t| t.elapsed().as_secs() >= 10).unwrap_or(true)
        {
            self.perf.last_hit_warn = Some(std::time::Instant::now());
            self.perf.note(crate::tf!(k::perf::note::LOW_VIDEO_HIT, format!("{v_hit:.0}")));
        }
        let offline_now = self.offline.lock().unwrap().len();
        if offline_now != self.perf.last_offline {
            self.perf.note(crate::tf!(k::perf::note::OFFLINE_CHANGED, offline_now));
            self.perf.last_offline = offline_now;
        }
    }

    fn draw_diagnostics(&mut self, ctx: &egui::Context) {
        if !self.perf.diag_open {
            return;
        }
        let mut open = self.perf.diag_open;
        let shown = egui::Window::new(tr(k::win::DIAG_TITLE))
            .id(egui::Id::new("kt_diag"))
            .open(&mut open)
            .default_size([560.0, 460.0])
            .min_size([420.0, 300.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                // video cache occupancy from the REAL limit (in v1 the limit constant
                // was duplicated by hand and was off by a factor of 384)
                let used = self.player.cache.video_bytes() as f64;
                let limit = self.player.cache.max_video_bytes() as f64;
                let occ = if limit > 0.0 { (used / limit * 100.0).min(100.0) } else { 0.0 };
                ui.horizontal(|ui| {
                    ui.weak(crate::tf!(
                        k::perf::CACHE_OCCUPANCY,
                        format!("{occ:.0}"),
                        format!("{:.0}", used / 1e6),
                        format!("{:.0}", limit / 1e6)
                    ));
                    let cache = self.player.cache.stats();
                    ui.weak(crate::tf!(k::perf::NO_READ_DROPS, cache.unused_evictions));
                });
                let (bar, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 6.0),
                    egui::Sense::hover(),
                );
                let pnt = ui.painter_at(bar);
                pnt.rect_filled(bar, 3.0, egui::Color32::from_gray(30));
                let fill_w = bar.width() * (occ as f32 / 100.0);
                let col = if occ > 90.0 {
                    egui::Color32::from_rgb(230, 120, 90)
                } else {
                    egui::Color32::from_rgb(95, 209, 155)
                };
                pnt.rect_filled(
                    egui::Rect::from_min_size(bar.min, egui::vec2(fill_w, bar.height())),
                    3.0,
                    col,
                );
                ui.add_space(6.0);
                self.perf.churn.draw(ui, 110.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr(k::perf::LOG_TITLE)).strong());
                    if ui.button(tr(k::perf::LOG_CLEAR)).clicked() {
                        self.perf.log.clear();
                        self.perf.last_msg = None;
                    }
                    if ui.button(tr(k::perf::GRAPH_RESET)).clicked() {
                        self.perf.quality.clear();
                        self.perf.decode.clear();
                        self.perf.churn.clear();
                    }
                });
                egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    for line in &self.perf.log {
                        ui.monospace(line);
                    }
                    if self.perf.log.is_empty() {
                        ui.weak(tr(k::perf::LOG_EMPTY));
                    }
                });
            });
        // the whole window frame — the README screenshots crop by it
        if let Some(r) = &shown {
            crate::widgets::field_probe::note("win.diag", r.response.rect);
        }
        self.perf.diag_open = open;
    }
}

impl eframe::App for KtApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // LIVE BENCH (R.68): the bench command is executed BEFORE the frame — a scenario
        // that waited for the frame must already see the new project. Without
        // KT_LIVE_CTL there is just one OnceLock check here and not a single system operation.
        if crate::bench::enabled()
            && let Some((seq, cmd)) = crate::bench::poll()
        {
            match cmd {
                crate::bench::Cmd::Open(p) => self.bench_open(ui.ctx(), &p),
                crate::bench::Cmd::Store(dir) => crate::bench::set_store(dir),
                crate::bench::Cmd::Noop => {}
            }
            crate::bench::finished(seq);
        }

        // Intercepting window close (item 354). `CancelClose` in eframe 0.36
        // works for the ROOT window too — so the question “save before
        // exit?” is asked by the standard path, without our own handling of system events.
        if ui.ctx().input(|i| i.viewport().close_requested())
            && !self.quit_confirmed
            && !self.guard_unsaved(Pending::Quit)
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if self.quit_confirmed {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // software-renderer detection — once (items 17, 19)
        if self.sw_render.is_none() {
            let sw = _frame
                .wgpu_render_state()
                .map(|rs| {
                    let info = rs.adapter.get_info();
                    self.gpu_info = Some(format!("{} / {:?}", info.name, info.backend));
                    let name = info.name.to_lowercase();
                    info.device_type == eframe::wgpu::DeviceType::Cpu
                        || name.contains("llvmpipe")
                        || name.contains("softpipe")
                        || name.contains("swiftshader")
                        // Windows' software rasterizer (WARP)
                        || name.contains("microsoft basic render")
                })
                .unwrap_or(false);
            self.sw_render = Some(sw);
            if sw {
                self.warn(tr(k::msg::SOFTWARE_RENDER));
            }
            // R.29: video FX and composite on the GPU by default — the engine on the
            // eframe device. Software rendering (llvmpipe) also counts as “GPU present”.
            self.init_video_gpu(_frame);
        }

        // R.31 item 2: long UI-thread frames go to the perf log. The DURATION
        // of a frame is measured (not the gap between frames — idle repaint is
        // legitimately rare): a block inside a frame (a disk write,
        // a decode on the UI thread) is seen here with an exact timestamp.
        let t0 = std::time::Instant::now();
        self.draw_all(ui);
        let ms = t0.elapsed().as_millis();
        if ms >= 100 {
            self.perf.note(crate::tf!(k::perf::note::UI_STALL, ms));
        }
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // R.31 item 2: eframe calls save() EVERY 30 seconds (auto_save_interval)
        // — only the cheap things remain here (egui-storage entries). The former
        // synchronous project save from here dropped the FPS to zero every 30 s:
        // “moved” (cursor/viewport shifted) was almost always true, and the whole
        // project was serialized and written ON THE UI THREAD even without edits.
        // Periodic saving is handled by our BACKGROUND autosave (draw_all), and the final
        // write on exit — on_exit below.
        storage.set_string("lang", crate::i18n::choice_code(self.lang_choice).to_string());
        if let Some(p) = &self.project_path {
            storage.set_string("last_project", p.to_string_lossy().to_string());
        }
        if let Some(v) = self.telemetry.as_ref().and_then(|t| t.stored_value()) {
            storage.set_string(crate::telemetry::STORAGE_KEY, v.to_string());
        }
    }

    fn on_exit(&mut self) {
        // The final save on exit — synchronous here is fine (the window is already gone).
        // First we wait for the background autosave: its rename must not
        // overwrite our fresher snapshot with the stale one.
        if let Some((_, rx)) = self.autosave_rx.take() {
            let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
        }
        // R.49 item 351: exit is NOT the moment to silently rewrite the user's project.
        // Unsaved work goes to the recovery file; saved work carries its own recovery file
        // away with it, so that the next start does not offer
        // to restore what is already in the project.
        self.save_workspace();
        if self.dirty_content || self.dirty_ui {
            self.model.set_ui_state(Some(self.ui_state()));
            self.write_recovery_now();
        } else {
            self.drop_own_recovery();
        }
        // after the recovery write (durability first): the clean-close event, within a
        // fixed budget; a dead network looks exactly like no telemetry
        if let Some(t) = &self.telemetry {
            t.closed(crate::i18n::current().code(), crate::telemetry::EXIT_BUDGET);
        }
    }
}

/// The last line of defence (R.49, item 354).
///
/// It fires where `on_exit` is not called at all: on `*self = app` (File →
/// New/Open recreate the application IN PLACE) and on stack unwinding from a
/// panic — `panic = "unwind"` is kept on purpose in our build.
///
/// The body contains NOT A SINGLE `unwrap`. A panic inside `Drop` during unwinding
/// is an immediate `abort`, and it would take with it exactly the file the whole thing
/// was for. For the same reason the guard is taken tolerant of a poisoned
/// mutex: a thread that panicked under the lock must not deprive us of the write.
impl Drop for KtApp {
    fn drop(&mut self) {
        if !self.recovery_on_drop || !(self.dirty_content || self.dirty_ui) {
            return;
        }
        self.write_recovery_now();
    }
}

/// GPU engine for export (R.29): its own headless device per job — kt-cli
/// has no window, and the UI queue does not share time with the 4K export. None → CPU
/// (for a 2D project this is a full-fledged path; `KT_VIDEO_GPU=0` — forced).
pub(crate) fn export_video_gpu() -> Option<std::sync::Arc<dyn kt_engine::VideoGpuExec>> {
    if std::env::var("KT_VIDEO_GPU").as_deref() == Ok("0") {
        return None;
    }
    match kt_gpu::GpuContext::new_headless().and_then(kt_gpu::GpuFxEngine::new) {
        Ok(e) => Some(e),
        Err(err) => {
            eprintln!("video export on CPU (no GPU): {err}");
            None
        }
    }
}

pub fn run() -> eframe::Result {
    let arg = std::env::args().nth(1);
    // the former data directory (Kinotrain) moves into place BEFORE eframe
    // reads its storage: that storage lives in the same directory
    let _ = crate::journal::data_dir();
    // the FFmpeg folder chosen with Locate… is the first step of the search (windows-build)
    crate::ffmpeg_setup::install_search_dirs();
    // GPU: the egui-wgpu default is HighPerformance, i.e. the discrete card; on
    // hybrid graphics this is PRIME copying of every frame and a stutter of the WHOLE
    // system (item 1, v1.4). The UI load is modest — we take the power-efficient
    // adapter; WGPU_POWER_PREF=high_performance returns the discrete one.
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.power_preference = eframe::wgpu::PowerPreference::from_env()
            .unwrap_or(eframe::wgpu::PowerPreference::LowPower);
    }
    // borderless (item 22, round 6): our own title bar in the toolbar row; KT_DECORATIONS=1
    // returns the system frame (insurance against WM quirks)
    let decorations = std::env::var_os("KT_DECORATIONS").is_some();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 860.0])
            .with_min_inner_size([640.0, 640.0])
            .with_icon(app_icon())
            // app_id for Wayland and the link with .desktop (on X11 WM_CLASS
            // is still the binary name, see StartupWMClass in .desktop)
            .with_app_id("kinetren")
            .with_decorations(decorations)
            .with_title("Kinetren"),
        wgpu_options,
        // eframe's settings (`app.ron`) in OUR data directory: the same file eframe would
        // pick on Linux and on Windows, named explicitly so the two cannot drift apart
        // (windows-build, design D4)
        persistence_path: Some(crate::journal::data_dir().join(kt_dirs::EFRAME_SETTINGS_FILE)),
        ..Default::default()
    };
    eframe::run_native(
        "kinetren",
        options,
        Box::new(move |cc| {
            // The language is resolved BEFORE KtApp is built: the strings below (the restored
            // autosave, the prompt to open a project) are already translated.
            // KT_LANG — a debug override in the row with KT_AUTOPLAY/KT_DECORATIONS.
            let lang_choice = std::env::var("KT_LANG")
                .ok()
                .or_else(|| cc.storage.and_then(|s| s.get_string("lang")))
                .and_then(|v| crate::i18n::choice_from_code(&v));
            crate::theme::apply(&cc.egui_ctx);
            crate::theme::apply_debug_flags(&cc.egui_ctx); // item 23, round 6
            crate::i18n::set_lang_ctx(&cc.egui_ctx, crate::i18n::resolve(lang_choice));
            crate::icons::install_fonts(&cc.egui_ctx);
            crate::journal::install_panic_hook();
            // before the first KtApp: every app of this process takes the same telemetry
            crate::telemetry::install(crate::telemetry::Telemetry::for_process(
                cc.storage.and_then(|s| s.get_string(crate::telemetry::STORAGE_KEY)).as_deref(),
            ));
            let restored = arg
                .clone()
                .or_else(|| cc.storage.and_then(|s| s.get_string("last_project")))
                .map(std::path::PathBuf::from)
                .filter(|p| p.exists());
            let mut app = match restored {
                Some(path) => {
                    let mut a = KtApp::open(&path).unwrap_or_else(|e| KtApp::new_empty(Some(e)));
                    a.claim_project(&path);
                    a
                }
                None => KtApp::new_empty(Some(tr(k::msg::OPEN_A_PROJECT).to_string())),
            };
            app.lang_choice = lang_choice;
            // R.49 item 353: unsaved work no longer “replaces the project if
            // last_project was not found”, but is OFFERED — with the path and
            // time of each candidate. The former branch did not cover the main
            // scenario: open project A, press “New”, an hour of work, crash —
            // last_project pointed to A, and not a line was said about the hour.
            // the previous session crashed — the report lies there and waits (item 355)
            if let Some(p) = crate::journal::take_crash_report() {
                app.warn(crate::tf!(k::msg::PREV_SESSION_CRASHED, p.display()));
            }
            // without FFmpeg the FFmpeg window opens once, with one notice (windows-build)
            app.ffmpeg_startup_check();
            // no per-user directory: the data lives in temp and may vanish (windows-build)
            if kt_dirs::is_fallback() {
                app.warn(crate::tf!(k::msg::DATA_DIR_TEMP, crate::journal::data_dir().display()));
            }
            app.offer_recovery(true);
            Ok(Box::new(app))
        }),
    )
}

impl KtApp {
    /// The File → Import an old-version project… menu (R.67; with R.70 — one
    /// door for the legacy `.ktproj`, our old `.otio` and a foreign `.otio`;
    /// the former separate item “Import .ktproj…” was merged here).
    pub fn import_older_dialog(&mut self) {
        if self.guard_unsaved(Pending::ImportOlder) {
            self.import_older_dialog_now();
        }
    }

    fn import_older_dialog_now(&mut self) {
        let Some(p) = pick_file(
            tr(k::dialog::IMPORT_OLDER),
            "OpenTimelineIO / legacy .ktproj",
            &["otio", "ktproj"],
        ) else {
            return;
        };
        self.import_older_path(&p);
    }

    /// The import body by path — shared by the dialog and by a file passed at
    /// startup (the live scenario and the command line). No lock is put
    /// on the source: after import the document is unnamed, and it does not yet
    /// have a project file.
    pub(crate) fn import_older_path(&mut self, p: &Path) {
        match Self::open_older(p) {
            Ok(app) => *self = app,
            Err(e) => self.note(crate::tf!(k::msg::OPEN_FAILED, e)),
        }
    }

    /// File → Export → OpenTimelineIO… (R.70): a SEPARATE named action,
    /// not a save. The exchange file is self-sufficient — the timeline body inline in
    /// each instance, the foreign passthrough is put back in place. The project
    /// remains `.kt` in this case, the asterisk does not go out: an export is not a project write.
    pub fn export_otio_dialog(&mut self) {
        let hint = self
            .project_path
            .as_deref()
            .or(self.save_hint.as_deref())
            .map(|p| p.with_extension("otio"));
        let Some(dst) = pick_save(
            tr(k::dialog::EXPORT_OTIO),
            "OpenTimelineIO",
            &["otio"],
            hint.as_deref(),
            "untitled.otio",
        ) else {
            return;
        };
        let dst = if dst.extension().is_none() { dst.with_extension("otio") } else { dst };
        self.export_otio_to(&dst);
    }

    /// The export body by path — for the menu item and for the live scenario.
    pub(crate) fn export_otio_to(&mut self, dst: &Path) {
        let out = kt_otio::save_project(&self.model.document(), &self.retained, dst.parent());
        match write_project_atomic(dst, &out) {
            Ok(()) => self.note(crate::tf!(k::msg::EXPORTED, dst.display().to_string())),
            Err(e) => self.warn(crate::tf!(k::msg::SAVE_FAILED, e)),
        }
    }

}

/// Merge overlapping/adjacent warmth ranges into one list
/// (R.46, item 322): pieces of one clip may be warmed at different decode
/// qualities, but the bar under the clip is one.
pub fn merge_runs(mut runs: Vec<std::ops::Range<i64>>) -> Vec<std::ops::Range<i64>> {
    if runs.len() < 2 {
        return runs;
    }
    runs.sort_by_key(|r| r.start);
    let mut out: Vec<std::ops::Range<i64>> = Vec::with_capacity(runs.len());
    for r in runs {
        match out.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => out.push(r),
        }
    }
    out
}

/// Frame safe-zone rectangles: action 5 % and title 10 % from each
/// side (percentages — F-TXT-001, a legacy of CRT tubes).
pub(crate) fn safe_zone_rects(img: egui::Rect) -> [egui::Rect; 2] {
    let inset = |f: f32| {
        egui::Rect::from_min_max(
            img.min + egui::vec2(img.width() * f, img.height() * f),
            img.max - egui::vec2(img.width() * f, img.height() * f),
        )
    };
    [inset(0.05), inset(0.10)]
}

#[cfg(test)]
mod repaint_tests {
    use super::repaint_delay;

    /// Test 58 (item 15 of round 6; replaces the former rules “from pause with a
    /// region — always back to the start” by the user's sanction): transport state machine
    /// Space — pause in place; resume from the spot; double Space (within the
    /// DOUBLE_PAUSE_MS window) = stop to the start of the region/0; resume with loop on and
    /// cursor outside the region — from the start of the region (wrap works from inside).
    #[test]
    fn t58_transport_toggle_rules() {
        use super::{KtApp, TransportAction as A};
        let toggle = KtApp::transport_toggle;
        // playing → pause (cursor not touched)
        assert_eq!(toggle(true, false, 500, Some((100, 200)), true), A::Pause);
        // pause → resume from the spot (region exists, but cursor inside / loop any)
        assert_eq!(toggle(false, false, 150, Some((100, 200)), true), A::Resume { seek: None });
        assert_eq!(toggle(false, false, 150, Some((100, 200)), false), A::Resume { seek: None });
        assert_eq!(toggle(false, false, 5, None, false), A::Resume { seek: None });
        // loop on, cursor OUTSIDE the region → resume from the start of the region
        assert_eq!(
            toggle(false, false, 500, Some((100, 200)), true),
            A::Resume { seek: Some(100) }
        );
        // t73 (item 29 of round 7, a sanctioned amendment of t58): loop off,
        // cursor ON/PAST the end of the region (after auto-pause) → Play plays the
        // region again from the start; previously Resume{None} with the same frame
        // fell back into the auto-pause window — “play does nothing”
        assert_eq!(
            toggle(false, false, 500, Some((100, 200)), false),
            A::Resume { seek: Some(100) }
        );
        assert_eq!(
            toggle(false, false, 200, Some((100, 200)), false),
            A::Resume { seek: Some(100) }
        );
        // ...but playback BEFORE the region is not touched (cursor < start)
        assert_eq!(toggle(false, false, 50, Some((100, 200)), false), A::Resume { seek: None });
        // double Space in pause → stop to the start of the region (or 0)
        assert_eq!(toggle(false, true, 150, Some((100, 200)), false), A::SeekStart(100));
        assert_eq!(toggle(false, true, 150, None, false), A::SeekStart(0));
        // empty region is ignored
        assert_eq!(toggle(false, true, 150, Some((100, 100)), true), A::SeekStart(0));
        assert_eq!(toggle(false, false, 500, Some((100, 100)), true), A::Resume { seek: None });
    }

    /// Test 765 (R.64): the preview fraction is SIMILAR to the project frame.
    ///
    /// Independent even rounding of the axes gave 1920×1080 → 240×134 at
    /// 1/8 and 120×66 at 1/16 (1.791 and 1.818 against 1.778): “fit”
    /// computed a different frame in the preview than in the export, and the on-screen control
    /// would show something other than what goes into the file.
    #[test]
    fn t765_preview_fraction_keeps_the_project_aspect() {
        use super::preview_quality_for;
        for (pw, ph) in [(1920u32, 1080u32), (1080, 1920), (1280, 720), (1998, 1080)] {
            let want = pw as f64 / ph as f64;
            for f in [1.0f32, 0.5, 0.25, 0.125, 0.0625] {
                let q = preview_quality_for(pw, ph, f);
                assert!(q.width >= 2 && q.height >= 2, "{pw}x{ph} @{f}: degenerate size {q:?}");
                assert!(
                    q.width.is_multiple_of(2) && q.height.is_multiple_of(2),
                    "{pw}x{ph} @{f}: odd size {q:?} — the 4:2:0 decoder will not return it"
                );
                // the aspect deviation — no more than half a pixel on the
                // SHORT side (parity does not allow more precision)
                let got = q.width as f64 / q.height as f64;
                let short = q.width.min(q.height) as f64;
                let tol = want * (0.5 / short) * 2.0;
                assert!(
                    (got - want).abs() <= tol,
                    "{pw}x{ph} @{f}: preview {q:?} is not similar to the frame — {got:.4} against {want:.4} \
                     (tolerance {tol:.4})"
                );
            }
        }
        // The “fit” FRAME is the same in FRACTIONS of the canvas at all fractions —
        // that is the purpose of similarity: preview and export must compute
        // one frame. (The check lives here, not in kt-fx: there is no preview
        // fraction there, while kt-app has both halves.)
        for view in [(1080.0f64, 1920.0f64), (3840.0, 2160.0), (720.0, 720.0)] {
            let mut ref_frac: Option<(f64, f64)> = None;
            for f in [1.0f32, 0.5, 0.25, 0.125, 0.0625] {
                let q = preview_quality_for(1920, 1080, f);
                let canvas = (q.width as f64, q.height as f64);
                let d = kt_fx::conform::conform_dest(
                    Some(view),
                    canvas,
                    kt_fx::conform::ConformRule::Fit,
                );
                let frac = (d.0 / canvas.0, d.1 / canvas.1);
                match ref_frac {
                    None => ref_frac = Some(frac),
                    Some(r) => {
                        let tol = 1.0 / q.width.min(q.height) as f64;
                        assert!(
                            (frac.0 - r.0).abs() <= tol && (frac.1 - r.1).abs() <= tol,
                            "view {view:?} @{f}: the “fit” frame in canvas fractions diverged — \
                             {frac:?} against {r:?} (tolerance {tol:.4})"
                        );
                    }
                }
            }
        }

        // …and this is NOT “any size fits”: the former independent
        // rounding does not pass its own tolerance at these same fractions
        for (f, w, h) in [(0.125f32, 240u32, 134u32), (0.0625, 120, 66)] {
            let want = 1920.0 / 1080.0;
            let got = w as f64 / h as f64;
            let tol = want * (0.5 / h.min(w) as f64) * 2.0;
            assert!(
                (got - want).abs() > tol,
                "the scene is degenerate: the previous rounding @{f} ({w}x{h}) fit within tolerance"
            );
            let now = preview_quality_for(1920, 1080, f);
            assert_ne!(
                (now.width, now.height),
                (w, h),
                "the @{f} share stayed the same — the function changed nothing"
            );
        }
    }

    /// Test 39 (item 1, v1.4): the preview key is aligned to the fps grid — within
    /// one frame cell the key is stable (the texture is not re-uploaded),
    /// at the boundary — a new one. The V-fps counter grows only on a new key.
    #[test]
    fn t39_preview_key_is_grid_aligned() {
        use super::preview_key_tick;
        let grid = kt_time::FpsGrid::new(30); // step 3000 ticks
        assert_eq!(preview_key_tick(grid, kt_time::Tick(0)), 0);
        assert_eq!(preview_key_tick(grid, kt_time::Tick(1)), 0);
        assert_eq!(preview_key_tick(grid, kt_time::Tick(2999)), 0, "inside the cell the key is the same");
        assert_eq!(preview_key_tick(grid, kt_time::Tick(3000)), 3000, "the boundary — a new key");
        // UI simulation at 60 fps (step 1500 ticks) on a 30 fps project:
        // of 60 ui frames exactly 30 yield a new key
        let mut shown = 0u64;
        let mut prev: Option<i64> = None;
        for ui_frame in 0..60i64 {
            let key = preview_key_tick(grid, kt_time::Tick(ui_frame * 1500));
            if prev != Some(key) {
                shown += 1;
            }
            prev = Some(key);
        }
        assert_eq!(shown, 30, "V-fps counts project frames, not UI frames");
    }

    /// Test 46 (items 4–5 of v1.4, REWRITTEN in R.57 item 455): mapping of the
    /// speed strip.
    ///
    /// The owner's claim: “the range −0.25…0…0.25 turns into 0, and I
    /// must be able to set 0.05 or −0.03”. The cause was right here: the dead zone of 7 % of
    /// travel gave 0, and its EDGE already equalled 0.25× — the interval (0, 0.25) was
    /// unrepresentable by construction. So the test now asserts not
    /// “the dead zone is wide”, but the reverse: only a narrow zone gives zero,
    /// the floor has no cliff, and 0.05 and 0.03 are spaced so that
    /// one can hit them with a mouse.
    #[test]
    fn t46_rate_slider_mapping() {
        use super::{RATE_FLOOR, RATE_MAX, RATE_STOP, rate_to_slider, slider_to_rate};
        // --- stop: ONLY |t| < RATE_STOP
        for t in [0.0f32, 0.01, -0.01, 0.0179, -0.0179] {
            assert_eq!(slider_to_rate(t), 0.0, "t={t} must be a stop");
        }
        for t in [RATE_STOP, -RATE_STOP, 0.02, -0.02, 0.05] {
            assert_ne!(slider_to_rate(t), 0.0, "t={t} is outside the stop zone, yet returns 0");
        }
        // --- continuity at the floor: a cliff at the edge of the zone — no more than RATE_FLOOR
        // (before R.57 it was 0.25× — and that is the defect)
        let edge = slider_to_rate(RATE_STOP);
        assert!(
            (edge - RATE_FLOOR).abs() < 1e-6,
            "the edge of the stop zone must give floor {RATE_FLOOR}, but gives {edge}"
        );
        // --- oddness
        for t in [0.02f32, 0.1, 0.3, 0.7, 1.0] {
            assert_eq!(slider_to_rate(t), -slider_to_rate(-t), "oddness violated at t={t}");
        }
        // --- strict monotonicity in steps of 0.001 (no plateaus or dips)
        let mut prev = 0.0f32;
        let mut t = RATE_STOP;
        while t <= 1.0 {
            let r = slider_to_rate(t);
            assert!(r > prev, "does not grow: t={t} gives {r} after {prev}");
            prev = r;
            t += 0.001;
        }
        // --- bounds
        assert!((slider_to_rate(1.0) - RATE_MAX).abs() < 1e-5);
        assert!((slider_to_rate(-1.0) + RATE_MAX).abs() < 1e-5);
        // --- round-trip, including WHAT THE OWNER ASKED FOR
        for r in [0.01f32, 0.03, 0.05, 0.1, 0.25, 0.5, 1.0, 1.5, 2.0, -0.03, -0.25, -1.0, -2.0] {
            let back = slider_to_rate(rate_to_slider(r));
            assert!((back - r).abs() < 1e-4, "round-trip {r} → {back}");
        }
        assert_eq!(rate_to_slider(0.0), 0.0, "stop — the center of the band");
        // |r| > 2 sits on the edge: the label stays visible
        assert_eq!(rate_to_slider(8.0), 1.0);
        assert_eq!(rate_to_slider(-8.0), -1.0);
        // --- 1× sits within [0.65, 0.80] of travel: there must be room on the right
        // for acceleration, and the whole slow section on the left
        let one = rate_to_slider(1.0);
        assert!((0.65..=0.80).contains(&one), "1× drifted by {one} steps");
        // --- 0.05 and 0.03 are spaced ~2 px apart (half of the 85 px strip):
        // “hit with the mouse on the first try” — rule 1. Before R.57 both values
        // lay in the dead zone and were indistinguishable from zero AT ALL.
        let px = |r: f32| rate_to_slider(r) * 85.0;
        assert!(
            px(0.05) - px(0.03) >= 1.9,
            "0.03 and 0.05 stuck together: {} px against {} px",
            px(0.03),
            px(0.05)
        );
    }

    /// Test 626 (R.57): arithmetic of input and fine adjustment.
    #[test]
    fn t626_rate_entry_arithmetic() {
        use super::{
            ENGINE_RATE_MAX, RATE_FLOOR, SPEED_FINE_DIV, boost_needs_update, drag_advance,
            normalize_rate, parse_rate_text,
        };
        // --- normalize_rate: almost-zero = stop, otherwise floor and ceiling with sign
        assert_eq!(normalize_rate(0.0), 0.0);
        assert_eq!(normalize_rate(0.004), 0.0);
        assert_eq!(normalize_rate(-0.004), 0.0);
        assert_eq!(normalize_rate(0.006), RATE_FLOOR, "above the threshold — not zero but the floor");
        assert_eq!(normalize_rate(0.05), 0.05);
        assert_eq!(normalize_rate(-0.03), -0.03);
        assert_eq!(normalize_rate(99.0), ENGINE_RATE_MAX);
        assert_eq!(normalize_rate(-99.0), -ENGINE_RATE_MAX);
        assert_eq!(normalize_rate(f32::NAN), 0.0);
        // --- parse_rate_text: what a person actually types
        for (s, want) in [
            ("0.05", 0.05f32),
            ("0,05", 0.05),
            ("-0.03", -0.03),
            ("\u{2212}0,03", -0.03), // typographic minus
            ("+1.5", 1.5),
            ("2×", 2.0),
            ("2x", 2.0),
            (" 0.5 ", 0.5),
        ] {
            let got = parse_rate_text(s).unwrap_or_else(|| panic!("“{s}” was not parsed"));
            assert!((got - want).abs() < 1e-6, "“{s}” → {got}, expected {want}");
        }
        for s in ["", "×", "--1", "fast", "1.2.3"] {
            assert!(parse_rate_text(s).is_none(), "“{s}” was parsed, and it should not have been");
        }
        // --- drag_advance: Shift divides the INCREMENT, not the position
        assert_eq!(drag_advance(85.0, 6.0, false), 91.0);
        assert_eq!(drag_advance(85.0, 6.0, true), 85.0 + 6.0 / SPEED_FINE_DIV);
        // a chain with Shift in the middle of a gesture does not teleport the value: it
        // continues from the same place where it was
        let mut vx = 85.0f32;
        for _ in 0..4 {
            vx = drag_advance(vx, 8.0, false);
        }
        let plain = vx;
        vx = drag_advance(vx, 8.0, true);
        assert!(
            vx > plain && vx - plain < 8.0,
            "Shift must slow the motion, not jump: {plain} → {vx}"
        );
        // --- acceleration hysteresis is RELATIVE
        assert!(boost_needs_update(1.05, 1.0, 1.0), "on a 1× base, 5 % must get there");
        assert!(!boost_needs_update(1.005, 1.0, 1.0), "on a 1× base, 0.5 % is noise");
        // the former absolute threshold 0.02 at base 0.05 is 40 % of the base:
        // the acceleration physically did not reach the engine
        assert!(
            boost_needs_update(0.06, 0.05, 0.05),
            "on a 0.05 base the double acceleration must reach the player"
        );
    }

    /// Test 47 (addition, regression of item 2 of round 5): the dnd position lives during
    /// hover and on the drop frame — the former unconditional take() erased it
    /// every frame, the drop always fell on the playhead, the silhouette was not drawn.
    #[test]
    fn t47_dnd_hover_survives_until_drop() {
        use super::dnd_keep_hover;
        assert!(dnd_keep_hover(true, false), "during hover — it persists");
        assert!(dnd_keep_hover(false, true), "on a drop frame (hover is already empty) — it persists");
        assert!(dnd_keep_hover(true, true));
        assert!(!dnd_keep_hover(false, false), "without hover and without a drop — it fades");
    }

    /// Test 74 (items 10–11 of R.7): the deferred-click machine for the preview —
    /// a single click waits for the double-click window and gives play/pause; a double
    /// cancels the single click and toggles detach.
    #[test]
    fn t74_preview_click_machine() {
        use super::{PendingOp as O, PreviewClick as C, preview_click_step};
        // click → arm, no action
        assert_eq!(preview_click_step(None, true, false, 300), (C::None, O::Arm));
        // window not yet expired → wait
        assert_eq!(preview_click_step(Some(100), false, false, 300), (C::None, O::Keep));
        // expired → play/pause
        assert_eq!(preview_click_step(Some(300), false, false, 300), (C::TogglePlay, O::Clear));
        assert_eq!(preview_click_step(Some(999), false, false, 300), (C::TogglePlay, O::Clear));
        // double click cancels the deferred single and detaches
        assert_eq!(preview_click_step(Some(120), false, true, 300), (C::ToggleDetach, O::Clear));
        // double without pending — also detach
        assert_eq!(preview_click_step(None, false, true, 300), (C::ToggleDetach, O::Clear));
        // no click and no pending — nothing
        assert_eq!(preview_click_step(None, false, false, 300), (C::None, O::Keep));
    }

    /// Test 92 (item 3 of round 10): project search — markers, media, tracks;
    /// case-insensitive, exact match ranks above substring.
    #[test]
    fn t92_search_hits() {
        use super::{HitKind, search_hits};
        use kt_model::*;
        use kt_time::Tick;
        let mut p = Project::new(ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        });
        p.media.push(MediaRef::new(MediaId(1), "/tmp/Interview.mp4"));
        p.markers.push(Marker::named(Tick(5000), "Intro"));
        p.markers.push(Marker::named(Tick(9000), "interview — start"));
        // the name of a track since R.62 lives on the object track
        p.track_at_mut(LaneKind::Audio, 0).unwrap().name = "Voice".into();
        let groups = vec![BinGroup {
            name: "City plans".into(),
            members: vec![MediaId(1)],
            locked: false,
        }];

        // empty query — empty result
        assert!(search_hits(&p, &groups, "   ").is_empty());

        // case does not matter; exact match first
        let hits = search_hits(&p, &groups, "intro");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, HitKind::Marker);
        assert_eq!(hits[0].at, 5000);

        // one substring finds both a marker and media
        let hits = search_hits(&p, &groups, "interview");
        assert!(hits.iter().any(|h| h.kind == HitKind::Marker));
        assert!(hits.iter().any(|h| h.kind == HitKind::Media));

        // a track by its user name and by its formula
        assert!(search_hits(&p, &groups, "voice").iter().any(|h| h.kind == HitKind::Track));
        assert!(search_hits(&p, &groups, "v1").iter().any(|h| h.kind == HitKind::Track));

        // no match
        assert!(search_hits(&p, &groups, "shchshch").is_empty());
        // bin groups are also searched by name (item 13, round 11)
        let hits = search_hits(&p, &groups, "plans");
        assert!(hits.iter().any(|h| h.kind == HitKind::Group), "group not found: {hits:?}");
        // an unnamed group does not appear in the results
        let anon = vec![BinGroup { name: String::new(), members: vec![MediaId(1)], locked: true }];
        assert!(search_hits(&p, &anon, "").is_empty());
    }

    /// Test 82 (item 16 of round 8): smart navigation — stop points and jumps.
    /// A cursor EXACTLY on a point moves further (does not stick), the chain of
    /// presses is monotonic, overlapping clips give one boundary.
    #[test]
    fn t82_stop_points_and_jumps() {
        use super::{next_stop, prev_stop, stop_points};
        use kt_model::*;
        use kt_time::Tick;
        let mut p = Project::new(ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        });
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        let clip = |id: u64, start: i64, dur: i64| ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(start),
            duration: Tick(dur),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        };
        // two OVERLAPPING clips: 1000..3000 and 2500..4000 → one
        // region 1000..4000; the joint inside is not counted as a boundary
        p.clips.push(clip(1, 1000, 2000));
        p.clips.push(clip(2, 2500, 1500));
        p.markers.push(Marker::at(Tick(5000)));
        p.region = Some(Region { start: Tick(7000), end: Tick(9000) });
        let pts = stop_points(&p);
        // points are aligned UP to the audio grid (a multiple of 15 ticks, item 7 of round 10):
        // a shift of ≤14 ticks = 0.15 ms, but a forward jump does not get stuck
        assert_eq!(pts, vec![0, 1005, 4005, 5010, 7005, 9000], "points: {pts:?}");
        assert!(pts.iter().all(|t| t % 15 == 0), "not aligned: {pts:?}");

        // cursor exactly on a point — move further, do not stick
        assert_eq!(prev_stop(5010, &pts), Some(4005));
        assert_eq!(next_stop(5010, &pts), Some(7005));
        // the chain of left presses is monotonic and without repeats
        let mut cur = 9000;
        let mut seen = vec![];
        while let Some(p) = prev_stop(cur, &pts) {
            assert!(p < cur);
            seen.push(p);
            cur = p;
        }
        assert_eq!(seen, vec![7005, 5010, 4005, 1005, 0]);
        // edges
        assert_eq!(prev_stop(0, &pts), None);
        assert_eq!(next_stop(9000, &pts), None);
        // empty project — only zero
        let empty = Project::new(ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
            pitch_engine: Default::default(),

            default_shape: Default::default(),

            conform: Default::default(),

            settings_explicit: false,
        });
        assert_eq!(stop_points(&empty), vec![0]);
    }

    /// Test 81 (item 4 of round 8): the shuttle's “second wind” — outside the widget
    /// the speed grows up to ±8×, symmetric by sign and monotonic.
    #[test]
    fn t81_overshoot_rate() {
        use super::{ENGINE_RATE_MAX, OVERSHOOT_PX_PER_DOUBLE as K, overshoot_rate};
        // inside the widget there is no multiplier
        assert_eq!(overshoot_rate(2.0, 0.0, K), 2.0);
        assert_eq!(overshoot_rate(-2.0, -5.0, K), -2.0);
        // exactly one doubling
        assert!((overshoot_rate(2.0, K, K) - 4.0).abs() < 1e-4);
        assert!((overshoot_rate(-2.0, K, K) + 4.0).abs() < 1e-4, "symmetry by sign");
        // engine ceiling
        assert_eq!(overshoot_rate(2.0, K * 10.0, K), ENGINE_RATE_MAX);
        assert_eq!(overshoot_rate(-2.0, K * 10.0, K), -ENGINE_RATE_MAX);
        // zero (stop) stays zero
        assert_eq!(overshoot_rate(0.0, K * 3.0, K), 0.0);
        // monotonicity
        let mut prev = 0.0;
        for i in 0..20 {
            let r = overshoot_rate(1.0, i as f32 * 10.0, K);
            assert!(r >= prev, "not monotonic at {i}");
            prev = r;
        }
    }

    /// Test 78 (item 14 of round 8): the single hint line — priorities:
    /// hover → mode → temporary message → idle line.
    #[test]
    fn t78_status_line_priority() {
        use super::{HintKind as K, STATUS_TTL_MS, status_line};
        let idle = "standby";
        // the impossibility of an action overrides even hover (item 5 of R.28)
        let (t, k) = status_line(
            Some("not allowed"),
            Some("hover"),
            Some("mode"),
            Some(("st", 0)),
            STATUS_TTL_MS,
            idle,
        );
        assert_eq!((t.as_str(), k), ("not allowed", K::Error));
        // hover overrides the rest
        let (t, k) =
            status_line(None, Some("hover"), Some("mode"), Some(("st", 0)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), ("hover", K::Hover));
        // mode overrides a temporary message
        let (t, k) = status_line(None, None, Some("mode"), Some(("st", 0)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), ("mode", K::Mode));
        // a temporary message lives until its TTL
        let (t, k) =
            status_line(None, None, None, Some(("st", STATUS_TTL_MS - 1)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), ("st", K::Status));
        // ...and yields to the idle line afterwards
        let (t, k) =
            status_line(None, None, None, Some(("st", STATUS_TTL_MS)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), (idle, K::Idle));
        // empty → idle line
        let (t, k) = status_line(None, None, None, None, STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), (idle, K::Idle));
    }

    /// Test 77 (items 11–13 of R.8, extends t64): window resize zones with TWO
    /// radii — corners are caught at a larger distance and take precedence over
    /// edges (previously the corner was 4×4 px and it was impossible to hit it).
    #[test]
    fn t77_resize_zones_edge_and_corner() {
        use super::{WIN_CORNER, WIN_EDGE, resize_zone};
        use eframe::egui::{Rect, ResizeDirection::*, pos2};
        let w = Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 300.0));
        let z = |x: f32, y: f32| resize_zone(w, pos2(x, y), WIN_EDGE, WIN_CORNER);
        // corners — by the WIN_CORNER radius, even far from the edge itself
        assert_eq!(z(12.0, 12.0), Some(NorthWest));
        assert_eq!(z(389.0, 14.0), Some(NorthEast));
        assert_eq!(z(10.0, 290.0), Some(SouthWest));
        assert_eq!(z(392.0, 288.0), Some(SouthEast));
        // edges — by the WIN_EDGE radius
        assert_eq!(z(3.0, 150.0), Some(West));
        assert_eq!(z(397.0, 150.0), Some(East));
        assert_eq!(z(200.0, 3.0), Some(North));
        assert_eq!(z(200.0, 297.0), Some(South));
        // past the edge line but not yet in a corner — nothing
        assert_eq!(z(200.0, 150.0), None, "center");
        assert_eq!(z(30.0, 150.0), None, "away from the edge");
        // exact boundaries
        assert_eq!(z(WIN_EDGE, 150.0), Some(West));
        assert_eq!(z(WIN_EDGE + 0.1, 150.0), None);
        assert_eq!(z(WIN_CORNER, WIN_CORNER), Some(NorthWest));
        assert_eq!(z(WIN_CORNER + 0.1, WIN_CORNER + 0.1), None);
        // outside the window
        assert_eq!(z(-1.0, 150.0), None);
    }

    /// Test 51 (item 7 of v1.4): state machine of preview detach — closing the window
    /// always returns the preview to the main area; the button toggles.
    #[test]
    fn t51_preview_detach_state() {
        use super::preview_detached_next as next;
        assert!(next(false, true, false), "the button detaches");
        assert!(!next(true, true, false), "the button returns");
        assert!(!next(true, false, true), "closing the window returns");
        assert!(!next(true, true, true), "closing is stronger than the button");
        assert!(!next(false, false, false), "rest");
        assert!(next(true, false, false), "stays detached");
    }

    /// Test 20-repaint (item 1): when idle NOT 60 fps.
    #[test]
    fn t20_idle_repaint_is_slow_playing_is_60fps() {
        assert_eq!(repaint_delay(true).as_millis(), 16);
        assert!(repaint_delay(false).as_millis() >= 100, "idle = rare tick");
    }
}
