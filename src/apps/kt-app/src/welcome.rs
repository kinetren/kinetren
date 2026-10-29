//! The welcome window (welcome-window-and-tour, capability `ui-reach`).
//!
//! The owner (2026-10-28): "an intro window (as in programming IDEs) offering either to create
//! a project from a template or a list of recent projects (with thumbnails), and right there a
//! tile 'start the tour'… a main tile where the most likely action lands: for the first opening
//! it is the tour, for later openings the tile of the previous project. And in the same window
//! we can show what's new."
//!
//! * The window is a registered window INSIDE the main viewport (design D3): a second OS window
//!   brings the tiling-WM and Alt+Tab defects F-HLP-027 found.
//! * Exactly ONE tile is primary, and it is chosen by the pure `primary()` (D4); the start order
//!   of `run` asks it once, with the file-existence facts gathered before the call.
//! * Posters are rendered by the offline path at save/close (D5) into `<store>/posters/`, never
//!   next to the project; the window only READS finished PNGs, on a helper thread, and uploads a
//!   texture when one arrives (one repaint each, invariant 12).
//! * Nothing animates: the window repaints on input only.

use crate::user_store::{RecentEntry, TourProgress, UserTemplate};

/// A built-in template (D6). The label is a technical token, the same in every language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Template {
    pub id: &'static str,
    pub label: &'static str,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// The five built-ins, in the order the window shows them.
pub const BUILTIN: [Template; 5] = [
    Template { id: "1080p25", label: "1080p25", width: 1920, height: 1080, fps: 25 },
    Template { id: "1080p30", label: "1080p30", width: 1920, height: 1080, fps: 30 },
    Template { id: "4k30", label: "4K30", width: 3840, height: 2160, fps: 30 },
    Template { id: "v916", label: "9:16", width: 1080, height: 1920, fps: 30 },
    Template { id: "sq11", label: "1:1", width: 1080, height: 1080, fps: 30 },
];

/// The template a "new from the last-used template" falls back to when none was used yet.
pub const DEFAULT_TEMPLATE: &str = "1080p30";

/// A user template's id: `user:<name>`.
pub fn user_template_id(name: &str) -> String {
    format!("user:{name}")
}

/// Frame size and rate of a template id (built-in or the person's own).
pub fn template_by_id(id: &str, user: &[UserTemplate]) -> Option<(u32, u32, u32)> {
    if let Some(name) = id.strip_prefix("user:") {
        return user.iter().find(|t| t.name == name).map(|t| (t.width, t.height, t.fps));
    }
    BUILTIN.iter().find(|t| t.id == id).map(|t| (t.width, t.height, t.fps))
}

/// The project a template creates (D6): audio stays 48 kHz (invariant 1), and the settings
/// are explicit — the first imported clip does not resize the project.
pub fn project_for(width: u32, height: u32, fps: u32) -> kt_model::Project {
    kt_model::Project::new(kt_model::ProjectSettings {
        fps,
        width,
        height,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: true,
    })
}

/// What the primary tile does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tile {
    Tour,
    /// 1-based lesson and step, as the tile reads them.
    ContinueTour { lesson: u32, step: u32 },
    /// The row of the recent list.
    Recent(usize),
    /// A template id.
    NewFromTemplate(String),
}

/// What the start does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartAction {
    /// "Show at start" is off: the former start — the last project opens silently.
    NoWindowOpenLast,
    /// A project on the command line, or a recovery offer: the former start, no window.
    NoWindow,
    /// FFmpeg is missing: its window first, then this.
    AfterFfmpeg(Box<StartAction>),
    Window { primary: Tile },
}

/// The facts of the start, gathered BEFORE the call (one `exists()` per recent row).
#[derive(Debug, Clone)]
pub struct StartFacts<'a> {
    pub cli_project: bool,
    pub recovery: bool,
    pub ffmpeg_missing: bool,
    pub show_at_start: bool,
    pub first_start: bool,
    pub tour: &'a TourProgress,
    /// The first unfinished step of a STARTED tour; `None` — not started or finished.
    pub tour_next: Option<(u32, u32)>,
    /// The recent rows with "the file exists".
    pub recent: &'a [(RecentEntry, bool)],
    pub opened_since_tour: bool,
    pub last_template: Option<&'a str>,
}

/// The primary tile — the most likely action (D4). The order of the cases is the scenario of
/// t984 and lives in this one function.
pub fn primary(f: &StartFacts) -> StartAction {
    if f.cli_project || f.recovery {
        return StartAction::NoWindow;
    }
    if !f.show_at_start {
        return StartAction::NoWindowOpenLast;
    }
    let w = StartAction::Window { primary: window_tile(f) };
    if f.ffmpeg_missing { StartAction::AfterFfmpeg(Box::new(w)) } else { w }
}

/// The tile inside the window (also asked when the window is reopened from Help).
pub fn window_tile(f: &StartFacts) -> Tile {
    if f.first_start {
        return Tile::Tour;
    }
    let cont = f.tour_next.map(|(lesson, step)| Tile::ContinueTour { lesson, step });
    if let Some(c) = cont.clone()
        && !f.opened_since_tour
    {
        return c;
    }
    let template = || {
        let from_row = f.recent.first().and_then(|(e, _)| e.template.as_deref());
        Tile::NewFromTemplate(f.last_template.or(from_row).unwrap_or(DEFAULT_TEMPLATE).to_string())
    };
    match f.recent.first() {
        Some((_, true)) => Tile::Recent(0),
        // the previous project is missing: its row stays (flagged), and the most likely
        // action becomes a new project like the last one
        Some((_, false)) => template(),
        None if !f.tour.started() => Tile::Tour,
        None => cont.unwrap_or_else(template),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, opened: u64) -> RecentEntry {
        RecentEntry {
            path: std::path::PathBuf::from(format!("/projects/{name}.kt")),
            name: name.into(),
            opened,
            template: Some("v916".into()),
        }
    }

    /// Test 984 (welcome-window-and-tour, ui-reach): the primary tile for each case of the spec,
    /// in its order — including FFmpeg missing and a recovery offer.
    #[test]
    fn t984_primary_tile_lands_on_the_most_likely_action() {
        let fresh = TourProgress::default();
        let mut finished = TourProgress::default();
        finished.touched = 100;
        let mut stopped = TourProgress::default();
        stopped.done.entry(1).or_default().extend([1, 2, 3]);
        stopped.done.entry(2).or_default().insert(1);
        stopped.touched = 100;
        let alive = [(row("previous", 50), true)];
        let dead = [(row("previous", 50), false)];
        let newer = [(row("previous", 200), true)];
        let base = StartFacts {
            cli_project: false,
            recovery: false,
            ffmpeg_missing: false,
            show_at_start: true,
            first_start: false,
            tour: &fresh,
            tour_next: None,
            recent: &[],
            opened_since_tour: false,
            last_template: None,
        };
        let win = |t: Tile| StartAction::Window { primary: t };

        // 1. the first start (an empty store): the tour
        assert_eq!(primary(&StartFacts { first_start: true, ..base.clone() }), win(Tile::Tour));
        // 2. the tour finished, the previous project exists: the previous project
        let done = StartFacts { tour: &finished, recent: &alive, opened_since_tour: true, ..base.clone() };
        assert_eq!(primary(&done), win(Tile::Recent(0)));
        // 3. the same with the file deleted: a new project from the last-used template;
        //    the row is not removed (the window draws it flagged)
        let gone = StartFacts { recent: &dead, ..done.clone() };
        assert_eq!(primary(&gone), win(Tile::NewFromTemplate("v916".into())));
        assert_eq!(gone.recent.len(), 1, "the missing row stays");
        let gone_tpl = StartFacts { last_template: Some("4k30"), ..gone.clone() };
        assert_eq!(primary(&gone_tpl), win(Tile::NewFromTemplate("4k30".into())));
        // 4. a tour stopped at lesson 2, no project opened since: continue the tour
        let paused = StartFacts {
            tour: &stopped,
            tour_next: Some((2, 2)),
            recent: &alive,
            opened_since_tour: false,
            ..base.clone()
        };
        assert_eq!(primary(&paused), win(Tile::ContinueTour { lesson: 2, step: 2 }));
        // 5. the same, a project opened since: the previous project
        let moved_on = StartFacts { recent: &newer, opened_since_tour: true, ..paused.clone() };
        assert_eq!(primary(&moved_on), win(Tile::Recent(0)));
        // 6. a project on the command line: no welcome window
        assert_eq!(primary(&StartFacts { cli_project: true, ..done.clone() }), StartAction::NoWindow);
        // 7. a pending recovery offer: no welcome window (the recovery first)
        assert_eq!(primary(&StartFacts { recovery: true, ..done.clone() }), StartAction::NoWindow);
        // 8. FFmpeg missing: its window first, the welcome window when it closes
        assert_eq!(
            primary(&StartFacts { ffmpeg_missing: true, first_start: true, ..base.clone() }),
            StartAction::AfterFfmpeg(Box::new(win(Tile::Tour)))
        );
        // 9. "Show at start" off: no window, the last project opens as before
        assert_eq!(primary(&StartFacts { show_at_start: false, ..done.clone() }), StartAction::NoWindowOpenLast);
        // the first start beats every other fact but the command line / recovery / the switch
        assert_eq!(
            primary(&StartFacts { first_start: true, show_at_start: false, ..base.clone() }),
            StartAction::NoWindowOpenLast
        );
        // no recent and the tour never started — the tour; started and stopped — continue it
        assert_eq!(primary(&base), win(Tile::Tour));
        let no_recent = StartFacts { recent: &[], opened_since_tour: true, ..paused.clone() };
        assert_eq!(primary(&no_recent), win(Tile::ContinueTour { lesson: 2, step: 2 }));
        // finished tour, no recent: a new project from the default template
        let empty_done = StartFacts { tour: &finished, ..base.clone() };
        assert_eq!(primary(&empty_done), win(Tile::NewFromTemplate(DEFAULT_TEMPLATE.into())));
    }

    #[test]
    fn templates_resolve_builtin_and_user() {
        let user = [UserTemplate { name: "Shorts 60".into(), width: 1080, height: 1920, fps: 60 }];
        assert_eq!(template_by_id("v916", &user), Some((1080, 1920, 30)));
        assert_eq!(template_by_id(&user_template_id("Shorts 60"), &user), Some((1080, 1920, 60)));
        assert_eq!(template_by_id("nope", &user), None);
        let p = project_for(1080, 1920, 30);
        assert!(p.settings.settings_explicit && p.settings.sample_rate == 48000);
    }
}
