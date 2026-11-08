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

/// A kernel that stands as long as the test says: an anomalous parameter with no end in sight.
struct StuckGpu {
    info: kt_engine::GpuBackendInfo,
    open: std::sync::Mutex<bool>,
    cv: std::sync::Condvar,
}

impl kt_engine::VideoGpuExec for StuckGpu {
    fn execute_into(
        &self,
        _p: &kt_engine::FramePlan<'_>,
        out: &mut Vec<u8>,
    ) -> Result<bool, kt_engine::GpuExecError> {
        let mut g = self.open.lock().unwrap();
        while !*g {
            g = self.cv.wait(g).unwrap();
        }
        out.clear();
        Ok(true)
    }

    fn info(&self) -> &kt_engine::GpuBackendInfo {
        &self.info
    }
}

/// Test t1842 (E8): with a composite stuck in its kernel the UI goes on: frames run, Undo acts at
/// the next frame and the program asks for the state Undo left. "Stuck" is a test's gate, so a frame
/// that waited for the picture could never return.
#[test]
fn t1842_undo_acts_while_the_kernel_is_stuck() {
    use kt_model::Command;
    let (mut h, _) = crate::layout_tests::harness_three_clips_dt(Some(0.02));
    let gpu = Arc::new(StuckGpu {
        info: kt_engine::GpuBackendInfo { name: "stuck".into(), backend: "Test".into(), software: true },
        open: std::sync::Mutex::new(false),
        cv: std::sync::Condvar::new(),
    });
    {
        let app = h.state_mut();
        app.compose_grace = Duration::ZERO;
        app.push_project_to_player();
        app.player.set_video_gpu(Some(gpu.clone()));
        app.preview_key = None;
    }
    h.step();
    let id = h.state().model.project().clips[0].id;
    let was = h.state().model.project().clips[0].fx.len();
    assert!(h.state_mut().apply(Command::SetFx {
        id,
        fx: vec![kt_model::FxInstance::blur(40)],
    }));
    for _ in 0..5 {
        h.step();
    }
    assert_eq!(h.state().model.project().clips[0].fx.len(), was + 1);
    h.state_mut().undo();
    h.step();
    assert_eq!(h.state().model.project().clips[0].fx.len(), was, "Undo waited for the picture");
    let version = h.state().player.project_version();
    assert!(
        h.state().preview_asked.is_some_and(|a| a.1.3 == version),
        "the frame of the state Undo left was not asked for"
    );
    *gpu.open.lock().unwrap() = true;
    gpu.cv.notify_all();
    h.state_mut().compose_grace = crate::app::COMPOSE_GRACE;
    h.step();
    assert!(h.state().preview_asked.is_none(), "the answer never came");
}
