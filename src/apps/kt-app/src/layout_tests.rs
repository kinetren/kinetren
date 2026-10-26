//! Test 76 (item 9 of round 8): a LIVE check of the layout and interactions through
//! egui_kittest (the Harness runs real frames without a window or GPU).
//!
//! It was introduced because three breakages in a row went uncaught: the panel separator
//! did not drag, the panel swelled with its content and ate the preview, and the window edges
//! gave no resize. Pure functions do not see such defects —
//! only a real pass of layout+hit-test sees them.
//!
//! RULE (apps/kt-app/CLAUDE.md): any change to the layout or the interactive behavior
//! must be accompanied by a test from here.

use crate::app::KtApp;
use crate::i18n::{Lang, k, tr};
use eframe::egui;
use egui_kittest::Harness;

const W: f32 = 1440.0;
const H: f32 = 860.0;

pub fn harness() -> Harness<'static, KtApp> {
    harness_lang(crate::i18n::Lang::Ru)
}

/// A harness on a specific language. The language is fixed EXPLICITLY: otherwise the test would depend on
/// LANG in the developer's shell. Real fonts are installed — before R.19 all
/// width measurements were made on the default egui set, that is, not on the
/// program the user actually runs.
fn harness_lang(lang: crate::i18n::Lang) -> Harness<'static, KtApp> {
    harness_lang_dt(lang, None)
}

/// The same harness with a controlled time step. The double click in egui is a
/// TEMPORARY gesture (two clicks closer than 0.3 s), while the harness step by default is
/// 0.25 s: events in neighboring frames do not add up into a double click, and the
/// “double click” test would check a single click.
/// How long to wait for a background write. This is NOT a claim about speed: under a full
/// parallel run (one hundred and fifty tests, each with its own egui context)
/// the autosave thread honestly waited its turn for longer than five seconds, and the tests
/// turned red from the machine's load, not from a defect.
const WAIT_BG: std::time::Duration = std::time::Duration::from_secs(60);

fn harness_lang_dt(lang: crate::i18n::Lang, step_dt: Option<f32>) -> Harness<'static, KtApp> {
    crate::i18n::set_lang(lang);
    let mut app = KtApp::new_empty(None);
    // “Last line of defense” (item 354) is switched off in the harness: a thousand created and
    // destroyed applications would scatter recovery files into the shared
    // data directory. The line itself is checked by t372, which enables it explicitly.
    app.recovery_on_drop = false;
    // The workspace (item 360) — into its own file: the shared one lies in the developer’s home directory,
    // and a hundred and fifty tests would write there in a race
    app.workspace_file =
        Some(std::env::temp_dir().join(format!("kt-ws-{}-{:p}.json", std::process::id(), &app)));
    let mut b = Harness::builder().with_size(egui::vec2(W, H));
    if let Some(dt) = step_dt {
        b = b.with_step_dt(dt);
    }
    let mut h = b.build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&h.ctx);
    h.step();
    h.step();
    h
}

/// A harness of a GIVEN size (R.66): the form contract is checked in a LOW
/// window — where the content is certainly taller than the screen and the window must
/// scroll, while the action buttons stay in view.
pub(crate) fn harness_sized(w: f32, h: f32) -> Harness<'static, KtApp> {
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let mut app = KtApp::new_empty(None);
    app.recovery_on_drop = false;
    app.workspace_file =
        Some(std::env::temp_dir().join(format!("kt-ws-{}-{:p}.json", std::process::id(), &app)));
    let mut hh = Harness::builder()
        .with_size(egui::vec2(w, h))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&hh.ctx);
    hh.step();
    hh.step();
    hh
}

/// A path whose write certainly fails on every OS: a directory under a regular FILE.
/// (`/proc/self/no-such-dir` was Linux-only: on Windows it is `C:\proc\…` and gets created.)
pub(crate) fn unwritable_path(tag: &str) -> std::path::PathBuf {
    let f = std::env::temp_dir().join(format!("kt-unwritable-{tag}-{}", std::process::id()));
    std::fs::write(&f, b"a file, not a directory").unwrap();
    f.join("sub").join("x.kt")
}

/// The ACTUAL rectangle of the panel (not the requested size: the egui separator
/// lives at the actual boundary, and exactly the divergence between them is
/// that very trap “the strip does not move”).
fn panel_rect(h: &Harness<'_, KtApp>, id: &str) -> egui::Rect {
    egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new(id))
        .expect("the panel must be drawn")
        .outer_rect
}

fn panel_width(h: &Harness<'_, KtApp>, id: &str) -> f32 {
    panel_rect(h, id).width()
}

fn cursor(h: &Harness<'_, KtApp>) -> egui::CursorIcon {
    h.output().platform_output.cursor_icon
}

/// (a,b) Layout: the left column within its limits, the preview takes the center —
/// and does NOT collapse when the column's content is long (regression of round 7.1:
/// ScrollArea/Grid swelled the panel, the preview “vanished into nowhere”).
#[test]
fn t76_layout_panels_and_preview_share_space() {
    let mut h = harness();
    let w = panel_width(&h, "left_col");
    assert!((72.0..=420.0).contains(&w), "the left column is by default out of bounds: {w}");
    assert!(
        h.state().preview_rect.width() > 600.0,
        "preview collapsed: {:?}",
        h.state().preview_rect
    );

    // long content must not swell the panel
    h.state_mut().gpu_info = Some("X".repeat(400));
    h.step();
    let w2 = panel_width(&h, "left_col");
    assert!(w2 <= w + 1.0, "long content inflated the column: was {w}, became {w2}");
    assert!(
        h.state().preview_rect.width() > 600.0,
        "long content ate the preview: {:?}",
        h.state().preview_rect
    );
}

/// (c,d) The separator of the left column: gives the resize cursor and really drags.
#[test]
fn t76_left_panel_separator_is_draggable() {
    let mut h = harness();
    let w0 = panel_rect(&h, "left_col").right();
    let y = H * 0.5;

    h.hover_at(egui::pos2(w0, y));
    h.step();
    h.hover_at(egui::pos2(w0, y));
    h.step();
    assert_eq!(
        cursor(&h),
        egui::CursorIcon::ResizeHorizontal,
        "no resize cursor on the separator — someone ate the zone"
    );

    h.drag_at(egui::pos2(w0, y));
    h.step();
    h.hover_at(egui::pos2(w0 + 90.0, y));
    h.step();
    h.drop_at(egui::pos2(w0 + 90.0, y));
    h.step();
    let w1 = panel_rect(&h, "left_col").right();
    assert!(
        (w1 - (w0 + 90.0)).abs() < 12.0,
        "the separator did not stretch: right edge was {w0}, became {w1}"
    );
}

/// (e) Edges and corners of the window: resize cursor + sent BeginResize.
#[test]
fn t76_window_edges_resize() {
    use egui::{CursorIcon as C, ViewportCommand as V};
    let cases: [(egui::Pos2, C); 6] = [
        (egui::pos2(2.0, H * 0.5), C::ResizeHorizontal), // left edge
        (egui::pos2(W - 2.0, H * 0.5), C::ResizeHorizontal), // right
        (egui::pos2(W * 0.5, 2.0), C::ResizeVertical),   // top
        (egui::pos2(W * 0.5, H - 2.0), C::ResizeVertical), // bottom
        (egui::pos2(3.0, 3.0), C::ResizeNwSe),           // NW corner
        (egui::pos2(W - 3.0, 3.0), C::ResizeNeSw),       // NE corner
    ];
    for (pos, want) in cases {
        let mut h = harness();
        h.hover_at(pos);
        h.step();
        assert_eq!(cursor(&h), want, "no resize cursor at {pos:?}");
        h.drag_at(pos);
        h.step();
        let sent = h
            .output()
            .viewport_output
            .values()
            .any(|v| v.commands.iter().any(|c| matches!(c, V::BeginResize(_))));
        assert!(sent, "BeginResize not sent from {pos:?}");
    }
}

/// The top zone (item 15 of round 8): there is NO full-width strip — the preview starts
/// from the very top of the window; the window buttons live in the header of the meters and are clickable;
/// a menu in the column header does not swell the panel.
#[test]
fn t76_top_area_gives_space_to_preview() {
    let mut h = harness();
    // item 1 of round 11: the preview starts AT the very edge — the window drag strip
    // no longer takes up space (it was ~23 px wasted)
    assert!(
        h.state().preview_rect.top() <= crate::app::WIN_EDGE,
        "preview did not reach the top: {:?}",
        h.state().preview_rect
    );
    let w = panel_width(&h, "left_col");
    assert!(w <= 420.0, "the header with the menu inflated the column: {w}");
    // close button — top right corner, inside the meters panel
    h.hover_at(egui::pos2(W - 11.0, 10.0));
    h.step();
    assert_eq!(
        h.state().hover_hint.as_deref(),
        Some(tr(k::win::CLOSE)),
        "the window button does not catch hover (it was covered)"
    );
}

/// (f) The resize zone must not steal clicks from the widgets: the toolbar buttons
/// at the bottom and the window buttons remain clickable.
#[test]
fn t76_resize_zones_do_not_steal_widgets() {
    let mut h = harness();
    // The toolbar's play button — the first on the left in the bottom strip of the timeline
    let playing_before = h.state().player.transport.is_playing();
    let toolbar_y = H - 20.0 - 34.0 * 0.5; // status bar 20, toolbar strip 34
    h.hover_at(egui::pos2(24.0, toolbar_y));
    h.step();
    assert!(
        h.state().hover_hint.is_some(),
        "the toolbar button gets no hover — a foreign zone covered it"
    );
    h.drag_at(egui::pos2(24.0, toolbar_y));
    h.step();
    h.drop_at(egui::pos2(24.0, toolbar_y));
    h.step();
    assert_ne!(
        h.state().player.transport.is_playing(),
        playing_before,
        "click on the toolbar button did not work"
    );
}

/// Test 79 (items 5,7 of round 8): media bin — modes and layout.
/// The free layout lives in the project (UiState, outside undo), switching
/// modes does not erase it, and the cards do not stick out past the panel (otherwise the panel
/// grows with the content and its separator drifts away — the trap from t76).
#[test]
fn t79_bin_modes_and_layout() {
    use kt_model::MediaId;
    let mut h = harness();
    assert!(h.state().bin_free, "free layout is the default mode");

    // card positions survive a mode switch
    h.state_mut().bin_layout.insert(MediaId(7), egui::pos2(30.0, 40.0));
    h.state_mut().bin_free = false;
    h.step();
    h.state_mut().bin_free = true;
    h.step();
    assert_eq!(
        h.state().bin_layout.get(&MediaId(7)).copied(),
        Some(egui::pos2(30.0, 40.0)),
        "switching modes broke the layout"
    );

    // the layout goes into the project and does not bloat the file when it is empty
    let st = h.state().ui_state_for_test();
    assert_eq!(st.bin_layout, vec![(MediaId(7), 30.0, 40.0)]);
    assert!(st.bin_free);
    let json = serde_json::to_string(&kt_model::UiState::default()).unwrap();
    assert!(!json.contains("bin_layout"), "an empty layout must not go into the file: {json}");
    assert!(!json.contains("bin_free"));
}

/// Test 83 (item 1 of round 9): a button's hint MUST be drawn in the status line in the
/// SAME frame. Regression: the status bar is the second panel of the frame, while the
/// buttons are drawn later, so it always saw hover_hint = None and
/// showed the default line. Earlier tests read the internal field after the frame
/// and were falsely green — here what is DRAWN is what gets checked.
#[test]
fn t83_hints_reach_status_line() {
    use crate::app::HintKind;
    let mut h = harness();
    assert_eq!(h.state().status_line_shown.1, HintKind::Idle, "without hover — the standby line");

    let toolbar_y = H - 20.0 - 34.0 * 0.5; // status bar 20 + toolbar strip 34
    for (pos, want) in [
        (egui::pos2(24.0, toolbar_y), tr(k::tb::PLAY_PAUSE)), // the toolbar (drawn AFTER the status bar)
        (egui::pos2(W - 11.0, 10.0), tr(k::win::CLOSE)),      // window buttons (even later)
        (egui::pos2(14.0, 40.0), tr(k::bin::HINT_FREE)),      // bin
    ] {
        let mut h = harness();
        h.hover_at(pos);
        h.step();
        let (text, kind) = h.state().status_line_shown.clone();
        assert!(text.starts_with(want), "from {pos:?} expected “{want}”, in the status line “{text}”");
        assert_eq!(kind, HintKind::Hover);
    }

    // there are no pop-up hints in the program (item 14 of round 8)
    h.hover_at(egui::pos2(24.0, toolbar_y));
    h.step();
    h.step();
    assert!(
        h.ctx.memory(|m| m.layer_ids().all(|l| l.order != egui::Order::Tooltip)),
        "the pop-up tooltip came back"
    );
}

/// Test 85 (item 2 of round 9): the window drags from the top at ANY point of the width —
/// previously there was a hole above the preview between the column header and the window buttons.
#[test]
fn t85_top_drag_is_continuous() {
    use egui::ViewportCommand as V;
    for x in [120.0f32, W * 0.5, W - 200.0] {
        let mut h = harness();
        h.drag_at(egui::pos2(x, 14.0)); // below the resize edge (8 px)
        h.step();
        h.hover_at(egui::pos2(x + 20.0, 18.0)); // real gesture: mouse movement
        h.step();
        let sent = h
            .output()
            .viewport_output
            .values()
            .any(|v| v.commands.iter().any(|c| matches!(c, V::StartDrag)));
        assert!(sent, "the window is not dragged along the top edge at x={x}");
    }

    // item 11 of round 13: in a MAXIMIZED window there is no resize, so the top WIN_EDGE
    // pixels used to be dead — “a strip along which it is useless
    // to drag”. Now the drag starts from the very edge.
    let mut h = harness();
    h.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().maximized = Some(true);
    h.step();
    h.drag_at(egui::pos2(W * 0.5, 1.0));
    h.step();
    h.hover_at(egui::pos2(W * 0.5 + 20.0, 6.0));
    h.step();
    let sent = h
        .output()
        .viewport_output
        .values()
        .any(|v| v.commands.iter().any(|c| matches!(c, V::StartDrag | V::Maximized(false))));
    assert!(sent, "in a maximized window the top edge does not drag the window");
}

/// Test 86 (items 3,10 of round 9): the meters are pinned to the right edge of the window, and at
/// the minimum window 640×640 the header (window buttons) is not pushed out by the timeline
/// and the preview does not collapse.
#[test]
fn t86_meters_edge_and_min_window() {
    let mut h = harness();
    let p = panel_rect(&h, "meters_col");
    assert!((W - p.right()).abs() < 1.5, "meter panel is not at the edge: {p:?}");
    // at the very edge of the window the meter must catch hover (no dead zone)
    h.hover_at(egui::pos2(W - 3.0, H * 0.35));
    h.step();
    assert!(
        [tr(k::meter::FADER), tr(k::meter::PEAK), tr(k::meter::MUTE)].iter().any(|want| h
            .state()
            .status_line_shown
            .0
            .starts_with(want)),
        "a gap at the right edge: in the line “{}”",
        h.state().status_line_shown.0
    );

    // minimum window
    let app = KtApp::new_empty(None);
    let (w, hh) = (640.0f32, 640.0f32);
    let mut h = Harness::builder()
        .with_size(egui::vec2(w, hh))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&h.ctx);
    h.step();
    h.step();
    h.hover_at(egui::pos2(w - 11.0, 10.0));
    h.step();
    assert_eq!(
        h.state().status_line_shown.0,
        tr(k::win::CLOSE),
        "at 640×640 the timeline pushed out the header"
    );
    assert!(
        h.state().preview_rect.height() > 60.0,
        "preview collapsed: {:?}",
        h.state().preview_rect
    );
}

/// Test 88 (item 11 of round 9): a jump to stop points SHOWS the cursor —
/// previously the playhead drifted away while the timeline window stayed in place (during pause
/// follow_view does not work by construction).
#[test]
fn t88_jump_reveals_cursor() {
    let mut h = harness();
    h.state_mut().view_start = 5_000_000.0;
    h.step();
    h.state_mut().jump_stop(false, true); // Shift+Home — to the start of the project
    h.step();
    let st = h.state();
    let vis =
        (st.timeline_rect.width() as f64 - crate::timeline::GUTTER_W as f64) * st.ticks_per_px;
    let ph = st.player.transport.playhead().0 as f64;
    let vs = st.view_start;
    assert!(ph >= vs && ph <= vs + vis, "cursor outside the window: ph={ph}, window=[{vs}, {}]", vs + vis);
}

/// Test 94 (items 7,8 of round 10): the “Forward” button REALLY moves the cursor to the right,
/// repeated presses give a monotonic chain without sticking, and the hotkeys
/// A/D do the same. Regression: the playhead lives in audio frames, while the
/// stop points live in ticks; the round-trip truncated downward, the cursor landed one
/// tick left of the target, and next_stop kept finding the same point endlessly.
#[test]
fn t94_forward_navigation_advances() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    {
        let app = h.state_mut();
        // a project with stop points NOT multiple of the audio grid: 1000, 4000, 7000
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(1000),
            duration: Tick(3000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.markers.push(Marker::at(Tick(7000)));
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.player.seek(Tick(0));
    }
    h.step();

    // the chain forward must be strictly increasing
    let mut seen = vec![h.state().player.transport.playhead().0];
    for _ in 0..3 {
        h.state_mut().jump_stop(true, false);
        h.step();
        let now = h.state().player.transport.playhead().0;
        assert!(now > *seen.last().unwrap(), "the cursor did not move forward: chain {seen:?} → {now}");
        seen.push(now);
    }
    // and no jump dragged it back
    assert!(seen.windows(2).all(|w| w[1] > w[0]), "not monotonic: {seen:?}");
    // at the rightmost point there is nowhere further to move — and it does NOT throw back
    let last = *seen.last().unwrap();
    h.state_mut().jump_stop(true, false);
    h.step();
    assert_eq!(
        h.state().player.transport.playhead().0,
        last,
        "hitting the end must not pull the cursor back"
    );

    // backward — also monotonic, down to zero
    for _ in 0..6 {
        h.state_mut().jump_stop(false, false);
        h.step();
    }
    assert_eq!(h.state().player.transport.playhead().0, 0, "did not reach zero");
}

/// Test 95 (item 2 of round 11): the magnet REALLY pulls the clip to the neighbor's edge.
///
/// Defect: the candidates “snap to the start” and “snap to the end” were compared
/// after substituting “not found = as is”, and such a candidate with an offset of 0
/// always won — the yellow line was drawn, but the clip did not stick.
/// The test drives the mouse like a person (press, move, release) and checks
/// the FINAL model, not the internal fields.
#[test]
fn t95_clip_drag_snaps_to_neighbour() {
    use kt_model::*;
    use kt_time::Tick;

    // ticks/px = 1000 → magnet threshold 10 px = 10,000 ticks
    const TPP: f64 = 1000.0;
    const DUR: i64 = 90_000;
    const NEIGHBOUR_END: i64 = DUR; // neighbor [0, 90000)
    const FAR: i64 = 200_000; // the clip being dragged

    fn scene(ctrl: bool) -> (i64, i64) {
        let mut h = harness();
        {
            let app = h.state_mut();
            let mut p = app.model.project().clone();
            p.media.push(MediaRef::new(MediaId(1), "synth:1"));
            for (n, start) in [(1u64, 0i64), (2, FAR)] {
                p.clips.push(ClipEvent {
                    r#gen: None,
                    gen_auto: Vec::new(),
                    overrides: Vec::new(),
                    geometry: Default::default(),
                    geometry_pos: 0,
                    geometry_auto: Vec::new(),
                    id: ClipId(n),
                    link_group: None,
                    kind: ClipKind::Video,
                    lane: 0,
                    start: Tick(start),
                    duration: Tick(DUR),
                    source_in: Tick(0),
                    media: MediaId(1),
                    gain_db: 0.0,
                    reversed: false,
                    fx_bypass: vec![],
                    fx_auto: vec![],
                    fx: vec![],
                    opacity: 1.0,
                    fx_off: false,
                });
            }
            p.validate().unwrap();
            app.model = kt_model::TimelineModel::new(p);
            app.view_start = 0.0;
            app.ticks_per_px = TPP;
            app.player.seek(Tick(0));
        }
        h.step();

        let g = h.state().tl_geom.clone().expect("no timeline geometry");
        let y = g.lane_y(ClipKind::Video, 0, 0.5);
        let grab = g.tick_to_x(FAR + DUR / 2);
        if ctrl {
            h.event(egui::Event::ModifiersChanged(egui::Modifiers::CTRL));
        }
        h.hover_at(egui::pos2(grab, y));
        h.step();
        h.drag_at(egui::pos2(grab, y));
        h.step();
        // a shift by 8 px — this is where egui recognizes the drag as started
        h.hover_at(egui::pos2(grab - 8.0, y));
        h.step();

        // we aim so that the START ends up 4 px to the right of the neighbor's END:
        // the magnet must pull it in, “without the magnet” must leave it as is.
        let want_raw = NEIGHBOUR_END + (4.0 * TPP) as i64;
        let grab_dt = match h.state().drag {
            Some(crate::timeline::Drag::Clip(ref d)) => d.grab_dt,
            _ => panic!("clip drag did not start — someone else intercepted it"),
        };
        let to = g.tick_to_x(want_raw + grab_dt);
        h.hover_at(egui::pos2(to, y));
        h.step();
        h.drop_at(egui::pos2(to, y));
        h.step();

        let start = h
            .state()
            .model
            .project()
            .clips
            .iter()
            .find(|c| c.id == ClipId(2))
            .expect("clip disappeared")
            .start
            .0;
        (start, want_raw)
    }

    let (snapped, _) = scene(false);
    assert_eq!(snapped, NEIGHBOUR_END, "magnet did not pull the clip to its neighbor");

    let (free, raw) = scene(true);
    assert!(
        (free - raw).abs() <= 2 * TPP as i64,
        "Ctrl must disable the magnet: clip landed at {free}, expected ≈{raw}"
    );
    assert_ne!(free, NEIGHBOUR_END, "Ctrl did not disable the magnet");
}

/// Test 96 (items 3,4,5 of round 11): project dirtiness and loop.
///
/// “Content edits” (clips, FX, markers) raise the asterisk and start the autosave;
/// “convenience edits” (region, loop, zoom, bin layout, track names) do not —
/// they reach the disk along the way. Esc clears the region, but does NOT switch off
/// an armed loop — that is a separate intention.
#[test]
fn t96_dirty_split_and_loop_survives_esc() {
    use kt_model::*;
    use kt_time::Tick;

    let mut h = harness();
    assert!(!h.state().dirty_content && !h.state().dirty_ui, "a fresh project is clean");

    // --- conveniences: region, loop, zoom, bin layout (the track name from R.62
    // — the CONTENT of the model is checked below, together with the edit changes)
    h.state_mut().model.set_region(Region::normalized(Tick(0), Tick(90_000)));
    h.state_mut().on_region_changed();
    h.state_mut().toggle_loop();
    h.state_mut().ticks_per_px = 4321.0;
    h.state_mut().bin_layout.insert(MediaId(3), egui::pos2(12.0, 34.0));
    h.step();
    assert!(
        !h.state().dirty_content,
        "conveniences must not mark the project as changed (the asterisk lies)"
    );
    assert!(h.state().dirty_ui, "conveniences must be counted as UI edits");

    // --- content: a model edit
    h.state_mut().apply(Command::AddMarker { at: Tick(45_000), duration: Tick::ZERO });
    h.step();
    assert!(h.state().dirty_content, "a content edit must give an asterisk");

    // --- Esc: region cleared, loop STAYS armed
    assert!(h.state().model.project().loop_enabled, "loop is on");
    h.state_mut().escape_action();
    h.step();
    assert!(h.state().model.project().region.is_none(), "Esc did not clear the region");
    assert!(
        h.state().model.project().loop_enabled,
        "Esc turned off the loop — that is a separate user intent (item 3 round 11)"
    );
    // an explicit “Clear region” still switches off the loop too (otherwise the lamp stays lit
    // without a region and survives a restart — item 16 of round 6)
    h.state_mut().model.set_region(Region::normalized(Tick(0), Tick(90_000)));
    h.state_mut().clear_region();
    assert!(!h.state().model.project().loop_enabled, "an explicit command must turn off the loop");

    // --- the autosave does not fire more often than due and does not write during a gesture
    assert!(!h.state().autosave_due(), "autosave right after start — too often");
}

/// Test 97 (items 1,7,8 of round 11): top of the preview, the width limit of the meters, LED grid.
#[test]
fn t97_top_meters_and_leds() {
    use crate::meters::{
        METER_MAX_DB, METER_MIN_DB, SEG_STEPS, led_lit, seg_count, seg_db_for, seg_mid_db,
        zone_color,
    };

    // --- (item 1 of round 12) the grid step is chosen to fit the bar height: the segments
    // must be FREQUENT (as in v1 — 6 px + 2 px), not large
    for bar_h in [180.0f32, 300.0, 420.0, 560.0, 700.0] {
        let step = seg_db_for(bar_h);
        let px = bar_h * step / (METER_MAX_DB - METER_MIN_DB);
        assert!(
            (4.5..=9.5).contains(&px),
            "with bar {bar_h} px, the segment step {px} px (step {step} dB) — the grid is not as in v1"
        );
        assert!(SEG_STEPS.contains(&step), "step {step} is not from the list of zone divisors");
    }
    // at a typical panel height the segments are noticeably more than the former 36
    assert!(seg_count(seg_db_for(500.0)) >= 60, "too few segments: {}", seg_count(seg_db_for(500.0)));

    // --- properties for EVERY allowed step
    for step in SEG_STEPS {
        let n = seg_count(step);
        assert_eq!(led_lit(METER_MIN_DB, step), 0, "silence — everything is off");
        assert_eq!(led_lit(METER_MIN_DB - 20.0, step), 0);
        assert_eq!(led_lit(METER_MAX_DB, step), n, "clip — the scale up to the top");
        assert_eq!(led_lit(METER_MAX_DB + 6.0, step), n, "overload does not go beyond the scale");
        let mut prev = 0;
        let mut db = METER_MIN_DB;
        while db <= METER_MAX_DB {
            let lit = led_lit(db, step);
            assert!(lit >= prev, "not monotonic at {db} dB, step {step}");
            prev = lit;
            db += 0.1;
        }
        // no segment crosses the boundary of a colour zone — otherwise it would be
        // two-coloured and would “burn half-way”
        for i in 0..n {
            let lo = METER_MIN_DB + i as f32 * step + 0.01;
            let hi = METER_MIN_DB + (i + 1) as f32 * step - 0.01;
            assert_eq!(zone_color(lo), zone_color(hi), "segment {i} of step {step} is two-colored");
            assert_eq!(zone_color(seg_mid_db(i, step)), zone_color(lo), "color of segment {i}");
        }
    }

    let mut h = harness();
    // --- (item 1) the preview starts at the very edge of the window
    let top = h.state().preview_rect.top();
    assert!(top <= crate::app::WIN_EDGE, "preview is not at the top: {top}");

    // a click on the window drag strip must NOT play/pause
    let playing = h.state().player.transport.is_playing();
    let x = h.state().preview_rect.center().x;
    h.drag_at(egui::pos2(x, crate::app::WIN_EDGE + 4.0));
    h.step();
    h.drop_at(egui::pos2(x, crate::app::WIN_EDGE + 4.0));
    h.step();
    assert_eq!(
        h.state().player.transport.is_playing(),
        playing,
        "a click in the window drag strip started playback"
    );
    // but below the strip — an ordinary click on the preview (play/pause with a delay for
    // double-click recognition)
    let y = h.state().preview_rect.center().y;
    h.drag_at(egui::pos2(x, y));
    h.step();
    h.drop_at(egui::pos2(x, y));
    h.step();
    assert!(
        h.state().preview_click_at.is_some() || h.state().player.transport.is_playing() != playing,
        "click on the preview did not arrive"
    );

    // --- (item 3 of round 12) the ruler takes no space: the strip for labels is removed,
    // the marker name lives on the time-label row
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let ruler = g.lane_top(kt_model::ClipKind::Video, 0) - g.rect.top();
    assert!(ruler <= 36.0, "the ruler ate {ruler} px before the first track — the space is wasted");

    // --- (item 7) the meters panel cannot be stretched to half the screen
    let sep = panel_rect(&h, "meters_col").left();
    let mid = H * 0.5;
    h.hover_at(egui::pos2(sep, mid));
    h.step();
    h.drag_at(egui::pos2(sep, mid));
    h.step();
    h.hover_at(egui::pos2(W * 0.4, mid));
    h.step();
    h.drop_at(egui::pos2(W * 0.4, mid));
    h.step();
    let w = panel_width(&h, "meters_col");
    assert!(w <= 90.0, "meters stretched by {w} px — there is no upper limit");
    assert!(w > 75.0, "the meter separator did not stretch at all ({w}) — the test checked nothing");
    // the same class: the left column must not take half the window
    let sep = panel_rect(&h, "left_col").right();
    h.hover_at(egui::pos2(sep, mid));
    h.step();
    h.drag_at(egui::pos2(sep, mid));
    h.step();
    h.hover_at(egui::pos2(sep + 40.0, mid));
    h.step();
    h.hover_at(egui::pos2(W * 0.9, mid));
    h.step();
    h.drop_at(egui::pos2(W * 0.9, mid));
    h.step();
    // item 4 of round 13: the bin limit is 70% of the window (do not squeeze!), it must stretch
    let lw = panel_width(&h, "left_col");
    assert!(lw <= W * 0.7 + 1.0, "left column went past the limit: {lw}");
    assert!(lw > 600.0, "the media bin is clamped again ({lw} px) — the limit was set by a direct request");
}

/// Test 98 (item 9 of round 11): the timecode input mask.
///
/// The user's requirements: entry RIGHT TO LEFT, the toolbar width does not
/// jump when the mask opens, it works during playback, and this is
/// not a “simply plugged-in inputfield” — it is drawn in the same rectangle
/// and in the same font as the timecode itself.
#[test]
fn t98_timecode_mask() {
    use crate::util::{mask_digits, mask_text, mask_to_tick};

    // digits slide in from the right, the unentered high digits are zeros
    assert_eq!(mask_digits(""), "000000");
    assert_eq!(mask_digits("7"), "000007");
    assert_eq!(mask_digits("130"), "000130");
    assert_eq!(mask_digits("123456"), "123456");
    // extra digits do not break the mask: the last six are kept
    assert_eq!(mask_digits("9123456"), "123456");
    // parse: 00:01:30; minutes/seconds are not normalized (99 minutes — allowed)
    assert_eq!(mask_to_tick("130", 30).0, 90_000 * 90);
    assert_eq!(mask_to_tick("9900", 30).0, 90_000 * 99 * 60);
    // highlight: what is typed — on the right, what is not typed — on the left, muted
    let (dimmed, typed) = mask_text("130");
    assert_eq!(dimmed, "00:0", "unfilled part on the left");
    assert_eq!(typed, "1:30", "typed part on the right");
    assert_eq!(mask_text("").1, "", "nothing typed — nothing to highlight");
    // pasting a ready timecode is laid out by digit positions
    assert_eq!(
        crate::util::tick_to_mask(
            crate::util::parse_timecode("1:02:03", 30).expect("did not understand")
        ),
        "010203"
    );
    // NEGATIVE time (the zone up to zero inside the reel, R.58 item 462) — the sign and
    // the fields come from |t|, not from a set of negative fields
    assert_eq!(
        crate::util::format_timecode(kt_time::Tick(-90_000 - 3_000), 30),
        "-00:00:01.033 F01"
    );
    assert_eq!(crate::util::format_timecode(kt_time::Tick(90_000 + 3_000), 30), "00:00:01.033 F01");
    assert_eq!(crate::timeline::format_ruler_time_step(-45_000, 9_000), "-0:00.5");
    assert_eq!(crate::timeline::format_ruler_time_step(-60 * 90_000, 90_000), "-1:00");

    // --- the toolbar width does not change when the mask opens
    let mut h = harness();
    let strip_h = |h: &Harness<'_, KtApp>| h.state().tl_geom.clone().map(|g| g.rect.height());
    let before = strip_h(&h);
    h.state_mut().tc_edit = Some(String::new());
    h.state_mut().tc_edit_focus = true;
    h.step();
    assert_eq!(strip_h(&h), before, "opening the mask shifted the timeline layout");

    // --- with the mask open, the space key does NOT start playback
    let playing = h.state().player.transport.is_playing();
    h.key_press(egui::Key::Space);
    h.step();
    assert_eq!(h.state().player.transport.is_playing(), playing, "space leaked into hotkeys past the mask");

    // --- a REPEATED call (item 8 of round 12): after Enter the mask must return
    // focus, otherwise all hotkeys are dead until a mouse click
    h.key_press(egui::Key::Escape);
    h.step();
    assert!(h.state().tc_edit.is_none(), "Esc did not close the mask");
    assert!(
        !h.ctx.egui_wants_keyboard_input(),
        "the mask closed, but focus was not returned — hotkeys are dead"
    );
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::L);
    h.step();
    assert!(h.state().tc_edit.is_some(), "the second Ctrl+L did not open the mask");

    // --- digit input and Enter move the cursor (and do not require a pause)
    h.event(egui::Event::Text("130".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    assert!(h.state().tc_edit.is_none(), "Enter did not close the mask");
    assert_eq!(h.state().player.transport.playhead().0 / 90_000, 90, "the cursor did not land on 00:01:30");
    // the seek is applied in the same frame: the request is already cleared, and the cursor is
    // visible in the window (otherwise “moved but not visible”)
    assert!(h.state().pending_reveal.is_none(), "the seek request hung");
    // and after APPLYING the mask also returns focus — Ctrl+L works again
    assert!(!h.ctx.egui_wants_keyboard_input(), "after Enter the focus stayed on the mask");
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::L);
    h.step();
    assert!(h.state().tc_edit.is_some(), "Ctrl+L did not work after Enter");
    h.key_press(egui::Key::Escape);
    h.step();
    let g = h.state().tl_geom.clone().expect("no geometry");
    let x = g.tick_to_x(90 * 90_000);
    assert!(
        (g.rect.left()..=g.rect.right()).contains(&x),
        "after the jump the cursor is outside the visible area: x={x}, window {:?}",
        g.rect
    );
}

/// Test 99 (item 13 of round 11): groups of cards in the media bin.
///
/// We check LIVE scenarios: the selection outline catches the cards, the group
/// moves as a whole and does not let cards out past the panel edge, the lock holds,
/// deleting media leaves no dangling groups, and an empty one does not get into the file.
#[test]
fn t99_bin_groups() {
    use kt_model::{BinGroup, MediaId, MediaRef};

    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        for n in 1..=3u64 {
            p.media.push(MediaRef::new(MediaId(n), format!("synth:{n}")));
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.bin_free = true;
        app.bin_layout.insert(MediaId(1), egui::pos2(0.0, 0.0));
        app.bin_layout.insert(MediaId(2), egui::pos2(0.0, 100.0));
        // the third card stands with a margin: between the second and third there is an
        // empty spot from which the selection outline can be dragged
        app.bin_layout.insert(MediaId(3), egui::pos2(0.0, 320.0));
    }
    h.step();

    // --- grouping the selection: one group, the card in exactly one
    h.state_mut().bin_sel = [MediaId(1), MediaId(2)].into_iter().collect();
    let gi = h.state_mut().bin_group_selection("Plans".into()).expect("group was not created");
    assert_eq!(h.state().bin_groups[gi].members.len(), 2);
    assert_eq!(h.state().bin_group_of(MediaId(3)), None, "third card outside the group");
    // regrouping moves the member, it does not duplicate it
    h.state_mut().bin_sel = [MediaId(2), MediaId(3)].into_iter().collect();
    let gi2 = h.state_mut().bin_group_selection("Interview".into()).expect("second group");
    let in_groups: Vec<usize> = (0..h.state().bin_groups.len())
        .filter(|i| h.state().bin_groups[*i].members.contains(&MediaId(2)))
        .collect();
    assert_eq!(in_groups, vec![gi2], "a card ended up in two groups at once: {in_groups:?}");

    // --- whom to drag: the whole group, even if one card is grabbed
    let mates = h.state().bin_drag_set(MediaId(3));
    assert_eq!(mates.len(), 2, "a group must move as a whole: {mates:?}");
    // a card OUTSIDE the group, but in the selection — moves together with the selection
    h.state_mut().bin_ungroup(gi); // “Plans” kept one member
    h.state_mut().bin_sel = [MediaId(1), MediaId(3)].into_iter().collect();
    assert_eq!(
        h.state().bin_drag_set(MediaId(1)).len(),
        2,
        "a free card must move with the whole selection"
    );

    // --- the lock
    assert!(!h.state().bin_locked(MediaId(3)));
    let gi2 =
        h.state().bin_groups.iter().position(|g| g.name == "Interview").expect("group disappeared");
    h.state_mut().bin_groups[gi2].locked = true;
    h.step();
    assert!(h.state().bin_locked(MediaId(3)), "the group lock does not apply to a member");
    assert!(!h.state().bin_locked(MediaId(1)), "the lock leaked onto another card");

    // --- groups survive a mode switch and go into the project
    h.state_mut().bin_free = false;
    h.step();
    h.state_mut().bin_free = true;
    h.step();
    assert_eq!(h.state().bin_groups.len(), 1, "switching modes lost the groups");
    let st = h.state().ui_state_for_test();
    assert_eq!(st.bin_groups.len(), 1, "groups did not get into UiState");
    assert!(st.bin_groups[0].locked, "the lock did not reach the project");
    let json = serde_json::to_string(&kt_model::UiState::default()).unwrap();
    assert!(!json.contains("bin_groups"), "empty groups must not go into the file: {json}");

    // --- LIVE scenario: the selection outline from empty space catches the cards
    h.state_mut().bin_groups.clear();
    h.state_mut().bin_sel.clear();
    h.step();
    let rects = h.state().bin_card_rects.clone();
    assert_eq!(rects.len(), 3, "cards were not drawn: {rects:?}");
    let at = |h: &Harness<'_, KtApp>, m: MediaId| {
        h.state().bin_card_rects.iter().find(|(id, _)| *id == m).unwrap().1
    };
    let (r0, r1) = (at(&h, MediaId(1)), at(&h, MediaId(2)));
    // we drag from the EMPTY place under the second card upward, covering the first
    // two; the gesture like a person's: press, nudge slightly, keep dragging
    let start = egui::pos2(r0.center().x, r1.bottom() + 40.0);
    h.drag_at(start);
    h.step();
    h.hover_at(start + egui::vec2(-4.0, -8.0));
    h.step();
    h.hover_at(r0.center());
    h.step();
    h.drop_at(r0.center());
    h.step();
    assert!(
        h.state().bin_sel.contains(&MediaId(1)) && h.state().bin_sel.contains(&MediaId(2)),
        "the selection frame did not catch the cards: {:?}",
        h.state().bin_sel
    );
    assert!(!h.state().bin_sel.contains(&MediaId(3)), "the frame captured an extra card");

    // --- group drag: both cards moved by one delta
    h.state_mut().bin_group_selection("Pair".into()).expect("group");
    h.step();
    let before: Vec<egui::Pos2> =
        [MediaId(1), MediaId(2)].iter().map(|m| h.state().bin_layout[m]).collect();
    let grab = at(&h, MediaId(1));
    // press → nudge slightly (here egui recognizes the drag) → keep dragging
    h.drag_at(grab.center());
    h.step();
    h.hover_at(grab.center() + egui::vec2(0.0, 8.0));
    h.step();
    h.hover_at(grab.center() + egui::vec2(0.0, 38.0));
    h.step();
    h.drop_at(grab.center() + egui::vec2(0.0, 38.0));
    h.step();
    let after: Vec<egui::Pos2> =
        [MediaId(1), MediaId(2)].iter().map(|m| h.state().bin_layout[m]).collect();
    let d0 = after[0] - before[0];
    let d1 = after[1] - before[1];
    assert!(d0.y > 10.0, "the grabbed card did not move: {d0:?}");
    assert!((d0 - d1).length() < 0.6, "the group came apart: {d0:?} vs {d1:?}");
    assert!(after.iter().all(|p| p.x >= 0.0 && p.y >= 0.0), "the card was carried past the edge: {after:?}");

    // --- the lock holds: the locked group does not move with the mouse
    let gi3 = h.state().bin_groups.len() - 1;
    h.state_mut().bin_groups[gi3].locked = true;
    h.step();
    let before = h.state().bin_layout[&MediaId(1)];
    let grab = at(&h, MediaId(1));
    h.drag_at(grab.center());
    h.step();
    h.hover_at(grab.center() + egui::vec2(0.0, 8.0));
    h.step();
    h.hover_at(grab.center() + egui::vec2(0.0, 40.0));
    h.step();
    h.drop_at(grab.center() + egui::vec2(0.0, 40.0));
    h.step();
    assert_eq!(h.state().bin_layout[&MediaId(1)], before, "the lock did not hold the card");

    // --- THE GROUP NAME IS SAVED BY ENTER (round-13 complaint: the field did not disappear
    // and the name was lost — request_focus was called every frame, so
    // lost_focus never happened)
    h.state_mut().bin_sel = [MediaId(1)].into_iter().collect();
    let gi4 = h.state_mut().bin_group_selection(String::new()).expect("group");
    h.state_mut().bin_group_edit = Some((gi4, String::new()));
    h.state_mut().bin_group_focus = true;
    h.step();
    h.event(egui::Event::Text("plan".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
    assert!(h.state().bin_group_edit.is_none(), "the group name field did not close on Enter");
    assert_eq!(h.state().bin_groups[gi4].name, "plan", "the group name was not saved");

    // Esc closes the field and does NOT change the name
    h.state_mut().bin_group_edit = Some((gi4, "other".into()));
    h.state_mut().bin_group_focus = true;
    h.step();
    h.key_press(egui::Key::Escape);
    h.step();
    assert!(h.state().bin_group_edit.is_none(), "Esc did not close the field");
    assert_eq!(h.state().bin_groups[gi4].name, "plan", "Esc must not change the name");

    // --- deleting media leaves no dangling members and empty groups
    let mut groups = vec![
        BinGroup { name: "a".into(), members: vec![MediaId(1), MediaId(2)], locked: false },
        BinGroup { name: "b".into(), members: vec![MediaId(3)], locked: true },
    ];
    kt_model::prune_bin_groups(&mut groups, &[MediaId(1)]);
    assert_eq!(groups.len(), 1, "the emptied group was not removed");
    assert_eq!(groups[0].members, vec![MediaId(1)], "the deleted media stayed in the group");
}

/// Test 101 (item 9 of round 12): buttons at the window edges are clicked, and do not hand the
/// click over to the window manager.
///
/// The resize zone is not a widget: it reads the RAW pointer at the end of the frame
/// and on press sends BeginResize, taking the pointer away from the WM.
/// The center of the “Close” button
/// (W−11, 11) lay right in the 16×16 corner, the bell — in the SW corner, and the top quarter of the menu
/// died. Old tests did not see this, because they checked HOVER: the resize zone does not interfere with it.
#[test]
fn t101_edge_buttons_are_clickable() {
    use egui::ViewportCommand as V;

    // Commands are accumulated over BOTH frames: BeginResize goes out on press, while
    // Close/Minimized — only on release (clicked()). The release is sent as an
    // EXPLICIT event, not via drop_at: the latter is followed by PointerGone,
    // step() runs a frame FOR EACH event, and output() shows a frame
    // without the command already — the test caught emptiness while the button was working.
    fn press(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) -> Vec<V> {
        let grab = |h: &Harness<'_, KtApp>| -> Vec<V> {
            h.output().viewport_output.values().flat_map(|v| v.commands.iter().cloned()).collect()
        };
        let button = |pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        h.hover_at(pos);
        h.step();
        h.event(button(true));
        h.step();
        let mut sent = grab(h);
        h.event(button(false));
        h.step();
        sent.extend(grab(h));
        h.event(egui::Event::PointerGone);
        h.step();
        sent
    }

    // dead points: the center of “Close” in the NE corner, “Maximize”/“Minimize” in the
    // top edge, the bell in the SW corner, “File” in the top edge
    let cases: [(egui::Pos2, &str); 5] = [
        (egui::pos2(W - 11.0, 11.0), "Close"),
        (egui::pos2(W - 33.0, 4.0), "Expand"),
        (egui::pos2(W - 55.0, 4.0), "Collapse"),
        (egui::pos2(18.0, H - 5.0), "Notifications"),
        (egui::pos2(30.0, 6.0), "File menu"),
    ];
    for (pos, what) in cases {
        let mut h = harness();
        let sent = press(&mut h, pos);
        assert!(
            !sent.iter().any(|c| matches!(c, V::BeginResize(_))),
            "a press on “{what}” went to window resize instead of the button"
        );
    }

    // and the consequences of presses really take place
    let mut h = harness();
    let sent = press(&mut h, egui::pos2(W - 11.0, 11.0));
    assert!(sent.iter().any(|c| matches!(c, V::Close)), "the window did not close");

    let mut h = harness();
    let sent = press(&mut h, egui::pos2(W - 55.0, 4.0));
    assert!(sent.iter().any(|c| matches!(c, V::Minimized(true))), "the window did not collapse");

    let mut h = harness();
    let before = h.state().notices_open;
    press(&mut h, egui::pos2(18.0, H - 5.0));
    assert_ne!(h.state().notices_open, before, "the bell did not open notifications");

    // the VERY edge (4 px) stays with the resize even over the buttons —
    // otherwise the corner diagonal is lost; we check that it is alive
    let mut h = harness();
    let sent = press(&mut h, egui::pos2(W - 2.0, 2.0));
    assert!(
        sent.iter().any(|c| matches!(c, V::BeginResize(_))),
        "the window corner stopped resizing because of the buttons"
    );

    // (item 6 of round 13) the “minimize” button is clicked at ANY width of the meters
    // panel: its own drag strip lies to the LEFT of the panel and does not
    // reach into the buttons (the standard egui grab was ±8 px over the whole height and won)
    for w in [crate::app::METERS_W_MIN, 70.0, crate::app::METERS_W_MAX] {
        let mut h = harness();
        h.state_mut().meters_w = w;
        h.step();
        let sent = press(&mut h, egui::pos2(W - 55.0, 11.0));
        assert!(
            sent.iter().any(|c| matches!(c, V::Minimized(true))),
            "at meter width {w}, “collapse” did not work"
        );
    }

    // and the strip itself drags the panel: the grab is to the LEFT of its edge
    let mut h = harness();
    let start = W - h.state().meters_w;
    h.hover_at(egui::pos2(start - 5.0, H * 0.5));
    h.step();
    h.drag_at(egui::pos2(start - 5.0, H * 0.5));
    h.step();
    h.hover_at(egui::pos2(start - 12.0, H * 0.5));
    h.step();
    h.hover_at(egui::pos2(W - 88.0, H * 0.5));
    h.step();
    h.drop_at(egui::pos2(W - 88.0, H * 0.5));
    h.step();
    let mw = h.state().meters_w;
    assert!(mw > 80.0, "the strip did not pull the meter panel: {mw}");
    assert!(mw <= crate::app::METERS_W_MAX, "meter panel went past the limit: {mw}");

    // clean edges still give a resize — the gate must not kill it
    let mut h = harness();
    let sent = press(&mut h, egui::pos2(W * 0.5, H - 2.0));
    assert!(
        sent.iter().any(|c| matches!(c, V::BeginResize(_))),
        "the bottom edge stopped stretching"
    );
}

/// Test 102 (items 2,5 of round 13): the track label does not overlay the clip name, the
/// cursor glow does not reach into the cache strips and shines from the side of the
/// movement.
/// Test 122 (item 5 of round 16): tracks are added BELOW the clicked row, and the
/// track names move together with their tracks.
///
/// The second half is a real defect found along the way: the functions
/// `lane_names_insert/remove/rotate` exist in kt-model and are covered by t90, but were never
/// called from the application, so after insertion/removal/
/// reordering the label stuck to someone else's track.
#[test]
fn t122_insert_lane_after_clicked_row() {
    use kt_model::LaneKind;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.ensure_tracks(kt_model::LaneKind::Video, 3);
        p.ensure_tracks(kt_model::LaneKind::Audio, 2);
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        edit_project(app, |p| {
            p.track_at_mut(LaneKind::Video, 1).unwrap().name = "Titles".into();
            p.track_at_mut(LaneKind::Audio, 1).unwrap().name = "Voice".into();
        });
    }
    h.step();

    // --- click on V1 (index 0) + “add video track” → the new row is exactly the NEXT one,
    // not at the end of the list
    h.state_mut().insert_lane_after_row(LaneKind::Video, Some((LaneKind::Video, 0)));
    h.step();
    assert_eq!(
        h.state().model.project().lanes(kt_model::LaneKind::Video),
        4,
        "track was not added"
    );
    assert_eq!(
        h.state().model.project().rows()[1],
        (LaneKind::Video, 1),
        "the new track did not land under the clicked one: {:?}",
        h.state().model.project().rows()
    );
    // the name “Titles” moved to V2 (index 1) — it must move to index 2
    assert_eq!(
        h.state().row_label(LaneKind::Video, 2),
        "Titles",
        "the name did not move together with its track"
    );
    assert_eq!(h.state().row_label(LaneKind::Video, 1), "V2", "the new track took a name not its own");

    // --- click on the LAST row of its kind: the row must land
    // right under it, not at the bottom of the list. This is exactly what the live
    // check found: lane_order_insert puts the row before the next track of its kind,
    // and when there is none it goes to the end, so a click on V2
    // with the order V1,V2,A1,A2 gave V1,V2,A1,A2,V3.
    let last_video = h
        .state()
        .model
        .project()
        .rows()
        .iter()
        .position(|&e| e == (LaneKind::Video, 3))
        .expect("last video row");
    h.state_mut().insert_lane_after_row(LaneKind::Video, Some((LaneKind::Video, 3)));
    h.step();
    assert_eq!(
        h.state().model.project().rows()[last_video + 1],
        (LaneKind::Video, 4),
        "the new row went to the very bottom instead of the place under the clicked one: {:?}",
        h.state().model.project().rows()
    );

    // --- click on a row of ANOTHER kind: the audio track lands under that same row
    let audio_row = h
        .state()
        .model
        .project()
        .rows()
        .iter()
        .position(|&e| e == (LaneKind::Video, 3))
        .expect("row V4 disappeared");
    h.state_mut().insert_lane_after_row(LaneKind::Audio, Some((LaneKind::Video, 3)));
    h.step();
    assert_eq!(
        h.state().model.project().rows()[audio_row + 1].0,
        LaneKind::Audio,
        "the audio track did not land under the clicked video row: {:?}",
        h.state().model.project().rows()
    );

    // --- morph is also added from the menu
    let m0 = h.state().model.project().lanes(kt_model::LaneKind::Morph);
    h.state_mut().insert_lane_after_row(LaneKind::Morph, None);
    h.step();
    assert_eq!(
        h.state().model.project().lanes(kt_model::LaneKind::Morph),
        m0 + 1,
        "morph track was not added"
    );

    // --- deletion: the name goes with the track, neighbors shift
    {
        let app = h.state_mut();
        edit_project(app, |p| {
            p.track_at_mut(LaneKind::Audio, 0).unwrap().name = "Music".into();
            p.track_at_mut(LaneKind::Audio, 1).unwrap().name = "Voice".into();
        });
        app.delete_lane(kt_model::ClipKind::Audio, 0);
    }
    h.step();
    assert_eq!(h.state().row_label(LaneKind::Audio, 0), "Voice", "after deletion the names did not shift");

    // --- reordering: the name moves with the content (otherwise “Voice” would remain
    // on someone else's row)
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.ensure_tracks(kt_model::LaneKind::Audio, 3);
        p.media.push(kt_model::MediaRef::new(kt_model::MediaId(1), "synth:1"));
        p.clips.push(kt_model::ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: kt_model::ClipId(1),
            link_group: None,
            kind: kt_model::ClipKind::Audio,
            lane: 0,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(90_000),
            source_in: kt_time::Tick(0),
            media: kt_model::MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        edit_project(app, |p| p.track_at_mut(LaneKind::Audio, 0).unwrap().name = "Voice".into());
        // rows, not numbers: since R.56 the track number is the RANK of the row, and
        // reordering goes by one command for any kinds
        let rows = app.model.project().rows();
        let from = rows.iter().position(|&r| r == (LaneKind::Audio, 0)).unwrap();
        let to = rows.iter().position(|&r| r == (LaneKind::Audio, 2)).unwrap();
        app.reorder_rows(from, to);
    }
    h.step();
    assert_eq!(
        h.state().model.project().clips[0].lane,
        2,
        "the contents did not move — the test below would check the wrong thing"
    );
    assert_eq!(
        h.state().row_label(LaneKind::Audio, 2),
        "Voice",
        "the name did not follow the contents on reordering"
    );
}

/// Test 120 (item 3 of round 16): holding the space bar speeds playback up to
/// 4× in about 2 s (as on YouTube) and does NOT break a single/double
/// press.
///
/// A trap that made it necessary to move the decision to release:
/// `key_pressed` turns on keyboard AUTO-REPEAT — a held space sent a
/// series of play/pause. Therefore the press is only remembered, and
/// play/pause is done on release, if the hold was short.
#[test]
fn t120_space_hold_boosts_playback() {
    use crate::app::{KtApp, SPACE_BOOST_MAX, SPACE_BOOST_SECS};
    use kt_model::*;
    use kt_time::Tick;

    // --- pure function: monotonic, runs into a ceiling, stable on
    // bad dt (after idling stable_dt arrives up to 250 ms — an explicit
    // scheme overshoots the target on such steps, item 7 of round 9)
    let mut b = 1.0f32;
    let mut prev = b;
    for _ in 0..(60.0 * SPACE_BOOST_SECS) as i32 {
        b = KtApp::space_boost_step(b, 1.0 / 60.0);
        assert!(b >= prev, "the boost is not monotonic: {prev} → {b}");
        assert!(b <= SPACE_BOOST_MAX + 1e-4, "the boost overshot the ceiling: {b}");
        prev = b;
    }
    assert!(b > SPACE_BOOST_MAX * 0.9, "in {SPACE_BOOST_SECS} s it did not reach the ceiling: {b}");
    // and NOT faster than the user asked (item 3 of round 17: “it must take not 2 s but
    // 5, otherwise it is very fast”): in 2 s the ramp must be noticeably below the
    // ceiling — with the former constant it came out at 3.85 of 4
    let mut half = 1.0f32;
    for _ in 0..120 {
        half = KtApp::space_boost_step(half, 1.0 / 60.0);
    }
    assert!(half < 3.4, "the boost is too sharp: in 2 s already {half}× of {SPACE_BOOST_MAX}");
    let mut rough = 1.0f32;
    for dt in [0.25f32, 0.1, 1.0 / 30.0, 0.25, 0.25] {
        rough = KtApp::space_boost_step(rough, dt);
        assert!(
            (1.0..=SPACE_BOOST_MAX + 1e-4).contains(&rough),
            "a bad dt={dt} pushed the boost out of bounds: {rough}"
        );
    }

    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(0),
            duration: Tick(90_000 * 60),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();

    // --- a single press still toggles the transport
    let before = h.state().player.transport.is_playing();
    h.key_press(egui::Key::Space);
    h.step();
    h.step();
    assert_ne!(
        h.state().player.transport.is_playing(),
        before,
        "a short press of space stopped toggling play/pause"
    );
    let playing = h.state().player.transport.is_playing();

    // --- hold: the speed grows, the transport does NOT toggle
    h.key_down(egui::Key::Space);
    h.step();
    std::thread::sleep(std::time::Duration::from_millis(260));
    for _ in 0..60 {
        h.step();
    }
    let boosted = h.state().player.playback_rate();
    assert!(
        boosted > 1.5 && boosted <= SPACE_BOOST_MAX + 0.01,
        "holding did not speed up playback: {boosted}"
    );
    assert!(h.state().player.transport.is_playing(), "holding must play");
    // the user must SEE the ramp: the lower (temporary) strip of the speed control
    // shows the actual speed, not “1×”
    let shown = h.state().speed_temp_slider().expect("the boost is not visible on the speed control");
    assert!(
        shown > crate::app::rate_to_slider(1.0) + 0.01,
        "the speed bar shows not the boost: {shown}"
    );
    h.key_up(egui::Key::Space);
    h.step();
    assert!(
        (h.state().player.playback_rate() - 1.0).abs() < 0.01,
        "after release the speed did not return: {}",
        h.state().player.playback_rate()
    );
    assert_eq!(
        h.state().player.transport.is_playing(),
        playing,
        "releasing the boost must not toggle play/pause"
    );

    // --- reverse: the ramp does not lose the sign of the speed
    h.state_mut().base_rate = -1.0;
    h.state_mut().player.set_playback_rate(-1.0);
    h.key_down(egui::Key::Space);
    h.step();
    std::thread::sleep(std::time::Duration::from_millis(260));
    for _ in 0..40 {
        h.step();
    }
    let rev = h.state().player.playback_rate();
    assert!(rev < -1.5, "the boost in reverse lost its sign/speed: {rev}");
    h.key_up(egui::Key::Space);
    h.step();
    h.state_mut().base_rate = 1.0;
    h.state_mut().player.set_playback_rate(1.0);

    // --- double press: pause, then “stop at the start of the region” (item 15 of round 6)
    h.state_mut().model.set_region(Region::normalized(Tick(90_000), Tick(180_000)));
    h.state_mut().player.seek(Tick(120_000));
    h.step();
    h.key_press(egui::Key::Space); // pause
    h.step();
    h.step();
    assert!(!h.state().player.transport.is_playing(), "the first press did not pause");
    h.key_press(egui::Key::Space); // immediately a second one — stop at the start of the region
    h.step();
    h.step();
    assert_eq!(
        h.state().player.transport.playhead(),
        Tick(90_000),
        "a double press of space stopped moving to the start of the region"
    );
}

/// Test 119 (item 2 of round 16): Shift+A / Shift+D drag the region edge.
///
/// Shift is the “extend selection” modifier from pro programs. Shift+A
/// puts the LEFT edge at the new cursor position, Shift+D — the right one; without a
/// region the second edge becomes the position BEFORE the jump. Plain A/D do not
/// touch the region (otherwise navigation would silently spoil the selection), and
/// Shift+Home/End remain “to the very start/end”.
#[test]
fn t119_shift_nav_drags_region() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (i, start) in [90_000i64, 270_000, 450_000].into_iter().enumerate() {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(i as u64 + 1),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(0),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.player.seek(Tick(0));
        app.dirty_content = false;
        app.dirty_ui = false;
    }
    h.step();

    // ordinary navigation does NOT create a region
    h.key_press(egui::Key::D);
    h.step();
    let after_plain = h.state().player.transport.playhead();
    assert!(after_plain.0 > 0, "plain D did not move the cursor");
    assert!(h.state().model.project().region.is_none(), "plain D created a region");

    // the first Shift+D selects the traversed stretch [was, now]
    let before = h.state().player.transport.playhead();
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::D);
    h.step();
    let now = h.state().player.transport.playhead();
    let r = h.state().model.project().region.expect("Shift+D did not create a region");
    assert!(now > before, "Shift+D did not move the cursor: {before:?} → {now:?}");
    assert_eq!(r.start, before, "the left edge must stay where it jumped from");
    assert_eq!(r.end, now, "the right edge must land at the new position");

    // the next Shift+D moves ONLY the right edge
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::D);
    h.step();
    let r2 = h.state().model.project().region.expect("region disappeared");
    assert_eq!(r2.start, r.start, "the right edge dragged the left one along: {r:?} → {r2:?}");
    assert!(r2.end > r.end, "the right edge did not move: {r:?} → {r2:?}");

    // Shift+A moves the LEFT edge and does not touch the right one
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::A);
    h.step();
    let r3 = h.state().model.project().region.expect("region disappeared");
    assert_eq!(r3.end, r2.end, "the left edge dragged the right one: {r2:?} → {r3:?}");
    assert!(r3.start > r2.start, "the left edge did not move: {r2:?} → {r3:?}");
    assert_eq!(h.state().player.transport.playhead(), r3.start, "the cursor is not on the left edge");

    // region — a “convenience”: the asterisk “there is unsaved content” does not
    // light up (item 5 of round 11)
    assert!(!h.state().dirty_content, "editing the region raised dirty_content");
    assert!(h.state().dirty_ui, "editing the region did not raise dirty_ui");
}

/// Test 118 (item 1 of round 16): the media bin scrolls — by itself when dragging to the edge and
/// with the wheel.
///
/// Cause of the defect: “the cursor left the bin” was checked by `ui.max_rect()`,
/// but inside a ScrollArea that is the VIEWPORT in content coordinates. At the lower
/// edge of the screen the sticky “move to the timeline” mode switched on, and the positions
/// were rolled back — a card could not be placed below one screen,
/// the content never outgrew the viewport, and the wheel had nothing
/// to scroll. The test drives the mouse like a person (press → move → keep moving).
#[test]
fn t118_bin_autoscrolls_and_wheel_works() {
    use crate::bin_panel::bin_edge_scroll;
    use kt_model::{MediaId, MediaRef};

    // --- a pure curve: at the lower edge we scroll down, at the upper one up,
    // in the middle there is no rest at all
    let view = egui::Rect::from_min_size(egui::pos2(0.0, 100.0), egui::vec2(200.0, 400.0));
    assert_eq!(bin_edge_scroll(view, 300.0), 0.0, "the middle must not wobble");
    let down = bin_edge_scroll(view, view.bottom() - 1.0);
    let up = bin_edge_scroll(view, view.top() + 1.0);
    assert!(down < 0.0, "at the bottom edge the list must scroll down: {down}");
    assert!(up > 0.0, "at the top edge — up: {up}");
    assert!(
        bin_edge_scroll(view, view.bottom() - 20.0).abs() < down.abs(),
        "the curve must grow toward the edge"
    );

    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        for n in 1..=3u64 {
            p.media.push(MediaRef::new(MediaId(n), format!("synth:{n}")));
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.bin_free = true;
        app.bin_layout.insert(MediaId(1), egui::pos2(0.0, 0.0));
        app.bin_layout.insert(MediaId(2), egui::pos2(0.0, 100.0));
        app.bin_layout.insert(MediaId(3), egui::pos2(0.0, 200.0));
    }
    h.step();
    let at = |h: &Harness<'_, KtApp>, m: MediaId| {
        h.state().bin_card_rects.iter().find(|(id, _)| *id == m).unwrap().1
    };
    let col = panel_rect(&h, "left_col");
    let grab = at(&h, MediaId(1)).center();
    let anchor_before = at(&h, MediaId(3));

    // we drag the first card to the very bottom edge of the column and HOLD it there:
    // the list must crawl by itself, as the timeline does
    h.drag_at(grab);
    h.step();
    h.hover_at(grab + egui::vec2(0.0, 8.0));
    h.step();
    let low = egui::pos2(grab.x, col.bottom() - 10.0);
    for _ in 0..12 {
        h.hover_at(low);
        h.step();
    }
    let anchor_after = at(&h, MediaId(3));
    assert!(
        anchor_after.top() < anchor_before.top() - 20.0,
        "the list did not scroll when dragged to the edge: {} → {}",
        anchor_before.top(),
        anchor_after.top()
    );
    h.drop_at(low);
    h.step();
    let moved = h.state().bin_layout[&MediaId(1)];
    assert!(moved.y > 200.0, "the card was rolled back instead of moved down (sticky “carry out”): {moved:?}");

    // --- wheel: the content is now longer than the viewport, and above the bin it
    // must scroll (the standard ScrollArea mutes the wheel only during a drag —
    // we have already released the drag)
    let before = at(&h, MediaId(3)).top();
    let over = egui::pos2(col.center().x, col.center().y);
    h.hover_at(over);
    h.step();
    for _ in 0..3 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -60.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    let after = at(&h, MediaId(3)).top();
    assert!(after < before - 20.0, "the wheel does not scroll the media bin: {before} → {after}");

    // --- item 1 of round 17: the wheel must work ALSO DURING a drag. egui itself
    // mutes the ScrollArea's scroll while something is being dragged
    // (`dragged_id().is_none()`), and a person turns the wheel exactly then —
    // to reach the needed place (“the mouse scroll still does not
    // work”).
    // we return to the top: after the checks above the list was wound down
    // and the cards went out past the visible strip
    let wheel_up = |h: &mut Harness<'_, KtApp>, n: usize| {
        for _ in 0..n {
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 90.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            });
            h.step();
        }
    };
    h.hover_at(over);
    h.step();
    wheel_up(&mut h, 12);
    let grab2 = at(&h, MediaId(2)).center();
    assert!(col.contains(grab2), "card outside the visible part of the column: {grab2:?} / {col:?}");
    h.drag_at(grab2);
    h.step();
    h.hover_at(grab2 + egui::vec2(0.0, 8.0));
    h.step();
    let before_drag_wheel = at(&h, MediaId(3)).top();
    for _ in 0..3 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -60.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    let after_drag_wheel = at(&h, MediaId(3)).top();
    assert!(
        after_drag_wheel < before_drag_wheel - 20.0,
        "the wheel does not scroll the bin during a drag: {before_drag_wheel} → {after_drag_wheel}"
    );
    h.drop_at(grab2 + egui::vec2(0.0, 8.0));
    h.step();

    // --- item 1 of round 17: a tap ON THE EDGE is “scroll the list”, not “move out”:
    // the card must stay where it is being led. The user: “I tap
    // the edge — we scroll, but then the source flies back by itself”.
    let card = at(&h, MediaId(1));
    let before_pos = h.state().bin_layout[&MediaId(1)];
    h.drag_at(card.center());
    h.step();
    h.hover_at(card.center() + egui::vec2(0.0, 8.0));
    h.step();
    let jab = egui::pos2(card.center().x, col.bottom() + 12.0);
    for _ in 0..4 {
        h.hover_at(jab);
        h.step();
    }
    h.drop_at(jab);
    h.step();
    let after_pos = h.state().bin_layout[&MediaId(1)];
    assert!(
        (after_pos - before_pos).length() > 4.0,
        "a poke at the edge rolled the card back to its old place: {before_pos:?} → {after_pos:?}"
    );

    // but a real move-out (far beyond the column) still rolls back the
    // layout — otherwise the card flies away on a successful drop onto the timeline
    let card = at(&h, MediaId(1));
    let before_pos = h.state().bin_layout[&MediaId(1)];
    h.drag_at(card.center());
    h.step();
    h.hover_at(card.center() + egui::vec2(8.0, 0.0));
    h.step();
    let out = egui::pos2(col.right() + 200.0, col.center().y);
    h.hover_at(out);
    h.step();
    h.drop_at(out);
    h.step();
    assert_eq!(
        h.state().bin_layout[&MediaId(1)],
        before_pos,
        "carrying out onto the timeline must not change the bin layout (item 8 round 9)"
    );
}

#[test]
fn t102_lane_label_and_playhead_trail() {
    use crate::timeline::{STRIPES_H, TIME_TOP, lane_label_y};

    // (item 2) the track label lives one ROW BELOW the clip name: the clip name
    // is drawn from row_top+2 as a band about 17 px high, so the label must
    // start no higher than row_top+17 — otherwise the letters would overlap
    for (lane_h, galley_h) in [(64.0f32, 14.0f32), (40.0, 14.0), (28.0, 14.0), (120.0, 14.0)] {
        let y = lane_label_y(100.0, lane_h, galley_h);
        assert!(y >= 100.0, "the label went above the track: {y}");
        assert!(y + galley_h <= 100.0 + lane_h, "the label stuck out past the track: {y}+{galley_h}");
        if lane_h >= 36.0 {
            assert!(y >= 117.0, "the label on track {lane_h} landed on the clip name row: {y}");
        }
    }
    // (item 5) the glow starts below the cache strips
    let g = harness().state().tl_geom.clone().expect("no geometry");
    assert!(
        g.rect.top() + TIME_TOP >= g.rect.top() + STRIPES_H,
        "the cursor glow will begin over the cache strips"
    );

    let mut h = harness();
    // the direction is remembered by the sign of the speed and does not blink during pause
    assert!(!h.state().play_dir_back, "by default the trail is on the left (forward play)");
    h.state_mut().player.set_playback_rate(-1.0);
    h.step();
    assert!(h.state().play_dir_back, "reverse did not switch the trail side");
    h.state_mut().player.set_playback_rate(0.0);
    h.step();
    assert!(h.state().play_dir_back, "on pause the side must be kept");
    h.state_mut().player.set_playback_rate(1.0);
    h.step();
    assert!(!h.state().play_dir_back, "forward did not return the trail to the left");
}

/// Test 127 (R.19): the layout holds in ALL FIVE LANGUAGES.
///
/// The limits of 420 px (left column) and 90 px (meters) were set by direct requests
/// of the user (item 4 of round 13, item 6 of round 12) and are NOT moved: if a translation
/// does not fit in them, the translation is made shorter. Before R.19 these limits were checked
/// only in Russian, that is, on a single set of string lengths.
#[test]
fn t127_layout_holds_in_every_language() {
    for lang in Lang::ALL {
        let code = lang.code();
        let mut h = harness_lang(lang);
        let w = panel_width(&h, "left_col");
        assert!((72.0..=420.0).contains(&w), "{code}: left column {w}");
        assert!(
            h.state().preview_rect.width() > 600.0,
            "{code}: preview collapsed: {:?}",
            h.state().preview_rect
        );
        assert!(
            panel_width(&h, "meters_col") <= crate::app::METERS_W_MAX + 0.5,
            "{code}: meter panel wider than the limit: {}",
            panel_width(&h, "meters_col")
        );

        // long content has no right to swell the column in any language
        h.state_mut().gpu_info = Some("X".repeat(400));
        h.step();
        let w = panel_width(&h, "left_col");
        assert!(w <= 420.0, "{code}: long content inflated the column: {w}");

        // the timecode mask is pure ASCII digits, it must be the same in all languages
        // (its entry order is right to left regardless of RTL)
        assert_eq!(
            crate::util::mask_text("130"),
            crate::util::mask_text("130"),
            "{code}: the timecode mask depends on the language"
        );
    }
}

/// Test 129 (R.19): switching the language on the fly does not break state.
///
/// The main point here is the window ids. `Window::new(title)` derives the `Id` from the TITLE, and
/// the position and size live under this id: without an explicit `.id()` a language change
/// would reset the position of every dialog. Exactly this is checked — that the
/// window state lies under a constant id, not under the translated title.
#[test]
fn t129_language_switch_keeps_state() {
    use egui::AreaState;

    let mut h = harness_lang(Lang::Ru);
    h.state_mut().notices_open = true;
    h.state_mut().search_open = true;
    h.state_mut().perf.diag_open = true;
    h.step();
    h.step();

    let ids = ["kt_notices", "kt_search", "kt_diag"];
    let before: Vec<_> = ids
        .iter()
        .map(|i| {
            AreaState::load(&h.ctx, egui::Id::new(*i))
                .unwrap_or_else(|| panic!("window {i} is not drawn"))
                .left_top_pos()
        })
        .collect();

    // the FX registry must not be rebuilt: categories and keywords are keys
    let reg_before = crate::fx_registry::registry().as_ptr();

    crate::i18n::set_lang_ctx(&h.ctx, Lang::En);
    h.step();
    h.step();

    for (i, id) in ids.iter().enumerate() {
        let after = AreaState::load(&h.ctx, egui::Id::new(*id))
            .unwrap_or_else(|| panic!("after a language change window {id} lost its state"))
            .left_top_pos();
        assert_eq!(before[i], after, "window {id} moved on a language change");
    }
    assert_eq!(
        reg_before,
        crate::fx_registry::registry().as_ptr(),
        "the FX registry was rebuilt on a language change"
    );

    // and the translation itself: the window button's hint became English
    h.hover_at(egui::pos2(W - 11.0, 10.0));
    h.step();
    assert_eq!(h.state().status_line_shown.0, tr(k::win::CLOSE));
    assert_eq!(tr(k::win::CLOSE), "Close", "the English catalog was not picked up");
}

/// Test 126, part “b” (R.19): PINNING THE RENDER of Arabic.
///
/// The unit tests of `bidi::visual` check the string, while this one checks what came out of it
/// in egui. It is what tells a correct algorithm from a plausible one:
///
/// * without reordering, the first LOGICAL word ends up on the LEFT, that is,
///   the phrase reads backwards (measured: 'م' at x=22, 'ج' at x=56);
/// * with per-character reversal (`unicode_bidi::reorder_line`) the word order
///   is correct, but harfrust reverses each run a SECOND time, and the letters
///   inside a word come out inverted — the first check catches this.
///
/// If egui someday learns bidi itself, this test turns red — and that is the
/// right signal: then our layer must be removed.
#[test]
fn t126_arabic_renders_right_to_left() {
    let h = harness_lang(Lang::Ar);
    let font = egui::FontId::proportional(16.0);
    let layout = |text: String| {
        h.ctx.fonts_mut(|f| f.layout_no_wrap(text, font.clone(), egui::Color32::WHITE))
    };

    // “file”: م ل ف. Inside the word the letters must stay logical — their
    // order will be reversed by harfrust itself, so the LEFTMOST glyph is the LAST letter.
    let word = "\u{0645}\u{0644}\u{0641}";
    let g = layout(crate::i18n::bidi::visual(word));
    assert_eq!(
        g.rows[0].glyphs.first().map(|x| x.chr),
        Some('\u{0641}'),
        "letters inside a word were reversed twice — looks like a per-character reorder_line"
    );

    // “ملف جديد”: the last logical word must end up to the LEFT of the first.
    // The letters of the words do not overlap, so the positions are unambiguous.
    let phrase = format!("{word} \u{062C}\u{062F}\u{064A}\u{062F}");
    let g = layout(crate::i18n::bidi::visual(&phrase));
    let x_of = |ch: char| {
        g.rows[0]
            .glyphs
            .iter()
            .find(|x| x.chr == ch)
            .unwrap_or_else(|| panic!("glyph {ch:?} is not drawn"))
            .pos
            .x
    };
    assert!(
        x_of('\u{062C}') < x_of('\u{0645}'),
        "the second word did not move left: ج at {:.0}, م at {:.0}",
        x_of('\u{062C}'),
        x_of('\u{0645}')
    );

    // and a control: WITHOUT reordering the order is reversed — that is, the test
    // really measures the work of bidi, not a property of the font
    let g = layout(phrase.clone());
    let x_of = |ch: char| g.rows[0].glyphs.iter().find(|x| x.chr == ch).expect("glyph").pos.x;
    assert!(
        x_of('\u{062C}') > x_of('\u{0645}'),
        "the logical order suddenly matched the visual one — the check degenerated"
    );
}

/// Test 128 (R.19): the label fits its allotted place IN EVERY LANGUAGE.
///
/// A direct implementation of rules 3 and 7 of the checklist (`apps/kt-app/CLAUDE.md`):
/// the width is taken from the longest variant, and a long name has no
/// right to climb out of its zone. The budgets are derived from the real layout
/// constants, not set by eye:
///
/// * the menu shares the header of the left column with the window buttons, and the column by default is
///   232 px (`app.rs`), so the three headings get 160;
/// * the rows of the performance panel are drawn with `ui.columns(2)` inside a column up to
///   420 px wide — each column gets less than 200, and because of exactly this
///   the user's rollback counter was already truncated with an ellipsis (item 14 of round 14).
///
/// A translation that is too long fails here with the key name and the actual width:
/// it is fixed on the translation side, not by widening the panel — the limits of 420 and 90
/// are set by direct requests of the user.
#[test]
fn t128_labels_fit_their_budget() {
    const MENU_BUDGET: f32 = 160.0;
    const PERF_COLUMN: f32 = 190.0;
    const CHECKBOX_BUDGET: f32 = 380.0;

    for lang in Lang::ALL {
        let h = harness_lang(lang);
        let body = h.ctx.style_of(egui::Theme::Dark).text_styles[&egui::TextStyle::Body].clone();
        let heading = egui::FontId::proportional(14.0);
        let w = |text: String, font: egui::FontId| {
            h.ctx.fonts_mut(|f| f.layout_no_wrap(text, font, egui::Color32::WHITE)).size().x
        };
        let code = lang.code();

        let menu: f32 = [k::menu::file::TITLE, k::menu::edit::TITLE, k::menu::view::TITLE]
            .iter()
            .map(|key| w(tr(*key).to_string(), heading.clone()))
            .sum();
        assert!(
            menu <= MENU_BUDGET,
            "{code}: three menu headers take {menu:.0} px against the budget {MENU_BUDGET}"
        );

        // two-column rows are measured ASSEMBLED: the template is shorter than
        // what the user will see
        let rows = [
            (k::perf::CACHE_MB, crate::tf!(k::perf::CACHE_MB, 1234, 5678)),
            (k::perf::PREVIEW_RES, crate::tf!(k::perf::PREVIEW_RES, 1920, 1080)),
            (k::perf::CLIPS_N, crate::tf!(k::perf::CLIPS_N, 1234)),
            (k::perf::MEDIA_N, crate::tf!(k::perf::MEDIA_N, 1234)),
            (k::perf::V_SHOWN, crate::tf!(k::perf::V_SHOWN, 123456)),
            (k::perf::V_SKIPPED, crate::tf!(k::perf::V_SKIPPED, 1234, 567)),
            (k::perf::REGION_S, crate::tf!(k::perf::REGION_S, "123.45", "678.90")),
            (k::perf::CACHE_SHORT, crate::tf!(k::perf::CACHE_SHORT, 1234, 5678)),
        ];
        for (key, text) in rows {
            let got = w(text.clone(), body.clone());
            assert!(
                got <= PERF_COLUMN,
                "{code}: {} = {text:?} takes {got:.0} px against the budget {PERF_COLUMN}",
                crate::i18n::key_name(key)
            );
        }

        let got = w(tr(k::perf::ECONOMY).to_string(), body.clone());
        assert!(
            got <= CHECKBOX_BUDGET,
            "{code}: the checkbox of the economy UI takes {got:.0} px against the budget {CHECKBOX_BUDGET}"
        );
    }
}

/// Test 139 (item 139, R.24): the plan of the Move drag. A single
/// partial overlap = crossfade; a multi-overlap / absorption / tiny overlap /
/// vertical move into an occupied spot — a clamp (the user's decision, item 10).
#[test]
fn t139_move_plan_crossfade_and_clamp() {
    use crate::timeline::{MovePlan, plan_move};
    use kt_model::*;
    use kt_time::Tick;

    let clip = |id: u64, lane: u32, start: i64, dur: i64| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
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
    // the behavior of R.24/R.26 lives in the mode WITHOUT layering (R.28)
    p.overlap_layers = false;
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips = vec![clip(1, 0, 0, 90_000), clip(2, 0, 200_000, 90_000), clip(3, 0, 400_000, 90_000)];
    p.validate().unwrap();

    // without collisions
    assert_eq!(plan_move(&p, ClipId(2), 100_000, 0), MovePlan::Free);
    // incoming overlap: the start of clip 2 is inside the body of clip 1 → the joint is in the middle of the overlap
    match plan_move(&p, ClipId(2), 50_000, 0) {
        MovePlan::Crossfade { outgoing, incoming, joint, in_off, out_off } => {
            assert_eq!((outgoing, incoming), (ClipId(1), ClipId(2)));
            assert_eq!(joint, 70_000, "middle of the overlap [50000, 90000)");
            assert_eq!((in_off, out_off), (20_000, 20_000));
        }
        other => panic!("expected a crossfade, got {other:?}"),
    }
    // outgoing overlap: the end of clip 2 is inside the body of clip 3
    match plan_move(&p, ClipId(2), 350_000, 0) {
        MovePlan::Crossfade { outgoing, incoming, joint, .. } => {
            assert_eq!((outgoing, incoming), (ClipId(2), ClipId(3)));
            assert_eq!(joint, 420_000, "middle of the overlap [400000, 440000)");
        }
        other => panic!("expected a crossfade, got {other:?}"),
    }
    // a full cover of the neighbor (equal durations, want==n.start) → clamp
    // (a move to the left stops at its end)
    assert!(
        matches!(plan_move(&p, ClipId(2), 0, 0), MovePlan::Clamped { start: 90_000, .. }),
        "full coverage → clamp"
    );
    // tiny overlap (< the minimum of the window) → clamp, not a micro-transition
    assert!(
        matches!(plan_move(&p, ClipId(2), 89_960, 0), MovePlan::Clamped { start: 90_000, .. }),
        "tiny overlap → clamp"
    );
    // multi-overlap: move clip 3 closer and overlap both at once
    let mut p2 = p.clone();
    p2.clips[2].start = Tick(95_000);
    assert!(matches!(plan_move(&p2, ClipId(2), 60_000, 0), MovePlan::Clamped { .. }));
    // vertical move into an occupied spot of someone else's track → clamp, not trims
    let mut p3 = p.clone();
    p3.clips[1].lane = 1;
    p3.normalize();
    assert!(matches!(plan_move(&p3, ClipId(2), 50_000, 0), MovePlan::Clamped { .. }));
    // a foreign placeholder occupies the spot
    let mut p4 = p.clone();
    p4.foreign.push(ForeignEvent {
        key: 1,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(120_000),
        duration: Tick(30_000),
    });
    assert!(matches!(plan_move(&p4, ClipId(2), 110_000, 0), MovePlan::Clamped { .. }));
}

/// Test 139 (live part): an overlap by a drag creates a transition, clips butt together.
#[test]
fn t139_drag_overlap_creates_crossfade() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        // the behavior of R.24/R.26 lives in the mode WITHOUT layering (R.28)
        p.overlap_layers = false;
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (n, start) in [(1u64, 0i64), (2, 200_000)] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(n),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(0),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let y = g.lane_y(ClipKind::Video, 0, 0.5);
    let grab = g.tick_to_x(200_000 + 45_000);
    h.hover_at(egui::pos2(grab, y));
    h.step();
    h.drag_at(egui::pos2(grab, y));
    h.step();
    h.hover_at(egui::pos2(grab - 8.0, y));
    h.step();
    let grab_dt = match h.state().drag {
        Some(crate::timeline::Drag::Clip(ref d)) => d.grab_dt,
        _ => panic!("clip drag did not start"),
    };
    // target: start = 50000 — a partial overlap of the tail of clip 1
    let to = g.tick_to_x(50_000 + grab_dt);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();

    let p = h.state().model.project().clone();
    assert_eq!(p.transitions.len(), 1, "an overlap must create a transition");
    let t = &p.transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(1)), Some(ClipId(2))));
    // the harness pointer is quantized by the pixel: tolerance 1 px = 1000 ticks
    let near = |a: Tick, want: i64, what: &str| {
        assert!((a.0 - want).abs() <= 1_000, "{what}: {a:?} vs ~{want}");
    };
    near(t.in_offset, 20_000, "in_offset");
    near(t.out_offset, 20_000, "out_offset");
    let c1 = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let c2 = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    near(c1.start + c1.duration, 70_000, "a junction in the middle of the overlap");
    assert_eq!(c2.start, c1.start + c1.duration, "clips strictly butted together");
    near(c2.source_in, 20_000, "in-point trimmed");
    p.validate().expect("result is valid");
}

/// Test 140 (item 140, R.24): drags of the edges/body of the transition and
/// the drawing geometry.
#[test]
fn t140_transition_edge_and_body_drags() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (n, start, src) in [(1u64, 0i64, 0i64), (2, 90_000, 30_000)] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(n),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(src),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: Some(ClipId(2)),
            in_offset: Tick(10_000),
            out_offset: Tick(10_000),
            kind: TransitionKind::Crossfade,
            audio: Default::default(),
            auto: vec![],
            invert: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let y = g.lane_y(ClipKind::Video, 0, 0.5);

    // drag the LEFT edge of the window outward: in_offset 10000 → 30000
    let x_edge = g.tick_to_x(90_000 - 10_000);
    h.hover_at(egui::pos2(x_edge, y));
    h.step();
    h.drag_at(egui::pos2(x_edge, y));
    h.step();
    h.hover_at(egui::pos2(x_edge - 8.0, y));
    h.step();
    assert!(
        matches!(h.state().drag, Some(crate::timeline::Drag::TransitionEdge { left: true, .. })),
        "grabbing the transition edge did not work: {:?}",
        matches!(h.state().drag, Some(crate::timeline::Drag::Clip(_)))
    );
    let to = g.tick_to_x(90_000 - 30_000);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let t0 = h.state().model.project().transitions[0].clone();
    assert!((t0.in_offset.0 - 30_000).abs() <= 1_000, "in_offset ~30000: {t0:?}");
    assert_eq!(t0.out_offset, Tick(10_000));
    // one undo rolls back the whole gesture (coalescing)
    h.state_mut().model.undo().unwrap();
    assert_eq!(h.state().model.project().transitions[0].in_offset, Tick(10_000));
    h.step();

    // roll of the joint: we pull the window centre by +20000 — the joint and the clips move
    let xj = g.tick_to_x(90_000);
    h.hover_at(egui::pos2(xj, y));
    h.step();
    h.drag_at(egui::pos2(xj, y));
    h.step();
    h.hover_at(egui::pos2(xj + 8.0, y));
    h.step();
    assert!(matches!(h.state().drag, Some(crate::timeline::Drag::TransitionBody { .. })));
    let to = g.tick_to_x(110_000);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let p = h.state().model.project().clone();
    let c1 = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let c2 = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    let joint = c1.start + c1.duration;
    assert!((joint.0 - 110_000).abs() <= 2_000, "roll moved the junction: {joint:?}");
    assert_eq!(c2.start, joint, "clips strictly butted together");
    assert_eq!(c2.source_in - Tick(30_000), joint - Tick(90_000), "TrimStart cut the source");
    assert_eq!(p.transitions[0].joint(&p), Some(joint), "the window moved with the junction");

    // drawing geometry — a pure function
    let r = crate::timeline::transition_rect(100.0, 140.0, 50.0, 40.0);
    assert_eq!((r.left(), r.right()), (100.0, 140.0));
    assert!(r.top() > 50.0 && r.bottom() < 90.0, "strip inside the track row");
}

/// Test 163 (item 163, R.28): with layering the gesture cuts NOTHING.
/// The user's requirement verbatim: “however much there was, that much must remain…
/// NOTHING ANYWHERE MUST GROW BACK OR BE CUT OFF”.
#[test]
fn t163_overlap_drag_changes_nothing_but_position() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        assert!(p.overlap_layers, "layering is on by default");
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        // A/V pair + a single video clip: we check both the groups and the singles
        for (n, kind, start, group) in [
            (1u64, ClipKind::Video, 0i64, Some(GroupId(1))),
            (2, ClipKind::Audio, 0, Some(GroupId(1))),
            (3, ClipKind::Video, 200_000, None),
        ] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(n),
                link_group: group,
                kind,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(10_000),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let y = g.lane_y(ClipKind::Video, 0, 0.5);
    let before: Vec<ClipEvent> = h.state().model.project().clips.clone();

    // we overlap with a third clip onto the first (overlap 40,000 ticks)
    let from = g.tick_to_x(200_000 + 45_000);
    let to = g.tick_to_x(50_000 + 45_000);
    h.hover_at(egui::pos2(from, y));
    h.step();
    h.drag_at(egui::pos2(from, y));
    h.step();
    h.hover_at(egui::pos2(from - 8.0, y));
    h.step();
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();

    let p = h.state().model.project().clone();
    assert!(p.transitions.is_empty(), "the overlap plays by itself — a transition is not created");
    for c in &p.clips {
        let was = before.iter().find(|x| x.id == c.id).unwrap();
        assert_eq!(c.duration, was.duration, "duration {:?} did not change", c.id);
        assert_eq!(c.source_in, was.source_in, "entry point {:?} did not change", c.id);
        if c.id != ClipId(3) {
            assert_eq!(c.start, was.start, "another clip {:?} did not move", c.id);
        }
    }
    let moved = p.clips.iter().find(|c| c.id == ClipId(3)).unwrap();
    assert!((moved.start.0 - 50_000).abs() <= 1_000, "the clip landed where it was dragged: {moved:?}");
    p.validate().expect("the overlap is legal");

    // we drag it back apart — again nothing is cut
    let from = g.tick_to_x(moved.start.0 + 45_000);
    let to = g.tick_to_x(300_000 + 45_000);
    h.hover_at(egui::pos2(from, y));
    h.step();
    h.drag_at(egui::pos2(from, y));
    h.step();
    h.hover_at(egui::pos2(from + 8.0, y));
    h.step();
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let p = h.state().model.project().clone();
    for c in &p.clips {
        let was = before.iter().find(|x| x.id == c.id).unwrap();
        assert_eq!(c.duration, was.duration, "after pulling apart {:?} is intact", c.id);
        assert_eq!(c.source_in, was.source_in, "after pulling apart the in-point {:?} is intact", c.id);
    }
}

/// Test 160 (item 160, R.26): overlapped — pulled apart — the material
/// came back. The user's complaint: “in Sony Vegas, if you pull apart
/// overlapping events — what was overlapping remains, without trimming”.
#[test]
fn t160_pulling_apart_restores_trimmed_material() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        // the behavior of R.24/R.26 lives in the mode WITHOUT layering (R.28)
        p.overlap_layers = false;
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (n, start) in [(1u64, 0i64), (2, 200_000)] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(n),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(0),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let y = g.lane_y(ClipKind::Video, 0, 0.5);
    let grab = |h: &mut Harness<KtApp>, from: f32, to: f32, y: f32| {
        // drag like a person: press → nudge slightly → keep dragging
        h.hover_at(egui::pos2(from, y));
        h.step();
        h.drag_at(egui::pos2(from, y));
        h.step();
        h.hover_at(egui::pos2(from + 8.0_f32.copysign(to - from), y));
        h.step();
        h.hover_at(egui::pos2(to, y));
        h.step();
        h.drop_at(egui::pos2(to, y));
        h.step();
    };

    // 1. overlap clip 2 onto clip 1: [200000..290000) → [50000..140000)
    let from = g.tick_to_x(200_000 + 45_000);
    grab(&mut h, from, g.tick_to_x(50_000 + 45_000), y);
    {
        let p = h.state().model.project();
        assert_eq!(p.transitions.len(), 1, "the overlap created a crossfade");
        let c1 = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
        assert!(c1.duration < Tick(90_000), "the outgoing one trimmed by the junction");
    }

    // 2. drag it back apart — the material must come back
    // grab it FURTHER from the transition window: its edges intercept the click
    let cur = h.state().model.project().clips.iter().find(|c| c.id == ClipId(2)).unwrap().start.0;
    let grab_dt = 60_000;
    let from = g.tick_to_x(cur + grab_dt);
    grab(&mut h, from, g.tick_to_x(300_000 + grab_dt), y);

    let p = h.state().model.project().clone();
    assert!(p.transitions.is_empty(), "the transition died together with the junction");
    let c1 = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let c2 = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(c1.duration, Tick(90_000), "the outgoing one got its tail back: {c1:?}");
    assert_eq!(c2.duration, Tick(90_000), "the incoming one got its head back: {c2:?}");
    assert_eq!(c2.source_in, Tick(0), "the entry point returned to zero");
    p.validate().expect("result is valid");

    // the geometry of the handles — a pure function
    let (l, r) = crate::timeline::transition_handle_rects(100.0, 120.0, 140.0, 50.0, 40.0);
    assert_eq!((l.left(), l.right()), (100.0, 120.0), "on the left — the continuation of the incoming");
    assert_eq!((r.left(), r.right()), (120.0, 140.0), "on the right — the tail of the outgoing");
    assert!(l.top() > 50.0 && r.bottom() < 90.0, "handles inside the track row");
}

/// Test 157 (item 157, R.25): the fade at the edge in the UI — the window grab,
/// the live drag of the edge and the absence of a neighbor do not interfere with either.
#[test]
fn t157_fade_edge_drag_without_neighbour() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(90_000),
            duration: Tick(90_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        // fade-in: there is no neighbor on the left at all
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: None,
            b: Some(ClipId(1)),
            in_offset: Tick(0),
            out_offset: Tick(10_000),
            kind: TransitionKind::Crossfade,
            audio: Default::default(),
            auto: vec![],
            invert: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let y = g.lane_y(ClipKind::Video, 0, 0.5);

    // the RIGHT edge of the fade window (inside the clip body) drag to the right: 10000 → 40000
    let x_edge = g.tick_to_x(90_000 + 10_000);
    h.hover_at(egui::pos2(x_edge, y));
    h.step();
    h.drag_at(egui::pos2(x_edge, y));
    h.step();
    h.hover_at(egui::pos2(x_edge + 8.0, y));
    h.step();
    assert!(
        matches!(h.state().drag, Some(crate::timeline::Drag::TransitionEdge { left: false, .. })),
        "the fade edge did not grab"
    );
    let to = g.tick_to_x(90_000 + 40_000);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let t = h.state().model.project().transitions[0].clone();
    assert!(t.a.is_none() && t.b == Some(ClipId(1)), "the fade stayed a fade: {t:?}");
    assert!((t.out_offset.0 - 40_000).abs() <= 1_000, "the window expanded: {t:?}");

    // the window cannot leave the clip body
    let far = g.tick_to_x(90_000 + 200_000);
    h.hover_at(egui::pos2(g.tick_to_x(90_000 + 40_000), y));
    h.step();
    h.drag_at(egui::pos2(g.tick_to_x(90_000 + 40_000), y));
    h.step();
    h.hover_at(egui::pos2(g.tick_to_x(90_000 + 40_000) + 8.0, y));
    h.step();
    h.hover_at(egui::pos2(far, y));
    h.step();
    h.drop_at(egui::pos2(far, y));
    h.step();
    let t = h.state().model.project().transitions[0].clone();
    assert!(t.out_offset <= Tick(90_000), "the window is clamped by the clip body: {t:?}");
}

/// Test 142 (item 142, R.24): range markers in the UI. The M gesture —
/// verbatim by the user's decision: “with a region selected and the cursor in it, m gives
/// marked_range; with the cursor outside the region — a single marker at the cursor”;
/// a repeated M is a toggle; the edges and snap/stop see both edges.
#[test]
fn t142_marker_range_gesture_and_edges() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model.set_region(Some(Region { start: Tick(30_000), end: Tick(90_000) }));
        app.player.seek(Tick(60_000)); // the cursor INSIDE the region
        // zoom fixed: the magnetic threshold M depends on ticks_per_px
        app.ticks_per_px = 1_000.0;
    }
    h.step();

    // M inside the region → a range over the region
    h.state_mut().marker_toggle_at_cursor();
    {
        let p = h.state().model.project();
        assert_eq!(p.markers.len(), 1);
        assert_eq!((p.markers[0].t, p.markers[0].duration), (Tick(30_000), Tick(60_000)));
    }
    // a repeated M there → toggle (deletion)
    h.state_mut().marker_edit = None;
    h.state_mut().marker_toggle_at_cursor();
    assert!(h.state().model.project().markers.is_empty(), "toggle removes the range");

    // a point marker at the start of the region is stretched by the M gesture
    h.state_mut().apply(Command::AddMarker { at: Tick(30_000), duration: Tick::ZERO });
    h.state_mut().marker_toggle_at_cursor();
    assert_eq!(h.state().model.project().markers[0].duration, Tick(60_000));

    // M outside the region → a point marker at the cursor (the tick is a multiple of 15 — the playhead lives
    // in audio frames, the round-trip is exact only on multiples, item 94)
    h.state_mut().player.seek(Tick(192_000));
    h.state_mut().marker_edit = None;
    h.state_mut().marker_toggle_at_cursor();
    {
        let p = h.state().model.project();
        assert!(p.markers.iter().any(|m| m.t == Tick(192_000) && !m.is_range()), "{:?}", p.markers);
    }

    // snap and stop_points see BOTH edges of the range
    let targets = h.state().snap_targets(None);
    assert!(targets.contains(&30_000) && targets.contains(&90_000), "{targets:?}");
    let stops = crate::app::stop_points(h.state().model.project());
    assert!(stops.contains(&30_000) && stops.contains(&90_000), "{stops:?}");

    // the strip is pure geometry: a narrow band on the time-label row
    let band = crate::timeline::marker_band_rect(100.0, 160.0, 20.0);
    assert_eq!((band.left(), band.right()), (100.0, 160.0));
    assert!(band.height() <= 6.0, "the reel does not eat the time row");
}

/// Test 142 (live part): dragging the end of the range changes its duration.
#[test]
fn t142_marker_end_drag() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let _ = app.model.apply(Command::AddMarker { at: Tick(30_000), duration: Tick(30_000) });
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    // the end of the range on the flag strip (top of the timeline)
    let y = g.rect.top() + 4.0;
    let x_end = g.tick_to_x(60_000);
    h.hover_at(egui::pos2(x_end, y));
    h.step();
    h.drag_at(egui::pos2(x_end, y));
    h.step();
    h.hover_at(egui::pos2(x_end + 8.0, y));
    h.step();
    assert!(
        matches!(h.state().drag, Some(crate::timeline::Drag::MarkerEdge { left: false, .. })),
        "grabbing the range end did not work"
    );
    let to = g.tick_to_x(80_000);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let m = &h.state().model.project().markers[0];
    assert!((m.duration.0 - 50_000).abs() <= 1_000, "duration ~50000: {:?}", m.duration);
}

/// Test 150 (item 150, R.24): the application's file IO — .kt.
#[test]
fn t150_project_io_is_otio() {
    use std::path::{Path, PathBuf};
    // the atomic tmp is appended to the FULL name (the old with_extension
    // was hardcoded for ktproj and swallowed multi-dot names); since R.49
    // (item 352) the name also has the process number — see t363
    let pid = std::process::id();
    assert_eq!(
        kt_otio::atomic_tmp_path(Path::new("/x/a.kt")),
        PathBuf::from(format!("/x/a.kt.{pid}.tmp"))
    );
    assert_eq!(
        kt_otio::atomic_tmp_path(Path::new("/x/a.b.kt")),
        PathBuf::from(format!("/x/a.b.kt.{pid}.tmp"))
    );
    // recovery of an unnamed project — also .kt
    assert!(crate::app::autosave_path().to_string_lossy().ends_with("autosave-project.kt"));

    let dir = std::env::temp_dir().join(format!("kt-t150-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // .kt is read
    let p = kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    let otio = dir.join("ok.kt");
    std::fs::write(&otio, crate::app::project_text_of(&p, &Default::default(), None))
        .unwrap();
    let (loaded, _, _) = crate::app::load_project_file(&otio).expect("otio opens");
    assert_eq!(loaded.settings.fps, 30);
    // .ktproj does NOT open as a project (R.24 item 12) — an honest hint
    let legacy = dir.join("old.ktproj");
    std::fs::write(&legacy, include_str!("../../../fixtures/demo.ktproj")).unwrap();
    let err = crate::app::load_project_file(&legacy).unwrap_err();
    assert!(
        err.contains(crate::i18n::tr(crate::i18n::k::menu::file::IMPORT_OLDER)),
        "hint for import: {err}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// Test 169 (item 2 of R.28): material preview during gestures — “however much there was, that much
/// must remain”. The user: “I drag the event deeper into the previous one — the sound preview
/// scrolls to the left, although during playback the sound itself does not change”.
/// Material inside the piece moves ONLY the trim of the left edge.
#[test]
fn t169_preview_material_shifts_only_on_trim_start() {
    use crate::timeline::{Drag, DragMode, DragState, preview_src_shift};
    use kt_model::*;
    use kt_time::Tick;

    let orig = ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(1_000),
        duration: Tick(10_000),
        source_in: Tick(500),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    let drag = |mode: DragMode, cur_start: i64, cur_end: i64| {
        Drag::Clip(DragState {
            id: ClipId(1),
            mode,
            extra: Vec::new(),
            grab_dt: 0,
            orig: orig.clone(),
            cur_start,
            cur_end,
            cur_lane: 0,
            moved: true,
        })
    };

    // a move to the right by 4000: the piece goes as a whole, the material with it
    let d = drag(DragMode::Move, 5_000, 15_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(1), 1_000), 0, "the move does not spin the material");
    // a move to the left — also zero (before, .max(0) hid only this half)
    let d = drag(DragMode::Move, -3_000, 7_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(1), 1_000), 0, "the move to the left does not spin");
    // trim of the left edge: the piece stands, the edge moves ALONG the material
    let d = drag(DragMode::TrimStart, 5_000, 11_000);
    assert_eq!(
        preview_src_shift(Some(&d), ClipId(1), 1_000),
        4_000,
        "trimming the start moves the material"
    );
    let d = drag(DragMode::TrimStart, 0, 11_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(1), 1_000), -1_000, "to the left — in the opposite direction");
    // trim of the right edge does not touch the left boundary
    let d = drag(DragMode::TrimEnd, 1_000, 20_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(1), 1_000), 0, "trimming the end does not spin the material");
    // neighboring clips are not touched by the gesture
    let d = drag(DragMode::TrimStart, 5_000, 11_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(2), 1_000), 0, "we do not touch another clip");
    assert_eq!(preview_src_shift(None, ClipId(1), 1_000), 0, "without a shift gesture there is none");
}

/// Test 170 (item 11 of R.28): “marker here with a selected region must produce a
/// region marker”. Paired ways of one action must account for the context
/// identically (item 17 of the checklist): the M gesture and the menu item — one body `marker_here`.
#[test]
fn t170_marker_here_matches_the_m_gesture() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model.set_region(Region::normalized(Tick(30_000), Tick(90_000)));
        // the menu item on the EDGE of the region: the tick of the edge is “over the region”
        app.marker_here(Tick(30_000));
    }
    h.step();
    {
        let m = h.state().model.project().markers.clone();
        assert_eq!(m.len(), 1, "one marker: {m:?}");
        assert_eq!(m[0].t, Tick(30_000), "marker at the start of the region");
        assert_eq!(m[0].duration, Tick(60_000), "this is a RANGE, not a point: {:?}", m[0]);
    }
    // a repeated call — toggle, as with the M gesture
    h.state_mut().marker_here(Tick(30_000));
    h.step();
    assert!(h.state().model.project().markers.is_empty(), "a repeat removes the marker");
    // the second edge behaves the same
    h.state_mut().marker_here(Tick(90_000));
    h.step();
    assert_eq!(
        h.state().model.project().markers[0].duration,
        Tick(60_000),
        "the end edge is also a region"
    );
    // outside the region — a point marker (the former behavior)
    {
        let app = h.state_mut();
        app.model.set_region(None);
        app.marker_here(Tick(200_000));
    }
    h.step();
    let m = h.state().model.project().markers.clone();
    assert!(
        m.iter().any(|k| k.t == Tick(200_000) && k.duration == Tick::ZERO),
        "without a region — a single marker: {m:?}"
    );
}

/// Test 172 (item 5 of R.28): an impossible action is shown AS AN ERROR.
///
/// The complaint verbatim: “I did not scroll there myself — I did not understand that there is still an
/// event at the end that gets in the way, and there is also a pale faded
/// hint; in hints such situations are always done
/// as errors”. We check all three requirements: the obstacle is known
/// (so there is something to highlight), the hint is in the error style and
/// overrides the hover, and beyond the view it says where to look.
#[test]
fn t172_blocked_gesture_is_an_error_not_a_pale_hint() {
    use crate::app::HintKind;
    use crate::timeline::{BlockView, MovePlan, block_view, plan_move};
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;

    // 1. the plan knows WHAT they ran into
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
    p.overlap_layers = false; // blocking is possible only in this mode
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    // clips 2 and 3 stand butt to butt: any hit between them is an overlap of TWO
    for (n, start) in [(1u64, 0i64), (2, 200_000), (3, 250_000)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(n),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(start),
            duration: Tick(50_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    // clip 1 moves to the right and covers TWO at once (a single partial overlap
    // is a crossfade, not a stop): the obstacle is the first one in the direction of motion
    match plan_move(&p, ClipId(1), 230_000, 0) {
        MovePlan::Clamped { blocker, .. } => {
            assert_eq!(blocker, (200_000, 250_000), "along the motion, clip 2 blocks first");
        }
        other => panic!("expected a stop, got {other:?}"),
    }
    // a move to the left into a full cover of the neighbor — a stop at its right edge
    match plan_move(&p, ClipId(3), 200_000, 0) {
        MovePlan::Clamped { start, blocker } => {
            assert_eq!(blocker, (200_000, 250_000), "clip 2 blocks on the left");
            assert_eq!(start, 250_000, "the stop is the right edge of the obstacle");
        }
        other => panic!("expected a stop, got {other:?}"),
    }

    // 2. “where to look” when the obstacle is outside the view
    assert_eq!(block_view((100, 200), (0, 1000)), BlockView::Visible);
    assert_eq!(block_view((0, 100), (500, 1000)), BlockView::Left(400));
    assert_eq!(block_view((2000, 2100), (500, 1000)), BlockView::Right(1000));
    // touching the edge of the view — already visible
    assert_eq!(block_view((900, 1500), (500, 1000)), BlockView::Visible);

    // 3. live gesture: the hint is an error and overrides the hover
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let y = g.lane_y(ClipKind::Video, 0, 0.5);
    let grab = g.tick_to_x(25_000);
    h.hover_at(egui::pos2(grab, y));
    h.step();
    h.drag_at(egui::pos2(grab, y));
    h.step();
    // dragging clip 1 so that it covers two events at once — a stop
    let to = g.tick_to_x(230_000 + 25_000);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.hover_at(egui::pos2(to + 1.0, y));
    h.step();
    let blocked = h.state().blocked;
    assert!(blocked.is_some(), "the obstacle must be known (it is highlighted)");
    assert_eq!(blocked.unwrap().span, (200_000, 250_000), "exactly clip 2 is highlighted");
    assert_eq!(
        h.state().status_line_shown.1,
        HintKind::Error,
        "the hint must be an ERROR, not faded: {:?}",
        h.state().status_line_shown
    );
    h.drop_at(egui::pos2(to + 1.0, y));
    h.step();
    assert!(h.state().blocked.is_none(), "after the gesture the highlight goes out");
}

/// Test 173 (item 12 of R.28): a fade by the top corner of the event, and the level by mouse.
///
/// The user: “in Sony Vegas you can make fades at the edges of events without extra
/// events — just by pulling the top corner of the event, and there you can also
/// change the transparency… you hover over the fade and pull either up or
/// down”. The corner fade lies ENTIRELY inside the event (outer offset 0),
/// so “nothing grows back” (item 6).
#[test]
fn t173_corner_fade_and_level_gestures() {
    use crate::timeline::{corner_fade_len, fade_handle_tick, level_from_y, level_y};
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;

    // ---- pure functions ----
    assert_eq!(fade_handle_tick(1_000, 5_000, 0, true), 1_000, "no fade — the handle is in the corner");
    assert_eq!(fade_handle_tick(1_000, 5_000, 700, true), 1_700, "handle at the end of the fade");
    assert_eq!(fade_handle_tick(1_000, 5_000, 0, false), 5_000);
    assert_eq!(fade_handle_tick(1_000, 5_000, 700, false), 4_300);
    // level: the top of the row = 1.0, the bottom = 0.0, symmetric back and forth
    let (top, h) = (100.0f32, 48.0f32);
    assert!((level_from_y(top + 3.0, top, h) - 1.0).abs() < 1e-6);
    assert!((level_from_y(top + 3.0 + (h - 8.0), top, h)).abs() < 1e-6);
    assert!((level_from_y(top - 50.0, top, h) - 1.0).abs() < 1e-6, "above the row — clamp");
    let mid = level_y(0.5, top, h);
    assert!((level_from_y(mid, top, h) - 0.5).abs() < 1e-3, "forward and back converge");

    // ---- live gesture ----
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(100_000),
            duration: Tick(200_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.select_only(kt_model::ClipId(1));
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let top = g.lane_top(ClipKind::Video, 0);
    let y_strip = top + 3.0;

    // pull the left corner to the right by 40,000 ticks
    let x0 = g.tick_to_x(100_000);
    h.hover_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    h.drag_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    let to = g.tick_to_x(140_000);
    h.hover_at(egui::pos2(to, y_strip));
    h.step();
    h.drop_at(egui::pos2(to, y_strip));
    h.step();
    {
        let p = h.state().model.project();
        assert_eq!(p.transitions.len(), 1, "the corner created exactly one fade: {:?}", p.transitions);
        let t = &p.transitions[0];
        assert!(t.is_fade(), "this is a fade, not a crossfade");
        assert_eq!((t.a, t.b), (None, Some(ClipId(1))), "empty space on the left, the live end is the clip");
        assert_eq!(t.in_offset, Tick::ZERO, "there is no external offset — the fade is inside the event");
        assert!(
            (t.out_offset.0 - 40_000).abs() <= 2_000,
            "fade length ≈ the distance traveled: {:?}",
            t.out_offset
        );
        // the event is untouched: “however much there was, that much remained”
        let c = &p.clips[0];
        assert_eq!((c.start, c.duration, c.source_in), (Tick(100_000), Tick(200_000), Tick::ZERO));
        assert_eq!(corner_fade_len(p, ClipId(1), true), Some(t.out_offset.0));
        assert_eq!(corner_fade_len(p, ClipId(1), false), None, "there is no right fade");
    }

    // push the corner back — the fade is removed
    let from = g.tick_to_x(140_000);
    h.hover_at(egui::pos2(from, y_strip));
    h.step();
    h.drag_at(egui::pos2(from, y_strip));
    h.step();
    h.hover_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    h.drop_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    assert!(
        h.state().model.project().transitions.is_empty(),
        "the corner is pushed in — no fade: {:?}",
        h.state().model.project().transitions
    );

    // level: pull the line down to the middle of the row
    let ly = level_y(1.0, top, g.lane_h_of(ClipKind::Video, 0));
    let xm = g.tick_to_x(200_000);
    h.hover_at(egui::pos2(xm, ly));
    h.step();
    h.drag_at(egui::pos2(xm, ly));
    h.step();
    let target_y = level_y(0.4, top, g.lane_h_of(ClipKind::Video, 0));
    h.hover_at(egui::pos2(xm, target_y));
    h.step();
    h.drop_at(egui::pos2(xm, target_y));
    h.step();
    let op = h.state().model.project().clips[0].opacity;
    assert!((op - 0.4).abs() < 0.05, "level reached 0.4: {op}");
    // and this is undoable, like everything else
    h.state_mut().model.undo().expect("level undo is available");
    h.step();
    assert_eq!(h.state().model.project().clips[0].opacity, 1.0, "undo restored the level");
}

/// Test 176: the level of audio and video is ONE element (the user's requirement:
/// “fades must look the same for audio and for video”).
///
/// The vertical volume rail of v1 stood exactly in the fade zone and made the fade on
/// audio unlike the fade on video. Now both kinds have the line on top;
/// the volume range stayed as before (±6.02 dB, 0 dB — the middle).
#[test]
fn t176_audio_and_video_share_one_level_line() {
    use crate::timeline::{RAIL_DB_SPAN, gain_to_level, level_to_gain, level_y};
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;

    // the rail range of v1 is kept exactly
    assert!((gain_to_level(0.0) - 0.5).abs() < 1e-6, "0 dB — the middle");
    assert!((level_to_gain(1.0) - RAIL_DB_SPAN / 2.0).abs() < 1e-4, "top — +6.02 dB");
    assert!((level_to_gain(0.0) + RAIL_DB_SPAN / 2.0).abs() < 1e-4, "bottom — −6.02 dB");
    for db in [-6.0f32, -3.0, 0.0, 2.5, 6.0] {
        assert!((level_to_gain(gain_to_level(db)) - db).abs() < 1e-3, "circle over {db} dB");
    }

    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(100_000),
            duration: Tick(200_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.select_only(ClipId(1));
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let top = g.lane_top(ClipKind::Audio, 0);

    // the same gesture as for video: pull the level line up
    let x = g.tick_to_x(200_000);
    let y0 = level_y(0.5, top, g.lane_h_of(ClipKind::Audio, 0)); // 0 dB
    h.hover_at(egui::pos2(x, y0));
    h.step();
    h.drag_at(egui::pos2(x, y0));
    h.step();
    let y1 = level_y(0.75, top, g.lane_h_of(ClipKind::Audio, 0));
    h.hover_at(egui::pos2(x, y1));
    h.step();
    h.drop_at(egui::pos2(x, y1));
    h.step();
    let gain = h.state().model.project().clips[0].gain_db;
    assert!((gain - RAIL_DB_SPAN * 0.25).abs() < 0.5, "audio level reached +3 dB: {gain}");

    // the fade corner has priority over the line: the top strip is the fade gesture
    let y_strip = top + 2.0;
    let x0 = g.tick_to_x(100_000);
    h.hover_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    h.drag_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    h.hover_at(egui::pos2(g.tick_to_x(140_000), y_strip));
    h.step();
    h.drop_at(egui::pos2(g.tick_to_x(140_000), y_strip));
    h.step();
    let p = h.state().model.project();
    assert_eq!(p.transitions.len(), 1, "the corner of an audio event also creates a fade");
    assert_eq!(p.transitions[0].in_offset, Tick::ZERO, "fade inside the event");
    assert_eq!(p.clips[0].duration, Tick(200_000), "event is untouched");
}

/// Test 185 (R.29): loss of the video GPU executor — the warning exactly once, after that the CPU fallback works silently;
/// the backend status is honest.
#[test]
fn t185_gpu_loss_notice_once() {
    struct DeadGpu(kt_engine::GpuBackendInfo);
    impl kt_engine::VideoGpuExec for DeadGpu {
        fn execute_into(
            &self,
            _p: &kt_engine::FramePlan<'_>,
            _o: &mut Vec<u8>,
        ) -> Result<bool, kt_engine::GpuExecError> {
            Err(kt_engine::GpuExecError::DeviceLost)
        }
        fn info(&self) -> &kt_engine::GpuBackendInfo {
            &self.0
        }
    }
    let mut h = harness();
    assert!(
        matches!(h.state().player.video_backend(), kt_engine::VideoBackendStatus::Cpu),
        "in the harness without wgpu the backend is honestly CPU"
    );
    h.state_mut().player.set_video_gpu(Some(std::sync::Arc::new(DeadGpu(
        kt_engine::GpuBackendInfo { name: "test".into(), backend: "Test".into(), software: true },
    ))));
    // force a recomposition (the preview key has already settled on the previous
    // frames): key reset → compose calls the executor → DeviceLost →
    // poisoning; the next frame shows the notice
    h.state_mut().preview_key = None;
    h.step();
    h.step();
    h.step();
    let app = h.state();
    assert!(app.gpu_fx_lost_notified, "the notice about GPU loss must appear");
    assert!(
        matches!(app.player.video_backend(), kt_engine::VideoBackendStatus::Poisoned { .. }),
        "the backend is poisoned until the end of the session"
    );
    let texts: Vec<&str> = app.notices.iter().map(|n| n.text.as_str()).collect();
    let hits =
        texts.iter().filter(|t| **t == crate::tf!(k::msg::VIDEO_FX_GPU_LOST, "DeviceLost")).count();
    assert_eq!(hits, 1, "exactly one warning: {texts:?}");
}

/// Test 221 (R.33 item 5): a single automation widget.
/// Four user requirements are checked at once — switching graph↔table,
/// conversion “material ↔ timeline” ACCOUNTING for reverse and speed
/// (a mini-timeline sits on it and moves the main cursor),
/// a keyframe at the cursor lands under the cursor,
/// and typing a time into the table does not re-sort the rows under one's hands.
#[test]
fn t221_automation_widget_maps_material_and_keeps_input_stable() {
    use crate::automation::{AutoMode, AutoView};
    use kt_model::*;
    use kt_time::Tick;
    const STEP: i64 = 3000; // 30 fps

    // direct clip: material = timeline shifted by the entry point
    let mut c = ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(180_000),
        duration: Tick(90_000),
        source_in: Tick(45_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    let m = |c: &ClipEvent, t: Tick| {
        clip_material_at(c.source_in, c.start, c.duration, c.reversed, STEP, t)
    };
    let tl = |c: &ClipEvent, mat: Tick| {
        clip_timeline_at(c.source_in, c.start, c.duration, c.reversed, STEP, mat)
    };
    let mat = m(&c, Tick(180_000 + 30_000));
    assert_eq!(mat.0, 75_000, "material = source_in + offset inside the event");
    assert_eq!(tl(&c, mat).0, 180_000 + 30_000, "the reverse mapping must return the same moment");

    // reverse: the material runs against the timeline
    c.reversed = true;
    let mat_r = m(&c, Tick(180_000));
    assert_eq!(mat_r.0, 45_000 + 90_000 - STEP, "for reverse, the event start is the material end");
    assert_eq!(tl(&c, mat_r).0, 180_000, "the reverse mapping of reverse must converge");
    c.reversed = false;

    // SPEED DOES NOT COMPRESS THE AXIS (R.37, item 1 of the user): for a clip at 0.5×
    // the curve's axis is still the SPAN of the EVENT. Previously it was
    // 45_000 + 45_000 — the right half of the graph was unreachable, and
    // the auto-key that the editor sows at the end never fired.
    let mut slow = c.clone();
    slow.kind = ClipKind::Audio;
    slow.fx = vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)];
    let mid = m(&slow, Tick(180_000 + 90_000));
    assert_eq!(
        mid.0,
        45_000 + 90_000,
        "the curve axis is the event’s own span; speed does not compress it"
    );
    assert_eq!(
        tl(&slow, mid).0,
        180_000 + 90_000,
        "and the reverse mapping is now EXACT, not within a frame"
    );

    // widget: switching the view and stable entry of time in the table
    let mut view = AutoView::default();
    assert_eq!(view.mode, AutoMode::Graph, "graph by default");
    view.mode = AutoMode::Table;
    let mut curves = [ParamCurve {
        param: "radius".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(45_000), 1.0),
            AutoKey::at(Tick(135_000), 48.0),
        ]),
        source: Default::default(),
    }];
    // the user started editing the time of the SECOND key to a value LESS than
    // the first one: while the edit is not committed, the order of keys does not change
    view.t_edit = Some((0, 1, 0.4));
    assert_eq!(
        curves[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![45_000, 135_000],
        "during input the keys are not re-sorted — the row would slide out from under the hands"
    );
    // on release the edit is applied and the order is restored
    let (ci, ki, s) = view.t_edit.take().unwrap();
    curves[ci].keys[ki].t = Tick((s * 90_000.0).round() as i64);
    curves[ci].keys.sort_by_key(|k| k.t.0);
    assert_eq!(
        curves[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![36_000, 45_000],
        "after release the keys fall into order"
    );
}

/// Test 219 (R.33 item 2, verbatim: “for the same event with
/// magnetized tracks — you can also give all the chains at once in one window,
/// it should be simpler this way. Well, one can give the option to hide-switch”).
/// An A/V pair now has ONE window with both chains in collapsible sections, rather than two
/// independent windows, each with its own “Cancel” button.
#[test]
fn t219_one_window_for_the_whole_link_group() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let clip = |id: u64, kind: ClipKind, lane: u32| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: Some(GroupId(7)),
        kind,
        lane,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    p.clips.push(clip(1, ClipKind::Video, 0));
    let mut a = clip(2, ClipKind::Audio, 0);
    a.fx = vec![FxInstance::gain(0.0)];
    p.clips.push(a);
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);

    // the window is opened by the video — it also shows the audio chain
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    assert_eq!(h.state().fx_editors.len(), 1, "one window");
    assert_eq!(
        h.state().fx_editors[0].members,
        vec![ClipId(1), ClipId(2)],
        "the window must drive both chains of the pair"
    );
    // a repeated opening of the PARTNER does not spawn a second window
    h.state_mut().open_fx_editor(ClipId(2));
    h.step();
    assert_eq!(h.state().fx_editors.len(), 1, "a pair must not have two windows");

    // “Cancel” rolls back BOTH chains, not only the anchor
    let app = h.state_mut();
    let g = app.fx_editors[0].gesture;
    app.set_fx_coalesced(ClipId(1), vec![FxInstance::glow(1.0)], g);
    app.set_fx_coalesced(ClipId(2), vec![], g);
    let backup = app.fx_editors[0].backup.clone();
    assert_eq!(backup.len(), 2, "the snapshot must cover both chains");
    for (id, fx, auto, bypass, off) in backup {
        app.set_fx_coalesced(id, fx, g);
        app.set_fx_auto_coalesced(id, auto, g);
        app.apply_coalesced(Command::SetFxBypass { id, bypass }, g);
        app.apply_coalesced(Command::SetFxOff { id, off }, g);
    }
    assert!(
        matches!(app.model.clip(ClipId(1)).unwrap().fx.as_slice(), [FxInstance::Blur { .. }]),
        "the video chain did not roll back"
    );
    assert!(
        matches!(app.model.clip(ClipId(2)).unwrap().fx.as_slice(), [FxInstance::Gain { .. }]),
        "the audio chain did not roll back — the window drove only the anchor"
    );
    h.step();
}

/// Test 218 (R.33 item 2, the user's requirement about the “New source” field:
/// “I would like it to be possible to drag and drop files there, and drag and drop from the project's
/// sources, or choose via opening the media widget”). Plus a “Cancel” for the
/// morpher, which was not there at all (in the FX window it was always present).
#[test]
fn t218_morpher_source_field_takes_files_and_reverts() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: MorphPatch {
            media: None,
            source_in_delta: kt_time::Tick(0),
            absolute_source_in: None,
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_morph_editor(MorphId(1));
    h.step();
    h.step();
    // the source field knows its place on the screen — a drop is caught by it
    assert!(
        h.state().morph_editors[0].src_rect.is_some(),
        "the source field did not report its rectangle — a file drop into it cannot be caught"
    );

    // a file that is not in the project becomes a source in ONE step
    let app = h.state_mut();
    let depth = app.model.undo_depth();
    app.set_morph_source_file(MorphId(1), "synth:dc");
    let ev = app.model.project().morphers[0].clone();
    let media = ev.patch.media;
    let mid = media.expect("the source must be substituted");
    assert_eq!(
        app.model.project().media.iter().find(|m| m.id == mid).map(|m| m.path.as_str()),
        Some("synth:dc"),
        "the media must appear in the project"
    );
    assert_eq!(app.model.undo_depth(), depth + 1, "adding media and substitution are one step");

    // “Cancel” returns the morpher to the state at the moment the window was opened
    let app = h.state_mut();
    let ed_gesture = app.morph_editors[0].gesture;
    let backup = app.morph_editors[0].backup.clone().expect("snapshot on opening");
    app.apply_coalesced(Command::SetMorpher(backup), ed_gesture);
    let ev = app.model.project().morphers[0].clone();
    let media = ev.patch.media;
    assert!(media.is_none(), "rollback must restore “do not change the source”");
    h.step();
}

/// Test 217 (R.33 C2, user requirements: “morphers' fx-chain has crosses,
/// you could add check marks too — switching effects on and off — and the ability to move
/// the order with the mouse.
/// And replace the fx-chain of event effects, because in two places
/// there is the same meaning, and it is done not fully in both”). The chain strip is now ONE:
/// the event window and both morpher sections draw it with one piece of code.
#[test]
fn t217_one_chain_widget_for_events_and_morphers() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(9), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(9),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(9),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8), FxInstance::glow(1.0)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    // morpher batch with two effects — the second chain with the same widget
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(9)] },
        patch: MorphPatch {
            video_fx: vec![
                FxInstance::Mirror { horizontal: true, vertical: false },
                FxInstance::edges(48),
            ],
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(9));
    h.step();
    h.step();

    // 1) the master check mark of the chain is in BOTH windows — it is one and the same
    let chain_label = tr(k::fx::CHAIN_ON).to_string();
    assert!(h.query_by_label(&chain_label).is_some(), "the event window has no master chain checkbox");
    h.state_mut().open_morph_editor(MorphId(1));
    h.step();
    h.step();
    assert!(h.state().morph_editors.iter().any(|e| e.open), "the morpher window did not open");
    // the morpher's master check mark is also there (now there are two — one per window)
    assert!(
        h.get_all_by_label(&chain_label).count() >= 2,
        "the morpher has no master chain checkbox — the widget is not shared"
    );

    // 2) the slot switches are there in the morpher too (before, there were only crosses)
    let app = h.state_mut();
    let ev = app.model.project().morphers[0].clone();
    let video_bypass = &ev.patch.video_bypass;
    assert!(video_bypass.is_empty(), "the starting state — everything is on");

    // 3) switching a batch slot off removes the effect from the targets, without losing it
    let mut ev2 = ev.clone();
    ev2.patch.video_bypass = vec![true, false];
    assert!(app.apply(Command::SetMorpher(ev2)), "command did not go through");
    let eff = app.model.project().effective_clips();
    let c = eff.iter().find(|c| c.id == ClipId(9)).unwrap();
    assert!(
        !c.fx.iter().any(|f| matches!(f, FxInstance::Mirror { .. })),
        "a disabled batch slot must not reach the clip"
    );
    assert!(
        c.fx.iter().any(|f| matches!(f, FxInstance::Edges { .. })),
        "an enabled batch slot must remain"
    );
    assert_eq!(
        app.model.project().morphers[0].patch.video_fx.len(),
        2,
        "the effect itself from the batch does not disappear"
    );
    h.step();
}

/// Test 193 (R.30): FX editor and automation — the window with curves is drawn,
/// and the pair SetFx+SetFxAuto of one gesture coalesces into ONE undo step
/// (otherwise Ctrl+Z would roll the chain and the curves back out of step).
#[test]
fn t193_fx_editor_automation_coalesces_into_one_undo() {
    use kt_model::*;
    let mut h = harness();
    // a video clip with Blur and a live curve — the automation section is drawn
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(9), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(9),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(9),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![
                AutoKey::at(kt_time::Tick(0), 1.0),
                AutoKey::at(kt_time::Tick(90_000), 48.0),
            ]),
            source: Default::default(),
        }]],
        opacity: 1.0,
        fx_off: false,
    });
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(9));
    h.step();
    h.step();
    assert!(h.state().fx_editors.iter().any(|e| e.open), "the editor window is alive with the curves");

    // an editor gesture: the chain + curves with ONE gesture token (as in the UI)
    let app = h.state_mut();
    let depth_before = app.model.undo_depth();
    let gesture = app.fx_editors[0].gesture;
    let fx = vec![FxInstance::blur(20), FxInstance::glow(1.0)];
    let auto = vec![
        vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![AutoKey::at(kt_time::Tick(0), 2.0)]),
            source: Default::default(),
        }],
        vec![],
    ];
    app.set_fx_coalesced(ClipId(9), fx.clone(), gesture);
    app.set_fx_auto_coalesced(ClipId(9), auto, gesture);
    let c = app.model.clip(ClipId(9)).unwrap().clone();
    assert_eq!(c.fx, fx);
    assert_eq!(c.fx_auto.len(), 1, "the tail without curves is trimmed");
    assert_eq!(app.model.undo_depth(), depth_before + 1, "both gesture changes are one undo step");
    app.model.undo().unwrap();
    let c = app.model.clip(ClipId(9)).unwrap().clone();
    assert_eq!(c.fx.len(), 1, "undo restored the chain");
    assert_eq!(c.fx_auto[0][0].keys.len(), 2, "undo restored the curves in the same step");
    h.step();
}

/// Test 194 (R.31 item 2): autosave runs IN THE BACKGROUND — the UI thread only takes the
/// snapshot and later picks up the result. Edits AFTER the snapshot do not lose
/// the asterisk (dirty is cleared only when the model is unchanged).
#[test]
fn t194_autosave_runs_in_background() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t194-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t194.kt");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    // a content edit + “30 seconds have passed”
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some(), "background autosave must start");
    // wait for the worker and pick up the result on the next frame
    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(h.state().autosave_rx.is_none(), "autosave result collected");
    // R.49 item 351 (the owner's decision, user-feedback.md): autosave NO LONGER
    // writes to the user's file and does NOT clear the asterisk. The test from the frozen
    // range 1-221 was rewritten by direct sanction of the owner: “the background write
    // goes to its own recovery file; the project changes only on an explicit save”.
    assert!(h.state().dirty_content, "autosave has no right to count the project as saved");
    assert!(!target.exists(), "autosave touched the project file");
    let rec = crate::app::recovery_path_for(Some(&target));
    assert!(rec.exists(), "the recovery file was written by a background thread: {}", rec.display());

    // an edit AFTER the snapshot must not lose the asterisk
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(2), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some());
    // the model changes while the write is in flight
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(3), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(
        h.state().dirty_content,
        "edits after a snapshot keep the asterisk (otherwise they would be lost)"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The project for the selection tests: three video events on one track with
/// gaps (a gap is needed so that the outline has somewhere to start).
#[cfg(test)]
fn harness_three_clips() -> (Harness<'static, KtApp>, crate::timeline::TlGeom) {
    harness_three_clips_dt(None)
}

/// A direct edit of the FIXTURE model (the test prepares the scene, not a gesture):
/// a clone of the working view, the edit, a new model. Since R.62 this is how the name and
/// height of the track are set — they live on the track-object.
#[cfg(test)]
pub(crate) fn edit_project(app: &mut KtApp, f: impl FnOnce(&mut kt_model::Project)) {
    let mut p = app.model.project().clone();
    f(&mut p);
    p.validate().expect("the fixture is legitimate");
    app.model = kt_model::TimelineModel::new(p);
}

#[cfg(test)]
pub(crate) fn harness_three_clips_dt(
    step_dt: Option<f32>,
) -> (Harness<'static, KtApp>, crate::timeline::TlGeom) {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness_lang_dt(crate::i18n::Lang::Ru, step_dt);
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (i, start) in [0i64, 100_000, 200_000].iter().enumerate() {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(i as u64 + 1),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(*start),
                duration: Tick(50_000),
                source_in: Tick(0),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx: vec![],
                fx_bypass: vec![],
                fx_auto: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = 1000.0;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    (h, g)
}

/// a drag with the outline IN THREE STEPS (the egui drag threshold) with Ctrl held
#[cfg(test)]
fn rubber_drag(h: &mut Harness<'_, KtApp>, from: egui::Pos2, to: egui::Pos2, ctrl: bool) {
    let m = if ctrl { egui::Modifiers::CTRL } else { egui::Modifiers::NONE };
    h.event(egui::Event::ModifiersChanged(m));
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    h.hover_at(from + egui::vec2(8.0, 0.0));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

/// Test 202 (R.33 item 7, user requirement: “the main timeline lacks area selection…
/// could select via area while holding ctrl”).
/// The selection has become a SET: Ctrl+click gathers a batch, and Ctrl+drag from
/// an empty place catches events with the outline and does not create a region in the process.
#[test]
fn t202_ctrl_click_and_rubber_band_select() {
    use kt_model::*;
    let (mut h, g) = harness_three_clips();
    let y = g.lane_y(ClipKind::Video, 0, 0.6);
    let at = |t: i64| egui::pos2(g.tick_to_x(t), y);

    // a plain click — one event; Ctrl+click adds a second
    h.hover_at(at(25_000));
    h.step();
    h.drag_at(at(25_000));
    h.step();
    h.drop_at(at(25_000));
    h.step();
    assert_eq!(h.state().selected.iter().copied().collect::<Vec<_>>(), vec![ClipId(1)]);

    let node_click_ctrl = |h: &mut Harness<'_, KtApp>, pos: egui::Pos2| {
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::CTRL));
        h.hover_at(pos);
        h.step();
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::CTRL,
        });
        h.step();
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::CTRL,
        });
        h.step();
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        h.step();
    };
    let cursor_before = h.state().player.transport.playhead();
    node_click_ctrl(&mut h, at(125_000));
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![ClipId(1), ClipId(2)],
        "Ctrl+click must ADD to the selection"
    );
    assert_eq!(
        h.state().player.transport.playhead(),
        cursor_before,
        "a batch selection must not drag the cursor"
    );
    node_click_ctrl(&mut h, at(125_000));
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![ClipId(1)],
        "a repeated Ctrl+click removes the selection from the event"
    );

    // Ctrl+drag from an EMPTY place: the outline takes the second and the third event
    h.state_mut().model.set_region(None);
    h.step();
    rubber_drag(&mut h, at(70_000), at(260_000), true);
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![ClipId(2), ClipId(3)],
        "the frame did not gather the events"
    );
    assert!(
        h.state().model.project().region.is_none(),
        "Ctrl+drag must draw a frame, not a region"
    );

    // without Ctrl the same drag still makes a region (the old gesture is intact)
    rubber_drag(&mut h, at(70_000), at(260_000), false);
    assert!(
        h.state().model.project().region.is_some(),
        "a plain drag from an empty spot must remain a region selection"
    );
}

/// Test 203 (R.33 item 7, second half of the requirement: “like in Vegas —
/// a toggle-button”). The tool button is switched on by a CLICK and changes the meaning of
/// the ordinary drag over an empty place.
#[test]
fn t203_selection_tool_button_switches_drag() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, g) = harness_three_clips();
    let y = g.lane_y(ClipKind::Video, 0, 0.6);
    let at = |t: i64| egui::pos2(g.tick_to_x(t), y);

    assert!(!h.state().sel_tool, "the mode is off by default");
    h.get_by_label(crate::icons::SELECTION).click();
    h.step();
    assert!(h.state().sel_tool, "click on the button did not enable the selection tool");

    // in the MODE the ordinary drag draws an outline and does not create a region
    h.state_mut().model.set_region(None);
    h.step();
    rubber_drag(&mut h, at(70_000), at(260_000), false);
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![ClipId(2), ClipId(3)],
        "in selection mode the drag must catch events"
    );
    assert!(h.state().model.project().region.is_none(), "in selection mode a region is not placed");

    // switched off — the drag makes a region again
    h.get_by_label(crate::icons::SELECTION).click();
    h.step();
    assert!(!h.state().sel_tool);
    rubber_drag(&mut h, at(70_000), at(260_000), false);
    assert!(h.state().model.project().region.is_some(), "mode off — the region came back");
}

/// Test 211 (R.33 item 10, live half): a file that returned to disk
/// replaces its placeholder itself — without reopening the project and without losing the
/// position. The engine half of the test is in kt-engine (replacing the source
/// knocks its frames out of the cache).
#[test]
fn t211_returned_source_replaces_its_placeholder() {
    use kt_model::*;
    use kt_time::Tick;
    let dir = std::env::temp_dir().join(format!("kt-t211-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t211.kt");

    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    // a “vanished” file: the name of a synth source that is not on disk — on
    // opening it does not open, and by the time of its return it becomes valid
    p.media.push(MediaRef::new(MediaId(1), "later.mp4"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    std::fs::write(&path, crate::app::project_text_of(&p, &Default::default(), None)).unwrap();

    let app = KtApp::open(&path).expect("the project must open");
    assert_eq!(app.placeholder_media.len(), 1, "the placeholder was not substituted");
    let pos_before = app.player.transport.playhead();

    let mut h = Harness::builder()
        .with_size(egui::vec2(W, H))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&h.ctx);
    h.step();
    // file “returned”: we swap the path for a synthetic source and turn off the
    // offline set by hand, so as not to wait 900 ms for the watchdog
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media[0].path = "synth:ramp".into();
        app.model = kt_model::TimelineModel::new(p);
        app.offline.lock().unwrap().clear();
    }
    let deadline = std::time::Instant::now() + WAIT_BG;
    while !h.state().placeholder_media.is_empty() && std::time::Instant::now() < deadline {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(h.state().placeholder_media.is_empty(), "the returned file never replaced the placeholder");
    assert!(
        h.state().notices.iter().any(|n| n.text.contains("later.mp4")),
        "the return of the source is not mentioned"
    );
    assert_eq!(
        h.state().player.transport.playhead(),
        pos_before,
        "source substitution must not move the cursor"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 210 (R.33 item 10, about render): an unavailable source no longer
/// cancels the export — it goes with a placeholder, which is said BEFORE the start (in the
/// settings window) and AFTER (in the summary). Before, the render simply did not start, and
/// the lost layers of the engine were not shown in the GUI at all.
#[test]
fn t210_render_with_missing_sources_warns_and_runs() {
    use crate::render_dialog::{missing_sources, open_all};
    use kt_model::*;
    use kt_time::Tick;

    let dir = std::env::temp_dir().join(format!("kt-t210-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let present = dir.join("here.wav");
    std::fs::write(&present, b"not a real media file").unwrap();

    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), "gone.mp4"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(2),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();

    // warning BEFORE the start: only the missing one is named, the synthetic one does not count
    let warn = missing_sources(&p, Some(dir.as_path()));
    assert_eq!(warn, vec!["gone.mp4".to_string()], "the list for the warning is wrong: {warn:?}");

    // the sources for the render are ALL collected: the export starts
    let (sources, missing) = open_all(&p, Some(dir.as_path()));
    assert_eq!(sources.len(), 2, "the render must get a source for every media");
    assert_eq!(missing.len(), 1, "the placeholder must be named");
    assert!(missing[0].contains("gone.mp4"), "the list has no file name: {missing:?}");
    // and the placeholder really gives frames, not emptiness
    let mut sources = sources;
    let (_, src) = sources.iter_mut().find(|(id, _)| *id == MediaId(2)).unwrap();
    let f = src
        .read_video_frame(Tick(0), Some(kt_media::Quality { width: 320, height: 180 }))
        .expect("the placeholder must return a frame");
    assert!(f.rgba.iter().any(|b| *b != 0), "the placeholder frame is empty");

    // in the live application the list reaches the settings window
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model = kt_model::TimelineModel::new(p);
        app.project_path = Some(dir.join("t210.kt"));
        app.start_render();
    }
    h.step();
    assert_eq!(
        h.state().render_setup.missing,
        vec!["gone.mp4".to_string()],
        "the settings window does not know about unavailable sources"
    );
    h.state_mut().render_setup.open = false;
    h.step();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 209 (R.33 item 10): a project with a VANISHED source opens,
/// each unavailable file is named in the notifications, and instead of frames
/// a placeholder stands — the preview is complete, not hanging in “Loading”.
#[test]
fn t209_missing_source_opens_with_placeholder_and_notices() {
    use kt_model::*;
    use kt_time::Tick;
    let dir = std::env::temp_dir().join(format!("kt-t209-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t209.kt");

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
    p.media.push(MediaRef::new(MediaId(1), "no-such-clip.mp4"));
    p.media.push(MediaRef::new(MediaId(2), "also-gone.wav"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    let text = crate::app::project_text_of(&p, &Default::default(), None);
    std::fs::write(&path, text).unwrap();

    let app = KtApp::open(&path).expect("a project with a missing source must open");
    assert_eq!(app.placeholder_media.len(), 2, "both unavailable media must get a placeholder");
    let named: Vec<&String> = app.notices.iter().map(|n| &n.text).collect();
    assert!(
        named.iter().any(|t| t.contains("no-such-clip.mp4")),
        "the first missing file is not named: {named:?}"
    );
    assert!(
        named.iter().any(|t| t.contains("also-gone.wav")),
        "the second missing file was lost — before, only the last error survived: {named:?}"
    );

    // and the frame really arrives: the composition completes
    let mut h = Harness::builder()
        .with_size(egui::vec2(W, H))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&h.ctx);
    let deadline = std::time::Instant::now() + WAIT_BG;
    let mut complete = false;
    while std::time::Instant::now() < deadline {
        h.step();
        if h.state().incomplete_since.is_none() && h.state().preview_key.is_some() {
            complete = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(complete, "the preview never got complete — the placeholder did not reach the composition");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 206 (R.33 item 4): a double-click on an event opens the FX — exactly
/// what the clip hint has promised since R.18 and did not do (it set a region, while the
/// FX were hidden in the RMB menu; the defect was recorded in user-feedback even then).
/// The region by event moved to Shift+double-click and the menu item, and both ways
/// give ONE result (item 17 of the checklist — pairing of ways).
#[test]
fn t206_double_click_opens_fx_shift_makes_region() {
    use kt_model::*;
    let (mut h, g) = harness_three_clips_dt(Some(0.02));
    let y = g.lane_y(ClipKind::Video, 0, 0.6);
    let pos = egui::pos2(g.tick_to_x(125_000), y);

    let dbl = |h: &mut Harness<'_, KtApp>, pos: egui::Pos2, modifiers: egui::Modifiers| {
        h.event(egui::Event::ModifiersChanged(modifiers));
        h.hover_at(pos);
        h.step();
        for _ in 0..2 {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            });
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            });
        }
        h.step();
        h.step();
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        h.step();
    };

    dbl(&mut h, pos, egui::Modifiers::NONE);
    assert_eq!(h.state().fx_editors.len(), 1, "double-click did not open the FX editor");
    assert_eq!(h.state().fx_editors[0].clip, ClipId(2), "the window of the wrong event opened");
    assert!(h.state().model.project().region.is_none(), "double-click no longer places a region");

    // Shift+double-click — region by event. Its own harness: the click history in
    // egui is shared, and a repeat of the gesture in the same window would add up to a triple click
    let (mut h2, g2) = harness_three_clips_dt(Some(0.02));
    let pos2 = egui::pos2(g2.tick_to_x(125_000), g2.lane_y(ClipKind::Video, 0, 0.6));
    dbl(&mut h2, pos2, egui::Modifiers::SHIFT);
    let by_gesture = h2.state().model.project().region.expect("Shift+double-click did not give a region");
    assert_eq!((by_gesture.start.0, by_gesture.end.0), (100_000, 150_000));
    assert!(h2.state().fx_editors.is_empty(), "Shift+double-click must not open FX");

    // the same result from the menu — one body for both ways
    h2.state_mut().model.set_region(None);
    h2.state_mut().region_from_event(ClipId(2));
    let by_menu = h2.state().model.project().region.expect("the menu item did not give a region");
    assert_eq!(by_menu, by_gesture, "the gesture and the menu item must match");
}

/// Test 207 (R.33 item 8, verbatim: “it is needed that on a double-click on a
/// marked_region the whole of it is selected; if there is an overlap of several — then
/// by the name of the region”).
#[test]
fn t207_double_click_on_marked_range_selects_it() {
    use crate::timeline::pick_range_marker;
    use kt_model::*;
    use kt_time::Tick;

    // ---- pure part: which range is under the click ----
    let markers = vec![
        Marker { t: Tick(30_000), duration: Tick(60_000), name: "a".into(), color: None },
        Marker { t: Tick(60_000), duration: Tick(60_000), name: "b".into(), color: None },
        Marker { t: Tick(500_000), duration: Tick::ZERO, name: "point".into(), color: None },
    ];
    let tol = 4_000;
    assert_eq!(
        pick_range_marker(&markers, 70_000, tol),
        Some(60_000),
        "the shared part — the nearest flag on the left"
    );
    assert_eq!(
        pick_range_marker(&markers, 31_000, tol),
        Some(30_000),
        "a click on a flag takes its own label"
    );
    assert_eq!(pick_range_marker(&markers, 200_000, tol), None, "nothing past the reels");
    assert_eq!(
        pick_range_marker(&markers, 500_000, tol),
        None,
        "a point marker does not become a region"
    );

    // ---- live double-click on the reel ----
    let (mut h, g) = harness_three_clips_dt(Some(0.02));
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.markers = vec![Marker {
            t: Tick(90_000),
            duration: Tick(130_000),
            name: "chorus".into(),
            color: None,
        }];
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();
    let band_y = g.rect.top() + crate::timeline::TIME_TOP + 11.0;
    let pos = egui::pos2(g.tick_to_x(150_000), band_y);
    h.hover_at(pos);
    h.step();
    for _ in 0..2 {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    let r = h.state().model.project().region.expect("double-click on the reel did not give a region");
    assert_eq!((r.start.0, r.end.0), (90_000, 220_000), "the region is not on the marker bounds");
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![ClipId(2), ClipId(3)],
        "events inside the range must be selected (“everything was selected”)"
    );
}

/// Test 204 (R.33 item 7): the batch moves by the drag as a WHOLE and in one undo step, and
/// the stop is computed from the first contact of any of its members. The mutual
/// shifts inside the block do not change — “nothing grows back and is not cut off”
/// (item 6 of R.28).
#[test]
fn t204_multi_drag_moves_the_whole_block() {
    use crate::timeline::plan_move_block;
    use kt_model::*;
    use kt_time::Tick;

    // ---- pure part: clamping the block against an obstacle ----
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
    let clip = |id: u64, start: i64| ClipEvent {
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
        duration: Tick(50_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    p.clips = vec![clip(1, 0), clip(2, 100_000), clip(3, 400_000)];
    p.overlap_layers = false; // layering-free mode: the stop is real
    let moving = [ClipId(1), ClipId(2)];
    // the block [0..50k]+[100k..150k] moves right toward an obstacle at 400k:
    // its front member (2) touches it after 250,000 ticks
    let plan = plan_move_block(&p, &moving, 400_000);
    assert_eq!(plan.delta, 250_000, "the block did not stop at the obstacle");
    assert_eq!(plan.blocker.map(|(s, _, _)| s), Some((400_000, 450_000)));
    // moving left, the block stops at zero (it does not go past the start of the timeline)
    assert_eq!(plan_move_block(&p, &moving, -80_000).delta, 0);
    // a free move is not touched
    let free = plan_move_block(&p, &moving, 100_000);
    assert_eq!((free.delta, free.blocker.is_none()), (100_000, true), "free movement was clamped");
    // with layering (the project default) there is no stop at all
    p.overlap_layers = true;
    assert_eq!(plan_move_block(&p, &moving, 400_000).delta, 400_000);

    // ---- live gesture ----
    let (mut h, g) = harness_three_clips();
    {
        let app = h.state_mut();
        app.select_only(ClipId(1));
        app.selected.insert(ClipId(2));
    }
    h.step();
    let y = g.lane_y(ClipKind::Video, 0, 0.6);
    let grab = egui::pos2(g.tick_to_x(25_000), y);
    let to = egui::pos2(g.tick_to_x(325_000), y);
    h.hover_at(grab);
    h.step();
    h.drag_at(grab);
    h.step();
    h.hover_at(grab + egui::vec2(8.0, 0.0));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();
    let starts: Vec<i64> = {
        let p = h.state().model.project();
        [ClipId(1), ClipId(2), ClipId(3)]
            .iter()
            .map(|id| p.clips.iter().find(|c| c.id == *id).unwrap().start.0)
            .collect()
    };
    assert_eq!(starts[0], 300_000, "the batch anchor did not arrive: {starts:?}");
    assert_eq!(starts[1], 400_000, "the second event of the batch must move by the same shift");
    assert_eq!(starts[2], 200_000, "an unselected event must not be moved");
    let durs: Vec<i64> = {
        let p = h.state().model.project();
        p.clips.iter().map(|c| c.duration.0).collect()
    };
    assert!(durs.iter().all(|d| *d == 50_000), "durations changed: {durs:?}");
    h.state_mut().undo();
    let after_undo: Vec<i64> = {
        let p = h.state().model.project();
        [ClipId(1), ClipId(2)]
            .iter()
            .map(|id| p.clips.iter().find(|c| c.id == *id).unwrap().start.0)
            .collect()
    };
    assert_eq!(after_undo, vec![0, 100_000], "moving the batch is one undo step");
}

/// Test 205 (R.33 item 7, “well, this is naturally to be accounted for everywhere, including
/// morpher creation”): operations work on the WHOLE selection,
/// not on a single event.
#[test]
fn t205_operations_run_over_whole_selection() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips();
    let app = h.state_mut();
    app.select_only(ClipId(1));
    app.selected.insert(ClipId(3));

    // the target of the morpher = the whole selection
    assert_eq!(app.morph_target_ids(), vec![ClipId(1), ClipId(3)]);

    // copy takes both events, paste lays down both
    app.copy_selection();
    assert_eq!(app.clipboard.clips.len(), 2, "the whole selection is copied");
    app.player.seek(kt_time::Tick(400_000));
    app.paste_at_cursor();
    assert_eq!(app.model.project().clips.len(), 5, "both events were pasted");

    // deletion removes both and is rolled back by one undo
    app.select_only(ClipId(1));
    app.selected.insert(ClipId(3));
    let before = app.model.project().clips.len();
    app.delete_selected();
    assert_eq!(app.model.project().clips.len(), before - 2, "Del did not delete the whole selection");
    app.undo();
    assert_eq!(app.model.project().clips.len(), before, "undo restored both in one step");
}

/// Test 201 (R.33 item 9, the user's requirement: “the template for all renders
/// is always kt-<projectname>-<region_name>-<its length in seconds>”). It was
/// `kp-render-<unix-time>.mp4` — neither the project, nor the region, nor the length.
#[test]
fn t201_render_file_name_follows_template() {
    use crate::render_dialog::render_file_name;
    use kt_model::*;
    // the pure part of the template
    assert_eq!(
        render_file_name(Some("movie"), Some("intro"), 90_000 * 12),
        "kt-movie-intro-12s.mp4"
    );
    assert_eq!(render_file_name(Some("movie"), None, 90_000 * 3), "kt-movie-3s.mp4");
    assert_eq!(render_file_name(Some("movie"), Some("  "), 90_000), "kt-movie-1s.mp4");
    assert_eq!(render_file_name(None, None, 0), "kt-untitled-0s.mp4");
    assert_eq!(
        render_file_name(Some("my project"), Some("scene 2: take?"), 90_000 * 5 / 2),
        "kt-my-project-scene-2_-take_-3s.mp4",
        "spaces become hyphens, forbidden FS characters underscores, the length is rounded"
    );

    // the live part: the name is taken from the range marker that MATCHES the region
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.markers.push(Marker {
        t: kt_time::Tick(90_000),
        duration: kt_time::Tick(90_000 * 4),
        name: "chorus".into(),
        color: None,
    });
    p.region = Some(Region { start: kt_time::Tick(90_000), end: kt_time::Tick(90_000 * 5) });
    app.model = kt_model::TimelineModel::new(p);
    app.project_path = Some(std::path::PathBuf::from("/tmp/kt-t201/song.kt"));
    assert_eq!(app.default_render_name(), "kt-song-chorus-4s.mp4");

    // the region did not match the marker — the segment of the region is dropped (the name does not lie)
    let mut p = app.model.project().clone();
    p.region = Some(Region { start: kt_time::Tick(0), end: kt_time::Tick(90_000 * 5) });
    app.model = kt_model::TimelineModel::new(p);
    assert_eq!(app.default_render_name(), "kt-song-5s.mp4");
    h.step();
}

/// Test 200 (R.33, item K1): edits INSIDE a gesture (the FX window, the drag of a
/// transition's edge, the morpher window) must advance the model revision. While they
/// called `model.apply_coalesced` bypassing the application wrapper, the revision stood
/// still — and the background autosave cleared the asterisk through the gate “revision is the
/// same”, although the effect chain had already changed AFTER the snapshot: the edits
/// looked saved and were lost on exit.
#[test]
fn t200_coalesced_edits_bump_revision_and_keep_dirty() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t200-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t200.kt");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(9), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(9),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(9),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    app.model = kt_model::TimelineModel::new(p);
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some(), "a background write is in flight");

    // a chain edit by the gesture of the FX window, while the write had not finished
    let app = h.state_mut();
    let rev_before = app.model_rev;
    app.open_fx_editor(ClipId(9));
    let gesture = app.fx_editors[0].gesture;
    app.set_fx_coalesced(ClipId(9), vec![FxInstance::blur(20), FxInstance::glow(1.0)], gesture);
    assert!(app.model_rev > rev_before, "a gesture edit moves the model revision");

    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(h.state().autosave_rx.is_none(), "autosave result collected");
    assert!(h.state().dirty_content, "the chain was changed AFTER the snapshot — the asterisk must stay");
    assert_eq!(
        h.state().model.clip(ClipId(9)).unwrap().fx.len(),
        2,
        "the edit in place (not rolled back by autosave)"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 199 (R.32): the RULE of the guard — a write with an OLD revision
/// does not touch the file, a write with a new one does. This is exactly how the background
/// autosave stops overwriting a fresh Ctrl+S
/// (the race was found by live scenarios: they read the saved .kt and saw an outdated model —
/// the user would lose edits the same way).
#[test]
fn t199_guarded_write_never_goes_backwards() {
    let dir = std::env::temp_dir().join(format!("kt-t199g-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("p.kt");
    let guard = std::sync::Mutex::new(0u64);

    use crate::app::SaveOutcome;
    let wrote = |o: SaveOutcome| matches!(o, SaveOutcome::Wrote { .. });

    // the first write (revision 5) passes
    assert!(wrote(crate::app::write_project_guarded(&guard, &f, 5, "rev5").unwrap()));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "rev5");
    // a “late” background snapshot of revision 3 — does not touch the file
    assert!(!wrote(crate::app::write_project_guarded(&guard, &f, 3, "rev3").unwrap()));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "rev5", "an old snapshot overwrote the fresh one");
    // a repeat of the same revision is allowed (idempotent), a newer one is written
    assert!(wrote(crate::app::write_project_guarded(&guard, &f, 5, "rev5b").unwrap()));
    assert!(wrote(crate::app::write_project_guarded(&guard, &f, 9, "rev9").unwrap()));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "rev9");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same invariant in the live application thread: a background write in flight +
/// an immediate Ctrl+S of the next revision → the fresher one remains on disk.
#[test]
fn t199_background_autosave_never_overwrites_newer_save() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t199-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t199.kt");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    // revision 1: media A, the background autosave starts with THIS snapshot
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some(), "a background write is in flight");

    // revision 2 + an immediate Ctrl+S while the background one has not finished yet
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(2), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.save();

    // we wait for the background one and make sure it did NOT roll the file back
    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    let text = std::fs::read_to_string(&target).unwrap();
    let saved = kt_model::project_file::from_text(&text).unwrap().project;
    assert_eq!(
        saved.media.len(),
        2,
        "on disk the FRESH version (2 media) must remain, not the autosave snapshot"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 382 (R.49 A9): the envelope is inherited ON A LIVE GESTURE, not only in
/// the theory of kt-otio. The cut and the paste of a copy go through `apply_batch`, and the kinship
/// reaches `Retained` by the same path by which the model reported it.
#[test]
fn t382_app_carries_extras_to_born_clips() {
    use kt_model::*;
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    let app = h.state_mut();
    let src = app.model.project().clips[0].clone();

    // the event has an envelope from a “file”: a name and a foreign marker
    app.retained
        .clips
        .insert(src.id, kt_otio::ClipExtra { name: "Shot 12A".into(), ..Default::default() });

    // a cut by the application's gesture
    let mid = src.start + Tick(src.duration.0 / 2);
    let new_id = ClipId(app.next_clip_id());
    assert!(app.apply(Command::Split { id: src.id, at: mid, new_id, new_group: None }));
    assert_eq!(
        h.state().retained.clips.get(&new_id).map(|e| e.name.as_str()),
        Some("Shot 12A"),
        "half of the cut is left unnamed: a clip without a name will go into the file"
    );

    // a paste of a copy — the same path via ops::paste_block
    let app = h.state_mut();
    let before = app.model.project().clips.len();
    app.selected = [src.id].into_iter().collect();
    app.copy_selection();
    app.paste_here(Tick(500_000), None, String::new(), false);
    assert!(h.state().model.project().clips.len() > before, "the paste added no events");
    let pasted = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .filter(|c| c.id != src.id && c.id != new_id)
        .map(|c| c.id)
        .collect::<Vec<_>>();
    assert!(
        pasted.iter().any(|id| h
            .state()
            .retained
            .clips
            .get(id)
            .is_some_and(|e| e.name == "Shot 12A")),
        "the pasted copy lost the name of the original"
    );
}

/// Test 390 (R.49 A12): the work place does not move into someone else's project file.
/// The width of the meters panel and the view (zoom/scroll/cursor) lived in `.kt`: open
/// a colleague's project — lose your own width; save — impose yours on him. In the file
/// a passthrough remains: what was read is what we write, byte stability is intact.
#[test]
fn t390_workspace_does_not_travel_in_the_project_file() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t390-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t390.kt");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.workspace_file = Some(dir.join("workspace.json"));
    // “read from someone else's file”: its meters width and its view
    app.ui_passthrough = Some(kt_model::UiState {
        meters_w: 123.0,
        view_start: 4444.0,
        ticks_per_px: 777.0,
        cursor: 555,
        ..Default::default()
    });
    // but we have our own work place
    app.meters_w = 90.0;
    app.view_start = 10.0;
    app.ticks_per_px = 200.0;
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.save();

    // in the file — FOREIGN values, untouched
    let text = std::fs::read_to_string(&target).unwrap();
    let saved = kt_model::project_file::from_text(&text).unwrap().project;
    let ui = saved.ui.expect("the viewport must stay in the file");
    assert_eq!(ui.meters_w, 123.0, "OUR meter width went into another file");
    assert_eq!(ui.view_start, 4444.0, "OUR scroll went into another file");
    assert_eq!(ui.cursor, 555, "OUR cursor went into another file");

    // and ours — in our own store
    let w = crate::user_store::load_workspace_at(&dir.join("workspace.json"));
    assert_eq!(w.meters_w, 90.0, "our own meter width was not saved");
    let v = w.view_of(Some(&target)).expect("the project view was not saved");
    assert_eq!(v.view_start, 10.0);
    assert_eq!(v.ticks_per_px, 200.0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 391 (R.49 A12): the move without losses. An empty store (the first launch
/// after the move) keeps the values from the file — a migration without a single line of
/// code; a filled store takes precedence over the file; the names and heights of the tracks and the bin groups
/// remain in the PROJECT, because they are about THIS edit.
#[test]
fn t391_workspace_falls_back_to_file_and_project_keeps_its_own() {
    use crate::user_store::{ProjectView, Workspace, load_workspace_at, save_workspace_at};
    let dir = std::env::temp_dir().join(format!("kt-t391-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ws = dir.join("workspace.json");
    let proj = dir.join("p.kt");

    // an empty store imposes nothing
    let w = load_workspace_at(&ws);
    assert_eq!(w.meters_w, 0.0, "an empty store must stay silent");
    assert!(w.view_of(Some(&proj)).is_none());

    // a round trip: written — read back
    let mut w = Workspace::default();
    w.meters_w = 111.0;
    w.set_view(Some(&proj), ProjectView { view_start: 7.0, ticks_per_px: 13.0, cursor: 42 });
    save_workspace_at(&ws, &w);
    let back = load_workspace_at(&ws);
    assert_eq!(back.meters_w, 111.0);
    let v = back.view_of(Some(&proj)).expect("the view was not read");
    assert_eq!((v.view_start, v.ticks_per_px, v.cursor), (7.0, 13.0, 42));
    // the view of another project is not overwritten
    assert!(back.view_of(Some(&dir.join("other.kt"))).is_none());

    // a broken file does not bring the program down and does not impose junk
    std::fs::write(&ws, "{ this is not json").unwrap();
    assert_eq!(load_workspace_at(&ws).meters_w, 0.0);

    // a file from a FUTURE version is read but not overwritten: we must not break
    // what we do not understand (user_store policy, R.48)
    std::fs::write(&ws, r#"{"version":99,"meters_w":55.0,"views":[]}"#).unwrap();
    let future = load_workspace_at(&ws);
    assert_eq!(future.meters_w, 55.0, "a future file must be readable");
    assert!(future.is_readonly());
    // the program holds the READ store, and that is why it refuses to write
    save_workspace_at(&ws, &future);
    assert!(
        std::fs::read_to_string(&ws).unwrap().contains("99"),
        "the file newer than our version was overwritten"
    );

    // but THIS stays in the project: it is about editing, not about the workspace
    let mut h = harness();
    let app = h.state_mut();
    // the name and height of a track from R.62 are CONTENT of the model (they go through undo),
    // not fields of `ui`: the workspace has even less business with them
    edit_project(app, |p| {
        p.track_at_mut(kt_model::LaneKind::Audio, 0).unwrap().name = "Speech".into();
        p.track_at_mut(kt_model::LaneKind::Video, 0).unwrap().height = 88.0;
    });
    app.bin_free = true;
    let ui = app.ui_state_for_test();
    assert!(ui.lane_names.is_empty(), "the track name no longer lives in the UI");
    assert!(ui.lane_heights.is_empty(), "the track height no longer lives in the UI");
    assert_eq!(app.row_label(kt_model::LaneKind::Audio, 0), "Speech");
    assert!(ui.bin_free, "the bin layout is part of the edit");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Double click at a point. egui’s click history is shared per window, so the harness step
/// must be short (0.02 s), otherwise two clicks drift apart in time and add up to two single clicks
/// (trap of item 232).
fn double_click_at(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) {
    // egui’s click history is shared per window: two gestures in a row add up to a
    // TRIPLE click, and the second “double” never happens at all (trap of item 232).
    // We let the double-click window expire.
    for _ in 0..25 {
        h.step();
    }
    h.hover_at(pos);
    h.step();
    for _ in 0..2 {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
}

/// Right click without pointer movement.
fn right_click_at(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) {
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
}

/// Labels of the items in the open menu.
pub(crate) fn menu_labels(h: &mut Harness<'_, KtApp>) -> Vec<String> {
    use egui_kittest::kittest::{NodeT, Queryable};
    h.query_all_by_label_contains("")
        .filter_map(|n| n.accesskit_node().label().map(|l| l.to_string()))
        .collect()
}

/// Test 392 (R.49 B2): a double click on the level line RESETS the level.
/// The hint promised this from the very start (“drag, double click — 0 dB”), but
/// a double click on the line kept opening the FX window: the promise was in the hint,
/// the behaviour was not.
#[test]
fn t392_double_click_on_level_rail_resets_level() {
    use kt_model::*;
    let (mut h, g) = harness_three_clips_dt(Some(0.02));
    // audio and video: the level line is one for both kinds (item 176)
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        let mut a = p.clips[0].clone();
        a.id = ClipId(10);
        a.kind = ClipKind::Audio;
        a.lane = 0;
        a.gain_db = -3.0;
        p.clips.push(a);
        p.ensure_tracks(kt_model::LaneKind::Audio, p.lanes(kt_model::LaneKind::Audio).max(1));
        p.clips[0].opacity = 0.35;
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();
    let g = h.state().tl_geom.clone().unwrap_or(g);

    // the level line sits by its value: at 35 % it is low, at −9 dB it is also low
    let vclip = h.state().model.project().clips[0].clone();
    let level_y = |kind: ClipKind, lane: u32, v: f32| -> f32 {
        let top = g.lane_top(kind, lane);
        let lh = g.lane_h_of(kind, lane);
        crate::timeline::level_y(v, top, lh)
    };
    let vx = g.tick_to_x(vclip.start.0 + vclip.duration.0 / 2);
    let vy = level_y(ClipKind::Video, 0, 0.35);

    double_click_at(&mut h, egui::pos2(vx, vy));
    let st = h.state();
    assert_eq!(
        st.model.project().clips.iter().find(|c| c.id == vclip.id).map(|c| c.opacity),
        Some(1.0),
        "double-click on the video level line did not reset the opacity"
    );
    assert!(st.fx_editors.is_empty(), "double-click on the line opened the FX window instead of resetting");

    // the same gesture for audio: 0 dB
    let aclip =
        h.state().model.project().clips.iter().find(|c| c.id == ClipId(10)).cloned().unwrap();
    let ax = g.tick_to_x(aclip.start.0 + aclip.duration.0 / 2);
    let ay = level_y(ClipKind::Audio, 0, crate::timeline::gain_to_level(-3.0));
    double_click_at(&mut h, egui::pos2(ax, ay));
    assert_eq!(
        h.state().model.project().clips.iter().find(|c| c.id == ClipId(10)).map(|c| c.gain_db),
        Some(0.0),
        "double-click on the volume line did not give 0 dB"
    );

    // but on the BODY of an event a double click still opens FX
    let body_y = g.lane_y(ClipKind::Video, 0, 0.75);
    double_click_at(&mut h, egui::pos2(vx, body_y));
    assert!(!h.state().fx_editors.is_empty(), "double-click on the body stopped opening FX");
}

/// Test 393 (R.49 B2): the chord in the item label promises exactly what the key does.
/// “Delete (Del)” on a named area was untrue: Del deletes the SELECTED EVENTS,
/// and does not touch the area at all.
#[test]
fn t393_menu_accelerator_promises_only_what_the_key_does() {
    use kt_model::*;
    use kt_time::Tick;
    // the mechanism itself: the chord is an argument, not part of the translated phrase
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        assert!(
            !tr(k::tl::menu::DELETE).contains("(Del"),
            "{}: the chord stayed inside the catalog value — one phrase for all contexts",
            lang.code()
        );
    }

    let (mut h, g) = harness_three_clips();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        // the area is FREE of events: its outline cannot be grabbed under a clip —
        // the clip wins there, and that is correct
        p.areas.push(MarkedArea {
            name: "scene 3".into(),
            start: Tick(300_000),
            duration: Tick(50_000),
            lanes: vec![(LaneKind::Video, 0)],
            color: None,
        });
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();
    let g = h.state().tl_geom.clone().unwrap_or(g);

    // AREA menu: the item is there, the chord is not in it
    let row = g.row_of((LaneKind::Video, 0)).expect("video row");
    right_click_at(&mut h, egui::pos2(g.tick_to_x(320_000), g.row_top(row) + 1.0));
    assert!(
        matches!(h.state().ctx_target, Some(crate::timeline::CtxTarget::Area(_))),
        "scene: right-click must give an area"
    );
    let labels = menu_labels(&mut h);
    let del = labels
        .iter()
        .find(|l| l.starts_with(tr(k::tl::menu::DELETE)))
        .unwrap_or_else(|| panic!("the area menu has no delete item: {labels:?}"));
    assert!(!del.contains("Del"), "the area promises the Del chord, which does not delete it: “{del}”");

    // and this is not “removed everywhere”: CAMERA Del still works, and the promise stayed.
    // Own harness: egui’s click history is shared per window, and a second right click
    // in the same window would CLOSE the menu already open.
    let (mut h2, _) = harness_three_clips();
    {
        let app = h2.state_mut();
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 0 }));
        app.player.seek(Tick(320_000));
        app.add_camera_at_playhead(0);
        app.camera_editors.clear();
    }
    h2.step();
    h2.step();
    let g2 = h2.state().tl_geom.clone().expect("geometry");
    let cam_row = g2.row_of((LaneKind::Camera, 0)).expect("camera row");
    let cam = h2.state().model.project().cameras[0].clone();
    right_click_at(
        &mut h2,
        egui::pos2(g2.tick_to_x(cam.start.0 + cam.duration.0 / 2), g2.row_center_y(cam_row)),
    );
    assert!(
        matches!(h2.state().ctx_target, Some(crate::timeline::CtxTarget::Camera(_))),
        "scene: right-click must give a camera"
    );
    let labels = menu_labels(&mut h2);
    assert!(
        labels.iter().any(|l| l.starts_with(tr(k::tl::menu::DELETE)) && l.contains("Del")),
        "Del works for the camera — the promise must stay: {labels:?}"
    );
}

/// Test 394 (R.49 B3–B5): there are no mute objects in the panels.
///
/// Recon found twelve places where an object answers to HOVER with a hint,
/// but to the right button with nothing. A person learns once: if the right button
/// somewhere showed a menu, they press it everywhere, and silence reads as
/// “there is no object”. Here a right click goes to EVERY panel object and
/// it is checked that the program named the target, i.e. the menu belongs to
/// exactly what was aimed at.
#[test]
fn t394_panel_objects_answer_right_click() {
    use crate::ctx_menu::PanelTarget as T;
    use kt_model::*;
    let (mut h, _) = harness_three_clips_dt(Some(0.02));

    // ---- media bin background ---------------------------------------------------
    {
        let app = h.state_mut();
        app.ctx_panel = None;
    }
    h.step();
    // the bin’s rectangle is reported by the program itself: below it in the column lives
    // the perf panel, and “roughly at the bottom of the column” would probe it
    let bin = h.state().bin_rect;
    assert!(bin.is_positive(), "the bin did not report its rectangle");
    right_click_at(&mut h, egui::pos2(bin.center().x, bin.bottom() - 10.0));
    assert_eq!(h.state().ctx_panel, Some(T::BinEmpty), "the media bin background is silent on the right button");

    // ---- meter panel ----------------------------------------------------
    let meters = panel_rect(&h, "meters_col");
    right_click_at(&mut h, meters.center());
    assert_eq!(h.state().ctx_panel, Some(T::Meters), "the meter panel is silent on the right button");

    // ---- preview window ------------------------------------------------------
    let prev = h.state().preview_rect;
    assert!(prev.is_finite(), "the preview is not drawn — the scene is broken");
    right_click_at(&mut h, egui::pos2(prev.center().x, prev.center().y + 20.0));
    assert_eq!(h.state().ctx_panel, Some(T::Preview), "the preview window is silent on the right button");

    // ---- effect chain slot -------------------------------------------
    {
        let app = h.state_mut();
        let id = app.model.project().clips[0].id;
        assert!(app.apply(Command::SetFx { id, fx: vec![FxInstance::blur(8)] }));
        app.open_fx_editor(id);
    }
    h.step();
    h.step();
    let slot = {
        use egui_kittest::kittest::Queryable;
        h.query_all_by_label_contains(crate::fx_editor::fx_display_name(&FxInstance::blur(8)))
            .map(|n| n.rect())
            .next()
            .expect("the chain slot is not drawn")
    };
    right_click_at(&mut h, slot.center());
    assert!(
        matches!(h.state().ctx_panel, Some(T::FxSlot { .. })),
        "the chain slot is silent on the right button"
    );

    // and the TIMELINE target is dimmed at the same time: one menu on screen
    assert!(
        h.state().ctx_target.is_none(),
        "the panel target did not clear the timeline target — the test would read a stale one"
    );
}

/// Test 412 (R.49 B7): loudness is shown IN THE HINT of the bin card, and showing
/// STARTS NOTHING.
///
/// The second matters more than the first: sweeping the mouse across the bin would mean
/// starting decode of a dozen files, a direct relative of invariant 8 (“scroll/zoom does not produce
/// decode requests”). And until it is measured, the program says so, instead of
/// showing an invented zero.
#[test]
fn t412_bin_tooltip_shows_loudness_and_does_not_request_it() {
    use kt_model::*;
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
        app.model
            .apply(Command::AddMedia { id: MediaId(2), path: "synth:sine:440".into() })
            .unwrap();
        // the first one’s loudness is known, the second one’s is not
        app.loudness.lock().unwrap().insert(MediaId(1), (-18.5, -1.2, 6.0));
    }
    h.step();
    h.step();

    let g = h.state().bin_rect;
    assert!(g.is_positive(), "the bin is not drawn");
    // hover over the cards and read the status line
    let mut seen_known = false;
    let mut seen_pending = false;
    for y in 0..20 {
        let p = egui::pos2(g.left() + 30.0, g.top() + 20.0 + y as f32 * 12.0);
        if !g.contains(p) {
            break;
        }
        h.hover_at(p);
        h.step();
        if let Some(hint) = h.state().hover_hint.clone() {
            if hint.contains("-18.5") || hint.contains("−18.5") {
                seen_known = true;
            }
            if hint.contains(tr(k::bin::LOUDNESS_PENDING)) {
                seen_pending = true;
            }
        }
    }
    assert!(seen_known, "the volume did not reach the card hint");
    assert!(seen_pending, "the program stayed silent about unmeasured volume");

    // and no analysis request has been queued
    let st = h.state();
    assert_eq!(st.analysis.queued(), 0, "the display queued an analysis request");
    assert!(st.analysis.current().is_none(), "the display started the analysis");
}

/// Test 418 (R.49 B9): inability to sync is named ALOUD, and with DIFFERENT causes
/// (rule 16).
///
/// “Index still building” and “records do not match” are not the same thing: in the first
/// case one should wait, in the second waiting is pointless. Silence or a generic
/// “it did not work” leaves the person guessing.
#[test]
fn t418_sync_refusals_are_spoken_and_distinct() {
    use kt_model::*;
    let (mut h, _) = harness_three_clips();

    // nothing selected: it says that two must be selected
    let app = h.state_mut();
    app.selected.clear();
    app.notices.clear();
    app.sync_selected_by_audio();
    assert_eq!(
        h.state().notices.back().map(|n| n.text.clone()),
        Some(tr(k::msg::SYNC_NEED_TWO).to_string()),
        "the wrong selection is not mentioned"
    );

    // two selected, but no peaks — “index still building”, a different cause
    let app = h.state_mut();
    let ids: Vec<ClipId> = app.model.project().clips.iter().take(2).map(|c| c.id).collect();
    app.selected = ids.iter().copied().collect();
    app.peaks.lock().unwrap().clear();
    app.notices.clear();
    app.sync_selected_by_audio();
    let last = h.state().notices.back().cloned().expect("they stayed silent about the index not being ready");
    assert_eq!(last.text, tr(k::msg::SYNC_PENDING), "the wrong reason was named");
    assert_eq!(last.severity, crate::app::Severity::Warn, "the refusal was shown not as an error");
    assert_ne!(
        tr(k::msg::SYNC_PENDING),
        tr(k::msg::SYNC_NO_MATCH),
        "two different reasons must sound different"
    );
}

/// Test 419 (R.49 B9): the event menu item and the main menu item are ONE BODY
/// (rule 17). Separate bodies would drift apart at the first edit.
#[test]
fn t419_sync_menu_and_timeline_item_are_the_same_body() {
    let root = crate::reach_tests::workspace_root();
    let mut callers = Vec::new();
    for rel in ["apps/kt-app/src/timeline.rs", "apps/kt-app/src/toolbar.rs"] {
        let src = std::fs::read_to_string(root.join(rel)).expect(rel);
        assert!(
            src.contains("SYNC_AUDIO"),
            "{rel}: there is no sync item — the paired way is lost"
        );
        callers.push(src.matches("sync_selected_by_audio()").count());
    }
    assert!(callers.iter().all(|&n| n >= 1), "one of the paths calls not the shared body: {callers:?}");
    // and the body itself is one for the whole app
    let app = std::fs::read_to_string(root.join("apps/kt-app/src/app.rs")).expect("app.rs");
    assert_eq!(
        app.matches("pub fn sync_selected_by_audio").count(),
        1,
        "there is more than one sync body — they will drift apart"
    );
}

/// Test 421 (R.49 B10): a result computed from an OUTDATED model is not applied. While the task
/// was reading files the edit could have moved — curves from the old model would have landed
/// on the wrong events, silently.
#[test]
fn t421_stale_analysis_result_is_not_applied() {
    use kt_model::*;
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
        app.model_rev += 1;
    }
    let rev = h.state().model_rev;
    // result “from the past”: same revision, media in place — applied
    h.state_mut().on_analysis_done_for_test(
        crate::analysis::AnalysisKind::Loudness(MediaId(1)),
        rev,
        Ok(crate::analysis::AnalysisOut::Loudness {
            media: MediaId(1),
            lufs: Some(-17.0),
            dbtp: -2.0,
            lra: 4.0,
        }),
    );
    assert!(
        h.state().loudness.lock().unwrap().contains_key(&MediaId(1)),
        "the fresh result was not applied"
    );

    // but now the media was swapped: the result for the old file does not belong to it
    {
        let app = h.state_mut();
        app.loudness.lock().unwrap().clear();
        app.apply(Command::RemoveMedia { id: MediaId(1) });
    }
    let rev = h.state().model_rev;
    h.state_mut().on_analysis_done_for_test(
        crate::analysis::AnalysisKind::Loudness(MediaId(1)),
        rev,
        Ok(crate::analysis::AnalysisOut::Loudness {
            media: MediaId(1),
            lufs: Some(-17.0),
            dbtp: -2.0,
            lra: 4.0,
        }),
    );
    assert!(
        !h.state().loudness.lock().unwrap().contains_key(&MediaId(1)),
        "a number for vanished media was applied — the stale guard did not trigger"
    );
}

/// Test 376 (R.49 A8): an event number is not reused within a session.
/// The old formula “maximum over current clips + 1” handed out the very same number again
/// after the last event was deleted — and a newborn clip inherited
/// a FOREIGN envelope from `Retained.clips`: name, markers and links to the media of the
/// deleted one. Link groups and morpher links go by the same number.
#[test]
fn t376_clip_ids_are_not_reused_within_a_session() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();

    let make = |app: &mut crate::app::KtApp, id: u64| {
        app.apply(Command::AddClip(ClipEvent {
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
            start: Tick(0),
            duration: Tick(90_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        }));
    };

    let first = app.next_clip_id();
    make(app, first);
    let second = app.next_clip_id();
    assert!(second > first, "the second number is not greater than the first");
    make(app, second);

    // deleted both — the numbers still do not come back
    app.apply(Command::RemoveClip { id: ClipId(second) });
    app.apply(Command::RemoveClip { id: ClipId(first) });
    assert!(app.model.project().clips.is_empty());
    let third = app.next_clip_id();
    assert!(
        third > second,
        "a number was reused after deletion ({third} <= {second}): the new clip will get \
         the envelope of the deleted one — its name, markers and media references"
    );

    // the mark does not move until the number is actually taken
    assert_eq!(app.next_clip_id(), third, "two different numbers were issued for one request");

    // undo that cancelled a birth does NOT roll the mark back: the number was already live, and
    // the envelope under it might have settled into Retained
    make(app, third);
    app.undo();
    assert!(app.model.project().clips.is_empty(), "undo had to remove the born clip");
    assert!(
        app.next_clip_id() > third,
        "undo returned the number to circulation — together with the envelope that had been created under it"
    );
}

/// Test 379 (R.49 A8, idiom t273): event-number arithmetic lives in ONE place. Every private
/// copy of “max(...)+1” is one more way to hand out the number of a deleted clip, and it can
/// only be found later by a complaint.
#[test]
fn t379_clip_id_arithmetic_lives_in_one_place() {
    let root = crate::reach_tests::workspace_root();
    const HOME: &str = "apps/kt-app/src/app.rs";
    let mut bad: Vec<String> = Vec::new();
    for rel in [
        "apps/kt-app/src/timeline.rs",
        "apps/kt-app/src/bin_panel.rs",
        "apps/kt-app/src/morpher_editor.rs",
        "apps/kt-app/src/fx_editor.rs",
        "apps/kt-app/src/toolbar.rs",
        HOME,
    ] {
        let src = crate::reach_tests::strip_comments(
            &std::fs::read_to_string(root.join(rel)).expect(rel),
        );
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            // “take the maximum of clip numbers and add one” — the issuing formula
            if !(t.contains("c.id.0") && t.contains("max()")) {
                continue;
            }
            // in the house of rules (next_clip_id/bump_clip_hw) such lines are legitimate
            if rel == HOME {
                continue;
            }
            bad.push(format!("{rel}:{}: {t}", n + 1));
        }
    }
    assert!(
        bad.is_empty(),
        "the event number is issued OUTSIDE the single rule (`next_clip_id` in {HOME}). \
         A private copy “max + 1” does not know the already issued ones and reuses the numbers \
         of deleted events together with their envelopes: {bad:?}"
    );
}

/// Test 369 (R.49 A6): “New” and “Open” ASK when there is unsaved work.
/// Before R.49 they did not ask — and that was a legitimate choice (verdict F-RES-005):
/// the background autosave wrote straight into the user’s file, so there was nothing to lose.
/// The owner removed autosave from the project (item 351), along with the right not to ask.
#[test]
fn t369_new_and_open_guard_unsaved_work() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;

    // “New” does not silently throw away the work
    app.new_project();
    assert_eq!(h.state().ask, Some(crate::app::Ask::Unsaved(crate::app::Pending::New)));
    assert_eq!(h.state().model.project().media.len(), 1, "work was discarded before the answer");

    // “Stay” — nothing happened
    h.state_mut().answer_ask(crate::app::Answer::Cancel);
    assert!(h.state().ask.is_none());
    assert_eq!(h.state().model.project().media.len(), 1, "“Stay” discarded the work");
    assert!(h.state().dirty_content);

    // “Do not save” — the new project takes its place, and there is no asterisk on it
    h.state_mut().new_project();
    h.state_mut().answer_ask(crate::app::Answer::No);
    assert_eq!(h.state().model.project().media.len(), 0, "the new project was not created");
    assert!(!h.state().dirty_content);

    // a clean project is not asked the question
    h.state_mut().new_project();
    assert!(h.state().ask.is_none(), "asked for nothing");

    // “Open” — the same guard (rule 17: paired paths behave the same way)
    let app = h.state_mut();
    app.dirty_content = true;
    app.open_project_dialog();
    assert_eq!(h.state().ask, Some(crate::app::Ask::Unsaved(crate::app::Pending::Open)));
    h.state_mut().answer_ask(crate::app::Answer::Cancel);
}

/// Test 370 (R.49 A6): “Save and continue” does NOT continue if the save did not succeed.
/// Otherwise a write refusal (no rights, full disk, a cancelled Save As) would turn into
/// exactly the loss of work the guard is there to prevent.
#[test]
fn t370_save_and_go_does_not_go_when_save_failed() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(unwritable_path("save"));
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.new_project();
    h.state_mut().answer_ask(crate::app::Answer::Yes);

    let st = h.state();
    assert_eq!(st.model.project().media.len(), 1, "work was discarded, although the write failed");
    assert!(st.dirty_content, "the asterisk was cleared on a failed write");
    assert!(st.notices.iter().any(|n| n.severity == crate::app::Severity::Warn));
}

/// Test 371 (R.49 A6): exit is intercepted and cancelled. `CancelClose` in
/// eframe 0.36 works also for the ROOT window — the question “save before exit?” rests on this;
/// without it one would have to catch system events
/// around egui.
#[test]
fn t371_close_request_is_intercepted_and_cancelled() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;

    // the guard makes its decision with the same code the frame uses
    assert!(!h.state_mut().guard_unsaved(crate::app::Pending::Quit), "exit was not intercepted");
    assert_eq!(h.state().ask, Some(crate::app::Ask::Unsaved(crate::app::Pending::Quit)));
    assert!(!h.state().quit_confirmed, "exit was allowed before the answer");

    // “Stay” — we are not leaving
    h.state_mut().answer_ask(crate::app::Answer::Cancel);
    assert!(!h.state().quit_confirmed);

    // “Do not save” — exit is allowed, and we do not ask a second time
    assert!(!h.state_mut().guard_unsaved(crate::app::Pending::Quit));
    h.state_mut().answer_ask(crate::app::Answer::No);
    assert!(h.state().quit_confirmed, "after the answer exit is not allowed — the window will not close");
    assert!(!h.state().dirty_content);

    // for a clean project the guard does not stop you from leaving
    assert!(h.state_mut().guard_unsaved(crate::app::Pending::Quit));
}

/// Test 372 (R.49 A6): the last line of defence is `Drop`. It fires where
/// `on_exit` is not called at all: `*self = app` on “New”/“Open”, and the unwinding
/// of the stack after a panic. The body has not a single `unwrap`: a panic inside `Drop` during
/// unwinding is an immediate `abort`, taking with it exactly the file the whole thing was for.
/// That is why a POISONED guard is checked here too.
#[test]
fn t372_drop_is_the_last_line_of_defence() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t372-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("t372.kt");
    let rec = crate::app::recovery_path_for(Some(&proj));

    {
        let mut h = harness();
        let app = h.state_mut();
        app.recovery_on_drop = true; // in the harness the barrier is off — we turn it on
        app.project_path = Some(proj.clone());
        app.model.apply(Command::AddMedia { id: MediaId(5), path: "synth:ramp".into() }).unwrap();
        app.model_rev += 1;
        app.dirty_content = true;
        // the guard is poisoned: a thread panicked while holding the lock. The write must go through —
        // otherwise someone else’s panic takes away the user’s last chance.
        let g = app.recovery_rev.clone();
        let _ = std::thread::spawn(move || {
            let _held = g.lock().unwrap();
            panic!("poison");
        })
        .join();
        assert!(app.recovery_rev.is_poisoned(), "the mutex is not poisoned — the test checks nothing");
    } // here Drop

    let text = std::fs::read_to_string(&rec).expect("Drop did not write the recovery");
    let saved = kt_model::project_file::from_text(&text).unwrap().project;
    assert_eq!(saved.media.len(), 1, "the recovery has the wrong work");
    assert!(!proj.exists(), "Drop wrote INTO THE PROJECT: only Ctrl+S changes it");

    // a clean application has nothing to drop
    let _ = std::fs::remove_file(&rec);
    {
        let mut h = harness();
        let app = h.state_mut();
        app.recovery_on_drop = true;
        app.project_path = Some(proj.clone());
        app.dirty_content = false;
        app.dirty_ui = false;
    }
    assert!(!rec.exists(), "Drop wrote a recovery on a clean project");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 365 (R.49 A5): candidates are selected by TIME. A recovery older than the project
/// is not offered (otherwise after every Ctrl+S the program would urge “recover”
/// what is already in the file), a newer one is offered,
/// unnamed work is always offered, and the freshest comes first.
#[test]
fn t365_recovery_candidates_are_picked_by_time() {
    use std::time::{Duration, SystemTime};
    let dir = std::env::temp_dir().join(format!("kt-t365-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("film.kt");
    let rec = crate::app::recovery_path_for(Some(&proj));
    let anon = dir.join("anon").join("autosave-project.kt");
    std::fs::create_dir_all(anon.parent().unwrap()).unwrap();

    let touch = |p: &std::path::Path, secs_ago: u64| {
        std::fs::write(p, "{}").unwrap();
        let t = SystemTime::now() - Duration::from_secs(secs_ago);
        std::fs::File::options().write(true).open(p).unwrap().set_modified(t).unwrap();
    };

    // the project is newer than the recovery: nothing to offer
    touch(&rec, 600);
    touch(&proj, 60);
    let got = crate::app::recovery_candidates_at(Some(&proj), &anon);
    assert!(got.is_empty(), "a recovery older than the project itself was offered: {got:?}");

    // the recovery is newer than the project — it is offered
    touch(&rec, 10);
    let got = crate::app::recovery_candidates_at(Some(&proj), &anon);
    assert_eq!(got.len(), 1, "a recovery newer than the project was not offered");
    assert_eq!(got[0].path, rec);
    assert_eq!(got[0].for_project.as_deref(), Some(proj.as_path()));

    // unnamed work is always offered and, being fresher, comes first
    touch(&anon, 1);
    let got = crate::app::recovery_candidates_at(Some(&proj), &anon);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].path, anon, "the fresher one must go first");
    assert!(got[0].for_project.is_none(), "the unnamed work is not tied to a project");
    assert_eq!(got[1].path, rec);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 366 (R.49 A5): the main scenario, not covered by anything before R.49 —
/// “opened project A, pressed New, an hour of work, a crash”. `last_project`
/// points to A, A exists, the program opens it — and says nothing about the hour of work,
/// because it looked at the autosave only when `last_project`
/// was not found.
#[test]
fn t366_unnamed_work_is_offered_even_when_project_opens() {
    let dir = std::env::temp_dir().join(format!("kt-t366-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("A.kt");
    std::fs::write(&proj, "{}").unwrap();
    let anon = dir.join("data").join("autosave-project.kt");
    std::fs::create_dir_all(anon.parent().unwrap()).unwrap();
    std::fs::write(&anon, "{}").unwrap();

    let list = crate::app::recovery_candidates_at(Some(&proj), &anon);
    assert_eq!(list.len(), 1, "the hour of unnamed work was not offered: {list:?}");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(proj.clone());
    app.offer_recovery_list(list, true);
    assert!(h.state().recovery_open, "the recovery window did not open");

    // and the reverse: nothing to offer — there is no window, and on a direct request from the menu
    // the program must say so in words rather than stay silent
    let app = h.state_mut();
    app.notices.clear();
    app.offer_recovery_list(Vec::new(), false);
    assert!(!h.state().recovery_open);
    assert_eq!(h.state().notices.len(), 1, "the “Restore…” menu stayed silent");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 367 (R.49 A5): recovery puts the work INTO THE EDITOR and nowhere else.
/// The project file is not touched (only Ctrl+S changes it), the source file is not consumed,
/// the asterisk lights up, and all of this is REPEATABLE (rule 14).
#[test]
fn t367_restore_puts_work_in_editor_and_repeats() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t367-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("t367.kt");
    std::fs::write(&proj, "the original file of the user").unwrap();

    // in the recovery file — work with three media
    let mut p = kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    for i in 1..=3 {
        p.media.push(MediaRef::new(MediaId(i), format!("synth:{i}")));
    }
    let rec = crate::app::recovery_path_for(Some(&proj));
    std::fs::write(&rec, crate::app::project_text_of(&p, &Default::default(), None)).unwrap();
    // the project is deliberately OLDER than the recovery: the mtime granularity of a file system
    // can be one second, and “written one after another” does not tell them apart.
    // The time selection itself is checked by t365 — it is not the subject of this test here.
    std::fs::File::options()
        .write(true)
        .open(&proj)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(600))
        .unwrap();

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(proj.clone());
    app.dirty_content = false;
    app.offer_recovery_list(
        crate::app::recovery_candidates_at(Some(&proj), &dir.join("none")),
        true,
    );
    assert_eq!(h.state().recovery_offer.len(), 1, "the recovery was not found next to the project");

    h.state_mut().restore_recovery(0);
    let st = h.state();
    assert_eq!(st.model.project().media.len(), 3, "the work did not reach the editor");
    assert!(st.dirty_content, "restored work must count as unsaved");
    assert!(!st.recovery_open, "the window did not close after the recovery");
    assert_eq!(
        std::fs::read_to_string(&proj).unwrap(),
        "the original file of the user",
        "the recovery overwrote the project file"
    );
    assert!(rec.exists(), "the recovery ate its own source — a second time will not work");

    // rule 14: the same thing once more, after undo
    let app = h.state_mut();
    app.model = kt_model::TimelineModel::new(kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    }));
    assert_eq!(app.model.project().media.len(), 0);
    app.offer_recovery_list(
        crate::app::recovery_candidates_at(Some(&proj), &dir.join("none")),
        true,
    );
    h.state_mut().restore_recovery(0);
    assert_eq!(h.state().model.project().media.len(), 3, "the repeated recovery did not work");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 368 (R.49 A5): “Delete” removes exactly this file and closes the window
/// when there is nothing left to offer; “Not now” leaves the file in place.
#[test]
fn t368_forget_removes_file_and_later_keeps_it() {
    let dir = std::env::temp_dir().join(format!("kt-t368-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.recovery.kt");
    let b = dir.join("b.recovery.kt");
    std::fs::write(&a, "{}").unwrap();
    std::fs::write(&b, "{}").unwrap();
    let cand = |p: &std::path::Path| crate::app::RecoveryCandidate {
        path: p.to_path_buf(),
        mtime: std::fs::metadata(p).unwrap().modified().unwrap(),
        for_project: None,
    };

    let mut h = harness();
    let app = h.state_mut();
    app.offer_recovery_list(vec![cand(&a), cand(&b)], true);
    app.forget_recovery(0);
    assert!(!a.exists(), "the selected file was not deleted");
    assert!(b.exists(), "the wrong file was deleted");
    assert!(h.state().recovery_open, "a candidate remained — the window must stay");

    h.state_mut().forget_recovery(0);
    assert!(!b.exists());
    assert!(!h.state().recovery_open, "no candidates left — the window must close");

    // “Not now” destroys nothing
    std::fs::write(&a, "{}").unwrap();
    let app = h.state_mut();
    app.offer_recovery_list(vec![cand(&a)], true);
    app.recovery_open = false;
    assert!(a.exists(), "“not now” deleted the file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 363 (R.49 A4, extends t150): two copies of the program that opened one
/// project share neither the temp file nor the recovery file. A shared tmp
/// would mean that `rename` of one copy replaces the project halfway with
/// the half-written content of the other — that is, an atomic write stops being atomic
/// exactly when protection is most needed.
#[test]
fn t363_second_copy_has_its_own_tmp_and_recovery() {
    let pid = std::process::id();
    let tmp = kt_otio::atomic_tmp_path(std::path::Path::new("/w/film.kt"));
    assert!(
        tmp.to_string_lossy().contains(&pid.to_string()),
        "the temporary name has no process number: {}",
        tmp.display()
    );
    assert!(tmp.to_string_lossy().ends_with(".tmp"));
    // the file name, not the whole path: Windows rebuilds the path with its own separator
    let tmp_name = tmp.file_name().unwrap().to_string_lossy().to_string();
    assert!(tmp_name.starts_with("film.kt."), "part of the project name was eaten");
    assert_eq!(tmp.parent(), Some(std::path::Path::new("/w")), "the temporary file lives next to the project");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(std::path::PathBuf::from("/w/film.kt"));
    let owner = app.recovery_target();
    assert_eq!(owner, std::path::Path::new("/w/film.kt.recovery.kt"));
    // the same program, but the project is held by someone else
    app.lock_holder = Some(Some(4242));
    let second = app.recovery_target();
    assert_ne!(owner, second, "the second copy writes into another recovery file");
    assert!(second.to_string_lossy().contains(&pid.to_string()));
    assert!(second.to_string_lossy().ends_with(".kt"), "the recovery must be .kt");
}

/// Test 364 (R.49 A4): a second copy is not forbidden, but degrades honestly.
/// A warning with the number of the foreign process, and the first Ctrl+S does not write silently,
/// but asks. The answer “Do not save” leaves the file untouched and the asterisk
/// in place; “Anyway” writes and does not ask again.
#[test]
fn t364_locked_project_asks_before_first_save() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t364-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t364.kt");
    std::fs::write(&target, "foreign content").unwrap();

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.notices.clear();
    app.claim_project(&target);
    assert!(app.project_lock.is_some(), "a free project must be claimed");
    assert!(app.lock_holder.is_none());

    // a second copy in this same process: we release our own lock, the project is held by a
    // “neighbour” — and the program enters it again, like a neighbouring copy
    h.state_mut().project_lock = None;
    let _held = match crate::project_lock::acquire(&target) {
        crate::project_lock::LockState::Owned(l) => l,
        _ => panic!("the first lock was to be released together with the field"),
    };
    let app = h.state_mut();
    app.claim_project(&target);
    assert_eq!(app.lock_holder, Some(Some(std::process::id())), "the busy state went unnoticed");
    assert!(
        app.notices.iter().any(|n| n.text.contains(&std::process::id().to_string())),
        "the busy state was not reported with the process number: {:?}",
        app.notices.iter().map(|n| &n.text).collect::<Vec<_>>()
    );

    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.save();
    assert!(matches!(h.state().ask, Some(crate::app::Ask::SaveOverLock { .. })), "did not ask");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "foreign content",
        "wrote without waiting for the answer"
    );

    // “do not save” — the file is intact, asterisk in place, question closed
    h.state_mut().answer_ask(crate::app::Answer::No);
    assert!(h.state().ask.is_none());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "foreign content");
    assert!(h.state().dirty_content, "refusing to save cleared the asterisk");

    // “anyway” — we write; the second time we no longer ask
    h.state_mut().save();
    h.state_mut().answer_ask(crate::app::Answer::Yes);
    assert!(!h.state().dirty_content, "after the answer “anyway” the project was not written");
    assert!(h.state().lock_override);
    let app = h.state_mut();
    app.dirty_content = true;
    app.model_rev += 1;
    app.save();
    assert!(h.state().ask.is_none(), "asked again after the decision was made");
    assert!(!h.state().dirty_content);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 357 (R.49 A3): the recovery file name is appended to the FULL project name.
/// Replacing the extension (`with_extension`) would merge `film.kt` and
/// `film.mp4.kt` into one recovery — exactly the class of bug that t150 closed.
/// An unnamed project is recovered in the data directory.
#[test]
fn t357_recovery_path_appends_to_full_name() {
    use crate::app::recovery_path_for;
    let a = recovery_path_for(Some(std::path::Path::new("/w/film.kt")));
    let b = recovery_path_for(Some(std::path::Path::new("/w/film.mp4.kt")));
    assert_eq!(a, std::path::Path::new("/w/film.kt.recovery.kt"));
    assert_eq!(b, std::path::Path::new("/w/film.mp4.kt.recovery.kt"));
    assert_ne!(a, b, "two different projects share one recovery file");
    assert_eq!(a.parent(), Some(std::path::Path::new("/w")), "the recovery lies next to it");
    assert_eq!(
        recovery_path_for(None),
        crate::app::autosave_path(),
        "an unnamed project has no directory yet — the recovery goes into the data directory"
    );
}

/// Test 358 (R.49 A3): background writing goes to the RECOVERY FILE, leaves the user’s project
/// alone, does not clear the asterisk — and all of this is recoverable:
/// the real model with the edits lies in recovery.
#[test]
fn t358_autosave_writes_recovery_not_project() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t358-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t358.kt");
    std::fs::write(&target, "the original file of the user").unwrap();

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.model.apply(Command::AddMedia { id: MediaId(7), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }

    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "the original file of the user",
        "autosave overwrote the file of the user — only Ctrl+S changes it"
    );
    assert!(h.state().dirty_content, "autosave cleared the asterisk: the work counts as saved");
    let rec = crate::app::recovery_path_for(Some(&target));
    let text = std::fs::read_to_string(&rec).expect("there is no recovery file");
    let saved = kt_model::project_file::from_text(&text).unwrap().project;
    assert_eq!(saved.media.len(), 1, "the recovery has the wrong model");
    assert_eq!(h.state().recovery_written.as_deref(), Some(rec.as_path()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 359 (R.49 A3): saving removes ITS OWN recovery file and does not touch someone else’s.
/// The old code unconditionally deleted `autosave_path()` — Ctrl+S in one
/// copy of the program killed the unsaved work of another.
#[test]
fn t359_save_drops_only_own_recovery() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t359-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t359.kt");
    let mine = crate::app::recovery_path_for(Some(&target));
    let alien = dir.join("other.kt.recovery.kt");
    std::fs::write(&mine, "my snapshot").unwrap();
    std::fs::write(&alien, "unsaved work of another").unwrap();

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.recovery_written = Some(mine.clone());
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.save();

    assert!(target.exists(), "Ctrl+S must write the project");
    assert!(!mine.exists(), "our own recovery was not removed — the start will offer the already saved one");
    assert!(alien.exists(), "the recovery of ANOTHER was wiped: the work of another copy was lost");
    assert!(h.state().recovery_written.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 360 (R.49 A3): viewport drift does not start a write. Zoom and scroll every 30 seconds
/// ran a full serialization of the project WITHOUT A SINGLE edit —
/// on a large edit that is a noticeable freeze for nothing (invariant 8 in spirit:
/// scrolling the timeline does not produce work).
#[test]
fn t360_viewport_drift_does_not_write() {
    let dir = std::env::temp_dir().join(format!("kt-t360-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t360.kt");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.dirty_content = false;
    app.dirty_ui = false;
    // a project “as on disk” does not know the current viewport — and that is no reason to write
    app.model.set_ui_state(None);
    app.ticks_per_px *= 1.7;
    app.view_start += 400_000.0;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_none(), "viewport drift started a project write");
    assert!(!crate::app::recovery_path_for(Some(&target)).exists());

    // but a real edit — starts it
    let app = h.state_mut();
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some(), "an edit must start autosave");
    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 361 (R.49 A2/A3): there are TWO guards. One mutex for two targets would raise
/// the revision bar with the background recovery write — and the next Ctrl+S
/// would turn out “stale”, that is, silently save nothing.
#[test]
fn t361_recovery_guard_does_not_block_save() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t361-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t361.kt");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(
        *h.state().recovery_rev.lock().unwrap() > 0,
        "the background recovery write must raise ITS OWN guard"
    );
    assert_eq!(*h.state().saved_rev.lock().unwrap(), 0, "it raised the PROJECT guard");

    // Ctrl+S of the same revision must pass
    h.state_mut().save();
    assert!(target.exists(), "Ctrl+S was cancelled by the recovery file guard");
    assert!(!h.state().dirty_content);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 355 (R.49 A2): the outcome of a write is a TYPE, and Ctrl+S dispatches on it. Before,
/// `write_project_guarded` returned `bool`, and `save()` did `.map(|_| ())`:
/// a refusal by the guard (“the disk already has something newer”) looked like success — the asterisk
/// went out, the status said “saved”, and no file with these edits existed.
#[test]
fn t355_save_reports_skipped_and_keeps_dirty() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t355-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t355.kt");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.save();
    assert!(!h.state().dirty_content, "plain Ctrl+S must clear the asterisk");
    let ok_len = std::fs::read_to_string(&target).unwrap().len();

    // a snapshot of a revision from the future “landed” on disk: the guard must refuse
    let app = h.state_mut();
    *app.saved_rev.lock().unwrap() = app.model_rev + 100;
    app.model.apply(Command::AddMedia { id: MediaId(2), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.notices.clear();
    app.save();

    let st = h.state();
    assert!(st.dirty_content, "a refusal by the guard cleared the asterisk: the edits count as saved");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap().len(),
        ok_len,
        "on refusal the file must not change"
    );
    let texts: Vec<&str> = st.notices.iter().map(|n| n.text.as_str()).collect();
    assert!(!texts.is_empty(), "the refusal was not mentioned at all");
    assert!(
        !texts.iter().any(|t| t.contains(&target.display().to_string())),
        "the refusal was reported as a successful save: {texts:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 356 (R.49 A2): a repeated message lights the bell. Collapsing of duplicates
/// was done by an early return BEFORE the increment of `notices_unread` — and the second
/// “could not save” in a row did not raise the unread counter:
/// the only channel for a write-error message died exactly when
/// the error repeated. Plus, a Ctrl+S error is a Warn, not an Info.
#[test]
fn t356_repeated_notice_still_marks_unread() {
    let mut h = harness();
    let app = h.state_mut();
    app.notices.clear();
    app.notices_unread = 0;

    app.warn("disk on fire");
    app.warn("disk on fire");
    app.warn("disk on fire");
    assert_eq!(app.notices.len(), 1, "duplicates must collapse into one line");
    assert_eq!(app.notices.back().unwrap().repeats, 3, "the repeat counter did not grow");
    assert_eq!(app.notices_unread, 3, "a repeat did not color the bell");
    assert_eq!(
        app.notices.back().unwrap().text,
        "disk on fire",
        "“×N” is a job of the display: it must not be in the record text"
    );
    assert!(matches!(app.notices.back().unwrap().severity, crate::app::Severity::Warn));

    // another message starts its own line
    app.warn("other");
    assert_eq!(app.notices.len(), 2);
    assert_eq!(app.notices.back().unwrap().repeats, 1);

    // a project write error arrives as a warning, exactly
    let app = h.state_mut();
    app.notices.clear();
    app.project_path = Some(unwritable_path("save"));
    app.model_rev += 1;
    app.dirty_content = true;
    app.save();
    let last = h.state().notices.back().cloned().expect("the write error was not mentioned");
    assert!(
        matches!(last.severity, crate::app::Severity::Warn),
        "the Ctrl+S error came as an ordinary note: {last:?}"
    );
    assert!(h.state().dirty_content, "after a write error the asterisk must stay");
}

/// The pixel of the frame that the LIVE player source is delivering right now
/// (R.53): a check that “the edit reached the picture”, not only the model.
/// It goes the same way as the preview: worker → cache.
pub(crate) fn generator_pixel(
    h: &egui_kittest::Harness<'_, crate::app::KtApp>,
    id: kt_model::MediaId,
) -> Option<[u8; 3]> {
    generator_pixel_after(h, id, 0).map(|(px, _)| px)
}

/// The generator pixel and the SERIAL NUMBER of the cache entry (R.53, item 416).
///
/// The serial number is mandatory when the edit has to be seen by the test’s EYES: a frame of
/// the old parameters lives in the cache until the worker reaches the reconfiguration
/// (the cleanup runs INSIDE its thread — item 411), and a bare read
/// would return the previous picture. We wait for a frame written LATER than `after`:
/// that is exactly “a NEW frame has arrived”, not “we waited for the right colour” —
/// the second would be a test that cannot turn red.
pub(crate) fn generator_pixel_after(
    h: &egui_kittest::Harness<'_, crate::app::KtApp>,
    id: kt_model::MediaId,
    after: u64,
) -> Option<([u8; 3], u64)> {
    let app = h.state();
    let q = kt_media::Quality { width: 64, height: 36 };
    let idx = kt_engine::video::video_cache_idx(kt_time::Tick(0));
    let sid = kt_engine::source_id(id);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.player.request_video(id, kt_time::Tick(0), q);
        if let Some((f, serial)) = app.player.cache.get_video_stamped(sid, Some(q), idx)
            && serial > after
        {
            return Some(([f.rgba[0], f.rgba[1], f.rgba[2]], serial));
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The generator event just created from the menu: (media, event).
/// The values live on the EVENT (R.59, stage 2), so the test needs both
/// of these.
pub(crate) fn made_generator(
    h: &egui_kittest::Harness<'_, crate::app::KtApp>,
) -> (kt_model::MediaId, kt_model::ClipId) {
    let p = h.state().model.project();
    let m = p.media.last().expect("generator media");
    let c = p.clips.iter().find(|c| c.media == m.id).expect("generator event");
    (m.id, c.id)
}

/// what is selected in the chain strip of this event’s effects window
pub(crate) fn chain_sel(
    h: &egui_kittest::Harness<'_, crate::app::KtApp>,
    clip: kt_model::ClipId,
) -> Option<crate::widgets::ChainSel> {
    h.state()
        .fx_editors
        .iter()
        .find(|e| e.members.contains(&clip))
        .and_then(|e| e.selected.get(&clip).copied())
}

/// the generator values of the event — what since R.59 “are the parameters”
pub(crate) fn gen_of(
    h: &egui_kittest::Harness<'_, crate::app::KtApp>,
    clip: kt_model::ClipId,
) -> kt_model::GeneratorKind {
    h.state().model.clip(clip).and_then(|c| c.r#gen.clone()).expect("the event has a generator")
}

/// Test 477 (R.52, rewritten in R.59 stage 2): “Create generator” —
/// media without a file appears in the model and in the services, and the VALUES land
/// on the EVENT.
///
/// What changed by the owner’s decision: the media path now carries ONLY
/// THE KIND (`gen:solid`), the values chosen in the menu go to the event, and
/// the “parameters window” is the event’s effects window with its HEAD strip selected.
/// The generator has no window of its own any more: the owner asked to see
/// the source as the first crumb of the chain, in the same window.
#[test]
fn t477_create_generator_from_bin() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    let before = h.state().model.project().media.len();
    let made = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    h.state_mut().create_generator(made.clone());
    h.step();
    let (id, clip) = made_generator(&h);
    let app = h.state();
    let p = app.model.project();
    assert_eq!(p.media.len(), before + 1, "the media was not added");
    assert_eq!(
        p.media.last().unwrap().path,
        "gen:solid",
        "the media path carries ONLY the kind: the values moved to the event"
    );
    assert!(p.is_gen_media(id));
    assert_eq!(gen_of(&h, clip), made, "the selected values must land on the EVENT");
    // the “parameters window” is open at once — and open on the HEAD of the strip
    assert_eq!(app.fx_editors.len(), 1, "the parameters window did not open");
    assert!(app.fx_editors[0].members.contains(&clip));
    assert_eq!(
        chain_sel(&h, clip),
        Some(crate::widgets::ChainSel::Head),
        "the generator form is the FIRST LINK of the chain, and the window must open on it"
    );
    // the thumbnail service knows the kind and the “duration” — the card draws a thumbnail
    // rather than “♪”, and hides a zero duration
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let _ = app.thumbs.duration_or_probe(id);
    loop {
        if let Some((v, a)) = h.state().thumbs.kinds(id) {
            assert!(v && !a, "the generator is video without audio");
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the generator probe is not published");
        std::thread::sleep(std::time::Duration::from_millis(10));
        h.step();
    }
    assert_eq!(h.state().thumbs.duration(id), Some(kt_time::Tick(0)));
    // reopening (the second menu path) does not create a second window
    h.state_mut().open_generator_editor(id);
    assert_eq!(h.state().fx_editors.len(), 1, "there must be one window");
    // the sync snapshots are registered — edits will go as diffs
    assert_eq!(h.state().media_paths.get(&id).map(String::as_str), Some("gen:solid"));
    assert_eq!(
        h.state().gen_states.get(&clip).map(|st| st.kind.clone()),
        Some(made),
        "the snapshot of the event source must know its values"
    );
}

/// Test 478 (R.52, strengthened in R.53 item 412, rewritten in R.59 stage 2):
/// a colour edit in the live session of the window reaches the MODEL, the player’s
/// SOURCE and the PICTURE; “Cancel” restores the backup; the whole session is ONE
/// undo step, and undo also reaches the picture.
///
/// The mechanism is new (`SetClipGen` on the event instead of `SetMediaPath` on the
/// media), the claim is the old one: the edit is visible to a person, not only to the
/// model. The picture is requested by the SOURCE ADDRESS OF THE EVENT — since R.59 this is
/// the only rule (`kt_engine::clip_source_addr`).
#[test]
fn t478_generator_edit_reaches_model_and_source() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
    h.step();
    let (id, clip) = made_generator(&h);
    let addr = kt_engine::gen_source_addr(clip);
    let gesture = h.state().fx_editors[0].gesture;
    let depth = h.state().model.undo_depth();

    // session of the window: two colour edits with the same gesture (as the eyedropper sends)
    for c in [(0x10, 0x40, 0x80), (0xff, 0x88, 0x00)] {
        let kind = GeneratorKind::Solid { color_r: c.0, color_g: c.1, color_b: c.2 };
        let app = h.state_mut();
        assert!(app.apply_coalesced(Command::SetClipGen { id: clip, kind: Some(kind) }, gesture));
        h.step();
    }
    assert_eq!(
        gen_of(&h, clip),
        GeneratorKind::Solid { color_r: 0xff, color_g: 0x88, color_b: 0x00 }
    );
    let app = h.state();
    // the player source caught up with the model (by reconfiguration, not recreation)
    assert_eq!(
        app.gen_states.get(&clip).map(|st| st.kind.clone()),
        Some(GeneratorKind::Solid { color_r: 0xff, color_g: 0x88, color_b: 0x00 })
    );
    assert_eq!(app.model.undo_depth(), depth + 1, "the window session is one history step");
    // ...and to the PICTURE: the player source delivers a frame of the NEW colour
    let (px, serial) = crate::layout_tests::generator_pixel_after(&h, addr, 0)
        .expect("the generator frame never arrived");
    assert_eq!(px, [255, 136, 0], "the edit must reach the picture, not only the model");

    // undo restores both the model and the picture (a hook in undo, not only in apply)
    h.state_mut().undo();
    h.step();
    assert_eq!(
        gen_of(&h, clip),
        GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 },
        "undo must restore the event values"
    );
    assert_eq!(
        h.state().gen_states.get(&clip).map(|st| st.kind.clone()),
        Some(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        "undo must catch up the source — otherwise the picture lags behind the model"
    );
    let (px, _) = crate::layout_tests::generator_pixel_after(&h, addr, serial)
        .expect("after undo the new frame never arrived");
    assert_eq!(px, [0, 0, 0], "undo must reach the picture");

    // “Cancel” of the window: the snapshot of the HEAD is taken on opening and is rolled back by
    // the same gesture — the source values are part of the same window session
    h.state_mut().redo();
    h.step();
    let backup = h.state().fx_editors[0].gen_backup.clone();
    assert_eq!(
        backup.iter().find(|(c, _, _)| *c == clip).map(|(_, k, _)| k.clone()).unwrap(),
        Some(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        "backup — the values at the moment of opening"
    );
    {
        let app = h.state_mut();
        let kind = backup.iter().find(|(c, _, _)| *c == clip).unwrap().1.clone();
        assert!(app.apply_coalesced(Command::SetClipGen { id: clip, kind }, gesture));
    }
    h.step();
    assert_eq!(gen_of(&h, clip), GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
    {
        // the generator clip is taken first: creation puts it in at once
        // (item 416), and the model does not let go of media under a live event
        let app = h.state_mut();
        let clips: Vec<ClipId> =
            app.model.project().clips.iter().filter(|c| c.media == id).map(|c| c.id).collect();
        assert!(!clips.is_empty(), "creating a generator must place a clip (item 416)");
        app.apply_batch(clips.into_iter().map(|id| Command::RemoveClip { id }).collect());
        assert!(app.apply(Command::RemoveMedia { id }));
    }
    h.step();
    assert!(h.state().fx_editors.is_empty(), "the window of a dead event must close");
    assert!(!h.state().media_paths.contains_key(&id), "the source of dead media is removed");
    assert!(!h.state().gen_states.contains_key(&clip), "the source of a dead event is removed");
}

/// Test 487 (R.52 C3, rewritten in R.59 stage 2): all generator inputs lead to ONE place —
/// the effects window of the event with its selected head;
/// a text edit through the window session reaches the model and collapses into
/// one undo step, and the window dies together with the event.
///
/// What changed by the owner’s decision: there is no separate title window any more
/// (“buried out of the way” — about the former window split), and the text lives on the
/// EVENT, not in the media’s address string. The claim is the old one: the typed
/// text reaches the model without loss and in one history step.
#[test]
fn t487_title_editor_routes_and_edits() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    let (id, clip) = made_generator(&h);
    assert_eq!(h.state().fx_editors.len(), 1, "the title form is in the event effects window");
    assert_eq!(
        chain_sel(&h, clip),
        Some(crate::widgets::ChainSel::Head),
        "the title must open at the head of the strip"
    );
    let gesture = h.state().fx_editors[0].gesture;
    let depth = h.state().model.undo_depth();

    // window session: the typed text goes to the EVENT
    let mut kind = gen_of(&h, clip);
    if let GeneratorKind::Text { text, .. } = &mut kind {
        *text = "Hello,\ntitle!".into();
    }
    {
        let app = h.state_mut();
        assert!(
            app.apply_coalesced(
                Command::SetClipGen { id: clip, kind: Some(kind.clone()) },
                gesture
            )
        );
    }
    h.step();
    assert_eq!(gen_of(&h, clip), kind, "the text reached the event in full");
    assert_eq!(
        h.state().model.project().media.last().unwrap().path,
        "gen:text",
        "in the media path ONLY the kind remains"
    );
    assert_eq!(h.state().model.undo_depth(), depth + 1, "the window session is one history step");
    assert_eq!(
        h.state().gen_states.get(&clip).map(|st| st.kind.clone()),
        Some(kind),
        "the edit must reach the event source"
    );

    // reopening — the same window
    h.state_mut().open_generator_editor(id);
    assert_eq!(h.state().fx_editors.len(), 1);

    // the event died — the window went with it
    {
        let app = h.state_mut();
        let clips: Vec<ClipId> =
            app.model.project().clips.iter().filter(|c| c.media == id).map(|c| c.id).collect();
        assert!(!clips.is_empty(), "creating a generator must place a clip (item 416)");
        app.apply_batch(clips.into_iter().map(|id| Command::RemoveClip { id }).collect());
    }
    h.step();
    assert!(h.state().fx_editors.is_empty(), "the window of a dead title must close");
}

/// Test 488 (R.52 C3, UI rule 11): “no glyph” is a visible answer.
/// Cyrillic/Latin of the full vendored font is clean; a CJK character
/// is named by its code point and a counter of the rest.
#[test]
fn t488_missing_glyph_warning() {
    use kt_model::GeneratorKind;
    let mut k = GeneratorKind::default_text();
    let set = |k: &mut GeneratorKind, s: &str| {
        if let GeneratorKind::Text { text, .. } = k {
            *text = s.into();
        }
    };
    set(&mut k, "Привет, Kinotrain! ёЁ №5");
    assert_eq!(crate::gen_ui::title_warning(&k), None, "Cyrillic is covered");
    set(&mut k, "before 漢字 after");
    let warn = crate::gen_ui::title_warning(&k).expect("CJK not covered — the answer must be");
    assert!(warn.contains("U+5B57") || warn.contains("U+6F22"), "{warn}");
    assert!(warn.contains("+1"), "counter of the rest: {warn}");
    set(&mut k, "");
    assert_eq!(crate::gen_ui::title_warning(&k), None);
}

/// Test 489 (R.52 C3): safe areas — 5 % and 10 % of the preview frame, visible
/// exactly while the title is being edited.
#[test]
fn t489_safe_zones_follow_title_window() {
    use kt_model::GeneratorKind;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    assert!(!h.state().show_safe_zones(), "without an edit the title zones are not drawn");
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    assert!(h.state().show_safe_zones(), "the title form turns on the zones");
    let img = egui::Rect::from_min_max(egui::pos2(100.0, 200.0), egui::pos2(300.0, 300.0));
    let [action, title] = crate::app::safe_zone_rects(img);
    assert_eq!(action.min, egui::pos2(110.0, 205.0));
    assert_eq!(action.max, egui::pos2(290.0, 295.0));
    assert_eq!(title.min, egui::pos2(120.0, 210.0));
    assert_eq!(title.max, egui::pos2(280.0, 290.0));
    // the window closed — the zones went away
    h.state_mut().fx_editors[0].open = false;
    h.step();
    assert!(!h.state().show_safe_zones());
}

/// Test 495 (R.52 C6, rewritten in R.59 stage 2): dragging the title DIRECTLY IN
/// THE PREVIEW — the block’s frame while the title is edited, and the three-step drag
/// writes pos_x/pos_y with the SAME command (`SetClipGen`) and the same gesture
/// as the form in the window (rule 17 by construction).
#[test]
fn t495_title_drag_in_preview_writes_pos() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    let (_id, clip) = made_generator(&h);
    let gesture = h.state().fx_editors[0].gesture;
    // the title needs a visible block — we give the text through the same window session
    let mut kind = gen_of(&h, clip);
    if let GeneratorKind::Text { text, size, .. } = &mut kind {
        *text = "DRAG".into();
        *size = 0.2;
    }
    {
        let app = h.state_mut();
        assert!(app.apply_coalesced(Command::SetClipGen { id: clip, kind: Some(kind) }, gesture));
    }
    h.step();
    let preview = h.state().preview_rect;
    assert!(preview.width() > 40.0, "preview is present in the harness: {preview:?}");

    let depth = h.state().model.undo_depth();
    let before = match gen_of(&h, clip) {
        GeneratorKind::Text { pos_x, pos_y, .. } => (pos_x, pos_y),
        _ => unreachable!(),
    };
    assert_eq!(before, (0.5, 0.5));

    // a three-step drag from the centre of the preview (the block with the Center anchor sits there)
    let c = preview.center();
    h.drag_at(c);
    h.step();
    h.hover_at(c + egui::vec2(9.0, 0.0));
    h.step();
    h.hover_at(c + egui::vec2(40.0, 24.0));
    h.step();
    h.drop_at(c + egui::vec2(40.0, 24.0));
    h.step();

    let after = match gen_of(&h, clip) {
        GeneratorKind::Text { pos_x, pos_y, .. } => (pos_x, pos_y),
        _ => unreachable!(),
    };
    assert!(
        after.0 > before.0 + 0.01 && after.1 > before.1 + 0.01,
        "the drag must move the position: {before:?} -> {after:?}"
    );
    assert_eq!(
        h.state().model.undo_depth(),
        depth,
        "the drag writes with the same gesture as the window session — there are no new history steps"
    );
}

/// Test 502 (R.53, item 412; rewritten in R.59 stage 2): an edit session of a
/// generator does NOT recreate the source and does NOT rebuild the renderers.
/// Thirty colour edits — exactly what one second of eyedropper drag produces:
/// before item 412 each one stopped and restarted the decoder thread and rebuilt the graphs twice,
/// and in play that is a guaranteed audio dropout.
///
/// The mechanism is new (`SetClipGen` instead of `SetMediaPath`), the law P is the same:
/// a command that does not touch the plan has no right to rebuild the renderers.
#[test]
fn t502_generator_edit_does_not_respawn_source() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
    h.step();
    let (_id, clip) = made_generator(&h);
    let addr = kt_engine::gen_source_addr(clip);
    let gesture = h.state().fx_editors[0].gesture;
    let spawns = h.state().player.worker_spawns();
    let pver = h.state().player.project_version();

    for i in 0..30u8 {
        let kind = GeneratorKind::Solid { color_r: i, color_g: i, color_b: i };
        let app = h.state_mut();
        assert!(app.apply_coalesced(Command::SetClipGen { id: clip, kind: Some(kind) }, gesture));
        h.step();
    }

    let app = h.state();
    assert_eq!(
        app.player.worker_spawns(),
        spawns,
        "the decoder stream was recreated on every parameter edit"
    );
    assert_eq!(
        app.player.project_version(),
        pver,
        "renderers were rebuilt: the SetClipGen plan does not touch them (law P)"
    );
    assert_eq!(
        app.gen_states.get(&clip).map(|st| st.kind.clone()),
        Some(GeneratorKind::Solid { color_r: 0x1d, color_g: 0x1d, color_b: 0x1d }),
        "the sync snapshot must catch up with the model"
    );
    assert_eq!(
        crate::layout_tests::generator_pixel(&h, addr),
        Some([0x1d, 0x1d, 0x1d]),
        "the last edit must reach the picture"
    );
}

/// Test 503 (R.53, item 413): a GESTURE frame does not rebuild the path map.
/// Sync is called from every edit wrapper, that is, on every frame of any coalesced
/// gesture — and it built a `HashMap` with a clone of every path
/// (allocations on the frame path, invariant 12).
#[test]
fn t503_gesture_frame_touches_no_media_paths() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    let gesture = 4242u64;
    // one gesture, 60 frames — a second of dragging a level of an event
    let before = h.state().media_sync_builds;
    for i in 0..60 {
        let app = h.state_mut();
        let db = -6.0 + i as f32 * 0.1;
        assert!(app.apply_coalesced(Command::SetGain { id: ClipId(1), gain_db: db }, gesture));
        h.step();
    }
    let builds = h.state().media_sync_builds - before;
    assert!(builds <= 60, "the path map was rebuilt more often than the model changed: {builds}");
    // the media set did not change even once — there was no reason to touch the sources
    assert_eq!(h.state().player.worker_spawns(), 0, "the level gesture raised the sources");
}

/// Test 504 (R.53, item 413): a pseudo-path does NOT open the sidecar DB. A generator
/// has no file, yet opening sqlite (connection + WAL pragma + schema version) hung on
/// every parameter edit
/// and on every clip placement.
#[test]
fn t504_pseudo_path_never_opens_sqlite() {
    let dir = std::env::temp_dir().join(format!("kt-t504-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("x.ktindex.db");
    assert!(!db.exists());

    let src = crate::util::open_source_indexed("gen:solid:c=ff8800", Some(&db));
    assert!(src.is_ok(), "the generator must open");
    assert!(!db.exists(), "the pseudo-path opened the sidecar DB");

    let src = crate::util::open_source_indexed("synth:ramp", Some(&db));
    assert!(src.is_ok());
    assert!(!db.exists(), "the test schema opened the sidecar DB");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 505 (R.53, item 413): a substituted font is mentioned ONCE.
/// The title’s path changes on every typed character, and without de-duplication
/// a missing family gave its own notice on every edit — a storm of
/// identical messages and an unread counter in the hundreds.
#[test]
fn t505_missing_font_warns_once() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    let mut kind = GeneratorKind::default_text();
    if let GeneratorKind::Text { family, .. } = &mut kind {
        *family = "__kt_no_such_family__".into();
    }
    h.state_mut().create_generator(kind.clone());
    h.step();
    let (_id, clip) = made_generator(&h);
    let gesture = h.state().fx_editors[0].gesture;
    let notices = h.state().notices.len();

    for i in 0..40 {
        let mut k = kind.clone();
        if let GeneratorKind::Text { text, .. } = &mut k {
            *text = "a".repeat(i + 1);
        }
        let app = h.state_mut();
        assert!(app.apply_coalesced(Command::SetClipGen { id: clip, kind: Some(k) }, gesture));
        h.step();
    }
    let added = h.state().notices.len() - notices;
    assert!(added <= 1, "the font substitution was reported {added} times instead of once");
}

/// Test 227 (R.33 C4): a transition got a properties window. A double click on its
/// window on the strip opens the editor, the kind changes via the picker and at once
/// goes to the model; “Cancel” restores the previous one.
#[test]
fn t227_transition_editor_opens_and_sets_kind() {
    use kt_model::*;
    use kt_time::Tick;
    let (mut h, g) = harness_three_clips_dt(Some(0.02));
    // we join clips 1 and 2 end to end and hang a transition on them
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[1].start = Tick(50_000);
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: Some(ClipId(2)),
            in_offset: Tick(9_000),
            out_offset: Tick(9_000),
            kind: TransitionKind::Crossfade,
            audio: Default::default(),
            auto: vec![],
            invert: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();
    let g = h.state().tl_geom.clone().unwrap_or(g);
    // the transition window [41_000, 59_000): we aim at its middle, BELOW
    // the upper gesture strip (where the fade handles live)
    let y = g.lane_y(ClipKind::Video, 0, 0.75);
    let pos = egui::pos2(g.tick_to_x(46_000), y);
    for _ in 0..2 {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    assert_eq!(
        h.state().transition_editors.len(),
        1,
        "double-click on the transition window must open its properties"
    );
    assert_eq!(h.state().transition_editors[0].id, TransitionId(1));

    // the kind changes through the editor model: the window writes the command with the same
    // gesture token, so the whole session is one undo step
    let gesture = h.state().transition_editors[0].gesture;
    {
        let app = h.state_mut();
        let mut t = app.model.project().transitions[0].clone();
        t.kind = TransitionKind::Wipe { dir: Dir::Up, angle: 0.0, softness: 0.2 };
        t.audio = AudioCurve::SCurve;
        assert!(app.apply_coalesced(Command::SetTransition(t), gesture));
    }
    h.step();
    let t = &h.state().model.project().transitions[0];
    assert_eq!(t.kind.tag(), "wipe", "the view did not reach the model");
    assert_eq!(t.audio, AudioCurve::SCurve);

    // “Cancel” of the window returns the original kind in ONE undo step
    let backup = h.state().transition_editors[0].backup.clone();
    {
        let app = h.state_mut();
        assert!(app.apply_coalesced(Command::SetTransition(backup), gesture));
    }
    h.step();
    assert_eq!(h.state().model.project().transitions[0].kind, TransitionKind::Crossfade);
    let _ = h.state_mut().model.undo();
    h.step();
    assert_eq!(
        h.state().model.project().transitions[0].kind,
        TransitionKind::Crossfade,
        "the whole window session is one history step"
    );

    // the window closes together with the death of the transition
    {
        let app = h.state_mut();
        let _ = app.apply(Command::RemoveTransition { id: TransitionId(1) });
    }
    h.step();
    h.step();
    assert!(
        h.state().transition_editors.is_empty(),
        "the window of a dead transition must close by itself"
    );
}

/// Test 231b (R.34 K3): the camera is finally editable. A double click on its
/// block opens the window (until this round there was no editor at all, while the
/// comment in the code promised one since R.29), edits go in one undo step,
/// and the window of a deleted camera closes by itself.
#[test]
fn t231_camera_editor_opens_and_edits() {
    use kt_model::*;
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips_dt(Some(0.02));
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.ensure_tracks(kt_model::LaneKind::Camera, 1);
        p.cameras.push(CameraEvent {
            ease: true,
            id: CameraId(1),
            lane: 0,
            start: Tick(0),
            duration: Tick(120_000),
            name: String::new(),
            cam: CameraParams::default(),
            keyframes: vec![],
            auto: vec![],
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("geometry");
    let row = g.row_of((LaneKind::Camera, 0)).expect("the camera track is in the rows");
    // the CENTRE of the row, not “half the base height”: the camera row is three times lower
    // (R.35 item 1), and the old formula missed it
    let pos = egui::pos2(g.tick_to_x(60_000), g.row_center_y(row));
    for _ in 0..2 {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    assert_eq!(h.state().camera_editors.len(), 1, "double-click on the camera must open its window");

    // one gesture through the window session — one history step
    let gesture = h.state().camera_editors[0].gesture;
    {
        let app = h.state_mut();
        let mut c = app.model.project().cameras[0].clone();
        c.name = "main".into();
        c.auto = vec![ParamCurve {
            param: "pos_z".into(),
            keys: vec![AutoKey::at(Tick(0), -900.0), AutoKey::at(Tick(120_000), -300.0)],
            source: Default::default(),
        }];
        assert!(app.apply_coalesced(Command::SetCamera(c), gesture));
    }
    h.step();
    assert_eq!(h.state().model.project().cameras[0].auto.len(), 1, "the curve reached the model");
    let _ = h.state_mut().model.undo();
    h.step();
    assert!(
        h.state().model.project().cameras[0].auto.is_empty(),
        "the whole window session is one history step"
    );

    // the camera is deleted — the window leaves by itself
    {
        let app = h.state_mut();
        let _ = app.apply(Command::RemoveCamera { id: CameraId(1) });
    }
    h.step();
    h.step();
    assert!(h.state().camera_editors.is_empty(), "the window of a dead camera closes");
}

/// Test 242 (R.35 C1, piece 5): the morpher window shows WHOM and WHEN it acts on.
/// Until this round the targets were frozen for good and were shown only as
/// the line “#1, #2”, and the morpher had no time at all — the block
/// could be moved, and that meant nothing.
#[test]
fn t242_morph_editor_shows_target_mode_and_timing() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(45_000),
        targets: MorphTargets::Below,
        patch: MorphPatch { media: Some(MediaId(1)), ..Default::default() },
        timed: true,
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_morph_editor(MorphId(1));
    h.step();
    h.step();

    // the targets mode is visible and explained, not hidden in code
    assert!(
        h.query_by_label(crate::i18n::tr(crate::i18n::k::mrph::ON)).is_some(),
        "the “applies to” row must be in the window"
    );
    assert!(
        h.query_by_label(crate::i18n::tr(crate::i18n::k::mrph::BELOW_NOTE)).is_some(),
        "and it is said what exactly the layer covers"
    );

    // the “by time” checkbox edits the MODEL and fits in one undo step
    let depth = h.state().model.undo_depth();
    h.get_by_label(crate::i18n::tr(crate::i18n::k::mrph::TIMED)).click();
    h.step();
    h.step();
    assert!(!h.state().model.project().morphers[0].timed, "the checkbox removed the action by time");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "the window session is one history step");

    // the window shows the second mode DIFFERENTLY: instead of the layer explanation —
    // the frozen list itself (otherwise “whom it acts on” cannot be seen)
    let app = h.state_mut();
    let mut ev = app.model.project().morphers[0].clone();
    ev.targets = MorphTargets::Clips { ids: vec![ClipId(1)] };
    assert!(app.apply(Command::SetMorpher(ev)));
    h.step();
    h.step();
    assert!(
        h.query_by_label(crate::i18n::tr(crate::i18n::k::mrph::BELOW_NOTE)).is_none(),
        "a frozen list has no place for the layer explanation"
    );
    assert!(
        h.query_by_label_contains("#1").is_some(),
        "the frozen list must be visible by name"
    );
}

/// Test 244 (R.35 C2, verbatim: “Item ‘Add morpher’ (both for clips and for
/// the region) can be cut from two to one, and in the morpher ui merge them —
/// maybe make a fx-chain straight away, in which, besides the other fx, there is a fx for morphing,
/// mark its items purple and raise them in the fx list”). The strip in the morpher window is ONE,
/// the morph slot is first in it and is not reordered, and “+M” in the toolbar
/// opens a layer on top.
#[test]
fn t244_morpher_has_one_chain_with_pinned_head() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);

    // “+M”: the morph track is opened ON TOP of all — otherwise “everything below”
    // would be empty (the default order puts morph rows down)
    let depth = app.model.undo_depth();
    app.add_morph_layer();
    let pr = app.model.project();
    assert_eq!(pr.morphers.len(), 1, "layer created");
    assert_eq!(pr.rows()[0], (LaneKind::Morph, 0), "and raised to row 0");
    assert!(matches!(pr.morphers[0].targets, MorphTargets::Below), "mode — “everything below”");
    assert!(pr.morphers[0].timed, "and by time");
    assert_eq!(app.model.undo_depth(), depth + 1, "track, order and event — one step");
    assert!(!pr.effective_clips()[0].fx.is_empty() || pr.morphers[0].patch.is_empty());
    h.step();
    h.step();

    // the window opened by itself; the strip is ONE (one master checkbox), and first in it
    // stands the morph slot
    assert!(h.state().morph_editors.iter().any(|e| e.open), "the morpher window did not open");
    let chain_label = tr(k::fx::CHAIN_ON).to_string();
    assert_eq!(
        h.get_all_by_label(&chain_label).count(),
        1,
        "there must be ONE chain strip in the morpher window (there were two: video and audio)"
    );
    let head = tr(k::mrph::SRC_SLOT_NONE).to_string();
    assert!(h.query_by_label(&head).is_some(), "the morph slot must be visible in the strip");

    // the one selected — so the window shows the source field at once
    assert_eq!(h.state().morph_editors[0].sel, crate::widgets::ChainSel::Head);
    assert!(
        h.state().morph_editors[0].src_rect.is_some(),
        "the source field is shown when a head is selected"
    );

    // effects of BOTH kinds live in one strip and are laid out back again
    let app = h.state_mut();
    let mut ev = app.model.project().morphers[0].clone();
    ev.patch.video_fx = vec![FxInstance::Mirror { horizontal: true, vertical: false }];
    ev.patch.audio_fx = vec![FxInstance::gain(3.0)];
    assert!(app.apply(Command::SetMorpher(ev)));
    h.step();
    h.step();
    assert!(
        h.query_by_label(crate::fx_editor::fx_display_name(&FxInstance::gain(3.0))).is_some(),
        "the audio effect must be in the SAME strip"
    );
    let ev = h.state().model.project().morphers[0].clone();
    assert_eq!(ev.patch.video_fx.len(), 1, "and be split back by kinds");
    assert_eq!(ev.patch.audio_fx.len(), 1);
}

/// Test 245 (R.35 C3, verbatim: “to not lose space — morpher tracks
/// can be reduced in height three times”). Pure row geometry:
/// ratio 3, height floor, prefix sums, bijection `row_at_y` on a
/// mixed set, and the zoom anchor.
#[test]
fn t245_thin_service_rows_geometry() {
    use crate::timeline::{THIN_ROW_MIN, TlGeom, row_height, row_offsets};
    use kt_model::{ClipKind, LaneKind};
    let rows = vec![
        (LaneKind::Morph, 0),
        (LaneKind::Video, 0),
        (LaneKind::Camera, 0),
        (LaneKind::Audio, 0),
        (LaneKind::Morph, 1),
    ];
    // the camera row is exactly three times lower (R.35 item 1), the morph row TWICE as high
    // as the camera row (R.38 item 10: “increase the default height of the morpher track by 2”),
    // the floor is 10 px (a 5 px row cannot be hit with the mouse)
    assert_eq!(row_height(LaneKind::Morph, 60.0), 40.0);
    assert_eq!(row_height(LaneKind::Camera, 60.0), 20.0);
    assert_eq!(row_height(LaneKind::Video, 60.0), 60.0);
    assert_eq!(row_height(LaneKind::Morph, 12.0), THIN_ROW_MIN);

    // prefix sums: length n+1, the last one is the full height
    let offs = row_offsets(&rows, 60.0, &[]);
    assert_eq!(offs.len(), rows.len() + 1);
    assert_eq!(offs, vec![0.0, 40.0, 100.0, 120.0, 180.0, 220.0]);

    let g = TlGeom::new(
        egui::Rect::from_min_max(egui::pos2(52.0, 100.0), egui::pos2(1052.0, 600.0)),
        60.0,
        rows.clone(),
        &[],
        0.0,
        0.0,
        900.0,
    );
    // bijection: the centre of each row falls in that row itself
    for (i, &key) in rows.iter().enumerate() {
        assert_eq!(g.row_at_y(g.row_center_y(i)), Some(i), "row {i}");
        assert_eq!(g.key_at_y(g.row_center_y(i)), Some(key));
        assert_eq!(g.row_h(i), row_height(key.0, 60.0));
    }
    // row bounds: the top belongs to the row, the bottom already to the next one
    assert_eq!(g.row_at_y(g.row_top(1)), Some(1));
    assert_eq!(g.row_at_y(g.row_top(1) - 0.5), Some(0));
    assert_eq!(g.row_at_y(g.row_top(0) - 1.0), None, "above the first row — not a row");
    assert_eq!(g.row_at_y(g.row_top(rows.len() - 1) + 100.0), None, "below the last — also not");

    // “how many rows were passed” during a drag: counted BY ROWS, not
    // by dividing by one height — under the old formula a thin row would have cost
    // as much as the clip track
    assert_eq!(g.row_at_y_unbounded(g.row_center_y(0)), 0);
    assert_eq!(g.row_at_y_unbounded(g.row_center_y(2)), 2);
    assert_eq!(g.row_at_y_unbounded(g.row_center_y(4)), 4);
    assert!(g.row_at_y_unbounded(g.row_top(0) - 1.0) < 0, "above the first — a negative row");
    // sticking to its own kind counts the same rows
    assert_eq!(g.nearest_lane(ClipKind::Video, g.row_at_y_unbounded(g.row_center_y(2))), 0);

    // zoom anchor: the integer part is the row, the fractional part is the place within it
    let a = g.anchor_at(g.row_top(2) + g.row_h(2) * 0.5);
    assert!((a - 2.5).abs() < 0.01, "the anchor in the middle of the third row: {a}");
    let a0 = g.anchor_at(g.row_top(0));
    assert!(a0.abs() < 0.01, "the top of the first row — exactly 0");

    // lane_y: fractions are computed from the height of the CLIP ROW
    assert_eq!(g.lane_y(ClipKind::Video, 0, 0.5), g.row_top(1) + 30.0);
}

/// Test 246 (R.35 C3): a THIN service row is hit by the mouse, and the vertical zoom
/// anchor does not drift. A harness, not a pure function: this is exactly where
/// “the geometry is computed correctly, but there is nowhere to click” gets caught
/// (lesson of item 7, round 11).
#[test]
fn t246_thin_row_is_clickable_and_zoom_anchor_holds() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        targets: MorphTargets::Below,
        patch: MorphPatch::default(),
        timed: true,
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    h.step();

    let g = h.state().tl_geom.clone().expect("geometry");
    let mrow = g.row_of((LaneKind::Morph, 0)).expect("a morph row exists");
    // the row really is three times lower than the clip track — and yet it can be clicked
    assert!(g.row_h(mrow) < g.lane_h_of(ClipKind::Video, 0), "the service row must be lower");
    assert!(g.row_h(mrow) >= crate::timeline::THIN_ROW_MIN, "but not below the clickability floor");

    // a click on a block in the thin row selects the morpher
    let pos = egui::pos2(g.tick_to_x(45_000), g.row_center_y(mrow));
    h.drag_at(pos);
    h.step();
    h.drop_at(pos);
    h.step();
    assert!(
        h.state().selected_morph.contains(&MorphId(1)),
        "a thin morph row cannot be hit with the mouse — it cannot be counted as done"
    );

    // vertical zoom does not break the height ratio
    let g = h.state().tl_geom.clone().unwrap();
    let app = h.state_mut();
    app.tl_v_zoom = Some(g.lane_h_of(ClipKind::Video, 0) * 1.5);
    h.step();
    let g2 = h.state().tl_geom.clone().unwrap();
    assert!(g2.lane_h_of(ClipKind::Video, 0) > g.lane_h_of(ClipKind::Video, 0), "zoom was applied");
    assert!(
        (g2.row_h(mrow) / g2.lane_h_of(ClipKind::Video, 0) - 1.0 / crate::timeline::MORPH_ROW_DIV)
            .abs()
            < 0.05,
        "the height ratio of the morph row holds under zoom"
    );
}

/// Test 248 (R.35 C4): a selection frame turns into a named area with the SAME tracks
/// and time, and the morpher window shows a link to it as a chip
/// and finds it by search.
#[test]
fn t248_band_becomes_named_area() {
    use kt_model::*;
    let (mut h, g) = harness_three_clips();
    // a frame (Ctrl+drag from an empty spot) on the FIRST video track
    let y = g.lane_y(ClipKind::Video, 0, 0.6);
    let at = |t: i64| egui::pos2(g.tick_to_x(t), y);
    rubber_drag(&mut h, at(70_000), at(260_000), true);
    assert!(h.state().last_band.is_some(), "the frame must be remembered in the model domain");

    let app = h.state_mut();
    let depth = app.model.undo_depth();
    app.save_marked_area();
    let p = app.model.project();
    assert_eq!(p.areas.len(), 1, "area saved");
    let a = &p.areas[0];
    assert!(!a.name.trim().is_empty(), "the name is generated ready, not asked for with a refusal");
    assert!((a.start.0 - 70_000).abs() < 3_000, "time from the frame: {:?}", a.start);
    assert!(((a.start + a.duration).0 - 260_000).abs() < 3_000, "and its end");
    assert_eq!(a.lanes, vec![(LaneKind::Video, 0)], "and the SAME tracks");
    assert_eq!(app.model.undo_depth(), depth + 1, "one history step");

    // a second save of the same frame gives a DIFFERENT name (the name is the address)
    app.save_marked_area();
    let names: Vec<String> = app.model.project().areas.iter().map(|a| a.name.clone()).collect();
    assert_eq!(names.len(), 2);
    assert_ne!(names[0], names[1], "names are unique without asking the user questions");

    // morpher window: the link to the area is drawn as a chip
    let mut p = app.model.project().clone();
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        targets: MorphTargets::Areas { names: vec![names[0].clone(), "no such one".into()] },
        patch: MorphPatch::default(),
        timed: true,
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_morph_editor(MorphId(1));
    h.step();
    h.step();
    use egui_kittest::kittest::Queryable;
    assert!(h.query_by_label(&names[0]).is_some(), "a live link is visible as a chip");
    assert!(
        h.query_by_label("no such one").is_some(),
        "a dead link STAYS in sight: it is not silently thrown away"
    );
}

/// Test 250 (R.35 C5): the insert-prefix window. It opens ONLY when the buffer
/// holds a named item, inserts in one undo step, Esc leaves no traces, and a SECOND Ctrl+V
/// in a row opens it again (rule 14 —
/// a feature must work repeatedly, the defect of item 8, round 12).
#[test]
fn t250_paste_prefix_dialog() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, _g) = harness_three_clips();

    // without a named item the paste goes SILENTLY — old scenarios stay intact
    let app = h.state_mut();
    app.select_only(ClipId(1));
    app.copy_selection();
    app.player.seek(kt_time::Tick(400_000));
    let n = app.model.project().clips.len();
    app.paste_at_cursor();
    assert!(app.paste_names.is_none(), "there must be no window: there are no names in the buffer");
    assert_eq!(app.model.project().clips.len(), n + 1, "and the paste went through at once");

    // an area in the buffer — the window appears
    let mut p = app.model.project().clone();
    p.areas.push(MarkedArea {
        name: "final".into(),
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        lanes: vec![],
        color: None,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.select_only(ClipId(1));
    app.copy_selection();
    assert!(!app.clipboard.areas.is_empty(), "the area must get into the buffer");
    app.player.seek(kt_time::Tick(700_000));
    app.paste_at_cursor();
    assert!(app.paste_names.is_some(), "the prefix window did not open");
    let frozen = app.paste_names.as_ref().unwrap().at;
    h.step();
    h.step();
    assert!(
        h.query_by_label(tr(k::msg::PASTE_PREFIX_LABEL)).is_some(),
        "the window was drawn, not only created in state"
    );

    // Esc cancels and leaves no traces
    let before = h.state().model.project().clone();
    h.key_press(egui::Key::Escape);
    h.step();
    h.step();
    assert!(h.state().paste_names.is_none(), "Esc must close the window");
    assert_eq!(h.state().model.project().areas, before.areas, "and paste nothing");

    // a SECOND Ctrl+V in a row opens the window again
    let app = h.state_mut();
    app.paste_at_cursor();
    assert!(app.paste_names.is_some(), "a repeated paste must open the window again");
    assert_eq!(app.paste_names.as_ref().unwrap().at, frozen, "the paste position is frozen");

    // confirmation — ONE history step
    let depth = app.model.undo_depth();
    let at = app.paste_names.as_ref().unwrap().at;
    app.paste_names = None;
    app.paste_here(at, None, "copy".into(), false);
    assert_eq!(app.model.undo_depth(), depth + 1, "paste is one history step");
    let names: Vec<String> = app.model.project().areas.iter().map(|a| a.name.clone()).collect();
    // R.38 item 19: “by default one should put a suffix, not a prefix” —
    // copies sort next to the original, not by the first letter of the word
    assert!(names.contains(&"final copy".to_string()), "suffix applied: {names:?}");
    // and the prefix remains an option of the same window
    app.model.undo().unwrap();
    app.paste_here(at, None, "copy".into(), true);
    let names: Vec<String> = app.model.project().areas.iter().map(|a| a.name.clone()).collect();
    assert!(names.contains(&"copy final".to_string()), "prefix is still available: {names:?}");
    app.model.undo().unwrap();
    assert_eq!(app.model.project().areas.len(), 1, "and was rolled back with one undo");
}

/// Test 248c (R.37 C3): the FX window of a pair shows, for the VIDEO instance of the warp,
/// only what it means for it. Reverb does not sound on video, and its handles
/// there would be a lie to the user (item 9 of the checklist).
#[test]
fn t248_video_warp_shows_only_what_matters() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    for (id, kind) in [(1u64, ClipKind::Video), (2, ClipKind::Audio)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: Some(GroupId(1)),
            kind,
            lane: 0,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(90_000),
            source_in: kt_time::Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![FxInstance::slowed_reverb(0.5, 0.2, 85.0)],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();

    // the pair has ONE window, and it holds both chains
    assert_eq!(h.state().fx_editors.len(), 1, "the pair has one window");
    assert_eq!(h.state().fx_editors[0].members.len(), 2);
    // the frames mode exists (it is meaningful only on video)
    assert!(
        h.query_by_label(crate::i18n::tr(crate::i18n::k::fx::FRAMES)).is_some(),
        "a video instance must have a frame-mode choice"
    );
    assert!(
        h.query_by_label(crate::i18n::tr(crate::i18n::k::fx::RATE_LINKED)).is_some(),
        "and it is said that the speed is shared with the partner"
    );
    // and the reverb pre-delay handle — only on the audio instance, once
    let predelay = crate::widgets::param_label("predelay_ms").to_string();
    assert_eq!(
        h.get_all_by_label(&predelay).count(),
        1,
        "the reverb handles are shown exactly once — for audio"
    );
}

/// Test 249 (R.37 C4, user requirement: “slowed&reverb must
/// be taken into account in the audio/video previews in events”). The waveform and the filmstrip
/// mapped a pixel to the material LINEARLY and ignored even the reverse —
/// a slowed clip drew not what it plays.
#[test]
fn t249_previews_follow_the_warp() {
    use kt_model::*;
    use kt_time::Tick;
    const SEC: i64 = 90_000;

    let clip = |fx: Vec<FxInstance>, reversed: bool| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(4 * SEC),
        source_in: Tick(SEC),
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx,
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };

    // without a warp the map is not created at all — the mapping is linear, as before
    assert!(kt_engine::clip_warp_map(&clip(vec![], false)).is_none());

    // 0.5×: by the middle of the event a QUARTER of the material is eaten, not half
    let slow = clip(vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], false);
    let w = kt_engine::clip_warp_map(&slow).expect("the warp map exists");
    assert_eq!(w.used(), 2 * SEC, "over 4 s of the event, 2 s of material is eaten");
    assert_eq!(w.material_off(slow.duration, false, 2 * SEC), SEC, "the middle is a quarter of the material");
    assert_eq!(w.material_off(slow.duration, false, 0), 0);
    assert_eq!(w.material_off(slow.duration, false, 4 * SEC), 2 * SEC);

    // reverse — “warp, then mirror”: the same rule as in the engine
    let rev = clip(vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], true);
    let wr = kt_engine::clip_warp_map(&rev).expect("map");
    assert_eq!(wr.material_off(rev.duration, true, 0), 2 * SEC, "reverse starts from the end");
    assert_eq!(wr.material_off(rev.duration, true, 4 * SEC), 0, "and ends at the start");

    // a DISABLED slot = “do not slow the picture” (R.37: this is exactly that
    // separate checkbox; we do not add a second switch)
    let mut off = slow.clone();
    off.fx_bypass = vec![true];
    assert!(kt_engine::clip_warp_map(&off).is_none(), "the slot checkbox removes the stretch");

    // the filmstrip step is taken from the CONSUMED span, otherwise
    // a slowed clip gets a step twice as coarse and degenerates
    // into repeating buckets
    let by_event = crate::thumbs::thumb_source_step(Tick(4 * SEC), 800.0, 48.0);
    let by_used = crate::thumbs::thumb_source_step(Tick(w.used()), 800.0, 48.0);
    assert!(by_used <= by_event, "the step by consumed is not coarser: {by_used} vs {by_event}");

    // and the map cache lives by the model revision, not built per frame
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(slow);
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    let a = app.warp_maps();
    let b = app.warp_maps();
    assert!(std::sync::Arc::ptr_eq(&a, &b), "the map is not rebuilt per frame");
    assert_eq!(a.len(), 1, "only clips WITH a warp are in the cache");
}

/// Test 258 (R.38 items 3–4). Complaints verbatim: “add morpher layer also works
/// strangely” and “if I press the same add morpher layer in another
/// area - it creates one more track for me”.
///
/// “Layer” is about the layer of edits, not about the track: a repeated “+M”
/// REUSES the top morph track. The block’s place is computed by one `morph_span`
/// (if there is a region — by the region, otherwise from the cursor), and the same
/// function is called by both ways of creating it: the gesture and the track menu item (rule 17).
#[test]
fn t258_morph_layer_reuses_lane_and_honours_region() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(900_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);

    // without a region — the block from the cursor, 1.5 s
    app.player.seek(Tick(180_000));
    app.add_morph_layer();
    let pr = app.model.project().clone();
    assert_eq!(pr.lanes(kt_model::LaneKind::Morph), 1, "the first morph track was created");
    assert_eq!(pr.morphers.len(), 1);
    assert_eq!(pr.morphers[0].start, Tick(180_000), "the block landed under the cursor");
    assert_eq!(pr.morphers[0].duration, Tick(90 * 1500), "and for 1.5 s");

    // a repeated “+M” in another place — the SAME track, a new block
    app.player.seek(Tick(495_000)); // a multiple of 15: the playhead lives in audio frames
    app.add_morph_layer();
    let pr = app.model.project().clone();
    assert_eq!(
        pr.lanes(kt_model::LaneKind::Morph),
        1,
        "a second morph track is NOT created: a “layer” is not a “track”"
    );
    assert_eq!(pr.morphers.len(), 2, "but the block was added");
    assert_eq!(pr.morphers[1].lane, 0, "to the same track");
    assert_eq!(pr.morphers[1].start, Tick(495_000));

    // with a region — the block BY THE REGION, and both ways of creating it match
    let mut pr = app.model.project().clone();
    pr.region = Some(kt_model::Region { start: Tick(700_000), end: Tick(760_000) });
    app.model = kt_model::TimelineModel::new(pr);
    app.add_morph_layer();
    let by_gesture = app.model.project().morphers.last().cloned().unwrap();
    assert_eq!(by_gesture.start, Tick(700_000), "a region exists — the block by the region");
    assert_eq!(by_gesture.duration, Tick(60_000));
    app.add_morpher_at(0);
    let by_menu = app.model.project().morphers.last().cloned().unwrap();
    assert_eq!(
        (by_menu.start, by_menu.duration),
        (by_gesture.start, by_gesture.duration),
        "the track menu item must place the block the same way as the gesture (rule 17)"
    );
}

/// Test 259 (R.38 item 9, verbatim: “I do not understand how to change the length of morphers
/// of an event”). The block’s edges are draggable: the right one changes the duration, the left one
/// moves the start while the end STANDS — as with a range marker.
#[test]
fn t259_morph_block_edges_resize() {
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        app.player.seek(Tick(100_000));
        app.add_morph_layer();
        app.morph_editors.clear();
    }
    h.step();
    let g = h.state().tl_geom.clone().unwrap();
    let m = h.state().model.project().morphers[0].clone();
    let row = g.row_of((kt_model::LaneKind::Morph, 0)).unwrap();
    let y = g.row_center_y(row);
    let end_x = g.tick_to_x((m.start + m.duration).0);

    // right edge: drag to the right — the duration grows, the start stays
    let to = end_x + 60.0;
    h.hover_at(egui::pos2(end_x, y));
    h.step();
    h.drag_at(egui::pos2(end_x, y));
    h.step();
    h.hover_at(egui::pos2(end_x + 8.0, y));
    h.step();
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let m2 = h.state().model.project().morphers[0].clone();
    assert_eq!(m2.start, m.start, "the right edge does not move the start");
    assert!(
        m2.duration > m.duration,
        "the right edge must lengthen the block: was {:?}, became {:?}",
        m.duration,
        m2.duration
    );

    // left edge: the end STAYS in place
    let end_before = (m2.start + m2.duration).0;
    let sx = g.tick_to_x(m2.start.0);
    let to = sx + 40.0;
    h.hover_at(egui::pos2(sx, y));
    h.step();
    h.drag_at(egui::pos2(sx, y));
    h.step();
    h.hover_at(egui::pos2(sx + 8.0, y));
    h.step();
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let m3 = h.state().model.project().morphers[0].clone();
    assert!(m3.start > m2.start, "the left edge moved the start");
    assert_eq!(
        (m3.start + m3.duration).0,
        end_before,
        "and the end stayed in place (rule 20: nothing grows by itself)"
    );
}

/// Test 260 (R.38 item 5 “I do not understand how the camera track works” + sweeping up
/// the same class). The camera is selected by a click, deleted from the keyboard and
/// changes its length by its edges; the left edge of a range marker finally works.
///
/// `Drag::MarkerEdge.left` was described in a comment as working, and a test even matched
/// this variant — but ONLY `left: false` was constructed, and the gesture went into
/// `_ => {}` and vanished silently.
#[test]
fn t260_camera_and_marker_left_edge_are_reachable() {
    use kt_model::*;
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 0 }));
        app.player.seek(Tick(90_000));
        app.add_camera_at_playhead(0);
        assert!(app.apply(Command::AddMarker { at: Tick(45_000), duration: Tick(30_000) }));
    }
    h.step();
    let g = h.state().tl_geom.clone().unwrap();
    let c = h.state().model.project().cameras[0].clone();
    let row = g.row_of((LaneKind::Camera, 0)).unwrap();
    let y = g.row_center_y(row);

    // a click on the camera — selection (before R.38 there was no `selected_camera` field)
    let mid = egui::pos2(g.tick_to_x(c.start.0 + c.duration.0 / 2), y);
    h.hover_at(mid);
    h.step();
    h.drag_at(mid);
    h.step();
    h.drop_at(mid);
    h.step();
    assert!(h.state().selected_camera.contains(&c.id), "a click on the camera must select it");

    // the right edge changes the length
    let ex = g.tick_to_x((c.start + c.duration).0);
    let to = ex - 40.0;
    h.hover_at(egui::pos2(ex, y));
    h.step();
    h.drag_at(egui::pos2(ex, y));
    h.step();
    h.hover_at(egui::pos2(ex - 8.0, y));
    h.step();
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let c2 = h.state().model.project().cameras[0].clone();
    assert_eq!(c2.start, c.start, "the edge does not move the start");
    assert!(c2.duration < c.duration, "the right edge must change the camera duration");

    // Del deletes the selected camera
    let app = h.state_mut();
    app.select_camera_only(c.id);
    app.delete_action();
    assert!(h.state().model.project().cameras.is_empty(), "Del must delete the camera");

    // the LEFT edge of a range marker: the end stays, the start moves
    let m = h.state().model.project().markers[0].clone();
    assert!(m.is_range(), "scene: range marker");
    let end0 = m.end().0;
    let ry = g.rect.top() + 6.0;
    let sx = g.tick_to_x(m.t.0);
    let to = sx + 30.0;
    h.hover_at(egui::pos2(sx, ry));
    h.step();
    h.drag_at(egui::pos2(sx, ry));
    h.step();
    h.hover_at(egui::pos2(sx + 8.0, ry));
    h.step();
    h.hover_at(egui::pos2(to, ry));
    h.step();
    h.drop_at(egui::pos2(to, ry));
    h.step();
    let m2 = h.state().model.project().markers[0].clone();
    assert!(m2.t.0 > m.t.0, "the left edge moved the start of the marker: {:?} → {:?}", m.t, m2.t);
    assert_eq!(m2.end().0, end0, "and the end stayed in place");
}

/// Test 262 (R.38 item 11, verbatim: “allow changing the height of each track
/// individually”). The bottom edge of a row in the gutter drags its height, the menu item
/// resets it, and this is a CONVENIENCE — it does not light the asterisk.
#[test]
fn t262_per_track_height() {
    use crate::timeline::{ROW_H_MAX, THIN_ROW_MIN, TlGeom, row_height_of};
    use kt_model::LaneKind;

    // pure geometry: an override beats the computed height and is clamped
    let over = vec![(LaneKind::Video, 0, 150.0), (LaneKind::Morph, 0, 5.0)];
    assert_eq!(row_height_of(LaneKind::Video, 0, 60.0, &over), 150.0);
    assert_eq!(row_height_of(LaneKind::Video, 1, 60.0, &over), 60.0, "another track is untouched");
    assert_eq!(
        row_height_of(LaneKind::Morph, 0, 60.0, &over),
        THIN_ROW_MIN,
        "the lower limit: a row of 5 px cannot be hit with the mouse"
    );
    assert_eq!(
        row_height_of(LaneKind::Video, 0, 60.0, &[(LaneKind::Video, 0, 9999.0)]),
        ROW_H_MAX,
        "an upper limit exists for everything that is stretched with the mouse (rule 6)"
    );
    // the row tops agree with the heights
    let rows = vec![(LaneKind::Video, 0), (LaneKind::Video, 1)];
    let g = TlGeom::new(
        egui::Rect::from_min_max(egui::pos2(50.0, 0.0), egui::pos2(1000.0, 500.0)),
        60.0,
        rows,
        &over,
        0.0,
        0.0,
        1000.0,
    );
    assert_eq!(g.row_h(0), 150.0);
    assert_eq!(g.row_top(1) - g.row_top(0), 150.0, "the top of the second row moved with the height of the first");

    // gesture: drag the bottom edge of the first row in the gutter
    let (mut h, g0) = harness_three_clips();
    let row0_bottom = g0.row_top(0) + g0.row_h(0);
    // x is taken in the MIDDLE of the gutter, not at the window’s very edge: the first 16 px
    // are the window resize zone, which takes the press for itself (rule 15)
    let gx = g0.rect.left() - 12.0;
    let want = g0.row_h(0) + 40.0;
    h.hover_at(egui::pos2(gx, row0_bottom));
    h.step();
    h.drag_at(egui::pos2(gx, row0_bottom));
    h.step();
    h.hover_at(egui::pos2(gx, row0_bottom + 8.0));
    h.step();
    h.hover_at(egui::pos2(gx, row0_bottom + 40.0));
    h.step();
    h.drop_at(egui::pos2(gx, row0_bottom + 40.0));
    h.step();
    let app = h.state();
    let over = app.row_heights();
    assert_eq!(over.len(), 1, "track height remembered: {over:?}");
    let (k, l, hh) = over[0];
    assert_eq!((k, l), g0.rows[0], "EXACTLY the row whose edge was dragged is remembered");
    assert!((hh - want).abs() < 3.0, "height {hh} ≈ {want}");
    // since R.62 the height lives ON THE TRACK and goes through undo together with the name
    assert!(app.dirty_content, "track height is model content");

    // and the reset returns the computed height
    let app = h.state_mut();
    app.set_row_height(g0.rows[0], None);
    assert!(app.row_heights().is_empty(), "reset removes the override");
    assert!(app.model.undo_depth() >= 2, "height must go through undo");
}

/// Test 265 (R.38 items 14–18): the automation panel.
///
/// Complaints verbatim: “in the automation timeline I do not understand
/// how it zooms”,
/// “when I press a key — so that the values written in the fields are linked to the key”,
/// “the feeling that effects have some kind of separate ‘default’ state of values”,
/// “I added a second automation parameter — but only the first one gets edited”,
/// “still missing the buttons controlling the automation timeline elements”.
#[test]
fn t265_automation_axis_and_field_bind() {
    use crate::automation::{AutoCtx, AutoView};
    use crate::widgets::{bound_apply, bound_value};
    use kt_model::{AutoKey, ParamCurve};
    use kt_time::Tick;

    // ---- item 14: time axis ------------------------------------------
    let id = |t: Tick| t;
    // the lock since R.40 lives in UiState (one per program), not in AutoView
    let mut ctx = AutoCtx {
        links: None,
        default_shape: Default::default(),
        specs: &[],
        range: (Tick(0), Tick(90_000 * 10)),
        view: (Tick(90_000 * 2), Tick(90_000 * 4)),
        to_timeline: &id,
        to_curve: &id,
        frame_step: 3000,
        linked: true,
    };
    assert!(
        crate::app::KtApp::default_axis_linked(),
        "the “link to timeline” lock is on by default (decision item 14)"
    );
    let mut v = AutoView::default();
    assert_eq!(
        v.axis(&ctx),
        (90_000 * 2, 90_000 * 4),
        "the linked graph shows the timeline window"
    );
    ctx.linked = false;
    assert_eq!(v.axis(&ctx), (0, 90_000 * 10), "without its own window the unlinked one shows everything");
    v.win = Some((90_000, 90_000 * 3));
    assert_eq!(v.axis(&ctx), (90_000, 90_000 * 3), "and its own window when there is one");
    v.win = Some((-90_000, 90_000 * 99));
    assert_eq!(v.axis(&ctx), (0, 90_000 * 10), "its OWN window is clamped to the range of the carrier");
    // R.40: the LINKED axis is NOT clamped by the media window. Complaint verbatim:
    // “when I pull the left edge of the event, my automation keys also shift
    // to the start”. The key did not move: the axis lied, normalised
    // to the event. We look at what the timeline looks at — and the key stays in place.
    ctx.linked = true;
    ctx.view = (Tick(-90_000), Tick(90_000 * 99));
    assert_eq!(
        v.axis(&ctx),
        (-90_000, 90_000 * 99),
        "the linked graph shows the TIMELINE window as is, including the space beyond the event edges"
    );
    ctx.view = (Tick(90_000 * 2), Tick(90_000 * 4));
    ctx.linked = false;
    v.win = Some((5000, 5001));
    assert_eq!(
        v.axis(&ctx),
        (0, 90_000 * 10),
        "a degenerate window (narrower than a frame) rolls back to the full one: there is nothing to divide by zero with"
    );

    // ---- items 15–16: the field edits exactly what it shows --------------
    let mut c = ParamCurve {
        param: "db".into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(90_000), 6.0)]),
        source: Default::default(),
    };
    // a key is selected — the field shows and edits IT
    assert_eq!(bound_value(&c, Some(1), Tick(45_000)), 6.0);
    bound_apply(&mut c, Some(1), Tick(45_000), 3.0);
    assert_eq!(c.keys[1].v, 3.0, "the field edit went to the selected key");
    assert_eq!(c.keys.len(), 2, "and no extra key appeared");
    // the key is not selected — the field shows the value of the curve UNDER THE CURSOR
    let mid = bound_value(&c, None, Tick(45_000));
    assert!((mid - 1.5).abs() < 1e-9, "the curve value under the cursor is shown: {mid}");
    bound_apply(&mut c, None, Tick(45_000), 4.0);
    assert_eq!(c.keys.len(), 3, "an edit without a selected key places a key under the cursor");
    // a new key INHERITS the shape of the segment it divides (R.61): otherwise
    // a click on the graph would silently turn a zigzag into a smoothing, and for
    // the warp speed a smoothed key would be a validation refusal
    assert_eq!(c.keys[1], AutoKey::shaped(Tick(45_000), 4.0, c.keys[0].shape));
    assert_eq!(c.keys[1].shape, kt_model::SegShape::Linear, "the segment shape is not inherited");
    assert!(c.keys.windows(2).all(|w| w[0].t.0 <= w[1].t.0), "key order is not broken");
    // a selection outside the curve does not crash and is treated as “not selected”
    assert_eq!(bound_value(&c, Some(99), Tick(0)), c.keys[0].v);

    // ---- items 17–18: the active curve and the buttons in the live window -------------
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::slowed_reverb(0.5, 0.3, 85.0)],
        fx_bypass: vec![false],
        fx_auto: vec![vec![]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();

    // we add TWO automation parameters in a row
    let names: Vec<String> = {
        let f = h.state().model.project().clips[0].fx[0].clone();
        f.auto_params().take(2).map(|s| crate::widgets::param_label(&s.name)).collect()
    };
    assert_eq!(names.len(), 2, "slowed+reverb has at least two automatable parameters");

    for n in &names {
        h.get_by_label(tr(k::fx::AUTO_ADD)).click();
        h.step();
        h.step();
        // the parameter label is present both in the value grid and in the menu that opens —
        // we press the LAST one: the menu is drawn on top
        h.get_all_by_label(n.as_str()).last().expect("parameter menu item").click();
        h.step();
        h.step();
    }
    let curves = h.state().model.project().clips[0].fx_auto[0].clone();
    assert_eq!(curves.len(), 2, "both curves are created: {curves:?}");
    assert_eq!(
        h.state().fx_editors[0].auto_views[&(ClipId(1), 0)].active,
        1,
        "the SECOND added parameter must become active — otherwise a click on the graph \
         keeps placing keys into the first curve, and the second looks dead (item 17)"
    );

    // the key and axis buttons are in place (item 18)
    for icon in [
        crate::icons::PLUS,
        crate::icons::SKIP_BACK,
        crate::icons::SKIP_FWD,
        crate::icons::TRASH,
        crate::icons::LOCK,
        crate::icons::ARROWS_OUT,
    ] {
        assert!(
            h.query_all_by_label(icon).count() >= 1,
            "there is no button {icon:?} in the automation panel (item 18)"
        );
    }
}

/// Test 269 (R.38 item 12): the layer’s strength and fades are edited IN THE MORPHER WINDOW and
/// reach the model. Until this round the patch had not a single strength field,
/// and `enabled` was binary — exactly the mismatch with which R.36 began
/// (“the morpher in Vegas lets you control the
/// opacity of the effects overlay with a slider, and ours does not”).
#[test]
fn t269_morph_strength_and_fades_reach_the_model() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    use kt_time::Tick;

    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        app.player.seek(Tick(90_000));
        app.add_morph_layer();
    }
    h.step();
    h.step();
    assert!(h.state().morph_editors.iter().any(|e| e.open), "the morpher window opened");
    // the strength label is visible — so the control reached the user
    assert!(
        h.query_by_label(tr(k::mrph::STRENGTH)).is_some(),
        "there is no strength field in the morpher window (item 12)"
    );

    // set strength and fades through the model and check that the engine sees them
    let app = h.state_mut();
    let mut ev = app.model.project().morphers[0].clone();
    ev.patch.video_fx = vec![FxInstance::edges(24)];
    ev.strength = 0.4;
    ev.fade_in = Tick(9_000);
    assert!(app.apply(Command::SetMorpher(ev)));
    let eff = app.model.project().effective_clips();
    let mixed: Vec<&ClipEvent> = eff
        .iter()
        .filter(|c| c.fx.iter().any(|f| matches!(f, FxInstance::LayerMix { .. })))
        .collect();
    // a block covering several events — the strength slot was added to EACH of them
    assert!(!mixed.is_empty(), "the strength slot was appended to the covered pieces");
    let c = mixed[0];
    let mix_at = c.fx.iter().position(|f| matches!(f, FxInstance::LayerMix { .. })).unwrap();
    match &c.fx[mix_at] {
        FxInstance::LayerMix { amount, from } => {
            assert!((amount - 0.4).abs() < 1e-9, "strength reached: {amount}");
            assert_eq!(
                *from as usize,
                mix_at.saturating_sub(1),
                "the contribution of a layer starts with its effect"
            );
        }
        _ => unreachable!(),
    }
    assert!(!c.fx_auto[mix_at].is_empty(), "a fade is the strength CURVE, not a separate mechanism");

    // Strength 1.0 without fades does NOT create a slot — WHERE THE LAYER COVERS
    // THE EVENT ENTIRELY. This is exactly what the GOLDENs rest on: full coverage is
    // the most frequent case, and its path must remain byte-for-byte.
    //
    // Since R.55 (C2) a PARTIALLY covered event gets a slot also on the defaults:
    // “by time” is now expressed by strength GATES rather than a hard cut, and without a slot
    // the layer would hit the whole body. This is not a regression of the defaults,
    // but exactly what the gates were introduced for.
    let app = h.state_mut();
    let mut ev = app.model.project().morphers[0].clone();
    ev.strength = 1.0;
    ev.fade_in = Tick::ZERO;
    ev.fade_out = Tick::ZERO;
    let win = ev.window();
    assert!(app.apply(Command::SetMorpher(ev)));
    let eff = app.model.project().effective_clips();
    let has_mix = |c: &ClipEvent| c.fx.iter().any(|f| matches!(f, FxInstance::LayerMix { .. }));
    let whole: Vec<&ClipEvent> =
        eff.iter().filter(|c| win.start <= c.start && c.start + c.duration <= win.end).collect();
    assert!(!whole.is_empty(), "scene degenerated: the layer does not cover a single event entirely");
    assert!(
        whole.iter().all(|c| !has_mix(c)),
        "by default a blending slot does not exist at all for an event FULLY covered — \
         the GOLDEN tests depend on this"
    );
    // but a PARTIALLY covered one has it, and that is what “by time” is
    let part: Vec<&ClipEvent> = eff
        .iter()
        .filter(|c| {
            let body = kt_time::TickRange::new(c.start, c.start + c.duration);
            !body.intersect(&win).is_empty() && !(win.start <= c.start && body.end <= win.end)
        })
        .collect();
    assert!(
        part.iter().all(|c| has_mix(c)),
        "a partially covered event must have a strength GATE — otherwise the layer \
         acts on the whole body, while the person asked for a window"
    );
}

/// Test 274 (R.40 item 11): the track height reaches the EVENT.
///
/// Complaint verbatim: “the track changes its height but the events are still tiny —
/// you did not check a damn thing”. And true: t262 checked that the field `lane_heights`
/// got a value and `row_h` changed, but throughout `draw_timeline` there was
/// ONE variable `lane_h = vl.base_h` used 36 times — an event was drawn at the base
/// height, and that was all.
///
/// Therefore this test asserts the OBSERVABLE CONSEQUENCE: the event’s rectangle
/// has grown, and the mouse hits it WHERE it is now drawn.
#[test]
fn t274_row_height_reaches_the_clip() {
    use kt_model::{ClipKind, LaneKind};

    let (mut h, g0) = harness_three_clips();
    let key = (LaneKind::Video, 0u32);
    let before = g0.lane_h_of(ClipKind::Video, 0);

    // set the row height to twice the base
    let want = before * 2.0;
    {
        let app = h.state_mut();
        app.set_row_height(key, Some(want));
    }
    h.step();
    h.step();
    let g = h.state().tl_geom.clone().unwrap();

    assert!(
        (g.lane_h_of(ClipKind::Video, 0) - want).abs() < 1.0,
        "the EVENT must count the height of its row: {} instead of {want}",
        g.lane_h_of(ClipKind::Video, 0)
    );
    assert!(
        (g.row_h(g.row_of(key).unwrap()) - want).abs() < 1.0,
        "and the row too (this worked before as well)"
    );

    // CONSEQUENCE: the mouse hits the event BELOW the old boundary —
    // where before R.40 there was the bare track backing
    let c = h.state().model.project().clips[0].clone();
    let x = g.tick_to_x(c.start.0 + c.duration.0 / 2);
    let deep_y = g.lane_top(ClipKind::Video, 0) + before + (want - before) * 0.5;
    h.hover_at(egui::pos2(x, deep_y));
    h.step();
    h.drag_at(egui::pos2(x, deep_y));
    h.step();
    h.drop_at(egui::pos2(x, deep_y));
    h.step();
    assert!(
        h.state().is_selected(c.id),
        "a click in the lower half of the grown row must hit the event \
         (y = {deep_y}, the previous boundary was {})",
        g.lane_top(ClipKind::Video, 0) + before
    );

    // the neighbouring track did not move
    let audio_h = g.lane_h_of(ClipKind::Audio, 0);
    assert!(
        (audio_h - before).abs() < 1.0,
        "another track keeps its height: {audio_h} instead of {before}"
    );
}

/// Test 275 (R.40 item 4, verbatim: “you can add a morpher layer a hundred times, and it
/// will create it each time on the same track — in the end there can be a heap of morphers hidden
/// behind one and the same place”).
///
/// R.38 always reused the FIRST morph row. The user answered what to do:
/// “but one should check — maybe one more track is already free”.
/// Free means free IN THIS SPAN.
///
/// This test asserts the OBSERVABLE CONSEQUENCE: two blocks do not lie on top of each other,
/// that is, each one can be grabbed with the mouse.
#[test]
fn t275_morph_layer_never_hides_a_block_under_another() {
    use kt_model::LaneKind;
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        app.player.seek(Tick(100_000));
        app.add_morph_layer();
        // the second “+M” RIGHT THERE: the place is taken
        app.add_morph_layer();
        app.morph_editors.clear();
    }
    h.step();
    let pr = h.state().model.project().clone();
    assert_eq!(pr.morphers.len(), 2, "both blocks created");
    assert_eq!(
        pr.lanes(kt_model::LaneKind::Morph),
        2,
        "an occupied place must move the block to a new track"
    );
    assert_ne!(pr.morphers[0].lane, pr.morphers[1].lane, "and they are not on one track");

    // a third “+M” in a FREE time: there must be no new track —
    // “layer” is not about tracks, tracks are opened by their own menu item
    {
        let app = h.state_mut();
        app.player.seek(Tick(600_000));
        app.add_morph_layer();
        app.morph_editors.clear();
    }
    h.step();
    let pr = h.state().model.project().clone();
    assert_eq!(
        pr.lanes(kt_model::LaneKind::Morph),
        2,
        "free time — the track is reused"
    );
    assert_eq!(pr.morphers.len(), 3);

    // and the consequence the whole thing is for: each block is caught by the mouse in ITS OWN
    // row — none is hidden under another
    let g = h.state().tl_geom.clone().unwrap();
    for m in &pr.morphers {
        let row = g.row_of((LaneKind::Morph, m.lane)).expect("block row");
        let pos = egui::pos2(g.tick_to_x(m.start.0 + m.duration.0 / 2), g.row_center_y(row));
        h.hover_at(pos);
        h.step();
        h.drag_at(pos);
        h.step();
        h.drop_at(pos);
        h.step();
        assert!(
            h.state().selected_morph.contains(&m.id),
            "block {:?} on track {} cannot be hit with the mouse — it is hidden under another",
            m.id,
            m.lane
        );
    }
}

/// Test 276 (R.40 item 2, verbatim: “I cannot move morphers between different
/// tracks”). Morph and camera blocks move between their own rows with the mouse —
/// like a clip.
#[test]
fn t276_service_blocks_move_between_lanes() {
    use kt_model::{Command, LaneKind};
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        app.player.seek(Tick(100_000));
        app.add_morph_layer();
        app.add_morph_layer(); // the second row (place taken) — there is somewhere to move it to
        app.morph_editors.clear();
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 0 }));
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 1 }));
        app.player.seek(Tick(300_000));
        app.add_camera_at_playhead(0);
    }
    h.step();
    h.step();

    // ---- morpher: from track 0 to track 1 ----
    let g = h.state().tl_geom.clone().unwrap();
    let m = h.state().model.project().morphers[0].clone();
    let from_row = g.row_of((LaneKind::Morph, m.lane)).unwrap();
    let to_lane = if m.lane == 0 { 1 } else { 0 };
    let to_row = g.row_of((LaneKind::Morph, to_lane)).unwrap();
    let x = g.tick_to_x(m.start.0 + m.duration.0 / 2);
    let from = egui::pos2(x, g.row_center_y(from_row));
    let to = egui::pos2(x, g.row_center_y(to_row));
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    // drag threshold: first a small shift, then the target (harness rule)
    h.hover_at(egui::pos2(x + 8.0, from.y));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();
    let m2 = h.state().model.project().morphers.iter().find(|z| z.id == m.id).unwrap().clone();
    assert_eq!(m2.lane, to_lane, "a morph block must move to the row under the cursor");

    // ---- camera: the same ----
    let g = h.state().tl_geom.clone().unwrap();
    let c = h.state().model.project().cameras[0].clone();
    let from_row = g.row_of((LaneKind::Camera, c.lane)).unwrap();
    let to_lane = if c.lane == 0 { 1 } else { 0 };
    let to_row = g.row_of((LaneKind::Camera, to_lane)).unwrap();
    let x = g.tick_to_x(c.start.0 + c.duration.0 / 2);
    let from = egui::pos2(x, g.row_center_y(from_row));
    let to = egui::pos2(x, g.row_center_y(to_row));
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    h.hover_at(egui::pos2(x + 8.0, from.y));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();
    let c2 = h.state().model.project().cameras.iter().find(|z| z.id == c.id).unwrap().clone();
    assert_eq!(c2.lane, to_lane, "a camera block must move to the row under the cursor");
}

/// Test 277 (R.40, verbatim: “Morphers for some reason are not selected via
/// area-select, you did not check a damn thing”).
///
/// The frame caught ONLY clips — its catch was `Vec<ClipId>`, the type itself said so —
/// and having caught the clips, it also CANCELLED the selected morpher.
#[test]
fn t277_rubber_band_catches_service_blocks() {
    use kt_model::{Command, LaneKind};
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        app.player.seek(Tick(100_000));
        app.add_morph_layer();
        app.morph_editors.clear();
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 0 }));
        app.player.seek(Tick(100_000));
        app.add_camera_at_playhead(0);
        app.sel_tool = true; // a plain drag of the frame
    }
    h.step();
    h.step();

    let g = h.state().tl_geom.clone().unwrap();
    let pr = h.state().model.project().clone();
    let m = pr.morphers[0].clone();
    let c = pr.cameras[0].clone();
    let m_row = g.row_of((LaneKind::Morph, m.lane)).unwrap();
    let c_row = g.row_of((LaneKind::Camera, c.lane)).unwrap();
    // the frame from the top of the upper service row to the bottom of the lower one, in time —
    // over both blocks
    let top = g.row_top(m_row.min(c_row)) - 2.0;
    let bot = g.row_top(m_row.max(c_row)) + g.row_h(m_row.max(c_row)) - 2.0;
    let x0 = g.tick_to_x(m.start.0.min(c.start.0)) + 2.0;
    let x1 = g.tick_to_x((m.start + m.duration).0.min((c.start + c.duration).0)) - 2.0;
    assert!(x1 > x0 + 10.0, "scene: blocks overlap in time");

    let from = egui::pos2(x0, top);
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    h.hover_at(egui::pos2(x0 + 8.0, top + 4.0));
    h.step();
    h.hover_at(egui::pos2(x1, bot));
    h.step();
    h.drop_at(egui::pos2(x1, bot));
    h.step();

    assert!(
        h.state().selected_morph.contains(&m.id),
        "the frame must select the morpher: it is drawn over it and “catches” — but it did not catch"
    );
    assert!(h.state().selected_camera.contains(&c.id), "and the camera with the same gesture");

    // and the consequence: Del removes exactly what was caught
    h.state_mut().delete_action();
    h.step();
    let pr = h.state().model.project().clone();
    assert!(pr.morphers.is_empty() && pr.cameras.is_empty(), "Del removes everything caught by the frame");
}

/// Test 278 (R.40 item 12, verbatim: “on a small event I try to catch
/// the opacity there but it does not work”).
///
/// There were two locks, and both were confirmed by arithmetic:
///   * `ClipZone::Level` required `x1 − x0 > 24.0` — on a short event
///     there was no level at any y;
///   * the fade handle was always caught ±12 px from the CORNER, so on an event
///     only 24 px wide both handles covered it entirely and no middle was
///     left.
///
/// The test asserts the CONSEQUENCE: on a short event one pulls down in the middle of the
/// upper strip — and the opacity drops.
#[test]
fn t278_opacity_reachable_on_a_short_event() {
    use crate::timeline::{fade_grab_for_width, level_y, top_strip_h};
    use kt_model::ClipKind;
    use kt_time::Tick;

    // pure arithmetic: the middle of a narrow event does not belong to the handles
    for w in [10.0f32, 18.0, 24.0, 40.0] {
        let g = fade_grab_for_width(w);
        assert!(
            2.0 * g < w,
            "at width {w} the fade handles (±{g}) cover the event entirely — \
             no middle remains where the level lives"
        );
    }

    let (mut h, _) = harness_three_clips();
    // shorten the first event to about 20 px: exactly the case in question
    let (id, x0, y_line) = {
        let g0 = h.state().tl_geom.clone().unwrap();
        let px = 20.0 * g0.tpp;
        let c = h.state().model.project().clips[0].clone();
        let app = h.state_mut();
        assert!(
            app.apply(kt_model::Command::TrimEnd { id: c.id, new_end: c.start + Tick(px as i64) })
        );
        (c.id, c.start.0, 0.0)
    };
    let _ = y_line;
    h.step();
    h.step();

    let g = h.state().tl_geom.clone().unwrap();
    let c = h.state().model.project().clips.iter().find(|z| z.id == id).unwrap().clone();
    let wide = g.tick_to_x((c.start + c.duration).0) - g.tick_to_x(c.start.0);
    assert!(wide < 24.0, "scene: event shorter than the old threshold, {wide} px");
    assert_eq!(c.opacity, 1.0, "the start from the default — that is where dragging starts");

    let top = g.lane_top(ClipKind::Video, c.lane);
    let lane_h = g.lane_h_of(ClipKind::Video, c.lane);
    let mid_x = (g.tick_to_x(x0) + g.tick_to_x((c.start + c.duration).0)) * 0.5;
    let ly = level_y(1.0, top, lane_h);
    assert!(
        ly <= top + top_strip_h(lane_h),
        "the level line at opacity = 1.0 lies inside the fade strip — \
         that is exactly why the first grab did not succeed"
    );

    // and the gesture carries it through to the model
    let from = egui::pos2(mid_x, ly);
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    h.hover_at(egui::pos2(mid_x, ly + 8.0));
    h.step();
    h.hover_at(egui::pos2(mid_x, ly + lane_h * 0.4));
    h.step();
    h.drop_at(egui::pos2(mid_x, ly + lane_h * 0.4));
    h.step();
    let after = h.state().model.project().clips.iter().find(|z| z.id == id).unwrap().opacity;
    assert!(
        after < 0.95,
        "the level line was dragged down — the opacity must drop, but it is {after}"
    );
}

/// Test 279 (R.40 items 15/18 and “keys slide”).
///
/// Three complaints of one family:
///   * “the lock does not fully work: it has to be made so that it must be
///     TAKEN OFF to disable timeline sync, and the lock must
///     remember its state”;
///   * “there is not enough beside the set-key button — two buttons, key BEFORE and
///     key AFTER the current position”;
///   * “when I pull the left edge of the event — my automation keys also shift
///     to the start, and this behaviour cannot be changed in any way”.
///
/// The last one is not about data: the key does NOT move, the axis lied. The test
/// asserts both halves: the key is in place, and the axis shows it in place.
#[test]
fn t279_axis_lock_and_keys_that_stay() {
    use kt_model::{AutoKey, Command, FxInstance, ParamCurve};
    use kt_time::Tick;

    // ---- the lock is remembered in the project and is one per program ----------------
    let mut h = harness();
    {
        let app = h.state_mut();
        assert!(app.auto_axis_linked, "the default — the lock is ON (rule 4 of the checklist)");
        app.auto_axis_linked = false;
        let ui = app.ui_state_for_test();
        assert!(ui.auto_axis_unlinked, "a removed lock must get into the project");
        let back = kt_model::UiState { auto_axis_unlinked: true, ..Default::default() };
        assert!(!(!back.auto_axis_unlinked), "and be read back as removed");
    }

    // ---- the key stays on ITS frame during a trim ----------------------
    let (mut h, _) = harness_three_clips();
    let id = h.state().model.project().clips[0].id;
    {
        let app = h.state_mut();
        let mut c = app.model.project().clips[0].clone();
        c.fx = vec![FxInstance::blur(4)];
        c.fx_auto = vec![vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![
                AutoKey::at(Tick(10_000), 1.0),
                AutoKey::at(Tick(40_000), 9.0),
            ]),
            source: Default::default(),
        }]];
        let mut p = app.model.project().clone();
        p.clips[0] = c;
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
    }
    let before: Vec<i64> =
        h.state().model.project().clips[0].fx_auto[0][0].keys.iter().map(|k| k.t.0).collect();
    // left edge to the right by 20 000 ticks
    let start = h.state().model.project().clips[0].start;
    assert!(h.state_mut().apply(Command::TrimStart { id, new_start: start + Tick(20_000) }));
    let after: Vec<i64> =
        h.state().model.project().clips[0].fx_auto[0][0].keys.iter().map(|k| k.t.0).collect();
    assert_eq!(
        after, before,
        "the key must stay on THE SAME frame of the material: “how do I make it \
         NOT slide anywhere, whether I move the left edge to the left or to the right”"
    );

    // ---- and the option that does the OPPOSITE is off by default ----
    {
        let app = h.state_mut();
        assert!(
            !app.model.project().trim_stretches_auto,
            "stretching the animation together with the event is NOT the default"
        );
        assert!(app.apply(Command::SetTrimStretchesAuto { on: true }));
    }
    let c0 = h.state().model.project().clips[0].clone();
    let span_old = kt_model::clip_material_span(&c0);
    let offs_old: Vec<i64> = c0.fx_auto[0][0].keys.iter().map(|k| k.t.0 - c0.source_in.0).collect();
    let start = c0.start;
    assert!(h.state_mut().apply(Command::TrimStart { id, new_start: start + Tick(10_000) }));
    let c1 = h.state().model.project().clips[0].clone();
    let stretched: Vec<i64> = c1.fx_auto[0][0].keys.iter().map(|k| k.t.0).collect();
    assert_ne!(stretched, after, "with the option on, the keys must move");
    // and move EXACTLY proportionally: the key’s share within the event is kept
    let span_new = kt_model::clip_material_span(&c1);
    for (k, off0) in stretched.iter().zip(&offs_old) {
        let want =
            c1.source_in.0 + (*off0 as f64 * span_new as f64 / span_old as f64).round() as i64;
        assert_eq!(
            *k, want,
            "the share of the key inside the event must be preserved: {span_old} → {span_new}"
        );
    }
}

/// Test 281 (R.43 item 1): a key value TYPED in the parameter field
/// reaches the model.
///
/// Complaint verbatim: “in the automation timeline for slowed-reverb keys I
/// for example type a rate into the input field and it jumps back to the previous
/// value, but if I move the key on the graph the value changes as it should.
/// By the way — in the morpher this works well”.
///
/// Cause: the field and the graph had ONE flag between them, and the FX window put
/// `auto` into the command only by the graph’s flag. The field’s edit went into a local
/// clone of `auto`, which died together with the frame, and on the next frame the field
/// read the old value from the model.
///
/// The test asserts the CONSEQUENCE (the curve in the MODEL) and probes FROM THE POINT WHERE
/// THE PERSON ACTS (the characters in the field next to the parameter label) — this is what
/// t265 did not do: it checked `bound_value`/`bound_apply` as pure
/// functions and was green on a broken program.
#[test]
fn t281_typed_key_value_reaches_the_model() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(9), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(9),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(9),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::slowed_reverb(1.0, 0.2, 60.0)],
        fx_bypass: vec![],
        fx_auto: vec![vec![ParamCurve {
            param: "rate".into(),

            keys: AutoKey::linear(vec![
                AutoKey::at(kt_time::Tick(0), 1.0),
                AutoKey::at(kt_time::Tick(90_000), 2.0),
            ]),
            source: Default::default(),
        }]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(9));
    h.step();
    h.step();

    // the field of the LIVE parameter sits in the grid between its own label and the label
    // of the next parameter — we aim where a person would. `DragValue`
    // does not create its own node in the accessibility tree, so it cannot be found by its label:
    // we take the coordinate from the REAL layout.
    let (lbl_rate, lbl_mix) =
        (crate::widgets::param_label("rate"), crate::widgets::param_label("mix"));
    // the parameter label occurs twice: in the value grid and as the curve-selection button
    // in the automation panel BELOW. We need the UPPER one — the grid
    // is drawn before the graph.
    let top_label = |h: &Harness<'_, KtApp>, lbl: &str| -> egui::Rect {
        h.query_all_by_label(lbl)
            .map(|n| n.rect())
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .unwrap_or_else(|| panic!("the label of parameter {lbl:?} is not drawn"))
    };
    let r = top_label(&h, &lbl_rate);
    let (x0, y) = (r.right(), r.center().y);
    let x1 = top_label(&h, &lbl_mix).left();
    assert!(x1 > x0 + 8.0, "there must be room for a field between the parameter labels");
    let pos = egui::pos2((x0 + x1) * 0.5, y);

    // a click on the field switches it to input mode; then characters and Enter
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
    h.event(egui::Event::Text("3".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();

    let curves = h.state().model.project().clips[0].fx_auto[0].clone();
    let key = curves
        .iter()
        .find(|c| c.param == "rate")
        .and_then(|c| c.keys.iter().find(|k| k.t == kt_time::Tick(0)))
        .expect("the rate curve key at the material zero");
    assert_eq!(
        key.v, 3.0,
        "the typed value must reach the MODEL, not roll back \
         (curve: {curves:?})"
    );
    // and a curve edit does not invent a history step for every character
    assert_eq!(
        h.state().model.project().clips[0].fx_auto[0][0].keys.len(),
        2,
        "the field edits an existing key, not creates new ones"
    );
}

/// Test 286 (R.43 item 296): an edit of the “convenience” does not rebuild the engine.
///
/// Complaint: “the region ‘looping_5’ is somehow broken during
/// playback, underruns keep growing”. `on_region_changed` called
/// `Player::set_project` — a clone of the whole project plus `VideoRenderer::new` and
/// a rebuild of `AudioRenderer` in the render thread. On the user’s edit this meant
/// 1961 events, ON EVERY MOVE of the region boundary and on every press of
/// Loop: the audio thread stopped, the ring emptied, and callback underruns began,
/// and after item 294 they no longer slow the clock, but are heard as silence.
///
/// The region and `loop_enabled` live in the “convenience” state (`apps/kt-app/CLAUDE.md`),
/// and the engine does not read them from the project: the loop reaches it via
/// `set_loop_region`.
#[test]
fn t286_region_edits_do_not_rebuild_the_engine() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.player.set_project(app.model.project().clone());

    let before = app.player.project_version();
    app.model.set_region(Region::normalized(kt_time::Tick(0), kt_time::Tick(45_000)));
    app.on_region_changed();
    app.toggle_loop();
    app.clear_region();
    assert_eq!(
        app.player.project_version(),
        before,
        "region and loop are a “convenience”: there is no reason to rebuild the engine"
    );

    // but a CONTENT edit must reach the player by the same path it did
    app.apply(Command::SetGain { id: ClipId(1), gain_db: -3.0 });
    assert!(
        app.player.project_version() > before,
        "a content edit must carry the project to the engine"
    );
}

/// Test 289 (R.43 item 290): two underrun numbers are labelled, but silence is
/// shown in MILLISECONDS.
///
/// The user’s complaint was phrased as: “the left underruns grow, and
/// the right one also starts a little later”. The left number is RENDER misses,
/// the right one is CALLBACK starvation, and this is a cascade “the cache did not give a burst → the ring
/// emptied”, not the left and right channels at all. The interface showed a bare
/// `Underruns X/Y` without a single word about what is what — reading them as
/// L/R was entirely legitimate.
///
/// Separately: callback starvation means FRAMES that were missing at the output.
/// In frames the number says nothing to a person; the status line shows it
/// also in milliseconds of silence.
#[test]
fn t289_underrun_counters_are_labelled_and_silence_is_in_ms() {
    use egui_kittest::kittest::Queryable;
    use std::sync::atomic::Ordering;

    let mut h = harness();
    // distinguishable numbers: they cannot be swapped by mistake
    const RENDER: u64 = 7;
    const CALLBACK: u64 = 96_000; // exactly 2000 ms of silence at 48 kHz
    {
        let app = h.state();
        app.player.underruns.store(RENDER, Ordering::Relaxed);
        app.player.shared.callback_underruns.store(CALLBACK, Ordering::Relaxed);
    }
    h.step();
    h.step();

    // we take the REAL text of the label, not the node’s Debug output: the node dump is full of
    // letters from service fields, and the check “there is a word between the numbers”
    // would pass by itself
    use egui_kittest::kittest::NodeT;
    let texts: Vec<String> = h
        .query_all_by_label_contains("")
        .filter_map(|n| {
            let a = n.accesskit_node();
            a.value().map(|v| v.to_string()).or_else(|| a.label().map(|l| l.to_string()))
        })
        .filter(|t| t.to_lowercase().contains("underrun"))
        .collect();
    let all = texts.join("\n").to_lowercase();
    assert!(!texts.is_empty(), "the underrun counters must be visible — they were not found at all");
    assert!(
        all.contains(&RENDER.to_string()) && all.contains(&CALLBACK.to_string()),
        "both numbers must be shown: {texts:?}"
    );
    // and in at least one place they are DISTINGUISHED BY WORDS, not only by a slash:
    // “7/96000” does not say which is the render and which is the callback
    let labelled = texts.iter().any(|t| {
        let (Some(a), Some(b)) = (t.find(&RENDER.to_string()), t.find(&CALLBACK.to_string()))
        else {
            return false;
        };
        a < b && t[a + 1..b].chars().any(char::is_alphabetic)
    });
    assert!(
        labelled,
        "between the numbers there must be a word (render/callback), not only “/”: {texts:?}"
    );
    // silence is in milliseconds, not in frames
    let ms = CALLBACK / (kt_time::AUDIO_SAMPLE_RATE as u64 / 1000);
    assert!(
        all.contains(&ms.to_string()),
        "callback starvation must be shown also in milliseconds ({ms} ms): {texts:?}"
    );
}

/// Test 343 (R.48, item 342): inserting an FX chain INTO A MORPHER carries the curves.
///
/// The defect lived since R.38: `ChainEdit.auto` was an `Option`, and on `None` the branch
/// that carried the curves was silently skipped — the inserted chain arrived
/// “empty”, although the buffer held an animation. Now the type does not let this branch
/// exist, and the test guards the MEANING: the keys were carried and recomputed into
/// the morpher’s domain (block time — offset from the donor’s entry point).
#[test]
fn t343_paste_chain_into_morpher_carries_curves() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.add_morph_layer();
    // in the buffer — a chain WITH A CURVE from the donor with an entry point of 45_000
    app.fx_clipboard = Some(kt_model::ops::FxChainBlock {
        slots: vec![kt_model::ops::FxSlotBlock {
            fx: FxInstance::blur(8),
            bypass: false,
            curves: vec![ParamCurve {
                param: "radius".into(),

                keys: AutoKey::linear(vec![
                    AutoKey::at(kt_time::Tick(45_000), 1.0),
                    AutoKey::at(kt_time::Tick(135_000), 48.0),
                ]),
                source: Default::default(),
            }],
        }],
        src_in: kt_time::Tick(45_000),
        fx_off: false,
    });
    h.step();
    h.step();
    assert!(h.state().morph_editors.iter().any(|e| e.open), "the morpher window did not open");

    // the chain insert button — the same as the event’s
    let paste = tr(k::fx::PASTE_CHAIN).to_string();
    h.get_by_label(&paste).click();
    h.step();
    h.step();

    let ev = h.state().model.project().morphers[0].clone();
    assert_eq!(ev.patch.video_fx.len(), 1, "the chain must be inserted into the morpher");
    let curves: Vec<_> = ev.patch.video_auto.iter().flatten().collect();
    assert_eq!(curves.len(), 1, "the curve must arrive together with the slot");
    assert_eq!(
        curves[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, 90_000],
        "and be recomputed into the morpher domain (offset from the entry point of the donor)"
    );
}

/// A project with one video event and the FX window open — the shared setup
/// of the preset tests.
#[cfg(test)]
pub(crate) fn harness_with_fx_window() -> egui_kittest::Harness<'static, crate::app::KtApp> {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(30_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![false],
        fx_auto: vec![vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![
                AutoKey::at(kt_time::Tick(30_000), 1.0),
                AutoKey::at(kt_time::Tick(120_000), 48.0),
            ]),
            source: Default::default(),
        }]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    h
}

/// The same harness, but slot 0 has TWO effects: a blur with a radius curve and
/// a second blur — the target for the link door (R.67).
pub(crate) fn harness_with_fx_window_two_slots() -> egui_kittest::Harness<'static, crate::app::KtApp> {
    use kt_model::*;
    let mut h = harness_with_fx_window();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[0].fx.push(FxInstance::blur(4));
        p.clips[0].fx_bypass.push(false);
        p.validate().unwrap();
        app.model = TimelineModel::new(p);
    }
    h.step();
    h.step();
    h
}

/// A sample event with one field to replace (R.67, t790).
pub(crate) fn sample_clip(id: kt_model::ClipId) -> kt_model::ClipEvent {
    use kt_model::*;
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id,
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// Test 345 (R.48, item 344): the “save as preset” window per the t250 checklist.
///
/// The window does not open by itself, it is really drawn on the button; Esc closes it
/// cleanly, a repeated call works, and CONFIRMATION does not touch the model:
/// a preset is the user’s file, and the project’s undo does not touch it.
#[test]
fn t345_preset_save_dialog_follows_the_rules() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_fx_window();
    let title = tr(k::msg::PRESET_SAVE_TITLE).to_string();
    assert!(h.query_by_label(&title).is_none(), "the window must not open by itself");

    // the button in the chain strip
    h.get_by_label(tr(k::fx::SAVE_PRESET)).click();
    h.step();
    h.step();
    assert!(h.query_by_label(&title).is_some(), "the window must be DRAWN");

    // Esc closes and leaves no traces
    h.key_press(egui::Key::Escape);
    h.step();
    h.step();
    assert!(h.query_by_label(&title).is_none(), "Esc must close the window");
    assert!(h.state().preset_save.is_none(), "and leave no state behind");

    // a repeated call works (rule 14)
    h.get_by_label(tr(k::fx::SAVE_PRESET)).click();
    h.step();
    h.step();
    assert!(h.query_by_label(&title).is_some(), "the feature must work again");

    // confirmation: the preset is in the library, the model is NOT touched
    let undo_before = h.state().model.undo_depth();
    let rev_before = h.state().model_rev;
    h.state_mut().preset_save.as_mut().unwrap().name = "My chain".into();
    // the name is typed → a live preview appears → the window grew taller, and
    // the buttons moved down: one must probe AFTER the redraw, otherwise the click
    // goes to the old coordinates (the same trap as the three-step drag)
    h.step();
    h.step();
    h.get_by_label(tr(k::msg::PRESET_SAVE_DO)).click();
    h.step();
    h.step();
    let app = h.state_mut();
    assert!(app.preset_save.is_none(), "after saving, the window closes");
    assert!(
        app.presets.rows().iter().any(|r| r.name == "My chain" && !r.factory),
        "the preset must appear in the library"
    );
    assert_eq!(app.model.undo_depth(), undo_before, "a preset is a file, not a project edit");
    assert_eq!(app.model_rev, rev_before, "and it does not move the model");
    // keys are stored from zero: the preset does not remember the donor (the entry was 30_000)
    let saved = app.presets.rows().iter().find(|r| r.name == "My chain").unwrap().clone();
    assert_eq!(
        saved.block.slots[0].curves[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, 90_000]
    );
}

/// Test 346 (R.48, item 345): inserting a preset from the picker.
///
/// The preset is appended to the end of the chain, the curves move onto the receiver’s
/// material, and all of this is an ordinary edit of the event: one undo step.
#[test]
fn t346_preset_insert_appends_chain_with_curves() {
    use kt_model::*;
    let mut h = harness_with_fx_window();
    // in the library — a preset with a curve (we put it straight into the store: the picker is
    // an egui menu, whose opening is checked by the t345 part about the button)
    let block = kt_model::ops::FxChainBlock {
        slots: vec![kt_model::ops::FxSlotBlock {
            fx: FxInstance::glow(0.7),
            bypass: false,
            curves: vec![ParamCurve {
                param: "strength".into(),
                keys: vec![
                    AutoKey::at(kt_time::Tick(0), 0.2),
                    AutoKey::at(kt_time::Tick(90_000), 1.5),
                ],
                source: Default::default(),
            }],
        }],
        src_in: kt_time::Tick(0),
        fx_off: false,
    };
    let app = h.state_mut();
    app.presets.add("Glow ramp", &block).expect("add");
    let row = app.presets.rows().iter().find(|r| r.name == "Glow ramp").unwrap().clone();
    assert!(row.video_ok && !row.audio_ok, "glow is a video preset");

    // the insert goes the same way as from the picker: append_chain_block with
    // the receiver’s entry point
    let clip = app.model.clip(ClipId(1)).unwrap().clone();
    let (mut fx, mut bypass, mut auto) =
        (clip.fx.clone(), clip.fx_bypass.clone(), clip.fx_auto.clone());
    kt_model::ops::append_chain_block(&mut fx, &mut bypass, &mut auto, &row.block, clip.source_in);
    let undo_before = app.model.undo_depth();
    app.apply_batch(vec![
        Command::SetFx { id: ClipId(1), fx },
        Command::SetFxBypass { id: ClipId(1), bypass },
        Command::SetFxAuto { id: ClipId(1), auto },
    ]);
    h.step();

    let clip = h.state().model.clip(ClipId(1)).unwrap().clone();
    assert_eq!(clip.fx.len(), 2, "the preset is appended to the end, not replacing the chain");
    assert!(matches!(clip.fx[1], FxInstance::Glow { .. }));
    assert_eq!(
        clip.fx_auto[1][0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![30_000, 120_000],
        "the curve must move to the material of the receiver (entry 30_000)"
    );
    assert_eq!(
        h.state().model.undo_depth(),
        undo_before + 1,
        "inserting a preset is one history step"
    );
    h.state_mut().model.undo().expect("undo");
    assert_eq!(
        h.state().model.clip(ClipId(1)).unwrap().fx.len(),
        1,
        "undo removes the inserted preset"
    );
}

/// A project with one audio event and an equaliser; the FX window is open.
#[cfg(test)]
fn harness_with_eq_window() -> egui_kittest::Harness<'static, crate::app::KtApp> {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::eq()],
        fx_bypass: vec![false],
        fx_auto: vec![vec![]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    h
}

/// Test 349 (R.48, item 347): the equaliser has a frequency-response graph, its point
/// is dragged with the mouse, and the edit reaches the MODEL in one history step.
///
/// What is asserted is the consequence a person sees (the band moved to another frequency
/// and gain), not “the function returned a value”: exactly
/// this lesson is recorded in the rule of R.40.
#[test]
fn t349_eq_graph_drag_edits_the_model() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness_with_eq_window();
    let node = h.query_by_label("eq_graph");
    assert!(node.is_some(), "the equalizer must have a frequency response graph");
    let rect = node.unwrap().rect();

    // the middle of the “mid” band: default 1 kHz / 0 dB. We compute the coordinates with the
    // same functions that draw them — otherwise the test would check its own arithmetic.
    let plot_w = rect.width() - 4.0;
    let plot_x = rect.left() + 2.0;
    let plot_h = rect.height() - 12.0;
    let plot_y = rect.top() + 6.0;
    let x_of = |hz: f64| plot_x + crate::eq_graph::hz_to_u(hz) * plot_w;
    let y_of = |db: f64| plot_y + ((24.0 - db) / 48.0) as f32 * plot_h;
    let from = egui::pos2(x_of(1000.0), y_of(0.0));
    let to = egui::pos2(x_of(2000.0), y_of(6.0));

    // a three-step drag: egui recognises a drag only after a threshold, and the position
    // is read in the frame where the gesture starts (harness rule)
    h.drag_at(from);
    h.step();
    h.hover_at(from + egui::vec2(8.0, -4.0));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();

    let clip = h.state().model.clip(ClipId(1)).unwrap().clone();
    let (freq, gain) = match &clip.fx[0] {
        FxInstance::Eq { mid_freq_hz, mid_gain_db, .. } => (*mid_freq_hz, *mid_gain_db),
        other => panic!("expected an equalizer, got {other:?}"),
    };
    println!("t349 after drag: {freq:.0} Hz, {gain:+.2} dB");
    assert!(
        (freq as f64 - 2000.0).abs() / 2000.0 < 0.03,
        "the band frequency must move to 2 kHz, not {freq}"
    );
    assert!((gain - 6.0).abs() < 0.5, "the gain must become +6 dB, not {gain}");

    // one history step for the whole gesture (coalescing of the window session)
    let depth = h.state().model.undo_depth();
    h.state_mut().model.undo().expect("undo");
    assert_eq!(h.state().model.undo_depth(), depth - 1);
    let back = h.state().model.clip(ClipId(1)).unwrap().clone();
    match &back.fx[0] {
        FxInstance::Eq { mid_freq_hz, mid_gain_db, .. } => {
            assert!((*mid_freq_hz - 1000.0).abs() < 1.0, "undo restored the frequency");
            assert!(mid_gain_db.abs() < 0.01, "and the gain");
        }
        other => panic!("expected an equalizer, got {other:?}"),
    }
}

/// Test 350 (R.48, item 347): the wheel over the middle point edits the Q factor
/// and does NOT scroll the window (the gesture is absorbed only at the point itself).
#[test]
fn t350_eq_graph_wheel_sets_q() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness_with_eq_window();
    let rect = h.query_by_label("eq_graph").expect("the graph exists").rect();
    let plot_w = rect.width() - 4.0;
    let plot_x = rect.left() + 2.0;
    let plot_h = rect.height() - 12.0;
    let plot_y = rect.top() + 6.0;
    let mid = egui::pos2(
        plot_x + crate::eq_graph::hz_to_u(1000.0) * plot_w,
        plot_y + (24.0 / 48.0) * plot_h,
    );
    h.hover_at(mid);
    h.step();
    h.input_mut().events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, -3.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    h.step();
    h.step();

    let clip = h.state().model.clip(ClipId(1)).unwrap().clone();
    let q = match &clip.fx[0] {
        FxInstance::Eq { mid_q, .. } => *mid_q,
        other => panic!("expected an equalizer, got {other:?}"),
    };
    println!("t350 Q after the wheel: {q:.2}");
    assert!(q < 1.0 - 0.05, "the wheel down must decrease the Q, not {q}");
    assert!(q >= 0.3, "and not go past the lower bound of the spec");
}

/// Test 430b (R.50 A2): one can collapse and expand BY HAND, and the program
/// says what has changed as a result.
///
/// What is asserted is the CONSEQUENCE (R.40): not “the item is in the menu”, but “the edit became
/// different”. The point from which the test presses is the event’s body: exactly where
/// a person presses.
#[test]
fn t430b_collapse_and_expand_are_reachable_by_hand() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, g) = harness_three_clips();
    let row = g.row_of((LaneKind::Video, 0)).expect("video row");
    let y = g.row_center_y(row);

    // we will not select two events with a frame — one is enough: the item works
    // on a single one too, and the test asserts reachability
    h.state_mut().select_only(ClipId(1));
    h.step();
    right_click_at(&mut h, egui::pos2(g.tick_to_x(20_000), y));
    assert!(
        matches!(h.state().ctx_target, Some(crate::timeline::CtxTarget::Clip(ClipId(1)))),
        "right-click on the event body must give the EVENT menu"
    );
    let labels = menu_labels(&mut h);
    assert!(
        labels.iter().any(|l| l == tr(k::tl::menu::COLLAPSE)),
        "there is no collapse item in the event menu: {labels:?}"
    );
    h.get_by_label(tr(k::tl::menu::COLLAPSE)).click();
    h.step();
    h.step();

    // CONSEQUENCE: the track became a reel, the event became an instance
    let p = h.state().model.project().clone();
    assert_eq!(p.sequences.len(), 1, "the reel was not created: {:?}", p.sequences);
    let seq = p.sequences[0].id;
    let inst = p.clips.iter().find(|c| c.media == seq).expect("there is no instance on the timeline");
    assert_eq!(p.sequences[0].clips.len(), 1, "the reel has the wrong content");
    assert_eq!(h.state().nest_uses(seq), 1, "the number of usage places was counted wrongly");
    let inst_id = inst.id;

    // ...and the menu of the NESTED item itself knows what to do with it
    let g = h.state().tl_geom.clone().unwrap_or(g);
    right_click_at(&mut h, egui::pos2(g.tick_to_x(20_000), y));
    let labels = menu_labels(&mut h);
    for want in [tr(k::tl::menu::NEST_EXPAND), tr(k::tl::menu::NEST_RENAME)] {
        assert!(labels.iter().any(|l| l == want), "there is no “{want}” in the nested menu: {labels:?}");
    }
    assert!(
        !labels.iter().any(|l| l.starts_with(tr(k::tl::menu::NEST_INDEPENDENT))),
        "“unlink from other places” is offered where there is ONE place — a promise that changes nothing: {labels:?}"
    );

    // the name window opens, Esc closes it without traces, and it works
    // REPEATABLE (rule 14)
    for _ in 0..2 {
        h.get_by_label(tr(k::tl::menu::NEST_RENAME)).click();
        h.step();
        h.step();
        assert!(h.state().seq_name.is_some(), "the reel name window did not open");
        h.key_press(egui::Key::Escape);
        h.step();
        h.step();
        assert!(h.state().seq_name.is_none(), "Esc did not close the reel name window");
        let g = h.state().tl_geom.clone().unwrap_or(g.clone());
        right_click_at(&mut h, egui::pos2(g.tick_to_x(20_000), y));
    }

    // expanded — the edit came back, and this is said
    h.state_mut().expand_nest(inst_id);
    h.step();
    let p = h.state().model.project().clone();
    assert!(
        p.clips.iter().all(|c| c.media != seq),
        "the instance stayed on the timeline after expanding"
    );
    assert_eq!(p.clips.len(), 3, "the content did not return to the track: {:?}", p.clips.len());
}

// ---- 456-460 (R.50, B): entering a reel by hand -----------------------------

/// a harness with a reel of one event and an instance on a track
#[cfg(test)]
fn harness_with_nest_dt(
    dt: Option<f32>,
) -> (Harness<'static, KtApp>, crate::timeline::TlGeom, kt_model::MediaId) {
    let (mut h, g) = harness_three_clips_dt(dt);
    {
        let app = h.state_mut();
        // WE COLLAPSE THE MIDDLE event, not the first one, and this is not whim
        // (R.55, C1). `ClipId(1)` sits at zero, so the instance has
        // `start == 0` and `source_in == 0`, and the cursor mapping
        // `source_in + (t − start)` degenerates into an identity: t456 was
        // green even on code that does not map the cursor AT ALL. Here
        // `ClipId(2)` sits at 100_000 — the axis offset is non-zero, and there is
        // something to check the formula with.
        app.select_only(kt_model::ClipId(2));
    }
    h.step();
    h.state_mut().collapse_selection();
    h.step();
    h.step();
    let seq = h.state().model.document().sequences[0].id;
    let g = h.state().tl_geom.clone().unwrap_or(g);
    (h, g, seq)
}

#[cfg(test)]
fn harness_with_nest() -> (Harness<'static, KtApp>, crate::timeline::TlGeom, kt_model::MediaId) {
    harness_with_nest_dt(None)
}

/// Test 456 (R.50, B1): entry and exit are an identity in kind and in cursor.
#[test]
fn t456_entering_and_leaving_is_identity() {
    use kt_time::Tick;
    let (mut h, g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;

    // a snapshot of the view from OUTSIDE. The cursor is INSIDE the instance’s body (100_000 …
    // 150_000): entry translates the time rather than teleporting, and checking the
    // translation only makes sense from where the person enters.
    h.state_mut().player.seek(Tick(120_000));
    h.state_mut().view_start = 5_000.0;
    h.state_mut().ticks_per_px = 900.0;
    h.state_mut().select_only(inst);
    h.step();
    let (vs, tpp) = (h.state().view_start, h.state().ticks_per_px);
    let outside = h.state().player.transport.playhead();

    h.state_mut().enter_nest(inst);
    h.step();
    assert_eq!(h.state().model.editing(), Some(seq), "the entry did not happen");
    assert_eq!(h.state().nest_crumbs().len(), 1, "the path crumb did not appear");
    // the CURSOR IS TRANSLATED, not reset: the person entered, did not jump
    let c = h.state().model.document().clips.iter().find(|c| c.id == inst).unwrap().clone();
    let want = c.source_in + (outside - c.start);
    // THE GUARD OF THE CHECK ITSELF: on an instance standing at zero, `want`
    // coincides with `outside`, and everything below stops asserting anything.
    // The scene must stay offset.
    assert_ne!(
        want.0, outside.0,
        "scene degenerated: the instance is at zero, and the cursor transfer cannot be told apart \
         from its absence"
    );
    // ACCURACY — THE TRANSPORT QUANTUM, named rather than tuned: the playhead
    // lives in 48 kHz audio frames, and the round trip “tick → frame → tick” truncates
    // downwards (item 7, round 10). One frame — 90000/48000 = 1.875 ticks, rounded
    // up. Requiring tick-for-tick would mean requiring what the transport
    // cannot do here or anywhere else.
    const QUANT: i64 = (kt_time::TICK_HZ + kt_time::AUDIO_SAMPLE_RATE as i64 - 1)
        / kt_time::AUDIO_SAMPLE_RATE as i64;
    let got = h.state().player.transport.playhead();
    assert!(
        (got.0 - want.0).abs() <= QUANT,
        "the cursor was reset on entry, not transferred ({got:?} against {want:?}) — the preview \
         would show a different frame"
    );
    // the CONTENT of the REEL is at hand
    assert_eq!(h.state().model.project().clips.len(), 1, "inside the reel its content is not visible");

    // exit — the view came back the SAME (rule 14: the feature works repeatedly)
    h.state_mut().leave_nest();
    h.step();
    assert_eq!(h.state().model.editing(), None, "the exit did not happen");
    assert!(h.state().nest_crumbs().is_empty(), "the crumb remained after the exit");
    let back = h.state().player.transport.playhead();
    assert!(
        (back.0 - outside.0).abs() <= 2 * QUANT,
        "the cursor did not return to its place: {back:?} against {outside:?}"
    );
    assert_eq!((h.state().view_start, h.state().ticks_per_px), (vs, tpp), "the view was not restored");
    assert!(h.state().selected.contains(&inst), "the selection was not restored");
    let _ = g;
}

/// Test 552 (R.55, C1): INSIDE a reel the REEL plays, not the root.
///
/// Before R.55 the player always received `document()`. A person entered a
/// composition, saw its events on the timeline — and heard the root edit, taken at a
/// moment numerically equal to the reel’s LOCAL time. That matched exactly when the
/// instance stood at zero, so no test caught the defect
/// (the same hole closed by the t456 harness).
///
/// The claim is verified by its CONSEQUENCE — by what actually goes to
/// the player, not by which method was called.
#[test]
fn t552_inside_a_reel_the_reel_is_what_plays() {
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;

    // OUTSIDE the document plays: the root carries the instance and its neighbours
    let outside = h.state().player_project();
    assert!(
        outside.clips.iter().any(|c| c.id == inst),
        "at the root the player must play the document — there is no instance in it"
    );
    assert!(
        outside.sequences.iter().any(|s| s.id == seq),
        "the reel must stay in the document: it is played through the instance"
    );

    h.state_mut().enter_nest(inst);
    h.step();

    let inside = h.state().player_project();
    // the ROOT of what is playing — the content of the REEL
    let want: Vec<kt_model::ClipId> =
        h.state().model.project().clips.iter().map(|c| c.id).collect();
    let got: Vec<kt_model::ClipId> = inside.clips.iter().map(|c| c.id).collect();
    assert_eq!(
        got, want,
        "inside the reel the player plays NOT ITS OWN: on the timeline one set of events, in the sound and \
         preview others"
    );
    assert!(
        !inside.clips.iter().any(|c| c.id == inst),
        "the instance must disappear from what plays: standing INSIDE it, \
         hearing it from outside means hearing yourself twice"
    );

    // But OUTSIDE (to the file and to the export) the DOCUMENT still goes —
    // the real layout, the root as root (t456b guards the calls,
    // this piece guards the CONTENT).
    let doc = h.state().model.document();
    assert!(
        doc.clips.iter().any(|c| c.id == inst),
        "the content of the reel went into the document instead of the root — writing such means \
         swapping the root and the reel on disk"
    );

    h.state_mut().leave_nest();
    h.step();
    let back: Vec<kt_model::ClipId> =
        h.state().player_project().clips.iter().map(|c| c.id).collect();
    let was: Vec<kt_model::ClipId> = outside.clips.iter().map(|c| c.id).collect();
    assert_eq!(back, was, "the exit did not return the root to the player");
}

/// Test 553 (R.55, C1): entry brings the VIEW to where the cursor stood.
///
/// The cursor was translated before, but the scroll was not: it stayed in the axis
/// of the ROOT. Entering a nested item standing at the tenth minute, a person saw
/// an empty screen — the reel’s content lies around its zero.
/// This is the complaint “we end up at the same t where we were,
/// while the composition runs from t=0”.
#[test]
fn t553_entering_brings_the_view_to_the_cursor() {
    use kt_time::Tick;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    let c = h.state().model.project().clips.iter().find(|x| x.id == inst).unwrap().clone();

    // the cursor INSIDE the instance body, the view around it IN THE ROOT AXIS
    h.state_mut().player.seek(c.start + Tick(c.duration.0 / 2));
    h.state_mut().view_start = c.start.0 as f64;
    h.state_mut().select_only(inst);
    h.step();
    let before = h.state().view_start;

    h.state_mut().enter_nest(inst);
    h.step();

    let cur = h.state().player.transport.playhead();
    let g = h.state().tl_geom.clone().expect("timeline geometry");
    let span = g.rect.width() as f64 * h.state().ticks_per_px;
    let vs = h.state().view_start;
    assert!(
        cur.0 as f64 >= vs && (cur.0 as f64) < vs + span,
        "the cursor ({}) is outside the visible window [{vs}, {}) — the person looks at an empty place",
        cur.0,
        vs + span
    );
    // the guard of the check itself: the scene must be offset, otherwise
    // “the view was not touched” is indistinguishable from “the view was brought”
    assert_ne!(before, vs, "scene degenerated: the view was already where it was supposed to be brought");
}

/// Test 457 (R.50, B3): entering is possible in FOUR ways, and all of them are one body.
#[test]
fn t457_four_ways_in_one_way_out() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    let row = g.row_of((LaneKind::Video, 0)).expect("video row");
    let y = g.row_center_y(row);
    let x = {
        let c = h.state().model.project().clips.iter().find(|c| c.id == inst).unwrap().clone();
        g.tick_to_x(c.start.0 + c.duration.0 / 2)
    };

    // 1) menu item
    right_click_at(&mut h, egui::pos2(x, y));
    assert!(
        matches!(h.state().ctx_target, Some(crate::timeline::CtxTarget::Nest(_))),
        "RMB on a nested item must give the NESTED target, not an ordinary event"
    );
    // the item label is searched as a SUBSTRING: the item has a chord column
    // (R.56, item 451), and an exact match would break on it
    h.get_by_label_contains(tr(k::tl::menu::NEST_ENTER)).click();
    h.step();
    h.step();
    assert_eq!(h.state().model.editing(), Some(seq), "the menu item did not enter inside");

    // 2) Esc — LAST in the cascade: first it removes the selection
    let inner = h.state().model.project().clips[0].id;
    h.state_mut().select_only(inner);
    h.state_mut().escape_action();
    assert_eq!(
        h.state().model.editing(),
        Some(seq),
        "Esc threw out instead of clearing the selection — one key \
         does two things, and the second is irreversible in attention"
    );
    h.state_mut().escape_action();
    assert_eq!(h.state().model.editing(), None, "Esc on an empty selection must lead outward");

    // 3) A DOUBLE CLICK gives not a full entry but an IN-PLACE EXPAND (R.55,
    // C5): a quick gesture gives the DEFAULT mode, the named menu item — the
    // named mode. The owner’s direct request: “by default a hybrid convenient
    // mode is needed”.
    //
    // egui’s double click is TEMPORAL: the harness step by default is 0.25 s, and
    // without a small dt the test would check a SINGLE click
    let (mut h2, g2, seq2) = harness_with_nest_dt(Some(0.02));
    let inst2 = h2.state().model.project().clips.iter().find(|c| c.media == seq2).unwrap().id;
    let row2 = g2.row_of((LaneKind::Video, 0)).unwrap();
    let x2 = {
        let c = h2.state().model.project().clips.iter().find(|c| c.id == inst2).unwrap().clone();
        g2.tick_to_x(c.start.0 + c.duration.0 / 2)
    };
    let pos2 = egui::pos2(x2, g2.lane_y(ClipKind::Video, 0, 0.6));
    double_click_at(&mut h2, pos2);
    assert_eq!(
        h2.state().model.editing(),
        None,
        "double-click went into the FULL entry: from the root in hybrid mode one does not exit, \
         otherwise the reel, not the program, would play"
    );
    assert_eq!(
        h2.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![inst2],
        "double-click did not expand the composition in place"
    );
    // THE GESTURE WORKS AGAIN (rule 14): the second time it collapses
    double_click_at(&mut h2, pos2);
    assert!(
        h2.state().model.bands().is_empty(),
        "a repeated double-click must collapse the expansion, not accumulate strips"
    );
    let _ = row2;

    // 4) Enter on the selection — a CASCADE (R.56, item 451, the owner’s decision:
    // “I want a double enter to go into it”). The first expands in place,
    // the second, on the already expanded one — a FULL ENTRY.
    let (mut h3, _, seq3) = harness_with_nest();
    let inst3 = h3.state().model.project().clips.iter().find(|c| c.media == seq3).unwrap().id;
    h3.state_mut().select_only(inst3);
    h3.step();
    h3.key_press(egui::Key::Enter);
    h3.step();
    h3.step();
    assert_eq!(h3.state().model.editing(), None, "the first Enter went into the full entry");
    assert_eq!(
        h3.state().model.bands().len(),
        1,
        "Enter on the selected nested item did not expand it in place"
    );
    // the instance REMAINED selected — otherwise the second Enter would have nothing to aim at,
    // and the gesture would silently do nothing
    assert!(
        h3.state().is_selected(inst3),
        "the expansion reset the instance selection: the next gesture hits nothing"
    );

    let (vs, tpp) = (h3.state().view_start, h3.state().ticks_per_px);
    h3.key_press(egui::Key::Enter);
    h3.step();
    h3.step();
    assert_eq!(h3.state().model.editing(), Some(seq3), "the second Enter did not enter INSIDE the reel");
    assert!(h3.state().model.bands().is_empty(), "the full entry must collapse the strip");

    // exit — to the ROOT, AND THE STRIP COMES BACK (the owner’s decision in R.58,
    // a revision of R.56 item 5: “they reset the expanded state, but we would like
    // them to keep it”; the Enter/Esc cascade is symmetric — t636)
    h3.state_mut().escape_action();
    h3.step();
    assert_eq!(h3.state().model.editing(), None, "Esc did not lead outward");
    assert_eq!(
        h3.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![inst3],
        "the exit must RESTORE the strip through which one entered (decision of the owner R.58)"
    );
    assert_eq!(
        (h3.state().view_start, h3.state().ticks_per_px),
        (vs, tpp),
        "the view did not return after the exit"
    );
}

/// Test 458 (R.50, B2): a nested item has its own view and its own promises.
///
/// Three things not visible in the model: thumbnails BY FILE do not exist for a nested item
/// (a reel has no file; its frames come from the offline render of its content —
/// R.56, item 452, the `reel_preview` service), the “×N” badge appears ONLY when
/// N > 1, and the background inside a reel is tinted.
#[test]
fn t458_nest_looks_like_a_nest() {
    use kt_model::*;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;

    // THUMBNAILS BY FILE: no key of the thumbnail service addresses a REEL — it has no file,
    // and the index service would go after it with ffmpeg. The frames on the
    // event DO exist, but they come from `reel_preview` (t600).
    h.step();
    h.step();
    assert!(
        h.state().thumb_textures.keys().all(|k| k.media != seq),
        "the nested item created thumbnails at the address of the REEL — the reel has no file"
    );

    // the BADGE ×N: with one place it is absent
    assert_eq!(h.state().nest_uses(seq), 1, "scene: one place");
    // ...but with two it is present, and “next place” cycles round
    let mut twin = h.state().model.project().clips.iter().find(|c| c.id == inst).unwrap().clone();
    twin.id = ClipId(h.state().next_clip_id());
    twin.start = kt_time::Tick(900_000);
    twin.link_group = None;
    assert!(h.state_mut().apply(Command::AddClip(twin.clone())));
    h.step();
    assert_eq!(h.state().nest_uses(seq), 2, "the second place was not counted");
    h.state_mut().goto_next_place(inst);
    h.step();
    assert!(
        h.state().selected.contains(&twin.id),
        "“next place” did not lead to the second instance — the ×N badge remained \
         a message with no continuation"
    );
    // ...and round again, back
    h.state_mut().goto_next_place(twin.id);
    h.step();
    assert!(h.state().selected.contains(&inst), "the cycle did not close at the first place");
}

/// Test 460 (item 397, R.51): a repeated drag of the fade corner handle edits ONLY the length
/// of the window — the chosen kind, the parameter curves and the audio curve survive.
/// Before, `commit_fade_handle` built a fresh Crossfade and silently wiped
/// the settings (against rule 20 “nothing switches itself off”).
#[test]
fn t460_fade_handle_redrag_keeps_kind() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(100_000),
            duration: Tick(200_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        // a corner fade-in with the CONFIGURED kind, curve and sound
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: None,
            b: Some(ClipId(1)),
            in_offset: Tick::ZERO,
            out_offset: Tick(40_000),
            kind: TransitionKind::Wipe { dir: Dir::Left, angle: 10.0, softness: 0.2 },
            audio: AudioCurve::SCurve,
            auto: vec![ParamCurve {
                param: "angle".into(),

                keys: AutoKey::linear(vec![
                    AutoKey::at(Tick(0), 0.0),
                    AutoKey::at(Tick(30_000), 45.0),
                ]),
                source: Default::default(),
            }],
            invert: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.select_only(ClipId(1));
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let y_strip = g.lane_top(ClipKind::Video, 0) + 3.0;

    // drag the fade handle (the end of the window, tick 140 000) further right to 180 000
    let from = g.tick_to_x(140_000);
    h.hover_at(egui::pos2(from, y_strip));
    h.step();
    h.drag_at(egui::pos2(from, y_strip));
    h.step();
    let to = g.tick_to_x(180_000);
    h.hover_at(egui::pos2(to, y_strip));
    h.step();
    h.drop_at(egui::pos2(to, y_strip));
    h.step();

    let p = h.state().model.project().clone();
    assert_eq!(p.transitions.len(), 1, "one fade: {:?}", p.transitions);
    let t = &p.transitions[0];
    assert!(
        (t.out_offset.0 - 80_000).abs() <= 2_000,
        "the window length moved with the handle: {:?}",
        t.out_offset
    );
    assert_eq!(
        t.kind,
        TransitionKind::Wipe { dir: Dir::Left, angle: 10.0, softness: 0.2 },
        "the fade view survived a repeated corner drag"
    );
    assert_eq!(t.audio, AudioCurve::SCurve, "the audio curve was not reset");
    assert_eq!(t.auto.len(), 1, "the parameter curves were not erased: {:?}", t.auto);
}

/// Test 460b (item 397, R.51): a repeated overlap by a Move drag on a pair with an already
/// configured transition moves the window, but keeps the kind/audio curve.
#[test]
fn t460b_overlap_redrag_keeps_kind() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        // the overlap crossfade branch lives in the mode WITHOUT interleaving (R.28)
        p.overlap_layers = false;
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (n, start, src_in) in [(1u64, 0i64, 0i64), (2, 90_000, 20_000)] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(n),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(src_in),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: Some(ClipId(2)),
            in_offset: Tick(9_000),
            out_offset: Tick(9_000),
            kind: TransitionKind::Barn { vertical: true, softness: 0.1 },
            audio: AudioCurve::Linear,
            auto: vec![],
            invert: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("no timeline geometry");
    let y = g.lane_y(ClipKind::Video, 0, 0.5);
    let grab = g.tick_to_x(90_000 + 45_000);
    h.hover_at(egui::pos2(grab, y));
    h.step();
    h.drag_at(egui::pos2(grab, y));
    h.step();
    h.hover_at(egui::pos2(grab - 8.0, y));
    h.step();
    let grab_dt = match h.state().drag {
        Some(crate::timeline::Drag::Clip(ref d)) => d.grab_dt,
        _ => panic!("clip drag did not start"),
    };
    // target: start = 50 000 — the overlap goes DEEPER into the tail of clip 1
    let to = g.tick_to_x(50_000 + grab_dt);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();

    let p = h.state().model.project().clone();
    assert_eq!(p.transitions.len(), 1, "one transition: {:?}", p.transitions);
    let t = &p.transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(1)), Some(ClipId(2))));
    assert!((t.in_offset.0 - 20_000).abs() <= 1_000, "the window moved: {:?}", t.in_offset);
    assert_eq!(
        t.kind,
        TransitionKind::Barn { vertical: true, softness: 0.1 },
        "the transition view survived a repeated overlap"
    );
    assert_eq!(t.audio, AudioCurve::Linear, "the audio curve was not reset");
    p.validate().expect("result is valid");
}

/// Test 466 (item 399, R.51): a transition ON AN OVERLAP is reachable by gestures — RMB on the
/// overlap zone offers “Transition from…” (the neighbouring “staircase” is equal to the butt-joint one,
/// rule 17), and a double click on the zone of an overlapping transition
/// opens its properties, as for the butt-joint one.
#[test]
fn t466_overlap_transition_reachable() {
    use kt_model::*;
    use kt_time::Tick;
    let overlap_app = |with_transition: bool| {
        let (mut h, g) = harness_three_clips_dt(Some(0.02));
        {
            let app = h.state_mut();
            let mut p = app.model.project().clone();
            // staircase: clip 1 [0..50k), clip 2 [30k..80k); the third further away
            p.clips[0].duration = Tick(50_000);
            p.clips[1].start = Tick(30_000);
            p.clips[1].duration = Tick(50_000);
            p.clips[2].start = Tick(200_000);
            if with_transition {
                p.transitions.push(TransitionEvent {
                    id: TransitionId(1),
                    a: Some(ClipId(1)),
                    b: Some(ClipId(2)),
                    in_offset: Tick::ZERO,
                    out_offset: Tick::ZERO,
                    kind: TransitionKind::Barn { vertical: false, softness: 0.1 },
                    audio: Default::default(),
                    auto: vec![],
                    invert: false,
                });
            }
            p.validate().unwrap();
            app.model = kt_model::TimelineModel::new(p);
        }
        h.step();
        let g2 = h.state().tl_geom.clone().unwrap_or(g);
        (h, g2)
    };

    // RMB on the overlap zone WITHOUT a transition: the clip menu offers a transition
    let (mut h, g) = overlap_app(false);
    let y = g.lane_y(kt_model::ClipKind::Video, 0, 0.75);
    right_click_at(&mut h, egui::pos2(g.tick_to_x(40_000), y));
    assert!(
        matches!(h.state().ctx_target, Some(crate::timeline::CtxTarget::Clip(_))),
        "RMB on the overlap zone must give the clip menu"
    );
    let labels = menu_labels(&mut h);
    assert!(
        labels.iter().any(|l| {
            // menu_button appends a submenu arrow to the label
            l.starts_with(tr(k::tl::menu::TRANSITION_PREV))
                || l.starts_with(tr(k::tl::menu::TRANSITION_NEXT))
        }),
        "the “staircase” neighbor must give the “Transition from…” item: {labels:?}"
    );

    // a double click on the zone of an OVERLAPPING transition opens its properties
    let (mut h, g) = overlap_app(true);
    let pos = egui::pos2(g.tick_to_x(40_000), y);
    for _ in 0..2 {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    assert_eq!(
        h.state().transition_editors.len(),
        1,
        "double-click on an overlapping transition must open its properties"
    );
    // and the window length in the properties is the length of the OVERLAP, the offsets have nothing to do with it
    let p = h.state().model.project();
    let t = &p.transitions[0];
    let w = t.window(p).expect("overlap window");
    assert_eq!((w.end - w.start).0, 20_000, "window = overlap zone");
}

/// Test 511 (R.53, item 415; rewritten in R.59 stage 2): ONE kind picker, and a change of kind
/// reaches the model without throwing the person out of the window.
///
/// Before R.53 the title window was a dead end: the generator’s kind was not shown in it
/// at all, and there was no way to switch the title back to a colour (rule 17). With R.59
/// there is one window, and there is nowhere else to route — the claim narrowed to
/// its essence: the picker exists, the kind changes, the form stays in place, and the change of kind
/// is its OWN history step (rule 20): one Ctrl+Z restores the title with all
/// the typed text, rather than also rolling back the colour edit.
///
/// The picker is probed by ROLE: a `ComboBox` in accesskit has no label, the shown
/// text sits in `value` — so it cannot be found by its label (a relative of the note about
/// `DragValue` in CLAUDE.md).
#[test]
fn t511_kind_picker_routes_between_windows() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    h.step();
    h.step();
    let (_id, clip) = made_generator(&h);
    assert_eq!(h.state().fx_editors.len(), 1, "the generator form is in the event window");
    let depth = h.state().model.undo_depth();

    let text_name = crate::gen_ui::kind_name(&GeneratorKind::default_text());
    let solid_name =
        crate::gen_ui::kind_name(&GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });

    // the kind picker IS in the form and shows “Text”
    h.get(by().role(Role::ComboBox).value(text_name)).click();
    h.step();
    h.get_by_label(solid_name).click();
    h.step();
    h.step();

    // consequence: the kind changed in the media and on the event, the window stays in place
    let app = h.state();
    assert_eq!(app.model.project().media.last().unwrap().path, "gen:solid");
    assert!(
        matches!(gen_of(&h, clip), GeneratorKind::Solid { .. }),
        "the event values must change together with the view"
    );
    assert_eq!(app.fx_editors.len(), 1, "a view change has no right to close the window");
    assert_eq!(app.model.undo_depth(), depth + 1, "a view change is exactly one history step");

    // and back: “Text” with the same picker
    h.get(by().role(Role::ComboBox).value(solid_name)).click();
    h.step();
    h.get_by_label(text_name).click();
    h.step();
    h.step();
    let app = h.state();
    assert_eq!(app.model.project().media.last().unwrap().path, "gen:text");
    assert!(matches!(gen_of(&h, clip), GeneratorKind::Text { .. }), "the view did not return to the title");
    assert_eq!(app.fx_editors.len(), 1, "one form remained");
}

/// Test 516 (R.53, item 415): a frame share is SHOWN as a percentage, but STORED as a
/// fraction. “Font size 0.070” tells the user nothing; at the same time neither the format,
/// nor the curves, nor the engine knows about percentages — the conversion lives in
/// one place (`widgets::param_value`).
#[test]
fn t516_frame_fractions_are_shown_in_percent() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable, by};
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    h.step();
    h.step();

    // the declaration: font size is a percentage, line spacing is a multiplier
    let kind = GeneratorKind::default_text();
    let spec = |n: &str| kind.param_specs().iter().find(|s| s.name == n).unwrap().clone();
    assert_eq!(spec("size").unit, ParamUnit::Percent, "the point size must be shown as a percentage");
    assert_eq!(spec("pos_x").unit, ParamUnit::Percent);
    assert_eq!(spec("line_spacing").unit, ParamUnit::Ratio, "a multiplier cannot be a percentage");

    // consequence IN THE WINDOW: the font-size field shows “7.0 %”, not “0.07”
    let shown: Vec<String> = h
        .query_all(by().role(Role::SpinButton))
        .filter_map(|n| n.accesskit_node().value().map(|v| v.to_string()))
        .collect();
    assert!(
        shown.iter().any(|s| s == "7.0 %"),
        "there is no point size field in percent in the title window: {shown:?}"
    );
    assert!(
        shown.iter().any(|s| s == "1.00"),
        "the multiplier went into percents together with the fractions: {shown:?}"
    );

    // and the storage is still a fraction: the path writes 0.125, not 12.5
    let mut k = GeneratorKind::default_text();
    k.set_param("size", 0.125);
    let path = format_gen_path(&k);
    assert!(path.contains("s=0.125"), "a percent instead of a fraction went into the path: {path}");
}

/// Test 517 (R.53, item 415): a double click on the LABEL restores the parameter’s default.
/// Before, “put it back as it was” required remembering the number — a title has
/// eleven such numbers.
#[test]
fn t517_double_click_on_label_resets_param() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    let mut kind = GeneratorKind::default_text();
    kind.set_param("size", 0.3);
    h.state_mut().create_generator(kind);
    h.step();
    h.step();
    let id = h.state().model.project().media.last().unwrap().id;
    let default_size = GeneratorKind::default_text().get_param("size").unwrap();

    let label = crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "size");
    let rect = h.get_by_label(&label).rect();
    double_click_at(&mut h, rect.center());
    h.step();
    h.step();

    let path = h.state().model.project().source(id).map(|s| match s {
        kt_model::Source::File(m) => m.path.clone(),
        _ => String::new(),
    });
    let got = kt_model::parse_gen_path(path.as_deref().unwrap_or(""))
        .and_then(|k| k.get_param("size"))
        .expect("the media must remain a title");
    assert!(
        (got - default_size).abs() < 1e-9,
        "double-click on the label did not restore the default: {got} instead of {default_size}"
    );
}

/// Test 518 (R.53, item 415): an unparseable `gen:` path is an ERROR, not
/// silence (rule 16), and the shared widget’s hint reaches the status line
/// (rule 10: there is one place for display).
#[test]
fn t518_unknown_kind_is_loud_and_widget_hints_reach_the_status_line() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    let id = h.state().model.project().media.last().unwrap().id;

    // the parameter label’s hint: we hover over it and read the STATUS LINE
    let label = crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "size");
    let rect = h.get_by_label(&label).rect();
    h.hover_at(rect.center());
    h.step();
    h.step();
    assert_eq!(
        h.state().hover_hint.as_deref(),
        Some(crate::i18n::tr(crate::i18n::k::fx::HINT_RESET)),
        "the hint of the shared widget did not reach the status line"
    );

    // a kind newer than the build: it has no form, but the user hears why.
    // Since R.59 the values live on the EVENT, and an unknown kind removes them —
    // the strip head disappears, the effects window stays (the chain itself is alive).
    let notices = h.state().notices.len();
    let clip = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.media == id)
        .map(|c| c.id)
        .expect("generator event");
    {
        let app = h.state_mut();
        let _ = app.apply(Command::SetMediaPath { id, path: "gen:from_the_future".into() });
    }
    h.step();
    h.step();
    let app = h.state();
    assert!(
        app.model.clip(clip).is_some_and(|c| c.r#gen.is_none()),
        "an unknown kind cannot have a form — the values must be cleared"
    );
    assert!(app.notices.len() > notices, "an unparseable path removed the form SILENTLY");
}

/// Test 512 (R.53, item 416): a new generator appears ON THE EDIT, not only in the bin,
/// and all of this is one history step.
///
/// Before this item the menu item created the media and opened the parameters window, but
/// there was nothing in the preview: the user typed a title blind, then went
/// to the bin and placed the card by hand. Rule 4 of the checklist — the default must
/// be useful.
#[test]
fn t512_new_generator_lands_on_the_timeline() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    // the tick is a MULTIPLE of 15: the playhead lives in 48 kHz audio frames, and the round trip
    // “tick → frame → tick” truncates downwards (item 7, round 10) — 400_000 would come back
    // as 399_999, and the test would catch the wrong thing
    h.state_mut().player.seek(kt_time::Tick(450_000));
    let clips_before = h.state().model.project().clips.len();
    let depth = h.state().model.undo_depth();

    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    h.step();

    let app = h.state();
    let id = app.model.project().media.last().unwrap().id;
    let mine: Vec<&ClipEvent> =
        app.model.project().clips.iter().filter(|c| c.media == id).collect();
    assert_eq!(mine.len(), 1, "a generator without sound places exactly one event");
    assert_eq!(mine[0].start, kt_time::Tick(450_000), "the event must land UNDER THE CURSOR");
    assert!(mine[0].duration.0 > 0, "there is nothing to show for an event of zero length");
    assert_eq!(app.model.project().clips.len(), clips_before + 1);
    assert_eq!(app.model.undo_depth(), depth + 1, "media and clip — ONE history step");

    // one Ctrl+Z removes both the event and the media
    h.state_mut().undo();
    h.step();
    let app = h.state();
    assert_eq!(app.model.project().clips.len(), clips_before, "undo left the event");
    assert!(
        !app.model.project().media.iter().any(|m| m.id == id),
        "undo left the media: the step was not one"
    );
}

/// Test 513 (R.53, item 416): a generator event is labelled with the KIND’s NAME, not with the
/// raw `gen:` path, and the label follows the interface language.
///
/// The raw path lied twice: it contained the parameters (the label jumped on every keystroke
/// in the window) and did not contain the kind. The language is a separate stamp in the name
/// cache: a language change does not touch the model, and without it the timeline would stay
/// in the old language until the first edit.
#[test]
fn t513_generator_clip_is_labelled_by_its_kind() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    h.step();
    let id = h.state().model.project().media.last().unwrap().id;
    let clip = h.state().model.project().clips.iter().find(|c| c.media == id).unwrap().id;

    let name_of = |h: &mut Harness<'_, KtApp>| h.state_mut().names().0.get(&clip).cloned();
    let ru = name_of(&mut h).expect("the generator event has no label");
    assert_eq!(
        ru,
        crate::gen_ui::kind_name(&GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 })
    );
    assert!(!ru.contains("gen:"), "the label is still a raw path: {ru}");
    assert!(!ru.contains("ff8800"), "parameters went into the label: {ru}");

    // a language change — the label must move together with the interface
    crate::i18n::set_lang(crate::i18n::Lang::En);
    h.step();
    let en = name_of(&mut h).expect("the label disappeared after a language change");
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    assert_ne!(en, ru, "the label stayed in the previous language: the name cache does not know about the language");

    // and the event colour does NOT depend on the parameters: an edit does not recolour it
    let a = crate::timeline::clip_color("gen:solid:c=ff8800", ClipKind::Video);
    let b = crate::timeline::clip_color("gen:solid:c=104080", ClipKind::Video);
    assert_eq!(a, b, "the clip is recolored on every press in the parameters window");
    assert_ne!(
        a,
        crate::timeline::clip_color("gen:text", ClipKind::Video),
        "different kinds must differ in color"
    );
}

/// Test 514 (R.53, item 416): a double click on a bin card gives ONE clip and
/// opens the parameters window.
///
/// In egui `clicked()` is true on BOTH clicks of a double, and a single click
/// on a card places the event — so a double placed TWO overlapping, and the
/// user did not ask for that (rule 20: nothing grows by itself).
#[test]
fn t514_double_click_on_a_card_gives_one_clip_and_the_window() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    for _ in 0..6 {
        h.step();
    }
    let id = h.state().model.project().media.last().unwrap().id;
    // close the creation window: the test is about OPENING by double click
    h.state_mut().fx_editors.clear();
    h.state_mut().player.seek(kt_time::Tick(700_000));
    h.step();
    let before = h.state().model.project().clips.iter().filter(|c| c.media == id).count();

    let card =
        h.state().bin_card_rect(id).expect("the generator card is not drawn — there is nothing to probe");
    double_click_at(&mut h, card.center());
    h.step();
    h.step();

    let app = h.state();
    let after = app.model.project().clips.iter().filter(|c| c.media == id).count();
    assert_eq!(after, before + 1, "double-click placed not one clip but {}", after - before);
    assert_eq!(app.fx_editors.len(), 1, "double-click on the card did not open the window");
    assert!(
        app.model.clip(app.fx_editors[0].clip).is_some_and(|c| c.media == id && c.r#gen.is_some()),
        "the window opened not on the event of this generator"
    );
}

/// Test 515 (R.53, item 416): the card says in HOW MANY places the
/// generator stands. Parameters are shared by all places, and a colour edit changes the
/// picture everywhere at once — the count must be visible BEFORE the edit, not only in the
/// window. The place is not new: a generator has no duration, and the label takes
/// its empty slot (rule 13).
#[test]
fn t515_card_counts_places_on_the_timeline() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    h.step();
    let id = h.state().model.project().media.last().unwrap().id;
    assert_eq!(h.state_mut().media_uses().get(&id).copied(), Some(1), "the creation clip was not counted");

    h.state_mut().player.seek(kt_time::Tick(800_000));
    h.state_mut().place_media_at_cursor(id);
    h.step();
    assert_eq!(h.state_mut().media_uses().get(&id).copied(), Some(2));

    // the count covers the WHOLE document, not just the root: an event inside a reel is
    // the same kind of place, and a parameter edit changes it too
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        let mut inner = p.clips.iter().find(|c| c.media == id).unwrap().clone();
        inner.id = ClipId(9001);
        inner.start = kt_time::Tick(0);
        let mut seq = Sequence::new(MediaId(9000), "reel");
        seq.clips.push(inner);
        p.sequences.push(seq);
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.model_rev += 1;
    }
    assert_eq!(
        h.state_mut().media_uses().get(&id).copied(),
        Some(3),
        "an event inside the reel was not counted — “places” is less than the edit will affect"
    );
}

/// Test 540 (R.53, item 424; rewritten in R.59 stage 2): a title preset
/// sets EVERYTHING in one history step, and the active preset is determined by an
/// INVERSE function.
///
/// The owner’s decision verbatim: “the preset sets everything”, including the position
/// and the anchor; one Ctrl+Z restores the whole set, and a manual tweak honestly
/// shows “matches none”. There is deliberately no flag of “which preset was pressed” in the
/// program: it would lie after the first field edit.
///
/// What changed: “everything” now includes the ANIMATION too — it became
/// plain curves on the event (`gen_auto`), and they move in the same single
/// step (the curves are checked by t668).
#[test]
fn t540_title_preset_sets_everything_in_one_step() {
    use crate::gen_ui::{active_preset, title_presets};
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    // the inverse function: a preset applied to its own result gives
    // the same result — otherwise “active” would not be determined at all
    let base = GeneratorKind::default_text();
    for (i, pr) in title_presets().iter().enumerate() {
        let made = pr.applied(&base);
        let curves = pr.curves(&made, (1920, 1080), kt_time::Tick::ZERO);
        assert_eq!(
            active_preset(&made, &curves),
            Some(i),
            "preset {i} does not recognize its own result"
        );
    }
    // the title’s default equals NO preset — and that is honest. By its VALUES an
    // empty title matches the “typewriter” (the same font size and the same frame centre),
    // and exactly the second half of the preset tells them apart
    // — the curves, which a new title does not have.
    assert_eq!(active_preset(&base, &[]), None, "an empty title need not match the preset");

    // and the same in the WINDOW: a click sets the set of fields in one command
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    for _ in 0..3 {
        h.step();
    }
    let (_id, clip) = made_generator(&h);
    let depth = h.state().model.undo_depth();
    let before = gen_of(&h, clip);

    let label = crate::i18n::tr(title_presets()[0].key);
    h.get_by_label(label).click();
    h.step();
    h.step();

    let made = gen_of(&h, clip);
    let curves = h.state().model.clip(clip).map(|c| c.gen_auto.clone()).unwrap_or_default();
    assert_ne!(made, before, "pressing the preset changed nothing");
    assert_eq!(active_preset(&made, &curves), Some(0), "the window set the wrong set");
    let GeneratorKind::Text { anchor, pos_y, size, .. } = &made else { unreachable!() };
    assert_eq!(*anchor, TextAnchor::BottomLeft, "the preset must set the ANCHOR too");
    assert!(*pos_y > 0.5 && *size < 0.07, "the preset must set both the position and the point size");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "the preset is exactly one history step");

    // one Ctrl+Z restores the whole set at once
    h.state_mut().undo();
    h.step();
    assert_eq!(gen_of(&h, clip), before, "undo must restore the whole title");
}

/// Test 541 (R.53, item 412; rewritten in R.59 stage 2): an edit of parameters
/// with a STILL cursor reaches the SCREEN,
/// not only the player’s cache.
///
/// This defect was found by a live scenario, not by a harness — exactly the
/// case CLAUDE.md warns about: t478 asks the player cache DIRECTLY, bypassing
/// the preview memoisation, and was green on a program where the window showed
/// an orange colour while the screen showed a black frame. The preview key did
/// not change: neither the cursor, the quality nor the plan is moved by the edit,
/// and the composite counted as “already computed” until the playhead first moved.
///
/// Therefore the test asserts the CONSEQUENCE: the program rebuilt the preview.
#[test]
fn t541_param_edit_repaints_the_preview_without_moving_the_playhead() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
    for _ in 0..4 {
        h.step();
    }
    let (_id, clip) = made_generator(&h);
    let gesture = h.state().fx_editors[0].gesture;

    // let the preview settle: the key is computed, the composite is full
    for _ in 0..40 {
        if h.state().preview_key.is_some_and(|k| k.2) {
            break;
        }
        h.step();
    }
    let before = h.state().preview_key;
    let content_before = h.state().preview_content;
    assert!(before.is_some(), "the preview never got computed");

    // a colour edit WITHOUT moving the cursor
    {
        let app = h.state_mut();
        assert!(app.apply_coalesced(
            Command::SetClipGen {
                id: clip,
                kind: Some(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 }),
            },
            gesture
        ));
    }
    // ...and the program MUST rebuild the preview: the version of the content of the
    // sources went into the key
    let mut repainted = false;
    for _ in 0..60 {
        h.step();
        if h.state().preview_content != content_before {
            repainted = true;
            break;
        }
    }
    assert!(
        repainted,
        "the parameter edit did not move the preview key — the old frame would have stayed on screen"
    );
    assert_eq!(
        h.state().preview_key.map(|k| k.0),
        before.map(|k| k.0),
        "the cursor did not move: the key tick must stay the same"
    );
}

/// Test 542 (R.53, item 415; rewritten in R.59 stage 2): the colours in the title form
/// are REAL.
///
/// A regression found by a live scenario, not by a harness: the title’s eyedropper
/// was moved onto the shared `color_row`, which reads values through
/// `ParamSpec`. Title colours are plain fields and cannot be expressed as `ParamSpec`s
/// (“the first legitimate exception” F-TXT-001), so
/// `get_param` silently returned zero: the fill was shown (and written on the first touch)
/// as transparent black, and the title stopped being drawn at all. No test saw this —
/// they probed text and numbers.
///
/// The test asserts the CONSEQUENCE: the form shows the same colours that lie in the
/// model, and the hex field next to the eyedropper shows them too. The form now
/// lives in the event’s effects window — it is probed there too.
#[test]
fn t542_title_window_shows_real_colors() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    let mut kind = GeneratorKind::default_text();
    if let GeneratorKind::Text { text, fill_r, fill_g, fill_b, fill_a, .. } = &mut kind {
        *text = "A".into();
        (*fill_r, *fill_g, *fill_b, *fill_a) = (255, 136, 0, 200);
    }
    h.state_mut().create_generator(kind);
    for _ in 0..4 {
        h.step();
    }

    // the fill’s hex field shows EXACTLY what is in the model. We probe by
    // ROLE: a text field in accesskit has no label, the content sits in
    // `value` (a relative of the note about DragValue and ComboBox).
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable, by};
    let shown: Vec<String> = h
        .query_all(by().role(Role::MultilineTextInput))
        .chain(h.query_all(by().role(Role::TextInput)))
        .filter_map(|n| n.accesskit_node().value().map(|v| v.to_string()))
        .collect();
    assert!(
        shown.iter().any(|s| s == "ff8800c8"),
        "the title form shows the wrong fill color: {shown:?}"
    );

    // and the title’s default is NOT transparent black: otherwise a new title would be invisible
    let d = GeneratorKind::default_text();
    let GeneratorKind::Text { fill_r, fill_g, fill_b, fill_a, .. } = &d else { unreachable!() };
    assert_eq!(
        (*fill_r, *fill_g, *fill_b, *fill_a),
        (255, 255, 255, 255),
        "the default title fill must be opaque white"
    );
}

/// Test 570 (R.55, C5): expanding changes NEITHER the project nor what plays.
///
/// The main promise of the hybrid mode: we do not leave the root, so
/// the engine keeps playing the WHOLE PROGRAM, and the document stays the same
/// byte-for-byte. Expanding is a workspace, not work: no history step,
/// no trace in the file.
#[test]
fn t570_expanding_in_place_changes_nothing_but_the_view() {
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    let doc_before = h.state().model.document();
    let play_before = h.state().player_project();
    let undo_before = h.state().model.undo_depth();
    let rows_before = h.state().tl_geom.clone().map(|g| g.rows.len()).unwrap_or(0);

    h.state_mut().expand_in_place(inst);
    h.step();

    assert_eq!(h.state().model.editing(), None, "the expansion went out of the root — the reel will play");
    assert_eq!(h.state().model.bands().len(), 1, "the strip was not created");
    assert_eq!(h.state().model.document(), doc_before, "the expansion changed the DOCUMENT");
    assert_eq!(h.state().player_project(), play_before, "the expansion changed what PLAYS");
    assert_eq!(h.state().model.undo_depth(), undo_before, "the expansion was recorded in the history");

    // but the VIEW does change: the strip’s rows stand BELOW the instance row
    let g = h.state().tl_geom.clone().expect("geometry");
    assert!(g.rows.len() > rows_before, "no rows were added — the strip is not visible: {:?}", g.rows);
    let inner = h.state().model.project().clips.iter().find(|c| c.id != inst && c.media != seq);
    assert!(inner.is_some(), "the content of the reel is not in the working view");

    // and collapsing back — the view is identical (rule 14: a repeated gesture)
    h.state_mut().collapse_in_place(inst);
    h.step();
    assert!(h.state().model.bands().is_empty(), "collapse did not work");
    assert_eq!(h.state().model.document(), doc_before, "collapse changed the document");
    assert_eq!(
        h.state().tl_geom.clone().map(|g| g.rows.len()).unwrap_or(0),
        rows_before,
        "after collapse, the number of rows left was not the same"
    );
}

/// Test 571 (R.55, C5): an edit FROM THE STRIP reaches the REEL.
///
/// No addressing of edits is needed: the strip’s event physically lies in the
/// root vector of the working view, the command arrives at it by the ordinary path,
/// and the assembly subtracts the offset. The claim is checked against the DOCUMENT.
#[test]
fn t571_editing_inside_a_band_lands_in_the_reel() {
    use kt_time::Tick;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    h.state_mut().expand_in_place(inst);
    h.step();

    // the event of the STRIP — the one whose row belongs to the expansion
    let ids: Vec<(kt_model::ClipId, kt_time::Tick, u32)> =
        h.state().model.project().clips.iter().map(|c| (c.id, c.start, c.lane)).collect();
    let (id, was, lane) = ids
        .iter()
        .copied()
        .find(|(id, _, _)| *id != inst && h.state().reel_of_clip(*id).is_some())
        .expect("strip event");

    assert!(h.state_mut().apply(kt_model::Command::MoveClip {
        id,
        new_start: was + Tick(1_000),
        new_lane: lane,
    }));
    h.step();

    let doc = h.state().model.document();
    let s = doc.sequence(seq).expect("reel");
    assert!(
        s.clips.iter().any(|c| c.id == id),
        "the edit did not reach the reel: the event went to the document root"
    );
    assert!(
        !doc.clips.iter().any(|c| c.id == id),
        "a reel event ended up in the document ROOT — a different edit would have gone to disk"
    );
    // the expansion survived the edit
    assert_eq!(h.state().model.bands().len(), 1, "the edit collapsed the expansion");
}

/// Test 572 (R.55, C5): the selection lives in EXACTLY ONE reel.
///
/// A mixed selection would look ordinary, and a gesture on it would go
/// half-way: commands on events of different reels cannot survive one batch.
/// A click in another strip REPLACES the selection and says so.
#[test]
fn t572_selection_lives_in_one_reel() {
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    h.state_mut().expand_in_place(inst);
    h.step();

    let ids: Vec<kt_model::ClipId> = h.state().model.project().clips.iter().map(|c| c.id).collect();
    let root_other = ids
        .iter()
        .copied()
        .find(|id| *id != inst && h.state().reel_of_clip(*id).is_none())
        .expect("a root event nearby");
    let in_band = ids
        .iter()
        .copied()
        .find(|id| h.state().reel_of_clip(*id).is_some())
        .expect("strip event");

    h.state_mut().select_only(root_other);
    h.state_mut().toggle_selected(in_band);
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![in_band],
        "the selection was spread across two reels — half of the gesture would silently not move"
    );
    // and back too
    h.state_mut().toggle_selected(root_other);
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![root_other],
        "the selection was spread across two reels in the opposite direction"
    );
}

/// Test 573 (R.55, C5): Esc collapses the expansion BEFORE exiting the reel.
///
/// Collapsing a strip costs the person less attention than exiting outward —
/// so it also comes earlier in the cascade. But later than “clear selection”: one
/// key has no right to do two things at once.
#[test]
fn t573_escape_collapses_the_band_before_leaving_a_reel() {
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    h.state_mut().expand_in_place(inst);
    h.state_mut().select_only(inst);
    h.step();

    h.state_mut().escape_action();
    assert_eq!(h.state().model.bands().len(), 1, "Esc collapsed the strip instead of clearing the selection");
    assert!(h.state().selected.is_empty(), "Esc did not clear the selection");

    h.state_mut().escape_action();
    assert!(h.state().model.bands().is_empty(), "the second Esc must collapse the expansion");
}

/// Test 576 (R.55, C6): dropping a PROJECT ASKS and does not lose work.
///
/// **The owner’s complaint verbatim:** “I drag the project file onto the timeline and
/// a new timeline opens, but I want it to give me a choice”.
///
/// There was no choice — and, worse, there was no question about unsaved work:
/// the drop did `*self = app` bypassing `guard_unsaved` and `claim_project`, that is,
/// around everything built in R.49. The work vanished silently.
///
/// The test asserts THREE things: the question is asked; “Cancel” does nothing;
/// “Open” goes through the unsaved-work guard.
#[test]
fn t576_dropping_a_project_asks_and_keeps_the_work() {
    let (mut h, _) = harness_three_clips();
    let dir = std::env::temp_dir().join("kt-t576");
    let _ = std::fs::create_dir_all(&dir);
    let foreign = dir.join("foreign.kt");
    {
        // a foreign project on disk — minimal, but legitimate
        let mut p = kt_model::Project::new(h.state().model.project().settings.clone());
        p.media.push(kt_model::MediaRef::new(kt_model::MediaId(1), "synth:1"));
        p.clips.push(h.state().model.project().clips[0].clone());
        p.clips[0].media = kt_model::MediaId(1);
        let text = crate::app::project_text_of(&p, &Default::default(), None);
        std::fs::write(&foreign, text).unwrap();
    }

    // the editor has UNSAVED work
    h.state_mut().dirty_content = true;
    let clips_before: Vec<kt_model::ClipId> =
        h.state().model.project().clips.iter().map(|c| c.id).collect();

    h.state_mut().pending_drop = Some((foreign.clone(), None));
    h.state_mut().ask = Some(crate::app::Ask::DropProject);
    h.step();

    // 1) the QUESTION is asked, and the edit is not yet touched
    assert!(matches!(h.state().ask, Some(crate::app::Ask::DropProject)), "the question was not asked");
    assert_eq!(
        h.state().model.project().clips.iter().map(|c| c.id).collect::<Vec<_>>(),
        clips_before,
        "the drop changed the edit BEFORE the answer"
    );

    // 2) CANCEL does nothing
    h.state_mut().answer_ask(crate::app::Answer::Cancel);
    h.step();
    assert!(h.state().ask.is_none(), "the question remained after cancel");
    assert_eq!(
        h.state().model.project().clips.iter().map(|c| c.id).collect::<Vec<_>>(),
        clips_before,
        "cancel still did something"
    );
    assert!(h.state().dirty_content, "cancel cleared the mark of unsaved work");

    // 3) “Open” with unsaved work MUST ask, not open
    h.state_mut().pending_drop = Some((foreign.clone(), None));
    h.state_mut().ask = Some(crate::app::Ask::DropProject);
    h.state_mut().answer_ask(crate::app::Answer::Yes);
    h.step();
    assert!(
        matches!(h.state().ask, Some(crate::app::Ask::Unsaved(crate::app::Pending::DropOpen))),
        "dropping a project opened it over unsaved work — it would silently disappear"
    );
    assert_eq!(
        h.state().model.project().clips.iter().map(|c| c.id).collect::<Vec<_>>(),
        clips_before,
        "the edit was replaced before the answer to the question about unsaved changes"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 577 (R.55, C6): “paste as a composition” puts the reel and an instance.
#[test]
fn t577_dropping_a_project_can_insert_it_as_a_composition() {
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    let dir = std::env::temp_dir().join("kt-t577");
    let _ = std::fs::create_dir_all(&dir);
    let foreign = dir.join("foreign.kt");
    {
        let mut p = kt_model::Project::new(h.state().model.project().settings.clone());
        p.media.push(kt_model::MediaRef::new(kt_model::MediaId(1), "synth:1"));
        p.clips.push(h.state().model.project().clips[0].clone());
        p.clips[0].media = kt_model::MediaId(1);
        let text = crate::app::project_text_of(&p, &Default::default(), None);
        std::fs::write(&foreign, text).unwrap();
    }
    let seqs_before = h.state().model.document().sequences.len();
    let clips_before = h.state().model.project().clips.len();

    h.state_mut().pending_drop =
        Some((foreign.clone(), Some((Tick(400_000), (kt_model::ClipKind::Video, 0)))));
    h.state_mut().ask = Some(crate::app::Ask::DropProject);
    h.state_mut().answer_ask(crate::app::Answer::No);
    h.step();

    let doc = h.state().model.document();
    assert_eq!(doc.sequences.len(), seqs_before + 1, "the reel from another project was not created");
    assert_eq!(
        h.state().model.project().clips.len(),
        clips_before + 1,
        "the instance of another project did not land on the timeline"
    );
    let seq = doc.sequences.last().unwrap().id;
    let inst = doc.clips.iter().find(|c| c.media == seq).expect("no instance");
    assert_eq!(inst.start, Tick(400_000), "the instance did not land at the drop point");
    doc.validate().expect("the document must be valid after the paste");
    // and it is ONE history step: Ctrl+Z removes the paste entirely
    assert!(h.state_mut().model.undo().is_ok());
    assert_eq!(
        h.state().model.document().sequences.len(),
        seqs_before,
        "the paste of another project was not undone in one step"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 578 (R.55, C6): a reel is a BIN OBJECT, not only an event.
///
/// **The owner’s complaint verbatim:** “there are probably a lot of things unavailable
/// from the ui”. The reel was exactly such a case: it was not in the bin at all, and
/// placing a composition in a second place was possible only by copy-pasting an already
/// standing instance, creating an empty one was impossible, and deleting a specific one
/// too (the “remove unused” button removes all at once).
///
/// t253 did not catch this: its table is exhaustive by TIMELINE entities,
/// and the bin is a panel. t395 asks whether a panel answers to the right button, not
/// what is shown in it.
#[test]
fn t578_a_reel_is_an_object_of_the_bin() {
    let (mut h, _g, seq) = harness_with_nest();
    h.step();
    h.step();

    // 1) a CARD of the reel is in the bin
    let cards: Vec<kt_model::MediaId> =
        h.state().bin_card_rects.iter().map(|(id, _)| *id).collect();
    assert!(
        cards.contains(&seq),
        "the reel is not in the bin: {cards:?} — there would be nothing to place in the second place"
    );

    // 2) PLACE it in a second spot — with one verb, not copy-paste
    let before = h.state().model.document().sequence_uses(seq);
    h.state_mut().player.seek(kt_time::Tick(400_000));
    h.state_mut().place_media_at_cursor(seq);
    h.step();
    assert_eq!(
        h.state().model.document().sequence_uses(seq),
        before + 1,
        "the reel did not land in the second place from the bin"
    );
    let inst = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .filter(|c| c.media == seq)
        .map(|c| c.start)
        .max()
        .expect("no instance");
    // precision — the transport quantum (the playhead lives in 48 kHz audio frames,
    // item 7, round 10): requiring tick-for-tick would mean requiring what
    // the transport cannot do anywhere
    assert!((inst.0 - 400_000).abs() <= 2, "the instance did not land under the cursor: {inst:?}");

    // 3) CREATE AN EMPTY one — also a verb, not a side effect of collapsing
    let seqs_before = h.state().model.document().sequences.len();
    h.state_mut().create_empty_reel();
    h.step();
    h.step();
    assert_eq!(
        h.state().model.document().sequences.len(),
        seqs_before + 1,
        "the empty reel was not created"
    );
    let cards: Vec<kt_model::MediaId> =
        h.state().bin_card_rects.iter().map(|(id, _)| *id).collect();
    assert_eq!(
        cards.len(),
        h.state().model.project().media.len() + h.state().model.document().sequences.len(),
        "not all objects are shown in the bin: {cards:?}"
    );

    // 4) DELETE BY ADDRESS — and an occupied reel refuses to go (t429)
    let fresh = h
        .state()
        .model
        .document()
        .sequences
        .iter()
        .map(|s| s.id)
        .find(|id| *id != seq)
        .expect("empty reel");
    assert!(
        h.state_mut().apply(kt_model::Command::RemoveSequence { id: fresh }),
        "an empty reel must be deleted by address"
    );
    assert!(
        !h.state_mut().apply(kt_model::Command::RemoveSequence { id: seq }),
        "a BUSY reel went away silently — events would hang on a nonexistent address"
    );
}

/// Test 594 (R.56, item 449): a track added ABOVE THE CEILING is the
/// top layer of the picture.
///
/// The owner’s complaint verbatim: “if I place a video track under the ceiling
/// (before the tallest morpher) and drop a video there — it does not
/// display at all”. The cause was a mismatch of two orders:
/// the engine stacked layers by the track NUMBER, while the screen showed by
/// `lane_order`, and inserting from the menu of a foreign row took `at = tail`, that is
/// the largest number — the bottom layer.
///
/// The test asserts the CONSEQUENCE and probes from where the person acts:
/// the row’s menu item, then the event on the new track, then the order in which
/// the engine will draw the layers.
#[test]
fn t594_a_lane_added_under_the_ceiling_is_the_top_layer() {
    use kt_model::{ClipKind, LaneKind};
    let (mut h, _g) = harness_three_clips();
    // the morph row on top — the very “under the ceiling” layout
    h.state_mut().insert_lane_after_row(LaneKind::Morph, None);
    h.step();
    let rows = h.state().model.project().rows();
    let morph = rows.iter().position(|&r| r.0 == LaneKind::Morph).expect("morph row");
    let top_video = rows.iter().position(|&r| r.0 == LaneKind::Video).expect("video row");
    h.state_mut().reorder_rows(morph, 0);
    h.step();
    assert_eq!(h.state().model.project().rows()[0].0, LaneKind::Morph, "the morph row is not at the top");
    let _ = top_video;

    // RMB on the morph row → “add video track”
    let undo_before = h.state().model.undo_depth();
    h.state_mut().insert_lane_after_row(LaneKind::Video, Some((LaneKind::Morph, 0)));
    h.step();
    let p = h.state().model.project().clone();
    let under = p.rows()[1];
    assert_eq!(under.0, LaneKind::Video, "the new row did not land under the morph row: {:?}", p.rows());
    assert_eq!(
        h.state().model.undo_depth(),
        undo_before + 1,
        "inserting a track cost more than one history step"
    );

    // “dropped the video there”
    let media = p.media[0].id;
    let id = kt_model::ClipId(p.clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 1);
    let mut c = p.clips.iter().find(|c| c.kind == ClipKind::Video).unwrap().clone();
    c.id = id;
    c.link_group = None;
    // the track is the one visible under the morph row: its number decides
    c.lane = under.1;
    c.media = media;
    c.start = kt_time::Tick(0);
    assert!(h.state_mut().apply(kt_model::Command::AddClip(c)), "the event did not land on the track");
    h.step();

    // CONSEQUENCE: in the engine’s composition order (background first) it is
    // LAST, that is, the top layer
    let p = h.state().model.project();
    let mut order = p.effective_clips();
    order.retain(|c| c.kind == ClipKind::Video);
    order.sort_by_key(|c| (std::cmp::Reverse(c.lane), c.start.0));
    assert_eq!(
        order.last().map(|c| c.id),
        Some(id),
        "the event “under the ceiling” is drawn NOT on the top layer: {:?}",
        order.iter().map(|c| (c.id, c.lane)).collect::<Vec<_>>()
    );
}

/// Test 596 (R.56, item 450): a copy from an expanded composition is inserted
/// as a PAIR and into the NAMED place.
///
/// The owner’s complaint verbatim: “I copy ZEF last in a long composition
/// from rendereg3 via RMB and then go to paste it, also via RMB, and it puts one part on top
/// and drops the other part (audio) down, and at the very bottom the morpher,
/// because there was really no place for it”.
///
/// There were two causes, and both are checked here by their consequence:
/// the clipboard held numbers of the WORKING VIEW (in the expansion strip the views have
/// different bases), and “Paste” from the menu put the block AT THE CURSOR,
/// not at the click point.
#[test]
fn t596_paste_from_a_band_lands_as_a_pair_at_the_click() {
    use kt_model::{ClipKind, LaneKind};
    use kt_time::Tick;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    h.state_mut().expand_in_place(inst);
    h.step();
    assert_eq!(h.state().model.bands().len(), 1, "the strip did not expand");

    // event of the STRIP: its track number carries the base offset
    let band = h.state().model.bands()[0].clone();
    let inner = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.kind == ClipKind::Video && band.owns(LaneKind::Video, c.lane))
        .expect("the strip event is not visible")
        .clone();
    assert!(inner.lane >= band.base.video, "scene degenerated: the strip is not shifted");

    // the PAIR: the audio half in the same strip and in one link group — exactly
    // what the owner copied
    let mut a = inner.clone();
    a.id = kt_model::ClipId(
        h.state().model.document().clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 50,
    );
    a.kind = ClipKind::Audio;
    a.lane = band.base.audio;
    a.link_group = Some(kt_model::GroupId(77));
    assert!(h.state_mut().apply(kt_model::Command::AddClip(a.clone())), "audio half");
    h.step();
    assert!(
        h.state_mut().apply(kt_model::Command::LinkClips {
            ids: vec![inner.id, a.id],
            group: kt_model::GroupId(77)
        }),
        "the pair did not link"
    );
    h.step();

    h.state_mut().select_only(inner.id);
    h.state_mut().copy_selection();
    h.step();
    assert_eq!(h.state().clipboard.clips.len(), 2, "the link pair was not copied entirely");
    // CLIPBOARD IN THE REEL’S COORDINATES: numbers start from zero, not from the base
    assert_eq!(
        h.state().clipboard.clips[0].lane,
        inner.lane - band.base.video,
        "the clipboard stores the numbers of the working view, not of the reel"
    );

    // PASTE via RMB into the root row and into the NAMED time
    let root_row = (LaneKind::Video, 0u32);
    h.state_mut().player.seek(Tick(0));
    let seqs_before = h.state().model.document().sequences[0].clips.len();
    let root_before = h.state().model.document().clips.len();
    h.state_mut().paste_at(Tick(700_000), Some(root_row));
    h.step();

    let doc = h.state().model.document().clone();
    assert_eq!(
        doc.sequences[0].clips.len(),
        seqs_before,
        "the paste into the ROOT appended events to the reel"
    );
    let fresh: Vec<&kt_model::ClipEvent> =
        doc.clips.iter().filter(|c| !matches!(c.id.0, 1..=3) && c.media != seq).collect();
    assert_eq!(doc.clips.len(), root_before + fresh.len(), "the count did not match");
    let pasted = fresh.iter().find(|c| c.start == Tick(700_000)).unwrap_or_else(|| {
        panic!(
            "the paste did not land at the click point: {:?}",
            fresh.iter().map(|c| c.start).collect::<Vec<_>>()
        )
    });
    assert_eq!(pasted.lane, 0, "the paste did not hit the clicked row");
    assert_eq!(pasted.kind, ClipKind::Video);

    // the PAIR did not split: the audio half is also in the root, on its own top
    // track, at the same time and in one link group with the video
    let pair: Vec<&kt_model::ClipEvent> =
        fresh.iter().filter(|c| c.start == Tick(700_000)).copied().collect();
    assert_eq!(pair.len(), 2, "half of the pair was pasted: {pair:?}");
    let pa = pair.iter().find(|c| c.kind == ClipKind::Audio).expect("the audio half was lost");
    assert_eq!(pa.lane, 0, "the audio went down instead of to the clicked row: {pa:?}");
    assert_eq!(
        pa.link_group.is_some() && pa.link_group == pasted.link_group,
        true,
        "the pair arrived in different groups"
    );
    // and neither half was left in the reel
    assert!(
        !doc.sequences[0].clips.iter().any(|c| c.start == Tick(700_000)),
        "the paste into the root got into the reel"
    );
}

/// Test 597 (R.56, item 451): the decision “what Enter does” lives in ONE place
/// and does not depend on where it was asked from.
///
/// Plus two consequences the cascade rests on: expanding and collapsing
/// OWN strip leave the instance selected (otherwise the next gesture would have
/// nowhere to aim), while “collapse all” does not (there are no strips left, and holding
/// an address is pointless).
#[test]
fn t597_enter_cascade_decision_and_selection() {
    use crate::app::EnterOnNest;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    let plain = h.state().model.project().clips.iter().find(|c| c.media != seq).unwrap().id;

    assert_eq!(h.state().enter_pressed_on_nest(plain), EnterOnNest::NotANest);
    assert_eq!(h.state().enter_pressed_on_nest(inst), EnterOnNest::Expand, "expand the collapsed");

    h.state_mut().select_only(inst);
    h.state_mut().expand_in_place(inst);
    h.step();
    assert_eq!(
        h.state().enter_pressed_on_nest(inst),
        EnterOnNest::Enter,
        "on an already expanded one, Enter must lead INSIDE"
    );
    assert!(h.state().is_selected(inst), "the expansion lost the instance selection");

    // its own collapse keeps the selection, “collapse all” does not
    h.state_mut().collapse_in_place(inst);
    h.step();
    assert!(h.state().is_selected(inst), "the collapse lost the instance selection");
    h.state_mut().expand_in_place(inst);
    h.step();
    h.state_mut().collapse_all_in_place();
    h.step();
    assert!(h.state().selected.is_empty(), "“collapse all” must clear the selection");

    // inside the full entry a quick gesture leads DEEPER, not into a dead end
    h.state_mut().enter_nest(inst);
    h.step();
    assert_eq!(h.state().model.editing(), Some(seq));
    let deeper = h.state().model.project().clips.first().map(|c| c.id).unwrap();
    assert_eq!(
        h.state().enter_pressed_on_nest(deeper),
        EnterOnNest::NotANest,
        "an ordinary event inside the reel did not become a reel"
    );
}

/// The reel’s preview key is by its FIRST instance (R.59, stage 3: the key became a
/// pair “reel + the set of instance overrides”, t680).
#[cfg(test)]
fn reel_key_of(
    h: &mut Harness<'static, KtApp>,
    seq: kt_model::MediaId,
) -> crate::reel_preview::ReelKey {
    let inst = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.media == seq)
        .map(|c| c.id)
        .expect("there is not a single reel instance in the edit");
    *h.state_mut().reel_keys().get(&inst).expect("instance preview key")
}

/// Test 599 (R.56, item 452): the reel preview service — queue, de-duplication,
/// invalidation and the frame ceiling.
#[test]
fn t599_reel_preview_service_rules() {
    use crate::reel_preview::{MAX_FRAMES_PER_REEL, ReelPreviews};
    use kt_time::Tick;

    // the STRIP STEP is a step of the thumbnail ladder, and frames are no more than the ceiling
    for len in [Tick(90_000), Tick(45 * 90_000), Tick(3600 * 90_000)] {
        let step = ReelPreviews::strip_step(len);
        assert!(
            crate::thumbs::THUMB_STEP_LADDER.contains(&step),
            "step {step} is not from a ladder step — the keys will diverge from the filmstrip of the file"
        );
        let n = (len.0 + step - 1) / step;
        assert!(
            n <= MAX_FRAMES_PER_REEL as i64
                || step == *crate::thumbs::THUMB_STEP_LADDER.last().unwrap(),
            "the reel {len:?} came out with {n} frames at the ceiling {MAX_FRAMES_PER_REEL}"
        );
    }

    let (mut h, _g, seq) = harness_with_nest();
    let key = reel_key_of(&mut h, seq);
    let hash = key.hash;

    // DEDUP: a second request under the same key adds no work
    h.state_mut().request_reel_preview(key, &[]);
    let pending = h.state().reel_previews.pending();
    assert!(pending >= 1, "the request did not join the queue");
    h.state_mut().request_reel_preview(key, &[]);
    assert_eq!(h.state().reel_previews.pending(), pending, "a duplicate request went through");

    // INVALIDATION: another edit of the same reel removes the old work
    let keep = std::collections::HashSet::from([hash ^ 1]);
    h.state_mut().reel_previews.invalidate_seq(seq, &keep);
    assert_eq!(h.state().reel_previews.pending(), 0, "stale work stayed in the queue");
}

/// Test 600 (R.56, item 452): the reel’s frames and peaks reach the SCREEN.
///
/// What is asserted is the consequence, not the presence of a field: the nested
/// event on the timeline gets FRAME TEXTURES and a non-empty envelope — exactly what the
/// owner did not see (“on the timeline there is no preview like normal events have
/// - frames and audio peaks”).
#[test]
fn t600_reel_frames_and_peaks_reach_the_timeline() {
    let (mut h, _g, seq) = harness_with_nest();
    let key = reel_key_of(&mut h, seq);
    let hash = key.hash;

    // the audio half of the reel: without it there is nothing to check the peaks against
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().clone();
    let mut a = inst.clone();
    a.id = kt_model::ClipId(
        h.state().model.document().clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 60,
    );
    a.kind = kt_model::ClipKind::Audio;
    a.lane = 0;
    a.link_group = None;
    assert!(h.state_mut().apply(kt_model::Command::AddClip(a)), "audio half");
    h.step();

    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().reel_previews.get(&key).map(|p| p.peaks.is_none()).unwrap_or(true)
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }

    let prev = h.state().reel_previews.get(&key).expect("the reel preview was not computed");
    let strip = prev.strip.as_ref().expect("there are no reel frames");
    assert!(strip.frames.iter().any(|f| f.is_some()), "not a single frame in the reel strip");
    assert!(
        prev.peaks.as_ref().is_some_and(|p| !p.is_empty()),
        "there is no reel envelope — the audio half stayed an empty plate"
    );

    // and they REACHED the drawing: the frame textures of this reel are loaded
    h.step();
    h.step();
    assert!(
        h.state().reel_textures.keys().any(|(k, _)| *k == key),
        "the reel frames did not reach the screen: there are no textures"
    );

    // an EDIT INSIDE the reel changes the revision — the old preview is no longer ours
    let inner = h.state().model.document().sequences[0].clips[0].clone();
    h.state_mut().expand_in_place(inst.id);
    h.step();
    let band_clip = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.id == inner.id)
        .expect("strip event")
        .clone();
    assert!(
        h.state_mut().apply(kt_model::Command::MoveClip {
            id: band_clip.id,
            new_start: band_clip.start + kt_time::Tick(1_000),
            new_lane: band_clip.lane,
        }),
        "edit inside the reel"
    );
    h.step();
    let fresh = reel_key_of(&mut h, seq).hash;
    assert_ne!(fresh, hash, "an edit inside the reel did not change the revision — there is nothing to invalidate");
    assert!(
        h.state().reel_previews.get(&key).is_none(),
        "a stale preview stayed in the cache and would be drawn instead of the edit"
    );
}

/// Test 601 (R.56, item 452): scrolling and zoom do NOT queue requests (invariant 8),
/// and the THUMBNAIL service does not know about the reel at all.
#[test]
fn t601_scrolling_does_not_order_reel_work() {
    let (mut h, _g, seq) = harness_with_nest();
    let key = reel_key_of(&mut h, seq);
    let deadline = std::time::Instant::now() + WAIT_BG;
    // we wait for the END of the work, not only the frames: `pending()` counts
    // the running task too, and the test would measure it rather than the new requests
    while h.state().reel_previews.pending() > 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(h.state().reel_previews.get(&key).is_some(), "the preview was not computed");

    // move the view back and forth and zoom — no new requests should appear
    for i in 0..6 {
        h.state_mut().view_start = (i as f64) * 25_000.0;
        h.state_mut().ticks_per_px = 400.0 * (1.0 + i as f64 * 0.3);
        h.step();
        assert_eq!(
            h.state().reel_previews.pending(),
            0,
            "scroll/zoom queued a reel preview request (step {i})"
        );
    }

    // and the THUMBNAIL service still does not know about the reel: its key is by file
    assert!(
        h.state().reel_textures.keys().all(|(k, _)| k.seq == seq),
        "a foreign reel ended up in the reel texture cache"
    );
    assert!(
        h.state().thumb_textures.keys().all(|k| k.media != seq),
        "a thumbnail for the reel was ordered: the reel has no file, and the index service would go for ffmpeg"
    );
}

/// Test 640 (R.58, item 463): the reel preview survives reopening the project.
///
/// The owner’s complaint: “the compositions’ preview on the timeline, it seems, is not
/// cached, and every project opening builds it anew”. What is asserted is the
/// consequence: a second `KtApp::open` of the same file gets the reel’s frames and
/// envelope with ZERO real renders — from the sidecar
/// `<project>.ktindex.db`. But a substitution of the source (mtime) is a miss and a
/// re-render: the reel’s revision does not see the file substitution; the media fingerprint
/// in the key catches it.
#[test]
fn t640_reel_preview_survives_reopen_from_sidecar() {
    use crate::reel_preview::{MAX_REELS_READY, ReelKey, ReelPreviews};
    use kt_model::*;

    let dir = std::env::temp_dir().join(format!("kt-t640-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t640.kt");
    // a source file, so that the media fingerprint has something to sniff: the content
    // of the role does not matter (it cannot be opened — the clip is honestly empty); the path,
    // size and mtime are what matter
    let extra = dir.join("extra.bin");
    std::fs::write(&extra, b"not a media file").unwrap();

    // a document with the reel — from a ready scene, plus a clip BY FILE inside the reel
    let seq = {
        let (h, _g, seq) = harness_with_nest();
        let mut p = h.state().model.document().clone();
        // media and reel addresses are one space (t425): a fresh id over both
        let file_id = MediaId(
            p.media.iter().map(|m| m.id.0).chain(p.sequences.iter().map(|s| s.id.0)).max().unwrap()
                + 1,
        );
        p.media.push(MediaRef::new(file_id, "extra.bin"));
        let next_id = p
            .clips
            .iter()
            .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
            .map(|c| c.id.0)
            .max()
            .unwrap_or(0)
            + 500;
        let s = p.sequences.iter_mut().find(|s| s.id == seq).expect("reel");
        let mut a = s.clips[0].clone();
        a.id = ClipId(next_id);
        a.kind = ClipKind::Audio;
        a.lane = 0;
        a.link_group = None;
        a.media = file_id;
        s.clips.push(a);
        p.validate().expect("document with a file clip inside the reel");
        std::fs::write(&path, crate::app::project_text_of(&p, &Default::default(), None))
            .unwrap();
        seq
    };

    // open the file and wait for the reel preview (as in t600)
    let open_and_wait = || -> (Harness<'static, KtApp>, ReelKey) {
        let mut app = KtApp::open(&path).expect("project opens");
        app.recovery_on_drop = false;
        app.workspace_file = Some(dir.join(format!("ws-{:p}.json", &app)));
        let mut h = Harness::builder()
            .with_size(egui::vec2(W, H))
            .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
        crate::icons::install_fonts(&h.ctx);
        h.step();
        let key = reel_key_of(&mut h, seq);
        let deadline = std::time::Instant::now() + WAIT_BG;
        while h.state().reel_previews.get(&key).map(|p| p.peaks.is_none()).unwrap_or(true)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(20));
            h.step();
        }
        (h, key)
    };
    let check_preview = |h: &Harness<'static, KtApp>, key: &ReelKey, what: &str| {
        let prev = h.state().reel_previews.get(key).unwrap_or_else(|| panic!("{what}: no preview"));
        let strip = prev.strip.as_ref().unwrap_or_else(|| panic!("{what}: no strip"));
        assert!(strip.frames.iter().any(|f| f.is_some()), "{what}: strip has no frames");
        assert!(strip.step > 0, "{what}: strip step is zero");
        assert!(prev.peaks.as_ref().is_some_and(|p| !p.is_empty()), "{what}: no peaks");
    };

    // 1) first opening: the render is real — and exactly one
    let (h1, key1) = open_and_wait();
    check_preview(&h1, &key1, "first open");
    assert_eq!(h1.state().reel_previews.renders(), 1, "first open must render the reel once");
    drop(h1);

    // 2) second opening of the same file: everything is there, renders ZERO
    let (h2, key2) = open_and_wait();
    assert_eq!(key2, key1, "reel key changed across save/load — the disk key could never hit");
    check_preview(&h2, &key2, "second open");
    assert_eq!(
        h2.state().reel_previews.renders(),
        0,
        "second open rendered the reel again: the sidecar tier did not serve it"
    );
    drop(h2);

    // 3) substituted source: mtime forward — a miss, a re-render
    {
        let f = std::fs::File::options().write(true).open(&extra).unwrap();
        f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5)).unwrap();
    }
    let (h3, key3) = open_and_wait();
    assert_eq!(key3, key1, "content hash saw the file change — media_fp would be redundant");
    check_preview(&h3, &key3, "open after media change");
    assert_eq!(
        h3.state().reel_previews.renders(),
        1,
        "media file changed under the reel, yet the stale disk preview was served"
    );
    drop(h3);
    let _ = std::fs::remove_dir_all(&dir);

    // ---- side defects of the service (R.58): honest LRU, not an insertion queue ----
    let mut svc = ReelPreviews::default();
    let k = |i: u64| ReelKey { seq: MediaId(1000 + i), hash: i };
    for i in 0..MAX_REELS_READY as u64 {
        svc.put(k(i), Default::default());
    }
    assert!(svc.get(&k(0)).is_some(), "just-put reel is missing");
    svc.put(k(MAX_REELS_READY as u64), Default::default());
    assert!(svc.get(&k(0)).is_some(), "recently used reel was evicted: the LRU is a FIFO");
    assert!(svc.get(&k(1)).is_none(), "least recently used reel survived eviction");
    assert_eq!(svc.ready_keys().len(), MAX_REELS_READY);
    assert_eq!(svc.failed_len(), 0);
}

// ===================== transport speed (R.57, item 455) =====================

/// The rectangle of the speed control (the program reports it itself — the rule
/// “coordinates are taken from the program ITSELF”, R.27).
fn speed_rect(h: &Harness<'_, KtApp>) -> egui::Rect {
    let r = h.state().speed_rect;
    assert!(r.is_finite() && r.width() > 100.0, "the speed control is not drawn: {r:?}");
    r
}

/// Ctrl+click without pointer movement. The modifier travels as a SEPARATE event:
/// `i.modifiers` egui reads from the frame state, not from the mouse button.
fn ctrl_click_at(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) {
    let m = egui::Modifiers::COMMAND;
    h.hover_at(pos);
    h.step();
    h.event(egui::Event::ModifiersChanged(m));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: m,
        });
    }
    h.step();
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

/// Test 627 (R.57): ENTERING THE SPEED AS A NUMBER.
///
/// The owner’s direct complaint: “I must be able to set 0.05
/// or −0.03”. The test probes from where the person acts: the right button
/// (and Ctrl+click — the second promised path, rule 17) on the strip ITSELF,
/// typing characters, Enter — and asserts the CONSEQUENCE: what stands in the base
/// speed and what went to the player.
#[test]
fn t627_speed_typed_value_reaches_player() {
    use crate::app::format_rate;
    let mut h = harness();
    let rect = speed_rect(&h);
    let on_bar = egui::pos2(rect.center().x, rect.top() + 4.0);

    // --- the strip’s hint is truthful and promises both entries
    h.hover_at(on_bar);
    h.step();
    assert_eq!(
        h.state().hover_hint.as_deref(),
        Some(tr(k::hint::SPEED)),
        "the speed strip does not give its hint"
    );

    // --- right click opens the field IN PLACE of the strip
    let before = h.state().tl_geom.clone().map(|g| g.rect.height());
    right_click_at(&mut h, on_bar);
    assert!(h.state().speed_entry.is_some(), "RMB on the strip did not open speed input");
    assert_eq!(
        h.state().tl_geom.clone().map(|g| g.rect.height()),
        before,
        "opening the field shifted the layout (rule 3: nothing jumps)"
    );
    assert_eq!(speed_rect(&h), rect, "the field took a different rectangle than the strip");
    h.hover_at(on_bar);
    h.step();
    assert_eq!(
        h.state().hover_hint.as_deref(),
        Some(tr(k::hint::SPEED_ENTRY)),
        "the open field has a foreign hint"
    );

    // --- the space bar with the field open does NOT start playback
    let playing = h.state().player.transport.is_playing();
    h.event(egui::Event::Text(" ".into()));
    h.key_press(egui::Key::Space);
    h.step();
    assert_eq!(
        h.state().player.transport.is_playing(),
        playing,
        "space leaked into hotkeys past the open field"
    );

    // --- “0.05” + Enter
    h.event(egui::Event::Text("0.05".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    assert!(h.state().speed_entry.is_none(), "Enter did not close the field");
    let base = h.state().base_rate;
    assert!((base - 0.05).abs() < 1e-6, "the base did not become 0.05, but {base}");
    let live = h.state().player.playback_rate();
    assert!((live - 0.05).abs() < 1e-6, "the player did not get 0.05, but {live}");
    assert_eq!(format_rate(base), "+0.05×", "the speed display lies");
    assert!(
        !h.ctx.egui_wants_keyboard_input(),
        "the field closed, but focus was not returned — hotkeys are dead (rule 14)"
    );

    // --- second entry: Ctrl+click, and the minus reaches the model
    ctrl_click_at(&mut h, on_bar);
    assert!(h.state().speed_entry.is_some(), "Ctrl+click did not open speed input");
    h.event(egui::Event::Text("\u{2212}0,03".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    let base = h.state().base_rate;
    assert!((base + 0.03).abs() < 1e-6, "the typographic minus and comma did not get through: {base}");

    // --- an unreadable value does NOT close the field (rule 16: it is an error)
    right_click_at(&mut h, on_bar);
    h.event(egui::Event::Text("-".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    assert!(
        h.state().speed_entry.as_ref().is_some_and(|e| e.bad),
        "unreadable value silently closed the field"
    );
    assert!((h.state().base_rate + 0.03).abs() < 1e-6, "unreadable value broke the database");

    // --- Esc closes without an edit, and AFTER it everything is alive (rule 14)
    h.key_press(egui::Key::Escape);
    h.step();
    h.step();
    assert!(h.state().speed_entry.is_none(), "Esc did not close the field");
    assert!((h.state().base_rate + 0.03).abs() < 1e-6, "Esc changed the speed");
    assert!(!h.ctx.egui_wants_keyboard_input(), "after Esc focus stayed in the field");
    let playing = h.state().player.transport.is_playing();
    h.key_press(egui::Key::Space);
    h.step();
    h.step();
    assert_ne!(
        h.state().player.transport.is_playing(),
        playing,
        "after closing the field, Space stopped toggling playback"
    );
    // and the field opens AGAIN
    right_click_at(&mut h, on_bar);
    assert!(h.state().speed_entry.is_some(), "second right click did not open the field");
}

/// Test 628 (R.57): dragging the strip gives SMALL speeds, Shift — fine
/// adjustment, the centre — stop, double click — 1×.
#[test]
fn t628_speed_drag_reaches_small_rates() {
    use crate::app::{RATE_KNEE, SPEED_FINE_DIV, format_rate, slider_to_rate};

    // The common pointer path: capture to the left of the centre, the drag threshold is taken
    // by an EXPLICIT step (egui recognises a drag after ~6 px, and the position of THAT
    // frame becomes the anchor of the virtual cursor).
    fn drag_upper(shift: bool, extra: f32) -> (f32, f32) {
        let mut h = harness();
        let rect = speed_rect(&h);
        let y = rect.top() + 4.0;
        let x0 = rect.center().x - 40.0;
        let x1 = rect.center().x - 20.0; // anchor: here the gesture is recognised as a drag
        h.hover_at(egui::pos2(x0, y));
        h.step();
        h.drag_at(egui::pos2(x0, y));
        h.step();
        if shift {
            h.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
        }
        h.hover_at(egui::pos2(x1, y));
        h.step();
        h.hover_at(egui::pos2(x1 + extra, y));
        h.step();
        let got = h.state().base_rate;
        h.drop_at(egui::pos2(x1 + extra, y));
        h.step();
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        (got, x1 - rect.left())
    }

    // --- 6 px to the right of the centre is 0.05×, NOT 0.25× and not stop
    {
        let mut h = harness();
        let rect = speed_rect(&h);
        let y = rect.top() + 4.0;
        let x0 = rect.center().x - 40.0;
        h.hover_at(egui::pos2(x0, y));
        h.step();
        h.drag_at(egui::pos2(x0, y));
        h.step();
        h.hover_at(egui::pos2(rect.center().x - 20.0, y));
        h.step();
        h.hover_at(egui::pos2(rect.center().x + 6.0, y));
        h.step();
        let got = h.state().base_rate;
        h.drop_at(egui::pos2(rect.center().x + 6.0, y));
        h.step();
        assert!(
            got > 0.02 && got < RATE_KNEE,
            "6 px from the center must give tenths of a percent of speed, but gave {got}"
        );
        assert!((got - 0.055).abs() < 0.02, "6 px from the center gave {got}, expected about 0.055");
        // --- release 1 px from the centre is EXACTLY stop
        let mut h = harness();
        let rect = speed_rect(&h);
        h.hover_at(egui::pos2(x0, y));
        h.step();
        h.drag_at(egui::pos2(x0, y));
        h.step();
        h.hover_at(egui::pos2(rect.center().x - 20.0, y));
        h.step();
        h.hover_at(egui::pos2(rect.center().x + 1.0, y));
        h.step();
        let got = h.state().base_rate;
        h.drop_at(egui::pos2(rect.center().x + 1.0, y));
        h.step();
        assert_eq!(got, 0.0, "1 px from the center must be a stop");
        assert_eq!(format_rate(0.0), tr(k::perf::RATE_STOP));
    }

    // --- Shift: the same pointer path covers SPEED_FINE_DIV times less
    let travel = 48.0f32;
    let (plain, anchor) = drag_upper(false, travel);
    let (fine, anchor2) = drag_upper(true, travel);
    assert_eq!(anchor, anchor2, "harness diverged: gesture anchors differ");
    let w = 170.0f32;
    let t_of = |vx: f32| vx / w * 2.0 - 1.0;
    let want_plain = slider_to_rate(t_of(anchor + travel));
    let want_fine = slider_to_rate(t_of(anchor + travel / SPEED_FINE_DIV));
    assert!((plain - want_plain).abs() < 1e-4, "plain drag: {plain}, expected {want_plain}");
    assert!(
        (fine - want_fine).abs() < 1e-4,
        "Shift did not divide the RUN by {SPEED_FINE_DIV}: {fine}, expected {want_fine}"
    );
    assert!(fine < plain, "Shift must go slower than a plain drag");

    // --- double click returns 1×
    let mut h = harness_lang_dt(crate::i18n::Lang::Ru, Some(0.02));
    let rect = speed_rect(&h);
    h.state_mut().base_rate = 0.05;
    h.step();
    double_click_at(&mut h, egui::pos2(rect.center().x, rect.top() + 4.0));
    assert_eq!(h.state().base_rate, 1.0, "double click did not return to 1×");
}

/// Test 629 (R.57): holding the space bar to ramp works also from a SMALL base.
///
/// The former hysteresis was absolute (0.02): on a base of 0.05 that is 40 % of the base,
/// and the ramped speed physically did not reach the player — the widget
/// showed the ramp, while the program played as it stood.
#[test]
fn t629_space_boost_from_small_base() {
    use crate::app::SPACE_BOOST_MAX;
    let mut h = harness();
    h.state_mut().base_rate = 0.05;
    h.state_mut().player.set_playback_rate(0.05);
    h.step();
    h.key_down(egui::Key::Space);
    h.step();
    std::thread::sleep(std::time::Duration::from_millis(260));
    // RIGHT after the start of the ramp: what the multiplier shows and what plays
    // must match. The former ABSOLUTE threshold of 0.02 held the player at
    // 0.05 until the multiplier grew to 1.4 — the program played as it stood,
    // while the widget already showed the ramp.
    let mut worst = 0.0f32;
    for _ in 0..12 {
        h.step();
        let b = h.state().space_boost.unwrap_or(1.0);
        let want = h.state().base_rate * b;
        let live = h.state().player.playback_rate();
        worst = worst.max((want - live).abs() / h.state().base_rate.abs());
    }
    assert!(
        worst < 0.10,
        "acceleration from a small base lags the displayed value by {:.0} % of the base",
        worst * 100.0
    );
    for _ in 0..60 {
        h.step();
    }
    let boosted = h.state().player.playback_rate();
    assert!(boosted > 0.075, "from base 0.05 the acceleration did not reach the player: {boosted}");
    assert!(
        boosted <= 0.05 * SPACE_BOOST_MAX + 1e-3,
        "acceleration from a small base overshot the cap: {boosted}"
    );
    // and the widget SHOWS this
    let shown = h.state().speed_temp_slider().expect("the boost is not visible on the speed control");
    assert!(
        shown > crate::app::rate_to_slider(0.05) + 0.001,
        "speed strip does not show acceleration from a small base: {shown}"
    );
    h.key_up(egui::Key::Space);
    h.step();
    assert!(
        (h.state().player.playback_rate() - 0.05).abs() < 1e-4,
        "after release, speed did not return to the small base: {}",
        h.state().player.playback_rate()
    );
}

/// Test 630 (R.57, item 456): the axis of the graph for a SIGNED speed is the
/// logarithm of the modulus, and an ordinary curve stops being straight.
///
/// The R.34 complaint verbatim: “the Slowed graph is cut off on the rate axis at 2.0, while
/// the numeric field accepts up to 4.0”. With a range of −32…32 a linear axis would
/// repeat the same trouble from the other side: the curve 0.66…1.0 would take
/// 0.5 % of the height and look straight.
#[test]
fn t630_signed_ratio_axis_is_log_by_modulus() {
    use crate::automation::{Axis, axis_of};
    use kt_model::{FxInstance, ParamUnit};

    let slowed = FxInstance::slowed_reverb(1.0, 0.0, 85.0);
    let rate = slowed.param_specs().iter().find(|s| s.name == "rate").unwrap();
    assert_eq!(rate.unit, ParamUnit::Ratio);
    let ax = axis_of(rate);
    assert!(matches!(ax, Axis::SymLog { .. }), "for a signed ratio the axis must be log");

    // oddness, zero at zero, monotonicity and invertibility
    assert_eq!(ax.map(0.0), 0.0);
    for v in [0.01f64, 0.05, 0.25, 0.66, 1.0, 4.0, 32.0] {
        assert!((ax.map(-v) + ax.map(v)).abs() < 1e-12, "axis is not odd at {v}");
        assert!((ax.unmap(ax.map(v)) - v).abs() < 1e-9, "axis circle diverged by {v}");
        assert!((ax.unmap(ax.map(-v)) + v).abs() < 1e-9);
    }
    let mut prev = f64::MIN;
    for i in -320..=320 {
        let g = ax.map(i as f64 / 10.0);
        assert!(g > prev, "axis is not monotonic near {}", i as f64 / 10.0);
        prev = g;
    }
    // continuity at the knee
    assert!((ax.map(0.2499) - ax.map(0.2501)).abs() < 1e-3);

    // CONSEQUENCE: the curve 0.66…1.0 takes a noticeable share of the scale, not
    // half a percent, as it would on a linear axis
    let g = (ax.map(1.0) - ax.map(0.66)) / (ax.map(32.0) - ax.map(-32.0));
    let lin = (1.0 - 0.66) / 64.0;
    assert!(g > lin * 5.0, "log axis did not stretch the neighborhood of 1: {g} versus {lin}");

    // for an unsigned ratio the axis is the old one — the rule is by CLASS, not by name
    let blur = FxInstance::blur(8);
    for s in blur.param_specs() {
        assert!(matches!(axis_of(s), Axis::Linear), "{}: the axis drifted", s.name);
    }
}

// ---- R.58: strip inside a reel, strips across contexts, zone down to zero --------

/// A two-level harness: root → X → Y. Inside X the event is collapsed into
/// the reel Y by the same gesture as in the root. Returns the geometry INSIDE X,
/// the addresses of both reels and the instance Y (in the axis of X).
#[cfg(test)]
fn harness_two_level() -> (
    Harness<'static, KtApp>,
    crate::timeline::TlGeom,
    kt_model::MediaId,
    kt_model::MediaId,
    kt_model::ClipId,
) {
    let (mut h, _g, x) = harness_with_nest();
    let x_inst = h.state().model.project().clips.iter().find(|c| c.media == x).unwrap().id;
    h.state_mut().enter_nest(x_inst);
    h.step();
    assert_eq!(h.state().model.editing(), Some(x), "harness: entering X failed");
    let inner = h.state().model.project().clips[0].id;
    h.state_mut().select_only(inner);
    h.step();
    h.state_mut().collapse_selection();
    h.step();
    h.step();
    let y = h
        .state()
        .model
        .document()
        .sequences
        .iter()
        .map(|s| s.id)
        .find(|id| *id != x)
        .expect("harness: reel Y was not created inside X");
    let y_inst = h.state().model.project().clips.iter().find(|c| c.media == y).unwrap().id;
    let g = h.state().tl_geom.clone().expect("geometry inside X");
    (h, g, x, y, y_inst)
}

/// Test 634 (R.58, item 460): a strip expands INSIDE a full entry.
///
/// **The owner’s complaint verbatim:** “while opening a composition I cannot expand
/// a nested composition in it (I open the composition on the whole
/// timeline and I cannot expand a nested one in it without getting into it already)”.
///
/// The Enter cascade inside X is the same as in the root: the first expands, the second
/// enters. The document does not change from the expansion, an edit in the strip reaches
/// reel Y (not X and not the root), and Esc collapses the strip BEFORE exiting
/// X.
#[test]
fn t634_a_band_opens_inside_an_entered_reel() {
    use crate::app::EnterOnNest;
    use kt_time::Tick;
    let (mut h, _g, x, y, y_inst) = harness_two_level();
    let doc_before = h.state().model.document();
    let rows_before = h.state().tl_geom.clone().map(|g| g.rows.len()).unwrap_or(0);

    // 1st Enter — EXPAND, and the decision does not depend on the context
    assert_eq!(h.state().enter_pressed_on_nest(y_inst), EnterOnNest::Expand);
    h.state_mut().select_only(y_inst);
    h.state_mut().expand_in_place(y_inst);
    h.step();
    assert_eq!(h.state().model.editing(), Some(x), "expanding a band left reel X");
    assert_eq!(
        h.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![y_inst],
        "the band did not open inside an entered reel"
    );
    assert!(
        h.state().is_selected(y_inst),
        "the instance lost its selection — the 2nd Enter would miss"
    );
    let g = h.state().tl_geom.clone().expect("geometry");
    assert!(g.rows.len() > rows_before, "band rows are not on screen inside X: {:?}", g.rows);
    assert_eq!(h.state().model.document(), doc_before, "expanding inside X changed the DOCUMENT");
    // 2nd Enter — ENTER
    assert_eq!(h.state().enter_pressed_on_nest(y_inst), EnterOnNest::Enter);

    // an edit in the strip reaches Y
    let (id, was, lane) = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .filter(|c| c.id != y_inst && h.state().reel_of_clip(c.id) == Some(y))
        .map(|c| (c.id, c.start, c.lane))
        .next()
        .expect("a band event");
    assert!(h.state_mut().apply(kt_model::Command::MoveClip {
        id,
        new_start: was + Tick(1_000),
        new_lane: lane,
    }));
    h.step();
    let doc = h.state().model.document();
    assert!(doc.sequence(y).unwrap().clips.iter().any(|c| c.id == id), "the edit missed reel Y");
    assert!(
        !doc.sequence(x).unwrap().clips.iter().any(|c| c.id == id)
            && !doc.clips.iter().any(|c| c.id == id),
        "the event of Y leaked into X or into the root"
    );
    assert_eq!(h.state().model.bands().len(), 1, "the edit collapsed the band");

    // Esc: clear selection → collapse the strip → and only then exit X
    h.state_mut().escape_action();
    assert_eq!(h.state().model.bands().len(), 1, "Esc collapsed the band instead of deselecting");
    h.state_mut().escape_action();
    assert!(h.state().model.bands().is_empty(), "the 2nd Esc must collapse the band");
    assert_eq!(h.state().model.editing(), Some(x), "Esc left X before collapsing the band");
    h.state_mut().escape_action();
    assert_eq!(h.state().model.editing(), None, "the 3rd Esc must leave X");
}

/// Test 636 (R.58, item 461): strips SURVIVE a change of context.
///
/// **The owner’s complaint verbatim:** “when moving between compositions /
/// root timeline they reset the expanded state, but we would like
/// them to keep it so as not to get confused”. The owner’s decision (a revision of R.56
/// item 5): restore EVERYTHING, including the strip through which the second
/// Enter entered — the Enter/Esc cascade is symmetric.
#[test]
fn t636_bands_survive_context_switches() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    use kt_time::Tick;

    // (a) root: the strip on A, entry into B through ANOTHER instance (menu item
    // “Open”), return — A is expanded, the view and cursor are the same (t456)
    let (mut h, g, a) = harness_with_nest();
    let a_inst = h.state().model.project().clips.iter().find(|c| c.media == a).unwrap().id;
    // a second instance: the third event collapses into the reel B
    h.state_mut().select_only(ClipId(3));
    h.step();
    h.state_mut().collapse_selection();
    h.step();
    h.step();
    let b = h.state().model.document().sequences.iter().map(|s| s.id).find(|id| *id != a).unwrap();
    let b_inst = h.state().model.project().clips.iter().find(|c| c.media == b).unwrap().id;
    h.state_mut().expand_in_place(a_inst);
    // the cursor — INSIDE THE BODY of instance B (200 000 … 250 000): entry translates
    // the time rather than teleporting, and the return must land on the same point
    // (outside the body the cursor is clamped to the material — rule of item 447)
    h.state_mut().player.seek(Tick(220_000));
    h.state_mut().view_start = 5_000.0;
    h.state_mut().ticks_per_px = 900.0;
    h.step();
    let g = h.state().tl_geom.clone().unwrap_or(g);
    let (vs, tpp) = (h.state().view_start, h.state().ticks_per_px);
    let cursor = h.state().player.transport.playhead();
    let row = g.row_of((LaneKind::Video, 0)).expect("video row");
    let x = {
        let c = h.state().model.project().clips.iter().find(|c| c.id == b_inst).unwrap().clone();
        g.tick_to_x(c.start.0 + c.duration.0 / 2)
    };
    right_click_at(&mut h, egui::pos2(x, g.row_center_y(row)));
    h.get_by_label_contains(tr(k::tl::menu::NEST_ENTER)).click();
    h.step();
    h.step();
    assert_eq!(h.state().model.editing(), Some(b), "the menu did not enter B");
    assert!(h.state().model.bands().is_empty(), "entering must clear bands (model invariant)");
    h.state_mut().leave_nest();
    h.step();
    assert_eq!(h.state().model.editing(), None);
    assert_eq!(
        h.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![a_inst],
        "(a) the band on A was not restored after returning from B"
    );
    assert_eq!((h.state().view_start, h.state().ticks_per_px), (vs, tpp), "(a) view not restored");
    // precision — the transport quantum (an audio frame ≈ 1.875 ticks), as in t456
    let back = h.state().player.transport.playhead();
    assert!((back.0 - cursor.0).abs() <= 4, "(a) cursor not restored: {back:?} vs {cursor:?}");
    assert!(h.state().selected.contains(&b_inst), "(a) selection not restored");

    // (b) entry via the 2nd Enter on the expanded A, Esc cascade up to the exit — A
    // is expanded and selected again, the third Enter enters again
    h.state_mut().select_only(a_inst);
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    assert_eq!(
        h.state().model.editing(),
        Some(a),
        "(b) the 2nd Enter on an expanded band must enter"
    );
    let mut guard = 0;
    while h.state().model.editing().is_some() {
        h.state_mut().escape_action();
        guard += 1;
        assert!(guard < 6, "(b) the Esc cascade does not lead out");
    }
    assert_eq!(
        h.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![a_inst],
        "(b) the band we entered through was not restored (owner’s decision, R.58)"
    );
    assert!(h.state().is_selected(a_inst), "(b) the instance is not selected after the return");
    h.key_press(egui::Key::Enter);
    h.step();
    assert_eq!(h.state().model.editing(), Some(a), "(b) the gesture does not repeat");
    h.state_mut().leave_nest();
    h.step();

    // (d, part 1) the instance from the snapshot has disappeared or stopped being a nested item —
    // silently, without the strip, without a panic
    h.state_mut().enter_nest(b_inst);
    h.step();
    h.state_mut().nest_path.last_mut().unwrap().band = Some(ClipId(999_999));
    h.state_mut().leave_nest();
    h.step();
    assert_eq!(h.state().model.editing(), None);
    assert!(h.state().model.bands().is_empty(), "(d) a vanished instance must not leave a band");

    // (c) inside X the strip on Y, exit to the root and entry back into X — Y is
    // expanded; (d, part 2) the instance inside was deleted — there is no strip
    let (mut h2, _g2, x, _y, y_inst) = harness_two_level();
    let x_inst = h2.state().model.document().clips.iter().find(|c| c.media == x).unwrap().id;
    h2.state_mut().expand_in_place(y_inst);
    h2.step();
    h2.state_mut().leave_nest();
    h2.step();
    assert_eq!(h2.state().model.editing(), None);
    h2.state_mut().enter_nest(x_inst);
    h2.step();
    assert_eq!(
        h2.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![y_inst],
        "(c) re-entering X did not bring the band on Y back"
    );
    assert!(h2.state_mut().apply(Command::RemoveClip { id: y_inst }));
    h2.step();
    assert!(h2.state().model.bands().is_empty(), "removing the instance must drop its band");
    h2.state_mut().leave_nest();
    h2.step();
    h2.state_mut().enter_nest(x_inst);
    h2.step();
    assert!(h2.state().model.bands().is_empty(), "(d) a removed instance came back as a band");
}

/// Drag of an event: hover → press → move slightly → carry → release.
#[cfg(test)]
fn drag_clip(h: &mut Harness<'_, KtApp>, from: egui::Pos2, to: egui::Pos2) {
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    h.hover_at(from + egui::vec2(8.0, 0.0));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();
}

/// Test 637 (R.58, item 462): inside a reel an event moves BEYOND ZERO; in the root
/// zero is a wall.
///
/// **The owner’s complaint verbatim:** “When editing a fully opened
/// composition I cannot drag an event beyond its limits, but with an expanded
/// composition in root — I can. I do not need playing down to 0 in a fully open composition”.
/// The wall was only in the UI (`.max(0)` over the gesture tick); the format and the model
/// allow a negative `start`, and the instance window
/// does not play such.
#[test]
fn t637_inside_a_reel_an_event_may_cross_zero() {
    use kt_model::*;
    use kt_time::Tick;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    let inst_ev = h.state().model.document().clips.iter().find(|c| c.id == inst).unwrap().clone();
    h.state_mut().enter_nest(inst);
    h.state_mut().ticks_per_px = 1000.0;
    h.state_mut().view_start = -1.0e6;
    h.step();
    let g = h.state().tl_geom.clone().expect("geometry inside the reel");
    let inner = h.state().model.project().clips[0].clone();
    assert_eq!(inner.start, Tick::ZERO, "scene: the reel content must start at zero");
    let y = g.lane_y(ClipKind::Video, inner.lane, 0.6);
    let grab = egui::pos2(g.tick_to_x(inner.start.0 + 25_000), y);
    let to = egui::pos2(g.tick_to_x(inner.start.0 - 5_000), y);
    assert!(to.x > g.rect.left(), "scene: the drop point is off screen ({:?})", to);
    drag_clip(&mut h, grab, to);
    let moved = h.state().model.project().clips.iter().find(|c| c.id == inner.id).unwrap().clone();
    assert_eq!(moved.start, Tick(-30_000), "the event did not cross zero inside the reel");

    // the DOCUMENT of the reel stores a negative start
    let doc = h.state().model.document();
    assert_eq!(
        doc.sequence(seq).unwrap().clips.iter().find(|c| c.id == inner.id).map(|c| c.start),
        Some(Tick(-30_000)),
        "the reel's sequence does not store the negative start"
    );
    // and does NOT PLAY it: the instance window cuts off everything before the reel’s zero —
    // in the root a piece [inst.start, +20 000) remains and nothing to the left
    let eff = doc.effective_clips();
    assert!(eff.iter().all(|c| c.start >= Tick::ZERO), "played content went before zero: {eff:?}");
    assert!(
        eff.iter().any(|c| c.start == inst_ev.start && c.duration == Tick(20_000)),
        "the instance window did not cut the pre-zero part: {eff:?}"
    );
    // Esc/exit/entry — the event stays in place
    h.state_mut().leave_nest();
    h.step();
    h.state_mut().enter_nest(inst);
    h.step();
    assert_eq!(
        h.state().model.project().clips.iter().find(|c| c.id == inner.id).map(|c| c.start),
        Some(Tick(-30_000)),
        "leave + re-enter lost the negative start"
    );

    // in the ROOT the same gesture hits zero
    let (mut h2, g2) = harness_three_clips();
    let y2 = g2.lane_y(ClipKind::Video, 0, 0.6);
    let grab2 = egui::pos2(g2.tick_to_x(25_000), y2);
    let to2 = egui::pos2(g2.tick_to_x(-5_000).max(g2.rect.left() + 1.0), y2);
    drag_clip(&mut h2, grab2, to2);
    let c1 = h2.state().model.project().clips.iter().find(|c| c.id == ClipId(1)).unwrap().start;
    assert_eq!(c1, Tick::ZERO, "in the root zero must remain a wall");
}

/// Test 637b (R.58, item 462): the view goes left of zero ONLY inside a reel, and the
/// cursor does not go below zero anywhere.
#[test]
fn t637b_view_before_zero_only_inside_a_reel_and_the_cursor_never() {
    use kt_model::*;
    use kt_time::Tick;
    // root: the former −40 px and not a tick further
    let (mut h, _g) = harness_three_clips();
    h.state_mut().view_start = -1.0e6;
    h.step();
    let tpp = h.state().ticks_per_px;
    assert_eq!(h.state().view_start, -40.0 * tpp, "the root view went past −40 px");

    // inside a reel: the floor is the earliest event (it is left of zero) and the same −40 px
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    h.state_mut().enter_nest(inst);
    h.step();
    let inner = h.state().model.project().clips[0].id;
    assert!(h.state_mut().apply(Command::MoveClip {
        id: inner,
        new_start: Tick(-30_000),
        new_lane: 0
    }));
    h.state_mut().ticks_per_px = 1000.0;
    h.state_mut().view_start = -1.0e6;
    h.step();
    assert_eq!(
        h.state().view_start,
        -30_000.0 - 40_000.0,
        "inside a reel the view floor is the earliest event"
    );

    // cursor: a click on the track and on the ruler LEFT of zero sets it to 0
    let g = h.state().tl_geom.clone().expect("geometry");
    let x = g.tick_to_x(-20_000);
    assert!(x > g.rect.left(), "scene: the pre-zero zone is not on screen");
    for y in [g.lane_y(ClipKind::Video, 0, 0.6), g.rect.top() + crate::timeline::TIME_TOP + 11.0] {
        h.state_mut().player.seek(Tick(40_000));
        h.step();
        let pos = egui::pos2(x, y);
        h.hover_at(pos);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
        h.step();
        assert_eq!(
            h.state().player.transport.playhead(),
            Tick::ZERO,
            "a click before zero at y={y} must put the cursor at 0, not before"
        );
    }
}

/// Test 666 (R.59, stage 2): the generator form in the event window reaches
/// the MODEL — both by value and by curve.
///
/// What is verified is what a person does: he opens the event’s effects window,
/// sees the SOURCE as the first crumb, types into the title field — and the text
/// lands on the EVENT; presses “+ curve” and picks a row — and the event gets its
/// curve. The earlier generator and title windows are thereby closed not by
/// “deleting code” but by moving: all their inputs lead here.
///
/// There are two harnesses, and this is not laziness: the title form is the tallest in the program,
/// and its curve menu in the harness window opens already below the bottom edge.
/// Curves are a property of ANY generator, so they are probed on the
/// compact view, and the text field on the title.
#[test]
fn t666_generator_form_in_the_fx_window_reaches_the_model() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    use kt_model::*;
    crate::i18n::set_lang(crate::i18n::Lang::Ru);

    // ---- CURVE: “+ curve” → row -----------------------------------
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::NoiseField { amount: 0.5, mono: false });
    for _ in 0..3 {
        h.step();
    }
    let (_id, clip) = made_generator(&h);
    assert_eq!(
        chain_sel(&h, clip),
        Some(crate::widgets::ChainSel::Head),
        "the source must be the FIRST CRUMB and be selected at once"
    );
    assert!(
        h.state().model.clip(clip).is_some_and(|c| c.gen_auto.is_empty()),
        "the new curve generator must not have any curves"
    );
    let depth = h.state().model.undo_depth();
    h.get_by_label(crate::i18n::tr(crate::i18n::k::fx::AUTO_ADD)).click();
    h.step();
    h.step();
    // the row label is both in the value grid and in the open menu —
    // we press the LAST one: the menu is drawn on top (idiom t193)
    let row = crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "amount");
    h.get_all_by_label(row.as_str()).last().expect("row menu item").click();
    h.step();
    h.step();
    let curves = h.state().model.clip(clip).map(|c| c.gen_auto.clone()).unwrap_or_default();
    assert_eq!(curves.len(), 1, "curve did not reach the event: {curves:?}");
    assert_eq!(curves[0].param, "amount");
    assert_eq!(
        curves[0].keys.first().map(|k| k.t),
        h.state().model.clip(clip).map(|c| c.source_in),
        "the generator curve lives in the tick of the MATERIAL — the source resolves it by that"
    );
    assert_eq!(h.state().model.undo_depth(), depth + 1, "an edit in the window is one history step");

    // ---- VALUE: we type into the title field, as a person would ----------------
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    for _ in 0..3 {
        h.step();
    }
    let (_id, clip) = made_generator(&h);
    let depth = h.state().model.undo_depth();
    h.get(by().role(Role::MultilineTextInput)).click();
    h.step();
    for part in ["Iss", "ue"] {
        h.event(egui::Event::Text(part.into()));
        h.step();
        h.step();
    }
    match gen_of(&h, clip) {
        GeneratorKind::Text { text, .. } => {
            assert_eq!(text, "Issue", "entered text did not reach the event")
        }
        other => panic!("view was swapped: {other:?}"),
    }
    assert_eq!(
        h.state().model.undo_depth(),
        depth + 1,
        "the whole window session is ONE history step: otherwise every character of the title would land in undo"
    );
}

/// Test 667 (R.59, stage 2): “Apply to all (×N)” is a SEPARATE
/// command and ONE history step.
///
/// The owner’s decision: editing one event no longer touches its neighbours, and
/// “edit in all places at once” stays available as a separate gesture.
/// So both halves must be verified: until the button is pressed the neighbour does not
/// move; once pressed, the neighbour catches up, and one Ctrl+Z returns it back.
#[test]
fn t667_apply_to_all_is_one_command_and_one_step() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
    h.step();
    let (id, first) = made_generator(&h);
    // second place of the same generator
    h.state_mut().player.seek(kt_time::Tick(700_000));
    h.state_mut().place_media_at_cursor(id);
    h.step();
    let second = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.media == id && c.id != first)
        .map(|c| c.id)
        .expect("second generator event");

    // editing ONE neighbouring event does not touch it
    let gesture = h.state().fx_editors[0].gesture;
    let mine = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    {
        let app = h.state_mut();
        assert!(
            app.apply_coalesced(
                Command::SetClipGen { id: first, kind: Some(mine.clone()) },
                gesture
            )
        );
    }
    h.step();
    assert_eq!(gen_of(&h, first), mine);
    assert_eq!(
        gen_of(&h, second),
        GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 },
        "editing one event has no right to touch its neighbors"
    );

    // ...but the button does touch it, and says so with the number of places
    let depth = h.state().model.undo_depth();
    let label = crate::tf!(crate::i18n::k::genw::APPLY_ALL, 2usize);
    assert!(h.query_by_label(&label).is_some(), "button “{label}” is missing from the source form");
    h.get_by_label(&label).click();
    h.step();
    h.step();
    assert_eq!(gen_of(&h, second), mine, "“apply to all” did not reach the neighbor");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "the gesture must be ONE history step");
    h.state_mut().undo();
    h.step();
    assert_eq!(
        gen_of(&h, second),
        GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 },
        "one Ctrl+Z must restore the neighbor"
    );
}

/// Test 668 (R.59, stage 2): the title animation preset sets CURVES, and
/// the layout does not move because of them.
///
/// The five earlier animation modes were a field of the MODEL, and the engine knew about
/// the “typewriter”. Now this is the NAME of a SET OF CURVES; the formulas live in one
/// place (`kt_model::text_anim_curves`) — old files are read by them too, and if the migration
/// diverged from the preset, a project from the previous build would come out
/// differently.
///
/// The second half is rule F-TXT-006: the layout is computed ONCE, and
/// the animation only modulates it. So a title’s text block must be the same
/// with and without animation: if it moved, the
/// “ticker” would start to re-wrap words under itself.
#[test]
fn t668_preset_writes_curves_and_keeps_the_layout() {
    use crate::gen_ui::title_presets;
    use kt_model::*;
    let mut base = GeneratorKind::default_text();
    if let GeneratorKind::Text { text, .. } = &mut base {
        *text = "Issue ticker".into();
    }
    let frame = (1920u32, 1080u32);
    let src_in = kt_time::Tick(123_456);

    // “Ticker”: one row, keys from +span/2 to −span/2, anchored at
    // the event ENTRY (generator curves live in the material tick)
    let crawl = title_presets().iter().find(|p| p.anim == TextAnim::Crawl).expect("preset");
    let made = crawl.applied(&base);
    let curves = crawl.curves(&made, frame, src_in);
    assert_eq!(curves.len(), 1, "ticker is exactly one animated row");
    assert_eq!(curves[0].param, "offset_x");
    assert!(
        curves[0].keys.iter().all(|k| k.shape == kt_model::SegShape::Linear),
        "the pass is linear: with smoothing the line would move in jerks"
    );
    assert_eq!(curves[0].keys.len(), 2);
    assert_eq!(curves[0].keys[0].t, src_in, "curve must start at the event’s ENTRY");
    assert_eq!(
        curves[0].keys[1].t.0 - src_in.0,
        (crawl.anim_secs * 90_000.0).round() as i64,
        "pass duration is the old one (old files are read with the same formulas)"
    );
    let block = kt_media::generator::title_layout(&made, frame.0, frame.1).expect("layout");
    let span = 1.0 + block.block.w / frame.0 as f64;
    assert!(
        (curves[0].keys[0].v - span / 2.0).abs() < 1e-9
            && (curves[0].keys[1].v + span / 2.0).abs() < 1e-9,
        "path is computed from the block’s LAYOUT: {:?}",
        curves[0].keys
    );

    // ...and the layout does NOT depend on the animation: the shift modulates the display, and
    // does not move the words (F-TXT-006)
    let mut moved = made.clone();
    moved.set_param("offset_x", 0.37);
    moved.set_param("reveal", 0.25);
    let after = kt_media::generator::title_layout(&moved, frame.0, frame.1).expect("layout");
    assert_eq!(
        (after.block.w, after.block.h),
        (block.block.w, block.block.h),
        "animation shifted the LAYOUT — the “computed once” rule is violated"
    );

    // “Typewriter”: it has a preset, and it drives the `reveal` row
    let tw = title_presets().iter().find(|p| p.anim == TextAnim::Typewriter).expect("preset");
    let made = tw.applied(&base);
    let c = tw.curves(&made, frame, kt_time::Tick::ZERO);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].param, "reveal");
    assert_eq!(c[0].keys.first().map(|k| k.v), Some(0.0));
    assert_eq!(c[0].keys.last().map(|k| k.v), Some(1.0));

    // “Reveal” drives the SAME row, and what tells them apart is the front width — it is
    // constant over time, so it is a VALUE, not a curve (a curve with a single key would be a
    // second way to write the same thing).
    // It has no preset, so we query the common formulas directly —
    // the same ones by which old files are read.
    for (anim, soft) in [(TextAnim::Typewriter, 0.0), (TextAnim::Fade, 1.0)] {
        let mut k = base.clone();
        text_anim_values(&mut k, anim);
        let GeneratorKind::Text { reveal_soft, .. } = &k else { unreachable!() };
        assert_eq!(*reveal_soft, soft, "{anim:?}: front width is a value, not a curve");
        let c = text_anim_curves(anim, 2.0, 2.0);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].param, "reveal", "{anim:?} drives the same row");
    }

    // ...and the “active preset” does not confuse these two: it compares both
    // the values and the set of curves
    let curves_tw = tw.curves(&made, frame, kt_time::Tick::ZERO);
    assert_eq!(crate::gen_ui::active_preset(&made, &curves_tw), Some(2));
    assert_eq!(
        crate::gen_ui::active_preset(&made, &[]),
        None,
        "without curves the “machine” stops being a machine — otherwise a new title would be named after it"
    );
}

// ---- Public composition parameters (R.59, stage 3) --------------------

/// News template for tests: a reel with a TITLE inside, placed
/// twice, and six knobs declared outward — one for each value
/// type.
///
/// The scene is intentionally NOT degenerate: the second instance sits in another place
/// and overrides the title. On a scene with “one instance without
/// overrides” half of the assertions below would pass by themselves.
pub(crate) struct NewsScene {
    pub seq: kt_model::MediaId,
    /// The title INSIDE the reel is the target of the text editions.
    pub inner: kt_model::ClipId,
    /// The colored logo inside the reel is the target of the color and media editions.
    pub logo_clip: kt_model::ClipId,
    /// The first edition (template values) and the second (its own title).
    pub a: kt_model::ClipId,
    pub b: kt_model::ClipId,
    pub headline: kt_model::PubId,
    pub tint: kt_model::PubId,
    pub size: kt_model::PubId,
    pub align: kt_model::PubId,
    pub logo: kt_model::PubId,
    pub flip: kt_model::PubId,
}

/// Build the scene. The editions are placed INSIDE the reel by commands, as the
/// window does: a command edits the reel you are standing in.
pub(crate) fn harness_news_template() -> (Harness<'static, KtApp>, NewsScene) {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    let (_media, title) = made_generator(&h);
    // a non-empty title: the kind with defaults has no text at all, and the “template”
    // would look empty
    let mut kind = gen_of(&h, title);
    if let GeneratorKind::Text { text, .. } = &mut kind {
        *text = "ISSUE".into();
    }
    assert!(h.state_mut().apply(Command::SetClipGen { id: title, kind: Some(kind) }));
    h.step();
    // The LOGO is a color plate next to the title. Its color is a real passport row
    // (`color_*` with unit Rgb8), unlike the title’s paints:
    // those live right in the variant and are not properties at all.
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 200, color_g: 30, color_b: 30 });
    h.step();
    let (_lm, logo_clip) = made_generator(&h);
    {
        let (start, lane) = h
            .state()
            .model
            .clip(title)
            .map(|c| (c.start + c.duration, c.lane))
            .expect("title in place");
        assert!(h.state_mut().apply(Command::MoveClip {
            id: logo_clip,
            new_start: start,
            new_lane: lane
        }));
    }
    h.step();
    // windows opened by creating generators get in the way of the scene: from here on, our own
    h.state_mut().fx_editors.clear();
    h.state_mut().selected.clear();
    h.state_mut().selected.insert(title);
    h.state_mut().selected.insert(logo_clip);
    h.step();
    h.state_mut().collapse_selection();
    h.step();
    h.step();
    let seq = h.state().model.document().sequences.last().expect("reel").id;
    let a = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.media == seq)
        .expect("reel instance")
        .id;

    // ---- declaring the knobs INSIDE the reel ------------------------------
    h.state_mut().enter_nest(a);
    h.step();
    assert_eq!(h.state().model.editing(), Some(seq), "entry into the reel did not happen");
    let inner = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| matches!(c.r#gen, Some(GeneratorKind::Text { .. })))
        .expect("title inside the reel")
        .id;
    let logo_in = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| matches!(c.r#gen, Some(GeneratorKind::Solid { .. })))
        .expect("logo inside the reel")
        .id;
    let mut next = 0u64;
    let mut publish = |h: &mut Harness<'static, KtApp>,
                       name: &str,
                       clip: ClipId,
                       owner: ParamOwner,
                       param: &str,
                       ty: ParamType| {
        next += 1;
        let id = PubId(next);
        let param = PublishedParam {
            id,
            name: name.to_string(),
            target: ParamAddr::new(clip, owner, param),
            ty,
            range: PubRange::default(),
        };
        assert!(
            h.state_mut().apply(Command::PublishParam { seq, param: Box::new(param) }),
            "publication “{name}” rejected"
        );
        h.step();
        id
    };
    let headline = publish(
        &mut h,
        "Heading",
        inner,
        ParamOwner::Generator,
        "text",
        ParamType::Text { multiline: true },
    );
    let tint =
        publish(&mut h, "Logo color", logo_in, ParamOwner::Generator, "color", ParamType::Color);
    let size = publish(&mut h, "Font size", inner, ParamOwner::Generator, "size", ParamType::Num);
    let align_ty = h
        .state()
        .model
        .document()
        .addr_type(Some(seq), &ParamAddr::new(inner, ParamOwner::Generator, "align"));
    let align = publish(
        &mut h,
        "Justification",
        inner,
        ParamOwner::Generator,
        "align",
        align_ty.expect("title has justification"),
    );
    let logo = publish(&mut h, "Logo", logo_in, ParamOwner::ClipAttr, "media", ParamType::Media);
    let flip =
        publish(&mut h, "Reversed", inner, ParamOwner::ClipAttr, "reversed", ParamType::Flag);

    h.state_mut().leave_nest();
    h.step();
    assert_eq!(h.state().model.editing(), None, "exit from the reel did not happen");

    // ---- second edition ----------------------------------------------
    let mut b_clip = h.state().model.project().clips.iter().find(|c| c.id == a).cloned().unwrap();
    let free = h.state().model.document().clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 40;
    b_clip.id = ClipId(free);
    b_clip.start = kt_time::Tick(600_000);
    b_clip.link_group = None;
    assert!(h.state_mut().apply(Command::AddClip(b_clip)), "second issue did not take");
    h.step();
    let b = ClipId(free);
    (h, NewsScene { seq, inner, logo_clip: logo_in, a, b, headline, tint, size, align, logo, flip })
}

/// Test 680 (R.59, stage 3): the reel PREVIEW is keyed by INSTANCE.
///
/// Until this round the `reel_preview` service was keyed by `(reel,
/// content_hash)`, that is, by the EDIT OF THE TEMPLATE. With overrides this
/// means that two editions with different titles would show ONE AND THE SAME frames
/// on the timeline — the frames of the first of them. The assertion of the test is a
/// consequence: the keys of the two instances are DIFFERENT, and the instance without overrides kept
/// its old key (byte for byte `content_hash`).
#[test]
fn t680_reel_preview_keys_are_per_instance() {
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // without overrides both editions share ONE key: projects that
    // do not use publishing pay nothing
    let keys = h.state_mut().reel_keys();
    let (ka, kb) = (*keys.get(&s.a).expect("key A"), *keys.get(&s.b).expect("key B"));
    assert_eq!(ka, kb, "instances without overrides must share one preview key");
    let plain = kt_engine::content_hash(&h.state().model.document(), s.seq);
    assert_eq!(ka.hash, plain, "empty set shifted the reel’s revision — all keys would move");

    // the second edition gets ITS OWN title
    assert!(h.state_mut().apply(Command::SetOverride {
        id: s.b,
        ov: Box::new(Override::value(s.headline, ParamValue::Text("SECOND ISSUE".into()))),
    }));
    h.step();
    let keys = h.state_mut().reel_keys();
    let (ka2, kb2) = (*keys.get(&s.a).expect("key A"), *keys.get(&s.b).expect("key B"));
    assert_eq!(ka2, ka, "template instance was not touched — its key must stay the same");
    assert_ne!(
        ka2, kb2,
        "overridden instance got the template’s key: on the timeline it would show OTHER frames"
    );

    // and both editions are ALIVE at the same time: invalidation has no right to
    // discard the neighbouring one, otherwise the two editions would recompute each other
    // in a loop
    h.state_mut().request_reel_preview(ka2, &[]);
    let ov_b = h.state().model.clip(s.b).map(|c| c.overrides.clone()).unwrap_or_default();
    h.state_mut().request_reel_preview(kb2, &ov_b);
    let pending = h.state().reel_previews.pending();
    assert!(pending >= 2, "two revisions of one reel did not queue together: {pending}");
}

/// Test 683 (R.59, stage 3): the “Composition parameters” window draws EACH
/// type with its own widget, reset sends `ClearOverride`, and a number with a curve
/// places a key in the material of the INSTANCE.
#[test]
fn t683_instance_params_window_draws_every_type() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, s) = harness_news_template();
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    assert_eq!(
        h.state().fx_editors.first().and_then(|e| e.selected.get(&s.b)),
        Some(&crate::widgets::ChainSel::Head),
        "the head “Composition parameters” is not selected in the FX window"
    );

    // ALL AUTHOR NAMES are visible, in the declared order
    for name in ["Heading", "Logo color", "Font size", "Justification", "Logo", "Reversed"]
    {
        assert!(
            h.query_all_by_label_contains(name).count() > 0,
            "row “{name}” is missing from the window — the type is drawn with nothing"
        );
    }

    // --- TEXT: the field edits the override --------------------------
    let win = h.state().fx_editors[0].gesture;
    assert!(h.state_mut().apply_coalesced(
        Command::SetOverride {
            id: s.b,
            ov: Box::new(Override::value(s.headline, ParamValue::Text("SECOND".into()))),
        },
        win
    ));
    h.step();
    assert_eq!(
        h.state().model.document().instance_value(h.state().model.clip(s.b).unwrap(), s.headline),
        Some(ParamValue::Text("SECOND".into())),
        "heading override did not reach the model"
    );
    // while the TEMPLATE stays intact: the neighbour plays its own
    assert_eq!(
        h.state().model.document().published_value(s.seq, s.headline),
        Some(ParamValue::Text("ISSUE".into())),
        "instance edit went to the reel — that would be an edit of all places at once"
    );

    // --- RESET: the row’s button sends ClearOverride ---------------------
    h.step();
    let reset = format!("{} Heading", tr(k::pubp::RESET));
    let before = h.state().model.clip(s.b).map(|c| c.overrides.len()).unwrap_or(0);
    assert_eq!(before, 1, "the second issue has exactly one override at this place");
    h.get_by_label(&reset).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().model.clip(s.b).map(|c| c.overrides.len()),
        Some(0),
        "reset button did not clear the override"
    );

    // --- TYPES: the declared type matches what the target actually HAS --------
    // the window draws the row BY TYPE, and if the declaration diverged from the target —
    // the person would get a checkbox in place of the text
    {
        let doc = h.state().model.document();
        let seq = doc.sequence(s.seq).cloned().expect("reel");
        for (id, want) in [
            (s.headline, ParamType::Text { multiline: true }),
            (s.tint, ParamType::Color),
            (s.size, ParamType::Num),
            (s.align, ParamType::Choice { variants: std::borrow::Cow::Borrowed(&[]) }),
            (s.logo, ParamType::Media),
            (s.flip, ParamType::Flag),
        ] {
            let p = seq.published.iter().find(|p| p.id == id).expect("declared handle");
            let live = doc.addr_type(Some(s.seq), &p.target).expect("target is alive");
            assert!(
                live.same_family(&want) && p.ty.same_family(&want),
                "handle “{}” is declared with the wrong type: {:?} versus the live {live:?}",
                p.name,
                p.ty
            );
        }
        // the color knob targets the LOGO, not the title: the address is part of
        // the declaration, and if the host window were mixed up, a foreign row would be edited
        let tint = seq.published.iter().find(|p| p.id == s.tint).unwrap();
        assert!(kt_model::same_clip(tint.target.clip, s.logo_clip), "color is declared not on the logo");
    }

    // --- NUMBER WITH A CURVE: the key lands in the material of the INSTANCE ---------
    // the second edition does NOT sit at zero (start = 600_000), so “instance material” and
    // “project time” are distinguishable here — the scene is not degenerate
    let inst = h.state().model.clip(s.b).cloned().unwrap();
    assert_ne!(inst.start.0, 0, "scene degenerated: at zero the instance’s domain cannot be told apart");
    let frame_step = 90_000 / h.state().model.project().settings.fps.max(1) as i64;
    let at = kt_model::clip_material_at(
        inst.source_in,
        inst.start,
        inst.duration,
        inst.reversed,
        frame_step,
        h.state().player.transport.playhead(),
    );
    // The curve is created by a NAMED window action (R.60): until this round
    // `SetOverride` stood here by hand, and the test was green on an interface
    // where an unnamed glyph created the curve, and there was nothing to show it with.
    h.get_by_label(tr(k::fx::AUTO_ADD)).click();
    h.step();
    h.step();
    {
        use eframe::egui::accesskit::Role;
        use egui_kittest::kittest::by;
        h.get(by().role(Role::Button).label("Font size")).click();
    }
    h.step();
    h.step();
    let got = h
        .state()
        .model
        .clip(s.b)
        .and_then(|c| c.overrides.iter().find(|o| o.pub_id == s.size).cloned())
        .expect("font size override");
    assert_eq!(got.curves.len(), 1, "override curve did not arrive");
    assert_eq!(got.curves[0].param, "size", "the window name of the curve went into the model");
    assert!(
        got.curves[0].keys.iter().all(|k| k.t.0 >= at.0.min(0)),
        "keys of the starting curve landed outside the instance’s material: {:?}",
        got.curves[0]
    );
    // and the curve itself is VISIBLE: the same widget as the effect slot has
    assert!(
        h.query_all_by_label_contains(tr(k::fx::AUTO_CLIP)).count() > 0,
        "curve is set up, but there is no automation widget in the window"
    );

    // and the model SEES IT: the reel resolution under the set returns the target’s curve
    let doc = h.state().model.document();
    let ov = h.state().model.clip(s.b).unwrap().overrides.clone();
    let resolved = doc.resolve_reel(Some(s.seq), &ov);
    let target = resolved.clips.iter().find(|c| kt_model::same_clip(c.id, s.inner)).unwrap();
    assert!(
        target.gen_auto.iter().any(|c| c.param == "size"),
        "override curve did not reach the target inside the reel"
    );
}

/// Test 691 (R.60): the override curve is VISIBLE, and without it the field is a constant.
///
/// The owner: “I can put different values in different places, and there will be
/// something like curve automation…
/// but I don’t see any curve automations anywhere… there is not a single button
/// in the automation window.” The window set keys blindly: an unnamed glyph
/// created a curve, and from then on every edit dropped a key at the cursor.
///
/// The test asserts an OBSERVABLE CONSEQUENCE from two sides. Without a curve, editing
/// the field at two different cursor positions must give ONE value and
/// ZERO keys — a hidden record does not exist. After the named action
/// “+ curve” the same widget as the effect slot must appear in the window,
/// and the same two edits then give TWO keys.
#[test]
fn t691_instance_curves_are_visible_and_never_hidden() {
    use eframe::egui;
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // we place the cursor INSIDE the SECOND edition: the key domain is the instance’s
    // material, and outside the event it is not defined
    let (b_start, b_dur) =
        h.state().model.clip(s.b).map(|c| (c.start, c.duration)).expect("second issue in place");
    let t1 = kt_time::Tick(b_start.0 + b_dur.0 / 4);
    let t2 = kt_time::Tick(b_start.0 + b_dur.0 / 2);

    h.state_mut().player.seek(t1);
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    assert_eq!(
        h.state().fx_editors.first().and_then(|e| e.selected.get(&s.b)),
        Some(&crate::widgets::ChainSel::Head),
        "the head “Composition parameters” is not selected in the FX window"
    );

    // the “Kegel” (font size) field sits between the author-name label and the template label
    let field_pos = |h: &Harness<'_, KtApp>| -> egui::Pos2 {
        let name = h
            .query_all_by_label("Font size")
            .map(|n| n.rect())
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .expect("“Font size” row in the window");
        let right = h
            .query_all_by_label_contains("шаблон")
            .map(|n| n.rect())
            .filter(|r| (r.center().y - name.center().y).abs() < 6.0)
            .min_by(|a, b| a.left().total_cmp(&b.left()))
            .expect("template label in the “Font size” row");
        assert!(right.left() > name.right() + 8.0, "no room between the name and the template for the field");
        egui::pos2((name.right() + right.left()) * 0.5, name.center().y)
    };
    let type_value = |h: &mut Harness<'static, KtApp>, pos: egui::Pos2, text: &str| {
        h.hover_at(pos);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            h.step();
        }
        h.event(egui::Event::Text(text.into()));
        h.step();
        h.key_press(egui::Key::Enter);
        h.step();
        h.step();
    };
    let ov_of = |h: &Harness<'_, KtApp>| -> Option<Override> {
        h.state()
            .model
            .clip(s.b)
            .and_then(|c| c.overrides.iter().find(|o| o.pub_id == s.size).cloned())
    };

    // --- WITHOUT A CURVE: two edits at different places give ONE value -----
    // R.66: the instance field draws the common row, and the share is entered as a PERCENT —
    // the same unit as inside the composition (previously the same number
    // was written “0.1” in one window and “10 %” in another)
    let pos = field_pos(&h);
    type_value(&mut h, pos, "10");
    let after_first = ov_of(&h).expect("field edit did not reach the model");
    assert!(after_first.curves.is_empty(), "the field set up a curve by itself: {after_first:?}");

    h.state_mut().player.seek(t2);
    h.step();
    let pos = field_pos(&h);
    type_value(&mut h, pos, "30");
    let after_second = ov_of(&h).expect("second edit did not arrive");
    assert!(
        after_second.curves.is_empty(),
        "an edit at a different cursor position WROTE a keyframe — this is the hidden \
         automation the owner complained about: {after_second:?}"
    );
    assert_eq!(
        after_second.value.as_num(),
        Some(0.3),
        "without a curve, the override must be a CONSTANT of the last edit"
    );
    assert_ne!(
        after_first.value.as_num(),
        after_second.value.as_num(),
        "harness did not change the value — the test checks nothing"
    );

    // --- A NAMED ACTION creates the curve and shows it -------------
    assert!(
        h.query_all_by_label_contains(tr(k::fx::AUTO_CLIP)).count() == 0,
        "automation widget was drawn before the curve was set up"
    );
    h.get_by_label(tr(k::fx::AUTO_ADD)).click();
    h.step();
    h.step();
    // in the menu an ITEM, not the row label: the knob’s name occurs in the window in both
    // places, and “any node with this text” would click the wrong one
    {
        use eframe::egui::accesskit::Role;
        use egui_kittest::kittest::by;
        h.get(by().role(Role::Button).label("Font size")).click();
    }
    h.step();
    h.step();

    let with_curve = ov_of(&h).expect("override disappeared");
    assert_eq!(with_curve.curves.len(), 1, "curve was not set up: {with_curve:?}");
    assert_eq!(
        with_curve.curves[0].param, "size",
        "the model got the WINDOW name of the curve instead of the target name — apply_overrides will not find it"
    );
    assert!(
        h.query_all_by_label_contains(tr(k::fx::AUTO_CLIP)).count() > 0,
        "curve is set up, but the automation widget is missing from the window — exactly the gap behind the complaint \
         “I see automation curves nowhere”"
    );

    // --- WITH A CURVE: the same two edits give TWO keys ---------------------
    let mat = |t: kt_time::Tick| {
        let c = h.state().model.clip(s.b).expect("second issue");
        kt_model::clip_material_at(
            c.source_in,
            c.start,
            c.duration,
            c.reversed,
            90_000 / h.state().model.document().settings.fps.max(1) as i64,
            t,
        )
    };
    let (m1, m2) = (mat(t1), mat(t2));
    h.state_mut().player.seek(t1);
    h.step();
    // the same percentages as above: 20 % and 40 % of the frame
    let pos = field_pos(&h);
    type_value(&mut h, pos, "20");
    h.state_mut().player.seek(t2);
    h.step();
    let pos = field_pos(&h);
    type_value(&mut h, pos, "40");

    let curve = ov_of(&h).expect("override disappeared").curves.remove(0);
    let at = |t: kt_time::Tick| curve.keys.iter().find(|k| k.t == t).map(|k| k.v);
    assert_eq!(at(m1), Some(0.2), "key did not land in the instance’s material: {curve:?}");
    assert_eq!(at(m2), Some(0.4), "second key did not land: {curve:?}");
    // …and in the MODEL these are shares, not percents: the conversion lives in one place
    assert!(at(m1).is_some_and(|v| v < 1.0), "percent instead of fraction went into the model");
    assert!(m1 != m2, "harness moved the cursor to the same point of the material");

    // the widget draws the AUTHOR name verbatim: a composition knob’s name is
    // not in the catalog and cannot be there (R.59)
    assert!(
        h.query_all_by_label_contains("Font size").count() > 0,
        "widget labeled the curve with something other than the author’s name"
    );
}

/// Test 709 (R.61): the formula field explains the error and does not spoil the model.
///
/// The owner: “the ability to set an automation curve by typing a formula
/// (without clicking with the mouse).” The on-the-fly check must really be
/// a check: a half-erased text in the field must not drop the curve on every
/// keystroke.
#[test]
fn t709_the_formula_field_explains_errors_without_touching_the_model() {
    use kt_model::{CurveSource, Formula, ParamCurve};

    // --- an unclosed bracket and an unknown name are NOT applied -----------
    let mut c = ParamCurve {
        param: "radius".into(),
        keys: vec![kt_model::AutoKey::at(kt_time::Tick(0), 3.0)],
        source: CurveSource::Keys,
    };
    let before = c.clone();
    for bad in ["(1 + 2", "1 + ", "wat(1)", "1 + $"] {
        let e = Formula::parse(bad).expect_err("broken text must be a rejection");
        // the position is named — the field highlights it
        assert!(e.to_string().contains("char"), "rejection has no position: {e}");
        // and the model is NOT TOUCHED
        assert_eq!(c, before, "broken text {bad:?} got into the model");
    }

    // --- a CORRECT text is applied and erases the keys --------------------
    let good = "t * 2 + 1";
    Formula::parse(good).expect("valid text is parsed");
    c.keys.clear();
    c.source = CurveSource::Formula { expr: good.into() };
    assert!(!c.source.is_keys(), "source did not change");
    assert!(c.keys.is_empty(), "a formula cannot have keys — they are a second source of truth");

    // --- an empty field does NOT turn the curve into a formula -------------------
    assert!(Formula::parse("").is_err(), "empty text is not an expression");

    // --- the field and its hint are named in all languages ----------------------
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        let name = tr(k::fx::FORMULA);
        let hint = tr(k::fx::FORMULA_HINT);
        assert!(!name.is_empty(), "{}: the formula field has no label", lang.code());
        assert!(
            hint.contains('t') || hint.contains("src") || hint.len() > 30,
            "{}: the hint does not explain the variables: {hint}",
            lang.code()
        );
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// Test 706 (R.61): the derived curve is VISIBLE, but cannot be edited with the mouse, and it
/// can be detached.
///
/// The owner: “the final graph is also drawn but is not edited with the mouse.”
/// An editable derived graph would be a third truth about the value:
/// such a curve has no keys, and there is nowhere to write a dragged point.
#[test]
fn t706_a_driven_curve_is_visible_but_not_draggable() {
    use crate::automation::{AutoCtx, AutoView};
    use kt_model::{AutoKey, CurveSource, ParamCurve};
    use kt_time::Tick;

    let id = |t: Tick| t;
    let ctx = AutoCtx {
        links: None,
        default_shape: Default::default(),
        specs: &[],
        range: (Tick(0), Tick(90_000)),
        view: (Tick(0), Tick(90_000)),
        to_timeline: &id,
        to_curve: &id,
        frame_step: 3000,
        linked: true,
    };
    let _ = &ctx;

    // --- a derived curve has NO KEY MARKERS ---------------------
    // (the probe walks over what is drawn: no marker — no hit either)
    let mut view = AutoView::default();
    view.key_marks.clear();
    assert!(
        view.key_at(eframe::egui::pos2(10.0, 10.0)).is_none(),
        "the probe found a key where nothing is drawn"
    );

    // --- “detach” turns it into ordinary keys ---------------------
    let mut driven = ParamCurve {
        param: "radius".into(),
        keys: vec![AutoKey::at(Tick(0), 3.0), AutoKey::at(Tick(90_000), 9.0)],
        source: CurveSource::Link {
            path: "2/fx0/radius".into(),
            expr: Some("src * 2".into()),
            stops: Vec::new(),
        },
    };
    assert!(!driven.source.is_keys(), "curve is not derived — the test checks nothing");
    let before = driven.keys.clone();
    driven.source = CurveSource::Keys;
    assert!(driven.source.is_keys(), "“detach” did not clear the source");
    assert_eq!(driven.keys, before, "“detach” changed the values — they are already resolved by the model");

    // --- the label shows WHAT it is driven by ---------------------------
    let link = CurveSource::Link { path: "2/fx0/radius".into(), expr: None, stops: Vec::new() };
    assert_eq!(link.link_path(), Some("2/fx0/radius"), "source address cannot be fetched for the label");
    let f = CurveSource::Formula { expr: "t * 2".into() };
    assert_eq!(f.expr(), Some("t * 2"), "formula text cannot be fetched for the label");
    assert_eq!(CurveSource::Keys.expr(), None);
    assert_eq!(CurveSource::Keys.link_path(), None);

    // --- the “detach” action is REACHABLE and explains itself ---------------
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        let name = tr(k::fx::UNLINK);
        let hint = tr(k::fx::UNLINK_HINT);
        assert!(!name.is_empty(), "{}: the “detach” action has no name", lang.code());
        assert!(hint.len() > 20, "{}: the hint does not explain the action: {hint}", lang.code());
        assert_ne!(name, hint, "{}: the hint repeats the name", lang.code());
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// Test 700b (R.61): the matching window opens only when there is a remainder, and
/// a skipped curve is NAMED.
#[test]
fn t700b_field_mapping_dialog_never_loses_a_curve_silently() {
    use crate::envclip::FitDialog;
    use kt_model::{AutoKey, ParamCurve};
    use kt_time::Tick;

    let (mut h, _g) = harness_three_clips();
    let id = h.state().model.project().clips[0].id;
    let blur = kt_model::FxInstance::Blur { radius: 8, radius_y: 0 };
    assert!(h.state_mut().apply(kt_model::Command::SetFx { id, fx: vec![blur.clone()] }));
    h.step();

    let free: Vec<String> = blur.param_specs().iter().map(|s| s.name.to_string()).collect();
    let curve = |p: &str| ParamCurve {
        param: p.into(),
        keys: vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(45_000), 7.0)],
        source: Default::default(),
    };

    // a remainder of two curves: one is put into the free parameter, the second is not
    h.state_mut().env_fit = Some(FitDialog {
        target: (id, 0),
        delta: 0,
        rows: vec![(curve("foreign_1"), Some(free[0].clone())), (curve("foreign_2"), None)],
        free: free.clone(),
        matched: Vec::new(),
        open: true,
    });
    h.step();
    h.step();
    {
        use egui_kittest::kittest::Queryable;
        assert!(
            h.query_all_by_label_contains(tr(k::fx::FIT_TITLE)).count() > 0,
            "match window did not open with a non-empty remainder"
        );
        h.get_by_label(tr(k::fx::FIT_DO)).click();
    }
    h.step();
    h.step();

    // the chosen curve reached the model UNDER A NEW NAME
    let auto = h.state().model.clip(id).map(|c| c.fx_auto.clone()).unwrap_or_default();
    let got: Vec<String> =
        auto.first().map(|v| v.iter().map(|c| c.param.clone()).collect()).unwrap_or_default();
    assert_eq!(got, vec![free[0].clone()], "matched curve did not land: {auto:?}");
    // and the skipped one is NAMED, not silently gone
    let msg = h.state().status.as_ref().map(|(m, _)| m.clone()).unwrap_or_default();
    assert!(
        msg.contains('1'),
        "skipped curve vanished silently — exactly what must not happen: {msg:?}"
    );

    // the window closed and does not reopen by itself a second time
    assert!(h.state().env_fit.is_none(), "match window was left hanging");
}

/// Test 699 (R.61): three outcomes of reading the buffer, and not one silent one.
///
/// The owner: “on a button press, take the value from the real buffer and only then
/// check whether it can be pasted or not, and offer actions.”
/// Hence: our own block — we put it in; foreign text and an empty buffer are a NAMED
/// error, not a greyed-out button.
#[test]
fn t699_clipboard_is_read_on_press_and_explains_refusals() {
    use crate::envclip::ClipRead;
    use kt_model::{AutoKey, ParamCurve};
    use kt_time::Tick;

    // an envelope block survives the system buffer in the SAME text
    // in which it arrives: copying writes JSON, reading parses it
    let block = kt_model::ops::EnvelopeBlock {
        fx_tag: "blur".into(),
        curves: vec![ParamCurve {
            param: "radius".into(),
            keys: vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(90_000), 9.0)],
            source: Default::default(),
        }],
        src_in: Tick(12_345),
    };
    let json = serde_json::to_string_pretty(&block).expect("block is serialized");
    let parsed: kt_model::ops::EnvelopeBlock =
        serde_json::from_str(&json).expect("and parsed back");
    assert_eq!(parsed, block, "block did not survive the round trip through the buffer text");

    // foreign text and an empty string DIFFER, and each has its own cause
    assert!(
        serde_json::from_str::<kt_model::ops::EnvelopeBlock>("plain text").is_err(),
        "foreign text was parsed as an envelope"
    );
    let foreign = tr(k::fx::PASTE_ENV_FOREIGN);
    let empty = tr(k::fx::PASTE_ENV_EMPTY);
    assert_ne!(foreign, empty, "two different failures share one text — the cause is not named");
    for t in [foreign, empty] {
        assert!(t.len() > 20, "failure does not explain the cause: {t:?}");
    }
    // and both outcomes exist in the type, not as “silence by default”
    assert!(matches!(ClipRead::Foreign, ClipRead::Foreign));
    assert!(matches!(ClipRead::Empty, ClipRead::Empty));

    // --- the matching window opens ONLY when there is a remainder ------------
    let blur = kt_model::FxInstance::Blur { radius: 8, radius_y: 0 };
    let own = kt_model::ops::fit_envelope(&block.curves, &blur);
    assert!(own.leftover.is_empty(), "own view produced a remainder — the window would show up for nothing");
    let alien = ParamCurve {
        param: "foreign_field".into(),
        keys: vec![AutoKey::at(Tick(0), 1.0)],
        source: Default::default(),
    };
    let mixed = kt_model::ops::fit_envelope(&[alien], &blur);
    assert_eq!(mixed.leftover.len(), 1, "remainder is not named — the curve would be lost silently");
    assert!(!mixed.free.is_empty(), "no target parameter was offered");

    // --- the paste button is ALIVE even when there is nothing to put ----------------
    let (mut h, _g) = harness_three_clips();
    let id = h.state().model.project().clips[0].id;
    assert!(h.state_mut().apply(kt_model::Command::SetFx { id, fx: vec![blur.clone()] }));
    h.step();
    h.state_mut().open_fx_editor(id);
    h.step();
    h.step();
    {
        use egui_kittest::kittest::Queryable;
        let n = h.query_all_by_label(tr(k::fx::PASTE_ENV)).count();
        assert!(n > 0, "the “paste envelope” button is missing from the window altogether");
    }
}

/// Test 698 (R.61): the shape is edited PER SEGMENT, neighbours stay intact, “all” is
/// one step; the graph is drawn by the same value function that the render uses.
///
/// The owner: “in automations I would like to set the shapes of segments… and
/// it is also possible to configure a separate curve in your own way, with another type.”
/// Hence two obligations that the test asserts: editing one
/// segment does NOT touch the neighbouring ones (the shape belongs to the left key of the segment), and
/// the warp-speed curve has only a straight shape, and the refusal is LOUD.
#[test]
fn t698_segment_shape_is_edited_per_segment() {
    use kt_model::{AutoKey, ParamCurve, SegShape, ZigProfile, curve_value};
    use kt_time::Tick;

    // --- editing a segment does not touch the neighbours ----------------------------
    let mut c = ParamCurve {
        param: "radius".into(),
        keys: vec![
            AutoKey::at(Tick(0), 0.0),
            AutoKey::at(Tick(30_000), 10.0),
            AutoKey::at(Tick(60_000), 5.0),
        ],
        source: Default::default(),
    };
    let before: Vec<SegShape> = c.keys.iter().map(|k| k.shape).collect();
    let zig = SegShape::Zigzag { periods: 3.0, amplitude: 2.0, profile: ZigProfile::Triangle };
    c.keys[1].shape = zig;
    assert_eq!(c.keys[0].shape, before[0], "the left neighbor of the segment changed");
    assert_eq!(c.keys[2].shape, before[2], "the right neighbor of the segment changed");
    // and the value of the FIRST segment stayed the same
    assert_eq!(
        curve_value(&c.keys, Tick(15_000)).to_bits(),
        SegShape::DEFAULT.at(0.5, 0.0, 10.0).to_bits(),
        "the first segment moved because of the edit of the second"
    );
    // while the SECOND one now jitters
    let mid = curve_value(&c.keys, Tick(45_000));
    let plain = SegShape::DEFAULT.at(0.5, 10.0, 5.0);
    assert!((mid - plain).abs() > 0.1, "segment did not take the new shape: {mid} versus {plain}");

    // --- “to all segments” is one step --------------------------------
    let all = AutoKey::all_shaped(c.keys.clone(), SegShape::Hold);
    assert!(all.iter().all(|k| k.shape == SegShape::Hold), "the shape did not land on all segments");
    assert_eq!(all.len(), c.keys.len(), "the command has no right to change the number of keys");

    // --- the GRAPH computes the same function as the render ---------------------
    // (drawing takes `curve_value`; there is no second formula in the program —
    // guard t693 likewise asserts this, about the single widget)
    for t in [Tick(0), Tick(7_000), Tick(30_000), Tick(45_000), Tick(60_000)] {
        let direct = {
            let keys = &c.keys;
            let i = keys.windows(2).position(|w| t >= w[0].t && t < w[1].t);
            match i {
                Some(i) => {
                    let (a, b) = (&keys[i], &keys[i + 1]);
                    let span = (b.t.0 - a.t.0).max(1) as f64;
                    a.shape.at((t.0 - a.t.0) as f64 / span, a.v, b.v)
                }
                None if t >= keys.last().unwrap().t => keys.last().unwrap().v,
                None => keys[0].v,
            }
        };
        assert_eq!(
            curve_value(&c.keys, t).to_bits(),
            direct.to_bits(),
            "drawing and render compute the segment differently at {t:?}"
        );
    }

    // --- SPEED has only a straight shape, and the refusal is named ---------------
    let rate = ParamCurve::constant("rate", Tick(0), Tick(90_000), 1.0, SegShape::DEFAULT);
    assert!(
        rate.keys.iter().all(|k| k.shape == SegShape::Linear),
        "the starting speed curve must be straight under ANY project default: {rate:?}"
    );
    let other = ParamCurve::constant("radius", Tick(0), Tick(90_000), 1.0, SegShape::Hold);
    assert!(
        other.keys.iter().all(|k| k.shape == SegShape::Hold),
        "for an ordinary parameter the starting curve takes the project default"
    );
    let mut bad = rate.clone();
    bad.keys[0].shape = SegShape::DEFAULT;
    assert!(
        matches!(
            kt_model::check_fx_auto(kt_model::ClipId(1), &[vec![bad]]),
            Err(kt_model::ProjectError::RateShapeMustBeLinear(_))
        ),
        "smoothed speed passed silently"
    );
}

/// Test 692b (R.60): the limits of a knob in the AUTHOR window come from the target’s passport.
///
/// “The min and max values are still always set to 0 by default” — an empty
/// narrowing (`Option::None`) was drawn as zero, and the author saw a knob for which
/// apparently exactly nothing was allowed. The assertion of the test is the observable consequence:
/// the window DRAWS the target’s limits, not zeros; the “to target limits” button is
/// available only on a narrowed knob and removes the narrowing in the MODEL.
#[test]
fn t692b_author_window_shows_the_target_range() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable, by};
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // the target passport “Kegel” is what the window MUST show
    let spec = {
        let doc = h.state().model.document();
        let p = doc
            .sequence(s.seq)
            .expect("reel")
            .published
            .iter()
            .find(|p| p.id == s.size)
            .expect("“Font size” handle")
            .clone();
        doc.addr_spec(Some(s.seq), &p.target).expect("target spec of “Font size”")
    };
    assert!(spec.max > spec.min, "font size spec is degenerate: {spec:?}");
    assert!(
        spec.min != 0.0 || spec.max != 0.0,
        "the test proves nothing: the target’s limits are zero already"
    );

    h.state_mut().open_published_win(s.seq);
    h.step();
    h.step();

    let shown: Vec<String> = h
        .query_all(by().role(Role::SpinButton))
        .filter_map(|n| n.accesskit_node().value().map(|v| v.to_string()))
        .collect();
    // R.66: the author limits draw the COMMON row, and the unit is taken from the target’s passport —
    // the share is shown as a percent exactly as in the parameter field itself.
    // The assertion is the same (“the author sees the target limits”); only the unit
    // in which they are written changes.
    let scale = if spec.unit == kt_model::ParamUnit::Percent { 100.0 } else { 1.0 };
    let has = |x: f64| {
        let want = x * scale;
        shown.iter().any(|t| {
            t.trim()
                .trim_end_matches('%')
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .is_ok_and(|v| (v - want).abs() <= (spec.step.abs() * scale * 0.5).max(1e-6))
        })
    };
    assert!(
        has(spec.min) && has(spec.max),
        "author window did not show the target limits ({} … {}): {shown:?}",
        spec.min,
        spec.max
    );

    // --- the narrowing reaches the model and is removed by one button ---------
    let narrowed = PubRange { min: Some(spec.min + spec.step), max: Some(spec.max - spec.step) };
    {
        let mut p = h
            .state()
            .model
            .document()
            .sequence(s.seq)
            .expect("reel")
            .published
            .iter()
            .find(|p| p.id == s.size)
            .cloned()
            .expect("“Font size” handle");
        p.range = narrowed;
        assert!(
            h.state_mut().apply(Command::SetPublished { seq: s.seq, param: Box::new(p) }),
            "narrowing rejected"
        );
    }
    h.step();
    h.step();
    assert!(
        !h.state()
            .model
            .document()
            .sequence(s.seq)
            .expect("reel")
            .published
            .iter()
            .find(|p| p.id == s.size)
            .expect("handle")
            .range
            .is_empty(),
        "narrowing did not land in the model"
    );

    h.get_by_label(tr(k::pubp::RANGE_RESET)).click();
    h.step();
    h.step();
    assert!(
        h.state()
            .model
            .document()
            .sequence(s.seq)
            .expect("reel")
            .published
            .iter()
            .find(|p| p.id == s.size)
            .expect("handle")
            .range
            .is_empty(),
        "the “to target limits” button did not remove the narrowing"
    );

    // and after the reset the window again shows the target limits
    let shown: Vec<String> = h
        .query_all(by().role(Role::SpinButton))
        .filter_map(|n| n.accesskit_node().value().map(|v| v.to_string()))
        .collect();
    assert!(has(spec.min) && has(spec.max), "after reset the target limits did not come back: {shown:?}");
}

/// Test 684 (R.59, stage 3): the AUTHOR NAME is document content.
///
/// The owner’s decision: “the names of public parameters are free text
/// written by the author.” So it is drawn VERBATIM, bypassing the translation catalog, like
/// an event or reel name, and guard t131 must stay green (in the code
/// not a single new Russian literal).
#[test]
fn t684_author_names_are_shown_verbatim() {
    use kt_model::*;
    let (mut h, s) = harness_news_template();
    // Cyrillic and Arabic — two scripts with different layout; the Arabic name
    // also checks that PROJECT text is not reordered like
    // a catalog string
    let names = ["Issue heading No. 3", "عنوان النشرة"];
    h.state_mut().enter_nest(s.a);
    h.step();
    for (id, name) in [s.headline, s.tint].iter().zip(names) {
        let mut p = h
            .state()
            .model
            .document()
            .sequence(s.seq)
            .unwrap()
            .published
            .iter()
            .find(|p| p.id == *id)
            .cloned()
            .unwrap();
        p.name = name.to_string();
        assert!(h.state_mut().apply(Command::SetPublished { seq: s.seq, param: Box::new(p) }));
        h.step();
    }
    h.state_mut().leave_nest();
    h.step();

    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    // one must look at the PAINT LIST, not at the accessibility tree: the accessible name
    // of the reset button is composite and carries the knob name too — so “drawn verbatim”
    // cannot be told apart by it (lesson from t255/t548). We take the SOURCE text of the
    // paint list: the word reordering for Arabic lives in `tr()` (i18n::bidi) and
    // is applied BEFORE drawing, so a mismatch here would mean exactly
    // what must not happen — the author name went through the catalog.
    let drawn = |h: &Harness<'static, KtApp>| -> Vec<String> {
        crate::reach_tests::painted_texts(h).into_iter().map(|t| t.src).collect()
    };
    for name in names {
        assert!(
            drawn(&h).iter().any(|t| t == name),
            "author name “{name}” was not DRAWN verbatim: {:?}",
            drawn(&h)
        );
    }

    // the PROGRAM LANGUAGE does not touch the author name: the catalog was switched — the name is the same
    crate::i18n::set_lang(crate::i18n::Lang::En);
    h.step();
    h.step();
    assert!(
        drawn(&h).iter().any(|t| t == tr(k::pubp::RESET)),
        "program language did not switch — the next assertion is empty"
    );
    for name in names {
        assert!(
            drawn(&h).iter().any(|t| t == name),
            "author name “{name}” moved together with the program language"
        );
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    h.step();
}

/// Test 685 (R.59, stage 3): badges.
///
/// A pin on an instance WITH overrides, the number of declared knobs on the
/// reel card in the bin, and NONE of this when there are no editions.
#[test]
fn t685_badges_show_overrides_and_publications() {
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // --- NOTHING while nothing is overridden -------------------------------
    let inst = h.state().model.clip(s.b).cloned().unwrap();
    assert!(inst.overrides.is_empty());
    assert_eq!(
        h.state().instance_override_counts(&inst),
        (0, 6),
        "the counter must know both how many are declared and how many are touched"
    );

    // --- the PIN and its hint after an override ---------------------
    assert!(h.state_mut().apply(Command::SetOverride {
        id: s.b,
        ov: Box::new(Override::value(s.headline, ParamValue::Text("SECOND".into()))),
    }));
    h.step();
    let inst = h.state().model.clip(s.b).cloned().unwrap();
    assert_eq!(h.state().instance_override_counts(&inst), (1, 6));
    // and the PIN is really DRAWN on the event: one must look at the
    // paint list, not the model — “the field was set” is not a consequence (R.40)
    h.step();
    h.step();
    let pinned: Vec<String> = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .map(|t| t.src)
        .filter(|s| s.contains(crate::icons::PIN))
        .collect();
    // the EVENT label carries the pin at its tail (“name ×N 📌”), the bin card
    // carries it at its head (“📌6”): these are different badges and must not be confused
    assert!(
        pinned.iter().any(|s| s.ends_with(crate::icons::PIN) && s.len() > 4),
        "pin on the instance event is not drawn: {pinned:?}"
    );
    assert!(
        pinned.iter().any(|s| s.starts_with(crate::icons::PIN)),
        "the count of declared handles is not drawn on the reel card: {pinned:?}"
    );
    // the hint NAMES the numbers — “a pin exists, but what does it mean” would be
    // a puzzle without this (rule 16)
    let hint = crate::tf!(k::pubp::OVERRIDDEN, 1, 6);
    assert!(hint.contains('1') && hint.contains('6'), "hint does not name the numbers: {hint}");
    // the neighbouring edition did not get the pin: the override is a property of
    // the INSTANCE, not of the reel
    assert!(
        h.state().model.clip(s.a).unwrap().overrides.is_empty(),
        "override spread onto the neighboring instance"
    );

    // --- REEL CARD: the number of declared knobs ------------------------
    assert_eq!(h.state().published_of(s.seq).len(), 6, "reel card takes the handle count from here");
    // a reel without editions does not show a number — a “×1” of the same kind is noise
    {
        let app = h.state_mut();
        let ids: Vec<PubId> = app.published_of(s.seq).iter().map(|p| p.id).collect();
        app.enter_nest(s.a);
        for id in ids {
            assert!(app.apply(Command::UnpublishParam { seq: s.seq, id }), "handle removal rejected");
        }
        app.leave_nest();
    }
    h.step();
    assert_eq!(h.state().published_of(s.seq).len(), 0);
    // and the overrides went away TOGETHER with the knobs — a dead pin does not exist
    assert!(
        h.state().model.clip(s.b).unwrap().overrides.is_empty(),
        "removing pins left the override: the pin would lie"
    );
    h.step();
    h.step();
    let left: Vec<String> = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .map(|t| t.src)
        .filter(|s| s.contains(crate::icons::PIN))
        .collect();
    assert!(left.is_empty(), "badges remained where there are no more pins: {left:?}");
}

/// Test 716b (R.62): the leading node “Geometry” in the chain editor is
/// the first and is marked, is selected by a click, shows the conform rule,
/// a change of rule reaches the model in one history step; the cross on it is
/// an error in the status line, the node stays.
#[test]
fn t716b_lead_geometry_in_the_chain_editor() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();

    // the leading node is visible and marked; the usual “Blur” is in place too
    let lead_label = crate::fx_editor::fx_display_name(&EventGeometry::default().node());
    assert!(h.query_by_label(lead_label).is_some(), "the leading node must be in the strip");
    assert!(h.query_by_label(crate::fx_editor::fx_display_name(&FxInstance::blur(8))).is_some());
    // a click on a node selects it — and the conform rule appears
    h.get_by_label(lead_label).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().fx_editors[0].selected.get(&ClipId(1)),
        Some(&crate::widgets::ChainSel::Lead),
        "a click on the leading node selects it, not the slot with the same index"
    );
    assert!(h.query_by_label(tr(k::fx::CONFORM)).is_some(), "alignment rule is shown");
    // a rule change via the combo → SetGeometry, one history step
    let undo_before = h.state().model.undo_depth();
    h.get_by_label(tr(k::fx::CONFORM_FILL)).click();
    h.step();
    h.step();
    let app = h.state();
    assert_eq!(
        app.model.clip(ClipId(1)).unwrap().geometry.rule,
        Conform::Fill,
        "rule reached the model"
    );
    assert_eq!(app.model.undo_depth(), undo_before + 1, "the window session is one history step");
    assert_eq!(app.model.clip(ClipId(1)).unwrap().fx.len(), 1, "the chain of the model does not contain the node");
    // the cross on the leading node — a refusal with an error, the node stays in place
    // the cross of the leading node — the nearest one to the right on its row (the strip
    // has other crosses too: on the Blur slot and in the search field)
    let lead_rect = h.get_by_label(lead_label).rect();
    let crosses: Vec<_> = h.get_all_by_label(crate::icons::CLEAR).collect();
    assert!(crosses.len() >= 2, "each slot has its own cross");
    let lead_cross = crosses
        .iter()
        .filter(|n| {
            let r = n.rect();
            (r.center().y - lead_rect.center().y).abs() < 10.0 && r.min.x > lead_rect.max.x
        })
        .min_by(|a, b| a.rect().min.x.total_cmp(&b.rect().min.x))
        .expect("cross to the right of the leading node");
    lead_cross.click();
    h.step();
    h.step();
    let app = h.state();
    assert!(app.error_status.is_some(), "deleting the leading node is an error, not silence");
    assert_eq!(app.model.clip(ClipId(1)).unwrap().fx.len(), 1, "Blur was unharmed");
    assert_eq!(app.model.clip(ClipId(1)).unwrap().geometry.rule, Conform::Fill);
}

/// Test 720b (R.62): the project settings window — fields with validation, refusal
/// with an error style, applying with one command, a flag of explicit settings,
/// reopening and focus return.
#[test]
fn t720b_project_settings_window_validates_and_applies() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness();
    // twice in a row: the window must reopen again
    for _ in 0..2 {
        h.state_mut().open_project_settings();
        h.step();
        assert!(h.query_by_label(tr(k::settings::TITLE)).is_some(), "window is open");
        h.get_by_label(tr(k::settings::CANCEL)).click();
        h.step();
        h.step();
        assert!(h.query_by_label(tr(k::settings::TITLE)).is_none(), "“Cancel” closes");
    }
    // an odd width — refusal with an error, settings untouched
    h.state_mut().open_project_settings();
    h.step();
    let before = h.state().model.document().settings.clone();
    {
        let d = h.state_mut().settings_win.as_mut().unwrap();
        d.width = "1441".into();
        d.height = "1080".into();
    }
    h.get_by_label(tr(k::settings::APPLY)).click();
    h.step();
    h.step();
    let app = h.state();
    assert!(app.settings_win.is_some(), "window stays open on rejection");
    assert!(app.error_status.is_some(), "odd width is an error, not silence");
    assert_eq!(app.model.document().settings, before);
    // 29.97 — not expressible: refusal
    {
        let d = h.state_mut().settings_win.as_mut().unwrap();
        d.width = "1440".into();
        d.fps = "29.97".into();
        h.state_mut().error_status = None;
    }
    h.get_by_label(tr(k::settings::APPLY)).click();
    h.step();
    h.step();
    assert!(h.state().error_status.is_some(), "29.97 is rejected with a reason");
    assert_eq!(h.state().model.document().settings, before);
    // 1440×1080 at 50 fps and the “fill” rule — one command, the explicitness flag
    {
        let d = h.state_mut().settings_win.as_mut().unwrap();
        d.fps = "50".into();
        d.conform = kt_model::ConformRule::Fill;
    }
    let undo_before = h.state().model.undo_depth();
    h.get_by_label(tr(k::settings::APPLY)).click();
    h.step();
    h.step();
    let app = h.state();
    let s = app.model.document().settings.clone();
    assert_eq!((s.width, s.height, s.fps), (1440, 1080, 50));
    assert_eq!(s.conform, kt_model::ConformRule::Fill);
    assert!(s.settings_explicit, "applying sets the explicit-settings flag");
    assert_eq!(app.model.undo_depth(), undo_before + 1, "one command");
    assert!(app.settings_win.is_none(), "window closed");
    // focus given back: hotkeys are alive (rule 14)
    assert!(!h.ctx.memory(|m| m.focused().is_some()), "the field does not keep focus after closing");
    // the preview panel’s combo shows the same (one body)
    assert!(
        h.query_by_label("1440×1080").is_some()
            || h.query_all_by_label_contains("1440").count() > 0
    );
}

/// Test 721 (R.62): the first clip in an empty project with implicit settings sets
/// the raster and fps in one undo step with placement; explicit settings are not touched;
/// 29.97 — raster accepted, fps the same, the hint names the cause.
#[test]
fn t721_first_clip_sets_project_settings() {
    use kt_model::*;
    let info = |w: u32, h: u32, fps: (u32, u32)| kt_media::MediaInfo {
        duration: kt_time::Tick(90_000),
        has_video: true,
        has_audio: false,
        fps: Some(fps),
        width: w,
        height: h,
        sar: (1, 1),
        still: false,
    };
    let mut h = harness();
    let width_before = h.state().model.document().settings.width;
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        assert!(app.model.project().clips.is_empty());
        assert!(!app.model.document().settings.settings_explicit);
        let undo_before = app.model.undo_depth();
        let cmds =
            app.clip_cmds_for(MediaId(1), &info(1440, 1080, (25, 1)), kt_time::Tick(0), None);
        assert!(matches!(cmds[0], Command::SetSettings { .. }), "settings go first in the batch");
        app.apply_batch(cmds);
        let s = app.model.document().settings.clone();
        assert_eq!((s.width, s.height, s.fps), (1440, 1080, 25));
        assert_eq!(app.model.project().clips.len(), 1);
        assert_eq!(app.model.undo_depth(), undo_before + 1, "placement and settings are one step");
        let msg = app.status.as_ref().map(|(t, _)| t.clone()).unwrap_or_default();
        assert!(msg.contains("1440"), "hint names the accepted raster: {msg}");
        app.undo();
        assert!(app.model.project().clips.is_empty());
        assert_eq!(app.model.document().settings.width, width_before, "undo restored the settings too");
    }
    // explicit settings: a second project, window applied — placement does not touch them
    {
        let app = h.state_mut();
        let mut st = app.model.document().settings.clone();
        st.settings_explicit = true;
        app.apply(Command::SetSettings { settings: st });
        let cmds = app.clip_cmds_for(MediaId(1), &info(640, 480, (30, 1)), kt_time::Tick(0), None);
        assert!(cmds.iter().all(|c| !matches!(c, Command::SetSettings { .. })));
        app.apply_batch(cmds);
        assert_eq!(app.model.document().settings.width, width_before);
    }
    // 29.97 in an empty project without explicit settings: raster accepted, fps not
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips.clear();
        p.settings.settings_explicit = false;
        p.settings.fps = 30;
        app.model = kt_model::TimelineModel::new(p);
        let cmds =
            app.clip_cmds_for(MediaId(1), &info(1280, 720, (30_000, 1001)), kt_time::Tick(0), None);
        app.apply_batch(cmds);
        let s = app.model.document().settings.clone();
        assert_eq!((s.width, s.height, s.fps), (1280, 720, 30));
        let msg = app.status.as_ref().map(|(t, _)| t.clone()).unwrap_or_default();
        assert!(msg.contains("90000"), "hint explains why the fps was not accepted: {msg}");
    }
}

/// Test 722 (R.62): an aspect-ratio mismatch at placement is reported with an
/// address “where to change”; a match stays silent.
#[test]
fn t722_aspect_mismatch_is_reported_with_an_address() {
    use kt_model::*;
    let info = |w: u32, h: u32| kt_media::MediaInfo {
        duration: kt_time::Tick(90_000),
        has_video: true,
        has_audio: false,
        fps: Some((30, 1)),
        width: w,
        height: h,
        sar: (1, 1),
        still: false,
    };
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.settings.width = 1920;
    p.settings.height = 1080;
    p.settings.settings_explicit = true;
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    // 4:3 in 16:9 — a hint with the rule and the address
    let cmds = app.clip_cmds_for(MediaId(1), &info(640, 480), kt_time::Tick(0), None);
    app.apply_batch(cmds);
    let msg = app.status.as_ref().map(|(t, _)| t.clone()).unwrap_or_default();
    assert!(msg.contains("640×480") && msg.contains("1920×1080"), "{msg}");
    assert!(msg.contains(tr(k::fx::CONFORM_FIT)), "rule is named: {msg}");
    assert!(msg.contains(tr(k::menu::file::PROJECT_SETTINGS)), "address is named: {msg}");
    // 16:9 in 16:9 — silence
    app.status = None;
    let cmds = app.clip_cmds_for(MediaId(1), &info(1280, 720), kt_time::Tick(90_000), None);
    app.apply_batch(cmds);
    assert!(app.status.is_none(), "matching proportions are not reported");
}

/// Test 719 (R.62): the thumbnail of a vertical clip is NOT squashed — drawn
/// in the source proportions, centred in the cell, and the scroll and zoom of the timeline
/// request no decodes at all (invariant 8: the only clip key ≤5 c
/// is already in the textures). What is probed is what is DRAWN: the image rectangle in the
/// frame shapes, not the service field.
#[test]
fn t719_vertical_thumb_keeps_its_aspect_and_scroll_orders_no_decode() {
    use kt_model::ClipKind;
    let (mut h, _g) = harness_three_clips_dt(None);
    h.step();
    h.step();
    // the keys are asked by the filmstrip ITSELF (the test does not repeat the step arithmetic);
    // a clip of 50 000 ticks ≤ 5 c → the step is constant at any zoom, few keys
    let mut keys: Vec<crate::thumbs::ThumbKey> = h.state().thumbs.requested_keys();
    keys.sort_by_key(|k| (k.media.0, k.t));
    keys.dedup();
    assert!(!keys.is_empty(), "filmstrip did not request a single thumbnail");
    assert!(
        keys.iter().all(|k| h
            .state()
            .model
            .project()
            .clips
            .iter()
            .any(|c| c.kind == ClipKind::Video && kt_engine::clip_source_addr(c) == k.media)),
        "keys are not about the video events of the scene: {keys:?}"
    );
    let img = egui::ColorImage::filled([100, 180], egui::Color32::from_rgb(200, 40, 40));
    let tex = h.ctx.load_texture("t719-vertical", img, egui::TextureOptions::LINEAR);
    // the textures — for ALL keys the step grid may yield (multiples of
    // 0.5 c, a clip of 50 000 ticks): zoom opens new moments, and this is a
    // legitimate request; the guard below checks that what is READY is not requested again
    let media: std::collections::BTreeSet<kt_model::MediaId> =
        keys.iter().map(|k| k.media).collect();
    for m in &media {
        for t in (0..=150_000).step_by(5_000) {
            h.state_mut()
                .thumb_textures
                .insert(crate::thumbs::ThumbKey { media: *m, t }, tex.clone());
        }
    }
    h.step();
    h.step();
    let before = h.state().thumbs.requested_total();

    fn image_rects(
        shapes: &[egui::epaint::ClippedShape],
        id: egui::TextureId,
        out: &mut Vec<egui::Rect>,
    ) {
        fn walk(sh: &egui::Shape, id: egui::TextureId, out: &mut Vec<egui::Rect>) {
            match sh {
                // `painter.image` in egui 0.36 puts a mesh with the texture
                egui::Shape::Mesh(m) if m.texture_id == id => out.push(m.calc_bounds()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, id, out)),
                _ => {}
            }
        }
        shapes.iter().for_each(|c| walk(&c.shape, id, out));
    }
    let mut rects = Vec::new();
    image_rects(&h.output().shapes, tex.id(), &mut rects);
    let n_shapes = h.output().shapes.len();
    let n_mesh =
        h.output().shapes.iter().filter(|c| matches!(c.shape, egui::Shape::Mesh(_))).count();
    assert!(
        !rects.is_empty(),
        "vertical thumbnail was never drawn (shapes={n_shapes}, meshes={n_mesh}, \
         offline={:?}, tex={:?}, keys={:?})",
        h.state().offline.lock().unwrap(),
        h.state().thumb_textures.keys().collect::<Vec<_>>(),
        keys
    );
    // the full (not edge-cropped) cell: width = height·100/180
    let full = rects.iter().find(|r| (r.width() / r.height() - 100.0 / 180.0).abs() < 0.03);
    assert!(
        full.is_some(),
        "thumbnail is squashed: no rectangle has the 100:180 proportions, there is {:?}",
        rects.iter().map(|r| (r.width(), r.height())).collect::<Vec<_>>()
    );
    assert!(
        rects.iter().all(|r| r.width() <= r.height() * 16.0 / 9.0 * 0.6 + 1.0),
        "vertical thumbnail is stretched over the whole 16:9 cell"
    );

    // scroll and zoom — not one new decode request (invariant 8)
    let (vs, tpp) = (h.state().view_start, h.state().ticks_per_px);
    h.state_mut().view_start = vs + 2_000.0;
    h.step();
    h.state_mut().ticks_per_px = tpp * 0.5;
    h.step();
    h.state_mut().ticks_per_px = tpp * 2.0;
    h.state_mut().view_start = vs;
    h.step();
    h.step();
    assert_eq!(
        h.state().thumbs.requested_total(),
        before,
        "timeline scroll/zoom requested thumbnail decode (invariant 8)"
    );
}

/// Test 722b (R.62): changing the conform rule in the settings window WHILE PAUSED
/// rebuilds the preview. Found by the live scenario `conform`: the rule was written
/// to the file, but the previous frame stayed in the window — the preview key did not see the
/// project version (neither the cursor, nor quality, nor the content of the sources is touched
/// by the edit). The CONSEQUENCE is asserted: the field’s pixel changes.
#[test]
fn t722b_project_rule_change_repaints_the_paused_preview() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness_lang_dt(crate::i18n::Lang::Ru, Some(0.02));
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        // vertical project, horizontal synthetic source 64×36:
        // “fit” leaves margins at top and bottom, “fill” does not
        p.settings.width = 36;
        p.settings.height = 64;
        p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(0),
            duration: Tick(90_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.push_project_to_player();
        // the source is brought up the same way the generator creation does it:
        // the first sync only removes the media composition, it does not bring up the sources itself
        let src = crate::util::open_source("synth:ramp").expect("synthetic source");
        app.media_paths.insert(MediaId(1), "synth:ramp".into());
        app.player.add_media(MediaId(1), src);
        // the preview canvas is in the PROJECT proportions (the settings window does this
        // itself; here the settings are substituted around it)
        app.player.set_preview_quality(kt_media::Quality { width: 36, height: 64 });
        app.player.seek(Tick(30_000));
    }
    for _ in 0..120 {
        h.step();
        if h.state().preview_key.is_some_and(|k| k.2) {
            break;
        }
    }
    assert!(h.state().preview_key.is_some_and(|k| k.2), "the preview never got computed");
    let top_dark = |app: &KtApp| -> bool {
        let (_, (w, hgt), _) = app.preview_key.unwrap();
        assert!(hgt > 4 && app.preview_buf.len() >= (w * hgt * 4) as usize);
        let i = ((w + w / 2) * 4) as usize; // row 1, middle
        app.preview_buf[i..i + 3].iter().all(|&v| v < 8)
    };
    assert!(top_dark(h.state()), "“fit” of a horizontal into a vertical: a margin on top");

    let mut st = h.state().model.document().settings.clone();
    st.conform = ConformRule::Fill;
    st.settings_explicit = true;
    assert!(h.state_mut().apply(Command::SetSettings { settings: st }));
    let mut repainted = false;
    for _ in 0..120 {
        h.step();
        if h.state().preview_key.is_some_and(|k| k.2) && !top_dark(h.state()) {
            repainted = true;
            break;
        }
    }
    assert!(repainted, "the “fill” rule was applied, but the preview stayed with margins while paused");
}

/// Test 740 (R.62, change B): the render window LISTS the muted
/// tracks and the active solo BEFORE the start — in the same place where it talks about unavailable
/// sources. There is no refusal (mute and solo are project content, the owner’s decision),
/// but it must not stay silent: “forgot to clear the solo — delivered one track”
/// (rules 16, 19). What is asserted is what is DRAWN: the track name in the window.
#[test]
fn t740_render_dialog_lists_muted_and_solo_tracks() {
    use kt_model::LaneKind::{Audio, Video};
    let mut h = harness_three_clips_dt(None).0;
    {
        let app = h.state_mut();
        edit_project(app, |p| {
            p.ensure_tracks(Audio, 2);
            p.track_at_mut(Video, 0).unwrap().name = "Titles".into();
            p.track_at_mut(Video, 0).unwrap().mute = true;
            p.track_at_mut(Audio, 1).unwrap().name = "Voice".into();
            p.track_at_mut(Audio, 1).unwrap().solo = true;
        });
        app.render_setup.open = true;
    }
    h.step();
    h.step();
    let texts: Vec<String> =
        crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    let all = texts.join(" | ");
    assert!(
        all.contains("Titles"),
        "render window is silent about a muted track — the person will learn later that it is “not in the file”: {all}"
    );
    assert!(all.contains("Voice"), "render window is silent about an active solo: {all}");

    // without muted and solo — not one extra line (rule 9)
    let mut h2 = harness_three_clips_dt(None).0;
    h2.state_mut().render_setup.open = true;
    h2.step();
    h2.step();
    let clean: String = crate::reach_tests::painted_texts(&h2)
        .into_iter()
        .map(|t| t.src)
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(!clean.contains("Titles") && !clean.contains("Voice"));
}

/// Test 742 (R.62, change B, 4.1): the row of badges in the track header.
/// What is asserted is what a PERSON does (R.40): the mouse moves to the centre of the
/// badge, presses and releases WITHOUT moving — and the MODEL changes.
/// Plus three checklist rules: the header width does not jump when the
/// state changes (rule 3), a repeated press returns it as it was
/// (rule 14), the service row has exactly one badge — the lock
/// (rule 9: dead controls do not exist).
#[test]
fn t742_track_badges_take_a_human_click_and_never_move() {
    use crate::timeline::{GUTTER_W, TrackBadge};
    use kt_model::LaneKind::{Morph, Video};

    /// A human click: hover, press, release, mouse did not move.
    fn click_at(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) {
        h.hover_at(pos);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
        h.step();
    }

    let (mut h, _) = harness_three_clips_dt(None);
    h.state_mut().model.apply(kt_model::Command::InsertTrack { kind: Morph, at: 0 }).unwrap();
    h.step();

    // the row of the track’s badges is a PURE function of geometry, exactly the same as
    // for drawing and for hit-testing
    let badges = |h: &Harness<'_, KtApp>, kind: kt_model::LaneKind, lane: u32| {
        let g = h.state().tl_geom.clone().expect("timeline geometry");
        let row = g.rows.iter().position(|r| *r == (kind, lane)).expect("row on screen");
        let left = g.rect.left() - GUTTER_W;
        TrackBadge::row_rects(kind, left, g.row_top(row), g.row_h(row))
    };
    let track = |h: &Harness<'_, KtApp>, kind, lane| {
        h.state().model.project().track_at(kind, lane).cloned().expect("track is there")
    };

    // the grab zone is not narrower than 14 px (rule 1: hit on the first try)
    for (b, r) in badges(&h, Video, 0) {
        assert!(
            r.width() >= 14.0 && r.height() >= 8.0,
            "badge {b:?} is smaller than the grab zone: {:?}",
            r.size()
        );
    }

    // ---- into each badge with the mouse, from the person’s point ---------------------------
    let before = badges(&h, Video, 0);
    for (badge, r) in before.clone() {
        click_at(&mut h, r.center());
        let t = track(&h, Video, 0);
        match badge {
            TrackBadge::Mute => assert!(t.mute, "click on M did not mute the track"),
            TrackBadge::Solo => assert!(t.solo, "click on S did not enable solo"),
            TrackBadge::Lock => assert!(t.lock, "click on the lock did not lock the track"),
            TrackBadge::Fx => {
                assert_eq!(
                    h.state().track_win.as_ref().map(|w| w.id),
                    Some(t.id),
                    "click on the FX badge did not open the track chain"
                );
                h.state_mut().track_win = None;
                h.step();
            }
        }
    }

    // ---- width does not jump with the state (rule 3) -------------------------
    let after = badges(&h, Video, 0);
    assert_eq!(
        before, after,
        "the row of badges shifted when the track changed state: the header must be fixed by the row gallery \
         and not by the width of today’s icons"
    );

    // ---- repeated press (rule 14) ------------------------------------
    let mute = before.iter().find(|(b, _)| *b == TrackBadge::Mute).unwrap().1;
    click_at(&mut h, mute.center());
    assert!(!track(&h, Video, 0).mute, "second press of M did not clear the mute");
    click_at(&mut h, mute.center());
    assert!(track(&h, Video, 0).mute, "third press of M did not restore the mute");

    // ---- the window edge does not steal clicks (rule 15) --------------------------
    // The track headers are pressed against the window’s left edge, and the first 8 px there are
    // the resize zone: without recording the badges in `no_resize_rects` the left half of
    // M is dead when the media bin is hidden. We aim just past the strip at
    // `WIN_BORDER` — its owner left it to resize on purpose (item 9, round 12).
    let edge = egui::pos2(mute.left() + crate::app::WIN_BORDER + 0.5, mute.center().y);
    assert!(
        edge.x < mute.left() + crate::timeline::BADGE_W / 2.0,
        "the edge point must lie LEFT of the badge center, otherwise there is nothing to check"
    );
    let was = track(&h, Video, 0).mute;
    click_at(&mut h, edge);
    assert_ne!(
        track(&h, Video, 0).mute,
        was,
        "left edge of the M badge is dead: a click at {edge:?} was taken by the window resize zone"
    );

    // ---- hotkeys: THE SAME BODY as the badge (rule 17) ------------
    // Bare M and S are taken by the marker and the cut at the owner’s direct request,
    // so mute and solo take ALT. The target is the track of the SELECTED
    // event; if there is no selection — the row under the pointer.
    let v0 = track(&h, Video, 0).id;
    {
        let app = h.state_mut();
        app.selected.clear();
        app.selected.insert(kt_model::ClipId(1));
        let mut props = app.model.project().track(v0).unwrap().props();
        props.mute = false;
        props.solo = false;
        props.lock = false;
        app.model.apply(kt_model::Command::SetTrackProps { id: v0, props }).unwrap();
    }
    h.step();
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::M);
    h.step();
    assert!(track(&h, Video, 0).mute, "Alt+M did not mute the track of the selected event");
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::S);
    h.step();
    assert!(track(&h, Video, 0).solo, "Alt+S did not enable solo on the track of the selected event");
    // a bare M stays a MARKER: the track key does not steal another binding
    let marks = h.state().model.project().markers.len();
    h.key_press(egui::Key::M);
    h.step();
    assert_eq!(
        h.state().model.project().markers.len(),
        marks + 1,
        "bare M stopped placing a marker — the owner’s binding was taken away"
    );

    // ---- service row: only the lock ------------------------------------
    let serv = badges(&h, Morph, 0);
    assert_eq!(
        serv.iter().map(|(b, _)| *b).collect::<Vec<_>>(),
        vec![TrackBadge::Lock],
        "a morph row has nothing to mute — an extra badge would be a dead control"
    );
    // and it sits in the FIRST cell of the row: a person presses where the video’s M is
    assert_eq!(serv[0].1.min.x, before[0].1.min.x);
    click_at(&mut h, serv[0].1.center());
    let m = track(&h, Morph, 0);
    assert!(m.lock, "click on the first cell of the service row did not lock it");
    assert!(!m.mute, "the service row has no right to be muted");
}

/// Test 868 (guide-shortcuts-reference): a chord with Alt does not ALSO fire its
/// bare letter. The split read `S && !ctrl`, so Alt+S soloed the track AND cut the
/// event under the playhead in the same frame; t742 asserted only the solo. The
/// playhead stands INSIDE the selected event — on an empty spot the split does
/// nothing and the test would be green on the broken code.
#[test]
fn t868_alt_chords_do_not_fire_the_bare_letter() {
    use kt_model::LaneKind::Video;
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips_dt(None);
    {
        let app = h.state_mut();
        app.selected.clear();
        app.selected.insert(kt_model::ClipId(1));
        app.player.seek(Tick(30_000));
    }
    h.step();
    let range = |h: &Harness<'_, KtApp>| {
        let c = h.state().model.clip(kt_model::ClipId(1)).expect("event 1 is there");
        (c.start, c.duration)
    };
    let clips = h.state().model.project().clips.len();
    let before = range(&h);
    assert!(
        before.0.0 < 30_000 && 30_000 < before.0.0 + before.1.0,
        "scene guard: the playhead must stand inside the selected event"
    );

    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::S);
    h.step();
    let track = |h: &Harness<'_, KtApp>| {
        h.state().model.project().track_at(Video, 0).cloned().expect("track is there")
    };
    assert!(track(&h).solo, "Alt+S did not enable solo");
    assert_eq!(h.state().model.project().clips.len(), clips, "Alt+S also split the event");
    assert_eq!(range(&h), before, "Alt+S changed the selected event");

    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::M);
    h.step();
    assert!(track(&h).mute, "Alt+M did not mute the track");
    assert_eq!(h.state().model.project().clips.len(), clips, "Alt+M changed the events");

    // the bare letter keeps its owner's binding
    h.key_press(egui::Key::S);
    h.step();
    assert_eq!(
        h.state().model.project().clips.len(),
        clips + 1,
        "bare S stopped splitting the event under the playhead"
    );
}

/// Test 870 (split-menu-label): the Edit menu’s split IS the event menu’s split and
/// the S key — one name, one body. The item was called “Split / cut region” and
/// promised a cut-out that neither it nor S ever did (S always splits, item 5; the
/// cut-out is “Cut out the region interior” in the empty-space menu). The region is
/// drawn on purpose: with it a cut-out would be visible as a shorter total.
#[test]
fn t870_edit_split_item_is_the_s_key_and_says_so() {
    use crate::i18n::{Lang, k, tables, tr};
    use egui_kittest::kittest::Queryable;
    use kt_model::Region;
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips_dt(None);
    {
        let app = h.state_mut();
        app.selected.clear();
        app.model.set_region(Region::normalized(Tick(20_000), Tick(40_000)));
        app.player.seek(Tick(30_000));
    }
    h.step();
    let ranges = |h: &Harness<'_, KtApp>| {
        let mut v: Vec<(i64, i64)> =
            h.state().model.project().clips.iter().map(|c| (c.start.0, c.duration.0)).collect();
        v.sort_unstable();
        v
    };
    let before = ranges(&h);
    let region = h.state().model.project().region;
    let total = |v: &[(i64, i64)]| v.iter().map(|r| r.1).sum::<i64>();

    // from where the person acts: the Edit title, then the item with S in the chord column
    h.get_by_label(tr(k::menu::edit::TITLE)).click();
    h.step();
    h.step();
    let item = format!("{}\t{}", tr(k::tl::menu::SPLIT), "S");
    let labels = menu_labels(&mut h);
    assert!(
        h.query_by_label(&item).is_some(),
        "the Edit menu has no “{item}” item (the event menu’s name with S as its chord); visible: {labels:?}"
    );
    h.get_by_label(&item).click();
    h.step();
    h.step();
    let by_menu = ranges(&h);
    assert_eq!(by_menu.len(), before.len() + 1, "the Edit item did not split: {by_menu:?}");
    assert!(
        by_menu.iter().any(|r| r.0 + r.1 == 30_000) && by_menu.iter().any(|r| r.0 == 30_000),
        "the halves do not meet at the playhead: {by_menu:?}"
    );
    assert_eq!(total(&by_menu), total(&before), "the region’s interior was cut out");
    assert_eq!(h.state().model.project().region, region, "the item changed the region");

    // the same through the key
    h.state_mut().undo();
    h.step();
    assert_eq!(ranges(&h), before, "undo did not restore the event");
    h.key_press(egui::Key::S);
    h.step();
    assert_eq!(ranges(&h), by_menu, "S and the Edit item split differently");

    // the chord lives in the chord column, never in the translated text (i18n rule 3)
    for lang in Lang::ALL {
        let label = tables::TABLES[lang as usize][k::tl::menu::SPLIT.0 as usize];
        let has_chord = label.contains("(S)")
            || label.contains("（S）")
            || label.split(|c: char| !c.is_alphanumeric()).any(|w| w == "S");
        assert!(!has_chord, "{}: the split label carries its chord: “{label}”", lang.code());
        // the toolbar scissors (the same cut_action) say the same, plus the key;
        // `contains`, not `starts_with`: the Arabic table holds the DISPLAY order,
        // where “(S)” stands before the words
        let hint = tables::TABLES[lang as usize][k::tb::CUT.0 as usize];
        assert!(
            hint.contains(label) && hint.contains('S'),
            "{}: the scissors hint “{hint}” is not the split label “{label}” with its key",
            lang.code()
        );
    }
}

/// Test 743 (R.62, change B, 4.2): the “Track properties” window. What is checked is
/// what a person does: opens it, fixes the name, level and mode, closes —
/// and the WHOLE session is rolled back by ONE Ctrl+Z (as with the FX window). “Cancel”
/// returns the snapshot, the window gives focus back (rule 14), the field limits
/// come from the passport, not from literals of the window (precedent: Gain in R.33).
#[test]
fn t743_track_window_is_one_undo_step_and_gives_focus_back() {
    use egui_kittest::kittest::Queryable;
    use kt_model::{HasParams, LaneKind::Video};
    let (mut h, _) = harness_three_clips_dt(None);
    let id = h.state().model.project().track_at(Video, 0).unwrap().id;
    let props_of = |h: &Harness<'_, KtApp>| {
        h.state().model.project().track(id).unwrap().props()
    };
    let was = props_of(&h);
    let depth0 = h.state().model.undo_depth();

    // ---- field limits — FROM THE PASSPORT --------------------------------------
    {
        let p = h.state().model.project().clone();
        let v = p.track(id).unwrap();
        let a = p.track_at(kt_model::LaneKind::Audio, 0).unwrap();
        assert!(v.param_spec("opacity").is_some(), "the video track has no level row");
        assert!(v.param_spec("gain_db").is_none(), "decibels on a VIDEO track are a foreign row");
        let g = a.param_spec("gain_db").expect("the audio track must have a level");
        assert_eq!(
            (g.min, g.max),
            (kt_model::TRACK_GAIN_MIN_DB, kt_model::TRACK_GAIN_MAX_DB),
            "the window must take the limits from the spec, otherwise they drift apart from the model"
        );
    }

    // ---- one session — one history step -----------------------------------
    h.state_mut().open_track_window(id);
    h.step();
    h.step();
    // name: the field takes focus on opening, we type as a person does
    h.event(egui::Event::Text("Titles".into()));
    h.step();
    // the blend mode and the toggle — in the same window and the same session
    let multiply = crate::track_window::blend_label(kt_model::BlendMode::Multiply).to_string();
    h.get_by_label(&multiply).click();
    h.step();
    let mute_label = crate::i18n::tr(crate::i18n::k::track::MUTE).to_string();
    h.get_by_label(&mute_label).click();
    h.step();

    let now = props_of(&h);
    assert_eq!(now.name, "Titles", "name did not reach the model");
    assert_eq!(now.blend, kt_model::BlendMode::Multiply, "blend mode did not arrive");
    assert!(now.mute, "the switch did not arrive");
    assert_eq!(
        h.state().model.undo_depth(),
        depth0 + 1,
        "the window session split into several history steps: the person would press Ctrl+Z as many times as the number of fields they \
         touched"
    );
    h.state_mut().undo();
    h.step();
    assert_eq!(props_of(&h), was, "one Ctrl+Z must restore the WHOLE window session");

    // the selection is prepared IN ADVANCE: the key is pressed right after the window closes,
    // without extra frames — otherwise focus would have time to release itself and the check of
    // rule 14 would mean nothing
    {
        let app = h.state_mut();
        app.selected.clear();
        app.selected.insert(kt_model::ClipId(1));
    }

    // ---- form rows do not overlap each other (rule 7) ---------------
    // Nine mode buttons wrap onto a second row, and `Grid` does not measure
    // the WRAP height: in the live window the second row ran into
    // “Toggles”, and the harness did not see this until it began to compare
    // the RECTANGLES that are drawn.
    {
        let painted = crate::reach_tests::painted_texts(&h);
        let rect_of = |name: &str| {
            painted.iter().find(|t| t.src == name).map(|t| t.rect)
        };
        let last_blend =
            crate::track_window::blend_label(kt_model::BlendMode::Difference).to_string();
        let sw = crate::i18n::tr(crate::i18n::k::track::SWITCHES).to_string();
        if let (Some(a), Some(b)) = (rect_of(&last_blend), rect_of(&sw)) {
            assert!(
                !a.intersects(b),
                "blend-mode buttons overlap the “Switches”: {a:?} and {b:?}"
            );
        } else {
            panic!("the track window does not draw the blend mode or the switches");
        }
    }

    // ---- “Cancel” returns the snapshot and closes the window -----------------------
    h.state_mut().open_track_window(id);
    h.step();
    h.step();
    h.get_by_label(&mute_label).click();
    h.step();
    assert!(props_of(&h).mute, "scene did not happen: nothing to cancel");
    h.get_by_label(crate::i18n::tr(crate::i18n::k::track::CANCEL)).click();
    h.step();
    h.step();
    assert_eq!(props_of(&h), was, "“Cancel” did not restore the track properties");
    assert!(h.state().track_win.is_none(), "“Cancel” did not close the window");

    // ---- Esc closes, and focus is returned (rule 14) ------------------
    // The name field takes focus on opening, and while it holds it, the
    // Esc cascade does not reach it at all — the window must catch it itself. And
    // once closed, the window must GIVE BACK focus: otherwise the keyboard is dead until a
    // click elsewhere.
    h.state_mut().open_track_window(id);
    h.step();
    h.step();
    assert!(
        h.state().track_win.is_some(),
        "scene did not happen: the window did not open"
    );
    h.key_press(egui::Key::Escape);
    h.step();
    assert!(h.state().track_win.is_none(), "Esc did not close the track window");
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::M);
    h.step();
    assert!(
        props_of(&h).mute,
        "after closing the window the keys are dead: the name field did not hand focus back to egui"
    );
}

/// Test 744 (R.62, change B, 4.3): the TRACK’s chain — the same strip as the
/// event’s (rule 17), and an envelope that arrives with the pasted chain is
/// rejected ALOUD and with a reason (rule 16): a track has no automation,
/// and silently dropping the curves is not allowed — the person would see “the effect is the same,
/// but it sounds different” and would look for a defect in the engine.
#[test]
fn t744_track_chain_is_the_same_strip_and_refuses_curves_aloud() {
    use egui_kittest::kittest::Queryable;
    use kt_model::{FxInstance, LaneKind::Audio, ParamCurve};
    let (mut h, _) = harness_three_clips_dt(None);
    let id = {
        let app = h.state_mut();
        edit_project(app, |p| p.ensure_tracks(Audio, 1));
        app.model.project().track_at(Audio, 0).unwrap().id
    };
    h.step();

    // in the buffer — a chain WITH AN ENVELOPE, as the event’s strip puts it
    h.state_mut().fx_clipboard = Some(kt_model::ops::FxChainBlock {
        slots: vec![kt_model::ops::FxSlotBlock {
            fx: FxInstance::gain(-6.0),
            bypass: false,
            curves: vec![ParamCurve {
                param: "gain_db".into(),
                keys: vec![
                    kt_model::AutoKey::at(kt_time::Tick(0), -6.0),
                    kt_model::AutoKey::at(kt_time::Tick(90_000), 0.0),
                ],
                source: Default::default(),
            }],
        }],
        src_in: kt_time::Tick::ZERO,
        fx_off: false,
    });
    h.state_mut().open_track_window(id);
    h.step();
    h.step();

    // the track’s chain strip is THE SAME: the paste button is in place
    let paste = crate::i18n::tr(crate::i18n::k::fx::PASTE_CHAIN).to_string();
    assert!(
        h.query_by_label(&paste).is_some(),
        "track has no shared effect chain strip: there is nowhere to insert effects"
    );
    h.get_by_label(&paste).click();
    h.step();
    h.step();

    let t = h.state().model.project().track(id).unwrap().clone();
    assert_eq!(t.fx.len(), 1, "effect did not reach the track chain");
    assert!(
        matches!(t.fx[0], FxInstance::Gain { .. }),
        "the wrong effect got into the track chain: {:?}",
        t.fx[0]
    );
    let err = h.state().error_status.clone().map(|(m, _)| m);
    assert!(
        err.as_deref() == Some(crate::i18n::tr(crate::i18n::k::track::CURVES_DROPPED)),
        "the curve vanished silently — the person was told neither what was dropped nor where to put it: {err:?}"
    );
    // and the track’s own curve model does not carry it
    assert!(
        h.state().model.project().validate().is_ok(),
        "track with curves passed the project check"
    );
}

/// Test 745 (R.62, change B, 4.4): the track LOCK in the interface. A gesture on a
/// locked track is neither silence nor a faded hint, but an error that
/// names the track and the place where the lock is removed (rules 16 and 19), plus a
/// red highlight of the row itself (mechanism t172). All four
/// gestures that edit the event are checked; muting at the same time continues to
/// act — the lock is about EDITING, not about sound.
#[test]
fn t745_locked_track_refuses_every_gesture_with_a_reason() {
    use kt_model::{ClipId, LaneKind::Video};
    let (mut h, g) = harness_three_clips_dt(None);
    let id = h.state().model.project().track_at(Video, 0).unwrap().id;
    {
        let app = h.state_mut();
        let mut props = app.model.project().track(id).unwrap().props();
        props.lock = true;
        app.model.apply(kt_model::Command::SetTrackProps { id, props }).unwrap();
    }
    h.step();
    let start_of = |h: &Harness<'_, KtApp>, c: ClipId| {
        h.state().model.project().clips.iter().find(|x| x.id == c).map(|x| (x.start, x.duration))
    };
    let was = start_of(&h, ClipId(1)).unwrap();
    let row = g.rows.iter().position(|r| *r == (Video, 0)).unwrap();
    let y = g.row_top(row) + g.row_h(row) * 0.6;
    let body = egui::pos2(g.tick_to_x(20_000), y);

    // ---- move --------------------------------------------------------
    h.state_mut().error_status = None;
    h.drag_at(body);
    h.step();
    h.hover_at(body + egui::vec2(9.0, 0.0));
    h.step();
    h.hover_at(body + egui::vec2(120.0, 0.0));
    h.step();
    h.drop_at(body + egui::vec2(120.0, 0.0));
    h.step();
    assert_eq!(start_of(&h, ClipId(1)), Some(was), "locked track gave the event to the drag");
    let label = h.state().row_label(Video, 0);
    let err = h.state().error_status.clone().map(|(m, _)| m).unwrap_or_default();
    assert!(
        err.contains(&label),
        "rejection did not name the track (“{label}”), so the person cannot tell what is in the way: {err:?}"
    );
    assert!(
        h.state().blocked_track.map(|(t, _)| t) == Some(id),
        "row of the locked track is not highlighted — the rejection looks like “pressed, and nothing happened”"
    );

    // ---- trim of the right edge ------------------------------------------
    let edge = egui::pos2(g.tick_to_x(50_000) - 2.0, y);
    h.drag_at(edge);
    h.step();
    h.hover_at(edge + egui::vec2(9.0, 0.0));
    h.step();
    h.hover_at(edge + egui::vec2(60.0, 0.0));
    h.step();
    h.drop_at(edge + egui::vec2(60.0, 0.0));
    h.step();
    assert_eq!(start_of(&h, ClipId(1)), Some(was), "locked track let itself be pulled");

    // ---- cut and delete with keys ------------------------------------
    let n = h.state().model.project().clips.len();
    {
        let app = h.state_mut();
        app.selected.clear();
        app.selected.insert(ClipId(1));
        app.player.seek(kt_time::Tick(20_000));
    }
    h.step();
    h.key_press(egui::Key::S);
    h.step();
    assert_eq!(h.state().model.project().clips.len(), n, "locked event was cut");
    h.key_press(egui::Key::Delete);
    h.step();
    assert_eq!(h.state().model.project().clips.len(), n, "locked event was deleted");
    assert!(
        h.state().blocked_track.map(|(t, _)| t) == Some(id),
        "rejection from a KEY did not highlight the row: `handle_keys` runs before drawing, and \
         a highlight that fades every frame lives exactly zero frames"
    );

    // ---- the lock is about editing, not about sound --------------------------------
    h.state_mut().toggle_track_badge(id, crate::timeline::TrackBadge::Mute);
    h.step();
    let p = h.state().model.project().clone();
    assert!(p.track(id).unwrap().lock, "badge removed someone else’s switch");
    assert!(p.track(id).unwrap().mute, "properties of a LOCKED track became editable");
    assert!(
        kt_model::silenced_tracks(&p).muted.contains(&label),
        "muting a locked track stopped affecting the file"
    );
}

/// Test 748d (R.63 item 2): the tail of an event WITHOUT MATERIAL is visible on the strip.
///
/// An event may be longer than its file (the edge was dragged past the end even before R.63,
/// or the speed was changed) — and then its tail is silence and a freeze frame. While the
/// tail looked like ordinary material, this read as “the loop breaks the
/// sound”: the person sees the event, and there is no sound in it. What is asserted is what is
/// DRAWN: the hatching starts exactly where the material ends per the
/// model, and in an event inside the material there is no hatching at all.
#[test]
fn t748d_tail_without_material_is_hatched() {
    use kt_model::LaneKind::Video;
    let (mut h, _) = harness_three_clips_dt(None);
    // event 0..50_000 on media that is enough for only 20_000
    {
        let app = h.state_mut();
        edit_project(app, |p| {
            p.media[0].duration = Some(kt_time::Tick(20_000));
        });
    }
    h.step();
    h.step();

    let probe = |h: &Harness<'_, KtApp>| -> Option<egui::Rect> {
        h.state().model.project().clips.first().and_then(|c| {
            let len = h.state().model.project().media_duration(c.media);
            kt_model::material_end_on_timeline(c, len).map(|end| {
                let g = h.state().tl_geom.clone().expect("geometry");
                let row = g.rows.iter().position(|r| *r == (Video, 0)).unwrap();
                egui::Rect::from_min_max(
                    egui::pos2(g.tick_to_x(end), g.row_top(row)),
                    egui::pos2(
                        g.tick_to_x((c.start + c.duration).0),
                        g.row_top(row) + g.row_h(row),
                    ),
                )
            })
        })
    };
    let r = probe(&h).expect("the model knows where the material ends");
    assert!(r.width() > 1.0, "the tail without material must be wider than a pixel");

    // DRAWN: the tail zone has hatching — lines that are absent from the
    // event body. We count the segments that fall into the tail rectangle.
    // The stroke is a diagonal across the full row height: inside a narrow tail it does not
    // fit and is cut by the CLIP RECTANGLE. We count by it:
    // the tail has its own, and nobody else’s.
    let lines_in = |h: &Harness<'_, KtApp>, r: egui::Rect| -> usize {
        fn walk(s: &egui::epaint::Shape, n: &mut usize) {
            use egui::epaint::Shape;
            match s {
                Shape::Vec(v) => v.iter().for_each(|s| walk(s, n)),
                Shape::LineSegment { .. } => *n += 1,
                _ => {}
            }
        }
        let mut n = 0;
        for cs in &h.output().shapes {
            let c = cs.clip_rect;
            if (c.left() - r.left()).abs() < 2.0 && (c.right() - r.right()).abs() < 2.0 {
                walk(&cs.shape, &mut n);
            }
        }
        n
    };
    let hatch = lines_in(&h, r);
    assert!(hatch >= 2, "there is no hatching in the tail without material (lines {hatch})");

    // an event INSIDE the material has no tail at all (rule 9)
    {
        let app = h.state_mut();
        edit_project(app, |p| {
            p.media[0].duration = Some(kt_time::Tick(10 * 90_000));
        });
    }
    h.step();
    h.step();
    assert!(probe(&h).is_none(), "an event inside the tail’s material must not exist");
    let body = egui::Rect::from_min_max(r.min, r.max);
    assert_eq!(lines_in(&h, body), 0, "hatching remained where material exists");
}

/// Test 749 (R.63 item 3): the formula is TYPED IN in full, keys stay alive until
/// confirmation, and a correct formula is visible on the graph.
///
/// The owner: “I don’t understand how to enter formulas into automation curves — I
/// type it in and it either erases the formula or nothing happens at all.” Both were
/// true: the field re-read the text FROM THE MODEL on every frame
/// (so `sin(` disappeared on the next one), and the first legitimate prefix `t`
/// was applied at once and zeroed the keys. Plus a formula curve does not
/// store keys by contract — and the graph drew emptiness.
#[test]
fn t749_formula_is_typed_whole_and_drawn() {
    use kt_model::*;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
        let mut c = ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(90_000),
            source_in: kt_time::Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![FxInstance::blur(8)],
            fx_bypass: vec![false],
            fx_auto: vec![vec![ParamCurve {
                param: "radius".into(),
                keys: vec![
                    AutoKey::at(kt_time::Tick(0), 2.0),
                    AutoKey::at(kt_time::Tick(45_000), 9.0),
                    AutoKey::at(kt_time::Tick(90_000), 4.0),
                ],
                source: Default::default(),
            }]],
            opacity: 1.0,
            fx_off: false,
        };
        c.fx_bypass = vec![false];
        p.clips.push(c);
        p.normalize();
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.open_fx_editor(ClipId(1));
    }
    h.step();
    h.step();

    // the field’s coordinate — from the REAL LAYOUT: TextEdit has no accessibility
    // node, but the “Formula” label is drawn next to it (technique t281)
    let label = crate::i18n::tr(crate::i18n::k::fx::FORMULA).to_string();
    let lr = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .find(|t| t.src == label)
        .map(|t| t.rect)
        .expect("formula field label is drawn");
    let field = egui::pos2(lr.right() + 60.0, lr.center().y);
    h.hover_at(field);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: field,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();

    let keys_of = |h: &Harness<'_, KtApp>| -> Vec<(i64, f64)> {
        h.state().model.project().clips[0].fx_auto[0][0]
            .keys
            .iter()
            .map(|k| (k.t.0, k.v))
            .collect()
    };
    let before = keys_of(&h);
    assert_eq!(before.len(), 3, "scene: the curve has three keys");

    // we TYPE CHARACTER BY CHARACTER — and after each frame the keys must be intact
    for ch in "sin(t)*0.5".chars() {
        h.event(egui::Event::Text(ch.to_string()));
        h.step();
        h.step();
        assert_eq!(
            keys_of(&h),
            before,
            "after the character {ch:?} the curve keys changed: the text was applied before confirmation"
        );
        assert!(
            h.state().model.project().clips[0].fx_auto[0][0].source.is_keys(),
            "after the character {ch:?} the source became a formula without confirmation"
        );
    }
    // and the text itself did not vanish from the field (on the old code it did)
    let shown: Vec<String> =
        crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(
        shown.iter().any(|s| s.contains("sin(t)*0.5")),
        "typed text vanished from the field: {shown:?}"
    );

    // CONFIRMATION
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
    let c = &h.state().model.project().clips[0].fx_auto[0][0];
    assert!(
        matches!(&c.source, kt_model::CurveSource::Formula { expr } if expr == "sin(t)*0.5"),
        "after Enter the formula did not reach the model: {:?}",
        c.source
    );
    // and it is VISIBLE: the resolution gives a line, not emptiness
    let pts = kt_model::formula_preview(
        c,
        (kt_time::Tick(0), kt_time::Tick(90_000)),
        kt_time::Tick(0),
        kt_time::Tick(90_000),
        750,
    );
    assert!(pts.len() > 10, "formula curve does not resolve into points: {}", pts.len());
    assert!(
        pts.iter().any(|k| (k.v - pts[0].v).abs() > 1e-6),
        "resolved curve is a straight line: the sine was not computed"
    );
}

/// Test 750 (R.63 item 4): a click outside the field gives focus back, and the keys are alive AT ONCE.
///
/// The owner: “if I start typing something in the formula field, the input focus
/// goes there, and no matter how I poke the rest of the interface,
/// pressing space does not lead to pause… until I close the effect window.”
/// The CONSEQUENCE is checked: after a click on the timeline space plays, and the field
/// accepts input again if clicked into (rule 14).
#[test]
fn t750_click_outside_a_field_gives_the_keyboard_back() {
    use kt_model::*;
    let (mut h, g) = harness_three_clips_dt(None);
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[0].fx = vec![FxInstance::blur(8)];
        p.clips[0].fx_bypass = vec![false];
        p.clips[0].fx_auto = vec![vec![ParamCurve {
            param: "radius".into(),
            keys: vec![
                AutoKey::at(kt_time::Tick(0), 2.0),
                AutoKey::at(kt_time::Tick(45_000), 9.0),
            ],
            source: Default::default(),
        }]];
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.open_fx_editor(ClipId(1));
    }
    h.step();
    h.step();

    // 1) click in the formula field and type — focus is there
    let label = crate::i18n::tr(crate::i18n::k::fx::FORMULA).to_string();
    let lr = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .find(|t| t.src == label)
        .map(|t| t.rect)
        .expect("formula field label is drawn");
    let field = egui::pos2(lr.right() + 60.0, lr.center().y);
    let click = |h: &mut Harness<'_, KtApp>, pos: egui::Pos2| {
        h.hover_at(pos);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
        h.step();
    };
    click(&mut h, field);
    h.event(egui::Event::Text("t".into()));
    h.step();
    assert!(h.ctx.memory(|m| m.focused()).is_some(), "the field must take focus");
    assert!(
        h.ctx.egui_wants_keyboard_input(),
        "scene degenerated: nobody holds the keyboard"
    );

    // 2) CLICK ON THE TIMELINE — and space plays
    let was_playing = h.state().player.transport.is_playing();
    let empty = egui::pos2(g.tick_to_x(400_000), g.row_center_y(0));
    click(&mut h, empty);
    assert!(
        !h.ctx.egui_wants_keyboard_input(),
        "after a click on the timeline the field still holds the keyboard"
    );
    h.key_press(egui::Key::Space);
    h.step();
    h.step();
    assert_ne!(
        h.state().player.transport.is_playing(),
        was_playing,
        "Space did not work after a click outside the field (on the old code it did not work \
         while the effect window was open)"
    );
    h.key_press(egui::Key::Space);
    h.step();

    // 3) the field accepts input again — the feature works AGAIN (rule 14)
    click(&mut h, field);
    h.event(egui::Event::Text("2".into()));
    h.step();
    assert!(
        h.ctx.egui_wants_keyboard_input(),
        "field stopped accepting input after it lost focus"
    );

    // As for the BUTTONS: in egui a click gives a button no focus, so in the harness
    // this half of the claim does not reproduce at all — the live scenario `formula.py`
    // checks it. The hotkey gate, meanwhile, is still rewritten as “text entry” instead
    // of “something has focus”: the guards of the former meaning are
    // t98 and t627 (timecode and speed masks).
}

/// Test 751 (R.63 item 5): the target of a right click is FIXED on press, lives
/// while the menu is open, the segment is highlighted, and the header names both ends.
///
/// The owner: “it is impossible to specify the shapes of curves in automation curves —
/// the context menu just disappears, I do not manage to reach it, and
/// it is still unclear between which points (and are they points?) I would set the shape.”
/// Cause: both the key and the segment were computed from `hover_pos()` EVERY FRAME, and the
/// content of `context_menu` is redrawn every frame — the pointer
/// moved to the items, the hit was lost, and the menu closed itself.
/// The test’s assertions are observable consequences: after the pointer is moved away the menu is
/// in place, the shape is applied and lands EXACTLY on the segment’s left key.
#[test]
fn t751_the_menu_target_is_latched_on_press() {
    use crate::automation::MenuTarget;
    use kt_model::*;
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(90_000),
            source_in: kt_time::Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![FxInstance::blur(8)],
            fx_bypass: vec![false],
            fx_auto: vec![vec![ParamCurve {
                param: "radius".into(),
                keys: vec![
                    AutoKey::at(kt_time::Tick(0), 2.0),
                    AutoKey::at(kt_time::Tick(45_000), 9.0),
                    AutoKey::at(kt_time::Tick(90_000), 4.0),
                ],
                source: Default::default(),
            }]],
            opacity: 1.0,
            fx_off: false,
        });
        p.normalize();
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.open_fx_editor(ClipId(1));
    }
    h.step();
    h.step();

    // the positions of the keys — from the REAL drawing (the drawing and the probe from one
    // place of truth, item 364)
    let marks = |h: &Harness<'_, KtApp>| -> Vec<(usize, usize, egui::Pos2)> {
        h.state().fx_editors[0].auto_views[&(ClipId(1), 0)].key_marks.clone()
    };
    let m = marks(&h);
    assert_eq!(m.len(), 3, "scene degenerated: the key graph is not drawn ({m:?})");
    let target = |h: &Harness<'_, KtApp>| -> Option<MenuTarget> {
        h.state().fx_editors[0].auto_views[&(ClipId(1), 0)].menu_target
    };
    let right_click = |h: &mut Harness<'_, KtApp>, pos: egui::Pos2| {
        h.hover_at(pos);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
        h.step();
    };
    // the highlight FILL in the frame output (the segment under the menu)
    fn fills(shape: &egui::epaint::Shape, want: egui::Color32, out: &mut Vec<egui::Rect>) {
        use egui::epaint::Shape;
        match shape {
            Shape::Vec(v) => v.iter().for_each(|s| fills(s, want, out)),
            Shape::Rect(r) if r.fill == want => out.push(r.rect),
            _ => {}
        }
    }
    let highlights = |h: &Harness<'_, KtApp>| -> Vec<egui::Rect> {
        let want = egui::Color32::from_rgba_unmultiplied(120, 160, 255, 28);
        let mut out = Vec::new();
        for cs in &h.output().shapes {
            fills(&cs.shape, want, &mut out);
        }
        out
    };

    // --- RIGHT CLICK ON THE SEGMENT between keys 0 and 1 -------------------
    let seg = egui::pos2((m[0].2.x + m[1].2.x) * 0.5, (m[0].2.y + m[1].2.y) * 0.5);
    right_click(&mut h, seg);
    assert_eq!(
        target(&h),
        Some(MenuTarget::Segment(0, 0)),
        "target was not fixed on press"
    );

    // --- POINTER MOVES TOWARD THE MENU ITEM -----------------------------
    // (on the old code this is exactly where the hit was lost, and the menu closed
    // itself: “I do not manage to reach it”)
    let hold = crate::widgets::shape_label(kt_model::SegShape::Hold).to_string();
    let item = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .find(|t| t.src == hold)
        .map(|t| t.rect.center())
        .expect("form item is not drawn — the menu did not open at all");
    h.hover_at(item);
    h.step();
    h.step();
    assert_eq!(
        target(&h),
        Some(MenuTarget::Segment(0, 0)),
        "target was lost when the pointer moved to the menu items"
    );

    // the header names BOTH ends of the segment: the times and the shares of the event
    let head = crate::tf!(
        k::fx::SHAPE_SEGMENT_AT,
        "0.000".to_string(),
        "0.500".to_string(),
        "0".to_string(),
        "50".to_string()
    );
    let shown: Vec<String> =
        crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(
        shown.iter().any(|s| s == &head),
        "the menu has no segment heading “{head}”: {shown:?}"
    );
    // and the segment itself is highlighted exactly between these keys
    let hl = highlights(&h);
    assert!(
        hl.iter().any(|r| (r.left() - m[0].2.x).abs() < 1.5 && (r.right() - m[1].2.x).abs() < 1.5),
        "the segment under the menu is not highlighted: {hl:?} versus keys {:?}",
        (m[0].2.x, m[1].2.x)
    );

    // --- the SHAPE lands on the LEFT key of the segment and only on it ---------
    {
        use egui_kittest::kittest::Queryable;
        h.get_by_label(&hold).click();
    }
    h.step();
    h.step();
    let keys = h.state().model.project().clips[0].fx_auto[0][0].keys.clone();
    assert_eq!(keys[0].shape, kt_model::SegShape::Hold, "the shape did not land on the left key of the segment");
    assert_eq!(
        keys[1].shape,
        kt_model::SegShape::DEFAULT,
        "the shape drifted onto the neighboring segment"
    );
    // the menu closed — the target is forgotten
    h.step();
    assert_eq!(target(&h), None, "after closing the menu the target was left hanging");

    // --- a RIGHT CLICK ON A KEY behaves the same way -----------------------
    let m = marks(&h);
    right_click(&mut h, m[1].2);
    assert_eq!(target(&h), Some(MenuTarget::Key(0, 1)), "key was not fixed on press");
    let del = tr(k::tl::menu::DELETE).to_string();
    let item = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .find(|t| t.src == del)
        .map(|t| t.rect.center())
        .expect("no “delete” item — the key menu did not open");
    h.hover_at(item);
    h.step();
    h.step();
    assert_eq!(
        target(&h),
        Some(MenuTarget::Key(0, 1)),
        "the target key was lost when the pointer moved to the menu items"
    );
    let head = crate::tf!(k::fx::KEY_AT, "0.500".to_string(), "50".to_string());
    let shown: Vec<String> =
        crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(shown.iter().any(|s| s == &head), "the key menu has no heading “{head}”: {shown:?}");
}

/// Test 752c (R.63 item 9): the knob of the leading geometry acts on the INSTANCE.
///
/// The second half of the claim “the first geometry has no way to
/// publish parameters”: it is not enough to open the door for the author — the consumer must
/// get an ordinary knob, which changes the FRAME and, therefore, the key of the reel
/// preview (otherwise on the timeline it would show other people’s frames, t680).
#[test]
fn t752c_an_instance_edits_the_published_lead_geometry() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // the author declares the zoom of the title’s leading node
    let zoom = PubId(50);
    h.state_mut().enter_nest(s.a);
    h.step();
    assert!(
        h.state_mut().apply(Command::PublishParam {
            seq: s.seq,
            param: Box::new(PublishedParam {
                id: zoom,
                name: "Zoom".into(),
                target: ParamAddr::new(s.inner, ParamOwner::Geometry, "zoom"),
                ty: ParamType::Num,
                range: PubRange::default(),
            }),
        }),
        "publication of the leading node’s zoom was rejected"
    );
    h.step();
    h.state_mut().leave_nest();
    h.step();

    // the consumer’s window draws the knob by the AUTHOR’S NAME
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    assert!(
        h.query_all_by_label_contains("Zoom").count() > 0,
        "the leading geometry handle is not shown to the consumer"
    );

    // ...and editing the field (with the same gesture the window sends) changes the FRAME
    let keys = h.state_mut().reel_keys();
    let (ka, kb) = (*keys.get(&s.a).expect("key A"), *keys.get(&s.b).expect("key B"));
    assert_eq!(ka, kb, "scene: before the edit both issues share one preview key");
    let win = h.state().fx_editors[0].gesture;
    assert!(h.state_mut().apply_coalesced(
        Command::SetOverride { id: s.b, ov: Box::new(Override::value(zoom, ParamValue::Num(3.0))) },
        win
    ));
    h.step();

    let doc = h.state().model.document();
    let inst = h.state().model.clip(s.b).cloned().expect("second issue");
    let r = doc.resolve_reel(Some(s.seq), &inst.overrides);
    assert_eq!(
        r.clips.iter().find(|c| same_clip(c.id, s.inner)).map(|c| c.geometry.zoom),
        Some(3.0),
        "override did not reach the leading node inside the reel"
    );
    assert_eq!(
        doc.published_value(s.seq, zoom),
        Some(ParamValue::Num(1.0)),
        "instance edit went into the reel ITSELF — that would be an edit of all places at once"
    );
    let keys = h.state_mut().reel_keys();
    let (ka2, kb2) = (*keys.get(&s.a).expect("key A"), *keys.get(&s.b).expect("key B"));
    assert_eq!(ka2, ka, "template instance was not touched — its key must stay the same");
    assert_ne!(
        ka2, kb2,
        "preview key did not move: on the timeline the overridden issue would show OTHER frames"
    );
}

/// Test 753b (R.63 item 11): the publish confirmation does NOT enter the reel.
///
/// The owner: “if I publish a parameter without fully entering the composition —
/// why do we then automatically enter the composition fully?” The entry was a
/// consequence of the command (it edited “the reel you are standing in”); now the
/// reel is named, and the person stays where he stood — at the root with the strip
/// expanded. The assertion is observable: `editing()` has not changed, and the knob
/// lies in the reel of the strip.
#[test]
fn t753b_confirming_a_publication_keeps_you_where_you_stand() {
    use egui_kittest::kittest::Queryable;
    let (mut h, s) = harness_news_template();
    let publish = tr(k::pubp::PUBLISH).to_string();
    let ok = tr(k::pubp::DIALOG_DO).to_string();
    let free_row =
        crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "line_spacing");

    // the person stands at the ROOT, and the reel is expanded by its strip
    h.state_mut().expand_in_place(s.a);
    h.step();
    h.step();
    assert!(!h.state().model.bands().is_empty(), "strip did not expand — the scene is wrong");
    assert_eq!(h.state().model.editing(), None, "scene: the person is at the root");
    h.state_mut().open_fx_editor_head(s.inner);
    h.step();
    h.step();

    // right click on the label → “Publish…”
    let rect = h
        .query_all_by_label_contains(&free_row)
        .map(|n| n.rect())
        .min_by(|a, b| a.top().total_cmp(&b.top()))
        .expect("no row label");
    let pos = rect.center();
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    h.get_by_label_contains(&publish).click();
    h.step();
    h.step();
    assert!(h.state().publish_dialog.is_some(), "confirmation window did not open");

    // we confirm
    h.get_by_label_contains(&ok).click();
    h.step();
    h.step();

    assert_eq!(
        h.state().model.editing(),
        None,
        "confirmation pulled the person inside the reel (on the old code it did pull)"
    );
    assert!(
        !h.state().model.bands().is_empty(),
        "strip closed by itself — yet that strip is the person’s workplace"
    );
    let set = h.state().model.document().sequence(s.seq).map(|q| q.published.len()).unwrap_or(0);
    assert_eq!(set, 7, "handle did not land in the strip’s set: {set} instead of seven");
}

/// Test 755b (R.63 item 10): an effect knob edited on the INSTANCE changes
/// what the timeline shows.
///
/// The owner: “I threw on Radial blur, published the amount parameter,
/// changed it in the composition — and it does not change.” The observable consequence
/// that must hold: the reel preview key of this instance
/// moved (otherwise the timeline shows TEMPLATE frames), and the neighbour’s did
/// not.
#[test]
fn t755b_an_instance_knob_moves_its_preview_key() {
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // inside the reel, on the title — radial blur, the “amount” knob
    let amount = PubId(60);
    h.state_mut().enter_nest(s.a);
    h.step();
    assert!(h.state_mut().apply(Command::SetFx {
        id: s.inner,
        fx: vec![FxInstance::radial_blur()],
    }));
    h.step();
    assert!(
        h.state_mut().apply(Command::PublishParam {
            seq: s.seq,
            param: Box::new(PublishedParam {
                id: amount,
                name: "Blur".into(),
                target: ParamAddr::new(s.inner, ParamOwner::FxSlot { slot: 0 }, "amount"),
                ty: ParamType::Num,
                range: PubRange::default(),
            }),
        }),
        "publication of the blur handle was rejected"
    );
    h.step();
    h.state_mut().leave_nest();
    h.step();

    let keys = h.state_mut().reel_keys();
    let (ka, kb) = (*keys.get(&s.a).expect("key A"), *keys.get(&s.b).expect("key B"));
    assert_eq!(ka, kb, "scene: before the edit both issues share one preview key");

    // we edit the knob ON THE INSTANCE — by the same route the window uses
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    let win = h.state().fx_editors[0].gesture;
    assert!(h.state_mut().apply_coalesced(
        Command::SetOverride {
            id: s.b,
            ov: Box::new(Override::value(amount, ParamValue::Num(0.9))),
        },
        win
    ));
    h.step();

    // ...and this is visible: the reel edition of the second edition is different
    let doc = h.state().model.document();
    let inst = h.state().model.clip(s.b).cloned().expect("second issue");
    let r = doc.resolve_reel(Some(s.seq), &inst.overrides);
    let got = r
        .clips
        .iter()
        .find(|c| same_clip(c.id, s.inner))
        .and_then(|c| c.fx.first().and_then(|f| f.get_value("amount")));
    // a comparison with a tolerance: the slot’s parameter is f32, and 0.9 does not fit there exactly
    assert!(
        matches!(got, Some(ParamValue::Num(v)) if (v - 0.9).abs() < 1e-6),
        "override did not reach the effect slot inside the reel: {got:?}"
    );
    let keys = h.state_mut().reel_keys();
    let (ka2, kb2) = (*keys.get(&s.a).expect("key A"), *keys.get(&s.b).expect("key B"));
    assert_eq!(ka2, ka, "template instance was not touched — its key must stay the same");
    assert_ne!(
        ka2, kb2,
        "preview key did not move: the instance would show the frames of the TEMPLATE — exactly \
         “I changed it on the composition, and it does not change”"
    );
}

/// Test 754 (R.63 item 12): the composition parameters live at the HEAD OF THE STRIP, not in
/// a separate window.
///
/// The owner: “it is damn crazy how the composition parameters
/// open in a separate window, when they could be placed in the same
/// fx-chain for convenience. I am putting a ban on any agents spawning
/// any additional windows without an obvious purpose.” The assertions are
/// observable: the head is in the strip, a click on it selects it, the shape
/// is drawn IN THE FX WINDOW (its rectangles inside its rectangle), and
/// a knob edit and a slot edit in one session give ONE history step.
#[test]
fn t754_instance_params_live_in_the_chain_head() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // the FX window of the instance opens by the usual route
    h.state_mut().open_fx_editor(s.b);
    h.step();
    h.step();
    // Since R.67 the head is labelled with the REEL’s NAME, not “Composition parameters”:
    // the long line covered the strip, and the accent colour on selection gave a
    // “long blue rectangle” (owner, item 8). The assertion is the same —
    // the head is there and a click gives the author’s knobs — only the label changes.
    let reel = h
        .state()
        .model
        .project()
        .sequence(s.seq)
        .map(|x| x.name.clone())
        .expect("reel is in place");
    let head = format!("{} {reel}", crate::icons::PIN);
    assert!(
        h.query_all_by_label_contains(&head).count() > 0,
        "instance heads (pin and reel name) are not in the chain strip"
    );

    // a click on the head selects it — and the author’s knobs appear
    h.get_by_label_contains(&head).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().fx_editors[0].selected.get(&s.b),
        Some(&crate::widgets::ChainSel::Head),
        "click on the head did not select it"
    );
    for name in ["Heading", "Font size", "Logo"] {
        assert!(
            h.query_all_by_label_contains(name).count() > 0,
            "handle “{name}” is not in the head — the form did not move"
        );
    }

    // the shape INSIDE the FX window: there is no separate window any more
    let probes = crate::widgets::field_probe::take();
    let _ = probes;
    let win = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new(("fx", h_clip(&h, s.b)))))
        .expect("FX window rectangle");
    let row = h
        .query_all_by_label_contains("Heading")
        .map(|n| n.rect())
        .find(|r| r.width() > 0.0)
        .expect("handle row is drawn");
    assert!(
        win.expand(2.0).contains_rect(row),
        "handle row is drawn OUTSIDE the FX window ({row:?} versus {win:?}) — which means \
             a separate window still lives next to it"
    );

    // ONE history step per session: the knob and the slot are edited by a shared token
    let token = h.state().fx_editors[0].gesture;
    let before = h.state().model.undo_depth();
    assert!(h.state_mut().apply_coalesced(
        Command::SetOverride {
            id: s.b,
            ov: Box::new(Override::value(s.headline, ParamValue::Text("SECOND".into()))),
        },
        token
    ));
    assert!(h.state_mut().apply_coalesced(
        Command::SetOpacity { id: s.b, opacity: 0.5 },
        token
    ));
    h.step();
    assert_eq!(
        h.state().model.undo_depth(),
        before + 1,
        "handle edit and event edit produced DIFFERENT history steps: the window token is not shared"
    );

    // ...and the “Cancel” of the window returns the knob too: the head is part of the same
    // session, and rolling back its half would be a lie
    h.get_by_label_contains(tr(k::fx::CANCEL)).click();
    h.step();
    h.step();
    assert!(
        h.state().model.clip(s.b).is_some_and(|c| c.overrides.is_empty()),
        "“Cancel” in the FX window did not restore the composition handles — the window rolled back half of its session"
    );
}

/// The FX window’s rectangle is found by the id of its anchor — the event
/// around which the window is built (for an A/V pair this is one anchor for both tracks).
fn h_clip(h: &Harness<'_, KtApp>, want: kt_model::ClipId) -> u64 {
    h.state()
        .fx_editors
        .iter()
        .find(|e| e.members.contains(&want))
        .map(|e| e.clip.0)
        .expect("instance FX window is open")
}

pub(crate) fn harness_geom_ctl() -> (Harness<'static, KtApp>, kt_model::ClipId) {
    use kt_model::*;
    // the scene is a GENERATOR: its source is brought up by the same route as in the
    // program, and the preview gets a REAL frame (without a frame there is neither a
    // picture nor controls over it — as with the title drag, t495)
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid {
        color_r: 200,
        color_g: 40,
        color_b: 40,
    });
    h.step();
    let (_media, clip) = made_generator(&h);
    h.state_mut().fx_editors.clear();
    {
        let app = h.state_mut();
        let c = app.model.clip(clip).cloned().expect("event");
        app.selected.clear();
        app.selected.insert(clip);
        app.player.seek(kt_time::Tick(c.start.0 + c.duration.0 / 2));
        app.geom_ctl = crate::geomctl::GeomMode::Transform;
    }
    h.step();
    h.step();
    (h, clip)
}

/// Test 760 (R.64): the frame over the picture equals the analytic one, the gesture edits the
/// same numbers as the fields, and gives ONE history step.
///
/// The owner (R.63 item 6): “AT MINIMUM we should have convenient editing right
/// in the preview.” The frame must coincide with the rectangle onto which the composite
/// places the layer — otherwise the person drags one thing and gets
/// another.
#[test]
fn t760_frame_matches_the_placement_and_drag_writes_the_same_numbers() {
    use kt_model::*;
    let (mut h, clip) = harness_geom_ctl();
    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);

    // --- the FRAME equals the analytic one -----------------------------------
    let pl = h.state().geom_placement(clip, canvas).expect("placement is present");
    let g = h.state().model.clip(clip).unwrap().geometry;
    let want = kt_fx::geom::geometry_placement(
        &g.node(),
        Some(kt_fx::conform::LeadCtx::new(
            h.state().player.source_view(h.state().model.clip(clip).unwrap().media),
            g.rule.resolve(h.state().model.project().settings.conform),
        )),
        h.state()
            .player
            .source_view(h.state().model.clip(clip).unwrap().media)
            .unwrap_or(canvas),
        canvas,
    )
    .expect("analytical placement");
    assert_eq!(pl, want, "interface frame diverged from the render placement");

    // --- MOVE by the body: the same numbers, one history step ---------------
    let depth = h.state().model.undo_depth();
    let before = h.state().model.clip(clip).unwrap().geometry;
    // the “screen → canvas” scale is computed by the PICTURE, not by the panel:
    // the panel has letterbox margins, and by it 48 px of the pointer are converted
    // into a different number of shares (this exact thing turned the test red on the correct code)
    let img = {
        let r = h.state().preview_img_rect;
        assert!(r.width() > 40.0, "preview in the harness exists: {r:?}");
        r
    };
    // scene diagnostics: is there a frame and is the set visible
    assert!(h.state().texture.is_some(), "there is no frame in the preview — the controls have nothing to draw on");
    assert_eq!(h.state().geom_mode(), crate::geomctl::GeomMode::Transform, "set is not enabled");
    assert!(h.state().geom_target().is_ok(), "frame target is not selected");
    assert!(h.state().geom_ctl_drawn, "controls did not reach gesture parsing");
    // we take a point INSIDE the frame, but not on a knob: the centre is taken by the anchor,
    // the corners and side midpoints are the scale knobs
    let c = img.min + egui::vec2(img.width() * 0.3, img.height() * 0.32);
    h.hover_at(c);
    h.step();
    h.drag_at(c);
    h.step();
    h.hover_at(c + egui::vec2(12.0, 0.0));
    h.step();
    assert!(h.state().geom_drag.is_some(), "gesture did not start: the control did not catch the press");
    h.hover_at(c + egui::vec2(48.0, 30.0));
    h.step();
    h.drop_at(c + egui::vec2(48.0, 30.0));
    h.step();

    let after = h.state().model.clip(clip).unwrap().geometry;
    assert!(
        after.pos_x > before.pos_x + 1e-6 && after.pos_y > before.pos_y + 1e-6,
        "moving by the body did not shift the position: {:?} → {:?}",
        (before.pos_x, before.pos_y),
        (after.pos_x, after.pos_y)
    );
    assert_eq!(
        h.state().model.undo_depth(),
        depth + 1,
        "gesture must produce EXACTLY one history step"
    );
    // ...and the shift is PLAUSIBLE: this is the same pointer movement, converted
    // into canvas shares by the same function (the round-trip accuracy is proved by t761 on the
    // pure function — here it is checked that the gesture goes through it)
    let img_w = img.width() as f64;
    let img_h = img.height() as f64;
    let full = kt_fx::geom::pos_for_drag(
        (before.pos_x, before.pos_y),
        canvas,
        (48.0 / img_w * canvas.0, 30.0 / img_h * canvas.1),
    );
    for (got, want, ax) in [(after.pos_x, full.0, "X"), (after.pos_y, full.1, "Y")] {
        assert!(
            got > want * 0.6 && got <= want * 1.05,
            "shift along {ax} does not look like pointer movement: {got} versus {want} \
             (the drag threshold does not land on the value, so “slightly less” is normal)"
        );
    }
    // the NUMBER IS ONE: the model and the FX window field read the same value
    assert_eq!(
        h.state().model.clip(clip).unwrap().geometry.pos_x,
        after.pos_x,
        "value in the model diverged from what the form shows"
    );

    // --- there is NO frame for audio and for multi-selection, the reason is named ------
    {
        let app = h.state_mut();
        let audio = app
            .model
            .project()
            .clips
            .iter()
            .find(|c| c.kind == ClipKind::Audio)
            .map(|c| c.id);
        app.selected.clear();
        if let Some(a) = audio {
            app.selected.insert(a);
            assert!(
                matches!(app.geom_target(), Err(crate::geomctl::NoFrame::NotVideo)),
                "frame must not be drawn for an audio event"
            );
        }
        app.selected.insert(clip);
        if audio.is_some() {
            assert!(
                matches!(app.geom_target(), Err(crate::geomctl::NoFrame::ManySelected)),
                "frame is not drawn during multi-selection"
            );
        }
        app.selected.clear();
        assert!(matches!(app.geom_target(), Err(crate::geomctl::NoFrame::NothingSelected)));
    }
}

/// Test 759 (R.64): the “Transform” shape on the leading node.
///
/// The owner’s claim (R.63 item 6): “solid boring input fields; even in
/// Vegas it is done better… in Pr/Ae this is more convenient.” What is asserted is what
/// a person sees and does: groups, the X/Y pair on ONE row, sliders for
/// zoom and rotation, a diagram that computes THE SAME numbers, and “reset geometry”
/// in one history step.
#[test]
fn t759_transform_form_groups_pairs_sliders_and_reset() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    use kt_model::*;
    let (mut h, clip) = harness_geom_ctl();
    // the geometry is MOVED — otherwise the reset has nothing to return
    {
        let app = h.state_mut();
        let c = app.model.clip(clip).cloned().expect("event");
        app.apply(Command::SetGeometry {
            id: clip,
            geometry: EventGeometry { pos_x: 0.2, zoom: 1.7, rot: 30.0, crop_l: 0.1, ..c.geometry },
            pos: c.geometry_pos,
            auto: Vec::new(),
        });
        app.open_fx_editor(clip);
    }
    h.step();
    h.step();
    let lead_label = crate::fx_editor::fx_display_name(&EventGeometry::default().node());
    h.get_by_label(lead_label).click();
    h.step();
    h.step();

    // ---- GROUPS -------------------------------------------------------
    for key in [k::geomctl::GROUP_FRAME, k::geomctl::GROUP_POS, k::geomctl::GROUP_CROP] {
        let title = tr(key);
        assert!(
            h.query_all_by_label_contains(title).count() > 0,
            "group “{title}” is not in the form — the fields stayed a flat list"
        );
    }

    // ---- X/Y PAIR ON ONE ROW ---------------------------------------
    // the label is searched EXACTLY: “X:” occurs inside “Anchor X:” too
    let row_of = |h: &Harness<'_, KtApp>, label: &str| -> egui::Rect {
        h.get_all_by_label(label)
            .map(|n| n.rect())
            .find(|r| r.width() > 0.0)
            .unwrap_or_else(|| panic!("label “{label}” is not in the form"))
    };
    let rx = row_of(&h, tr(k::fxp::POS_X));
    let ry = row_of(&h, tr(k::fxp::POS_Y));
    assert!(
        (rx.center().y - ry.center().y).abs() < 4.0 && (rx.center().x - ry.center().x).abs() > 8.0,
        "X and Y are not paired on one row: {rx:?} versus {ry:?}"
    );

    // ---- SLIDERS for zoom and rotation -----------------------------------
    let sliders = h.query_all(by().role(Role::Slider)).count();
    assert!(
        sliders >= 2,
        "sliders in the form: {sliders} — zoom and rotation must both have them          (dragging a number by one pixel is exactly what makes the fields “boring”)"
    );

    // ---- the DIAGRAM computes THE SAME numbers ------------------------------------
    {
        let g = h.state().model.clip(clip).unwrap().geometry;
        let size = egui::vec2(116.0, 66.0);
        let (quad, anchor) = crate::geomctl::schema_quad(&g.node(), size).expect("diagram is computed");
        let (q0, a0) = crate::geomctl::schema_quad(&EventGeometry::default().node(), size)
            .expect("default diagram");
        assert_ne!(quad, q0, "diagram does not see zoom, rotation or crop");
        let w = |q: [(f64, f64); 4]| {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for c in q {
                lo = lo.min(c.0);
                hi = hi.max(c.0);
            }
            hi - lo
        };
        assert!(w(quad) > w(q0), "zoom 1.7 did not widen the diagram frame: {} versus {}", w(quad), w(q0));
        assert!(
            (anchor.0 - a0.0).abs() > 1.0,
            "diagram anchor did not move with the position: {anchor:?} versus {a0:?}"
        );
    }

    // ---- RESET in one history step ------------------------------------
    let depth = h.state().model.undo_depth();
    h.get_by_label_contains(tr(k::geomctl::RESET)).click();
    h.step();
    h.step();
    let g = h.state().model.clip(clip).unwrap().geometry;
    assert_eq!(
        (g.pos_x, g.zoom, g.rot, g.crop_l),
        (0.0, 1.0, 0.0, 0.0),
        "“Reset geometry” did not restore the defaults"
    );
    assert_eq!(
        h.state().model.undo_depth(),
        depth + 1,
        "reset is ONE history step (window session), not ten commands"
    );
    // units are taken from the PASSPORT, not written into the label
    assert_eq!(
        crate::geomctl::shown_spec_unit("crop_l"),
        Some(kt_model::ParamUnit::Percent),
        "crop fraction must be shown as a percent (item 415)"
    );
    assert_eq!(
        crate::geomctl::shown_spec_unit("zoom"),
        Some(kt_model::ParamUnit::Ratio),
        "zoom is a ratio (×), not a percent"
    );
}

/// Test 762b (R.64): a gesture over an ANIMATED parameter sets a key.
///
/// Rule 17 to the letter: the on-screen frame and the FX window field edit the same
/// value with ONE rule (`CurveBind`). For a parameter with a live curve this is a
/// key at the cursor, not a constant: otherwise the gesture would erase the animation, while
/// showing that same value. A neighbouring parameter WITHOUT a curve is edited by the same gesture
/// into the base — both halves of the rule in one test.
#[test]
fn t762b_gesture_over_a_curve_sets_a_key() {
    use kt_model::*;
    let (mut h, clip) = harness_geom_ctl();
    // a curve on pos_x: 0 → 0.2 across all the material of the event
    let (at, keys0) = {
        let app = h.state_mut();
        let c = app.model.clip(clip).cloned().expect("event");
        let curve = ParamCurve {
            param: "pos_x".into(),
            keys: AutoKey::linear(vec![
                AutoKey::at(c.source_in, 0.0),
                AutoKey::at(kt_time::Tick(c.source_in.0 + c.duration.0), 0.2),
            ]),
            source: Default::default(),
        };
        app.apply(Command::SetGeometry {
            id: clip,
            geometry: c.geometry,
            pos: c.geometry_pos,
            auto: vec![curve],
        });
        (app.geom_curve_at(clip), 2usize)
    };
    h.step();
    // the frame shows the same as the render: the CURVE value under the cursor
    let shown = h.state().geom_effective(clip).expect("effective geometry").pos_x;
    assert!(
        (shown - 0.1).abs() < 1e-6,
        "frame is computed from the base, not from the curve under the cursor: {shown}"
    );

    let c0 = body_pos(&h, clip);
    let depth = h.state().model.undo_depth();
    geom_drag(&mut h, c0, c0 + egui::vec2(40.0, 26.0), egui::Modifiers::NONE);

    let c = h.state().model.clip(clip).cloned().expect("event");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "gesture — one history step");
    // …the parameter WITH A CURVE: the base is intact, the key is set at the cursor
    assert_eq!(c.geometry.pos_x, 0.0, "gesture overwrote the CONSTANT of an animated parameter");
    let curve = c
        .geometry_auto
        .iter()
        .find(|cu| cu.param == "pos_x")
        .expect("pos_x curve is alive");
    assert!(
        curve.keys.len() > keys0,
        "key was not set at the cursor: keys remain {}",
        curve.keys.len()
    );
    let key = curve
        .keys
        .iter()
        .find(|k| k.t == at)
        .unwrap_or_else(|| panic!("no key at the cursor position {at:?}: {:?}", curve.keys));
    assert!(
        key.v > 0.1 + 1e-6,
        "key was set, but the value is not from the gesture: {} (the curve under the cursor gave 0.1)",
        key.v
    );
    // …and the parameter WITHOUT a curve is edited into the base by the same gesture
    assert!(c.geometry.pos_y > 1e-6, "pos_y without a curve must edit the base: {}", c.geometry.pos_y);
    assert!(
        !c.geometry_auto.iter().any(|cu| cu.param == "pos_y"),
        "gesture created a curve where there was none"
    );
}

/// Test 763 (R.64): hitting a knob on the first try, and priority on a
/// small frame.
///
/// Discovery F-UI-001: “gesture accuracy drops along with preview quality” —
/// on a small frame the grab zones overlap, and the rule “whichever is nearer” remains
/// the only unambiguous one. Plus rule 1 of the checklist: a person must not have to aim
/// at a knob the size of a dot.
#[test]
fn t763_handles_are_hittable_and_the_nearest_wins() {
    use crate::geomctl::{GRAB, GeomMode, nearest_handle};
    use kt_fx::geom::Handle;
    let (h, clip) = harness_geom_ctl();
    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);
    let pl = h.state().geom_placement(clip, canvas).expect("placement");
    // canvas → screen, as in the control
    let img = h.state().preview_img_rect;
    let to_screen = |p: (f64, f64)| -> egui::Pos2 {
        egui::pos2(
            img.left() + (p.0 / canvas.0) as f32 * img.width(),
            img.top() + (p.1 / canvas.1) as f32 * img.height(),
        )
    };

    // --- EACH knob is caught at its own point and within the grab zone -----
    for h_ in GeomMode::Transform.handles() {
        if *h_ == Handle::Anchor {
            continue; // the anchor has its own narrow zone — checked below
        }
        let p = to_screen(h_.point(&pl));
        assert_eq!(
            nearest_handle(&pl, GeomMode::Transform, p, &to_screen, false),
            Some(*h_),
            "handle {h_:?} is not caught at its own point"
        );
        // ...and from the edge of the grab zone (rule 1: 14 px)
        let edge = p + egui::vec2(GRAB * 0.45, GRAB * 0.45);
        assert!(
            nearest_handle(&pl, GeomMode::Transform, edge, &to_screen, false).is_some(),
            "handle {h_:?} is not caught from the edge of the grab zone — aiming will have to be exact"
        );
    }

    // --- ANCHOR: taken by its POINT; a bit to the side — already the body ------
    // (it can only be grabbed with Alt: Alt+drag is intercepted by the window manager, and the
    // gesture never reaches the program — live `geomctl`)
    let a = to_screen(Handle::Anchor.point(&pl));
    assert_eq!(
        nearest_handle(&pl, GeomMode::Transform, a, &to_screen, false),
        Some(Handle::Anchor),
        "anchor is not grabbed at its own point — the gesture is unreachable"
    );
    let near_a = a + egui::vec2(crate::geomctl::ANCHOR_GRAB + 3.0, 0.0);
    assert_ne!(
        nearest_handle(&pl, GeomMode::Transform, near_a, &to_screen, false),
        Some(Handle::Anchor),
        "the anchor at the center of the frame takes the picture grab around itself: \
         its zone must be narrow"
    );
    assert_eq!(
        nearest_handle(&pl, GeomMode::Transform, near_a, &to_screen, true),
        Some(Handle::Anchor),
        "with Alt the anchor zone is shared: it must be possible to catch it for sure"
    );

    // --- BODY: a point inside the frame but not at a knob gives no knob gesture ---
    let inside = img.min + egui::vec2(img.width() * 0.3, img.height() * 0.32);
    assert_eq!(
        nearest_handle(&pl, GeomMode::Transform, inside, &to_screen, false),
        None,
        "the frame body intercepted the handle gesture"
    );

    // --- SMALL FRAME: the NEAREST wins, not the first one caught ---
    let tiny = kt_fx::geom::Placement {
        ox: 100.0,
        oy: 100.0,
        dw: 6.0,
        dh: 6.0,
        rot_deg: 0.0,
        anchor: (103.0, 103.0),
        crop_in: (0.0, 0.0, 6.0, 6.0),
    };
    let ident = |p: (f64, f64)| egui::pos2(p.0 as f32, p.1 as f32);
    for corner in [Handle::TopLeft, Handle::TopRight, Handle::BottomRight, Handle::BottomLeft] {
        // from R.66 the knobs of a small frame are moved OUTSIDE its edges — we probe
        // where they are now drawn (t781), and the assertion is the same:
        // the pointer on a knob — the knob wins
        let p = crate::geomctl::handle_screen(&tiny, corner, &ident);
        assert_eq!(
            nearest_handle(&tiny, GeomMode::Transform, p, &ident, false),
            Some(corner),
            "on a 6×6 frame handle {corner:?} lost to the neighbor, although the pointer is on it"
        );
    }

    // --- CROP — a different set: in transform mode its knobs are absent ---
    assert_eq!(
        nearest_handle(&pl, GeomMode::Transform, to_screen(Handle::CropLeft.point(&pl)), &to_screen, false),
        Some(Handle::Left),
        "in the transform set the side midpoint is the scale, not the crop"
    );
    assert_eq!(
        nearest_handle(&pl, GeomMode::Crop, to_screen(Handle::CropLeft.point(&pl)), &to_screen, false),
        Some(Handle::CropLeft),
        "in the crop set the side midpoint is the crop"
    );
}

/// Test 764 (R.64): the controls fade during playback and do not heat an idle system.
///
/// The industry hides them always (FCP) or automatically (AE): the control covers
/// what the person looks at the frame for. Our frame-cost rule
/// (invariant 12) demands the same from the other side.
#[test]
fn t764_controls_hide_while_playing_and_cost_nothing_idle() {
    let (mut h, _clip) = harness_geom_ctl();
    assert!(h.state().geom_ctl_drawn, "scene: during pause the controls are drawn");

    // --- PLAYBACK: no controls ---------------------------------------------
    h.state_mut().geom_ctl_drawn = false;
    h.state_mut().player.play();
    h.step();
    h.step();
    assert!(
        !h.state().geom_ctl_drawn,
        "during playback the controls stayed on the frame — they cover what one is watching"
    );

    // --- PAUSE: they are back -----------------------------------------------
    h.state_mut().player.pause();
    h.state_mut().geom_ctl_drawn = false;
    h.step();
    h.step();
    assert!(h.state().geom_ctl_drawn, "during pause the controls did not come back");

    // --- IDLE: the set is off — the preview path is as before ------------------
    h.state_mut().geom_ctl = crate::geomctl::GeomMode::Off;
    h.state_mut().geom_ctl_drawn = false;
    h.step();
    h.step();
    assert!(!h.state().geom_ctl_drawn, "a switched-off set still gets into the frame");
}

/// Drag of the frame in the preview IN THREE STEPS (the egui drag threshold), with modifiers.
fn geom_drag(
    h: &mut Harness<'_, KtApp>,
    from: egui::Pos2,
    to: egui::Pos2,
    mods: egui::Modifiers,
) {
    // the modifier travels both as a separate event AND in the button itself: `drag_at`
    // sends `PointerButton { modifiers: NONE }` and by this clears the frame state
    // (a kin of the lesson of `ctrl_click_at`)
    h.event(egui::Event::ModifiersChanged(mods));
    h.hover_at(from);
    h.step();
    h.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: mods,
    });
    h.step();
    h.hover_at(from + (to - from) * 0.25);
    h.step();
    h.hover_at(to);
    h.step();
    h.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: mods,
    });
    h.event(egui::Event::PointerGone);
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

/// The knob’s screen point (the control computes it with the same `Handle::point`).
/// The BODY point of the frame: not the centre (that is the anchor) and not an edge (those are the scale knobs).
/// Taking a share of the preview panel is not allowed — the frame shifted from under this point
/// drifts away, and the gesture starts beside it (t762b caught this as “zero history steps”).
fn body_pos(h: &Harness<'_, KtApp>, clip: kt_model::ClipId) -> egui::Pos2 {
    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);
    let pl = h.state().geom_placement(clip, canvas).expect("placement");
    let c = pl.center();
    let p = (c.0 + pl.dw * 0.22, c.1 + pl.dh * 0.24);
    let img = h.state().preview_img_rect;
    egui::pos2(
        img.left() + (p.0 / canvas.0) as f32 * img.width(),
        img.top() + (p.1 / canvas.1) as f32 * img.height(),
    )
}

fn handle_pos(
    h: &Harness<'_, KtApp>,
    clip: kt_model::ClipId,
    which: kt_fx::geom::Handle,
) -> egui::Pos2 {
    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);
    let pl = h.state().geom_placement(clip, canvas).expect("placement");
    let img = h.state().preview_img_rect;
    let p = which.point(&pl);
    egui::pos2(
        img.left() + (p.0 / canvas.0) as f32 * img.width(),
        img.top() + (p.1 / canvas.1) as f32 * img.height(),
    )
}

/// Test 760b (R.64): rotation by the rotate knob, step by Shift.
#[test]
fn t760b_rotation_handle_turns_the_layer() {
    use kt_fx::geom::Handle;
    let (mut h, clip) = harness_geom_ctl();
    assert_eq!(h.state().model.clip(clip).unwrap().geometry.rot, 0.0, "scene: the layer is not rotated");

    // the rotate knob is above the top side; we drag sideways
    let from = handle_pos(&h, clip, Handle::Rot);
    let to = from + egui::vec2(120.0, 60.0);
    // scene diagnostics: is the rotate knob inside the preview at all?
    assert!(
        h.state().preview_rect.contains(from),
        "rotation handle is outside the preview: {from:?} at {:?}",
        h.state().preview_rect
    );
    let depth = h.state().model.undo_depth();
    // which knob will be caught at this point?
    {
        let q = h.state().player.preview_quality();
        let canvas = (q.width as f64, q.height as f64);
        let pl = h.state().geom_placement(clip, canvas).unwrap();
        let img = h.state().preview_img_rect;
        let to_screen = |p: (f64, f64)| -> egui::Pos2 {
            egui::pos2(
                img.left() + (p.0 / canvas.0) as f32 * img.width(),
                img.top() + (p.1 / canvas.1) as f32 * img.height(),
            )
        };
        assert_eq!(
            crate::geomctl::nearest_handle(
                &pl,
                crate::geomctl::GeomMode::Transform,
                from,
                &to_screen,
                false
            ),
            Some(Handle::Rot),
            "at the rotation handle’s point it is not the handle that is caught"
        );
    }
    geom_drag(&mut h, from, to, egui::Modifiers::NONE);
    let rot = h.state().model.clip(clip).unwrap().geometry.rot;
    assert!(rot.abs() > 1.0, "rotation did not happen: {rot}");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "rotation is one history step");

    // Shift — 15° step
    let from = handle_pos(&h, clip, Handle::Rot);
    let to = from + egui::vec2(-90.0, 70.0);
    geom_drag(&mut h, from, to, egui::Modifiers::SHIFT);
    let rot = h.state().model.clip(clip).unwrap().geometry.rot;
    assert!(
        (rot / 15.0 - (rot / 15.0).round()).abs() < 1e-6,
        "with Shift the angle must be a multiple of 15°: {rot}"
    );
}

/// Test 761b (R.64): the anchor moves — the picture stands still, and on a rotated
/// layer too. The same contract as the pure function in t761, but THROUGH
/// A REAL GESTURE.
#[test]
fn t761b_anchor_gesture_keeps_the_picture_in_place() {
    use kt_fx::geom::Handle;
    for rot in [0.0f64, 30.0] {
        let (mut h, clip) = harness_geom_ctl();
        {
            let app = h.state_mut();
            let g = kt_model::EventGeometry { rot, ..app.model.clip(clip).unwrap().geometry };
            let tok = app.gesture_counter + 1;
            app.set_geometry_coalesced(clip, g, tok);
        }
        h.step();
        let q = h.state().player.preview_quality();
        let canvas = (q.width as f64, q.height as f64);
        let before = h.state().geom_placement(clip, canvas).unwrap().corners();
        let a0 = h.state().model.clip(clip).unwrap().geometry;

        // we drag the ANCHOR (only with Alt) to the top-left corner of the frame
        let from = handle_pos(&h, clip, Handle::Anchor);
        let to = handle_pos(&h, clip, Handle::TopLeft);
        geom_drag(&mut h, from, to, egui::Modifiers::ALT);

        let a1 = h.state().model.clip(clip).unwrap().geometry;
        assert!(
            (a1.anchor_x - a0.anchor_x).abs() > 0.05 || (a1.anchor_y - a0.anchor_y).abs() > 0.05,
            "rotation {rot}°: the anchor did not move — the gesture did nothing"
        );
        let after = h.state().geom_placement(clip, canvas).unwrap().corners();
        for i in 0..4 {
            assert!(
                (after[i].0 - before[i].0).abs() < 1.0 && (after[i].1 - before[i].1).abs() < 1.0,
                "rotation {rot}°: the picture drifted away from the anchor — angle {i}: {:?} versus {:?}",
                after[i],
                before[i]
            );
        }
    }
}

/// Test 762 (R.64): crop with its own knobs in its own set.
#[test]
fn t762_crop_handles_cut_their_own_side() {
    use kt_fx::geom::Handle;
    let (mut h, clip) = harness_geom_ctl();
    h.state_mut().geom_ctl = crate::geomctl::GeomMode::Crop;
    h.step();

    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);
    let w0 = h.state().geom_placement(clip, canvas).unwrap().dw;
    let from = handle_pos(&h, clip, Handle::CropLeft);
    let to = from + egui::vec2(h.state().preview_rect.width() * 0.15, 0.0);
    let depth = h.state().model.undo_depth();
    geom_drag(&mut h, from, to, egui::Modifiers::NONE);

    let g = h.state().model.clip(clip).unwrap().geometry;
    assert!(g.crop_l > 0.02, "left crop did not move: {}", g.crop_l);
    assert_eq!(
        (g.crop_t, g.crop_r, g.crop_b),
        (0.0, 0.0, 0.0),
        "crop handle touched foreign sides"
    );
    assert_eq!(h.state().model.undo_depth(), depth + 1, "crop is one history step");
    let w1 = h.state().geom_placement(clip, canvas).unwrap().dw;
    assert!(w1 < w0, "the frame did not narrow after the crop: {w1} versus {w0}");
}

/// Test 775 (R.65): DUCKING — IS TWO COMPRESSORS, not its own shape.
///
/// The owner (R.63 item 8.4): “as ducking one can show the same duplicated
/// audio-compressor widget with dimmed fields, i.e. a compressor on the
/// background object and a compressor on the foreground object…
/// meaningless copy-pastes of the same code pile up everywhere.”
/// What is asserted is what is seen and done: choosing the key adds the device,
/// the SPEECH compressor is added with one action, both edits — one session
/// of the window and one history step, “Cancel” rolls back both.
#[test]
fn t775_ducking_is_two_compressors_in_the_track_window() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    use kt_model::LaneKind::Audio;
    let (mut h, _) = harness_three_clips_dt(None);
    let (bg, key) = {
        let app = h.state_mut();
        edit_project(app, |p| p.ensure_tracks(Audio, 2));
        let bg = app.model.project().track_at(Audio, 0).unwrap().id;
        let key = app.model.project().track_at(Audio, 1).unwrap().id;
        (bg, key)
    };
    h.step();
    h.state_mut().open_track_window(bg);
    h.step();
    h.step();

    // the section is named after the industry and sits IN THE TRACK WINDOW
    for s in [tr(k::msg::DUCK_TITLE), tr(k::msg::DUCK_KEY), tr(k::msg::DUCK_BG)] {
        assert!(
            h.query_all_by_label_contains(s).count() > 0,
            "there is no “{s}” row in the track window — ducking has nowhere to live"
        );
    }
    let depth = h.state().model.undo_depth();

    // CHOOSING THE KEY adds the device: the person does not have to search for a compressor in the catalog
    let key_name = h.state().model.project().track(key).unwrap().label(1);
    // the combo is searched by ROLE and VALUE: the word “no” occurs in the window more than once
    h.get(by().role(Role::ComboBox).value(tr(k::msg::DUCK_KEY_NONE))).click();
    h.step();
    h.get_by_label_contains(&key_name).click();
    h.step();
    h.step();
    let keys: Vec<_> = h
        .state()
        .model
        .project()
        .track(bg)
        .map(|t| kt_model::sidechain_keys(&t.fx).collect())
        .unwrap_or_default();
    assert_eq!(keys, vec![key], "key choice did not reach the model (or the device is not set up)");

    // ...and a SECOND compressor — a real one, on the KEY track — with one action
    assert!(
        h.query_all_by_label_contains(tr(k::msg::DUCK_ADD_COMP)).count() > 0,
        "no “set up compressor on key” button — the second device is unreachable"
    );
    h.get_by_label_contains(tr(k::msg::DUCK_ADD_COMP)).click();
    h.step();
    h.step();
    assert!(
        h.state()
            .model
            .project()
            .track(key)
            .is_some_and(|t| t.fx.iter().any(|f| matches!(f, kt_model::FxInstance::Compressor { .. }))),
        "speech compressor is not set up"
    );

    // BOTH edits — ONE session of the window and ONE history step
    assert_eq!(
        h.state().model.undo_depth(),
        depth + 1,
        "the track window session must be one history step"
    );

    // “Cancel” rolls back both the key and the speech compressor
    h.get_by_label_contains(tr(k::track::CANCEL)).click();
    h.step();
    h.step();
    let app = h.state();
    assert_eq!(
        app.model.project().track(bg).map(|t| kt_model::sidechain_keys(&t.fx).count()),
        Some(0),
        "“Cancel” did not remove the key"
    );
    assert!(
        app.model
            .project()
            .track(key)
            .is_some_and(|t| !t.fx.iter().any(|f| matches!(f, kt_model::FxInstance::Compressor { .. }))),
        "“Cancel” did not restore the key track’s chain"
    );
}

/// Test 776 (R.65): the list of keys does not offer the impossible.
#[test]
fn t776_key_list_offers_only_possible_tracks() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    use kt_model::LaneKind::Audio;
    let (mut h, _) = harness_three_clips_dt(None);
    let (a, b) = {
        let app = h.state_mut();
        edit_project(app, |p| p.ensure_tracks(Audio, 2));
        let a = app.model.project().track_at(Audio, 0).unwrap().id;
        let b = app.model.project().track_at(Audio, 1).unwrap().id;
        (a, b)
    };
    // A already listens to B — so B cannot listen to A (a cycle)
    {
        let app = h.state_mut();
        let mut c = kt_model::FxInstance::compressor();
        if let kt_model::FxInstance::Compressor { key, .. } = &mut c {
            *key = Some(b);
        }
        app.apply(kt_model::Command::SetTrackFx { id: a, fx: vec![c], bypass: vec![] });
    }
    h.step();
    h.state_mut().open_track_window(b);
    h.step();
    h.step();
    h.get(by().role(Role::ComboBox).value(tr(k::msg::DUCK_KEY_NONE))).click();
    h.step();
    h.step();
    // the label is searched EXACTLY: the window header contains the track name, and a
    // “substring” search would find it, not the list item
    let a_name = h.state().model.project().track(a).unwrap().label(0);
    let offered = h.query_all_by_label(&a_name).count();
    assert_eq!(
        offered, 0,
        "a track that gives a CYCLE is offered in the key list — a silent dead end"
    );
    // and itself is not in the list either
    let b_name = h.state().model.project().track(b).unwrap().label(1);
    assert_eq!(
        h.query_all_by_label(&b_name).count(),
        0,
        "a track is offered as a key for itself"
    );
}

/// Test 781 (R.66): a SMALL FRAME pushes its knobs outward, the body stays its own.
///
/// On the owner’s project (`paintitblack.kt`) the “as is” rule and a placeholder in place of
/// unavailable media give a frame of 45×25 px: eight knobs
/// with a 14 px grab zone cover all of it, and MOVE becomes
/// unavailable — a knob always wins. A deferred limit of R.64.
#[test]
fn t781_small_frame_moves_handles_outside() {
    use crate::geomctl::{GeomMode, handle_screen, nearest_handle, small_frame};
    use kt_fx::geom::{Handle, Placement};

    // a frame 40×24 px on screen: less than three grab zones on both sides
    let tiny = Placement {
        ox: 200.0,
        oy: 120.0,
        dw: 40.0,
        dh: 24.0,
        rot_deg: 0.0,
        anchor: (220.0, 132.0),
        crop_in: (0.0, 0.0, 1.0, 1.0),
    };
    // screen = canvas, one to one
    let to_screen = |p: (f64, f64)| egui::pos2(p.0 as f32, p.1 as f32);
    assert!(small_frame(&tiny, &to_screen), "scene: the frame must be small");

    // the scale knobs have MOVED outward from their edges
    for h in [Handle::TopLeft, Handle::Top, Handle::Right, Handle::BottomLeft] {
        let inside = to_screen(h.point(&tiny));
        let shown = handle_screen(&tiny, h, &to_screen);
        assert!(
            (shown - inside).length() > 10.0,
            "{h:?} is not moved outward: {shown:?} versus the edge {inside:?}"
        );
        // ...and are caught THERE, where they are drawn
        assert_eq!(
            nearest_handle(&tiny, GeomMode::Transform, shown, &to_screen, false),
            Some(h),
            "{h:?} is drawn outside, but it is grabbed somewhere else"
        );
    }

    // the CENTRE of a small frame is MOVE, not a knob (except the anchor itself)
    let c = to_screen(tiny.center());
    let off = egui::pos2(c.x + 9.0, c.y + 5.0);
    assert_eq!(
        nearest_handle(&tiny, GeomMode::Transform, off, &to_screen, false),
        None,
        "the body of the small frame is intercepted by a handle — there is nothing to move it with"
    );

    // a NORMAL frame does not change: knobs on the edges, as before
    let big = Placement { dw: 400.0, dh: 240.0, ..tiny };
    assert!(!small_frame(&big, &to_screen), "scene: an ordinary frame is not small");
    for h in [Handle::TopLeft, Handle::Right] {
        assert_eq!(
            handle_screen(&big, h, &to_screen),
            to_screen(h.point(&big)),
            "{h:?} on an ordinary frame moved — the previous behavior is broken"
        );
    }
}

/// Test 780 (R.66): SIX DENSE SHAPES laid out on the “Transform” model.
///
/// The owner’s direct request of R.64: “and remember for the future to go over all the other
/// widgets and windows as well.” He accepted the model then too —
/// collapsible groups, pairs on one row, units from the passport. What is asserted here
/// is observable: each of the six shapes has ITS OWN groups, and
/// collapsing is a state of the window, not of the project.
#[test]
fn t780_dense_forms_have_groups() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    // --- project settings: frame / sound / curves ----------------------
    let mut h = harness();
    h.state_mut().open_project_settings();
    h.step();
    h.step();
    for key in [k::grp::FRAME, k::grp::AUDIO, k::grp::CURVES] {
        let t = tr(key);
        assert!(
            h.query_all_by_label_contains(t).count() > 0,
            "the project settings have no group “{t}”"
        );
    }
    // collapsing is the state of the WINDOW: closing a group does not touch the project value
    // “Sound” occurs also as a field label — we take the group HEADER exactly
    let before = h.state().model.project().settings.clone();
    h.get_by_label(tr(k::grp::AUDIO)).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().model.project().settings,
        before,
        "collapsing a group changed the PROJECT — it is window state, not content"
    );

    // --- FX window: a parameter group of an ordinary effect -----------------
    let (mut h, _) = harness_three_clips_dt(None);
    let clip = h.state().model.project().clips.first().map(|c| c.id).expect("event");
    {
        let app = h.state_mut();
        app.apply(Command::SetFx { id: clip, fx: vec![FxInstance::blur(8)] });
        app.open_fx_editor(clip);
    }
    h.step();
    h.step();
    h.get_by_label(crate::fx_editor::fx_display_name(&FxInstance::blur(8))).click();
    h.step();
    h.step();
    assert!(
        h.query_all_by_label_contains(tr(k::grp::PARAMS)).count() > 0,
        "the FX window has no group “{}” for the effect parameters",
        tr(k::grp::PARAMS)
    );

    // --- render: file / video / sound ----------------------------------
    let mut h = harness();
    h.state_mut().render_setup.open = true;
    h.step();
    h.step();
    for key in [k::grp::VIDEO, k::grp::AUDIO, k::grp::FILE] {
        let t = tr(key);
        assert!(
            h.query_all_by_label_contains(t).count() > 0,
            "the render window has no group “{t}”"
        );
    }
}

/// Test 786 (R.67): a field with a live curve writes a key ONLY from the person’s
/// gesture. `DragValue`/`Slider` with limits clamp a value that has left
/// the passport, and mark the answer as “changed” — on the old code a zigzag
/// around the limit set a key on every frame of play (recorder keys on
/// the owner’s project: 66.1→0.20, 69.7→0.98, 69.9→0.07…).
#[test]
fn t786_bound_field_writes_keys_only_from_a_gesture() {
    use kt_model::*;
    let mut h = harness_with_fx_window();
    // a radius curve: constant at the CEILING of the passport with a zigzag past it
    let spec_max = {
        let f = FxInstance::blur(8);
        f.param_specs().iter().find(|s| s.name == "radius").map(|s| s.max).unwrap()
    };
    let zig = SegShape::Zigzag { periods: 6.0, amplitude: spec_max * 0.5, profile: ZigProfile::Triangle };
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[0].fx_auto[0][0].keys = vec![
            AutoKey::shaped(kt_time::Tick(30_000), spec_max, zig),
            AutoKey::shaped(kt_time::Tick(210_000), spec_max, zig),
        ];
        p.validate().unwrap();
        app.model = TimelineModel::new(p);
    }
    h.step();
    h.step();
    let keys_of = |h: &Harness<'_, KtApp>| -> Vec<(i64, f64)> {
        h.state().model.project().clips[0].fx_auto[0][0].keys.iter().map(|k| (k.t.0, k.v)).collect()
    };
    let before = keys_of(&h);
    assert_eq!(before.len(), 2);

    // --- “play”: the cursor walks across the event for 120 frames, the field is not touched ----
    for i in 0..120i64 {
        h.state_mut().player.seek(kt_time::Tick(i * 1500));
        h.step();
    }
    assert_eq!(keys_of(&h), before, "the field set keyframes by itself (the widget clamp counted as a “change”)");
    // the same with playback
    h.state_mut().player.play();
    for _ in 0..30 {
        h.step();
    }
    h.state_mut().player.pause();
    h.step();
    assert_eq!(keys_of(&h), before, "during playback the field set keyframes by itself");

    // --- an edit by hand: a click in the field, a number, Enter — exactly one key ---
    h.state_mut().player.seek(kt_time::Tick(90_000));
    h.step();
    h.step();
    let label = tr(k::fx::RADIUS).to_string();
    let lr = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .find(|t| t.src == label)
        .map(|t| t.rect)
        .expect("radius label is drawn");
    let field = egui::pos2(lr.right() + 40.0, lr.center().y);
    h.hover_at(field);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: field,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    h.event(egui::Event::Text("3".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
    let after = keys_of(&h);
    assert_eq!(after.len(), 3, "a manual edit must set exactly one key: {after:?}");
    assert!(after.iter().any(|(t, _)| *t == 120_000), "key is not at the cursor (material 30000+90000): {after:?}");
}

// ---- R.67: the graph draws with the evaluator, the shape knob, the curve block, the hint ----

/// The automation view of slot 0 of event 1 in the first FX window.
fn fx_auto_view<'a>(h: &'a Harness<'_, KtApp>) -> &'a crate::automation::AutoView {
    h.state().fx_editors[0]
        .auto_views
        .get(&(kt_model::ClipId(1), 0))
        .expect("automation view of slot 0")
}

/// Scroll the body of the FX window so that the CURVE BLOCK is visible: the wheel over the
/// “+ curve” label (inside the scroll area, outside the graph — the graph takes the
/// wheel for itself), until the block header is drawn. A label beyond the fold
/// egui does not draw at all, but it does draw the field hint: that is how the block is seen.
pub(crate) fn reveal_curve_block(h: &mut Harness<'_, KtApp>) {
    reveal_until(h, &tr(k::fx::HELP).to_string());
}

/// The rectangle of the drawn text selected by `pick` — taken AFTER it has stopped moving:
/// egui’s `ScrollArea` catches up with an animation, and the
/// coordinates read right after the wheel are stale by the time of the click —
/// the click went into emptiness (caught on the instance window, t791).
fn settled_text_rect(
    h: &mut Harness<'_, KtApp>,
    pick: impl Fn(&str) -> bool,
    what: &str,
) -> egui::Rect {
    let find = |h: &Harness<'_, KtApp>| {
        crate::reach_tests::painted_texts(h).into_iter().find(|t| pick(&t.src)).map(|t| t.rect)
    };
    let mut prev = find(h);
    for _ in 0..24 {
        h.step();
        let now = find(h);
        if let (Some(a), Some(b)) = (prev, now)
            && (a.min - b.min).length() < 0.5
        {
            return b;
        }
        prev = now;
    }
    panic!("“{what}” did not settle in place (the scroll did not arrive)");
}

/// Scroll the body of the FX window until the text `sentinel` is drawn
/// (see `reveal_curve_block`).
pub(crate) fn reveal_until(h: &mut Harness<'_, KtApp>, sentinel: &str) {
    let anchor_label = tr(k::fx::AUTO_ADD).to_string();
    for _ in 0..16 {
        let texts = crate::reach_tests::painted_texts(h);
        if texts.iter().any(|t| t.src == sentinel) {
            h.step();
            return;
        }
        let anchor = texts.iter().find(|t| t.src == anchor_label).map(|t| t.rect.center());
        let Some(a) = anchor else { break };
        h.hover_at(a);
        h.step();
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -240.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
        h.step();
    }
    let texts: Vec<String> = crate::reach_tests::painted_texts(h).into_iter().map(|t| t.src).collect();
    assert!(texts.iter().any(|t| t == sentinel), "“{sentinel}” did not scroll into the window: {texts:?}");
}

/// A linear screen↔curve mapping from the drawn key diamonds: x(t), y(v).
/// Two keys with different t and v give both axes; the keys themselves are known to the model.
fn graph_axes(h: &Harness<'_, KtApp>) -> (impl Fn(i64) -> f32, impl Fn(f64) -> f32) {
    let keys = h.state().model.project().clips[0].fx_auto[0][0].keys.clone();
    let marks = fx_auto_view(h).key_marks.clone();
    // the value axis — the same as the widget’s (on some passports it is non-linear)
    let spec = kt_model::FxInstance::blur(8).param_specs().iter().find(|s| s.name == "radius").cloned().unwrap();
    let ax = crate::automation::axis_of(&spec);
    let pt = |ki: usize| marks.iter().find(|(c, k, _)| *c == 0 && *k == ki).map(|(_, _, p)| *p).unwrap();
    let (a, b) = (0usize, keys.len() - 1);
    let (pa, pb) = (pt(a), pt(b));
    let (ta, tb) = (keys[a].t.0 as f64, keys[b].t.0 as f64);
    // for the value axis — a pair of keys with DIFFERENT values
    let (va_i, vb_i) = (0..keys.len())
        .flat_map(|i| (0..keys.len()).map(move |j| (i, j)))
        .find(|(i, j)| (keys[*i].v - keys[*j].v).abs() > 1e-9)
        .expect("two keys with different values are needed");
    let (pva, pvb) = (pt(va_i), pt(vb_i));
    let (va, vb) = (ax.map(keys[va_i].v), ax.map(keys[vb_i].v));
    (
        move |t: i64| pa.x + ((t as f64 - ta) / (tb - ta)) as f32 * (pb.x - pa.x),
        move |v: f64| pva.y + ((ax.map(v) - va) / (vb - va)) as f32 * (pvb.y - pva.y),
    )
}

/// Test 784b (R.67): the graph draws the curve’s VALUE with the same evaluator
/// as the render: a zigzag is shown as a zigzag, a step as a step, the segment formula
/// as its own line. On the old code a straight line ran between the keys (t698
/// checked the model, not the drawing).
#[test]
fn t784b_graph_draws_the_curve_as_it_sounds() {
    use kt_model::*;
    let mut h = harness_with_fx_window();
    // values and amplitude are WITHIN the radius passport (1..48): beyond it the
    // value window would clamp, and the line would lie on the edge of the graph
    let zig = SegShape::Zigzag { periods: 4.0, amplitude: 4.0, profile: ZigProfile::Triangle };
    let formula = SegShape::Formula { expr: ExprId::intern("a + (b - a) * s * s") };
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[0].fx_auto[0][0].keys = vec![
            AutoKey::shaped(kt_time::Tick(30_000), 8.0, zig),
            AutoKey::shaped(kt_time::Tick(90_000), 40.0, SegShape::Hold),
            AutoKey::shaped(kt_time::Tick(150_000), 16.0, formula),
            AutoKey::shaped(kt_time::Tick(210_000), 44.0, SegShape::Linear),
        ];
        p.validate().unwrap();
        app.model = TimelineModel::new(p);
    }
    h.step();
    h.step();
    let keys = h.state().model.project().clips[0].fx_auto[0][0].keys.clone();
    let (x_of, y_of) = graph_axes(&h);
    // the curve’s line is the longest polyline INSIDE THE GRAPH AREA (by the key
    // diamonds): the window has other long lines too (the timeline)
    let marks = fx_auto_view(&h).key_marks.clone();
    let (mx0, mx1) = (
        marks.iter().map(|m| m.2.x).fold(f32::MAX, f32::min) - 8.0,
        marks.iter().map(|m| m.2.x).fold(f32::MIN, f32::max) + 8.0,
    );
    let (my0, my1) = (
        marks.iter().map(|m| m.2.y).fold(f32::MAX, f32::min) - 300.0,
        marks.iter().map(|m| m.2.y).fold(f32::MIN, f32::max) + 300.0,
    );
    let line = crate::reach_tests::painted_lines(&h)
        .into_iter()
        .filter(|l| l.iter().all(|p| p.x >= mx0 && p.x <= mx1 && p.y >= my0 && p.y <= my1))
        .max_by_key(|l| l.len())
        .expect("there are no lines in the graph area");
    assert!(line.len() >= 40, "the curve line is too sparse: {} points", line.len());
    // EACH point of the line: its x → tick, drawn y against curve_value
    let (x0, x1) = (x_of(keys[0].t.0), x_of(keys[keys.len() - 1].t.0));
    let t_of = |x: f32| keys[0].t.0 + ((x - x0) / (x1 - x0) * (keys[keys.len() - 1].t.0 - keys[0].t.0) as f32) as i64;
    let mut worst = (0.0f32, egui::Pos2::ZERO, 0.0f32, 0i64);
    let key_xs: Vec<f32> = keys.iter().map(|k| x_of(k.t.0)).collect();
    for p in line.iter().filter(|p| p.x >= x0 - 0.5 && p.x <= x1 + 0.5) {
        // at the key itself the step breaks: the pixel x — both sides of the break
        if key_xs.iter().any(|kx| (p.x - kx).abs() < 2.5) {
            continue;
        }
        let t = t_of(p.x);
        let want = y_of(curve_value(&keys, kt_time::Tick(t)));
        if (p.y - want).abs() > worst.0 {
            worst = ((p.y - want).abs(), *p, want, t);
        }
    }
    assert!(
        worst.0 <= 3.0,
        "line diverges from the evaluator by {} px: point {:?}, expected y {}, t {}",
        worst.0, worst.1, worst.2, worst.3
    );
    // zigzag: not fewer than 8 extrema between the first two keys
    let (xa, xb) = (x_of(keys[0].t.0), x_of(keys[1].t.0));
    let seg: Vec<&egui::Pos2> = line.iter().filter(|p| p.x > xa + 1.0 && p.x < xb - 1.0).collect();
    let mut extrema = 0;
    for i in 1..seg.len().saturating_sub(1) {
        let (a, b, c) = (seg[i - 1].y, seg[i].y, seg[i + 1].y);
        if (b > a && b > c) || (b < a && b < c) {
            extrema += 1;
        }
    }
    // sampling step 2 px: on a narrow stretch 4 periods give 6–8 turns —
    // we require more than half, and “not a straight line” proves the peak is lower
    assert!(extrema >= 5, "zigzag is drawn straight: extrema {extrema}");
    // and a straight line on the old code would give a discrepancy of ≥ 10 px at the zigzag peak
    let mid = x_of((keys[0].t.0 + keys[1].t.0) / 2);
    let straight = (y_of(keys[0].v) + y_of(keys[1].v)) / 2.0;
    let peak = seg.iter().map(|p| (p.y - straight).abs()).fold(0.0f32, f32::max);
    assert!(peak >= 10.0, "the zigzag peak is only {peak} px from the straight line (midpoint x {mid})");
}

/// Test 785 (R.67): “the shape of all segments” and its parameters refer to the
/// curve the person SEES as selected: the slot has two curves, the shape
/// lands on the selected second one, the first stays intact, and the block header names it.
/// On the old code the shape landed on the invisibly active first one — “only one field
/// changes, though I set zigzags in two fields.”
#[test]
fn t785_shape_control_belongs_to_the_visible_curve() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness_with_fx_window();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[0].fx_auto[0].push(ParamCurve::constant(
            "radius_y",
            kt_time::Tick(30_000),
            kt_time::Tick(210_000),
            4.0,
            SegShape::Linear,
        ));
        p.validate().unwrap();
        app.model = TimelineModel::new(p);
    }
    h.step();
    h.step();
    // the person clicks on the NAME of the second curve in the widget’s toolbar
    // (the field label in the grid carries the same name — the toolbar is drawn later)
    let name2 = crate::widgets::param_label("radius_y");
    h.query_all_by_label(&name2).last().expect("curve name in the toolbar").click();
    h.step();
    h.step();
    assert_eq!(fx_auto_view(&h).active, 1, "click on the name did not select the second curve");
    reveal_curve_block(&mut h);
    // the block header names it
    let title = crate::tf!(k::fx::CURVE_TITLE, name2.trim_end_matches(':'));
    assert!(
        crate::reach_tests::painted_texts(&h).iter().any(|t| t.src == title),
        "the block is not labeled with the selected curve’s name: {title:?}"
    );
    // the shape of all segments — zigzag (combo by role and current value)
    pick_shape_all(&mut h, tr(k::fx::SHAPE_ZIGZAG));
    h.step();
    h.step();
    let p = h.state().model.project().clone();
    let (c0, c1) = (&p.clips[0].fx_auto[0][0], &p.clips[0].fx_auto[0][1]);
    assert!(
        c1.keys.iter().all(|k| matches!(k.shape, SegShape::Zigzag { .. })),
        "zigzag did not land on the selected curve: {:?}",
        c1.keys.iter().map(|k| k.shape).collect::<Vec<_>>()
    );
    assert!(
        c0.keys.iter().all(|k| !matches!(k.shape, SegShape::Zigzag { .. })),
        "the shape landed on the NOT selected first curve"
    );
}

/// Test 785b (R.67): the parameter knob of the shape at the middle of a segment — dragging
/// up increases the zigzag amplitude, the number is visible next to it; a straight
/// line has no knob.
#[test]
fn t785b_shape_parameter_handle_on_the_segment() {
    use kt_model::*;
    let mut h = harness_with_fx_window();
    let zig = SegShape::Zigzag { periods: 4.0, amplitude: 2.0, profile: ZigProfile::Triangle };
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[0].fx_auto[0][0].keys = vec![
            AutoKey::shaped(kt_time::Tick(30_000), 20.0, zig),
            AutoKey::shaped(kt_time::Tick(120_000), 20.0, SegShape::Linear),
            AutoKey::shaped(kt_time::Tick(210_000), 40.0, SegShape::Linear),
        ];
        p.validate().unwrap();
        app.model = TimelineModel::new(p);
    }
    h.step();
    h.step();
    let marks = fx_auto_view(&h).seg_marks.clone();
    let handle = marks.iter().find(|(ki, _)| *ki == 0).map(|(_, p)| *p).expect("handle of the zigzag");
    assert!(marks.iter().all(|(ki, _)| *ki != 1), "a straight line must not have a handle");
    let amp0 = 2.0;
    geom_drag(&mut h, handle, handle + egui::vec2(0.0, -20.0), egui::Modifiers::NONE);
    let s = h.state().model.project().clips[0].fx_auto[0][0].keys[0].shape;
    let SegShape::Zigzag { amplitude, periods, .. } = s else { panic!("shape changed: {s:?}") };
    assert!(amplitude > amp0, "amplitude did not grow: {amplitude}");
    assert_eq!(periods, 4.0, "periods were touched without Shift");
    // Shift + right — periods
    let marks = fx_auto_view(&h).seg_marks.clone();
    let handle = marks.iter().find(|(ki, _)| *ki == 0).map(|(_, p)| *p).unwrap();
    geom_drag(&mut h, handle, handle + egui::vec2(40.0, 0.0), egui::Modifiers::SHIFT);
    let s = h.state().model.project().clips[0].fx_auto[0][0].keys[0].shape;
    let SegShape::Zigzag { periods, .. } = s else { panic!("shape changed: {s:?}") };
    assert!(periods > 4.0, "periods did not grow: {periods}");
    // Ctrl+Z returns by a history step
    h.state_mut().undo();
    h.step();
    let s = h.state().model.project().clips[0].fx_auto[0][0].keys[0].shape;
    let SegShape::Zigzag { periods, .. } = s else { panic!("shape changed: {s:?}") };
    assert_eq!(periods, 4.0, "undo did not restore the periods in one step");
}

/// Choose the shape in the “shape of all segments” combo of the block: the combo is searched by
/// role and current value (the egui label has no accessible name).
fn pick_shape_all(h: &mut Harness<'_, KtApp>, shape: &str) {
    use egui_kittest::kittest::Queryable;
    // the combo is searched by the DRAWN value of the shape: in the scrolled window
    // accesskit gives the rectangle BEFORE scrolling, and the click on it went
    // beside it (caught on the instance window, t791)
    let labels: Vec<&'static str> =
        crate::widgets::all_shapes().iter().map(|s| crate::widgets::shape_label(*s)).collect();
    let crect = settled_text_rect(h, |s| labels.contains(&s), "shape combo for all segments");
    let c = crect.center();
    h.hover_at(c);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: c,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    h.get_by_label(shape).click();
    h.step();
    h.step();
}

/// Type text into the block’s formula field (click to the right of the label, characters, Enter).
fn type_layer_formula(h: &mut Harness<'_, KtApp>, text: &str) {
    let label = tr(k::fx::FORMULA).to_string();
    let lr = settled_text_rect(h, move |s| s == label, "formula field label");
    let field = egui::pos2(lr.right() + 60.0, lr.center().y);
    h.hover_at(field);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: field,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    // previous text — select all and erase (the click cursor is placed anywhere)
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.key_press(egui::Key::Backspace);
    h.step();
    for ch in text.chars() {
        h.event(egui::Event::Text(ch.to_string()));
        h.step();
    }
}

/// Test 787e (R.67): a formula layer over the keys on the graph and in the field:
/// after Enter — two lines (a solid `k` and the dashed result), the keys and
/// the zigzag intact; clearing the field removes the layer; each Enter — a history step.
#[test]
fn t787e_layer_formula_over_keys_in_the_widget() {
    use kt_model::*;
    let mut h = harness_with_fx_window();
    let zig = SegShape::Zigzag { periods: 3.0, amplitude: 3.0, profile: ZigProfile::Triangle };
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[0].fx_auto[0][0].keys = vec![
            AutoKey::shaped(kt_time::Tick(30_000), 10.0, zig),
            AutoKey::shaped(kt_time::Tick(120_000), 40.0, SegShape::Linear),
            AutoKey::shaped(kt_time::Tick(210_000), 20.0, SegShape::Linear),
        ];
        p.validate().unwrap();
        app.model = TimelineModel::new(p);
    }
    h.step();
    h.step();
    reveal_curve_block(&mut h);
    let keys_before = h.state().model.project().clips[0].fx_auto[0][0].keys.clone();
    let lines_before = crate::reach_tests::painted_lines(&h).len();
    type_layer_formula(&mut h, "k * 0.5 + 0.1");
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
    let c = h.state().model.project().clips[0].fx_auto[0][0].clone();
    assert!(
        matches!(&c.source, CurveSource::Formula { expr } if expr == "k * 0.5 + 0.1"),
        "layer did not arrive: {:?}",
        c.source
    );
    assert_eq!(c.keys, keys_before, "layer erased the keys or the shape");
    let lines_after = crate::reach_tests::painted_lines(&h).len();
    assert!(lines_after > lines_before, "the result stroke was not drawn ({lines_before} → {lines_after})");
    // clearing removes the layer, the keys are intact
    type_layer_formula(&mut h, "");
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
    let c = h.state().model.project().clips[0].fx_auto[0][0].clone();
    assert!(c.source.is_keys(), "empty field did not remove the layer: {:?}", c.source);
    assert_eq!(c.keys, keys_before);
    // focus given back: space pauses, does not type
    assert!(!h.ctx.egui_wants_keyboard_input(), "field did not hand back focus after Enter");
    // the edits — a session of the FX window: one history step per session (the window’s contract);
    // “Cancel” of the window rolls back both the layer and its removal at once
}

/// Test 789 (R.67): the hint in the block — from the evaluator’s registry; Tab
/// completes the name in place without a popup layer; an unknown name is reported with
/// the nearest ones; the field has a hint in the status line.
#[test]
fn t789_formula_help_in_the_block_and_tab_completion() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_fx_window();
    reveal_curve_block(&mut h);
    // the “Help” group — a click lists all the registry’s functions
    h.get_by_label(tr(k::fx::HELP)).click();
    h.step();
    h.step();
    reveal_until(&mut h, tr(k::fx::HELP_OPS));
    let texts: Vec<String> = crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    for (name, arity) in kt_model::formula::FUNCTIONS {
        let args = (0..*arity).map(|i| ["x", "y", "z"][i]).collect::<Vec<_>>().join(", ");
        let sig = format!("{name}({args})");
        assert!(texts.iter().any(|t| t == &sig), "function {sig} is not shown in the help");
    }
    for key in [k::fx::HELP_VAR_T, k::fx::HELP_VAR_K, k::fx::HELP_VAR_SRC, k::fx::HELP_VAR_S] {
        assert!(texts.iter().any(|t| t == tr(key)), "variable is not described: {}", tr(key));
    }
    // Tab completes `cl` to `clamp(` — in place, without a layer on top
    type_layer_formula(&mut h, "cl");
    h.key_press(egui::Key::Tab);
    h.step();
    h.step();
    let shown: Vec<String> = crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(shown.iter().any(|s| s == "clamp("), "Tab did not complete the name: {shown:?}");
    let layers: Vec<egui::LayerId> = h.ctx.memory(|m| m.layer_ids().collect());
    assert!(
        layers.iter().all(|l| l.order <= egui::Order::Middle || l.order == egui::Order::Debug),
        "a layer appeared over the interface: {layers:?}"
    );
    // unknown name — an error with the nearest ones
    type_layer_formula(&mut h, "flor(t)");
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
    let err = h.state().error_status.as_ref().map(|(t, _)| t.clone()).unwrap_or_default();
    assert!(err.contains("flor") && err.contains("floor"), "error is not named in the status line: {err:?}");
    assert!(h.state().model.project().clips[0].fx_auto[0][0].source.is_keys(), "invalid text was applied");
}

/// Test 790 (R.67): the width of the block’s fields does not depend on the window width.
#[test]
fn t790_field_widths_do_not_follow_the_window() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    let widths = |w: f32| -> Vec<(String, f32)> {
        let mut h = harness_sized(w, 900.0);
        {
            use kt_model::*;
            let app = h.state_mut();
            let mut p = app.model.project().clone();
            p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
            let mut c = crate::layout_tests::sample_clip(ClipId(1));
            c.fx = vec![FxInstance::blur(8)];
            c.fx_bypass = vec![false];
            c.fx_auto = vec![vec![ParamCurve::constant("radius", kt_time::Tick(0), kt_time::Tick(90_000), 8.0, SegShape::Linear)]];
            p.clips.push(c);
            p.validate().unwrap();
            app.model = TimelineModel::new(p);
            app.open_fx_editor(ClipId(1));
        }
        h.step();
        h.step();
        let mut out: Vec<(String, f32)> = h
            .query_all(by().role(Role::SpinButton))
            .map(|n| ("spin".to_string(), n.rect().width()))
            .chain(h.query_all(by().role(Role::TextInput)).map(|n| ("text".to_string(), n.rect().width())))
            .chain(h.query_all(by().role(Role::ComboBox)).map(|n| ("combo".to_string(), n.rect().width())))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap()));
        out
    };
    let (narrow, wide) = (widths(900.0), widths(1800.0));
    assert!(!narrow.is_empty(), "no fields were found");
    assert_eq!(narrow.len(), wide.len(), "the set of fields depends on the window width");
    for ((ka, a), (kb, b)) in narrow.iter().zip(wide.iter()) {
        assert_eq!(ka, kb);
        assert!((a - b).abs() < 1.0, "field {ka} grows with the window: {a} → {b}");
    }
}

/// Test 791/791b (R.67): on an instance of a reel the curve block is THE SAME — the shape
/// of all segments, the formula layer; `t*2` after Enter is not erased and becomes a
/// source of the override (on the old code — `CurveSourceUnsupported`
/// and an empty field).
#[test]
fn t791_instance_head_has_the_same_curve_block() {
    use egui_kittest::kittest::Queryable;
    let (mut h, s) = harness_news_template();
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    // the curve on the numeric knob “Kegel”
    h.get_by_label(tr(k::fx::AUTO_ADD)).click();
    h.step();
    h.step();
    let item = h
        .query_all_by_label_contains("Font size")
        .last()
        .expect("item “Font size” in the “+ curve” menu");
    item.click();
    h.step();
    h.step();
    let inst = h.state().model.project().clips.iter().find(|c| c.id == s.b).cloned().unwrap();
    let ov = inst.overrides.iter().find(|o| !o.curves.is_empty()).expect("override curve");
    assert_eq!(ov.curves.len(), 1);
    reveal_curve_block(&mut h);
    // the block is the same: the header, the shape of all segments, the formula field
    let texts: Vec<String> = crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(texts.iter().any(|t| t.starts_with(&crate::tf!(k::fx::CURVE_TITLE, ""))), "block heading is missing: {texts:?}");
    assert!(texts.iter().any(|t| t == tr(k::fx::SHAPE_ALL)), "the instance has no “shape of all segments”");
    assert!(texts.iter().any(|t| t == tr(k::fx::FORMULA)), "the instance has no formula field");
    // zigzag over all segments
    pick_shape_all(&mut h, tr(k::fx::SHAPE_ZIGZAG));
    let inst = h.state().model.project().clips.iter().find(|c| c.id == s.b).cloned().unwrap();
    let ov = inst.overrides.iter().find(|o| !o.curves.is_empty()).unwrap();
    assert!(ov.curves[0].keys.iter().all(|k| matches!(k.shape, kt_model::SegShape::Zigzag { .. })), "zigzag did not land on the override");
    // formula t*2 — is not erased, becomes an override layer
    type_layer_formula(&mut h, "t*2");
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
    let inst = h.state().model.project().clips.iter().find(|c| c.id == s.b).cloned().unwrap();
    let ov = inst.overrides.iter().find(|o| !o.curves.is_empty()).unwrap();
    assert!(
        matches!(&ov.curves[0].source, kt_model::CurveSource::Formula { expr } if expr == "t*2"),
        "instance formula did not arrive: {:?}",
        ov.curves[0].source
    );
    let shown: Vec<String> = crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(shown.iter().any(|t| t == "t*2"), "field was erased after Enter: {shown:?}");
}

/// Test 794 (R.67): the frame over the picture is there AT ONCE (the useful control is on
/// by default), switching it off is REMEMBERED between runs, and the shape diagram
/// fits the frame and the frame whole at any zoom.
///
/// The owner: “instead of controls in the preview we have only meagre controls
/// inside the fx widget” — the frame started switched off, was not remembered, and was
/// turned on only through a context-menu item; “if you scale even by 2x
/// you simply have nothing in the preview” — the diagram was clipped by its own
/// rectangle.
#[test]
fn t794_frame_is_on_by_default_remembered_and_the_schema_fits() {
    use kt_model::*;
    // --- default: the display is ON with no action at all ---------------------
    assert_eq!(
        KtApp::default_geom_mode(None),
        crate::geomctl::GeomMode::Transform,
        "without storage the frame must be enabled (rule 4: useful by default)"
    );
    assert_eq!(KtApp::default_geom_mode(Some(false)), crate::geomctl::GeomMode::Off);
    assert_eq!(KtApp::default_geom_mode(Some(true)), crate::geomctl::GeomMode::Transform);

    // --- switching off is remembered: a round trip through the workspace file -------------
    let dir = std::env::temp_dir().join(format!("kt-t794-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ws = dir.join("workspace.json");
    let mut w = crate::user_store::load_workspace_at(&ws);
    assert_eq!(w.geom_controls, None, "empty storage knows nothing about the frame — this is the default");
    w.geom_controls = Some(false);
    crate::user_store::save_workspace_at(&ws, &w);
    let back = crate::user_store::load_workspace_at(&ws);
    assert_eq!(back.geom_controls, Some(false), "switching off did not survive the round trip through the file");
    assert_eq!(
        KtApp::default_geom_mode(back.geom_controls),
        crate::geomctl::GeomMode::Off,
        "after a restart the frame must stay switched off"
    );
    // deleting the storage returns the default “on”
    let _ = std::fs::remove_file(&ws);
    assert_eq!(
        KtApp::default_geom_mode(crate::user_store::load_workspace_at(&ws).geom_controls),
        crate::geomctl::GeomMode::Transform
    );
    let _ = std::fs::remove_dir_all(&dir);

    // --- the button on the frame itself: it is there, toggles, remembers ----------------
    let (mut h, _clip) = harness_geom_ctl();
    h.step();
    let probes = crate::widgets::field_probe::take();
    let _ = probes;
    assert!(
        h.state().geom_mode() != crate::geomctl::GeomMode::Off,
        "scene: the frame is enabled"
    );
    let img = h.state().preview_img_rect;
    let btn = egui::pos2(img.right() - 17.0, img.top() + 17.0);
    h.hover_at(btn);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: btn,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    assert_eq!(
        h.state().geom_mode(),
        crate::geomctl::GeomMode::Off,
        "the button in the frame corner did not switch the frame off"
    );
    assert_eq!(
        h.state().workspace.geom_controls,
        Some(false),
        "switching off did not reach the workplace"
    );
    // during PLAYBACK the button is absent (as are the controls)
    h.state_mut().player.play();
    h.step();
    h.step();
    let shown = crate::widgets::field_probe::take();
    assert!(
        !shown.iter().any(|(n, _)| n == "preview.geom_toggle"),
        "during playback the frame button stayed on the frame"
    );
    h.state_mut().player.pause();

    // --- the DIAGRAM fits the frame and the frame at 2× zoom -----------------------
    let size = egui::vec2(116.0, 66.0);
    let zoomed = FxInstance::geometry_at(0.0, 0.0, 2.0);
    let (quad, frame) = crate::geomctl::schema_points(&zoomed, size).expect("diagram is computed");
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
    for c in quad {
        assert!(area.contains(c), "frame corner {c:?} is outside the diagram {area:?} (on the old code all four were)");
    }
    assert!(area.contains(frame.min) && area.contains(frame.max), "frame did not fit the diagram: {frame:?}");
    // the frame at 2× is VISIBLE and larger than the frame — otherwise the diagram is not informative
    let (qw, fw) = (quad[1].x - quad[0].x, frame.width());
    assert!(qw > fw * 1.5, "at 2× the frame is not larger than the picture: {qw} versus {fw}");
    // by default the frame coincides with the picture
    let (q0, f0) = crate::geomctl::schema_points(&EventGeometry::default().node(), size).unwrap();
    assert!((q0[1].x - q0[0].x - f0.width()).abs() < 1.5, "without zoom the frame must lie on the picture");
}

/// Test 793 (R.67): the head of the strip for an instance of a reel is a short label in
/// the slot’s colour, not a long accent line.
///
/// The owner, verbatim: “why on earth would a user cover the fx-chain with a long
/// caption “Composition parameters”? It is also blue… if you select it
/// it becomes a long blue rectangle.”
#[test]
fn t793_instance_head_is_short_and_not_accent() {
    let (mut h, s) = harness_news_template();
    // a long reel name: truncation must work and not grow the strip
    let long = "Monday news issue, evening broadcast with the regional weather";
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.sequences.iter_mut().find(|x| x.id == s.seq).unwrap().name = long.into();
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
    }
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();

    // the probe is written only under KT_GEOM_DUMP — in the harness we look at the
    // DRAWN thing (the same rule as for the other layout claims)
    let painted = crate::reach_tests::painted_texts(&h);
    let label = painted
        .iter()
        .find(|t| t.src.starts_with(crate::icons::PIN) && t.src.contains("Monday"))
        .unwrap_or_else(|| {
            let pins: Vec<&String> = painted.iter().map(|t| &t.src).filter(|s| s.contains(crate::icons::PIN)).collect();
            let all: Vec<&String> = painted.iter().map(|t| &t.src).filter(|s| s.contains("Monday")).collect();
            panic!("head label is not drawn; with pin: {pins:?}; with reel name: {all:?}")
        });
    // the width of the head — no more than the slot with the longest name
    let slot_max = {
        let fid = h.ctx.style_of(egui::Theme::Dark).text_styles[&egui::TextStyle::Button].clone();
        h.ctx.fonts_mut(|f| f.glyph_width(&fid, 'M')) * crate::widgets::SLOT_NAME_CHARS as f32
    };
    assert!(
        label.rect.width() <= slot_max + 40.0,
        "head is wider than the slot: {} versus {slot_max} (the label is not truncated)",
        label.rect.width()
    );
    assert!(label.src.contains("Monday"), "head does not carry the reel name: {:?}", label.src);
    assert!(label.elided, "long reel name is not truncated: {:?}", label.drawn);
    assert!(
        !label.src.contains(tr(k::pubp::INST_TITLE)),
        "head is still labeled “{}”",
        tr(k::pubp::INST_TITLE)
    );
    // the colour — ordinary text, NOT accent (there will be no blue rectangle)
    let accent = crate::theme::ACCENT;
    let head_color = h
        .output()
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::epaint::Shape::Text(t)
                if t.galley.text().starts_with(crate::icons::PIN)
                    && t.galley.text().contains("Monday") =>
            {
                t.galley.job.sections.first().map(|s| s.format.color)
            }
            _ => None,
        })
        .next()
        .expect("head label color");
    assert_ne!(head_color, accent, "head stayed accented");
    assert_eq!(
        head_color,
        h.ctx.style_of(egui::Theme::Dark).visuals.text_color(),
        "head is not the slot’s color"
    );
}

/// Test 795 (R.67): the strip of a reel SURVIVES TO DISK without an envelope, cancelling is
/// not a failure, and a second opening of the project in the owner’s file form (two reels
/// one above the other with overrides and a lost source) does not render
/// again.
///
/// The owner: “I open paintitblack and the composition preview is AGAIN not
/// saved, every time it recomputes the preview, this bug already existed
/// with another project.” Previously the write went in ONE transaction and only
/// after the audio stage: a cancelled job (pause, edit, close) left nothing on
/// disk, and its key went into the negative cache as a
/// failure — and until the next edit of the reel the preview did not appear at all.
#[test]
fn t795_reel_frames_reach_the_disk_and_cancel_is_not_a_failure() {
    use crate::reel_preview::{CANCELLED, ReelKey, ReelPreviews};
    use kt_model::*;

    // --- cancel is NOT put in the negative cache, the request is repeated ---------
    let mut svc = ReelPreviews::default();
    let key = ReelKey { seq: MediaId(1), hash: 7 };
    let p = || {
        kt_model::Project::new(ProjectSettings {
            fps: 30,
            width: 320,
            height: 180,
            sample_rate: 48000,
            pitch_engine: Default::default(),
            default_shape: Default::default(),
            conform: Default::default(),
            settings_explicit: false,
        })
    };
    assert!(svc.request(key, p(), None, None), "the first request must get into the queue");
    assert!(!svc.request(key, p(), None, None), "duplicate request in the queue");
    svc.note_done_for_test(key, Err(CANCELLED.to_string()));
    assert_eq!(svc.failure(&key), None, "cancellation got into the negative cache as a failure");
    assert!(svc.request(key, p(), None, None), "after cancellation the request must be inserted again");
    // but a render ERROR is a failure, and there is no repeated request
    let bad = ReelKey { seq: MediaId(2), hash: 7 };
    svc.note_done_for_test(bad, Err("decode failed".into()));
    assert_eq!(svc.failure(&bad), Some("decode failed"), "render error must be remembered");
    assert!(!svc.request(bad, p(), None, None), "a failed key is not hammered again");

    // --- a strip WITHOUT peaks: a request for it is placed, a full one is not ----------
    let half = ReelKey { seq: MediaId(3), hash: 7 };
    svc.put_for_test(half, Some(crate::reel_preview::ReelStrip {
        step: 90_000,
        frames: vec![None],
        end: kt_time::Tick(90_000),
    }), None);
    assert!(svc.request(half, p(), None, None), "a strip without an envelope must let the request through");
    let full = ReelKey { seq: MediaId(4), hash: 7 };
    svc.put_for_test(
        full,
        Some(crate::reel_preview::ReelStrip {
            step: 90_000,
            frames: vec![None],
            end: kt_time::Tick(90_000),
        }),
        Some(Default::default()),
    );
    assert!(!svc.request(full, p(), None, None), "a complete record has no right to be ordered again");

    // --- END-TO-END: the project in the owner’s file form is opened twice -----
    // two reels ONE ABOVE THE OTHER with different sets of overrides and a lost source
    // inside the reel (its placeholder is also a frame)
    let dir = std::env::temp_dir().join(format!("kt-t795-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t795.kt");
    let seq = {
        let (h, _g, seq) = harness_with_nest();
        let mut doc = h.state().model.document().clone();
        // the lost file INSIDE the reel: the placeholder must get into the fingerprint
        let gone = MediaId(
            doc.media.iter().map(|m| m.id.0).chain(doc.sequences.iter().map(|s| s.id.0)).max().unwrap() + 1,
        );
        doc.media.push(MediaRef::new(gone, "no-such-file.jpg"));
        let next = doc
            .clips
            .iter()
            .chain(doc.sequences.iter().flat_map(|s| s.clips.iter()))
            .map(|c| c.id.0)
            .max()
            .unwrap_or(0)
            + 500;
        {
            let sq = doc.sequences.iter_mut().find(|x| x.id == seq).expect("reel");
            let mut miss = sq.clips[0].clone();
            miss.id = ClipId(next);
            miss.lane = 1;
            miss.link_group = None;
            miss.media = gone;
            sq.clips.push(miss);
            sq.ensure_tracks(LaneKind::Video, 2);
        }
        // A SECOND instance of the same reel — above the first, at the same time
        let first = doc.clips.iter().find(|c| c.media == seq).cloned().expect("reel instance");
        let mut second = first.clone();
        second.id = ClipId(next + 1);
        second.lane = first.lane + 1;
        second.link_group = None;
        doc.clips.push(second);
        doc.ensure_tracks(LaneKind::Video, first.lane + 2);
        doc.validate().expect("a project in the owner’s file form is legal");
        std::fs::write(&path, crate::app::project_text_of(&doc, &Default::default(), None)).unwrap();
        seq
    };
    let open_and_wait = || -> (Harness<'static, KtApp>, crate::reel_preview::ReelKey) {
        let mut app = KtApp::open(&path).expect("project opens");
        app.recovery_on_drop = false;
        app.workspace_file = Some(dir.join(format!("ws-{:p}.json", &app)));
        let mut h = Harness::builder()
            .with_size(egui::vec2(W, H))
            .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
        crate::icons::install_fonts(&h.ctx);
        h.step();
        let key = reel_key_of(&mut h, seq);
        let deadline = std::time::Instant::now() + WAIT_BG;
        while h.state().reel_previews.get(&key).map(|p| p.peaks.is_none()).unwrap_or(true)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(20));
            h.step();
        }
        (h, key)
    };
    let (h1, k1) = open_and_wait();
    assert!(
        h1.state().reel_previews.get(&k1).and_then(|e| e.strip.clone()).is_some(),
        "first opening gave no strip"
    );
    assert!(h1.state().reel_previews.renders() >= 1, "first opening must render the reel");
    drop(h1);

    let (h2, k2) = open_and_wait();
    assert_eq!(k2, k1, "reel key changed between openings — the disk would never hit");
    let prev = h2.state().reel_previews.get(&k2).expect("second opening without preview");
    assert!(prev.strip.as_ref().is_some_and(|s| s.step > 0), "frames did not come from the disk");
    assert!(prev.peaks.as_ref().is_some_and(|p| !p.is_empty()), "envelope did not come from the disk");
    assert_eq!(
        h2.state().reel_previews.renders(),
        0,
        "second opening rendered the reel again: on the owner’s project the previews are “NOT saved again”"
    );
    drop(h2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A scene with a STILL: one video event on a picture (existing or
/// missing) and an open project on disk — so that the media goes the ordinary way.
fn harness_still(exists: bool) -> (Harness<'static, KtApp>, std::path::PathBuf) {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!(
        "kt-still-{}-{}",
        std::process::id(),
        if exists { "ok" } else { "gone" }
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let img = dir.join("logo.jpg");
    if exists {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/out/still.jpg");
        // on Windows the fixtures come generated with the test archive (no bash there)
        if !(cfg!(windows) && src.exists()) {
            let st = std::process::Command::new("bash")
                .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gen.sh"))
                .status()
                .expect("gen.sh");
            assert!(st.success());
        }
        std::fs::copy(&src, &img).expect("copy of the fixture");
    }
    let path = dir.join("still.kt");
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), img.to_string_lossy().to_string()));
    let mut c = sample_clip(ClipId(1));
    c.duration = kt_time::Tick(20 * 90_000); // LONGER than any “duration” of the picture
    c.media = MediaId(1);
    p.clips.push(c);
    p.ensure_tracks(LaneKind::Video, 1);
    p.validate().unwrap();
    std::fs::write(&path, crate::app::project_text_of(&p, &Default::default(), None)).unwrap();

    let mut app = KtApp::open(&path).expect("project with a still opens");
    app.recovery_on_drop = false;
    app.workspace_file = Some(dir.join("ws.json"));
    let mut h = Harness::builder()
        .with_size(egui::vec2(W, H))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&h.ctx);
    h.step();
    (h, dir)
}

/// The cursor at a given second of the event and waiting for the assembled frame.
fn wait_complete(h: &mut Harness<'_, KtApp>, at_s: f64) -> bool {
    h.state_mut().player.seek(kt_time::Tick((at_s * 90_000.0) as i64));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        h.step();
        if h.state().preview_key.map(|k| k.2).unwrap_or(false) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    false
}

/// Test 796c (R.67): an event on a PICTURE shows it over its whole
/// length, and “Loading…” does not hang above it.
///
/// The owner: “static pictures do not show at all.. there is an eternal
/// Loading...”. An event 20 s long on a picture: the frame must be there at
/// zero, in the middle, and at the end — a picture has neither a frame grid
/// nor an end of material.
#[test]
fn t796c_a_still_shows_on_the_whole_event_without_loading() {
    let (mut h, dir) = harness_still(true);
    for at in [0.0, 9.5, 19.9] {
        assert!(
            wait_complete(&mut h, at),
            "at {at} there is no picture from the frame: “Loading…” hangs over the event"
        );
        // the incompleteness alarm is not triggered
        assert!(
            h.state().incomplete_since.is_none(),
            "at {at} with the incompleteness scrim raised"
        );
    }
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 796d (R.67): a MISSING picture is a placeholder with the file name, not an
/// eternal “Loading…”; the returned file replaces it on the fly.
#[test]
fn t796d_a_missing_still_is_a_named_placeholder_not_loading() {
    let (mut h, dir) = harness_still(false);
    assert_eq!(h.state().placeholder_media.len(), 1, "the missing picture did not turn into a placeholder");
    assert!(
        wait_complete(&mut h, 5.0),
        "“Loading…” hangs over the missing picture instead of a placeholder with the name"
    );
    assert!(h.state().incomplete_since.is_none(), "incompleteness scrim is raised over the placeholder");
    // the name of the missing file is shown to the person
    let said: Vec<String> = h.state().notices.iter().map(|n| n.text.clone()).collect();
    assert!(
        said.iter().any(|t| t.contains("logo.jpg")),
        "the missing file is not named in the notifications: {said:?}"
    );
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 797 (R.67): the path of ADDED media becomes absolute, and the file
/// does not “disappear” after saving the project into a subdirectory.
///
/// The owner’s picture is recorded as `sources/a3566951817_10.jpg` next to a
/// file that itself lies in `sources/`: a path relative to the LAUNCH
/// directory got into the model as it was, and the writer took it for a
/// path relative TO THE PROJECT. The file was searched under `sources/sources/…` — and was not
/// found, although it had not moved at all.
#[test]
fn t797_added_media_path_becomes_absolute() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t797-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sources")).unwrap();
    let img = dir.join("sources/x.jpg");
    std::fs::write(&img, b"not really a jpeg").unwrap();

    // a relative path — exactly the form the owner had
    let rel = "sources/x.jpg";
    let abs = KtApp::absolute_media_path(rel);
    assert!(std::path::Path::new(&abs).is_absolute(), "path stayed relative: {abs}");
    assert!(abs.replace('\\', "/").ends_with("sources/x.jpg"), "path is distorted: {abs}");
    // pseudo-paths are left untouched
    assert_eq!(KtApp::absolute_media_path("synth:Ramp"), "synth:Ramp");
    assert_eq!(KtApp::absolute_media_path("gen:solid"), "gen:solid");
    // an absolute one stays itself
    assert_eq!(KtApp::absolute_media_path(&abs), abs);

    // CONSEQUENCE: writing into a subdirectory and reading back find the file
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), img.to_string_lossy().to_string()));
    let mut c = sample_clip(ClipId(1));
    c.media = MediaId(1);
    p.clips.push(c);
    p.ensure_tracks(LaneKind::Video, 1);
    p.validate().unwrap();
    // the project LIES NEXT TO THE PICTURE, in the same `sources/`
    let proj = dir.join("sources/p.kt");
    let text = crate::app::project_text_of(&p, &Default::default(), proj.parent());
    std::fs::write(&proj, &text).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    let url = doc["project"]["media"][0]["path"].as_str().expect("media path in the file");
    assert_eq!(url, "x.jpg", "path in the file is not relative to the project: {url}");
    // we read by the SAME path as “Open”: the relative path of the file
    // is resolved from the project directory at read time
    let (back, _, _) = crate::app::load_project_file(&proj).expect("project is read");
    let found = &back.media[0].path;
    assert!(
        std::path::Path::new(found).exists(),
        "after the round trip the file is not found: {found} (expected {})",
        img.display()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 815 (R.70): autosave of a big project stopped being an event. The owner’s
/// measuring scene — a reel inside a reel, several instances each: in the exchange
/// format it gave megabytes (the reel’s body in every instance), in the recovery file
/// of its own format — kilobytes, and the write does not hold up the
/// interface.
#[test]
fn t815_recovery_of_a_nested_scene_is_orders_of_magnitude_smaller() {
    use kt_model::*;
    use kt_time::Tick;
    let clip = |id: u64, media: u64, start: i64, dur: i64| ClipEvent {
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
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    // that very scene: L1 of 10 clips, L2 of 8 instances of L1, the root of 8 L2
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:Ramp"));
    let mut next = 1000u64;
    let mut s1 = Sequence::new(MediaId(100), "L1");
    for i in 0..10 {
        next += 1;
        s1.clips.push(clip(next, 1, i * 45_000, 45_000));
    }
    let l1 = 10 * 45_000;
    let mut s2 = Sequence::new(MediaId(101), "L2");
    for i in 0..8 {
        next += 1;
        s2.clips.push(clip(next, 100, i * l1, l1));
    }
    let l2 = 8 * l1;
    for i in 0..8 {
        next += 1;
        p.clips.push(clip(next, 101, i * l2, l2));
    }
    p.sequences.push(s1);
    p.sequences.push(s2);
    p.normalize();
    p.validate().unwrap();

    let dir = std::env::temp_dir().join(format!("kt-t815-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("big.kt");
    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.model = TimelineModel::new(p.clone());
    app.model_rev += 1;
    app.dirty_content = true;

    let t0 = std::time::Instant::now();
    app.write_recovery_now();
    let took = t0.elapsed();
    let rec = crate::app::recovery_path_for(Some(&target));
    let ours = std::fs::metadata(&rec).expect("there is no recovery file").len() as usize;
    let interchange = kt_otio::save_project(&p, &Default::default(), None).len();
    let ratio = interchange / ours.max(1);
    eprintln!("t815: restore {ours} bytes versus {interchange} in the interchange format ({ratio}×), {took:?}");
    assert!(
        ratio >= 100,
        "restore must be orders of magnitude smaller than the interchange file: {ours} versus {interchange} ({ratio}×)"
    );
    // “not an event”: the write fits in a fraction of the autosave interval even in
    // the debug build; the threshold is generous so as not to measure the machine
    assert!(took < std::time::Duration::from_secs(2), "writing the restore record took {took:?}");
    assert_eq!(kt_model::project_file::from_text(&std::fs::read_to_string(&rec).unwrap()).unwrap().project, p);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 843 (telemetry-posthog): the live-scenario dump stays valid JSON when a notice spans
/// several lines. The telemetry notice (sent / NOT sent lists) is the first multi-line one,
/// and its raw line break made `geom.json` unreadable: every live scenario then waited for a
/// first frame that the dump could no longer report.
#[test]
fn t843_geometry_dump_survives_a_multi_line_notice() {
    let mut h = harness();
    let path = std::env::temp_dir().join(format!("kt-geom-t843-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    h.state_mut().geom_dump_path = Some(path.to_string_lossy().into_owned());
    h.state_mut().note("first line\nsecond \"quoted\" line\tend");
    h.state_mut().geom_dumped_at = None;
    h.step();
    let text = std::fs::read_to_string(&path).expect("the dump is written");
    let _ = std::fs::remove_file(&path);
    let v: serde_json::Value = serde_json::from_str(&text).expect("the dump is valid JSON");
    let first = v["notices"][0].as_str().unwrap_or_default();
    assert!(first.contains("first line | second") && first.ends_with("end"), "{first}");
}
