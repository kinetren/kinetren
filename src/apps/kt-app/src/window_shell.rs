//! The window shell (shared-window-shell): EVERY window of the program is built here.
//!
//! Each of the twenty-three windows used to build its own `egui::Window`, and each drifted a
//! little: seven kept egui's collapse arrow by default, four kept a window-level scroll (the
//! morpher window widened by one physical pixel per frame at the owner's zoom 1.1 — kkt item 2),
//! six did not react to Esc (the camera window — ktprompts 26.10.18 item 6). The owner: "I asked
//! that all widgets and windows reuse code and have the widest behaviour".
//!
//! A window site declares only its KIND and, where the kind allows it, its size and place; the
//! shell sets the rest:
//!
//! * the title cross — for tool windows and dialogs, never for progress;
//! * no collapsing, ever (nobody chose the arrow; a folded window still covers the timeline);
//! * NO window-level scroll: the body scrolls in `widgets::window_body` (R.66/R.67 — one scroll
//!   per window); a scroll of the window itself is the width ratchet of kkt item 2;
//! * Esc: the window ON TOP for the person closes, one per press (`escape_action` picks it
//!   from the list of the previous frame, `arm_esc` hands it back, `show` reports it). The
//!   cross and Esc come back to the site as the same `Closed`, so they take one path.
//!
//! The guard is t921: `egui::Window::new`, `Modal::new`, `.vscroll(`, `.hscroll(` and
//! `.collapsible(` stand in this file and nowhere else.

use eframe::egui;

/// What a window is, and so what its cross and Esc mean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A tool window (effects, morpher, camera, lists): the cross and Esc close it and KEEP
    /// its edits — they are already in the model, the session is one undo step.
    Tool,
    /// A dialog: the cross and Esc take the action that loses nothing (Cancel, No, Later).
    Dialog,
    /// A progress window: no cross, and Esc does not touch it — a job of minutes must not die
    /// from a key meant for something else. The reason is written in words (≥ 25 characters).
    Progress { why: &'static str },
}

impl Kind {
    fn has_cross(self) -> bool {
        !matches!(self, Kind::Progress { .. })
    }

    fn takes_esc(self) -> bool {
        !matches!(self, Kind::Progress { .. })
    }
}

/// How the window was closed this frame. The cross and Esc are ONE path at the site: both
/// arrive here, and the site runs the action of its kind (close and keep, or cancel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Closed {
    Cross,
    Esc,
}

/// The result of one `show`: how the window was closed (if it was) and its response (the frame
/// rectangle). The body talks to its site through the variables it captures, as before.
pub(crate) struct Shown {
    pub closed: Option<Closed>,
    pub response: Option<egui::Response>,
}

impl Shown {
    pub fn closed(&self) -> bool {
        self.closed.is_some()
    }
}

/// A window under construction: the id and the title, its kind, and the size and place the
/// site may give. Everything else is the shell's.
pub(crate) struct Shell {
    id: egui::Id,
    title: egui::WidgetText,
    kind: Kind,
    resizable: bool,
    default_size: Option<egui::Vec2>,
    min_size: Option<egui::Vec2>,
    default_width: Option<f32>,
    anchor: Option<(egui::Align2, egui::Vec2)>,
    default_pos: Option<egui::Pos2>,
    probe: Option<&'static str>,
}

impl Shell {
    pub fn new(id: egui::Id, title: impl Into<egui::WidgetText>, kind: Kind) -> Self {
        if let Kind::Progress { why } = kind {
            debug_assert!(why.chars().count() >= 25, "a progress window says why Esc does not touch it");
        }
        Shell {
            id,
            title: title.into(),
            kind,
            resizable: false,
            default_size: None,
            min_size: None,
            default_width: None,
            anchor: None,
            default_pos: None,
            probe: None,
        }
    }

    /// A resizable window: its default size and the minimum the person can shrink it to.
    pub fn size(mut self, default: [f32; 2], min: [f32; 2]) -> Self {
        self.resizable = true;
        self.default_size = Some(default.into());
        self.min_size = Some(min.into());
        self
    }

    /// A resizable window that keeps egui's default height rule.
    pub fn resizable(mut self) -> Self {
        self.resizable = true;
        self
    }

    pub fn default_size(mut self, size: [f32; 2]) -> Self {
        self.default_size = Some(size.into());
        self
    }

    pub fn default_width(mut self, w: f32) -> Self {
        self.default_width = Some(w);
        self
    }

    pub fn anchor(mut self, align: egui::Align2, offset: impl Into<egui::Vec2>) -> Self {
        self.anchor = Some((align, offset.into()));
        self
    }

    pub fn default_pos(mut self, pos: egui::Pos2) -> Self {
        self.default_pos = Some(pos);
        self
    }

    /// The name under which the window's frame is published to live scenarios (R.27).
    pub fn probe(mut self, name: &'static str) -> Self {
        self.probe = Some(name);
        self
    }

    pub fn show<R>(self, ctx: &egui::Context, body: impl FnOnce(&mut egui::Ui) -> R) -> Shown {
        let id = self.id;
        let kind = self.kind;
        let mut open = true;
        let mut w = egui::Window::new(self.title)
            .id(id)
            .collapsible(false)
            .resizable(self.resizable);
        if kind.has_cross() {
            w = w.open(&mut open);
        }
        if let Some(s) = self.default_size {
            w = w.default_size(s);
        }
        if let Some(s) = self.min_size {
            w = w.min_size(s);
        }
        if let Some(x) = self.default_width {
            w = w.default_width(x);
        }
        if let Some((a, o)) = self.anchor {
            w = w.anchor(a, o);
        }
        if let Some(p) = self.default_pos {
            w = w.default_pos(p);
        }
        let r = w.show(ctx, body);
        let esc = take_armed(ctx, id);
        let response = r.map(|r| {
            record(ctx, id, r.response.layer_id, kind.takes_esc());
            if let Some(p) = self.probe {
                crate::widgets::field_probe::note(p, r.response.rect);
            }
            r.response
        });
        let closed = if esc && kind.takes_esc() {
            Some(Closed::Esc)
        } else if !open {
            Some(Closed::Cross)
        } else {
            None
        };
        Shown { closed, response }
    }
}

/// A modal window (the recovery offer, a question): it has no cross, and Esc gives its safe
/// answer — the site maps `Closed::Esc` to Later / Cancel.
pub(crate) fn modal<R>(
    ctx: &egui::Context,
    id: egui::Id,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Shown {
    let r = egui::Modal::new(id).show(ctx, body);
    record(ctx, id, r.response.layer_id, true);
    let esc = take_armed(ctx, id);
    Shown {
        closed: esc.then_some(Closed::Esc),
        response: Some(r.response),
    }
}

/// Windows drawn during one frame, in the order they were shown.
#[derive(Clone, Default)]
struct Frame {
    shown: Vec<(egui::Id, egui::LayerId, bool)>,
    armed: Option<egui::Id>,
}

fn key() -> egui::Id {
    egui::Id::new("kt_window_shell")
}

fn record(ctx: &egui::Context, id: egui::Id, layer: egui::LayerId, esc: bool) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Frame>(key()).shown.push((id, layer, esc)));
}

fn take_armed(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.data_mut(|d| {
        let f = d.get_temp_mut_or_default::<Frame>(key());
        if f.armed == Some(id) {
            f.armed = None;
            true
        } else {
            false
        }
    })
}

/// Start of a frame: the windows of the PREVIOUS frame that take Esc, bottom to top by the layer
/// order the person sees. `escape_action` closes the last one.
pub(crate) fn begin_frame(ctx: &egui::Context) -> Vec<egui::Id> {
    let shown = ctx.data_mut(|d| {
        let f = d.get_temp_mut_or_default::<Frame>(key());
        f.armed = None;
        std::mem::take(&mut f.shown)
    });
    let order: Vec<egui::LayerId> = ctx.memory(|m| m.layer_ids().collect());
    let mut v: Vec<(usize, egui::Id)> = shown
        .into_iter()
        .filter(|(_, _, esc)| *esc)
        .map(|(id, layer, _)| (order.iter().position(|l| *l == layer).unwrap_or(0), id))
        .collect();
    v.sort_by_key(|(pos, _)| *pos);
    v.into_iter().map(|(_, id)| id).collect()
}

/// Hand Esc to the window `escape_action` picked: its `show` this frame reports `Closed::Esc`.
pub(crate) fn arm_esc(ctx: &egui::Context, id: Option<egui::Id>) {
    if let Some(id) = id {
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Frame>(key()).armed = Some(id));
    }
}
