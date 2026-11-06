# Proposal

## Why

Round R.72 is the first round of the claims registry `docs/ktprompts/v26.10-index.md` (the owner's
order, answer to question 11 of R.71, 2026-10-30): the timeline and transport defects he meets every
day. Seventeen rows carry stage R.72. Two of them repeat older themes: "resize steals clicks" (r.12/r.13)
and "no obvious labels" (item 17 of R.7). Each is cheap alone; together they are why the timeline
"fights the hand". The owner's words are in `docs/ktprompts/v26.10.txt`, his decisions in
`docs/user-feedback.md` R.71.

## What Changes

- **Space bar (U2, U3, U4)** — `transport`:
  - a short press keeps today's toggle, and the double press still stops (R.16: "don't break the
    functions of the double and single press");
  - a press held from pause past ~0.3 s plays only while held, and release stops;
  - **Ctrl+Space** held plays in reverse;
  - the boost rises with an ease-in and, on release during playback, falls back to the base speed
    smoothly but faster than it rose.
- **Event hit zones by on-screen size (U5.1–U5.3)** — `ui-timeline`:
  - the trims are tested before the level line, so a bottom corner always gives a length drag;
  - the level line's grab follows the row height;
  - on a low row or a narrow event there is no level gesture at all, only move, trim and fade.
- **Event gain down to mute and a level chip (U6)** — `ui-timeline`, `audio-render`:
  - the event's gain goes from +6 dB to silence;
  - a non-default gain or opacity is written as a small chip after the source name on the event.
- **The range marker's end (U7)** — `ui-timeline`:
  - the end is grabbed where it is drawn;
  - hover names it;
  - its drag does not snap to its own old position.
  - The split of "Marker over the region" into two items is NOT made here: the owner did not confirm
    it (R.71, question 9), so it is shown to him live first.
- **Vertical zoom keeps the row under the mouse (T1)** on rows of different heights — `ui-timeline`.
- **The vertical scrollbar can be caught (T2)**:
  - it is a no-resize rect;
  - its grab is wider than its drawing;
  - the jump zone does not cover it — `ui-timeline`.
- **The left jump zone is whole again (T3)** — the track badges do not eat it — `ui-timeline`.
- **In the jump zone a click jumps, even over an event (T4a)**; a quick second click is another jump,
  not "open the FX window" — `ui-timeline`.
- **No obvious labels on empty service rows (T4b)**:
  - "Add morpher here" / "Add camera at the playhead" leave the rows; the actions stay in the row's
    menu and its status-line hint;
  - a guard test keeps captions out of empty rows — `ui-timeline`.
- **Track reorder by the mouse works and is findable (T6)**. The drag in the header gutter was dead
  since R.38 (found at apply: the height drag's release took it), had no test, and the R.62 badges crowd
  it. It is repaired and gets a grab cursor, a hint and a test that drags from where a person grabs
  — `ui-reach`.
- **Frame step (T11)** — `transport`:
  - **← / →** step one frame (as in Premiere, Resolve, FCP), and pause first when playing;
  - **Ctrl+click** on Back/Forward does the same with the mouse;
  - Shift keeps the region edge.
- **"playback froze" reaches the bell (M5)** — `transport`:
  - the engine records ONE stall per episode (today it floods the ring every loop pass), and not at
    rate 0;
  - the application posts one warning per episode with its time.
- **Freeze frame here (S1)** — `transport`, `ui-timeline`:
  - a menu command splits the event at the playhead and holds that frame for 2 s (rate 0);
  - the rest continues from the frozen frame;
  - the event keeps its length and its neighbours do not move;
  - one undo step.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `transport`: the space-bar contract (hold from pause, reverse hold, release decay), the frame step,
  one stall per episode in the bell, the freeze-frame command.
- `ui-timeline`:
  - hit zones by on-screen size;
  - event gain to mute and the level chip;
  - the range-marker end;
  - the vertical zoom anchor;
  - the scrollbar and the jump zone;
  - no captions in empty rows.
- `ui-reach`: the track-reorder drag is reachable from the header and named by a hint.
- `audio-render`: an event gain at the floor is exact silence.

## Impact

- `src/apps/kt-app/src/app.rs`:
  - space handling (`handle_keys`, `tick_space_boost`, `on_space_released`, `space_boost_step`);
  - `nav_jump` and a frame-step body;
  - stall notices;
  - the freeze-frame command.
- `src/apps/kt-app/src/timeline.rs`:
  - `clip_at` zone order;
  - `level_*`;
  - the marker-end hit, hover and snap;
  - the vertical zoom;
  - the scrollbar;
  - the jump zone;
  - `empty_svc` captions;
  - the gutter reorder;
  - the gain range and the chip.
- `src/apps/kt-app/src/toolbar.rs` — Ctrl+click on Back/Forward.
- `src/libs/kt-engine/src/player.rs` — stall once per episode.
- The engine's clip gain — the floor as silence.
- `src/libs/kt-model` — the event gain range constant.
- Catalogs `i18n/*.toml`: the new hints and the command name; the two row captions go.
- `docs/guide/shortcuts.md`: Space hold, Ctrl+Space, ← / →, Ctrl+click Back/Forward.
- Tests t1620–t1637: `layout_tests.rs`, `reach_tests.rs`, `kt-engine` tests; live scenarios for the
  gestures.
- No format change: the gain range is a value range of an existing field, and a freeze is an existing
  rate 0.
