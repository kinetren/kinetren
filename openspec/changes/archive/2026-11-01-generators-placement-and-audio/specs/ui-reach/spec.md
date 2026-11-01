## MODIFIED Requirements

### Requirement: Paired ways of doing one action — one body; an action has one door
An action available through a gesture, a menu item, a button and a key SHALL be executed by ONE
body that accounts for the context identically (region, selection, reel). Publishing a parameter of a
composition SHALL have exactly one door — a right click on the field's caption INSIDE the reel (both in
full entry and in the expanded strip); outside the reel there is no item at all, but the status line
explains where it lives — and only when compositions exist in the document. The handle's name is the author's
text, drawn verbatim, bypassing the catalog.
Why: the reference failure — the M gesture with a region made a range label, while the menu item made
a point one; two entries with two copies — the same "two tracks from one +M".

#### Scenario: A generator is created from the main menu and from the bin with one list
- **WHEN** the "Create a generator" submenu is opened from the Edit menu, from a clip track's right-click menu and from the bin's background menu; the File menu is opened
- **THEN** the list of kinds is built by one body for all three entries (the track's filtered by its kind), creation puts the media and the clip in one history step, the track entry at the clicked track and time; File offers no generator item
- **PATH** click "Edit" → "Create a generator" → the groups; right click on a track → "Create a generator" first; right click on the bin's background → "Create a generator"; click "File" → no such row
- **TEST** t560 (`src/apps/kt-app/src/reach_tests.rs`), t1165 (`src/apps/kt-app/src/gen_audio_tests.rs`)

#### Scenario: The publish door is one and reachable only from inside
- **WHEN** a right click arrives at the parameter's caption inside the reel (full entry and strip), at the same caption outside the reel, and the instance's menu is opened
- **THEN** inside — "Publish…"/"Unpublish"; outside there is no item, and the status line says why; the instance's menu reaches BOTH windows — the consumer's and the author's
- **TEST** t682 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The consumer window draws each type with its own widget
- **WHEN** the instance's window of composition parameters is open with handles of all types
- **THEN** each type is drawn with its own widget, reset sends the removal of the override, a number with a curve places a key in the material of the INSTANCE; the window session is one history step
- **TEST** t683 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The author's name is drawn verbatim and does not move with the language
- **WHEN** a handle is declared with an arbitrary name and the program's language is switched
- **THEN** in the DRAWN galleries the name is present letter for letter in every language (the accessibility tree is not queried — there the reset button's compound name is used)
- **TEST** t684 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Badges speak of overrides, zero is not written
- **WHEN** the instance has overrides, the reel has declared handles, or there are no publications at all
- **THEN** the pin at the tail of the event's caption and the number in the head of the reel card are drawn; at zero — nothing
- **TEST** t685 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The tour is lessons in their own view; a step closes on the project's state
The tour SHALL be a set of lessons shown in a docked side panel of the main window (it narrows the
editor, it does not cover it); a lesson is an ordered list of steps, each a short structured text of
the translation catalogs (paragraphs and list items, `<kbd>` keycaps for what is pressed, bold for
names; the names of the program's own buttons and menu items taken from their catalog keys, never
retyped), its targets, an optional "Show" button and a done predicate. A step SHALL close by itself
when its predicate over the project's state and the program's observable state becomes true (for
example: the project holds a title event; the event at the marker is cut in two; a chain holds a blur;
a loop region is set; the window of a registry id is open) — never by "Next" alone, never by "Show"
and never by a command registry. The predicate SHALL be re-checked whenever anything it reads changes —
the model revision, the region, the loop switch, the open-window set, the render result — in the same
frame, and the check SHALL cost no allocation per frame at rest. Every step SHALL name at least one
target where the person acts: a menu item, an event, a marker, the ruler, a transition, the morpher
block, an item of an event's right-click menu, or a named control of a window or the toolbar; each
target SHALL draw a static accent outline while its step is current. "Show" SHALL point at the target:
bring it into view (scroll the timeline to it, open the menu root, scroll the control into view) and
SHALL NOT perform the step (it never opens the window the step asks the person to open, never makes
the edit). A step whose result shows elsewhere than where the person acted (an effect, a transition, a
morpher batch) SHALL, when it closes, say in one line what changed and move the playhead to where it
shows. Every target named by a lesson SHALL exist in the registry, the menu catalog or the closed list
of named controls, and every named control SHALL be marked by the program's own drawing code; a lesson
whose target is missing SHALL fail the build's guard rather than show a dead step. The lessons SHALL
run on a fresh COPY of the built-in demo project, built from generators only (no media files), placed
in the user data directory and never in the person's own folders; "Start the tour" asks about unsaved
work like any other open. Progress (finished steps per lesson) SHALL live in the user store and SHALL
be reset by Help → Start the tour → "Start over". No layer above the window layer SHALL explain the
interface: the program's layers above `Order::Middle` are a closed, named list, and a lesson adds none.
While the export lesson's render runs, the panel SHALL show a rotating short joke from the catalogs.
Why: the owner's request (2026-10-28) revises F-HLP-028: of the three forms the reconnaissance looked
at, the one it deferred — a lesson that CHECKS the done action, in its own view (Ableton Learn View) —
is taken; the coach mark over the interface stays rejected (invariant 12, the "one hint" rule, and
t83 catches only `Order::Tooltip`). The deferral reason "there is no command registry" (F-UI-002) is
removed by closing steps on the project's state, which the model already exposes. The owner's walk of
2026-11-01 (kkt-26.11.01 items 2–8) found the first form teaching nothing past lesson 1: a step that
counted late, text without keycaps, no outline but one, "Show" that did the step, a lesson promising
a crossfade the program never makes, an effect added with nothing said about it.

#### Scenario: A lesson step closes on the project's state and "Show" reaches the target
- **WHEN** the harness starts the tour from the welcome tile; in lesson 1 the step "Add a title" is
  shown and its "Show" pressed; a title is added through the generator menu → Title; the next step "Cut
  the event at the marker" is done with S at the marker; the panel is closed and the program
  restarted; then "Start over" is used
- **THEN** a demo copy opens from the data directory (generators only, no missing media); "Show"
  opens the menu root and "Create a generator…" carries the static accent outline (the program
  never moves the person's pointer); the step closes by itself when the title event exists,
  without pressing anything in the panel, and the cut step closes when the event at the marker is in
  two pieces; after the restart the welcome window offers "Continue the tour — lesson 1, step 3" and
  the progress is intact; "Start over" clears it and makes a new demo copy
- **PATH** start Kinetren for the first time → the welcome window, "Start the tour" focused → click
  it → the demo project opens and a panel on the right reads "Lesson 1 of 5 · The first edit" with
  step 1 "Add a title" and a "Show" button → click "Show" → the Edit menu opens and "Create a
  generator…" carries an accent outline → Create a generator… → Titles and countdown → Text → a title
  event appears on the timeline and step 1 turns into a check mark, step 2 "Cut the event at the
  marker" appears → put the playhead on the marker and press S → the event is in two pieces and step 2
  is checked → close the panel and quit → start again → the welcome window's primary tile reads
  "Continue the tour — lesson 1, step 3"
- **TEST** t988 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Every lesson target exists and no teaching layer appears
- **WHEN** the guard walks every step of every lesson and checks its targets against the `WINDOWS`
  registry, the menu catalog and the closed list of named controls (each named control must be marked
  somewhere in the program's sources), and its text against the five catalogs; separately, with the
  lessons panel open on each step, the egui layers above `Order::Middle` are listed
- **THEN** every step has a target and every target exists; every text line has five translations
  within the catalog budget; the layers above `Order::Middle` are exactly the closed named list
  (status hint, title, in-place name editors, the overlays that show state), and none belongs to a
  lesson
- **PATH** there is no gesture: the guard runs with the program's own lessons; a person opening any
  lesson never sees a callout, a dimmed screen or a frame drawn over the editor
- **TEST** t988 (`src/apps/kt-app/src/reach_tests.rs`), `reach_tests::no_teaching_layer_over_the_ui` (t988, same file), t1092 (same file)

#### Scenario: The current step's target carries a static outline, drawn by the target itself
- **WHEN** each step of each lesson is made current in turn, with what the step needs on screen (the
  effects window for "+ effect…", the right-click menu for "Transition with previous", the render
  settings for Start)
- **THEN** each step's target draws a static accent outline in its own layer (a menu item inside its
  menu, a control inside its window, an event, a marker, the ruler, a transition or the morpher block
  on the timeline, clipped to the tracks) over the element it names, for as long as the step is
  current; nothing pulses, nothing dims, no arrow is drawn, and no layer is added above
  `Order::Middle`
- **PATH** open lesson 2 → the first video event on the timeline is outlined → double-click it → the
  effects window opens, step 1 is checked and "+ effect…" in the window is outlined → pick Blur → the
  step is checked and "+ curve" is outlined
- **TEST** t1087 (`src/apps/kt-app/src/reach_tests.rs`)

