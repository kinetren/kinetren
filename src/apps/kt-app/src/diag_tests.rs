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
/// window, the journal button too; "Economy UI" lives only in View, with its hint.
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
        k::perf::LOG_TITLE,
    ] {
        assert!(has(&head(key)), "the window lacks the line “{}…”", head(key));
    }
    // the FX backend line — one of its three forms
    let fx = [k::perf::VIDEO_FX_GPU, k::perf::VIDEO_FX_CPU, k::perf::VIDEO_FX_CPU_LOST]
        .map(head);
    assert!(fx.iter().any(|p| has(p)), "the window lacks the FX backend line");
    assert!(has("FFmpeg: open"), "the window lacks the FFmpeg totals");
    assert!(h.query_by_label(tr(k::perf::FLIGHT_SAVE)).is_some(), "the journal button is lost");
    assert!(
        h.query_by_label(tr(k::menu::view::ECONOMY)).is_none(),
        "“Economy UI” is repeated in the window"
    );

    // the one "Economy UI" switch: in View, and hovering it says what it does
    h.state_mut().perf.diag_open = false;
    h.step();
    h.get_by_label(tr(k::menu::view::TITLE)).click();
    h.step();
    h.step();
    let item = h.get_by_label(tr(k::menu::view::ECONOMY)).rect();
    h.event(egui::Event::PointerMoved(item.center()));
    h.step();
    h.step();
    assert_eq!(
        h.state().status_line_shown.0,
        tr(k::hint::ECONOMY),
        "hovering View → Economy UI does not say what it does"
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

