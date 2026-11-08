//! idle-cpu, stage 2: the UI never waits for a picture. The executor below holds its composite
//! until the test lets it go, so "does not wait" is a fact and not a clock: a call that waited for
//! it could never return.

use kt_engine::{FramePlan, GpuBackendInfo, GpuExecError, Player, VideoGpuExec};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// A kernel that takes as long as the test says.
struct HeldExec {
    info: GpuBackendInfo,
    open: Mutex<bool>,
    cv: Condvar,
    started: AtomicU32,
}

impl HeldExec {
    fn new() -> Arc<HeldExec> {
        Arc::new(HeldExec {
            info: GpuBackendInfo { name: "held".into(), backend: "Test".into(), software: true },
            open: Mutex::new(false),
            cv: Condvar::new(),
            started: AtomicU32::new(0),
        })
    }

    fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.cv.notify_all();
    }
}

impl VideoGpuExec for HeldExec {
    fn execute_into(&self, _p: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        self.started.fetch_add(1, Ordering::SeqCst);
        let mut g = self.open.lock().unwrap();
        while !*g {
            g = self.cv.wait(g).unwrap();
        }
        out.clear();
        out.extend_from_slice(&[1, 2, 3, 255]);
        Ok(true)
    }

    fn info(&self) -> &GpuBackendInfo {
        &self.info
    }
}

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: true,
    });
    p.media.push(MediaRef::new(MediaId(1), "src"));
    p
}

fn player() -> Player {
    let src = SyntheticSource::new(Tick(600_000), AudioPattern::Silence);
    Player::new(project(), vec![(MediaId(1), Box::new(src))]).0
}

fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = kt_time::test_deadline();
    while !cond() {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Test t1840: with a composite stuck in its kernel, an ask, an edit and another ask all return;
/// the frame of the state the edit left is abandoned, the waiting asks collapse into the newest, and
/// only that one comes out.
#[test]
fn t1840_a_stuck_composite_holds_nobody() {
    let exec = HeldExec::new();
    let p = player();
    p.set_video_gpu(Some(exec.clone()));
    let first = p.ask_preview(Tick(0), None);
    wait_until("the composite never started", || exec.started.load(Ordering::SeqCst) == 1);
    assert!(p.take_composed(Duration::ZERO).is_none(), "nothing is finished");

    // everything here is what an Undo does, on the UI thread, while the kernel is stuck
    let mut edited = project();
    edited.settings.fps = 30;
    p.set_project(edited);
    assert_eq!(p.video_grid().frame_step(), 3000, "the edit reached the renderer at once");
    let mut last = first;
    for i in 1..5 {
        last = p.ask_preview(Tick(i * 3000), None);
    }
    assert!(p.missing_layers(Tick(0)).is_empty(), "a query of the renderer waits for nobody");

    exec.release();
    let mut got = None;
    wait_until("the last ask was never answered", || {
        got = p.take_composed(Duration::from_millis(5));
        got.is_some()
    });
    let done = got.unwrap();
    assert_eq!(done.seq, last, "a frame of a state the model has left came out");
    assert_eq!(done.t, Tick(4 * 3000));
    assert_eq!(done.rgba, vec![1, 2, 3, 255]);
    assert_eq!(exec.started.load(Ordering::SeqCst), 2, "the queue is one composite, not five");
}

/// Test t1840b: a finished frame wakes the UI, which has stopped asking for frames.
#[test]
fn t1840b_a_finished_frame_wakes_the_ui() {
    let exec = HeldExec::new();
    exec.release();
    let p = player();
    p.set_video_gpu(Some(exec));
    let woken = Arc::new(AtomicU32::new(0));
    let w = woken.clone();
    p.set_compose_wake(Arc::new(move || {
        w.fetch_add(1, Ordering::SeqCst);
    }));
    p.ask_preview(Tick(0), None);
    wait_until("no wake", || woken.load(Ordering::SeqCst) == 1);
    assert!(p.take_composed(Duration::ZERO).is_some());
}

/// A display executor that keeps every presented frame under one key, and whose memo answers only
/// when the test lets it: the engine's lock held by a composite.
struct KeptExec {
    info: GpuBackendInfo,
    keys_open: Mutex<bool>,
    cv: Condvar,
    asked: AtomicU32,
}

impl KeptExec {
    fn gate(&self, open: bool) {
        *self.keys_open.lock().unwrap() = open;
        self.cv.notify_all();
    }
}

impl VideoGpuExec for KeptExec {
    fn execute_into(&self, _p: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        out.clear();
        Ok(true)
    }

    fn info(&self) -> &GpuBackendInfo {
        &self.info
    }

    fn can_present(&self) -> bool {
        true
    }

    fn present(&self, _p: &FramePlan<'_>, _o: kt_engine::PresentOptions) -> Result<bool, GpuExecError> {
        Ok(true)
    }

    fn present_key(&self, _p: &FramePlan<'_>) -> Option<u64> {
        Some(7)
    }

    fn holds(&self, _key: u64) -> bool {
        self.asked.fetch_add(1, Ordering::SeqCst);
        let mut g = self.keys_open.lock().unwrap();
        while !*g {
            g = self.cv.wait(g).unwrap();
        }
        true
    }
}

/// Test t1843 (ui-never-waits): the bars' "plays without work" never waits for the executor or a
/// plan. With the executor's memo held, an edit leaves the kept moment stale: the bars' query
/// returns at once and says the check is pending, a new frame is asked, and the check runs on the
/// preview's thread when the memo answers.
#[test]
fn t1843_the_bars_never_wait_for_the_executor() {
    let exec = Arc::new(KeptExec {
        info: GpuBackendInfo { name: "kept".into(), backend: "Test".into(), software: true },
        keys_open: Mutex::new(true),
        cv: Condvar::new(),
        asked: AtomicU32::new(0),
    });
    let p = player();
    p.set_video_gpu(Some(exec.clone()));
    p.ask_preview(Tick(0), Some(kt_engine::PresentOptions::default()));
    let covers = |runs: &[std::ops::Range<i64>]| runs.iter().any(|r| r.start <= 0 && 0 < r.end);
    wait_until("the presented moment never reached the bars", || {
        let (runs, pending) = p.final_video_runs();
        !pending && covers(&runs)
    });

    // the memo stands, as the engine's lock does under a composite of seconds
    exec.gate(false);
    let asked = exec.asked.load(Ordering::SeqCst);
    let mut edited = project();
    edited.settings.width = 640;
    p.set_project(edited);
    let (runs, pending) = p.final_video_runs();
    assert!(pending && !covers(&runs), "a moment presented before the edit is still drawn as kept");
    wait_until("the check never started on the preview's thread", || exec.asked.load(Ordering::SeqCst) > asked);
    // the preview's thread now stands in the memo; everything the UI does goes on
    for _ in 0..3 {
        let (_, pending) = p.final_video_runs();
        assert!(pending);
        p.ask_preview(Tick(0), Some(kt_engine::PresentOptions::default()));
    }
    exec.gate(true);
    wait_until("the kept moment was never checked against the edit", || {
        let (runs, pending) = p.final_video_runs();
        !pending && covers(&runs)
    });
}
