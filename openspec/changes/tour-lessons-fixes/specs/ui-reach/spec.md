# Spec Delta

## MODIFIED Requirements

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
  step 1 "Add a title" and a "Show" button → click "Show" → the File menu opens and "Create a
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

## ADDED Requirements

### Requirement: A lesson step is re-checked when what it reads changes
The re-check of the current step SHALL follow every input of its predicate, not only the model
revision: a region drawn on the ruler and the loop switched on SHALL close "Play a part in a loop" in
the frame they happen, with any window open. The key that decides a re-check SHALL be built every
frame without an allocation.
Why: the region and the loop switch are convenience state that does not bump the model revision
(R.43 item 296: bumping it rebuilt both renderers and starved the audio); the first form keyed the
re-check on the revision and the open-window set only, so the step closed only when a window closed
(kkt-26.11.01 item 2).

#### Scenario: The loop step closes while the generator window stays open
- **WHEN** lesson 1 is at "Play a part in a loop" with the text generator's window open; a region is
  dragged on the ruler and L is pressed; nothing else happens
- **THEN** the step is checked within two frames and the window is still open; the re-check key
  holds no heap data
- **PATH** lesson 1, step 4, the title's window still open → drag on the ruler from 0:20 to 0:30 → a
  region appears → press L → the loop button lights up and step 4 turns into a check mark at once,
  the title window stays open
- **TEST** t1085 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Lesson text has paragraphs, lists and keycaps
A step's text SHALL be one or more catalog lines, each a paragraph or a list item, drawn by the
Guide's inline renderer: `<kbd>` as a keycap for every key and every button or menu item pressed,
bold for names. The label of a program control named in a step SHALL be substituted from that
control's own catalog key; a menu path SHALL be built from the step's target. All five catalogs SHALL
give a step the same number of lines, the same kinds of lines and the same number of keycaps; the
parsed text SHALL be kept per language and not rebuilt per frame.
Why: kkt-26.11.01 item 3: "press + effect… and pick" ran together as plain text, while the Guide
already draws `<kbd>`. A label retyped in the lesson drifts from the button (the generator menu moves
out of File in `generators-placement-and-audio`).

#### Scenario: Every step parses into lines with keycaps in five languages
- **WHEN** the text of every step is parsed in each of the five languages and the panel draws lesson
  2, step 2
- **THEN** every step's lines parse in the Guide subset with the same line count, line kinds and
  keycap count across languages; every key and button the English text names is a keycap; the label
  inside "+ effect…"'s keycap equals the button's own catalog text in that language; the panel shows
  the step as separate lines, not one run-on line
- **PATH** open lesson 2, step 2 → the panel reads "Add a blur." and then a list: "Press [+ effect…]
  in the effects window" and "Pick **Blur**", the button name in a keycap
- **TEST** t1086 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: "Show" points at the step's target and never performs the step
Pressing "Show" SHALL bring the current step's first reachable target into view — the timeline
scrolled so the event, marker, transition or morpher block is visible, the menu root opened so the
item is visible, a window's control scrolled into view — and SHALL NOT open a window the step asks the
person to open, make an edit, or close the step. When a target lives inside a window that is closed,
"Show" points at the next target of the step (the element on the timeline whose double-click opens
that window).
Why: kkt-26.11.01 item 5: "Show" did nothing for panels, and in lesson 4 it opened the morpher, which
closed the step and left the person not knowing where the morpher is.

#### Scenario: Show on every step leaves the step open and its target in view
- **WHEN** for every step of every lesson, the timeline scrolled far away from the target, "Show" is
  pressed
- **THEN** no step closes; no window opens that was not open; the outlined target lies inside the
  visible timeline, the open menu or the window; on lesson 4 step 1 the morpher window does not open
  and the BATCH block is outlined in view
- **PATH** lesson 4 → the timeline scrolled to the end → press Show → the timeline scrolls to the
  BATCH morpher block, outlined → double-click it → the morpher window opens and step 1 is checked
- **TEST** t1088 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Lesson 3 teaches the transition the program makes
The transitions lesson SHALL teach "Transition with previous" from an event's right-click menu and
close its first step on a video transition between two events; the item SHALL be outlined in that
menu. Dragging an event onto its neighbour SHALL NOT close the step: an overlap stays a cut (t163,
t464).
Why: kkt-26.11.01 item 6 and the owner's answer 1 of 2026-11-01: the lesson promised a crossfade from
an overlap; the overlap semantics are the owner's decision and stay.

#### Scenario: An overlap does not close the step; Transition with previous does
- **WHEN** lesson 3 is open; the second video event is dragged onto the first so they overlap; then
  the second event is right-clicked and "Transition with previous" → Dissolve picked
- **THEN** after the drag the step is still open and the project holds no transition; with the
  right-click menu open the item is outlined; after the pick step 1 is checked
- **PATH** lesson 3 → drag the second event left over the first → the step stays, the events meet as
  a cut → right-click the second event → "Transition with previous" is outlined → hover it → Dissolve
  → a transition appears between them and step 1 is checked
- **TEST** t1089 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A step that changes the picture says what changed and shows it
When a step whose result shows in the preview closes (an effect added to an event, its curve, a
transition, its kind, an effect added to a morpher's batch), the panel SHALL show one line under the
checked step saying what changed and where, and the playhead SHALL move to a moment where the result
shows: inside the event with the effect, the middle of the transition, inside the morpher's span. The
move happens once, when the step closes, not on later frames.
Why: kkt-26.11.01 item 7: after the effect was added the lesson said nothing, and the person had to
guess where to bring the playhead to see the result.

#### Scenario: The morpher's new effect is announced and the playhead moves into its span
- **WHEN** lesson 4, step 2 is current with the playhead at 0:00, and an effect is added to the
  morpher's batch
- **THEN** the step is checked; under it the panel says the effect now works on the events under the
  BATCH block; the playhead is inside the morpher's span; the same holds for lesson 2's blur (inside
  the event with the blur) and lesson 3's transition (inside the transition)
- **PATH** lesson 4 → double-click BATCH → "+ effect…" → pick any effect → step 2 is checked, the
  line "The effect now works on both events under BATCH…" appears and the playhead jumps to 0:52, the
  preview shows the effect
- **TEST** t1090 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Jokes while the export lesson renders
While the render started from the export lesson runs, the lessons panel SHALL show one short silly
line from a catalog set, changing every few seconds; the set SHALL exist in all five catalogs; the
change SHALL request one repaint at the next switch, not a continuous repaint; nothing SHALL show
once the render finishes.
Why: kkt-26.11.01 item 8: "you could make some silly jokes in the lessons block while the render
runs, it is too boring".

#### Scenario: A joke shows and changes while the render runs, and goes when it is done
- **WHEN** lesson 5, step 2 is current and a render job is running; time advances past the switch
  interval; then the render finishes
- **THEN** one joke of the set is shown, a different one after the interval, every joke has five
  translations; after the render no joke is shown and step 2 is checked
- **PATH** lesson 5 → File › Render to MP4… → Start → while the progress runs the lessons panel shows
  a line like "Teaching the pixels to stand in line…", a few seconds later another → the render ends,
  the joke goes and the step is checked
- **TEST** t1091 (`src/apps/kt-app/src/reach_tests.rs`)
