//! A window the window manager never took (found-defects 830, 954). On X11 a program ASKS to be shown:
//! the request goes to the window manager, and one that is starting drops it (openbox, reproduced on an
//! idle machine: a map asked 16–32 ms into its start is lost; on a loaded one that start lasts seconds —
//! a session that starts its programs beside its window manager). The window then exists and draws, and
//! nobody sees it. eframe asks once, after the first frame, so the program looks once more: while its
//! window is a child of the root, unmapped and without `WM_STATE` (nobody manages it), it asks again.

use std::time::Duration;

/// How the window stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    /// A direct child of the root: not inside a frame of the window manager's.
    pub on_root: bool,
    pub mapped: bool,
    /// `WM_STATE` is set: the window manager manages it (an iconified one too).
    pub managed: bool,
}

/// Was the map request lost? (t3726)
pub fn lost(s: State) -> bool {
    s.on_root && !s.mapped && !s.managed
}

/// Between looks, and how many: a window manager's start on a loaded machine, not a poll for life.
const EVERY: Duration = Duration::from_secs(1);
const LOOKS: u32 = 120;

/// Look until the window is taken; called once, after the first frames have been painted.
#[cfg(all(not(test), target_os = "linux"))]
pub fn start() {
    // winit's own choice: Wayland first
    if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty()) || std::env::var_os("WAYLAND_SOCKET").is_some() {
        return;
    }
    let _ = std::thread::Builder::new().name("map-watch".into()).spawn(|| {
        if let Err(e) = watch() {
            crate::journal::log("INFO", &format!("map-watch: {e}"));
        }
    });
}

#[cfg(not(all(not(test), target_os = "linux")))]
pub fn start() {}

#[cfg(all(not(test), target_os = "linux"))]
fn watch() -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, MapState, Window};
    let (c, screen) = x11rb::connect(None)?;
    let root = c.setup().roots[screen].root;
    let atom = |n: &str| -> Result<u32, Box<dyn std::error::Error>> { Ok(c.intern_atom(false, n.as_bytes())?.reply()?.atom) };
    let (pid_atom, wm_state) = (atom("_NET_WM_PID")?, atom("WM_STATE")?);
    let me = std::process::id();
    let mine = |w: Window| -> bool {
        let pid = c.get_property(false, w, pid_atom, AtomEnum::CARDINAL, 0, 1).ok().and_then(|r| r.reply().ok());
        if pid.and_then(|r| r.value32().and_then(|mut v| v.next())) != Some(me) {
            return false;
        }
        let class = c.get_property(false, w, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256).ok().and_then(|r| r.reply().ok());
        class.is_some_and(|r| r.value.split(|b| *b == 0).any(|s| s == crate::app::APP_ID.as_bytes()))
    };
    let mut asked = 0;
    for _ in 0..LOOKS {
        std::thread::sleep(EVERY);
        // only a window on the root can be one nobody took; a framed one is not in the list
        let Some(w) = c.query_tree(root)?.reply()?.children.into_iter().find(|w| mine(*w)) else { break };
        let Ok(attrs) = c.get_window_attributes(w)?.reply() else { continue };
        let managed = c.get_property(false, w, wm_state, AtomEnum::ANY, 0, 1)?.reply().is_ok_and(|r| r.type_ != 0);
        if !lost(State { on_root: true, mapped: attrs.map_state != MapState::UNMAPPED, managed }) {
            break;
        }
        asked += 1;
        c.map_window(w)?;
        c.flush()?;
    }
    if asked > 0 {
        crate::journal::log("WARN", &format!("the window manager did not take the window: asked {asked} more time(s)"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 3726 (found-defects 830): only a window nobody manages is asked for again — one shown, one in a
    /// frame, one the window manager holds iconified is left alone.
    #[test]
    fn t3726_only_a_window_nobody_took_is_asked_for_again() {
        let s = |on_root, mapped, managed| State { on_root, mapped, managed };
        assert!(lost(s(true, false, false)), "asked to be shown and nobody took it");
        assert!(!lost(s(true, true, false)), "shown with no window manager");
        assert!(!lost(s(true, false, true)), "iconified by the person");
        assert!(!lost(s(false, true, true)), "in its frame");
        assert!(!lost(s(false, false, false)));
        assert!(EVERY * LOOKS >= Duration::from_secs(60), "a window manager starting on a loaded machine");
    }
}
