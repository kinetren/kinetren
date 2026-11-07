//! Waking the UI from a background thread (idle-cpu).
//!
//! The program at rest draws nothing, so nothing polls. A thread that has something to show says
//! so here; a clock the UI itself runs on is `KtApp::rest_wake`. Egui keeps the earliest repaint it
//! was asked for, so a storm of `soon` is one frame per [`SOON`].

use eframe::egui;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// What a stream of small results waits for: a thumbnail storm must not become a frame storm.
pub const SOON: Duration = Duration::from_millis(120);

static CTX: OnceLock<egui::Context> = OnceLock::new();
/// Every wake so far: something a frame may show has changed behind its back.
static NEWS: AtomicU64 = AtomicU64::new(0);

/// The count of wakes: a frame that kept something from an earlier frame asks this first.
pub fn news() -> u64 {
    // the harness's apps share a process, and another test's thread is no news to this one
    if cfg!(test) { 0 } else { NEWS.load(Ordering::Relaxed) }
}

/// One context for the life of the process (`app::run`); the harness has none and steps by itself.
pub fn install(ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
}

/// A result a person is waiting for.
pub fn now() {
    NEWS.fetch_add(1, Ordering::Relaxed);
    if let Some(c) = CTX.get() {
        c.request_repaint();
    }
}

/// A frame after `d`, with no news.
pub fn later(d: Duration) {
    if let Some(c) = CTX.get() {
        c.request_repaint_after(d);
    }
}

/// One of many small results.
pub fn soon() {
    NEWS.fetch_add(1, Ordering::Relaxed);
    if let Some(c) = CTX.get() {
        c.request_repaint_after(SOON);
    }
}
