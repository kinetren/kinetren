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
//! * EVERY window is resizable and its body fills it (windows-fill-their-area, kkt-26.11.01
//!   item 12: "you look through a needle's eye on a big window … and so everywhere"): a site gives
//!   its size and minimum, or takes its kind's; a window opened per object remembers the size its
//!   FAMILY was last given (`remember`), a single window keeps egui's persisted resize state;
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

    /// The width and the minimum size of a window whose site gives none; its height fits its
    /// content when it first appears (see `show`), and the person can pull it larger after.
    fn default_size(self) -> (f32, egui::Vec2) {
        match self {
            Kind::Tool => (520.0, egui::vec2(360.0, 200.0)),
            Kind::Dialog | Kind::Progress { .. } => (420.0, egui::vec2(280.0, 60.0)),
        }
    }
}

/// Where a window family's last size lives (persisted egui memory: eframe writes it to `app.ron`).
fn family_id(family: &'static str) -> egui::Id {
    egui::Id::new(("kt_win_size", family))
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
    default_size: Option<egui::Vec2>,
    min_size: Option<egui::Vec2>,
    default_width: Option<f32>,
    anchor: Option<(egui::Align2, egui::Vec2)>,
    default_pos: Option<egui::Pos2>,
    probe: Option<&'static str>,
    family: Option<&'static str>,
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
            default_size: None,
            min_size: None,
            default_width: None,
            anchor: None,
            default_pos: None,
            probe: None,
            family: None,
        }
    }

    /// The window's default size and the minimum the person can shrink it to. Every window is
    /// resizable; a site without a size takes its kind's width and fits its content's height.
    pub fn size(mut self, default: [f32; 2], min: [f32; 2]) -> Self {
        self.default_size = Some(default.into());
        self.min_size = Some(min.into());
        self
    }

    pub fn default_size(mut self, size: [f32; 2]) -> Self {
        self.default_size = Some(size.into());
        self
    }

    /// The default width and the minimum size; the height fits the content when it first appears.
    /// The minimum height stays UNDER the content's, or the window would open fitted and then
    /// jump to its minimum.
    pub fn width(mut self, w: f32, min: [f32; 2]) -> Self {
        self.default_width = Some(w);
        self.min_size = Some(min.into());
        self
    }

    /// A window opened per object (an event's effects, a morpher, a transition…) opens at the
    /// size its FAMILY was last given: egui keeps a size per window id, and these ids are per
    /// object, so without this every new event's window came back small (design D3).
    pub fn remember(mut self, family: &'static str) -> Self {
        self.family = Some(family);
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
        let family = self.family.map(family_id);
        let remembered = family.and_then(|f| ctx.data_mut(|d| d.get_persisted::<egui::Vec2>(f)));
        let (kind_default, kind_min) = kind.default_size();
        let min_size = self.min_size.unwrap_or(kind_min);
        // A window whose site gives no height FITS its content when it first appears: its
        // default height is the minimum, and for a few frames the body is content-sized (egui
        // raises a resizable window to its content). From then on the body fills whatever size
        // the window has — the person's (design D2). A window egui already knows (its area state
        // lives in memory, persisted across runs) or whose family has a size does not refit.
        let fresh = egui::AreaState::load(ctx, id).is_none() && remembered.is_none();
        // counted in FRAMES, not passes: egui's sizing pass asks for a discard, so one frame can
        // run the window twice, and a pass count ended the fit before the window had its size
        let fit_id = id.with("kt_fit");
        let frame = ctx.cumulative_frame_nr();
        if fresh && self.default_size.is_none() {
            ctx.data_mut(|d| d.insert_temp(fit_id, frame));
        }
        let fitting = ctx
            .data(|d| d.get_temp::<u64>(fit_id))
            .is_some_and(|first| frame < first + FIT_FRAMES);
        // the frame this window was first drawn in this run: its first frame is egui's sizing
        // pass, whose content size must not overwrite the family's remembered size
        let seen = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id.with("kt_seen"), || frame));
        let settled = frame >= seen + 2;
        let default_size = remembered.or(self.default_size).unwrap_or_else(|| {
            egui::vec2(self.default_width.unwrap_or(kind_default), min_size.y)
        });
        let opened = ctx.data_mut(|d| {
            let f = d.get_temp_mut_or_default::<Frame>(key());
            if f.open.iter().any(|(o, _)| *o == id) {
                return None;
            }
            let title = self.title.text().to_string();
            f.open.push((id, title.clone()));
            Some(title)
        });
        if let Some(t) = opened {
            crate::tele_journal::record(crate::tele_journal::Kind::Act, &format!("window opened: {t}"));
        }
        let mut open = true;
        let mut w = egui::Window::new(self.title)
            .id(id)
            .collapsible(false)
            .resizable(true)
            .default_size(default_size)
            .min_size(min_size);
        if kind.has_cross() {
            w = w.open(&mut open);
        }
        if let Some((a, o)) = self.anchor {
            w = w.anchor(a, o);
        }
        if let Some(p) = self.default_pos {
            w = w.default_pos(p);
        }
        let r = w.show(ctx, |ui| {
            FITTING.set(fitting);
            let r = body(ui);
            FITTING.set(false);
            crate::widgets::fill_probe::measure_tail(ui);
            r
        });
        // The family keeps the OUTER size (egui's `default_size` is the outer footprint, read from
        // the area's state: the show's response keeps the first frame's). It is written when THIS
        // window's size changed — the person resized it: two windows of one family open at once
        // must not overwrite each other every frame.
        if let (Some(f), Some(area)) = (family, egui::AreaState::load(ctx, id))
            && r.is_some()
            && settled
        {
            let size = area.rect().size().round();
            let changed = ctx.data_mut(|d| {
                let last = d.get_temp_mut_or_insert_with(id.with("kt_size"), || size);
                std::mem::replace(last, size) != size
            });
            let kept = ctx.data_mut(|d| d.get_persisted::<egui::Vec2>(f));
            if changed || kept.is_none() {
                ctx.data_mut(|d| d.insert_persisted(f, size));
            }
        }
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
    // a modal cannot be resized: its body must not fill (it would take the whole screen) —
    // `window_body` reads this flag and keeps the content-sized, screen-capped body
    let r = egui::Modal::new(id).show(ctx, |ui| {
        IN_MODAL.set(true);
        let r = body(ui);
        IN_MODAL.set(false);
        r
    });
    record(ctx, id, r.response.layer_id, true);
    let esc = take_armed(ctx, id);
    Shown {
        closed: esc.then_some(Closed::Esc),
        response: Some(r.response),
    }
}

/// How many frames a fresh window's body stays content-sized: the first frame of a window is
/// egui's invisible sizing pass, the second raises the window to the content. Content that
/// arrives later (a poster, a section filled in the background) goes into the scroll, it does not
/// stretch an open window under the person's hand (t918).
const FIT_FRAMES: u64 = 2;


thread_local! {
    static IN_MODAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FITTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Is the body being drawn inside a modal (which cannot be resized)?
pub(crate) fn in_modal() -> bool {
    IN_MODAL.get()
}

/// Is a fresh window still fitting its content (its body content-sized, not filling)?
pub(crate) fn fitting() -> bool {
    FITTING.get()
}

/// Windows drawn during one frame, in the order they were shown.
#[derive(Clone, Default)]
struct Frame {
    shown: Vec<(egui::Id, egui::LayerId, bool)>,
    armed: Option<egui::Id>,
    /// The windows open as the telemetry journal knows them, with their titles: a window that
    /// appears is journaled once, one that a frame no longer draws is journaled closed
    /// (telemetry-journal). Allocates only when a window opens.
    open: Vec<(egui::Id, String)>,
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
    let (shown, closed) = ctx.data_mut(|d| {
        let f = d.get_temp_mut_or_default::<Frame>(key());
        f.armed = None;
        let shown = std::mem::take(&mut f.shown);
        let mut closed = Vec::new();
        f.open.retain(|(id, title)| {
            let still = shown.iter().any(|(s, _, _)| s == id);
            if !still {
                closed.push(title.clone());
            }
            still
        });
        (shown, closed)
    });
    for t in closed {
        crate::tele_journal::record(crate::tele_journal::Kind::Act, &format!("window closed: {t}"));
    }
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
