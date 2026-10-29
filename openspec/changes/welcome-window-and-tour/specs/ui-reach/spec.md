# Spec Delta

## ADDED Requirements

### Requirement: A welcome window at start; one primary tile lands on the most likely action
When the program starts without a project on the command line, without a recovery offer, and with
"Show the welcome window at start" on (the default), it SHALL open the welcome window over an empty
editor (when FFmpeg is not found, the FFmpeg window comes first and the welcome window opens when it
is closed). The window SHALL hold: "New from template" tiles — the built-in templates (1080p25, 1080p30,
4K30, 9:16 vertical 1080×1920 30, 1:1 square 1080×1080 30) followed by the person's own templates;
the recent projects, newest first, each with its poster thumbnail (or a placeholder until it exists),
its name and its folder; a "Start the tour" tile (or "Continue the tour — lesson N" once started);
a link to the Guide; and a What's new panel for the running version. Exactly ONE tile SHALL be
primary — drawn larger and focused, so Enter takes it — and it SHALL be chosen by a pure function of
(is this the first start, the tour progress, the recent list with each file's existence): the tour on
the first start; the previous project when it exists; "Continue the tour" when a tour was left
unfinished and no project was opened since; a new project from the last-used template when the
previous project is missing (the missing row stays in the list, says where the file was, and
offers to locate it — it is not silently dropped). A click on a template SHALL create an unnamed
project with that frame size and rate in the same window, without a question unless there is unsaved
work. The What's new panel SHALL show the notes compiled into the binary for the running version
(`ktYY.MM`), or the newest notes with their version label when the running one has none, and SHALL
show one calm line "Kinetren <version> is out — Releases page" when the update-check state already on
disk says a newer release exists; the window MUST NOT make a request of its own. The window SHALL
carry the checkbox "Show at start"; unchecking it restores the former start (the last project opens
silently) and the same checkbox lives in the View menu, where the welcome window can be reopened from
Help → Welcome window…. Nothing in the window SHALL animate: it repaints on input only.
Why: the owner (2026-10-28) asked for an IDE-style intro window with templates, recent projects with
thumbnails, a tour tile, a primary tile for the most likely action, and What's new with an update
hint. F-HLP-027 had found that the only first-run instruction (`msg::OPEN_A_PROJECT`) dies in the
status line after 6 s and that one invisible recent path was kept; its verdict "a start screen as a
separate window — no" is revised by this request (design, D1).

#### Scenario: The primary tile for each case
- **WHEN** the choice function is asked for: a first start (empty store); a start after the tour was
  finished with a recent project that exists; the same with the project file deleted; a tour stopped
  at lesson 2 with no project opened since; a tour stopped at lesson 2 and a project opened since; a
  project on the command line; a pending recovery offer; FFmpeg not found at start; "Show at start" off
- **THEN** in order: the tour; the previous project; a new project from the last-used template (and
  the missing row is flagged, not removed); "Continue the tour — lesson 2"; the previous project; no
  welcome window; no welcome window (recovery first); the FFmpeg window first and the welcome window when
  it closes; no welcome window and the last project opens as before
- **PATH** there is no gesture: the person starts Kinetren and sees the window with the primary tile
  already focused — on the first start "Start the tour", later the previous project's poster tile
- **TEST** t984 (`src/apps/kt-app/src/welcome.rs`)

#### Scenario: The welcome window — templates, recent, What's new, the switch
- **WHEN** the harness starts with a user store holding two recent projects (one with a poster, one
  without), one user template "Shorts 60", an update-check state that names a newer release; then
  clicks the "9:16" tile; then reopens the window from Help and clicks the recent project; then
  unchecks "Show at start" and restarts
- **THEN** the window shows the five built-in tiles, then "Shorts 60", the two recent rows (one poster,
  one placeholder) with names and folders, the tour tile, the Guide link and What's new for the running
  version with the line about the newer release; the 9:16 click gives an unnamed 1080×1920 30 fps
  project; the recent click opens that project; after the restart no window opens and the last project
  is open; no frame is drawn while the mouse is still (repaint on input only)
- **PATH** start Kinetren → the welcome window over an empty editor: a row of template tiles
  "1080p25 · 1080p30 · 4K30 · 9:16 · 1:1 · Shorts 60", below it "Recent" with two cards (a frame of
  the project and a grey placeholder), on the right "Start the tour", "Guide" and "What's new in
  kt26.10" ending with "Kinetren kt26.11 is out — Releases page" → click "9:16" → the window closes,
  the preview is tall and narrow, Project settings would say 1080×1920 30 fps, the title says
  "Untitled" → Help → Welcome window… → the window again → click the first recent card → that
  project opens → Help → Welcome window… → uncheck "Show at start" → quit and start again → the last
  project opens directly, no window
- **TEST** t986 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: The tour is lessons in their own view; a step closes on the project's state
The tour SHALL be a set of lessons shown in a docked side panel of the main window (it narrows the
editor, it does not cover it); a lesson is an ordered list of steps, each a short paragraph of the
translation catalogs, an optional "Show" button and a done predicate. A step SHALL close by itself
when its predicate over the project's state and the program's observable state becomes true (for
example: the project holds a title event; the event at the marker is cut in two; a chain holds a blur;
a loop region is set; the window of a registry id is open) — never by "Next" alone and never by a
command registry. "Show" SHALL open or focus the step's target: a window of the `WINDOWS` registry, a
panel, or a menu path; every target named by a lesson SHALL exist in the registry or the menu catalog,
and a lesson whose target is missing SHALL fail the build's guard rather than show a dead step. The
lessons SHALL run on a fresh COPY of the built-in demo project, built from generators only (no media
files), placed in the user data directory and never in the person's own folders; "Start the tour"
asks about unsaved work like any other open. Progress (finished steps per lesson) SHALL live in the
user store and SHALL be reset by Help → Start the tour → "Start over". No layer above the window
layer SHALL explain the interface: the program's layers above `Order::Middle` are a closed, named
list, and a lesson adds none.
Why: the owner's request (2026-10-28) revises F-HLP-028: of the three forms the reconnaissance looked
at, the one it deferred — a lesson that CHECKS the done action, in its own view (Ableton Learn View) —
is taken; the coach mark over the interface stays rejected (invariant 12, the "one hint" rule, and
t83 catches only `Order::Tooltip`). The deferral reason "there is no command registry" (F-UI-002) is
removed by closing steps on the project's state, which the model already exposes.

#### Scenario: A lesson step closes on the project's state and "Show" reaches the target
- **WHEN** the harness starts the tour from the welcome tile; in lesson 1 the step "Add a title" is
  shown and its "Show" pressed; a title is added through File → Create a generator… → Title; the next step "Cut
  the event at the marker" is done with S at the marker; the panel is closed and the program
  restarted; then "Start over" is used
- **THEN** a demo copy opens from the data directory (generators only, no missing media); "Show"
  opens the File menu and "Create a generator…" carries the static accent outline (the program
  never moves the person's pointer); the step closes by itself when the title event exists,
  without pressing anything in the panel, and the cut step closes when the event at the marker is in
  two pieces; after the restart the welcome window offers "Continue the tour — lesson 1, step 3" and
  the progress is intact; "Start over" clears it and makes a new demo copy
- **PATH** start Kinetren for the first time → the welcome window, "Start the tour" focused → click
  it → the demo project opens and a panel on the right reads "Lesson 1 of 5 · The first edit" with
  step 1 "Add a title" and a "Show" button → click "Show" → the File menu opens and "Create a
  generator…" carries an accent outline → Create a generator… → Titles and countdown → Text → a title event appears on the timeline and step 1 turns into a
  check mark, step 2 "Cut the event at the marker" appears → put the playhead on the marker and
  press S → the event is in two pieces and step 2 is checked → close the panel and quit → start
  again → the welcome window's primary tile reads "Continue the tour — lesson 1, step 3"
- **TEST** t988 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Every lesson target exists and no teaching layer appears
- **WHEN** the guard walks every step of every lesson and checks its "Show" target against the
  `WINDOWS` registry, the panel list and the menu catalog, and its paragraph against the five
  catalogs; separately, with the lessons panel open on each step, the egui layers above
  `Order::Middle` are listed
- **THEN** every target exists and every paragraph has five translations within the catalog budget;
  the layers above `Order::Middle` are exactly the closed named list (status hint, title, in-place
  name editors, the overlays that show state), and none belongs to a lesson
- **PATH** there is no gesture: the guard runs with the program's own lessons; a person opening any
  lesson never sees a callout, a dimmed screen or a frame drawn over the editor
- **TEST** t988 (`src/apps/kt-app/src/reach_tests.rs`), `reach_tests::no_teaching_layer_over_the_ui` (t988, same file)

#### Scenario: The current step's target carries a static outline, drawn by the target itself
- **WHEN** a lesson step whose "Show" target is a menu item or a panel is current, the panel open
- **THEN** the target draws a static accent outline in its own layer (the menu item inside its
  menu, the timeline on the panels' layer) for as long as the step is current; nothing pulses,
  nothing dims, no arrow is drawn, and no layer is added above `Order::Middle`
- **PATH** there is no gesture of its own: open a lesson → the step's target (the File menu item
  after "Show", or the timeline) is outlined in the accent colour → do the step → the outline moves
  to the next step's target
- **TEST** t988 (`src/apps/kt-app/src/reach_tests.rs`), `reach_tests::no_teaching_layer_over_the_ui` (t988, same file)

### Requirement: Help → Guide: "I want to … → where", with pages from the repository's guide
The program SHALL have a Help menu with Guide…, Start the tour, Welcome window… and What's new. The
Guide window SHALL hold a search field, an index of entries "I want to … → where / which gesture"
(each entry a catalog key, translated ×5, naming the place — a menu path, a panel, a window — and the
gesture or key), and the guide pages rendered from `docs/guide/*.md` (including `shortcuts.md`), which
SHALL be compiled into the binary from the same files the repository shows — no second copy. Typing
in the search SHALL filter the index entries and the page headings together (a heading is found
by its own words or by the words of its section); clicking an entry SHALL
open the page at its heading. Every index entry SHALL point to an existing heading of a guide page,
and the renderer SHALL accept only the markdown subset the guide uses (headings, paragraphs, lists,
bold, italic, inline code, `<kbd>` key caps, tables, links, fenced code blocks; an image is shown as
its caption); a guide file using anything else
SHALL fail the guard, not render wrongly. The welcome window SHALL link to the Guide. The guide pages
are English (capability `showcase`); the index entries are translated.
Why: the owner — "we have no guideline inside the program explaining what can be done and where"
(2026-10-27); the answer recorded the same day — "a Guide window and a welcome screen". The guide is
already written and checked (`readme-check.py`, t826); F-HLP-023 asked for addresses first, and an
index entry pointing at a heading is that address.

#### Scenario: The Guide finds "where" and opens the page
- **WHEN** the harness opens Help → Guide…, types "loop", clicks the entry "Play a part in a loop",
  then clears the search and opens "Keyboard and mouse"; separately the guard checks every index
  entry's heading and every guide file's markdown against the subset
- **THEN** the index shows "Play a part in a loop → drag on the ruler, then L" and the page heading
  "Markers and the region" from `timeline.md`; the click shows that page scrolled to that heading; the
  shortcuts page is the content of `docs/guide/shortcuts.md`; every entry resolves and every file
  passes the subset check, while a test copy with an HTML block fails it naming the file and line
- **PATH** open Help → Guide… → a window with a search field on top, "I want to…" entries on the left
  and the pages list below them → type "loop" → two lines remain: "Play a part in a loop → drag on the
  ruler, then L" and the page heading "Markers and the region" → click the first → the right side shows the
  timeline page at "Markers and the region" → clear the search → click "Keyboard and mouse" → the key table
  of the guide
- **TEST** t989 (`src/apps/kt-app/src/reach_tests.rs`)
