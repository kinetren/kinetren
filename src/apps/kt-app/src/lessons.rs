//! The tour: lessons in their own view (welcome-window-and-tour D2, D7, D8; capability
//! `ui-reach`).
//!
//! The owner's request of 2026-10-28 revised F-HLP-028: of the three forms its reconnaissance
//! looked at, the lesson that CHECKS the done action in its own view (Ableton Learn View) is
//! taken; a coach mark over the interface stays rejected. So:
//!
//! * the lessons live in a docked side panel (`Panel::right`) — it narrows the editor, it does
//!   not cover it, and it adds no layer above the window layer (t988,
//!   `no_teaching_layer_over_the_ui`);
//! * a step closes by ITSELF on a predicate over the project's state and the program's
//!   observable state — never on "Next", never on a command registry (F-UI-002). Predicates
//!   look at the state, not the gesture, so every door counts (a cut by S or by the menu);
//! * a predicate is re-evaluated only when the model revision, the open-window set or the
//!   render result changes — never per frame at rest (invariant 12);
//! * "Show" opens or focuses the step's target: a window of the `WINDOWS` registry, a menu path
//!   of catalog keys, or a panel. The target draws a STATIC accent outline itself while the step
//!   is current (the owner, 2026-10-28: "yes, it may draw") — no layer, no pulsation, no dimming;
//! * every lesson runs on a program-owned COPY of the demo project made of generators, in
//!   `<store>/lessons/` — never in the person's folders, never in Recent (D8).

use crate::i18n::{Key, k, tr};
use crate::user_store::TourProgress;
use eframe::egui;
use kt_model::Project;
use kt_time::Tick;

/// The panels a step may point at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Timeline,
}

/// What "Show" reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// An id of the `WINDOWS` registry.
    Window(&'static str),
    /// A menu path of catalog keys: the root title, then the item.
    Menu(&'static [Key]),
    Panel(Panel),
}

/// When a step is done — a closed set, evaluated by one pure function (`done`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Predicate {
    /// An event of a generator of this tag (`text` — a title).
    HasGenerator(&'static str),
    /// Two video events meet at the marker of this name on one track: the event was cut there.
    SplitAtMarker(&'static str),
    /// The event of this id no longer stands where the demo put it (seconds, track).
    ClipMoved { id: u64, start_s: i64, lane: u32 },
    /// A region exists and the loop is on.
    LoopRegionSet,
    /// A window of the registry is open.
    WindowOpen(&'static str),
    /// An event's chain holds an effect of this tag.
    ChainHas(&'static str),
    /// An event's chain holds an effect of this tag with a curve on it.
    ChainCurve(&'static str),
    /// A transition between two video events.
    VideoTransition,
    /// A video transition of a kind other than the crossfade.
    TransitionKindChanged,
    /// A morpher whose batch holds at least this many effects.
    MorpherFx(usize),
    /// A render finished in this session.
    RenderDone,
}

/// One step: its paragraph (a catalog key, ≤ 160 characters ×5), "Show" and the predicate.
#[derive(Debug, Clone, Copy)]
pub struct Step {
    pub text: Key,
    pub show: Option<Target>,
    pub done: Predicate,
}

/// One lesson.
#[derive(Debug, Clone, Copy)]
pub struct Lesson {
    pub title: Key,
    pub steps: &'static [Step],
}

/// The marker the cut step aims at (the lesson copy adds it in the middle of the first event).
pub const CUT_MARKER: &str = "CUT HERE";

const FILE_GENERATOR: &[Key] = &[k::menu::file::TITLE, k::bin::menu::NEW_GENERATOR];
const FILE_RENDER: &[Key] = &[k::menu::file::TITLE, k::menu::file::RENDER];

/// The lessons (design D7, open question 4: as recommended).
pub const LESSONS: &[Lesson] = &[
    Lesson {
        title: k::lessons::L1,
        steps: &[
            Step {
                text: k::lessons::L1_S1,
                show: Some(Target::Menu(FILE_GENERATOR)),
                done: Predicate::HasGenerator("text"),
            },
            Step {
                text: k::lessons::L1_S2,
                show: Some(Target::Panel(Panel::Timeline)),
                done: Predicate::SplitAtMarker(CUT_MARKER),
            },
            Step {
                text: k::lessons::L1_S3,
                show: Some(Target::Panel(Panel::Timeline)),
                done: Predicate::ClipMoved { id: 7, start_s: 48, lane: 0 },
            },
            Step {
                text: k::lessons::L1_S4,
                show: Some(Target::Panel(Panel::Timeline)),
                done: Predicate::LoopRegionSet,
            },
        ],
    },
    Lesson {
        title: k::lessons::L2,
        steps: &[
            Step { text: k::lessons::L2_S1, show: Some(Target::Window("fx")), done: Predicate::WindowOpen("fx") },
            Step { text: k::lessons::L2_S2, show: Some(Target::Window("fx")), done: Predicate::ChainHas("blur") },
            Step { text: k::lessons::L2_S3, show: Some(Target::Window("fx")), done: Predicate::ChainCurve("blur") },
        ],
    },
    Lesson {
        title: k::lessons::L3,
        steps: &[
            Step {
                text: k::lessons::L3_S1,
                show: Some(Target::Panel(Panel::Timeline)),
                done: Predicate::VideoTransition,
            },
            Step {
                text: k::lessons::L3_S2,
                show: Some(Target::Panel(Panel::Timeline)),
                done: Predicate::TransitionKindChanged,
            },
        ],
    },
    Lesson {
        title: k::lessons::L4,
        steps: &[
            Step { text: k::lessons::L4_S1, show: Some(Target::Window("morph")), done: Predicate::WindowOpen("morph") },
            Step { text: k::lessons::L4_S2, show: Some(Target::Window("morph")), done: Predicate::MorpherFx(2) },
        ],
    },
    Lesson {
        title: k::lessons::L5,
        steps: &[
            Step {
                text: k::lessons::L5_S1,
                show: Some(Target::Menu(FILE_RENDER)),
                done: Predicate::WindowOpen("kt_render_settings"),
            },
            Step {
                text: k::lessons::L5_S2,
                show: Some(Target::Window("kt_render_settings")),
                done: Predicate::RenderDone,
            },
        ],
    },
];

/// The windows whose openness the program observes (`KtApp::open_window_ids`). A `WindowOpen`
/// of any other id could never close — the guard (t988) refuses it.
#[cfg_attr(not(test), allow(dead_code))]
pub const OBSERVED_WINDOWS: &[&str] = &["fx", "morph", "kt_render_settings", "kt_render_progress"];

/// The observable state a predicate may read besides the project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Observed {
    pub open: Vec<&'static str>,
    pub render_done: bool,
}

fn video_meet_at(p: &Project, at: Tick) -> bool {
    use kt_model::ClipKind;
    p.clips.iter().filter(|a| a.kind == ClipKind::Video && a.start + a.duration == at).any(|a| {
        p.clips.iter().any(|b| b.kind == ClipKind::Video && b.lane == a.lane && b.start == at && b.id != a.id)
    })
}

/// Is the step done? Pure: the project and the observed state, nothing else.
pub fn done(pr: &Predicate, p: &Project, seen: &Observed) -> bool {
    use kt_model::{ClipKind, TransitionKind};
    match *pr {
        Predicate::HasGenerator(tag) => p.clips.iter().any(|c| c.r#gen.as_ref().is_some_and(|g| g.tag() == tag)),
        Predicate::SplitAtMarker(name) => {
            p.markers.iter().filter(|m| m.name == name).any(|m| video_meet_at(p, m.t))
        }
        Predicate::ClipMoved { id, start_s, lane } => p
            .clips
            .iter()
            .find(|c| c.id.0 == id)
            .is_some_and(|c| c.start != Tick(start_s * 90_000) || c.lane != lane),
        Predicate::LoopRegionSet => p.region.is_some() && p.loop_enabled,
        Predicate::WindowOpen(id) => seen.open.contains(&id),
        Predicate::ChainHas(tag) => p.clips.iter().any(|c| c.fx.iter().any(|f| f.tag() == tag)),
        Predicate::ChainCurve(tag) => p.clips.iter().any(|c| {
            c.fx.iter().enumerate().any(|(i, f)| f.tag() == tag && c.fx_auto.get(i).is_some_and(|a| !a.is_empty()))
        }),
        Predicate::VideoTransition | Predicate::TransitionKindChanged => {
            let video = |id: Option<kt_model::ClipId>| {
                id.and_then(|id| p.clips.iter().find(|c| c.id == id)).is_some_and(|c| c.kind == ClipKind::Video)
            };
            p.transitions.iter().filter(|t| video(t.a) && video(t.b)).any(|t| {
                *pr == Predicate::VideoTransition || !matches!(t.kind, TransitionKind::Crossfade)
            })
        }
        Predicate::MorpherFx(n) => p
            .morphers
            .iter()
            .any(|m| m.patch.video_fx.len() + m.patch.audio_fx.len() >= n),
        Predicate::RenderDone => seen.render_done,
    }
}

/// The first unfinished step of a STARTED tour (1-based); `None` — not started, or finished.
pub fn next_step(tour: &TourProgress) -> Option<(u32, u32)> {
    if !tour.started() {
        return None;
    }
    first_open_step(tour)
}

fn first_open_step(tour: &TourProgress) -> Option<(u32, u32)> {
    for (li, l) in LESSONS.iter().enumerate() {
        for si in 0..l.steps.len() {
            if !tour.is_done(li as u32 + 1, si as u32 + 1) {
                return Some((li as u32 + 1, si as u32 + 1));
            }
        }
    }
    None
}

/// The current step of one lesson (1-based), `None` when the lesson is done.
pub fn current_step(tour: &TourProgress, lesson: u32) -> Option<u32> {
    let l = LESSONS.get(lesson.checked_sub(1)? as usize)?;
    (1..=l.steps.len() as u32).find(|s| !tour.is_done(lesson, *s))
}

/// The lesson copy of the demo project: generators only; the region and the loop cleared (a
/// lesson step sets them), and the marker the cut step aims at, in the middle of the first event.
pub fn lesson_project() -> Project {
    let mut p = kt_model::demo::demo_project(kt_model::demo::DemoSources::Generators);
    p.region = None;
    p.loop_enabled = false;
    p.markers.push(kt_model::Marker::named(Tick(6 * 90_000), CUT_MARKER));
    p.normalize();
    p
}

/// A lesson sandbox: `<store>/lessons/lesson-<n>.kt`. It is not the person's project — it is
/// never listed in Recent and gets no poster.
pub fn is_sandbox(path: &std::path::Path) -> bool {
    path.parent().and_then(|d| d.file_name()).is_some_and(|n| n == "lessons")
        && path.file_stem().is_some_and(|s| s.to_string_lossy().starts_with("lesson-"))
}

/// The lessons panel's state (lives in `KtApp::start`).
#[derive(Default)]
pub struct LessonsUi {
    pub open: bool,
    /// The lesson shown (1-based).
    pub lesson: u32,
    /// The state the predicates last saw: (model revision, observed state). A new evaluation
    /// only when it changes.
    seen: Option<(u64, Observed)>,
}

impl crate::app::KtApp {
    /// The windows of the registry that are open now (the observed half of a predicate).
    pub(crate) fn open_window_ids(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if !self.fx_editors.is_empty() {
            v.push("fx");
        }
        if !self.morph_editors.is_empty() {
            v.push("morph");
        }
        if self.render_setup.open {
            v.push("kt_render_settings");
        }
        if self.render_job.is_some() {
            v.push("kt_render_progress");
        }
        v
    }

    fn observed(&self) -> Observed {
        Observed {
            open: self.open_window_ids(),
            render_done: self.render_job.as_ref().is_some_and(|j| matches!(j.result, Some(Ok(_)))),
        }
    }

    /// The step the panel shows now and its target, when the panel is open.
    pub(crate) fn lesson_target(&self) -> Option<Target> {
        let l = &self.start.lessons;
        if !l.open {
            return None;
        }
        let s = current_step(&self.start.store.tour, l.lesson)?;
        LESSONS.get(l.lesson as usize - 1)?.steps.get(s as usize - 1)?.show
    }

    /// The menu item of `key` is the current step's target: it draws its outline.
    pub(crate) fn lesson_marks_menu(&self, key: Key) -> bool {
        matches!(self.lesson_target(), Some(Target::Menu(path)) if path.last() == Some(&key))
    }

    /// Start or continue a lesson: write its copy when needed and open it.
    pub(crate) fn open_lesson(&mut self, lesson: u32, fresh: bool) {
        let lesson = lesson.clamp(1, LESSONS.len() as u32);
        let dir = self
            .start
            .store
            .dir()
            .map(|d| d.join("lessons"))
            .unwrap_or_else(|| std::env::temp_dir().join(format!("kt-lessons-{}", std::process::id())));
        let path = dir.join(format!("lesson-{lesson}.kt"));
        if fresh {
            self.start.store.reset_tour();
        }
        if fresh || !path.exists() {
            let _ = std::fs::create_dir_all(&dir);
            let text = crate::app::project_text_of(&lesson_project(), &Default::default(), Some(&dir));
            if let Err(e) = crate::app::write_project_atomic(&path, &text) {
                self.warn(crate::tf!(k::msg::SAVE_FAILED, e));
                return;
            }
        }
        match crate::app::KtApp::open(&path) {
            Ok(mut app) => {
                app.claim_project(&path);
                self.replace_with(app);
            }
            Err(e) => {
                self.note(crate::tf!(k::msg::OPEN_FAILED, e));
                return;
            }
        }
        self.start.store.touch_tour();
        self.start.lessons.open = true;
        self.start.lessons.lesson = lesson;
        self.start.lessons.seen = None;
    }

    /// "Show": open or focus the step's target.
    pub(crate) fn lesson_show(&mut self, ctx: &egui::Context, t: Target) {
        match t {
            Target::Window("fx") => {
                let first = self.model.project().clips.iter().find(|c| c.kind == kt_model::ClipKind::Video).map(|c| c.id);
                if let Some(id) = first {
                    self.open_fx_editor(id);
                }
            }
            Target::Window("morph") => {
                if let Some(id) = self.model.project().morphers.first().map(|m| m.id) {
                    self.open_morph_editor(id);
                }
            }
            Target::Window("kt_render_settings") => self.start_render(),
            Target::Window(_) => {}
            Target::Menu(path) => {
                // the root menu opens by code (probe 1.1); the item outlines itself
                if path.first() == Some(&k::menu::file::TITLE)
                    && let Some(id) = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("kt_menu_root_file")))
                {
                    egui::Popup::open_id(ctx, id.with("popup"));
                }
            }
            // a panel is always on screen: its outline is the answer
            Target::Panel(_) => {}
        }
    }

    /// Re-evaluate the current step only when what it reads has changed; a done step is
    /// recorded and the next one becomes current.
    fn lesson_tick(&mut self) {
        let lesson = self.start.lessons.lesson;
        let seen = self.observed();
        let key = (self.model_rev, seen.clone());
        if self.start.lessons.seen.as_ref() == Some(&key) {
            return;
        }
        self.start.lessons.seen = Some(key);
        let Some(l) = LESSONS.get(lesson.saturating_sub(1) as usize) else { return };
        // a step can close several in a row (a title added before its step was reached)
        while let Some(s) = current_step(&self.start.store.tour, lesson) {
            let step = &l.steps[s as usize - 1];
            if !done(&step.done, self.model.project(), &seen) {
                break;
            }
            self.start.store.finish_step(lesson, s);
        }
    }

    /// The lessons panel (docked on the right; drawn before the timeline so it spans the height).
    pub(crate) fn draw_lessons_panel(&mut self, ui: &mut egui::Ui) {
        if !self.start.lessons.open {
            return;
        }
        self.lesson_tick();
        let lesson = self.start.lessons.lesson;
        let Some(l) = LESSONS.get(lesson.saturating_sub(1) as usize) else { return };
        let tour = self.start.store.tour.clone();
        let cur = current_step(&tour, lesson);
        let mut show: Option<Target> = None;
        let mut go: Option<(u32, bool)> = None;
        let mut close = false;
        egui::Panel::right("lessons")
            .resizable(false)
            .exact_size(280.0)
            .frame(crate::theme::panel_frame())
            .show(ui, |ui| {
                crate::widgets::field_probe::note("lessons.panel", ui.max_rect());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(crate::tf!(k::lessons::HEAD, lesson, LESSONS.len(), tr(l.title)))
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let r = ui.add(egui::Button::new(crate::icons::CLEAR).frame(false));
                        crate::widgets::hint(&r, tr(k::lessons::CLOSE_HINT));
                        if r.clicked() {
                            close = true;
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([true, false]).show(ui, |ui| {
                    for (i, step) in l.steps.iter().enumerate() {
                        let n = i as u32 + 1;
                        if tour.is_done(lesson, n) {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new(crate::icons::CHECK).color(crate::theme::AREA));
                                ui.weak(tr(step.text));
                            });
                        } else if cur == Some(n) {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(crate::tf!(k::lessons::STEP, n)).strong());
                            ui.add(egui::Label::new(tr(step.text)).wrap());
                            if let Some(t) = step.show {
                                let r = ui.button(tr(k::lessons::SHOW));
                                crate::widgets::field_probe::note("lessons.show", r.rect);
                                if r.clicked() {
                                    show = Some(t);
                                }
                            }
                        }
                    }
                    if cur.is_none() {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(tr(k::lessons::LESSON_DONE)).color(crate::theme::AREA));
                        if (lesson as usize) < LESSONS.len() {
                            if ui.button(tr(k::lessons::NEXT)).clicked() {
                                go = Some((lesson + 1, false));
                            }
                        } else {
                            ui.weak(tr(k::lessons::ALL_DONE));
                        }
                    }
                    ui.add_space(12.0);
                    ui.separator();
                    let r = ui.small_button(tr(k::lessons::START_OVER));
                    crate::widgets::field_probe::note("lessons.start_over", r.rect);
                    crate::widgets::hint(&r, tr(k::lessons::START_OVER_HINT));
                    if r.clicked() {
                        go = Some((1, true));
                    }
                });
            });
        if let Some(t) = show {
            self.lesson_show(ui.ctx(), t);
        }
        if close {
            self.start.lessons.open = false;
        }
        if let Some((n, fresh)) = go {
            self.start_go(crate::welcome::StartGo::Lesson { lesson: n, fresh });
        }
    }

    /// The static outline of a panel target (drawn on the panels' own layer, after them).
    pub(crate) fn draw_lesson_outline(&self, ctx: &egui::Context) {
        let rect = match self.lesson_target() {
            Some(Target::Panel(Panel::Timeline)) => self.timeline_rect,
            _ => return,
        };
        ctx.layer_painter(egui::LayerId::background()).rect_stroke(
            rect.shrink(1.0),
            2.0,
            egui::Stroke::new(2.0, crate::theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
}
