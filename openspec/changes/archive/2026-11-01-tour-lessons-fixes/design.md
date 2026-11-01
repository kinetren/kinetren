# Design

## Context

`welcome-window-and-tour` (archived 2026-10-30) built the lessons as a docked panel whose steps close on
predicates over the project (`src/apps/kt-app/src/lessons.rs`, design D2 of
`openspec/specs/ui-reach/design.md`). The owner walked them on 2026-11-01 and found seven defects
(kkt-26.11.01 items 2–8). The panel form, the "no layer above `Order::Middle`" rule, the static outline
("yes, it may draw", 2026-10-28) and the lesson sandbox all stay. What changes is the inside of a step.

## Goals / Non-Goals

**Goals:** a step that closes when its state changes; text with structure and keycaps; a target for
every step that a person can see; "Show" that points; lesson 3 teaching the real gesture; a result line
and the playhead moved to the result; jokes during the lesson-5 render.

**Non-goals:** moving "Create a generator" out of File (`generators-placement-and-audio`); the overlap
semantics (t163/t464 stay); a coach mark, an arrow, a dimmed screen or any layer (still rejected).

## Decisions

### D1. The re-check key is the predicate's inputs, a `Copy` value
`Observed` was `{ open: Vec<&str>, render_done }`, rebuilt every frame (`open_window_ids` allocated a
`Vec` per frame — an invariant-12 defect of its own), and the key was `(model_rev, Observed)`. The
region and the loop switch live in the project but are set by `set_region`/`set_loop_enabled`, which
deliberately do not bump `model_rev` (R.43 item 296). The key becomes
`(model_rev, region, loop_enabled, open-window bits: u8, render_done)` — all `Copy`, compared every
frame, nothing allocated. Rejected: bumping `model_rev` on a region change — that is exactly what R.43
removed (a full renderer rebuild per region edge). Rejected: re-evaluating every frame without a key —
cheap on the sandbox, but the panel can stay open over the person's own project of thousands of events.

### D2. Text: lines of catalog keys, parsed by the Guide's inline parser, memoized per language
A step's text is `&'static [Key]`: each key is one line, a paragraph or (prefix `- `) a list item, ≤160
characters (the catalog ceiling, `MAX_VALUE_CHARS`, which exists because a reordered Arabic paragraph
cannot be re-wrapped). Inline markup is the Guide's subset (`guide::inlines`): `<kbd>`, `**bold**`. A
line may carry `{0}`…: the step's `args` substitute the catalog label of the control (`Arg::Label`) or
a menu path built from keys (`Arg::Path`, joined by ` › `, Arabic ` ‹ `). Arabic: the catalog table
holds visual order (reordered by `build.rs`), which would scramble the tags, so the lesson parses
`ar_logical` and reorders each run's text afterwards (`bidi::visual`) with the run order reversed — the
same atoms the build would produce for a one-line label. The parsed lines are memoized in `LessonsUi`
by `(language, lesson, step)`; the draw path reads the memo. Rejected: `\n` inside one catalog value —
it breaks the 160 ceiling and the Arabic reorder per line. Rejected: a new markup — the Guide's is
already tested (t989) and the owner pointed at it.

Convention: every key and every button or menu item a person presses is a `<kbd>`; names of things
(an effect, a marker, a track) are bold.

### D3. Targets are data; each draws itself through one thread-local mark
`Target` becomes `Menu(path) | ClipMenu{clip,item} | Clip(id) | Marker(name) | Ruler | Transition |
Morpher | Control(name)`, and a step has `targets: &'static [Target]` (the first one is where "Show"
goes). Each frame the panel publishes the current targets into `widgets::lesson_mark` (a `Cell` of a
`&'static` slice — no allocation). Menu items and controls are drawn deep inside closures without
`&mut KtApp` (`gen_ui::generator_menu`, `fx_registry`'s picker, the `+ curve` menu): they call
`lesson_mark::item(ui, key, rect)` / `lesson_mark::control(ui, name, rect)`, which draws the outline
only when that key/name is current — so the generator menu is outlined wherever it is drawn (File
today, Edit and the track menu after `generators-placement-and-audio`). The timeline targets are
computed in `lessons.rs` from `tl_geom` (the geometry the timeline already publishes for mapping) and
drawn on the background layer after the panels, clipped to the tracks. Every drawn outline records its
rect in the same thread-local (reused buffer), which "Show" and the tests read.

`Control` names are a closed list (`lessons::CONTROLS`); t1092 checks that each is marked by a
`lesson_mark::control("<name>"` call in the production sources — a renamed button cannot leave a dead
step. Rejected: outlines through `field_probe` — it is off outside `KT_GEOM_DUMP`.

### D4. "Show" scrolls, opens a menu root, never performs
Per target: `Menu` opens its root popup (each root's id is stored as
`("kt_menu_root", root_key)`, not only File's); `Clip`/`ClipMenu`/`Marker`/`Transition`/`Morpher`
centre the view on it (`center_view_on`) and scroll the rows so its row is visible; `Ruler` is always in
view; `Control` asks its widget to `scroll_to_me` on the next frame. A `Control` inside a closed window
was not drawn last frame — "Show" then goes to the step's next target (e.g. lesson 4 step 2:
`[Control("fxreg.add"), Morpher]`). The former `lesson_show` opened the fx editor, the morpher window
and the render settings — exactly the steps; all three are gone.

### D5. Lesson 3: the right-click item; the step stays on `p.transitions`
Text: right-click the second event → "Transition with previous" → a kind. Targets:
`[ClipMenu{clip: 3, item: TRANSITION_PREV}]` (clip 3 follows clip 1, butted at 0:12, in every fresh
lesson copy). The predicate already reads `p.transitions`; an overlap made by a drag adds none, so it
cannot close the step — the overlap = cut rule is untouched. Step 2 (another kind) targets the
transition on the timeline and the transition window's kind field.

### D6. The result line and the playhead: `Step::result` and `Step::look`
A step may carry `result: Option<Key>` (the line shown under it once checked, until the next step
closes) and `look: Option<Look>` (`ChainWith(tag) | Transition | Morpher`). When `lesson_tick` records
a step done during the tick (not on a reload of a finished lesson), it seeks the player to the look's
moment — the middle of the first event with that effect, the middle of the transition window, the
morpher's start + 4 s clamped into its span — and centres the view there. The seek is the program
showing the consequence; the person made the edit. Rejected: a third "watch it" step closing on
playback — it would close on the playhead, which moves every frame, and adds a gesture the owner did
not ask for.

### D7. Jokes: a catalog set, rotated on the UI clock, one repaint per switch
Eight keys `lessons.joke_1..8`. While step 5.2 is current and `render_job` runs without a result, the
panel shows `joke[(t / 6 s) % 8]` with `t` the egui input time (UI-only, never an offline path, so
invariant 11 is not touched) and calls `request_repaint_after` for the next switch (the render progress
already repaints; this only guarantees the switch). Rejected: random order — a test could not tell
"changed" from "the same by chance".

### D8. The panel is drawn after the timeline (found by the live walk)
The first form drew the lessons panel BEFORE the timeline so it spanned the window's height. It then
narrowed the timeline AND its bottom toolbar, a fixed left-to-right row clipped at the panel's
edge: in a 1440 px window the Loop button lesson 1 names (and Undo, Redo, Render, Search) was not on
screen. The lessons point at the timeline more than at anything else, so the timeline keeps the full
width and the panel sits beside the preview — it still narrows the editor and still adds no layer.
The harness missed it: a widget's recorded rect ignores the clip, so the guard (t1094) compares the
outline with the timeline panel's visible width. Rejected: making the toolbar row fit beside the
panel — a wider layout change of shared code, while the lessons need the timeline whole anyway.

## Risks / Trade-offs

- **The CJK/Arabic font subsets** must be regenerated for the new catalog text
  (`scripts/subset-fonts.py`); parallel branches touching catalogs will conflict on the binary subsets —
  resolved by re-running the script after the merge (noted in the hand-over).
- **Shared files**: `toolbar.rs` (menu roots, two marks), `timeline.rs` (one mark), `fx_registry.rs`
  (one line), `render_dialog.rs` (one line) — other executors edit those files; hunks are kept to
  one or two lines.
- **The bottom toolbar row is ~1460 px wide** (found by t1087 at 1440×860, with or without the
  lessons panel): below that width the Render button at its right end is cut. Not this change's
  layout: the lesson tests run at the owner's 1920×1080, t1094 holds the Loop button at 1440, and the
  row's own width is a follow-up for the window-layout work of this round (kkt-26.11.01 item 12).
- **Arabic run order** for wrapped lines stays as imperfect as every wrapped Arabic label today (the
  build-time limitation named at `MAX_VALUE_CHARS`).
