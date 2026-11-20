//! A click's count kept to ONE gesture (found-defects 817).
//!
//! egui counts a click as a triple when the click two before it fell within twice the double-click
//! window, wherever that click was: only the distance to the LAST click is checked. The program
//! widens that window by its busy frame (`KeyClock::widen_clicks`, up to 1.3 s, so a triple spans
//! 2.6 s), and under load a click on the effects window's "Done" 2.5 s before a double click on an
//! event made that double click a triple — no window opened, a region was set over the event and
//! the cursor jumped to its start. egui's `double_clicked()` is false on a count of three, so every
//! double-click gesture of the program lost its double the same way.
//!
//! Here a triple is a triple only when the click before it was itself a double; otherwise the click
//! is a double when it follows the click before within the window, as egui judges a double. Every
//! double and triple click in the program is read through [`double`] and [`triple`].

use eframe::egui;
use std::cell::Cell;

#[derive(Clone, Copy)]
struct St {
    /// The effective count of the last primary click, its time and place.
    count: u32,
    at: f64,
    pos: Option<egui::Pos2>,
    /// The effective count of this frame's primary click (0 — none).
    now: u32,
}

thread_local! {
    static ST: Cell<St> = const { Cell::new(St { count: 0, at: f64::NEG_INFINITY, pos: None, now: 0 }) };
}

/// Read this frame's primary click once, before any widget asks (`draw_all`).
pub fn begin_frame(ctx: &egui::Context) {
    use egui::PointerButton::Primary;
    let (delay, dist) =
        ctx.options(|o| (o.input_options.max_double_click_delay, o.input_options.max_click_dist));
    let (clicked, egui_count, time, pos) = ctx.input(|i| {
        let p = &i.pointer;
        let count = if p.button_triple_clicked(Primary) {
            3
        } else if p.button_double_clicked(Primary) {
            2
        } else {
            1
        };
        (p.button_clicked(Primary), count, i.time, p.interact_pos())
    });
    ST.with(|s| {
        let mut st = s.get();
        if !clicked {
            st.now = 0;
            s.set(st);
            return;
        }
        let near = match (st.pos, pos) {
            (Some(a), Some(b)) => a.distance(b) < dist,
            _ => true,
        };
        let follows = time - st.at < delay && near;
        st.now = effective(egui_count, st.count, follows);
        let since_us = if st.at.is_finite() { ((time - st.at) * 1e6).max(0.0) as u64 } else { u64::MAX };
        kt_time::trace::ev(kt_time::trace::Ev::Click)
            .a(egui_count as u64)
            .b(st.now as u64)
            .c(since_us)
            .aux((delay * 1000.0) as u32)
            .emit();
        st.count = st.now;
        st.at = time;
        st.pos = pos;
        s.set(st);
    });
}

/// The count of a click from egui's count, the count of the click before and whether this click
/// follows that one within the double-click window: pure, for t3130.
pub fn effective(egui_count: u32, before: u32, follows: bool) -> u32 {
    match egui_count {
        3 if before == 2 => 3,
        3 if follows => 2,
        3 => 1,
        n => n,
    }
}

fn now() -> u32 {
    ST.with(|s| s.get().now)
}

/// The widget was double clicked: egui's double, or egui's triple whose first click belonged to
/// another gesture.
pub fn double(r: &egui::Response) -> bool {
    r.double_clicked() || (r.triple_clicked() && now() == 2)
}

/// The widget was triple clicked: three clicks of one gesture.
pub fn triple(r: &egui::Response) -> bool {
    r.triple_clicked() && now() == 3
}
