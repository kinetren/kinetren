//! ui-never-waits (kkt-26.11.07 E8, kkt-26.11.08 H9): no interface action waits for a picture. The
//! executor below holds its composite until the test lets it go, so "does not wait" is a fact and not
//! a clock: a frame that waited for the picture could never return.

use crate::app::KtApp;
use eframe::egui;
use egui_kittest::Harness;
use kt_model::{ClipId, Command};
use std::sync::Arc;
use std::time::Duration;

/// A kernel that stands as long as the test says: an anomalous parameter with no end in sight.
struct StuckGpu {
    info: kt_engine::GpuBackendInfo,
    open: std::sync::Mutex<bool>,
    cv: std::sync::Condvar,
}

impl StuckGpu {
    fn new() -> Arc<StuckGpu> {
        Arc::new(StuckGpu {
            info: kt_engine::GpuBackendInfo { name: "stuck".into(), backend: "Test".into(), software: true },
            open: std::sync::Mutex::new(false),
            cv: std::sync::Condvar::new(),
        })
    }

    fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.cv.notify_all();
    }
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

/// The three-clip harness with the stuck executor injected and nothing waited for.
fn stuck_harness() -> (Harness<'static, KtApp>, Arc<StuckGpu>) {
    let (mut h, _) = crate::layout_tests::harness_three_clips_dt(Some(0.02));
    let gpu = StuckGpu::new();
    {
        let app = h.state_mut();
        app.compose_grace = Duration::ZERO;
        app.push_project_to_player();
        app.player.set_video_gpu(Some(gpu.clone()));
        app.preview_key = None;
    }
    h.step();
    (h, gpu)
}

/// Test t1842 (E8): with a composite stuck in its kernel the UI goes on: frames run, Undo acts at
/// the next frame and the program asks for the state Undo left.
#[test]
fn t1842_undo_acts_while_the_kernel_is_stuck() {
    let (mut h, gpu) = stuck_harness();
    let id = h.state().model.project().clips[0].id;
    let was = h.state().model.project().clips[0].fx.len();
    assert!(h.state_mut().apply(Command::SetFx { id, fx: vec![kt_model::FxInstance::blur(40)] }));
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
    gpu.release();
    h.state_mut().compose_grace = crate::app::COMPOSE_GRACE;
    h.step();
    assert!(h.state().preview_asked.is_none(), "the answer never came");
}

/// Test t1848 (E8, as the owner met it): an effect whose every frame never ends is added, its window
/// is open, and the person works: the window draws and its Done button closes it, the real Ctrl+Z
/// removes the effect, and playback starts and stops, all while the composite stands in its kernel.
#[test]
fn t1848_the_effects_window_and_ctrl_z_answer_while_the_kernel_is_stuck() {
    let (mut h, gpu) = stuck_harness();
    let id: ClipId = h.state().model.project().clips[0].id;
    let was = h.state().model.project().clips[0].fx.clone();
    let mut fx = was.clone();
    fx.push(kt_model::FxInstance::blur(40));
    assert!(h.state_mut().apply(Command::SetFx { id, fx }));
    h.state_mut().open_fx_editor(id);
    for _ in 0..4 {
        h.step();
    }
    let done = crate::widgets::field_probe::rect_of("fx.done").expect("the effects window drew its Done button");

    let ctrl = egui::Modifiers { ctrl: true, command: true, ..Default::default() };
    h.key_press_modifiers(ctrl, egui::Key::Z);
    h.step();
    h.step();
    assert_eq!(h.state().model.project().clips[0].fx, was, "Ctrl+Z did not remove the effect");

    h.key_press(egui::Key::Space);
    h.step();
    assert!(h.state().player.transport.is_playing(), "Space did not start playback");
    h.key_press(egui::Key::Space);
    h.step();
    assert!(!h.state().player.transport.is_playing(), "Space did not stop playback");

    let pos = done.center();
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
    assert!(h.state().fx_editors.is_empty(), "the effects window did not close on Done");
    gpu.release();
}

/// Test t1846 (H9: the owner's journal said a frame stood seconds, not where): a long frame's
/// stages are measured by the wall clock too, so a wait (a lock, a file, the device) is named by its
/// stage even when the thread spent no processor time in it.
#[test]
fn t1846_a_long_frame_says_which_stage_waited() {
    use crate::frame_cost::{Stage, last_frame_wall, span};
    let frame = span(Stage::Frame);
    {
        let _timeline = span(Stage::Timeline);
        // a wait, not work: a lock held elsewhere stands for it
        std::thread::sleep(Duration::from_millis(5));
    }
    {
        let _centre = span(Stage::Centre);
    }
    drop(frame);
    let wall = last_frame_wall();
    let ms: u64 = wall
        .split(", ")
        .find_map(|s| s.strip_prefix("timeline "))
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("the waiting stage is not named: {wall:?}"));
    assert!(ms >= 5, "{wall}");
    assert!(!wall.contains("centre"), "a stage under a millisecond is named: {wall}");
    assert!(!wall.contains("frame"), "the frame itself is not a stage of itself: {wall}");
}

/// A one-second stereo 48 kHz PCM WAV: a real file FFmpeg opens.
fn write_wav(path: &std::path::Path) {
    let frames = 48_000u32;
    let data = frames * 4;
    let mut b = Vec::with_capacity(44 + data as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for i in 0..frames {
        let v = (((i as f32) * 0.05).sin() * 8000.0) as i16;
        b.extend_from_slice(&v.to_le_bytes());
        b.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

/// Test t2236 (one-scheduler, rows 98 and 148): the person's media and save gestures make no
/// blocking call on the UI thread — a file brought onto the timeline, the same media placed again
/// from the bin, Undo and Redo of its arrival (the player's source removed and raised again, a
/// decoder joined), and Ctrl+S. The files open, the decoder joins and the write with its fsyncs
/// run on the scheduler's lanes; every gesture still lands.
#[test]
fn t2236_media_and_save_gestures_block_nothing_on_the_ui_thread() {
    use kt_time::Tick;
    let (mut h, _) = crate::layout_tests::harness_three_clips_dt(Some(0.02));
    let dir = std::env::temp_dir().join(format!("kt-t2236-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("tone.wav");
    write_wav(&wav);
    h.step();
    // the harness's frames run on this thread: it is the UI thread (`ui` marks it)
    assert_eq!(kt_time::block::role_here(), kt_time::block::role::UI);
    let before = kt_time::block::here();
    let clips0 = h.state().model.project().clips.len();

    h.state_mut().add_media_clip_at(&wav.to_string_lossy(), Tick(300_000), None);
    h.step();
    let media = h
        .state()
        .model
        .project()
        .media
        .iter()
        .find(|m| m.path.ends_with("tone.wav"))
        .map(|m| m.id)
        .expect("the brought-in file is in the project");
    let clips1 = h.state().model.project().clips.len();
    assert!(clips1 > clips0, "the file's clip did not land");

    h.state_mut().place_media_at(media, Tick(500_000), None);
    h.step();
    assert!(h.state().model.project().clips.len() > clips1, "placing from the bin did not land");

    // undo the placement and the arrival, then redo them: the source goes and comes back
    for _ in 0..2 {
        h.state_mut().undo();
        h.step();
    }
    for _ in 0..2 {
        h.state_mut().redo();
        h.step();
    }
    assert!(h.state().model.project().media.iter().any(|m| m.id == media));

    let path = dir.join("t2236.kt");
    h.state_mut().project_path = Some(path.clone());
    h.state_mut().save();
    h.step();
    assert!(path.exists(), "Ctrl+S wrote nothing");
    assert!(!h.state().dirty_content, "a finished save left the asterisk");

    let after = kt_time::block::here();
    let named: Vec<String> = (0..kt_time::block::kind::COUNT)
        .filter(|&k| after[k] != before[k])
        .map(|k| format!("{} ×{}", kt_time::block::kind::name(k as u32), after[k] - before[k]))
        .collect();
    assert!(named.is_empty(), "the UI thread waited: {named:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test t2215 (test-hygiene, `ui_wait` at load 142): a long frame's note says how long the UI thread stood
/// ready with no processor. `ui_wait` judged a frame's wall time, and at load 142 a frame of 10 ms of
/// work stood 410 ms: the machine, not a lock. The reading grows when the thread competes for the
/// cores, and does not when it sleeps.
#[test]
fn t2215_the_run_queue_wait_is_read_apart_from_a_sleep() {
    use crate::frame_cost::run_delay_ns;
    if !cfg!(target_os = "linux") {
        return;
    }
    let slept0 = run_delay_ns();
    std::thread::sleep(Duration::from_millis(50));
    let slept = run_delay_ns() - slept0;
    // every core busy several times over, and this thread among them
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let spinners: Vec<_> = (0..cores * 4)
        .map(|_| {
            let stop = stop.clone();
            std::thread::spawn(move || {
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    std::hint::spin_loop();
                }
            })
        })
        .collect();
    let q0 = run_delay_ns();
    let deadline = kt_time::test_deadline();
    let mut queued = 0;
    while queued == 0 && std::time::Instant::now() < deadline {
        // work, not a clock: a run of spins the scheduler shares out
        for _ in 0..2_000_000 {
            std::hint::spin_loop();
        }
        queued = run_delay_ns() - q0;
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    for s in spinners {
        s.join().unwrap();
    }
    assert!(queued > 0, "competing for the cores, the thread's run-queue wait did not grow");
    assert!(slept < 50_000_000, "a sleep is not a wait for a processor: {slept} ns");
}

/// Test t2217 (test-hygiene, found-defects 133/170): kept moments still being rechecked after an edit
/// ask the UI for no frame of its own; the preview's thread wakes it after each pass. The timeline
/// asked for the next frame on every frame while the check was pending: on a loaded machine the UI
/// ran ~150 frames a second "at rest" for seconds (live `edit_drag`: 39 frames a quarter second with no
/// input and no work counter), beside the very check it waited for.
#[test]
fn t2217_a_pending_recheck_of_kept_moments_asks_no_frame() {
    let mut h = crate::layout_tests::harness();
    for _ in 0..3 {
        h.step();
    }
    h.state_mut().coverage_cache.final_pending = true;
    h.step();
    h.step();
    let causes: Vec<(&'static str, u32)> = h.ctx.repaint_causes().iter().map(|c| (c.file, c.line)).collect();
    assert!(
        !causes.iter().any(|(f, _)| f.ends_with("timeline.rs")),
        "the timeline asks for frames while kept moments are rechecked: {causes:?}"
    );
}

/// The save's answer is taken in a LATER frame, as in the live program (save-then-act): the harness
/// otherwise waits the test's bound inside `save()`, and a caller acting on the flags right after it
/// is green there and broken in the window. Restored when the test ends, red or green.
struct LiveSaveGrace;

impl LiveSaveGrace {
    fn set() -> LiveSaveGrace {
        crate::app::set_save_grace(Some(Duration::ZERO));
        LiveSaveGrace
    }
}

impl Drop for LiveSaveGrace {
    fn drop(&mut self) {
        crate::app::set_save_grace(None);
    }
}

/// Frames until the save in flight has been taken (landed or failed): a consequence, not a clock.
fn until_save_taken(h: &mut Harness<'static, KtApp>) {
    let deadline = kt_time::test_deadline();
    while h.state().saving.is_some() {
        assert!(std::time::Instant::now() < deadline, "the save never answered");
        std::thread::sleep(Duration::from_millis(1));
        h.step();
    }
    h.step();
}

/// A project of the person's with an unsaved edit at `path` (a medium added, the asterisk up).
fn dirty_named(path: &std::path::Path) -> Harness<'static, KtApp> {
    let mut h = crate::layout_tests::harness();
    let app = h.state_mut();
    app.project_path = Some(path.to_path_buf());
    app.model.apply(Command::AddMedia { id: kt_model::MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    h
}

fn media_on_disk(path: &std::path::Path) -> usize {
    KtApp::open(path).map(|a| a.model.project().media.len()).unwrap_or(0)
}

/// Test t2306 (save-then-act, durability): "Save and continue" before opening another project opens
/// it when the write has LANDED, and the work is in the file. Since one-scheduler the write runs on
/// the save lane and `save()` returns with the asterisk still up: the answer checked the flags right
/// after it, found them up and silently stayed — the person pressed "Save and continue" and nothing
/// went on.
///
/// - **PATH** an edited project → Help › Welcome window… → a Recent card → "Unsaved work" → "Save and
///   continue" → the window closes, the project stays on screen for the moment the write takes, then
///   the chosen project opens; the first one's file holds the edit
#[test]
fn t2306_save_and_continue_goes_on_when_the_write_lands() {
    crate::plugins::install();
    let _grace = LiveSaveGrace::set();
    let dir = std::env::temp_dir().join(format!("kt-t2306-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.kt");
    let b = dir.join("b.kt");
    let text = crate::app::project_text_of(&crate::lessons::lesson_project(), &Default::default(), Some(&dir));
    crate::app::write_project_atomic(&b, &text).unwrap();
    let mut h = dirty_named(&a);
    h.step();
    h.state_mut().start_go(crate::welcome::StartGo::Recent(b.clone()));
    assert_eq!(h.state().ask, Some(crate::app::Ask::Unsaved(crate::app::Pending::Start)));
    h.state_mut().answer_ask(crate::app::Answer::Yes);
    if h.state().saving.is_some() {
        assert_eq!(h.state().project_path.as_deref(), Some(a.as_path()), "the open ran before the write landed");
    }
    until_save_taken(&mut h);
    assert!(h.state().ask.is_none(), "the landed save asked again: {:?}", h.state().ask);
    assert_eq!(h.state().project_path.as_deref(), Some(b.as_path()), "\"Save and continue\" saved and stayed");
    assert_eq!(media_on_disk(&a), 1, "the edit is not in the first project's file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test t2307 (save-then-act, durability): a gesture that leaves the project while its Ctrl+S is
/// still writing waits for that write instead of asking about work already being saved, and goes on
/// when it lands.
///
/// - **PATH** an edited project → Ctrl+S → at once File › New → no "Unsaved work" window; the empty
///   project opens when the write lands, the file holds the edit
#[test]
fn t2307_a_gesture_after_ctrl_s_waits_for_the_write() {
    let _grace = LiveSaveGrace::set();
    let dir = std::env::temp_dir().join(format!("kt-t2307-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.kt");
    let mut h = dirty_named(&a);
    h.step();
    h.state_mut().save();
    h.state_mut().new_project();
    assert!(h.state().ask.is_none(), "asked about the work Ctrl+S is writing: {:?}", h.state().ask);
    until_save_taken(&mut h);
    assert!(h.state().ask.is_none(), "the landed save asked: {:?}", h.state().ask);
    assert!(h.state().project_path.is_none(), "File › New did not go on after the write");
    assert_eq!(media_on_disk(&a), 1, "the edit is not in the file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test t2308 (save-then-act, durability): a write that fails drops what waited for it — the
/// project stays with its asterisk and the warning, whether the action came from "Save and
/// continue" or met a Ctrl+S in flight.
///
/// - **PATH** an edited project whose folder cannot be written → File › New → "Save and continue" →
///   a warning names the failed save, the project and its asterisk stay; Ctrl+S → File › New → the
///   same warning, nothing opens
#[test]
fn t2308_a_failed_write_drops_what_waited_for_it() {
    let _grace = LiveSaveGrace::set();
    let mut h = dirty_named(&crate::layout_tests::unwritable_path("t2308"));
    h.step();
    let warns = |h: &Harness<'static, KtApp>| {
        h.state().notices.iter().filter(|n| n.severity == crate::app::Severity::Warn).map(|n| n.repeats).sum::<u32>()
    };
    for via_ctrl_s in [false, true] {
        let w0 = warns(&h);
        if via_ctrl_s {
            h.state_mut().save();
            h.state_mut().new_project();
        } else {
            h.state_mut().new_project();
            h.state_mut().answer_ask(crate::app::Answer::Yes);
        }
        until_save_taken(&mut h);
        let st = h.state();
        assert!(st.project_path.is_some(), "the failed write let File › New go on (Ctrl+S first: {via_ctrl_s})");
        assert_eq!(st.model.project().media.len(), 1, "the work was discarded after a failed write");
        assert!(st.dirty_content, "the asterisk went out on a failed write");
        assert!(st.after_save.is_none(), "the action still waits after the write failed");
        assert!(warns(&h) > w0, "the failed write said nothing");
    }
}
