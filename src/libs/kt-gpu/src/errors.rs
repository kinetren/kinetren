//! A GPU error never ends the program (gpu-errors-survive).
//!
//! wgpu reports an error that no error scope caught to the device's uncaptured-error handler, and
//! without one its default handler panics. The owner's editor died that way on a maximize while a
//! region looped (kkt-26.11.15 N1): egui's `Surface::configure` on the UI thread found the queue
//! busy with the preview's present from the preview thread. So:
//! - every device the process creates or receives is `watch`ed: an uncaptured error is counted per
//!   role and written to the program's sink (the first ones in full, then a count), a lost device is
//!   written too — nothing panics;
//! - a frame's own errors are caught by thread-local scopes around it (`scoped`) and become its
//!   `GpuExecError::Validation`;
//! - a `Surface::configure` refused because the queue was busy holds the display engine's frames for
//!   the next UI frames (`hold_display`, `display_frame`), so egui's retry finds the queue idle.

use std::collections::HashMap;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use kt_engine::GpuExecError;

/// Who owns a device, as the log names it.
pub mod role {
    /// The window's device: eframe's, shared with the preview's engine.
    pub const DISPLAY: &str = "display";
    /// A headless device: the offline engine, the shadow, kt-cli, tests.
    pub const HEADLESS: &str = "headless";
    /// An export's device.
    pub const EXPORT: &str = "export";
    /// The compute host.
    pub const COMPUTE: &str = "compute";
    /// The D3D12 compile check.
    pub const D3D12: &str = "d3d12";
}

/// Errors written in full per role; past it every hundredth, with the count.
const FULL: usize = 20;

/// UI frames the display's frames wait after a refused reconfigure: the frame whose paint retries
/// the configure, and the one after it.
const HOLD_FRAMES: u32 = 2;

/// The longest a display frame waits for a hold: the UI may stop ticking (minimized, closing).
const HOLD_MAX: Duration = Duration::from_millis(250);

type Sink = Box<dyn Fn(&str) + Send + Sync>;

static SINK: OnceLock<Sink> = OnceLock::new();
static COUNTS: Mutex<Option<HashMap<&'static str, usize>>> = Mutex::new(None);
static HOLD: (Mutex<u32>, Condvar) = (Mutex::new(0), Condvar::new());

/// Where the error lines go (the editor's log and telemetry journal, kt-cli's stderr). Set once;
/// without it the lines go to stderr.
pub fn set_sink(f: impl Fn(&str) + Send + Sync + 'static) {
    let _ = SINK.set(Box::new(f));
}

pub(crate) fn emit(line: &str) {
    match SINK.get() {
        Some(f) => f(line),
        None => eprintln!("kt-gpu: {line}"),
    }
}

/// Install the handlers on `device`: an uncaptured error and a lost device are written, counted per
/// `role`, and never panic. A device given to `watch` again (a project reopened on the window's
/// device) keeps one handler, the last.
pub fn watch(device: &wgpu::Device, role: &'static str) {
    device.on_uncaptured_error(std::sync::Arc::new(move |e: wgpu::Error| on_error(role, &e)));
    device.set_device_lost_callback(move |reason, message| {
        // a device dropped at exit says so too: not a fault
        if !matches!(reason, wgpu::DeviceLostReason::Destroyed) {
            emit(&format!("{role} device lost ({reason:?}): {}", one_line(&message)));
        }
    });
}

/// What the handler does with an error on a device of `role`. Public for the tests: a
/// `Surface::configure` error cannot be made without a window.
#[doc(hidden)]
pub fn on_error(role: &'static str, e: &wgpu::Error) {
    let (kind, text) = describe(e);
    let n = {
        let mut c = COUNTS.lock().unwrap_or_else(|p| p.into_inner());
        let n = c.get_or_insert_with(HashMap::new).entry(role).or_insert(0);
        *n += 1;
        *n
    };
    if role == role::DISPLAY && text.contains("Surface::configure") {
        hold_display();
    }
    if n <= FULL || n % 100 == 0 {
        emit(&format!("{role} device: {kind} error #{n}: {text}"));
    }
}

/// Uncaptured errors seen on devices of `role` (tests, the diagnostics line).
pub fn errors_seen(role: &str) -> usize {
    COUNTS.lock().unwrap_or_else(|p| p.into_inner()).as_ref().and_then(|c| c.get(role).copied()).unwrap_or(0)
}

fn describe(e: &wgpu::Error) -> (&'static str, String) {
    let kind = match e {
        wgpu::Error::OutOfMemory { .. } => "out-of-memory",
        wgpu::Error::Validation { .. } => "validation",
        wgpu::Error::Internal { .. } => "internal",
    };
    // `Display` carries the description and the chain of causes ("In Surface::configure …")
    let mut text = e.to_string();
    let mut source = std::error::Error::source(e);
    while let Some(s) = source {
        let s_text = s.to_string();
        if !text.contains(&s_text) {
            text.push_str(" / ");
            text.push_str(&s_text);
        }
        source = s.source();
    }
    (kind, one_line(&text))
}

fn one_line(s: &str) -> String {
    let joined: Vec<&str> = s.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let mut t = joined.join(" / ");
    if t.len() > 600 {
        let cut = (0..=600).rev().find(|&i| t.is_char_boundary(i)).unwrap_or(0);
        t.truncate(cut);
        t.push('…');
    }
    t
}

/// The window's surface could not be reconfigured because the queue was busy: the display engine's
/// next frames wait until the UI has painted twice.
fn hold_display() {
    let mut h = HOLD.0.lock().unwrap_or_else(|p| p.into_inner());
    *h = HOLD_FRAMES;
}

/// A UI frame begins (the editor calls it from `ui`): one frame of a hold is spent.
pub fn display_frame() {
    let mut h = HOLD.0.lock().unwrap_or_else(|p| p.into_inner());
    if *h > 0 {
        *h -= 1;
        if *h == 0 {
            HOLD.1.notify_all();
        }
    }
}

/// Whether the display's frames are held (tests).
pub fn display_held() -> bool {
    *HOLD.0.lock().unwrap_or_else(|p| p.into_inner()) > 0
}

/// Work submitted on the window's device waits out a hold, at most `HOLD_MAX`: the display engine's
/// frames, and anything else that submits there off the UI thread. Never the UI thread itself: its
/// paint is the reconfigure being waited for.
pub fn yield_to_display() {
    let h = HOLD.0.lock().unwrap_or_else(|p| p.into_inner());
    if *h == 0 {
        return;
    }
    let _ = HOLD.1.wait_timeout_while(h, HOLD_MAX, |h| *h > 0);
}

/// Run `f` inside thread-local Validation and Internal error scopes of `device`: an error raised in
/// it is `f`'s error, not the process's.
pub(crate) fn scoped<T>(device: &wgpu::Device, f: impl FnOnce() -> Result<T, GpuExecError>) -> Result<T, GpuExecError> {
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let r = f();
    let v = pollster::block_on(validation.pop());
    let i = pollster::block_on(internal.pop());
    match v.or(i) {
        Some(e) => Err(GpuExecError::Validation(describe(&e).1)),
        None => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validation(text: &str) -> wgpu::Error {
        wgpu::Error::Validation { source: text.to_string().into(), description: text.to_string() }
    }

    /// t2722 (gpu-errors-survive): a `Surface::configure` refused on the display device holds the
    /// display's frames for two UI frames; another error, or one on another device, holds nothing; a
    /// held frame waits out `HOLD_MAX` when the UI stops ticking, and then goes on.
    #[test]
    fn t2722_a_refused_reconfigure_holds_the_display_for_two_ui_frames() {
        // the hold is process-wide: this test alone touches it in this binary
        on_error(role::HEADLESS, &validation("In Surface::configure\n  Failed to wait for GPU to come idle"));
        assert!(!display_held(), "a headless device has no surface");
        on_error(role::DISPLAY, &validation("In Device::create_buffer\n  bad usage"));
        assert!(!display_held(), "only a refused reconfigure holds");
        on_error(role::DISPLAY, &validation("In Surface::configure\n  Failed to wait for GPU to come idle before reconfiguring the Surface"));
        assert!(display_held());
        display_frame();
        assert!(display_held(), "the paint that retries the configure is still held");
        display_frame();
        assert!(!display_held());
        display_frame();
        assert!(!display_held(), "a tick without a hold does nothing");
        // a stalled UI: the wait ends at its bound
        on_error(role::DISPLAY, &validation("In Surface::configure\n  Failed to wait for GPU to come idle"));
        let t = std::time::Instant::now();
        yield_to_display();
        assert!(t.elapsed() >= HOLD_MAX, "the held frame waited its bound: {:?}", t.elapsed());
        display_frame();
        display_frame();
        assert!(errors_seen(role::DISPLAY) >= 3);
    }

    /// t2723: the line names the role, the kind and the whole cause chain on one line, and is cut.
    #[test]
    fn t2723_an_error_line_carries_the_cause_on_one_line() {
        let (kind, text) = describe(&validation("In Surface::configure\n    Failed to wait for GPU to come idle"));
        assert_eq!(kind, "validation");
        assert!(text.contains("Surface::configure") && text.contains("come idle") && !text.contains('\n'), "{text}");
        let long = one_line(&"й".repeat(1000));
        assert!(long.len() <= 604 && long.ends_with('…'));
    }
}
