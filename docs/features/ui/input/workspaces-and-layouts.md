---
id: F-UI-003
title: Window layouts — presets by task and where the layout lives
status: candidate
priority: B
cost: M
category: ui/input
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: partial
crates: [kt-app, kt-model, kt-otio]
invariants: []
ui_rules: [4, 5, 6, 13, 19, 20]
round: null
point: null
tests: []
depends_on: [F-UI-002]
trackers: [kde-bugzilla, kde-invent, olive, pitivi, shotcut]
checked: 2026-09-08
---

# F-UI-003 — Window layouts — presets by task and where the layout lives

## Essence

The editor arranges windows for the TASK: when sorting material a large bin is needed,
when mixing — meters at full height, when colour correction — the preview and scopes, and
the timeline as a single strip. A layout is a set of named states between which one
switches with a single press, not "set it up once and live with it". Hence three subjects:
PRESETS (ready-made layouts by stage of work), SAVING one's own, and RESET to the factory
one. And one question beneath all three: does the layout belong to the PROGRAM or to the
PROJECT — that is, does it travel with the file to another machine.

## Who has it and how

| Editor | Version | What it is called | How it is built |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Window Layouts | up to **ten** slots; `Ctrl+Alt+D` + digit saves, `Alt+D` + digit recalls. Files `.VegasWindowLayout` in `AppData\Roaming\Vegas\VEGAS Pro\NN` — that is, they belong to the PROGRAM and are transferred by copying. "Reload Selected Layout" returns to its last save; there is no return to the factory state in the help |
| Adobe Premiere | 26.x | Workspaces | a bar of panels with names by stage (Assembly, Editing, Color, Effects, Audio, Graphics). The layout at the moment of saving is **written inside the project file**, and `Window → Workspaces → Import Workspace from Projects` (on by default) decides whose wins on opening |
| DaVinci Resolve | 21 | Pages + Layout Presets | a **page** (Media/Cut/Edit/Fusion/Color/Fairlight/Deliver) is a change of TOOL, not an arrangement of windows; each page has its own layout. On top — `Workspace → Layout Presets`: save/load/delete, **Export/Import Preset** as a file for another installation, and a separate `Workspace → Reset UI Layout` |
| Final Cut Pro | 12.3 | Workspaces | `Window → Workspaces`: system ones (Default, Organize, Color & Effects) and one's own — "Save Workspace as…", "Update … Workspace". `Open Workspace Folder in Finder` shows them as files in the user settings. System ones cannot be changed, nor the default one; there is no layout library |
| After Effects | 26.x | Workspaces | a list by task (Standard, Small Screen, Motion Tracking, Paint, Text, Effects, Color…), "Reset to Saved Layout" — return to the saved view. The layout has been **embedded in `.aep`** for decades; the switch `Maintain current workspace when opening projects` (Settings → Project) appeared only in the 2026 release |

**Outside the core.** Kdenlive is the only source where the fork "program or project" has
been lived through publicly and entirely: the request, the refusal, the repeated request
and the implementation lie in the tracker with dates (the `_editors.md` rule "the program
is the ORIGIN of the mechanism").

| Program | Version | What it is called | How it is built |
|---|---|---|---|
| Kdenlive | 26.08.0 | Workspace Layouts | five factory layouts named by a STAGE of work — Logging, Editing, Audio, Effects, Color; toggle buttons in the top-right corner; `View → Save Layout`, a layout manager (delete, reorder, reset, import/export). Stored in `~/.config/kdenlive-layoutsrc` — belongs to the PROGRAM, not to the project |

## The trick behind it

**The fork is not resolved by choosing a side — the industry stores the layout in BOTH
places and sells a switch that decides whose wins.** In Premiere this is
`Import Workspace from Projects`, on by default: the project carries the layout, but the
user may forbid it to override him. After Effects kept the layout in `.aep` for decades
WITHOUT a switch, and the switch arrived only in the 2026 release — seven years after the
problem was formulated on a forum. The practical conclusion for us is the reverse of
intuition: what is expensive is not "saving the layout into the project", what is expensive
is a mandatory second mechanism that ignores it from there. Whoever puts the layout into the
file must ship both an off switch and the instruction "if the layout arrived empty — switch
the import off and open again".

**A layout switcher is useless while layouts are named by number.** VEGAS recalls a layout
with the chord `Alt+D` + DIGIT: ten slots, and what is in the fourth — only the user remembers.
Kdenlive and Resolve name them by stage of work (Logging / Editing / Audio / Effects / Color),
and then the switcher stops being a setting and becomes the TABLE OF CONTENTS of the workflow:
a newcomer reads from the buttons what editing consists of at all. This is exactly the push
channel of the second wave (conclusion 1 of the README): showing what the user did not look
for, without covering anything. The factory list in the request kde-invent #407 was written
BEFORE any interface — six stage names, and only afterwards "add an indicator which layout is
active".

**"Reset" is requested more often than "save".** Of the ten tasks of the filter in
kde-bugzilla, six are about reset rather than saving: kde-invent #249 (2019) asks for a button
"reset layout to default", kde-bugzilla #497207 (December 2024) asks the same question five
years later and on the same day collects the duplicate #497183. The asymmetry is mechanical:
breaking a layout is one wrong drag, assembling one takes minutes. Moreover, "reset" in the
VEGAS help means "return to ITS OWN last save", not to the factory one, and these are different
commands which users confuse.

## Pros

- A layout in the PROGRAM — a foreign project does not rearrange the furniture. You open a
  colleague's file and work with your own hands in your own window; no class of defects of the
  kind "the project arrived with a layout made for a 4K monitor" exists at all.
- A layout in the PROJECT is the only way in which "put the meters wider for this mix" travels
  with the work to another machine. For an editor who moves between a home and a work computer
  this is not a convenience but a saved part of the work.
- Presets named by stage give a free push channel: the names of the buttons tell about the
  workflow without taking a separate place (rule 13).
- Exporting a preset as a file (Resolve, VEGAS) separates "my layout" from "my project": the
  layout is transferred by a deliberate action, not as a side effect of opening someone else's file.

## Cons

- Storing in the project makes the interface setting part of the file's CONTENT — and this is the
  only place where the look of the program can conflict during an exchange. Hence the second system
  (the import switch), and hence Adobe's official instruction for the case "I imported the project
  and the layout is empty".
- Storing in the program loses the link with the task: a layout "for the mix" and the mix project
  are two independent subjects, and switching them has to be done by hand twice (open the project,
  choose the layout).
- Number slots (VEGAS) save development effort and shift memory onto the person; after a month's
  break ten slots are ten unknowns.
- Presets by stage work only if the stage matches the STRUCTURE of the program. In Resolve it
  matches (the page = the engine); in Premiere it no longer does: "Color" there is simply another
  arrangement of the same panels, and the difference between a "page" and a "layout" is not visible
  at a glance until it turns out that a tool is unavailable on another page.
- A separate layout for each stage doubles the surface of reset: in Resolve "Reset UI Layout" and
  "Load Preset" are different menu items with a similar result.

## What users say

> THIS REPORT IS CLOSED AS WONTFIX; DO NOT REOPEN IT. Some extensive googling
> shows that this is a general issue with Qt. I'm now closing this report as we
> can't fix this issue on a reliable basis.
> — Wegwerf, kde-bugzilla #343322 "Workspace layout is not remembered / saved to
> project KF5", 27.10.2016, https://bugs.kde.org/show_bug.cgi?id=343322

The refusal is remarkable for what it does NOT contain. The maintainer did not analyse the fork
"program or project" at all: he replied that the saving mechanism could not be made RELIABLE, and
closed the question together with the subject. A substantive answer came four years later and from
the other side — kde-invent #407 (2019, closed 27.07.2020) brought six named factory layouts, user
ones of one's own, "reset current layout" and the file `$HOME/.config/kdenlive-layoutsrc`, that is,
it chose the PROGRAM. The switcher that grew out of this is still being redone:
kde-invent #1999 "Redesign Layout switcher in top-right corner" was opened on 19.04.2025 and is
still alive.

> workspace layouts are also saved with / tied to individual AEP project files.
> When I open up an old project in After Effects, the old layout saved with that
> project opens up as well, which I usually don't want.
> — Paul Hasas, Creative COW, 08.09.2019,
> https://creativecow.net/forums/thread/avoid-loading-old-workspace-when-opening-an-old-pr/

This is the price of the "project" side, named by a user seven years before Adobe issued the switch.

> Is there no feature to reset the Kdenlive interface to its default layout after
> distributing and mixing windows?
> — mouradbenhelli, kde-bugzilla #497207, 08.12.2024,
> https://bugs.kde.org/show_bug.cgi?id=497207

Closed the next day as NOT A BUG with a link to the forum; the duplicate #497183 was merged there the
same day. The reset mechanism exists in the program — and yet the question keeps being asked, because
people cannot find it.

SIGNAL: a layout as a named preset — 10 tasks in kde-bugzilla, `(preset|custom|default|save|switch|reset|restore|manage) ?\w* ?layouts?|layouts?:? ?\w{0,12} ?(preset|switcher|profile|manager|reset|default|template)|workspaces? (preset|layout|switch)`; share 0.2% against 0.0% (not one of 2672) in pitivi

SIGNAL: panels and layout in general — 37 tasks in olive, `layout|workspace|dock(ed|able|ing)?\b|panel`; share 2.0% against 0.4% in pitivi

The second line is exactly the "spread" for which the topic was chosen: in Olive, where the arrangement
of panels is free and young, the share is five times higher than in Pitivi, where the layout is rigid and
there is nothing to argue about. The topic "interface" in `--wishes` holds a `gap` of 5.4 at a `universal`
of 0.2% — that is, the pain is not structural but depends on the DEVICE of the particular program. For us
this is a direct hint: the price of the question for us is determined by what we have built, not by "that
is how everyone does it".

## How it fits Kinotrain

### What already exists

**The answer to the fork is already accepted with us — and it is SEPARATE.** The boundary runs
along `dirty_content` / `dirty_ui` (`src/apps/kt-app/src/app.rs:52` and `:56`), and physically
these are two different files:

1. **In the PROJECT** (`.otio`, `metadata["kt"].ui`) — the structure `kt_model::UiState`
   (`src/libs/kt-model/src/project.rs:1521`): `view_start`, `ticks_per_px`, `cursor`, `bin_free`,
   `bin_layout`, `bin_groups`, `lane_names`, `lane_heights` and — the only geometry of a panel —
   `meters_w`. It is written as a snapshot by `KtApp::ui_state()` (`app.rs:750`), read on opening
   (`app.rs:695`–`:713`) with clamping of `ticks_per_px` and `meters_w`. It does not light the
   asterisk, does not trigger autosave, and is carried along in passing with the save and in
   `on_exit` (`app.rs:5188`).
2. **In the PROGRAM** (`~/.local/share/kinotrain/app.ron`, eframe Storage; `eframe` is connected
   with the `persistence` feature, `src/Cargo.toml:42`) — `lang`, `last_project`
   (`app.rs:5174`–`:5186`) and, automatically, the whole memory of egui: the window geometry and the
   panel sizes. The file on the machine was checked: `"window"` with `inner_size_points:(1440,860)`,
   the `PanelState` of the lower timeline `(0,540)-(1440,840)` and of the left column `(0,0)-(232,540)`.

The layout itself with us is **fixed in code**, not dockable: the left column
`Panel::left("left_col")` (`app.rs:868`, min 72, max 70% of the width), the lower
`Panel::bottom("timeline")` (`app.rs:841`, ceiling `available_height − 140`), the right
`Panel::right("meters_col")` (`app.rs:925`) with its own drag strip and clamping `METERS_W_MIN/MAX`,
the status bar `exact_size(20)`, the rest is `CentralPanel`. Panels cannot be dragged, hidden or
swapped — they can only be pulled by the edge. All of them have limits (rule 6), and the limit of the
left column is explicitly marked as set following a user complaint ("do not compress without a new
request", `app.rs:860`).

Test **t129** (`src/apps/kt-app/src/layout_tests.rs:1773`) guards the technical premise of any
layout: the windows `kt_notices`, `kt_search`, `kt_diag` have an explicit `.id()`, because
`Window::new(title)` derives the `Id` from the TITLE — under a translated title egui would start a
new state, and the window position would be lost on every language change. That is, we already have
the invariant "window state lies under a permanent key, not under the text" — without it a named layout
would not survive even a language switch.

### What is missing

- **Not one named layout and not one switcher.** Presets by stage (analysis / editing / sound / output)
  are cheap for us to express: our "layout" is not a tree of docks, but a handful of numbers
  (`meters_w`, the width of the left column, the height of the timeline, `tl_v_zoom`,
  `preview_detached`) plus visibility flags. A preset is a named tuple, not a serialised layout engine.
- **There is no "reset layout" command.** In the catalog `src/apps/kt-app/i18n/ru.toml` there are
  `reset_height`, `reset_gain`, `graph_reset` — and nothing about the interface. If you drag the lower
  timeline almost to zero, the only way back is by hand, by erasing
  `~/.local/share/kinotrain/app.ron`, and together with it the language and the "last project" fly away.
  This is exactly the task that was requested twice in Kdenlive with a five-year difference.
- **`preview_detached` is stored nowhere** (`app.rs:425`, `:565`): the most radical change of a layout —
  moving the preview to a second monitor — is set only by the variable `KT_DETACH_PREVIEW` and dies on
  exit. Neither the project nor `app.ron` remembers it. The same goes for `notices_open`, `search_open`,
  `perf.diag_open`: the window position survives a restart, the fact "it was open" does not.
- **A user storage outside the project does not exist as an OBJECT** — this is the conclusion 2 of the
  first wave (`README.md`), where F-UI-002 already ran into it. A refinement of the reconnaissance: the
  place exists (`app.ron`), it holds two lines and the memory of egui, and nobody except eframe puts
  anything there.

### What gets in the way

`meters_w` sits on the WRONG side of our own boundary, and this is the only structural obstacle. The
width of the meters panel is window geometry, but it lives in the project, because the standard
`resizable` of this panel is switched off (`app.rs:919`, the egui grip was stealing clicks from the
window buttons) and egui does not remember it. As a result two neighbouring panels of one window are
stored in two files with different lifetimes: you open someone else's project — the meters take the
width of someone else's, while the left column stays at yours. Until this one anomaly is resolved
consciously, "layout" has no single address, and a preset would have to be assembled from two sources.

The second obstacle is milder: without docking, a "layout for colour correction" can with us change only
sizes and visibility. This does not hinder presets, but it sets them a ceiling — a comparison with the
Resolve pages would be dishonest, and promising "like the pages" is not allowed.

### Which invariants it touches

Invariants 1–12 of the root `CLAUDE.md` — **not one**: the layout touches neither sound,
nor determinism, nor the composition; this is a strong argument for "cheap". UI checklist
rules: **4** (a useful default — the factory layout must be working, not empty), **5**
(switching the layout must not extinguish the neighbouring state: region, loop, selection,
playback), **6** (the limits already stand and, following a user complaint, are not
compressed), **13** (the switcher must sit on an ALREADY occupied place — a toolbar/title-bar
row, not a new strip; Kdenlive keeps its own in the corner of the same row), **19** (the dialog
must say where to return — this concerns exactly the reset), **20** (nothing grows by itself:
presets are a closed list plus explicitly saved ones, without "automatically remembered and
substituted").

ADDRESS: `src/apps/kt-app/src/app.rs:759` (`meters_w` in `UiState`) against `src/apps/kt-app/src/app.rs:5174` (`eframe::App::save` writes only `lang` and `last_project`) — the class "an interface setting travelled with someone else's project" is POSSIBLE with us on exactly one field, and nothing catches it: the width of the meters panel travels in the `.otio` and, on opening, overrides mine (`app.rs:709`), whereas the width of the left column and the height of the timeline lie in `~/.local/share/kinotrain/app.ron` and do not travel with the project; t101 checks the strip and the clamp, t129 checks the permanence of window ids, but WHICH file owns which panel is checked by no test.

## Estimate

| What | Answer |
|---|---|
| Volume | M — 2–3 stages: separate the ownership (project / program), presets + switcher, reset |
| Where the main work is | kt-app (`app.rs`: snapshot and application of the layout, the switcher in the toolbar), kt-model/kt-otio — only if the layout DELIBERATELY stays in the project |
| Project format | not touched if the layout moves wholly into the application settings; a format version is needed only for the opposite decision (moving `meters_w` out of `UiState` — removing the field, the reader must survive old files) |
| Tests that will have to be written | kt-app `layout_tests.rs`: switching a preset does not extinguish region/loop/selection (rule 5); "reset layout" returns EXACTLY the factory numbers and does not touch the language and the last project; opening someone else's project does not move the panel widths; t101 and t129 must stay green without edits |
| Depends on | F-UI-002 (the general "storage outside the project"), conclusion 2 of the README |
| Breaks compatibility | no; when `meters_w` is moved out of `UiState` — the reader of old files must silently ignore the field |

## Verdict

Take it, but not as a "layouts feature", but as a RECONCILED OWNERSHIP plus two cheap
consequences. The fork is already resolved with us in favour of separation — the convenience of
viewing (zoom, cursor, bin, track names and heights) is in the project, the window geometry is in
the application settings — and the trackers confirm that this is the right side: the only vendor
that went through the question publicly, over ten years arrived exactly here (`kdenlive-layoutsrc`),
and both who put the layout into the project were forced to pay extra with a switch — Premiere at
once, After Effects in 2026. So the work is not to decide anything, but to **fix one exception**
(`meters_w`) and name the rule aloud before there are more panels.

Priority **B**: we have one window, a fixed grid of panels and one user on one machine — "the layout
travelled with someone else's project" does not threaten him today. But two consequences are worth
their money independently of presets: **"reset the layout"** (today physically unreachable — only by
deleting `app.ron` together with the language) and **memory of `preview_detached`** (a second monitor
survives exit in all five editors, with us it does not). Presets by stage are the upper, most expensive
half, and we honestly defer it until hideable panels appear: while panels are only pulled by the edge,
"the layout for sound" differs from "the layout for editing" by three numbers, and the switcher has
nothing to show.

## Sources

- https://bugs.kde.org/show_bug.cgi?id=343322 — WONTFIX "Workspace layout is not
  remembered / saved to project KF5" (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/407 — "Layouts: add
  preset layouts and customized layout", six stage names and
  `kdenlive-layoutsrc` (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1999 — "Redesign Layout
  switcher in top-right corner", open since 19.04.2025 (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/249 — "add button:
  reset layout to default" (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=497207 — "reset the Kdenlive interface to
  its default layout", NOT A BUG + duplicate #497183 (2026-09-08)
- https://docs.kdenlive.org/en/user_interface/workspace_layouts.html — five factory
  layouts by stage (2026-09-08)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/2-window/window_layouts.htm
  — VEGAS: slots by digits, `.VegasWindowLayout` in AppData (2026-09-08)
- https://helpx.adobe.com/premiere/desktop/get-started/tour-the-workspace/import-workspaces.html
  — Premiere: the layout inside the project, `Import Workspace from Projects`
  on by default. The page itself did not open (the proxy refused); the details come from
  the search results for it (2026-09-08)
- https://creativecow.net/forums/thread/avoid-loading-old-workspace-when-opening-an-old-pr/
  — a quote from an AE user, 08.09.2019 (2026-09-08)
- https://helpx.adobe.com/after-effects/using/workspaces-panels-viewers.html —
  AE: workspaces and "Reset to Saved Layout"; the switch `Maintain current
  workspace when opening projects` (Settings → Project) since the 2026 release. The page
  did not open (the proxy refused); the details come from search results (2026-09-08)
- https://support.apple.com/guide/final-cut-pro/change-the-workspace-layout-ver6b3626147/mac
  — FCP: "Save Workspace as…", "Open Workspace Folder in Finder" (2026-09-08)
- Resolve: `Workspace → Layout Presets` (Save/Load/Export/Import) and
  `Workspace → Reset UI Layout` — from the Resolve manual and secondary analyses; the official
  manual page (a steakunderwater mirror) returned 404 (2026-09-08)
- Numbers: `python3 src/scripts/tracker-rank.py --query …` over the dumps
  `~/.cache/kinotrain/trackers/` of 08.09.2026 (9 dumps, 18445 records;
  `--check` is green). `blender-vse`, `openshot` and `ffmpeg-trac` were absent from the cache
  or outside the ranking at the time of writing — they do not take part in the shares;
  `flowblade` was exported and in the broad filter gives 10 tasks out of 788 (1.3%).
