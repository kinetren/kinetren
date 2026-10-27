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
