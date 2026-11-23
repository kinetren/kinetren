//! idle-cpu (kkt-26.11.07 items E1, E2, E4): the program at rest costs nothing. What a harness can
//! see of it; the numbers on the heavy project are the live scenario `idle`.

use crate::app::{KtApp, STATUS_TTL_MS};
use crate::layout_tests::harness_lang_dt;
use crate::reel_preview::{MAX_REELS_READY, ReelKey, ReelPreview, ReelPreviews, reel_row};
use eframe::egui;
use egui_kittest::Harness;
use kt_model::{MediaId, Override, ParamValue, PubId};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

fn rest_harness() -> Harness<'static, KtApp> {
    let mut h = harness_lang_dt(crate::i18n::Lang::En, Some(0.02));
    for _ in 0..6 {
        h.step();
    }
    h
}

fn next_repaint(h: &Harness<'_, KtApp>) -> Duration {
    h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay
}

fn complete() -> ReelPreview {
    ReelPreview { strip: None, peaks: Some(Default::default()) }
}

/// Test t1830 (E1, E4): with more live releases of a composition than the service's cap, nothing is
/// evicted and nothing is asked again. The cap once evicted the oldest release, the frame asked it
/// again, its job evicted the next one: a circle of jobs for as long as the program stood on a
/// pause (the scenario `idle` counts 2 to 4 jobs a second on the old code).
#[test]
fn t1830_more_live_releases_than_the_cap_do_not_go_round() {
    let mut svc = ReelPreviews::default();
    let keys: Vec<ReelKey> =
        (0..MAX_REELS_READY as u64 + 3).map(|i| ReelKey { seq: MediaId(1000), hash: i }).collect();
    let live: HashMap<ReelKey, i64> = keys.iter().map(|k| (*k, 1000)).collect();
    svc.set_live(live, Arc::new(Vec::new()));
    for k in &keys {
        svc.put(*k, complete());
    }
    for k in &keys {
        assert!(svc.get(k).is_some(), "a release an event shows was evicted: {k:?}");
        assert!(!svc.wants(k), "a finished release asks for a job again: {k:?}");
    }
    // what no event shows is what the cap bounds
    svc.set_live(HashMap::new(), Arc::new(Vec::new()));
    svc.put(ReelKey { seq: MediaId(1), hash: 1 }, complete());
    assert_eq!(svc.ready_keys().len(), MAX_REELS_READY, "the cap still holds for what nothing shows");
}

/// Test t1831 (E4): two releases have two rows in the sidecar. With one row per reel each release
/// overwrote the other, and all but the last rendered again on every opening.
#[test]
fn t1831_each_release_has_its_own_sidecar_row() {
    let seq = MediaId(1000);
    let set = |t: &str| vec![Override::value(PubId(1), ParamValue::Text(t.into()))];
    assert_eq!(reel_row(seq, &[]), 1000, "an instance with no overrides keeps the row it always had");
    let (a, b) = (reel_row(seq, &set("A")), reel_row(seq, &set("B")));
    assert_ne!(a, b, "two releases share a row");
    assert!(a < 0 && b < 0, "a release's row collides with a reel's id");
    assert_eq!(a, reel_row(seq, &set("A")), "the row must not depend on anything but the set");
    let dir = std::env::temp_dir().join(format!("kt-t1831-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("t.ktindex.db");
    let mut db = kt_media::index::MediaIndexDb::open(&db_path).unwrap();
    let frame: &[u8] = b"jpeg";
    db.put_reel_frames(a, 11, 5, 90_000, 90_000, &[Some(frame)]).unwrap();
    db.put_reel_frames(b, 22, 5, 90_000, 90_000, &[Some(frame)]).unwrap();
    assert!(db.reel_preview(a, 11, 5).unwrap().is_some(), "release A lost its row to release B");
    assert!(db.reel_preview(b, 22, 5).unwrap().is_some());
    db.prune_reels(&[a]).unwrap();
    assert!(db.reel_preview(b, 22, 5).unwrap().is_none(), "a row nothing uses stays");
    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test t1832 (E2, E8): a frame that came out incomplete is not composed again on every UI frame.
/// The harness has no device, so every composite is incomplete and nothing ever arrives: the
/// frames below are the mouse moving over a picture that lacks something. Each used to pay the
/// whole chain.
#[test]
fn t1832_a_moving_pointer_does_not_recompose_an_incomplete_frame() {
    let (mut h, _) = crate::layout_tests::harness_three_clips_dt(Some(0.02));
    h.state_mut().push_project_to_player();
    h.state_mut().player.seek(kt_time::Tick(10_000));
    for _ in 0..3 {
        h.step();
    }
    assert!(h.state().incomplete_since.is_some(), "the scene must hold an incomplete frame");
    // past the retries on a clock, or a slow machine is a retry and this test measures the machine
    h.state_mut().compose_retry_gap = crate::app::COMPOSE_RETRY_MAX * 2;
    let before = h.state().composites;
    for i in 0..40 {
        h.input_mut().events.push(egui::Event::PointerMoved(egui::pos2(300.0 + i as f32, 300.0)));
        h.step();
    }
    let n = h.state().composites - before;
    assert!(n <= 1, "{n} composites over 40 frames of a moving pointer");
}

/// Test t1833 (E1): at rest the program asks for no frame at all, and a timed state asks for
/// exactly the frame it needs. The tick of 4 Hz drew the whole window for nobody.
#[test]
fn t1833_at_rest_nothing_asks_for_a_frame() {
    let mut h = rest_harness();
    h.step();
    assert!(next_repaint(&h) > Duration::from_secs(3600), "a program at rest asks for a frame in {:?}", next_repaint(&h));

    // a status line must go away by itself
    h.state_mut().status = Some(("x".to_string(), std::time::Instant::now()));
    h.step();
    let d = next_repaint(&h);
    assert!(d <= Duration::from_millis(STATUS_TTL_MS + 5), "a status line asks for its end in {d:?}");
    h.state_mut().status = None;

    // unsaved work must reach the recovery file with no input
    h.state_mut().dirty_content = true;
    h.state_mut().last_autosave = std::time::Instant::now();
    for _ in 0..4 {
        h.step();
    }
    let d = next_repaint(&h);
    assert!(
        d > Duration::from_secs(1) && d <= Duration::from_secs(h.state().autosave_secs) + Duration::from_millis(5),
        "autosave asks for its frame in {d:?}"
    );
    h.state_mut().dirty_content = false;
}

/// Test t1938 (cached-range-plays): the bright "plays without work" layer of the strips does not
/// cost a hover. Kept ranges over the events: a pointer passing over them lays the events out once
/// and the layer stays drawn — the bars' refresh is keyed by what the stores report, never by the
/// frame.
#[test]
fn t1938_a_hover_over_kept_ranges_does_not_lay_out_the_events() {
    let (mut h, g) = crate::layout_tests::harness_three_clips_dt(Some(0.02));
    let span = h.state().model.project().clips.iter().map(|c| (c.start + c.duration).0).max().unwrap_or(0);
    let kept: Vec<std::ops::Range<i64>> = std::iter::once(0..span).collect();
    h.state_mut().coverage_cache.final_runs =
        std::sync::Arc::new(crate::app::FinalRuns { video: kept.clone(), audio: kept });
    let hover = |h: &mut Harness<'_, KtApp>, p: egui::Pos2| {
        h.input_mut().events.push(egui::Event::PointerMoved(p));
        h.step();
    };
    // over the events of the first row, which the kept ranges cover
    let y = g.row_top(0) + 8.0;
    let x = g.rect.left() + 60.0;
    hover(&mut h, egui::pos2(x, y));
    hover(&mut h, egui::pos2(x + 1.0, y));
    let before = h.state().event_passes;
    for i in 2..30 {
        hover(&mut h, egui::pos2(x + i as f32 * 0.5, y));
    }
    assert_eq!(h.state().event_passes, before, "the pointer over kept ranges laid the events out again");
    assert!(h.state().event_paint.is_some());
    assert!(!h.state().coverage_cache.final_runs.video.is_empty(), "scene guard: the kept layer is drawn");
}

/// Test t1834 (E2): a pointer passing over static things lays the events out once, and only a
/// change of what lies under it does so again. Without this the cost of a hover frame scaled
/// with what the view held, so with the scroll (kkt-26.11.07 item E2).
#[test]
fn t1834_a_hover_frame_does_not_lay_out_the_events() {
    let (mut h, g) = crate::layout_tests::harness_three_clips_dt(Some(0.02));
    let hover = |h: &mut Harness<'_, KtApp>, p: egui::Pos2| {
        h.input_mut().events.push(egui::Event::PointerMoved(p));
        h.step();
    };
    // the empty end of the first row, past the three events
    let y = g.row_top(0) + 8.0;
    let x = g.rect.right() - 40.0;
    hover(&mut h, egui::pos2(x, y));
    hover(&mut h, egui::pos2(x - 1.0, y));
    let before = h.state().event_passes;
    for i in 2..30 {
        hover(&mut h, egui::pos2(x - i as f32, y));
    }
    assert_eq!(h.state().event_passes, before, "the pointer alone laid the events out again");
    assert!(h.state().event_paint.is_some());

    // anything but the pointer drops what was kept: a stale picture is worse than a slow one
    h.state_mut().view_start += 1.0;
    h.input_mut().events.push(egui::Event::Key {
        key: egui::Key::F20,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    });
    h.step();
    assert!(h.state().event_passes > before, "a frame with other input reused the events");
    assert!(h.state().event_paint.is_none());
}

/// The process's processor time so far (user + system), from `/proc/self/stat`; `None` elsewhere.
fn cpu_time() -> Option<Duration> {
    let s = std::fs::read_to_string("/proc/self/stat").ok()?;
    let rest = &s[s.rfind(')')? + 2..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    let ticks: u64 = f.get(11)?.parse::<u64>().ok()? + f.get(12)?.parse::<u64>().ok()?;
    Some(Duration::from_millis(ticks * 10))
}

/// Test t2559 (chain-everywhere): the effects gallery of a heavy event (Datamosh then VHS Tape) costs
/// nothing at rest once its cells are composed — the real-frame cells are computed once, a pointer
/// moving over the open gallery renders no cell and leaves no cell asked, and the program asks for
/// no frame on its account.
/// - **PATH** select an event carrying Datamosh and VHS Tape, open its effects window, click
///   "+ effect…": the cells read "Loading", then each shows the event's mosh with the candidate;
///   moving the mouse over the gallery changes nothing and the processor stays idle
#[test]
fn t2559_an_open_gallery_of_a_heavy_event_costs_nothing_at_rest() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    crate::plugins::install();
    if kt_gpu::offline_engine().is_none() {
        eprintln!("skipped: no offline device");
        return;
    }
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let mut h = crate::layout_tests::harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let mut mosh = FxInstance::plugin("datamosh").expect("datamosh");
    for (name, v) in [("lines", 0.0), ("before", 0.0), ("mosh", 1.0), ("gop", 12.0)] {
        assert!(mosh.set_param(name, v), "{name}");
    }
    let clip = |id: u64, start: i64, fx: Vec<FxInstance>| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(start),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    };
    p.clips.push(clip(1, 0, vec![mosh, FxInstance::plugin("vhs_tape").expect("vhs_tape")]));
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.push_project_to_player();
    app.open_fx_editor(ClipId(1));
    for ed in app.fx_editors.iter_mut() {
        ed.opened_at -= crate::fx_thumbs::DECODE_GUARD * 2;
    }
    h.step();
    h.step();
    let add = crate::i18n::tr(crate::i18n::k::fxreg::ADD).to_string();
    h.get_by_label(&add).click();
    h.step();

    // the cells come: the real frames, once
    let fill = cpu_time();
    let deadline = kt_time::test_deadline();
    let mut quiet = 0;
    while quiet < 5 {
        h.step();
        quiet = if h.state().fx_thumbs.pending() { 0 } else { quiet + 1 };
        assert!(std::time::Instant::now() < deadline, "the cells never settled");
        std::thread::sleep(Duration::from_millis(20));
    }
    let renders = kt_time::work::count(kt_time::work::Work::FxThumb);
    assert!(renders > 0, "no cell was composed");

    // at rest, the pointer over the gallery
    let rest = cpu_time();
    for i in 0..60 {
        h.input_mut().events.push(egui::Event::PointerMoved(egui::pos2(300.0 + (i % 30) as f32 * 8.0, 300.0)));
        h.step();
    }
    let after = cpu_time();
    assert_eq!(kt_time::work::count(kt_time::work::Work::FxThumb), renders, "a moving pointer rendered cells");
    assert!(!h.state().fx_thumbs.pending(), "cells were asked again at rest");
    if let (Some(a), Some(b), Some(c)) = (fill, rest, after) {
        eprintln!("t2559: {renders} cells composed in {:?} of processor time; 60 frames of a moving pointer {:?}", b - a, c - b);
    }
}

/// Test t2813 (render-settings): the render settings window open over a heavy event (Datamosh and
/// VHS Tape) costs nothing at rest: still or with the pointer moving over the window, the program
/// asks for no frame on its account, composes nothing new and starts no export.
/// - **PATH** a project with an event carrying Datamosh and VHS Tape → File → Render… → the
///   settings window stands open; the mouse rests, then moves over the window: nothing redraws by
///   itself and the processor stays idle
#[test]
fn t2813_an_open_render_settings_window_costs_nothing_at_rest() {
    use kt_model::*;
    crate::plugins::install();
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let mut h = rest_harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let fx = vec![FxInstance::plugin("datamosh").expect("datamosh"), FxInstance::plugin("vhs_tape").expect("vhs_tape")];
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
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
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.push_project_to_player();
    // the heavy event warms its history first: the project itself settles, then the window opens
    let deadline = kt_time::test_deadline();
    let mut quiet = 0;
    while quiet < 10 {
        h.step();
        quiet = if next_repaint(&h) > Duration::from_secs(3600) { quiet + 1 } else { 0 };
        assert!(std::time::Instant::now() < deadline, "the project never came to rest");
        std::thread::sleep(Duration::from_millis(20));
    }
    crate::layout_tests::render_settings_scene(&mut h);
    for _ in 0..20 {
        h.step();
    }
    let composites = h.state().composites;
    let cpu0 = cpu_time();
    for _ in 0..120 {
        h.step();
    }
    assert!(next_repaint(&h) > Duration::from_secs(3600), "the open window asks for a frame in {:?}", next_repaint(&h));
    let win = crate::widgets::field_probe::rect_of("win.render");
    let c = win.map_or(egui::pos2(500.0, 400.0), |r| r.center());
    for i in 0..120 {
        h.input_mut().events.push(egui::Event::PointerMoved(c + egui::vec2((i % 40) as f32 * 3.0 - 60.0, (i % 7) as f32 * 4.0)));
        h.step();
    }
    // a hovered button fades its highlight for a few frames, then the window is still again
    for _ in 0..15 {
        h.step();
    }
    assert!(next_repaint(&h) > Duration::from_secs(3600), "after the pointer the window asks for a frame in {:?}", next_repaint(&h));
    assert!(h.state().composites - composites <= 1, "the open window recomposed the preview");
    assert!(h.state().render_job.is_none(), "an export started by itself");
    if let (Some(a), Some(b)) = (cpu0, cpu_time()) {
        eprintln!("t2813: 240 frames with the render settings open cost {:?} of processor", b.saturating_sub(a));
    }
}

/// Test t3003 (fx-graph v1b): the effects window's node editor over a heavy event (Datamosh, VHS
/// Tape, Camera ISP) costs nothing at rest: still, or with the pointer moving over the graph, it
/// asks for no frame of its own.
/// - **PATH** a project with an event carrying Datamosh, VHS Tape and Camera ISP → double click it →
///   the effects window's node editor stands open; the mouse rests, then moves over the nodes:
///   nothing redraws by itself
#[test]
fn t3003_an_open_node_editor_costs_nothing_at_rest() {
    use kt_model::*;
    crate::plugins::install();
    let mut h = rest_harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let fx = ["datamosh", "vhs_tape", "camera_isp"].iter().map(|id| FxInstance::plugin(id).expect(id)).collect::<Vec<_>>();
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
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
        fx_bypass: vec![false; 3],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    // past the window's misclick guard, which asks for the frame that ends it
    for ed in app.fx_editors.iter_mut() {
        ed.opened_at -= crate::fx_thumbs::DECODE_GUARD * 2;
    }
    for _ in 0..6 {
        h.step();
    }
    let graph = crate::widgets::field_probe::rect_of("fx.graph").expect("the node editor is open");
    assert!(next_repaint(&h) > Duration::from_secs(3600), "the open node editor asks for a frame in {:?}", next_repaint(&h));
    for i in 0..30 {
        h.input_mut().events.push(egui::Event::PointerMoved(graph.left_center() + egui::vec2(8.0 + i as f32 * 12.0, 0.0)));
        h.step();
    }
    // the pointer rests where it stopped: the frames of the last move settle
    for _ in 0..6 {
        h.step();
    }
    assert!(next_repaint(&h) > Duration::from_secs(3600), "after a hover the node editor asks for a frame in {:?}", next_repaint(&h));
}

/// t3236 (track-device-sound): a video track's window with VHS Tape and Camera ISP and its sound
/// track — one graph of both chains and the Device sound row — costs nothing at rest: still, or with
/// the pointer moving over the graph. Its name field holds the focus, and a focused field's caret
/// does not blink (found-defects 821: egui's blink asked a frame every half second for ever; the
/// harness's text edit paints no caret, so the style itself is held here and live `idle` measures).
/// - **PATH** a video track carrying VHS Tape and Camera ISP, linked to Audio 1 → Track properties →
///   the track window's graph shows both lanes; the mouse rests, then moves over the nodes: nothing
///   redraws by itself
#[test]
fn t3236_an_open_track_pair_costs_nothing_at_rest() {
    use kt_model::*;
    crate::plugins::install();
    let mut h = rest_harness();
    // the program's window has the keyboard focus, as on a person's desktop: egui blinks a focused
    // text field's caret only then (the harness's window has none by default, and the live `idle`
    // found what this hid: the track window's name field took the focus and blinked for ever)
    h.input_mut().focused = true;
    let app = h.state_mut();
    let v1 = app.model.project().track_at(LaneKind::Video, 0).unwrap().id;
    let fx = ["vhs_tape", "camera_isp"].iter().map(|id| FxInstance::plugin(id).expect(id)).collect::<Vec<_>>();
    // the model built, not edited: an edit arms the autosave's clock, which is not the window's cost
    let mut m = kt_model::TimelineModel::new(app.model.project().clone());
    m.apply(Command::SetTrackFx { id: v1, fx, bypass: vec![] }).unwrap();
    assert!(m.project().track(v1).unwrap().sound.is_some(), "the pair is not linked");
    app.model = kt_model::TimelineModel::new(m.project().clone());
    app.open_track_window(v1);
    for _ in 0..6 {
        h.step();
    }
    let graph = crate::widgets::field_probe::rect_of("fx.graph").expect("the node editor is open");
    assert!(h.ctx.memory(|m| m.focused()).is_some(), "the window's name field holds the focus");
    assert!(!h.ctx.global_style().visuals.text_cursor.blink, "a focused field's caret blinks: a repaint every half second at rest");
    assert!(crate::widgets::field_probe::rect_of("track.sound").is_some(), "the pair's row is not drawn");
    assert!(next_repaint(&h) > Duration::from_secs(3600), "the open track pair asks for a frame in {:?}", next_repaint(&h));
    for i in 0..30 {
        h.input_mut().events.push(egui::Event::PointerMoved(graph.left_center() + egui::vec2(8.0 + i as f32 * 12.0, 0.0)));
        h.step();
    }
    for _ in 0..6 {
        h.step();
    }
    assert!(next_repaint(&h) > Duration::from_secs(3600), "after a hover the track pair asks for a frame in {:?}", next_repaint(&h));
}

/// Test t3822 (stream-picture-stays D2, D3): a paused Datamosh moment on one device costs nothing at
/// rest. The compute host holds the mosh's pictures and the preview composes on its device; once the
/// history is walked and the frame shown, a pointer moving over the preview walks no position, composes
/// nothing, submits nothing to the device, runs no prefetch pass, and no frame is asked.
/// - **PATH** the heavy showcase with a Datamosh loop → Space plays a lap → Space stops → the mouse
///   rests, then moves over the preview → nothing redraws or walks by itself
#[test]
fn t3822_a_paused_mosh_on_one_device_costs_nothing() {
    use kt_model::*;
    use kt_time::work::{count, Work};
    crate::plugins::install();
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    kt_gpu::compute::hold_pictures_for_tests();
    let Some(engine) = kt_gpu::front_on_compute() else {
        eprintln!("skipped: no compute device");
        return;
    };
    let mut h = rest_harness();
    let app = h.state_mut();
    // the first sync only records the paths; the second (below) raises the new source on the player
    app.model_rev += 1;
    app.sync_media_sources();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let mut mosh = FxInstance::plugin("datamosh").expect("datamosh");
    for (name, v) in [("lines", 0.0), ("before", 12.0), ("mosh", 1.0), ("gop", 12.0)] {
        assert!(mosh.set_param(name, v), "{name}");
    }
    let clip = |id: u64, start: i64, fx: Vec<FxInstance>| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(start),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    };
    p.clips.push(clip(1, 0, vec![]));
    p.clips.push(clip(2, 90_000, vec![mosh]));
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.model_rev += 1;
    app.sync_media_sources();
    app.push_project_to_player();
    app.player.set_video_gpu(Some(engine as std::sync::Arc<dyn kt_engine::VideoGpuExec>));
    app.player.seek(kt_time::Tick(120_000));
    app.preview_key = None;

    // the history walks and the frame comes: the walk first — a run of still counters before it is
    // a source still loading under load, not rest (found-defects 950's class)
    let snapshot = || {
        (
            kt_plugin_api::stream::positions_walked(),
            count(Work::Composite),
            count(Work::DeviceSubmit),
            count(Work::PrefetchPass),
        )
    };
    let deadline = kt_time::test_deadline();
    while kt_plugin_api::stream::positions_walked() == 0 || h.state().preview_catching_up > 0 {
        // the harness hears no wake (`wake::news` is 0 under test): the frame the walk completes is
        // asked again here, as the program's wake asks it
        h.state_mut().preview_key = None;
        h.step();
        std::thread::sleep(Duration::from_millis(20));
        assert!(std::time::Instant::now() < deadline, "the history was never walked to a complete frame");
    }
    // then nothing moves for a while
    let (mut last, mut still) = (snapshot(), 0);
    while still < 25 {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
        let now = snapshot();
        still = if now == last { still + 1 } else { 0 };
        last = now;
        assert!(std::time::Instant::now() < deadline, "the paused mosh never came to rest");
    }
    // at rest, the pointer over the preview
    for i in 0..60 {
        h.input_mut().events.push(egui::Event::PointerMoved(egui::pos2(600.0 + (i % 30) as f32 * 4.0, 200.0)));
        h.step();
    }
    assert_eq!(snapshot(), last, "a moving pointer walked, composed, submitted or passed (walked, composites, submits, passes)");
    // the pointer stops: a hover's own frames end, and nothing asks another
    for _ in 0..6 {
        h.step();
    }
    assert!(next_repaint(&h) > Duration::from_secs(3600), "a program at rest asks for a frame in {:?}", next_repaint(&h));
    assert_eq!(snapshot(), last, "the rest after the pointer ran work");
}
