//! diagnostics-three-graphs (kkt-26.11.01 item 14, registry K14): the perf box of the left column
//! moves into the Diagnostics window, which then carries three graphs.
//!
//! The owner (line 24 of `docs/ktprompts/kkt-26.11.01.txt`, translated): Performance must move
//! into the Diagnostics window, so that there are three graphs there. Tests t1145–t1148 (t1149 is in `graphs.rs`); the contract — `openspec/specs/ui-timeline`
//! ("The performance panel tells the person the truth").

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::reach_tests::painted_texts;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// Every text painted this frame inside `area`.
fn texts_in(h: &Harness<'_, KtApp>, area: egui::Rect) -> Vec<(String, egui::Rect)> {
    painted_texts(h)
        .into_iter()
        .filter(|t| area.intersects(t.rect))
        .map(|t| (t.src, t.rect))
        .collect()
}

/// The text before the first placeholder of a catalog template ("Cache {0}/{1} MB" → "Cache").
fn head(key: crate::i18n::Key) -> String {
    tr(key).split('{').next().unwrap_or_default().trim().to_string()
}

fn diag_rect(h: &Harness<'_, KtApp>) -> Option<egui::Rect> {
    egui::AreaState::load(&h.ctx, egui::Id::new("kt_diag")).and_then(|a| a.rect().is_positive().then(|| a.rect()))
}

/// The top of each graph title as painted, in the order quality, FFmpeg, churn.
fn graph_tops(h: &Harness<'_, KtApp>) -> Vec<f32> {
    let painted = painted_texts(h);
    [k::perf::GRAPH_QUALITY, k::perf::GRAPH_DECODE, k::perf::GRAPH_CHURN]
        .iter()
        .map(|key| {
            painted
                .iter()
                .find(|t| t.src == tr(*key))
                .unwrap_or_else(|| panic!("the graph “{}” is not painted", tr(*key)))
                .rect
                .top()
        })
        .collect()
}

fn click_at(h: &mut Harness<'static, KtApp>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
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

/// Test 1145: the left column is the bin's; the gauge and View → Diagnostics… open the window.
#[test]
fn t1145_left_column_is_the_bins_and_both_doors_open_diagnostics() {
    // the owner's working width: at 1440 the transport row puts the gauge past the right edge
    // (the toolbar-width follow-up of kkt-26.11.01 item 12, not this change)
    let mut h = crate::layout_tests::harness_sized(1900.0, 1000.0);
    h.step();
    let col = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("left_col"))
        .expect("the left column is drawn")
        .outer_rect;
    let in_col = texts_in(&h, col);
    for key in [
        k::perf::TITLE,
        k::perf::GRAPH_QUALITY,
        k::perf::GRAPH_DECODE,
        k::perf::GRAPH_CHURN,
        k::perf::FLIGHT_SAVE,
    ] {
        assert!(
            !in_col.iter().any(|(t, _)| t == tr(key)),
            "“{}” is still painted in the left column: {in_col:?}",
            tr(key)
        );
    }
    for key in [k::perf::CACHE_MB, k::perf::PREVIEW_RES, k::perf::COMPOSE, k::perf::V_SHOWN] {
        let prefix = head(key);
        assert!(
            !in_col.iter().any(|(t, _)| t.starts_with(&prefix)),
            "a perf line “{prefix}…” is still in the left column"
        );
    }
    // the bin reaches the bottom of the column: nothing stands under it any more
    let bin = h.state().bin_rect;
    assert!(bin.is_positive(), "the bin did not report its rectangle");
    assert!(
        col.bottom() - bin.bottom() < 40.0,
        "the bin ends at {:.0}, the column at {:.0}: something still stands under the bin",
        bin.bottom(),
        col.bottom()
    );

    // door 1: the gauge button of the toolbar
    assert!(!h.state().perf.diag_open);
    let gauges: Vec<_> = h.query_all_by_label(crate::icons::GAUGE).map(|n| n.rect()).collect();
    assert_eq!(gauges.len(), 1, "one gauge button in the toolbar: {gauges:?}");
    click_at(&mut h, gauges[0].center());
    assert!(h.state().perf.diag_open, "the gauge button did not open Diagnostics");
    assert!(diag_rect(&h).is_some(), "the Diagnostics window is not drawn");
    let win = diag_rect(&h).unwrap();
    let title = painted_texts(&h)
        .into_iter()
        .find(|t| t.src == tr(k::perf::TITLE) && win.contains_rect(t.rect))
        .expect("the window has no “Performance” section");
    assert!(title.rect.top() < win.top() + 80.0, "“Performance” is not at the top of the window");
    h.state_mut().perf.diag_open = false;
    h.step();
    h.step();

    // door 2: View → Diagnostics…
    h.get_by_label(tr(k::menu::view::TITLE)).click();
    h.step();
    h.step();
    h.get_by_label(tr(k::menu::view::DIAGNOSTICS)).click();
    h.step();
    h.step();
    assert!(h.state().perf.diag_open, "View → Diagnostics… did not open the window");
}

/// Test 1146: nothing of the former box is lost — the three graphs and every line are in the
/// window, the journal button too; "Economy UI" lives only in File › Preferences…, with its hint.
#[test]
fn t1146_diagnostics_carries_every_line_and_three_graphs() {
    let mut h = crate::layout_tests::harness();
    h.state_mut().perf.diag_open = true;
    h.step();
    h.step();
    let win = diag_rect(&h).expect("the Diagnostics window is drawn");
    let inside = texts_in(&h, win);
    let has = |prefix: &str| inside.iter().any(|(t, _)| t.starts_with(prefix));
    for key in [k::perf::GRAPH_QUALITY, k::perf::GRAPH_DECODE, k::perf::GRAPH_CHURN, k::perf::TITLE] {
        assert!(has(tr(key)), "the window lacks “{}”: {inside:?}", tr(key));
    }
    let tops = graph_tops(&h);
    assert!(tops[0] < tops[1] && tops[1] < tops[2], "the graphs are not one under another: {tops:?}");
    for key in [
        k::perf::CACHE_MB,
        k::perf::UNDERRUNS,
        k::perf::PREVIEW_RES,
        k::perf::CLIPS_N,
        k::perf::MEDIA_N,
        k::perf::OUT_LATENCY,
        k::perf::CALLBACK_FRAMES,
        k::perf::V_SHOWN,
        k::perf::V_SKIPPED,
        k::perf::COMPOSE,
        k::perf::CACHE_OCCUPANCY,
        k::perf::MEMORY_LEDGER,
        k::perf::LOG_TITLE,
    ] {
        assert!(has(&head(key)), "the window lacks the line “{}…”", head(key));
    }
    // the FX backend line — one of its three forms
    let fx = [k::perf::VIDEO_FX_GPU, k::perf::VIDEO_FX_NONE, k::perf::VIDEO_FX_LOST]
        .map(head);
    assert!(fx.iter().any(|p| has(p)), "the window lacks the FX backend line");
    assert!(has("FFmpeg: open"), "the window lacks the FFmpeg totals");
    assert!(h.query_by_label(tr(k::perf::FLIGHT_SAVE)).is_some(), "the journal button is lost");
    assert!(
        h.query_by_label(tr(k::prefs::ECONOMY)).is_none(),
        "“Economy UI” is repeated in the window"
    );

    // the one "Economy UI" switch: in File › Preferences… (welcome-window-and-settings moved the
    // program's preferences out of View, kkt-26.11.01 item 13), and hovering it says what it does
    h.state_mut().perf.diag_open = false;
    h.step();
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();
    h.step();
    h.get_by_label(tr(k::menu::file::PREFERENCES)).click();
    h.step();
    h.step();
    let item = h.get_by_label(tr(k::prefs::ECONOMY)).rect();
    h.event(egui::Event::PointerMoved(item.center()));
    h.step();
    h.step();
    assert_eq!(
        h.state().status_line_shown.0,
        tr(k::hint::ECONOMY),
        "hovering Preferences › Economy UI does not say what it does"
    );
}

/// Drag the window's bottom-right corner by `by`, in three steps (the harness rule).
fn drag_corner(h: &mut Harness<'static, KtApp>, by: egui::Vec2) {
    let win = diag_rect(h).unwrap();
    let from = win.right_bottom() - egui::vec2(3.0, 3.0);
    h.event(egui::Event::PointerMoved(from));
    h.step();
    h.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    h.event(egui::Event::PointerMoved(from + egui::vec2(8.0, 8.0)));
    h.step();
    h.event(egui::Event::PointerMoved(from + by));
    h.step();
    h.step();
    h.event(egui::Event::PointerButton {
        pos: from + by,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    h.step();
}

/// Test 1147: the three graphs grow with the window and stay inside it; the window keeps the
/// size it was given.
#[test]
fn t1147_graphs_grow_with_the_window() {
    let mut h = crate::layout_tests::harness();
    h.state_mut().perf.diag_open = true;
    // samples, so the graphs draw lines and not the empty state
    for v in [10.0, 40.0, 25.0] {
        let app = h.state_mut();
        app.perf.quality.push(&[60.0, 30.0, v, v]);
        app.perf.decode.push(&[v, v, v, v, v]);
        app.perf.churn.push(&[v, v, v]);
    }
    h.step();
    h.step();
    let before = diag_rect(&h).unwrap();
    let tops = graph_tops(&h);
    let gaps = [tops[1] - tops[0], tops[2] - tops[1]];
    drag_corner(&mut h, egui::vec2(120.0, 120.0));
    let after = diag_rect(&h).unwrap();
    assert!(
        after.height() > before.height() + 60.0,
        "the corner drag did not enlarge the window: {before:?} → {after:?}"
    );
    let tops2 = graph_tops(&h);
    let gaps2 = [tops2[1] - tops2[0], tops2[2] - tops2[1]];
    for i in 0..2 {
        assert!(gaps2[i] > gaps[i] + 15.0, "graph {i} did not grow with the window: {gaps:?} → {gaps2:?}");
    }
    // the log is still inside, under the third graph
    let log = painted_texts(&h)
        .into_iter()
        .find(|t| t.src == tr(k::perf::LOG_TITLE))
        .expect("the problem log heading is not painted");
    assert!(log.rect.top() > tops2[2] && after.contains_rect(log.rect), "the log left the window");
    // nothing grows by itself after the gesture
    for _ in 0..6 {
        h.step();
    }
    let still = diag_rect(&h).unwrap();
    assert!(
        (still.height() - after.height()).abs() < 1.0 && (still.width() - after.width()).abs() < 1.0,
        "the window grew by itself: {after:?} → {still:?}"
    );
}

/// Test 1148: an open Diagnostics window does not heat an idle program — the next repaint is
/// asked no sooner than the idle tick, and the graphs rebuild nothing between frames.
#[test]
fn t1148_open_diagnostics_does_not_heat_an_idle_program() {
    // a SHORT harness step: with the default 0.25 s the program's own 250 ms idle tick is already
    // due by the next frame and every frame reads "repaint now"
    let mut h = crate::layout_tests::harness_lang_dt(crate::i18n::Lang::Ru, Some(0.02));
    h.state_mut().perf.diag_open = true;
    for v in [10.0, 40.0] {
        let app = h.state_mut();
        app.perf.quality.push(&[60.0, 30.0, v, v]);
        app.perf.decode.push(&[v, v, v, v, v]);
        app.perf.churn.push(&[v, v, v]);
    }
    // the window's appearance may animate for a few frames; then it must settle
    let _ = h.try_run();
    h.step();
    let builds = |h: &Harness<'_, KtApp>| {
        let p = &h.state().perf;
        (p.quality.builds, p.decode.builds, p.churn.builds)
    };
    let before = builds(&h);
    for _ in 0..5 {
        h.step();
        let delay = h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        assert!(
            delay >= std::time::Duration::from_millis(100),
            "an idle frame with Diagnostics open asks for a repaint in {delay:?}"
        );
    }
    // the sampler may push once in these frames (every 500 ms of frames): allow exactly that
    let after = builds(&h);
    for (a, b) in [(before.0, after.0), (before.1, after.1), (before.2, after.2)] {
        assert!(b - a <= 1, "a graph rebuilt {} times in five idle frames", b - a);
    }
}


/// A journal bound to this test thread BEFORE the app is built: the flight sink takes the
/// journal current where the player is built.
fn journal(name: &str) -> (std::sync::Arc<crate::tele_journal::Journal>, crate::tele_journal::testing::Bound) {
    use crate::tele_journal::testing::*;
    let clock = FakeClock::new(1_762_498_800_000, 0);
    let j = fresh(name, clock, crate::tele_journal::ROTATE_BYTES);
    let b = bind(j.clone());
    (j, b)
}

/// The kind and text of each journal record, in order: `ACT edit AddMarker`.
fn records(j: &crate::tele_journal::Journal) -> Vec<String> {
    j.tail(1000)
        .iter()
        .map(|r| {
            let rest = r.line.split_once("  +").and_then(|(_, x)| x.split_once("  ")).map_or("", |(_, x)| x);
            rest.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .collect()
}

/// The index of the first record from `from` that starts with `prefix`.
fn find_from(recs: &[String], from: usize, prefix: &str) -> usize {
    recs.iter()
        .skip(from)
        .position(|r| r.starts_with(prefix))
        .map(|i| i + from)
        .unwrap_or_else(|| panic!("no “{prefix}” after record {from}: {recs:#?}"))
}

/// Test t1961 (telemetry-journal): what the person did shows up in the journal by name and in
/// order — an edit, an undo, a window opened and closed, a playback with its summary, a mark.
/// The owner: "Ты как поймешь что я делал в load-report.txt?"
#[test]
fn t1961_an_action_shows_up_with_its_name() {
    let (j, _bound) = journal("t1961");
    let mut h = crate::layout_tests::harness();
    h.step();
    h.state_mut().apply(kt_model::Command::AddMarker { at: kt_time::Tick(90_000), duration: kt_time::Tick(0) });
    h.state_mut().undo();
    h.state_mut().perf.diag_open = true;
    h.step();
    h.step();
    h.state_mut().perf.diag_open = false;
    h.step();
    h.step();
    h.state().player.play();
    h.step();
    h.state().player.pause();
    h.step();
    h.state_mut().mark_moment("  stall here ");
    let recs = records(&j);
    let mut at = find_from(&recs, 0, "ACT edit AddMarker");
    at = find_from(&recs, at + 1, "ACT undo");
    let title = tr(k::win::DIAG_TITLE);
    at = find_from(&recs, at + 1, &format!("ACT window opened: {title}"));
    at = find_from(&recs, at + 1, &format!("ACT window closed: {title}"));
    at = find_from(&recs, at + 1, "PLAY PLAY head=");
    at = find_from(&recs, at + 1, "PLAY PAUSE head=");
    at = find_from(&recs, at + 1, "PLAY playback stopped after");
    at = find_from(&recs, at + 1, "MARK stall here");
    assert!(recs[at].ends_with("stall here"), "the note is trimmed, nothing else: {}", recs[at]);
    // a live gesture is one record, however many frames it sends
    for _ in 0..5 {
        h.state_mut().apply_coalesced(kt_model::Command::AddMarker { at: kt_time::Tick(180_000), duration: kt_time::Tick(0) }, 77);
    }
    let gestures = records(&j).iter().filter(|r| r.contains("(a live gesture)")).count();
    assert_eq!(gestures, 1, "{:#?}", records(&j));
}

/// The rectangle of a published place, after the frame.
fn probe(name: &str) -> egui::Rect {
    crate::widgets::field_probe::rect_of(name).unwrap_or_else(|| panic!("“{name}” is not published"))
}

/// Test t1968 (telemetry-journal, found-defects 109): the journal pane is on screen at the
/// window's default size and at its minimum — the owner had to make the window taller to see
/// the log at all (kkt-26.11.07b line 17).
#[test]
fn t1968_journal_is_on_screen_at_any_window_height() {
    let (j, _bound) = journal("t1968");
    for i in 0..40 {
        j.record(crate::tele_journal::Kind::Note, &format!("note {i}"));
    }
    let mut h = crate::layout_tests::harness();
    h.state_mut().perf.diag_open = true;
    h.step();
    h.step();
    h.step();
    let line_h = 6.0 * 12.0;
    let check = |h: &Harness<'_, KtApp>, when: &str| {
        let win = diag_rect(h).expect("the Diagnostics window is drawn");
        let log = probe("diag.log");
        let mark = probe("diag.mark");
        assert!(win.contains_rect(log), "{when}: the journal pane {log:?} leaves the window {win:?}");
        assert!(log.height() >= line_h, "{when}: the journal pane is {:.0} points high", log.height());
        assert!(win.contains_rect(mark), "{when}: the mark button {mark:?} is outside the window {win:?}");
        // the newest record is painted inside the pane
        let painted = painted_texts(h);
        let last = painted.iter().find(|t| t.src.ends_with("note 39")).unwrap_or_else(|| panic!("{when}: the newest record is not painted"));
        assert!(log.contains_rect(last.rect) || log.intersects(last.rect), "{when}: the newest record is outside the pane");
    };
    check(&h, "default size");
    drag_corner(&mut h, egui::vec2(-2000.0, -2000.0));
    let win = diag_rect(&h).unwrap();
    assert!(win.height() < 360.0, "the window did not shrink to its minimum: {win:?}");
    h.step();
    check(&h, "minimum size");
}

/// Test t1969 (telemetry-journal): a mark from the Diagnostics button with its note, then from
/// the key in the main window; the note field empties.
#[test]
fn t1969_a_mark_from_the_button_and_from_the_key() {
    let (j, _bound) = journal("t1969");
    let mut h = crate::layout_tests::harness();
    h.state_mut().perf.diag_open = true;
    h.step();
    h.step();
    click_at(&mut h, probe("diag.mark_note").center());
    h.event(egui::Event::Text("stall here".into()));
    h.step();
    click_at(&mut h, probe("diag.mark").center());
    assert!(h.state().perf.mark_note.is_empty(), "the note field kept “{}”", h.state().perf.mark_note);
    h.state_mut().perf.diag_open = false;
    h.step();
    h.step();
    let key = egui::Key::M;
    let mods = egui::Modifiers { ctrl: true, alt: true, ..Default::default() };
    h.event(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: mods });
    h.event(egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: mods });
    h.step();
    h.step();
    let marks: Vec<String> = records(&j).into_iter().filter(|r| r.starts_with("MARK")).collect();
    assert_eq!(marks, ["MARK stall here", "MARK the person marked this moment"], "{:#?}", records(&j));
    let said = h.state().notices.back().map(|n| n.text.clone()).unwrap_or_default();
    let head = tr(k::msg::MOMENT_MARKED).split('{').next().unwrap_or_default().to_string();
    assert!(said.starts_with(&head), "the program does not say the moment is marked: {said:?}");
}

/// t2301 (memory-ledger): under the cache occupancy bar the window names where the memory went —
/// the kept composites, the history, the frames, the device buffers — and the room, in MB.
#[test]
fn t2301_the_ledger_line_is_painted_under_the_occupancy_bar() {
    let held = kt_time::mem::open(kt_time::mem::Tenant::Kept, 0);
    held.set(3 << 20);
    let mut h = crate::layout_tests::harness();
    h.state_mut().perf.diag_open = true;
    h.step();
    h.step();
    let win = diag_rect(&h).expect("the Diagnostics window is drawn");
    let inside = texts_in(&h, win);
    let line = |key| inside.iter().find(|(t, _)| t.starts_with(&head(key))).cloned();
    let (occ, occ_rect) = line(k::perf::CACHE_OCCUPANCY).expect("the occupancy line");
    let (mem, mem_rect) = line(k::perf::MEMORY_LEDGER).unwrap_or_else(|| panic!("no ledger line under “{occ}”: {inside:?}"));
    assert!(mem_rect.top() > occ_rect.bottom() + 4.0, "the ledger line is under the occupancy bar: {occ_rect:?} {mem_rect:?}");
    let kept: u64 = mem
        .trim_start_matches(&head(k::perf::MEMORY_LEDGER))
        .split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("the kept composites' MB first: {mem}"));
    assert!(kept >= 3, "the kept composites held: {mem}");
    let unit = tr(k::perf::MEMORY_LEDGER).rsplit('}').next().unwrap_or_default().trim().to_string();
    assert!(mem.ends_with(&unit), "the room in the catalog's unit “{unit}”: {mem}");
    drop(held);
}

/// t2304 (orch stage 9): the Diagnostics window opens at its floors where the screen has the room —
/// the instruments, the three graphs at their floor and the journal's six lines — so the third
/// graph is not under the fold, and a corner drag gives each graph its quarter of the growth (D2).
/// The fixed 720-point default fell under the floors as rows were added: the live `diagnostics`
/// drag first paid off the shortfall and the graphs grew 9 points of 140.
#[test]
fn t2304_diagnostics_opens_at_its_floors() {
    let mut h = crate::layout_tests::harness();
    h.set_size(egui::vec2(1440.0, 1100.0));
    h.step();
    // the rows a working program has and a bare harness does not: the UI frame's two lines and the
    // journal button's stall count (the live run's rows put the floors past the fixed default)
    {
        let app = h.state_mut();
        app.perf.frame_ms = Some([0.5; crate::frame_cost::Stage::ALL.len()]);
        app.player.flight.push(kt_engine::flight::Ev::Stall, 1_500, 0);
        app.perf.diag_open = true;
    }
    for _ in 0..3 {
        h.step();
    }
    let win = diag_rect(&h).expect("the Diagnostics window is drawn");
    let churn = probe("diag.graph.churn");
    let mark = probe("diag.mark");
    assert!(
        churn.bottom() < mark.top(),
        "the third graph opens under the fold: graph {churn:?}, journal head {mark:?}, window {win:?}"
    );
    assert!(win.bottom() <= 1100.0, "the window leaves the screen: {win:?}");
    let before: Vec<f32> = ["diag.graph.quality", "diag.graph.decode", "diag.graph.churn"].iter().map(|g| probe(g).height()).collect();
    drag_corner(&mut h, egui::vec2(100.0, 100.0));
    let after_win = diag_rect(&h).unwrap();
    let grown = after_win.height() - win.height();
    assert!(grown > 60.0, "the corner drag did not enlarge the window: {win:?} → {after_win:?}");
    for (i, g) in ["diag.graph.quality", "diag.graph.decode", "diag.graph.churn"].iter().enumerate() {
        let d = probe(g).height() - before[i];
        assert!(d >= 0.25 * grown - 2.0, "{g} took {d:.1} of {grown:.1}: a graph takes a quarter of the growth");
    }
}

/// t2302 (memory-ledger D11): opening a project gives its preview decoders a render's threads by the
/// memory measured then, not libavcodec's auto (a thread per core).
#[test]
fn t2302_the_preview_decoders_take_a_renders_threads() {
    kt_ffmpeg::set_decode_threads(0);
    let _h = crate::layout_tests::harness();
    let n = kt_ffmpeg::decode_threads_set();
    assert!(n == 2 || n == 4, "a render's rule, not auto: {n}");
}
