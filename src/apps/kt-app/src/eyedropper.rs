//! The colour eyedropper (chroma-key-eyedropper, kkt-26.11.01b B2): "greenscreen не имеет пипетки,
//! чтобы ткнуть в тот же кадр превью или еще где-либо взять цвет с экрана".
//!
//! Every colour row (`widgets::color_row`, `color_row_rgba`) carries a pipette button. A click arms
//! it; while armed, one transparent layer over the whole window takes the next click, so the click
//! acts on nothing under it (a click on the preview does not toggle playback), reads the pixel of the
//! SCREEN under the pointer and hands it to the row that armed it on the next frame. Esc cancels.
//! The screen, not the composite: what the person points at is what they see — the preview's frame,
//! a bin thumbnail, a swatch in another window of the program.
//!
//! The rows are drawn inside window closures with no `&mut KtApp`: the arming and the answer go
//! through thread-locals, as `hint_sink` and `field_probe` do (only the UI thread touches them).

use eframe::egui;
use std::cell::Cell;

thread_local! {
    static ARMED: Cell<Option<egui::Id>> = const { Cell::new(None) };
    static PICKED: Cell<Option<(egui::Id, [u8; 3])>> = const { Cell::new(None) };
}

/// Whether a row is waiting for a pick.
pub fn armed() -> bool {
    ARMED.with(|a| a.get().is_some())
}

/// Esc, or the same button again.
pub fn cancel() -> bool {
    ARMED.with(|a| a.take().is_some())
}

/// The pipette button of the colour row `id`: a click arms (again — disarms); returns the colour
/// picked for this row since the last frame.
pub fn button(ui: &mut egui::Ui, id: egui::Id, probe: Option<&str>) -> Option<[u8; 3]> {
    let on = ARMED.with(|a| a.get() == Some(id));
    let r = ui.add(egui::Button::selectable(on, crate::icons::EYEDROPPER).frame_when_inactive(true));
    // the hint goes to the status bar, the one place of hints (rule 10)
    if r.hovered() {
        crate::widgets::hint_sink::put(crate::i18n::tr(crate::i18n::k::fx::EYEDROP_HINT));
    }
    if let Some(name) = probe {
        crate::widgets::field_probe::note(name, r.rect);
    }
    if r.clicked() {
        ARMED.with(|a| a.set(if on { None } else { Some(id) }));
    }
    PICKED.with(|p| match p.get() {
        Some((pid, rgb)) if pid == id => {
            p.set(None);
            Some(rgb)
        }
        _ => None,
    })
}

/// The pick layer, drawn once a frame by the app while a row is armed: the crosshair, the line in
/// the status bar, and the click. `Err(())` — the screen cannot be read here (Wayland).
pub fn overlay(ctx: &egui::Context) -> Option<Result<(), ()>> {
    let id = ARMED.with(|a| a.get())?;
    let rect = ctx.content_rect();
    let mut out = None;
    egui::Area::new(egui::Id::new("eyedropper"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let r = ui.allocate_rect(egui::Rect::from_min_size(rect.min, rect.size()), egui::Sense::click());
            crate::widgets::field_probe::note("eyedrop.layer", r.rect);
            ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            if r.clicked()
                && let Some(pos) = r.interact_pointer_pos()
            {
                ARMED.with(|a| a.set(None));
                out = Some(match screen_pixel(ctx, pos) {
                    Some(rgb) => {
                        PICKED.with(|p| p.set(Some((id, rgb))));
                        ctx.request_repaint();
                        Ok(())
                    }
                    None => Err(()),
                });
            }
        });
    out.or(Some(Ok(())))
}

/// The pixel of the screen under `pos` (window points).
#[cfg(test)]
fn screen_pixel(_ctx: &egui::Context, pos: egui::Pos2) -> Option<[u8; 3]> {
    testing::SCREEN.with(|s| s.borrow().as_ref().and_then(|f| f(pos)))
}

#[cfg(all(not(test), target_os = "linux"))]
fn screen_pixel(_ctx: &egui::Context, _pos: egui::Pos2) -> Option<[u8; 3]> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
    // the pointer's own root position: no mapping of window points to the screen to get wrong
    let (c, screen) = x11rb::connect(None).ok()?;
    let root = c.setup().roots[screen].root;
    let p = c.query_pointer(root).ok()?.reply().ok()?;
    let img = c.get_image(ImageFormat::Z_PIXMAP, root, p.root_x, p.root_y, 1, 1, !0).ok()?.reply().ok()?;
    // a 24/32-bit TrueColor visual stores B, G, R, (pad) — the X server's own order on x86
    match img.data.as_slice() {
        [b, g, r, ..] if img.depth >= 24 => Some([*r, *g, *b]),
        _ => None,
    }
}

#[cfg(all(not(test), windows))]
fn screen_pixel(_ctx: &egui::Context, _pos: egui::Pos2) -> Option<[u8; 3]> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{CLR_INVALID, GetDC, GetPixel, ReleaseDC};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut pt = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut pt) } == 0 {
        return None;
    }
    let dc = unsafe { GetDC(std::ptr::null_mut()) };
    let c = unsafe { GetPixel(dc, pt.x, pt.y) };
    unsafe { ReleaseDC(std::ptr::null_mut(), dc) };
    (c != CLR_INVALID).then(|| [(c & 0xff) as u8, ((c >> 8) & 0xff) as u8, ((c >> 16) & 0xff) as u8])
}

#[cfg(all(not(test), not(target_os = "linux"), not(windows)))]
fn screen_pixel(_ctx: &egui::Context, _pos: egui::Pos2) -> Option<[u8; 3]> {
    None
}

#[cfg(test)]
pub mod testing {
    use eframe::egui;
    use std::cell::RefCell;

    type Screen = Box<dyn Fn(egui::Pos2) -> Option<[u8; 3]>>;

    thread_local! {
        pub(super) static SCREEN: RefCell<Option<Screen>> = RefCell::new(None);
    }

    /// The harness has no screen: a test says what the pixel under a point is.
    pub fn set_screen(f: impl Fn(egui::Pos2) -> Option<[u8; 3]> + 'static) {
        SCREEN.with(|s| *s.borrow_mut() = Some(Box::new(f)));
    }
}
