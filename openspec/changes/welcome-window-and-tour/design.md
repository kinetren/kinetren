# Design

## Context

See proposal.md — Why. Observed facts (read-only, tree at 345c94e):

- **Start order** (`src/apps/kt-app/src/app.rs:9120–9165`, inside `run`): language, theme, fonts,
  panic hook, telemetry install; then `restored = arg.or(storage "last_project").filter(exists)` →
  `KtApp::open` or `KtApp::new_empty(Some(msg::OPEN_A_PROJECT))`; then the crash-report warning,
  `ffmpeg_startup_check()` (`src/apps/kt-app/src/ffmpeg_setup.rs:426` — opens the FFmpeg window when
  `backend_info()` is none), the temp-data-dir warning, `offer_recovery(true)` (`app.rs:2994`,
  `recovery_open` when candidates exist). `last_project` is written in `save` (`app.rs:9006`).
- **An empty project** is 1280×720@30 (`app.rs:669`, `empty_project`), drawn as a full editor.
- **Menus** (`src/apps/kt-app/src/toolbar.rs`): File (`:329`; holds "Create a generator…" through the
  shared `gen_ui::generator_menu`, `:350`), Edit (`:400`), View (`:459`; language, diagnostics,
  notices, FFmpeg). There is **no Help menu**.
- **Window registry** `WINDOWS` (`src/apps/kt-app/src/reach_tests.rs:2644`, guard t757): 22 entries;
  a new `egui::Window`/`Modal` needs an entry with a reason ≥ 25 characters.
- **Repaint** (`app.rs:2209`): `animating` = playing ‖ space held ‖ drag ‖ render job ‖ meters active;
  at rest `repaint_delay(false)` (`app.rs:7733`, t20).
- **Layers above `Order::Middle`** today (F-HLP-028, ui-reach design): eight named layers (status hint,
  project title, ambient overlay, speed-drag overlay, bin group name, marker/lane/area name editors);
  t83 checks only `Order::Tooltip`; the guard `reach_tests::no_teaching_layer_over_the_ui` named by
  F-HLP-028 does not exist yet.
- **The user store** (`src/apps/kt-app/src/user_store.rs`): `store_dir()` (`:31`, the live rig's
  directory, `KT_USER_STORE_DIR`, else `journal::data_dir()`), presets with a factory file, the
  workspace with future-version read-only and `.broken` rules (`load_workspace_at`/`save_workspace_at`,
  `:531`/`:568`).
- **The offline reel-preview path** (`src/apps/kt-app/src/reel_preview.rs`): a service with a queue,
  its own short-lived sources (invariant 5), a pause during playback, frames 320×180, disk tier in the
  project's sidecar `<project>.ktindex.db` (R.58). `image` with the `png` feature is already a kt-app
  dependency.
- **The demo edit** is `demo_project(synth: bool)` in `src/apps/kt-cli/src/main.rs:1167`, unreachable
  from the GUI (F-HLP-029).
- **The guide**: `docs/guide/` — getting-started, timeline, effects-and-automation, morpher,
  compositions-and-generators, export, shortcuts (464 lines, English, guarded by
  `src/scripts/readme-check.py` — capability `showcase`, t826). `shortcuts.md` uses `<kbd>` tags and
  tables; `getting-started.md` shows one image.
- **Update check** (`openspec/changes/update-check`, not applied yet): the day's answer lives in
  `<data_dir>/update-check` (`day=`, `latest=`), decided by `decide`/`due_in` in a future
  `src/apps/kt-app/src/update_check.rs`.
- **Notes**: F-HLP-027 (welcome screen; verdict "a start screen as a separate window — no",
  `TEST: reach_tests::first_run_instruction_outlives_the_status_line`), F-HLP-028 (tour — rejected,
  with the lesson form deferred until F-UI-002), F-HLP-023 (manual — addresses first), F-HLP-029 (demo
  project — "a COPY in the folder the user named"), F-HLP-021 (what's new).

## Goals / Non-Goals

**Goals:**
- One window answers the first second: continue, start new, learn.
- A lesson knows when its step is done without asking the person to press "Next".
- The guide is reachable from the program from ONE source of text.

**Non-Goals:**
- A coach-mark layer, pulsing highlights, dimming, or any animation.
- A command registry (F-UI-002) or a keymap editor.
- Translating the guide pages; a site manual (`site/`); network access of any kind.
- "Open demo project" as a person's own project in a folder he names (F-HLP-029 stays a candidate;
  the lessons use a program-owned copy).

## Decisions

### D1. Revision: a start window is taken (F-HLP-027 verdict)
F-HLP-027 decided "a start screen as a separate window — no: at launch we do not have an empty window
but a drawn editor, and a modal over it violates rule 13". Revision: **the welcome window is taken,
switchable, and the former start stays one checkbox away** — reason: the owner's direct request of
2026-10-28; the note's own concern is answered by (a) opening over an EMPTY project only (a project
on the command line or a recovery skips it), (b) the primary tile making the common case one Enter,
(c) "Show at start" off restoring exactly today's silent open, named in the window and in View (UI
rule 19, "say where to get it back"). The note's smaller half (a visible recent list) is delivered
inside the window.

### D2. Revision: lessons in their own view are taken; the layer stays rejected (F-HLP-028, ui-reach)
`openspec/specs/ui-reach/design.md` "Rejected and why" names three stops for a tour. Revision:
**lessons in a docked side panel, steps closed by predicates over the project's state, are taken;
coach marks over the interface stay rejected** — reason: the owner's request of 2026-10-28, and each
stop is answered without weakening it: (1) repaint — nothing animates; a predicate is re-evaluated
only when the model revision or the open-window set changes, never per frame at rest; (2) "one hint"
— the panel is a panel (`SidePanel`), not a layer; the new guard `no_teaching_layer_over_the_ui`
turns the closed list of layers above `Order::Middle` into a test (t988), which also closes the hole
F-HLP-028 found in t83; (3) "nothing to aim at" — a step does not aim at pixels: it closes on the
project's state, and "Show" names a registry id (a window of `WINDOWS`, a panel, a menu path of
catalog keys). The addressee objection (the owner's complaints came mid-work) is answered by the
Guide window (D9), which serves the middle of work; the lessons serve the first start.

### D3. The welcome window is a registered window inside the main viewport
`kt_welcome` joins `WINDOWS` ("start: templates, recent projects with posters, the tour, what's new —
the first second has no project yet"). It is an `egui::Window` centred in the main viewport, not a
second OS viewport: F-HLP-027's ADDRESS line found that a second top-level window brings tiling-WM and
Alt+Tab defects (kde-invent #2124, kde-bugzilla #513581), and the detached preview is the only one we
have. It obeys the form contract (t779: the body scrolls, the actions stay outside the scroll).

### D4. The primary tile is a pure function
```
fn primary(f: &StartFacts) -> StartAction
StartFacts { cli_project: bool, recovery: bool, ffmpeg_missing: bool, show_at_start: bool,
             first_start: bool, tour: TourProgress, recent: &[RecentEntry /* with exists */],
             opened_since_tour: bool }
StartAction { NoWindowOpenLast, NoWindow, AfterFfmpeg(Box<StartAction>), Window { primary: Tile } }
Tile { Tour, ContinueTour { lesson, step }, Recent(i), NewFromTemplate(id) }
```
The order of the cases is the scenario of t984 and lives in one `match`; the file-existence facts are
gathered before the call (the UI thread does one `exists()` per recent entry at start — 12 at most).

### D5. Posters: rendered at save/close by the offline path, stored in the data directory
On a successful Ctrl+S of a named project and on closing a named project, a poster job is queued:
the same offline single-frame render the reel-preview service uses (own short-lived sources, the
`frame_plan` composite, paused during playback), at the playhead or at a tenth of the length, scaled
to 320×180, written as PNG to `<data_dir>/posters/<xxh3(canonical path)>.png` by tmp + rename. Not
into the sidecar: the sidecar is the project's content cache next to the project; a poster is the
program's list decoration and must not appear in the person's folder. The welcome window loads the
finished PNGs on a helper thread and uploads textures when they arrive (one repaint each, invariant
12); a missing poster is a placeholder with the project's initials. Clearing the list deletes only
poster files the list owns.

### D6. Templates
Built-ins are a `const` table in `welcome.rs` (id, label key, width, height, fps): 1080p25,
1080p30, 4K30 (3840×2160 30), 9:16 (1080×1920 30), 1:1 (1080×1080 30). User templates live in
`templates.json` of the store (name, width, height, fps), saved by "Save as template…" in the Project
settings window and deleted from the tile's context menu (a right click is never silent). A template
creates `Project::new(ProjectSettings { width, height, fps, sample_rate: 48000, ..default })` — audio
stays 48 kHz (invariant 1); `settings_explicit: true`, so the first imported clip does not resize the
project.

### D7. Lessons: data, predicates and targets are closed enums
`lessons.rs` holds a `const` table: lesson id, title key, steps `{ text_key, show: Option<Target>,
done: Predicate }`. `Predicate` is a closed enum evaluated by one pure function
`fn done(p: &Predicate, m: &Project, open: &OpenWindows) -> bool` — e.g. `HasGenerator(tag)`,
`ClipSplitAtMarker(name)`, `ChainHas(fx_tag)`, `LoopRegionSet`, `TransitionAt(marker)`,
`WindowOpen(id)`, `RenderDone`. `Target` is `Window(&'static str)` (an id of `WINDOWS`),
`Panel(Bin | Preview | Timeline | Meters)`, `Menu(&'static [Key])`. The guard (t988) checks every
target against `WINDOWS`, the panel list and the menu catalog, and every text against the five
catalogs and the catalog budget (t100/t128 — a paragraph that does not fit becomes two steps).
Proposed lessons (Open Question 4): 1 The first edit (title, cut, move, play a loop); 2 Effects and
automation (add a blur, put a curve on it); 3 Transitions (crossfade, change the kind); 4 The morpher
(an FX batch over two events); 5 Export (render the region).

### D8. The lesson project is a program-owned copy built from generators
`demo_project` moves from `src/apps/kt-cli/src/main.rs:1167` into a shared module the GUI can call
(kt-model, as a pure `Project` builder: it needs no media, only generator media `gen:<kind>`), in a
generator-only form, so every lesson project opens with real thumbnails and no missing sources (the
F-HLP-029 defect "eight nonexistent media" does not apply). Each lesson writes its copy to
`<data_dir>/lessons/lesson-<n>.kt` when started; "Start over" rewrites it. kt-cli's `demo` command
calls the same builder (one body, UI rule 17). F-HLP-029's "a copy in the folder the user named"
stays the rule for a future "Open demo project" — a lesson sandbox is not the person's project and
is not listed in Recent.

### D9. The Guide window: an index of catalog entries, pages compiled from `docs/guide/`
`kt_guide` joins `WINDOWS` ("a searchable index 'I want to… → where' and the guide pages — lives beside
the work"). The pages are `include_str!("../../../../docs/guide/<file>.md")` for a fixed list in
`guide.rs`; a guard compares the list with the directory (a new guide file must be added, or the test
says which). The renderer is our own small one over the subset the guide uses (headings, paragraphs,
lists, bold, inline code, `<kbd>` key caps, tables, links between guide pages; an image → its alt text
as a caption) — no new dependency; anything else in a guide file fails the guard with file and line.
The index lives in a `[guide.index]` catalog section: each entry has the "I want to…" text, the
"where" text, and an anchor `<file>#<heading-slug>` (the GitHub slug rule, so the same anchor works on
the repository page). Search is a case-insensitive substring over the translated entries and the
English page headings — no stemming (F-HLP-023: up to a few dozen topics, structure beats an index).

### D10. What's new and the update line
`docs/guide/whats-new.md` (English, one `## ktYY.MM` section per month, compiled in) is the source of
the panel; the running `kt_version::SHORT` selects the section, falling back to the newest with its
label. It is a guide file, so `readme-check.py` guards it. The update line reads the update-check state
file through that change's own reader and decision function (no request, no second parser); when
update-check is not applied yet, the line is simply absent.

### D11. "Show at start" and the Help menu
The choice is stored as `welcome_at_start` in the user store's start record (not eframe storage:
the welcome window's records travel together, t987). The View menu gets the checkbox; a new Help menu
(after View) holds Guide…, Start the tour, Welcome window…, What's new (opens the welcome window at the
What's new panel). All labels are catalog keys ×5 with `subset-fonts.py` run for new glyphs.

## Risks / Trade-offs

- [egui 0.36 may not open a `menu_button` popup programmatically, so "Show" for a menu target cannot
  open the File menu] → task 1.1 is a probe; if it cannot, a menu target opens the menu bar's top
  item and the status line names the item ("File → Create a generator…"), and the spec's PATH is
  adjusted before the code.
- [A poster job on every Ctrl+S costs CPU] → one 320×180 frame, coalesced (a pending job for the same
  project is replaced), paused during playback; measured on release in task 3.3.
- [The guide's English pages in a Russian UI] → labelled "English" in the window; the index is
  translated (Open Question 3).
- [Predicates miss a way of doing the step (the cut done by a context-menu item instead of S)] →
  predicates look at the project's state, not the gesture, so every door counts by construction.
- [The welcome window hides the FFmpeg window or the recovery offer] → D4 orders them; t984 covers it.

## Migration Plan

On the first start of the new build, `last_project` from eframe storage seeds the recent list once
(t985); "Show at start" defaults on, so the owner sees the window once and can switch it off. No
project file changes.

## Open Questions

1. **May a step draw an accent frame on its target widget?** DECIDED by the owner (2026-10-28):
   yes, as recommended below — a static outline drawn by the target itself; stage work includes it. `reach_tests::no_teaching_layer_over_the_ui`
   and invariant 12 forbid a layer and a repaint loop, not a static mark. Recommendation: yes, but only
   as a STATIC outline drawn by the target's own drawing code in its own layer — the same mechanism as
   the existing obstacle highlight of a refused gesture (`blocked`, `app.rs:518`) — for as long as the
   step is current, with no pulsation, no dimming and no arrow; the guard keeps the layer list closed.
   The spec delta gains the outline scenario when the stage is applied.
2. **Welcome window inside the main viewport or as its own OS window (JetBrains style)?**
   Recommendation: inside the main viewport (D3), because of the tiling-WM and Alt+Tab defects
   F-HLP-027 found.
3. **Guide pages in five languages?** Recommendation: not in this change — the index entries are
   translated, the pages stay English (capability `showcase`), labelled.
4. **The lesson list** of D7 (five lessons). Recommendation: as listed; the owner may reorder.
5. **Where the lesson copies live.** Recommendation: the program's data directory (D8), not a folder
   the person names; the person never saves into them by accident because they are not in Recent.
