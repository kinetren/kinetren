//! The window frame — ONE decision (wine-frame-and-dark-theme, capability ui-timeline).
//!
//! Borderless with our own title row (logo, menus, window buttons — R.6) is the program's look
//! on Linux and on real Windows. Under wine the window manager decorates every window, a
//! borderless one too, and its frame covered our 22 px row and cut the status bar (the owner,
//! 2026-10-26): under wine the program asks for the system frame. `KT_DECORATIONS=1` asks for
//! it anywhere (insurance against WM quirks, as since R.6); `KT_DECORATIONS=0` keeps the
//! borderless look even under wine (for wine prefixes with `Decorated=N`).
//!
//! Everything that depends on the frame — the window builder, the window buttons, the drag
//! strip, the resize edges — reads `KtApp::window_frame`, set from [`current`] once.

/// The frame of the main window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// Our own title row with the window buttons; resize from our edges.
    Borderless,
    /// The system's title bar and edges; our row keeps the logo and the menus.
    System,
}

/// The rule (t863): the variable wins when it says `0` or `1`, otherwise wine decides.
pub fn decide(kt_decorations: Option<&str>, under_wine: bool) -> Frame {
    match kt_decorations.map(str::trim) {
        Some("0") => Frame::Borderless,
        Some(v) if !v.is_empty() => Frame::System,
        _ if under_wine => Frame::System,
        _ => Frame::Borderless,
    }
}

/// The decision for this process (computed once).
pub fn current() -> Frame {
    static FRAME: std::sync::OnceLock<Frame> = std::sync::OnceLock::new();
    *FRAME.get_or_init(|| decide(std::env::var("KT_DECORATIONS").ok().as_deref(), under_wine()))
}

/// Running under wine? wine's `ntdll` exports `wine_get_version`, Windows' does not — the
/// standard check; no process, no registry.
#[cfg(windows)]
pub fn under_wine() -> bool {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    let name: Vec<u16> = "ntdll.dll".encode_utf16().chain([0]).collect();
    let ntdll = unsafe { GetModuleHandleW(name.as_ptr()) };
    if ntdll.is_null() {
        return false;
    }
    unsafe { GetProcAddress(ntdll, c"wine_get_version".as_ptr().cast()) }.is_some()
}

#[cfg(not(windows))]
pub fn under_wine() -> bool {
    false
}

/// The main window's last size that was not maximized, kept between runs (egui's saved memory).
pub fn normal_size(ctx: &eframe::egui::Context) -> Option<eframe::egui::Vec2> {
    ctx.data_mut(|d| d.get_persisted::<[f32; 2]>(normal_id())).map(|[w, h]| eframe::egui::vec2(w, h))
}

fn normal_id() -> eframe::egui::Id {
    eframe::egui::Id::new("kt-window-normal-size")
}

/// Is `size` a normal window's, not one filling the monitor? (A window that started maximized reads
/// so for its first frames.)
pub fn is_normal(size: eframe::egui::Vec2, monitor: Option<eframe::egui::Vec2>) -> bool {
    monitor.is_some_and(|m| m.x > 0.0 && m.y > 0.0 && (size.x < 0.9 * m.x || size.y < 0.85 * m.y))
}

/// The size a maximized window goes back to (`v26.10` T5, t3937): the remembered normal one, or
/// 80 % of the monitor.
pub fn restored_size(remembered: Option<eframe::egui::Vec2>, monitor: eframe::egui::Vec2) -> eframe::egui::Vec2 {
    remembered.filter(|s| is_normal(*s, Some(monitor))).unwrap_or(monitor * 0.8)
}

/// Frames after which the window is on screen and its state is the window manager's.
pub const STARTED: u8 = 3;

/// A window that came up over the whole monitor and is NOT maximized: eframe saved a maximized window's
/// state and size, and winit asks for the state before the window is mapped, which the window manager
/// drops (X11) — so it looked maximized, moved at full size and "restored" to nothing (`v26.10` T5). It
/// is given its normal size and then maximized in truth: the window manager knows both.
pub fn adopt_start(ctx: &eframe::egui::Context) {
    use eframe::egui::ViewportCommand as V;
    let (size, v) = ctx.input(|i| (i.content_rect().size(), i.viewport().clone()));
    let Some(monitor) = v.monitor_size else { return };
    if v.maximized != Some(false) || v.fullscreen.unwrap_or(false) || is_normal(size, Some(monitor)) {
        return;
    }
    let normal = restored_size(normal_size(ctx), monitor);
    ctx.send_viewport_cmd(V::InnerSize(normal));
    ctx.send_viewport_cmd(V::OuterPosition(((monitor - normal) * 0.5).to_pos2()));
    ctx.send_viewport_cmd(V::Maximized(true));
}

/// Note the window's size when it is a normal one; `true` — it is. Writes only a changed size.
pub fn note_normal(ctx: &eframe::egui::Context) -> bool {
    let (size, monitor, full) = ctx.input(|i| {
        let v = i.viewport();
        (i.content_rect().size(), v.monitor_size, v.maximized.unwrap_or(false) || v.fullscreen.unwrap_or(false))
    });
    if full || !is_normal(size, monitor) {
        return false;
    }
    if normal_size(ctx) != Some(size) {
        ctx.data_mut(|d| d.insert_persisted(normal_id(), [size.x, size.y]));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t863, the pure part: the four environments of the spec.
    #[test]
    fn t863_frame_decision_per_environment() {
        assert_eq!(decide(None, false), Frame::Borderless, "Linux / real Windows");
        assert_eq!(decide(None, true), Frame::System, "under wine");
        assert_eq!(decide(Some("1"), false), Frame::System, "KT_DECORATIONS=1");
        assert_eq!(decide(Some("0"), true), Frame::Borderless, "KT_DECORATIONS=0 under wine");
        // the former meaning stays: any non-empty value other than 0 asks for the system frame
        assert_eq!(decide(Some("yes"), false), Frame::System);
        assert_eq!(decide(Some(""), true), Frame::System, "an empty value is not set");
    }
}
