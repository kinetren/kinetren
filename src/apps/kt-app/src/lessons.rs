//! The tour: lessons in their own view (welcome-window-and-tour D2, D7, D8; tour-lessons-fixes
//! D1–D7; capability `ui-reach`).
//!
//! The owner's request of 2026-10-28 revised F-HLP-028: of the three forms its reconnaissance
//! looked at, the lesson that CHECKS the done action in its own view (Ableton Learn View) is
//! taken; a coach mark over the interface stays rejected. So:
//!
//! * the lessons live in a docked side panel (`Panel::right`) — it narrows the editor, it does
//!   not cover it, and it adds no layer above the window layer (t988,
//!   `no_teaching_layer_over_the_ui`);
//! * a step closes by ITSELF on a predicate over the project's state and the program's
//!   observable state — never on "Next", never on "Show", never on a command registry
//!   (F-UI-002). Predicates look at the state, not the gesture, so every door counts;
//! * a predicate is re-evaluated when anything it reads changes — the model revision, the
//!   region, the loop switch, the open windows, the render result (`Seen`, a `Copy` key built
//!   every frame without an allocation; invariant 12). The region and the loop do NOT bump
//!   `model_rev` (R.43 item 296), and keying on the revision alone closed "Play a part in a loop"
//!   only when a window closed (kkt-26.11.01 item 2);
//! * every step names where the person acts (`Target`); each target draws a STATIC accent
//!   outline itself while its step is current (the owner, 2026-10-28: "yes, it may draw") — no
//!   layer, no pulsation, no dimming. "Show" POINTS: it brings the target into view and never
//!   performs the step (kkt-26.11.01 item 5: it opened the morpher, which taught nothing);
//! * a step's text is catalog lines with the Guide's inline markup — `<kbd>` keycaps, bold names
//!   (kkt-26.11.01 item 3); the labels of the program's own controls are substituted from their
//!   catalog keys, so a lesson cannot drift from the button;
//! * every lesson runs on a program-owned COPY of the demo project made of generators, in
//!   `<store>/lessons/` — never in the person's folders, never in Recent (D8).

use crate::guide::{Block, Inline};
use crate::i18n::{Key, Lang, k, tr};
use crate::user_store::TourProgress;
use eframe::egui;
use kt_model::Project;
use kt_time::Tick;

/// Where a step's person acts. The first target is where "Show" goes; a target that cannot be
/// brought into view (a control of a closed window) hands "Show" to the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A menu path of catalog keys: the root title, then the items. Every item past the root
    /// outlines itself wherever that menu is drawn.
    Menu(&'static [Key]),
    /// An item of an event's right-click menu; "Show" brings the event into view.
    ClipMenu { clip: u64, item: Key },
    /// An event of the lesson copy (by its demo id).
    Clip(u64),
    /// A marker on the ruler, by name.
    Marker(&'static str),
    /// The ruler (a region is drawn on it).
    Ruler,
    /// The first transition between two video events.
    Transition,
    /// The first morpher block.
    Morpher,
    /// A named control of a window or the toolbar (`CONTROLS`), marked by its own drawing code
    /// through `widgets::lesson_mark::control`.
    Control(&'static str),
}

/// The named controls a lesson may point at. Each is marked by a `lesson_mark::control("<name>"`
/// call in the program's sources (t1092): a renamed control cannot leave a dead step.
#[cfg_attr(not(test), allow(dead_code))]
pub const CONTROLS: &[&str] = &["fxreg.add", "fx.auto_add", "tb.loop", "tb.render", "tr.kind", "render.start"];

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

/// A value substituted for `{n}` in a step's lines — always the program's own text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    /// A control's catalog label, as a keycap.
    Label(Key),
    /// A menu path, each item a keycap, joined by `›`.
    Path(&'static [Key]),
    /// A generator kind's name by tag, as a keycap.
    Gen(&'static str),
    /// An effect's name by tag (effect names are not translated, `FxEntry::name`), bold.
    Fx(&'static str),
}

/// Where the result of a step shows: the playhead goes there when the step closes (D6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Inside the first event whose chain holds this effect.
    ChainWith(&'static str),
    /// The middle of the first video transition.
    Transition,
    /// Inside the first morpher's span.
    Morpher,
}

/// One step: its lines (catalog keys, each a paragraph or a `- ` list item ≤ 160 characters),
/// what fills their `{n}`, its targets, the predicate, the line that says what changed and where
/// the playhead goes when it closes.
#[derive(Debug, Clone, Copy)]
pub struct Step {
    pub text: &'static [Key],
    pub args: &'static [Arg],
    pub targets: &'static [Target],
    pub done: Predicate,
    pub result: Option<Key>,
    pub look: Option<Look>,
}

/// One lesson.
#[derive(Debug, Clone, Copy)]
pub struct Lesson {
    pub title: Key,
    pub steps: &'static [Step],
}

/// The marker the cut step aims at (the lesson copy adds it in the middle of the first event).
pub const CUT_MARKER: &str = "CUT HERE";

/// Where "Create a generator…" lives. ONE constant: `generators-placement-and-audio` moves the
/// item out of File by editing this path; the lesson's text and "Show" follow it.
pub const GENERATOR_MENU: &[Key] = &[k::menu::file::TITLE, k::bin::menu::NEW_GENERATOR, k::genkind::GROUP_TITLES];
const FILE_RENDER: &[Key] = &[k::menu::file::TITLE, k::menu::file::RENDER];

const fn step(text: &'static [Key], targets: &'static [Target], done: Predicate) -> Step {
    Step { text, args: &[], targets, done, result: None, look: None }
}

/// The lessons (design D7 of welcome-window-and-tour; the texts — tour-lessons-fixes D2).
pub const LESSONS: &[Lesson] = &[
    Lesson {
        title: k::lessons::L1,
        steps: &[
            Step {
                args: &[Arg::Path(GENERATOR_MENU), Arg::Gen("text")],
                ..step(&[k::lessons::L1_S1, k::lessons::L1_S1_B], &[Target::Menu(GENERATOR_MENU)], Predicate::HasGenerator("text"))
            },
            step(
                &[k::lessons::L1_S2, k::lessons::L1_S2_B, k::lessons::L1_S2_C],
                &[Target::Marker(CUT_MARKER)],
                Predicate::SplitAtMarker(CUT_MARKER),
            ),
            step(&[k::lessons::L1_S3], &[Target::Clip(7)], Predicate::ClipMoved { id: 7, start_s: 48, lane: 0 }),
            Step {
                result: Some(k::lessons::L1_S4_DONE),
                ..step(
                    &[k::lessons::L1_S4, k::lessons::L1_S4_B, k::lessons::L1_S4_C],
                    &[Target::Ruler, Target::Control("tb.loop")],
                    Predicate::LoopRegionSet,
                )
            },
        ],
    },
    Lesson {
        title: k::lessons::L2,
        steps: &[
            step(&[k::lessons::L2_S1], &[Target::Clip(1)], Predicate::WindowOpen("fx")),
            Step {
                args: &[Arg::Label(k::fxreg::ADD), Arg::Fx("blur")],
                result: Some(k::lessons::L2_S2_DONE),
                look: Some(Look::ChainWith("blur")),
                ..step(
                    &[k::lessons::L2_S2, k::lessons::L2_S2_B, k::lessons::L2_S2_C],
                    &[Target::Control("fxreg.add"), Target::Clip(1)],
                    Predicate::ChainHas("blur"),
                )
            },
            Step {
                args: &[Arg::Label(k::fx::AUTO_ADD)],
                result: Some(k::lessons::L2_S3_DONE),
                look: Some(Look::ChainWith("blur")),
                ..step(
                    &[k::lessons::L2_S3, k::lessons::L2_S3_B, k::lessons::L2_S3_C],
                    &[Target::Control("fx.auto_add"), Target::Clip(1)],
                    Predicate::ChainCurve("blur"),
                )
            },
        ],
    },
    Lesson {
        title: k::lessons::L3,
        steps: &[
            // the program makes a transition from the menu, never from an overlap: an overlap is
            // a cut (the owner's decision, t163/t464; kkt-26.11.01 item 6 and answer 1)
            Step {
                args: &[Arg::Label(k::tl::menu::TRANSITION_PREV), Arg::Label(k::trn::kind::CROSSFADE)],
                result: Some(k::lessons::L3_S1_DONE),
                look: Some(Look::Transition),
                ..step(
                    &[k::lessons::L3_S1, k::lessons::L3_S1_B, k::lessons::L3_S1_C, k::lessons::L3_S1_D],
                    &[Target::ClipMenu { clip: 3, item: k::tl::menu::TRANSITION_PREV }],
                    Predicate::VideoTransition,
                )
            },
            Step {
                args: &[Arg::Label(k::trn::KIND)],
                result: Some(k::lessons::L3_S2_DONE),
                look: Some(Look::Transition),
                ..step(
                    &[k::lessons::L3_S2, k::lessons::L3_S2_B, k::lessons::L3_S2_C],
                    &[Target::Transition, Target::Control("tr.kind")],
                    Predicate::TransitionKindChanged,
                )
            },
        ],
    },
    Lesson {
        title: k::lessons::L4,
        steps: &[
            step(&[k::lessons::L4_S1], &[Target::Morpher], Predicate::WindowOpen("morph")),
            Step {
                args: &[Arg::Label(k::fxreg::ADD)],
                result: Some(k::lessons::L4_S2_DONE),
                look: Some(Look::Morpher),
                ..step(
                    &[k::lessons::L4_S2, k::lessons::L4_S2_B, k::lessons::L4_S2_C],
                    &[Target::Control("fxreg.add"), Target::Morpher],
                    Predicate::MorpherFx(2),
                )
            },
        ],
    },
    Lesson {
        title: k::lessons::L5,
        steps: &[
            Step {
                args: &[Arg::Path(FILE_RENDER)],
                ..step(
                    &[k::lessons::L5_S1, k::lessons::L5_S1_B],
                    &[Target::Menu(FILE_RENDER), Target::Control("tb.render")],
                    Predicate::WindowOpen("kt_render_settings"),
                )
            },
            Step {
                args: &[Arg::Label(k::render::START)],
                result: Some(k::lessons::L5_S2_DONE),
                ..step(
                    &[k::lessons::L5_S2, k::lessons::L5_S2_B],
                    &[Target::Control("render.start"), Target::Control("tb.render")],
                    Predicate::RenderDone,
                )
            },
        ],
    },
];

/// The silly lines of the render wait (kkt-26.11.01 item 8: "you could make some silly jokes in
/// the lessons block while the render runs, it is too boring").
pub const JOKES: &[Key] = &[
    k::lessons::JOKE_1,
    k::lessons::JOKE_2,
    k::lessons::JOKE_3,
    k::lessons::JOKE_4,
    k::lessons::JOKE_5,
    k::lessons::JOKE_6,
    k::lessons::JOKE_7,
    k::lessons::JOKE_8,
];
/// How long one joke stays.
pub const JOKE_SECS: f64 = 6.0;

/// The joke of the moment `t` (seconds of the UI clock) — a fixed order, so "changed" is testable.
pub fn joke_at(t: f64) -> Key {
    JOKES[((t / JOKE_SECS).max(0.0) as usize) % JOKES.len()]
}

/// The windows whose openness the program observes (`KtApp::open_window_bits`). A `WindowOpen`
/// of any other id could never close — the guard (t988) refuses it.
pub const OBSERVED_WINDOWS: &[&str] = &["fx", "morph", "kt_render_settings", "kt_render_progress"];

/// The observable state a predicate may read besides the project — and with the model revision,
/// the key of a re-check (D1). `Copy`: built every frame, compared, never allocated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Observed {
    /// Bit `i` — `OBSERVED_WINDOWS[i]` is open.
    pub open: u8,
    pub render_done: bool,
}

impl Observed {
    pub fn is_open(&self, id: &str) -> bool {
        OBSERVED_WINDOWS.iter().position(|w| *w == id).is_some_and(|i| self.open & (1 << i) != 0)
    }
}

/// A timeline row: its kind and track number.
type Row = (kt_model::LaneKind, u32);

/// The re-check key: everything a predicate reads that is not inside the model revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seen {
    rev: u64,
    region: Option<(Tick, Tick)>,
    loop_on: bool,
    clips: usize,
    seen: Observed,
}

fn video_meet_at(p: &Project, at: Tick) -> bool {
    use kt_model::ClipKind;
    p.clips.iter().filter(|a| a.kind == ClipKind::Video && a.start + a.duration == at).any(|a| {
        p.clips.iter().any(|b| b.kind == ClipKind::Video && b.lane == a.lane && b.start == at && b.id != a.id)
    })
}

fn video_transitions(p: &Project) -> impl Iterator<Item = &kt_model::TransitionEvent> {
    let video = |id: Option<kt_model::ClipId>| {
        id.and_then(|id| p.clips.iter().find(|c| c.id == id)).is_some_and(|c| c.kind == kt_model::ClipKind::Video)
    };
    p.transitions.iter().filter(move |t| video(t.a) && video(t.b))
}

/// Is the step done? Pure: the project and the observed state, nothing else.
pub fn done(pr: &Predicate, p: &Project, seen: &Observed) -> bool {
    use kt_model::TransitionKind;
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
        Predicate::WindowOpen(id) => seen.is_open(id),
        Predicate::ChainHas(tag) => p.clips.iter().any(|c| c.fx.iter().any(|f| f.tag() == tag)),
        Predicate::ChainCurve(tag) => p.clips.iter().any(|c| {
            c.fx.iter().enumerate().any(|(i, f)| f.tag() == tag && c.fx_auto.get(i).is_some_and(|a| !a.is_empty()))
        }),
        Predicate::VideoTransition => video_transitions(p).next().is_some(),
        Predicate::TransitionKindChanged => {
            video_transitions(p).any(|t| !matches!(t.kind, TransitionKind::Crossfade))
        }
        Predicate::MorpherFx(n) => p
            .morphers
            .iter()
            .any(|m| m.patch.video_fx.len() + m.patch.audio_fx.len() >= n),
        Predicate::RenderDone => seen.render_done,
    }
}

/// The moment where a step's result shows (D6), `None` when the project has no such thing.
pub fn look_at(look: Look, p: &Project) -> Option<Tick> {
    match look {
        Look::ChainWith(tag) => p
            .clips
            .iter()
            .find(|c| c.fx.iter().any(|f| f.tag() == tag))
            .map(|c| Tick(c.start.0 + c.duration.0 / 2)),
        Look::Transition => {
            let w = video_transitions(p).find_map(|t| t.window(p))?;
            Some(Tick((w.start.0 + w.end.0) / 2))
        }
        Look::Morpher => p.morphers.first().map(|m| Tick(m.start.0 + (4 * 90_000).min(m.duration.0 / 2))),
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

// ---- the text (D2) -----------------------------------------------------------------------------

/// A catalog string in LOGICAL order: the Arabic table is reordered by the build, which would
/// mirror the `<kbd>` tags; the lesson parses the logical text and reorders the runs itself.
fn logical(key: Key) -> &'static str {
    if crate::i18n::current().is_rtl() { crate::i18n::ar_logical(key) } else { tr(key) }
}

fn gen_name(tag: &str) -> String {
    let Some(kind) = kt_model::GeneratorKind::default_for_tag(tag) else { return tag.to_string() };
    if crate::i18n::current().is_rtl()
        && let kt_model::GeneratorKind::Plugin(p) = &kind
        && let Some(key) = crate::i18n::key_by_name(p.manifest().ui.name_key)
    {
        return crate::i18n::ar_logical(key).to_string();
    }
    crate::gen_ui::kind_name(&kind).to_string()
}

fn fx_name(tag: &str) -> &'static str {
    crate::fx_registry::registry().iter().find(|e| e.template.tag() == tag).map_or("", |e| e.name)
}

/// The text of one argument, with its markup.
fn arg_text(a: &Arg) -> String {
    match a {
        Arg::Label(key) => format!("<kbd>{}</kbd>", logical(*key).trim_end_matches(':')),
        Arg::Path(path) => {
            let sep = if crate::i18n::current().is_rtl() { " \u{2039} " } else { " \u{203A} " };
            path.iter().map(|k| format!("<kbd>{}</kbd>", logical(*k))).collect::<Vec<_>>().join(sep)
        }
        Arg::Gen(tag) => format!("<kbd>{}</kbd>", gen_name(tag)),
        Arg::Fx(tag) => format!("**{}**", fx_name(tag)),
    }
}

fn substitute(line: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(line.len() + 32);
    let mut rest = line;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        match tail.find('}').and_then(|e| tail[..e].parse::<usize>().ok().map(|n| (n, e))) {
            Some((n, e)) => {
                out.push_str(args.get(n).map_or("", |s| s.as_str()));
                rest = &tail[e + 1..];
            }
            None => {
                out.push('{');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// One catalog line → a block: `- ` makes a list item; the runs are the Guide's inline subset.
pub fn line_block(line: &str) -> Result<Block, String> {
    let (item, body) = match line.strip_prefix("- ") {
        Some(b) => (true, b),
        None => (false, line),
    };
    let mut runs = crate::guide::inlines(body)?;
    if crate::i18n::current().is_rtl() {
        // the same atoms the build gives a one-line label: each run reordered, the runs reversed
        runs.reverse();
        for r in &mut runs {
            match r {
                Inline::Text(t) | Inline::Bold(t) | Inline::Italic(t) | Inline::Code(t) | Inline::Kbd(t) => {
                    *t = crate::i18n::bidi::visual(t)
                }
                Inline::Link(t, _) => *t = crate::i18n::bidi::visual(t),
            }
        }
    }
    Ok(if item { Block::Item { depth: 0, number: None, text: runs } } else { Block::Para(runs) })
}

/// A step's text in the current language, as Guide blocks.
pub fn step_blocks(s: &Step) -> Result<Vec<Block>, String> {
    let args: Vec<String> = s.args.iter().map(arg_text).collect();
    s.text.iter().map(|key| line_block(&substitute(logical(*key), &args))).collect()
}

fn line_of(key: Key) -> Vec<Block> {
    line_block(logical(key)).map(|b| vec![b]).unwrap_or_default()
}

// ---- the panel -------------------------------------------------------------------------------

/// The lessons panel's state (lives in `KtApp::start`).
#[derive(Default)]
pub struct LessonsUi {
    pub open: bool,
    /// The lesson shown (1-based).
    pub lesson: u32,
    /// The state the predicates last saw; a new evaluation only when it changes.
    seen: Option<Seen>,
    /// The parsed text of the shown lesson: (language, lesson) → per step. Rebuilt on a change
    /// of language or lesson, never per frame.
    text: Option<(Lang, u32, Vec<StepText>)>,
}

/// A step's text, parsed once (D2).
#[derive(Default)]
pub struct StepText {
    pub body: Vec<Block>,
    /// The line under the step once it is checked (what changed).
    pub result: Vec<Block>,
    /// The body as plain text: a checked step is drawn muted, as one wrapped label.
    pub plain: String,
}

impl LessonsUi {
    fn text_of(&mut self, lesson: u32) -> &[StepText] {
        let lang = crate::i18n::current();
        if !matches!(&self.text, Some((l, n, _)) if *l == lang && *n == lesson) {
            let steps = LESSONS
                .get(lesson.saturating_sub(1) as usize)
                .map(|l| {
                    l.steps
                        .iter()
                        .map(|s| {
                            let body = step_blocks(s).unwrap_or_default();
                            let plain = crate::guide::plain_blocks(&body);
                            StepText { body, result: s.result.map(line_of).unwrap_or_default(), plain }
                        })
                        .collect()
                })
                .unwrap_or_default();
            self.text = Some((lang, lesson, steps));
        }
        self.text.as_ref().map_or(&[], |(_, _, v)| v.as_slice())
    }
}

impl crate::app::KtApp {
    /// The windows of the registry that are open now (the observed half of a predicate).
    fn open_window_bits(&self) -> u8 {
        let open = [
            !self.fx_editors.is_empty(),
            !self.morph_editors.is_empty(),
            self.render_setup.open,
            self.render_job.is_some(),
        ];
        open.iter().enumerate().fold(0u8, |acc, (i, o)| if *o { acc | (1 << i) } else { acc })
    }

    fn observed(&self) -> Observed {
        Observed {
            open: self.open_window_bits(),
            render_done: self.render_job.as_ref().is_some_and(|j| matches!(j.result, Some(Ok(_)))),
        }
    }

    /// The current step of the shown lesson, when the panel is open.
    fn lesson_step(&self) -> Option<&'static Step> {
        let l = &self.start.lessons;
        if !l.open {
            return None;
        }
        let s = current_step(&self.start.store.tour, l.lesson)?;
        LESSONS.get(l.lesson as usize - 1)?.steps.get(s as usize - 1)
    }

    /// The current step's targets (empty when the panel is closed or the lesson done).
    pub(crate) fn lesson_targets(&self) -> &'static [Target] {
        self.lesson_step().map_or(&[], |s| s.targets)
    }

    /// The menu item of `key` is one of the current step's targets.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn lesson_marks_menu(&self, key: Key) -> bool {
        self.lesson_targets().iter().any(|t| match t {
            Target::Menu(path) => path.iter().skip(1).any(|k| *k == key),
            Target::ClipMenu { item, .. } => *item == key,
            _ => false,
        })
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

    /// The timeline span of a timeline target, and its row.
    fn target_span(&self, t: Target) -> Option<(Tick, Tick, Option<Row>)> {
        use kt_model::{ClipKind, LaneKind};
        let p = self.model.project();
        match t {
            Target::Clip(id) | Target::ClipMenu { clip: id, .. } => {
                let c = p.clips.iter().find(|c| c.id.0 == id)?;
                let kind = if c.kind == ClipKind::Video { LaneKind::Video } else { LaneKind::Audio };
                Some((c.start, c.start + c.duration, Some((kind, c.lane))))
            }
            Target::Marker(name) => p.markers.iter().find(|m| m.name == name).map(|m| (m.t, m.t, None)),
            Target::Transition => {
                let tr = video_transitions(p).next()?;
                let w = tr.window(p)?;
                let lane = tr.a.and_then(|a| p.clips.iter().find(|c| c.id == a)).map(|c| c.lane)?;
                Some((w.start, w.end, Some((LaneKind::Video, lane))))
            }
            Target::Morpher => {
                let m = p.morphers.first()?;
                Some((m.start, m.start + m.duration, Some((LaneKind::Morph, m.lane))))
            }
            Target::Menu(_) | Target::Ruler | Target::Control(_) => None,
        }
    }

    /// Bring a timeline span and its row into view (the view's own arithmetic, `center_view_on`).
    fn reveal_span(&mut self, a: Tick, b: Tick, row: Option<Row>) {
        let visible = self.tl_geom.as_ref().is_some_and(|g| {
            let (x0, x1) = (g.tick_to_x(a.0), g.tick_to_x(b.0));
            x0 >= g.rect.left() && x1 <= g.rect.right()
        });
        if !visible {
            self.center_view_on(Tick((a.0 + b.0) / 2));
        }
        if let (Some(key), Some(g)) = (row, self.tl_geom.as_ref())
            && let Some(r) = g.row_of(key)
        {
            let top = g.row_top(r);
            let bottom = top + g.row_h(r);
            let lanes = g.rect.top() + crate::timeline::RULER_H + 3.0;
            if top < lanes || bottom > g.rect.bottom() {
                // the row's offset from the first row, minus a little air; the timeline clamps it
                self.tl_scroll = (top - g.lanes_top() - 8.0).max(0.0);
            }
        }
        self.dirty_ui = true;
    }

    /// "Show": point at the step's target — bring it into view, open its menu root — and never
    /// perform the step (D4).
    pub(crate) fn lesson_show(&mut self, ctx: &egui::Context, targets: &'static [Target]) {
        for t in targets {
            match *t {
                Target::Menu(path) => {
                    if let Some(root) = path.first()
                        && let Some(id) = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new(("kt_menu_root", root.0))))
                    {
                        egui::Popup::open_id(ctx, id.with("popup"));
                        return;
                    }
                }
                // a control is pointed at only when it is on screen; a closed window hands
                // "Show" to the next target (the element whose double-click opens it)
                Target::Control(name) => {
                    if crate::widgets::lesson_mark::seen(name).is_some() {
                        crate::widgets::lesson_mark::request_scroll(name);
                        return;
                    }
                }
                Target::Ruler => return,
                t => {
                    if let Some((a, b, row)) = self.target_span(t) {
                        self.reveal_span(a, b, row);
                        return;
                    }
                }
            }
        }
    }

    /// Re-evaluate the current step when what it reads has changed; a done step is recorded and
    /// the next one becomes current. A step that closes HERE and has a `look` moves the playhead
    /// to where its result shows (D6).
    fn lesson_tick(&mut self) {
        let lesson = self.start.lessons.lesson;
        let seen = self.observed();
        let p = self.model.project();
        let key = Seen {
            rev: self.model_rev,
            region: p.region.map(|r| (r.start, r.end)),
            loop_on: p.loop_enabled,
            // the harness and an open replace the model wholesale without a revision bump
            clips: p.clips.len() + p.transitions.len() + p.morphers.len(),
            seen,
        };
        if self.start.lessons.seen == Some(key) {
            return;
        }
        self.start.lessons.seen = Some(key);
        let Some(l) = LESSONS.get(lesson.saturating_sub(1) as usize) else { return };
        let mut look = None;
        // a step can close several in a row (a title added before its step was reached)
        while let Some(s) = current_step(&self.start.store.tour, lesson) {
            let step = &l.steps[s as usize - 1];
            if !done(&step.done, self.model.project(), &seen) {
                break;
            }
            self.start.store.finish_step(lesson, s);
            look = step.look.or(look);
        }
        if let Some(at) = look.and_then(|lk| look_at(lk, self.model.project())) {
            self.player.seek(at);
            self.center_view_on(at);
            self.dirty_ui = true;
        }
    }

    /// The lessons panel (docked on the right; drawn before the timeline so it spans the height).
    pub(crate) fn draw_lessons_panel(&mut self, ui: &mut egui::Ui) {
        crate::widgets::lesson_mark::begin(self.lesson_targets());
        if !self.start.lessons.open {
            return;
        }
        self.lesson_tick();
        let lesson = self.start.lessons.lesson;
        let Some(l) = LESSONS.get(lesson.saturating_sub(1) as usize) else { return };
        // the targets may have changed in the tick
        crate::widgets::lesson_mark::set(self.lesson_targets());
        let tour = self.start.store.tour.clone();
        let cur = current_step(&tour, lesson);
        // the result line belongs to the step closed last (the one before the current)
        let last_done = match cur {
            Some(n) => n.checked_sub(1).filter(|n| *n > 0),
            None => Some(l.steps.len() as u32),
        };
        let rendering = self.render_job.as_ref().is_some_and(|j| j.result.is_none())
            && cur.is_some_and(|n| l.steps[n as usize - 1].done == Predicate::RenderDone);
        let now = ui.input(|i| i.time);
        if rendering {
            let next = JOKE_SECS - now.rem_euclid(JOKE_SECS);
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(next));
        }
        let mut show: Option<&'static [Target]> = None;
        let mut go: Option<(u32, bool)> = None;
        let mut close = false;
        let start = &mut self.start;
        let blocks = start.lessons.text_of(lesson);
        egui::Panel::right("lessons")
            .resizable(false)
            .exact_size(280.0)
            .frame(crate::theme::panel_frame())
            .show(ui, |ui| {
                crate::widgets::field_probe::note("lessons.panel", ui.max_rect());
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(crate::tf!(k::lessons::HEAD, lesson, LESSONS.len(), tr(l.title)))
                                .strong(),
                        )
                        .wrap(),
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
                        let Some(StepText { body, result, plain }) = blocks.get(i) else { continue };
                        if tour.is_done(lesson, n) {
                            ui.horizontal_top(|ui| {
                                ui.label(egui::RichText::new(crate::icons::CHECK).color(crate::theme::AREA));
                                ui.vertical(|ui| {
                                    ui.add(egui::Label::new(egui::RichText::new(plain.as_str()).weak()).wrap());
                                });
                            });
                            if last_done == Some(n) && !result.is_empty() {
                                ui.add_space(2.0);
                                egui::Frame::new()
                                    .fill(crate::theme::HOVER)
                                    .inner_margin(6.0)
                                    .corner_radius(3.0)
                                    .show(ui, |ui| {
                                        let r = ui.vertical(|ui| crate::guide::render(ui, result, None)).response;
                                        crate::widgets::field_probe::note("lessons.result", r.rect);
                                    });
                            }
                        } else if cur == Some(n) {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(crate::tf!(k::lessons::STEP, n)).strong());
                            let r = ui.vertical(|ui| crate::guide::render(ui, body, None)).response;
                            crate::widgets::field_probe::note("lessons.step", r.rect);
                            if !step.targets.is_empty() {
                                ui.add_space(4.0);
                                let r = ui.button(tr(k::lessons::SHOW));
                                crate::widgets::field_probe::note("lessons.show", r.rect);
                                crate::widgets::hint(&r, tr(k::lessons::SHOW_HINT));
                                if r.clicked() {
                                    show = Some(step.targets);
                                }
                            }
                            if rendering {
                                ui.add_space(8.0);
                                let r = ui.add(
                                    egui::Label::new(egui::RichText::new(tr(joke_at(now))).italics().color(crate::theme::ACCENT))
                                        .wrap(),
                                );
                                crate::widgets::field_probe::note("lessons.joke", r.rect);
                            }
                        }
                    }
                    if cur.is_none() {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(tr(k::lessons::LESSON_DONE)).color(crate::theme::AREA));
                        if (lesson as usize) < LESSONS.len() {
                            let r = ui.button(tr(k::lessons::NEXT));
                            crate::widgets::field_probe::note("lessons.next", r.rect);
                            if r.clicked() {
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
            // "Next lesson" leaves the program's OWN copy: its edits are kept without a question
            // (the live walk met "Unsaved work" about the sandbox there). "Start over" still
            // asks — it discards them (t988)
            if !fresh && self.project_path.as_deref().is_some_and(is_sandbox) && (self.dirty_content || self.dirty_ui) {
                self.save();
            }
            self.start_go(crate::welcome::StartGo::Lesson { lesson: n, fresh });
        }
    }

    /// The static outlines of the timeline targets (on the panels' own layer, after them,
    /// clipped to the tracks; the menu items and the controls outline themselves).
    pub(crate) fn draw_lesson_outline(&self, ctx: &egui::Context) {
        let targets = self.lesson_targets();
        if targets.is_empty() {
            return;
        }
        let Some(g) = self.tl_geom.as_ref() else { return };
        let ruler = egui::Rect::from_min_max(g.rect.min, egui::pos2(g.rect.right(), g.rect.top() + crate::timeline::RULER_H));
        let tracks = egui::Rect::from_min_max(egui::pos2(g.rect.left(), ruler.bottom()), g.rect.max);
        let paint = |rect: egui::Rect, clip: egui::Rect, tag: &'static str| {
            let shown = rect.intersect(clip);
            if !shown.is_positive() {
                return;
            }
            ctx.layer_painter(egui::LayerId::background()).with_clip_rect(clip).rect_stroke(
                rect.shrink(1.0),
                2.0,
                egui::Stroke::new(2.0, crate::theme::ACCENT),
                egui::StrokeKind::Inside,
            );
            crate::widgets::lesson_mark::note(tag, shown);
        };
        for t in targets {
            let tag = match t {
                Target::Clip(_) | Target::ClipMenu { .. } => "clip",
                Target::Marker(_) => "marker",
                Target::Transition => "transition",
                Target::Morpher => "morpher",
                Target::Ruler => {
                    paint(ruler, ruler, "ruler");
                    continue;
                }
                Target::Menu(_) | Target::Control(_) => continue,
            };
            let Some((a, b, row)) = self.target_span(*t) else { continue };
            let (x0, x1) = (g.tick_to_x(a.0), g.tick_to_x(b.0));
            if matches!(t, Target::Marker(_)) {
                // the marker's flag on the ruler and its line down the tracks
                let r = egui::Rect::from_min_max(egui::pos2(x0 - 7.0, ruler.top()), egui::pos2(x0 + 7.0, ruler.bottom()));
                paint(r, ruler, tag);
                continue;
            }
            let Some(r) = row.and_then(|key| g.row_of(key)) else { continue };
            let top = g.row_top(r);
            let rect = egui::Rect::from_min_max(egui::pos2(x0, top), egui::pos2(x1.max(x0 + 4.0), top + g.row_h(r)));
            paint(rect, tracks, tag);
        }
    }
}
