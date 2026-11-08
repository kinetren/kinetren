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
