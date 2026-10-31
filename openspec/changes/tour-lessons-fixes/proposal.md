# Proposal

## Why

The owner walked the tour on 2026-11-01 (`docs/ktprompts/kkt-26.11.01.txt` items 2–8, registry rows
K2–K8; `docs/user-feedback.md` 2026-11-01). The lessons shipped in `welcome-window-and-tour`
(archived 2026-10-30) work in the harness and fail a person:

- **K2** — lesson 1's "Play a part in a loop" counted only after he closed the text generator's
  window. The step is re-checked only when `(model_rev, open windows, render result)` changes, and a
  region or the loop switch does not bump `model_rev` (they are "convenience" state, R.43 item 296).
- **K3** — "press + effect… and pick" runs together as one plain line; the Guide has `<kbd>`
  keycaps and lessons do not use them.
- **K4** — only lesson 1's File menu item is outlined; no other step shows where to act (a timeline
  outline exists in code, but it outlines the whole panel).
- **K5** — "Show" does nothing for panels; for the morpher it opens the window itself, which
  completes the step and teaches nothing ("he still does not know where the morpher is").
- **K6** — lesson 3 promises a crossfade from dragging an event onto its neighbour. An overlap is a
  cut by the owner's decision (t163/t464); only "Transition with previous" makes one. The owner,
  2026-11-01: the complaint is the crooked LESSON, not the overlap semantics.
- **K7** — lesson 4 does not say what changed after the effect, nor bring the playhead to where it
  shows; the person has to guess where to look.
- **K8** — "you could make some silly jokes in the lessons block while the render runs, it is too
  boring" (lesson 5).

## What Changes

- **A step is re-checked when what it reads changes** (K2): the region, the loop switch and the
  open-window set are part of the key next to the model revision; the key is a small copyable value
  built every frame without an allocation (the current per-frame `Vec` is an invariant-12 defect too).
- **Lesson text is structured** (K3): a step is a list of catalog lines — paragraphs and list items —
  with `<kbd>` keycaps for what is pressed and **bold** for names, drawn by the Guide's own inline
  renderer. UI names inside a step come from the catalog keys of the UI itself (one source: the
  lesson cannot drift from the button's label), menu paths are built from the step's target.
- **Every step points at where to act** (K4): a step names one or more targets — a menu item, an
  event, a marker, the ruler, a transition, the morpher block, an event's right-click item, or a
  named control in a window or the toolbar — and each draws a static accent outline while the step
  is current. No layer is added, nothing pulses (the owner's 2026-10-28 rule stays).
- **"Show" points, never performs** (K5): it brings the target into view (scrolls the timeline to it,
  opens the menu root, scrolls a window's control into view) and leaves the step itself to the
  person. A step is never closed by "Show".
- **Lesson 3 teaches the real way** (K6): right-click the second event → "Transition with previous" →
  a kind; the step closes on a video transition and the item is outlined in the right-click menu.
  Overlap stays a cut (t163/t464 untouched).
- **A step that changes the picture says what changed and shows it** (K7): when such a step closes,
  the panel writes one line of what changed and the playhead moves to where it shows (the event with
  the effect, the transition, the morpher's span) so the preview shows the result at once.
- **Jokes while lesson 5 renders** (K8): a rotating set of short silly lines in the lessons panel
  while the render runs, in all five catalogs.
- A live scenario walks all five lessons as a person, a screenshot per step.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `ui-reach`: the tour requirement — the re-check key, the structured text, targets with outlines,
  "Show" that points, lesson 3's real gesture, the result line and the playhead, the render jokes.

## Impact

- `src/apps/kt-app/src/lessons.rs` (most of the change), `widgets.rs` (the outline mark),
  `toolbar.rs` (menu roots, the Loop/Render buttons mark themselves), `gen_ui.rs` (the generator
  menu marks itself wherever it is drawn), `fx_registry.rs` (+ effect…), `fx_editor.rs` (+ curve),
  `timeline.rs` (the right-click "Transition with previous" item), `transition_editor.rs` (the kind
  field), `render_dialog.rs` (Start), `guide.rs` (the inline renderer made reusable).
- Catalogs ×5 (`[lessons]`), the CJK/Arabic font subsets (`scripts/subset-fonts.py`).
- Tests t1085–t1092 in `reach_tests.rs`, t988 and `no_teaching_layer_over_the_ui` adjusted to the
  new step shape; a live scenario `src/scripts/live/lessons.py`.
- Not here: moving "Create a generator" out of File (`generators-placement-and-audio`); the lesson
  names the menu path through its target constant, so that change edits one constant.
