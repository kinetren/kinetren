# Design

## Context

The motivation and the measurement table are in `proposal.md`. Here only what is needed to explain
the approach.

What is already built in R.68 and works: the program reports next to the geometry dump the number of the
drawn frame (`bench::frames()`) and the number of input events it has read
(`bench::inputs()`, `ui.ctx().input(|i| i.events.len())`); the harness has `seen()`,
`wait_seen(mark)`, `wait_frame(n)` and `settle()`; `settle` measures silence in OBSERVABLE
frame steps (`self._dt`). All of this is used by the probes (`_probe_rect`, `has_probe`,
`pixel_settled`) and is not used by the gestures themselves.

What is still on the clock (measured on cbc5a3e, a live run of 1582 s sequentially):

- `kt_live.py` — 265 s: `click` 0.55, `right_click` 0.65, `double_click` 0.69, `drag`
  1.35, `key` 0.60, `hover` 0.70, `close_window` 1.00 (each — 0.2 s on `focus()`);
- `ui-probe.py` — 123 s: `move` 0.05, `button` 0.09 for each press/release, `key`
  0.05 + 0.12. The harness can already pass its pause to the driver — the docstring of
  `button()` says so in plain text — but not a single call from `kt_live.py` passes it;
- reading the screen — ≈341 calls to `xwd` + `convert` per run.

Limits that must not be touched: the set of checks is not weakened; the verdicts of all 45 live
scenarios must match the current ones; the product (`kt-app`) does not change — it already reports everything needed; the rule "red in parallel is re-run once" remains the verdict of the run.

## Goals / Non-Goals

**Goals:**
- Not a single "by eye" wait around a gesture: the harness asks the program whether it has read the input.
- The remaining sleep — enumerable, explained AND bounded by a number, not only in prose.
- The guard sees the WHOLE live check, including the X driver.
- Reading a screen point costs an X request, not two ImageMagick processes.

**Non-Goals:**
- The fixed costs of a scenario (raising the project in the stand, `wait_loaded`, generation of ffmpeg fixtures) — a separate floor, named in `durability/design.md`.
- Speeding up the workspace run (215 s) — the second floor there.
- Editing the scenarios themselves: by construction they should not notice the change, and that is the acceptance test.
- Reaching the 300 s target: the arithmetic in `durability/design.md` says the floor with 45 scenarios is ≈330 s. The round takes what can be taken and names the remainder.

## Decisions

### 1. The gesture waits for the GROWTH OF THE INPUT COUNTER, and the fallback is a named pause, not silence

`seen()` is taken BEFORE the gesture, `wait_seen(mark)` waits after it. This is not a new mechanism — it is the
very one for which the counter was created (R.68: "this is where a level drag was silently lost"), simply
carried through to the gestures.

A subtlety that decides the shape: the counter counts EGUI EVENTS per frame, not our XTest events.
egui and winit have the right to merge consecutive pointer movements into one, so waiting for "the counter grew
EXACTLY by as many as we sent" is not allowed — the wait would hang on a legitimate
merge. We wait for the GROWTH (the counter became greater than the mark) plus a frame — this is enough:
growth means the events have reached egui, and the frame means they got into the state.

An event that egui LEGITIMATELY does NOT generate (a movement to the same point, a key into a window without
focus) gives a wait that ends with nothing. Therefore each such wait has a
`soft` outcome, and it is NOT silent: it is a named pause, it goes into the sleep budget and is
visible to the guard. A silent fallback would bring back exactly the blindness for whose removal the round was set up.

Alternatives: count the sent events and wait for the exact number (breaks on merging);
wait only for the frame (the frame could have STARTED before the event — this is already checked in R.68 and recorded
in `ui-reach/design.md`); wait only for the `settle` silence (the silence sets in even before
the event has arrived).

### 2. The pauses that REMAIN, and why they will not disappear

Three of them, and for each the reason is not the speed of the frame, but the mechanics of egui:

- **The double-click gap** (0.04 s between clicks). In egui a double click is a TIME-BASED gesture;
  measured in R.34: 0.04 s — 5 of 5, 0.10 s — 0 of 5. There is nothing to wait for here: the time itself is being checked.
- **The drag step between positions.** egui counts a drag by a threshold AND by a frame; a frame must pass between
  positions. This is a wait for a FRAME — it is replaced by `wait_frame(1)`, and the
  pause that remains is only the press before the first shift, if a frame does not arrive after the press
  (the program might not have redrawn: repaint only on movement, invariant No. 12).
- **The autosave clock and playback in real time.** A scenario that listens to a second and a half of playback sleeps for a reason — this is the check itself.

This is the content of the rule "enumerable": the list is closed, each item has a measurable
reason, everything else is a wait.

### 3. The window is raised on a CHANGE of window: `raise_once` is already written, it must be connected

`focus()` makes a round trip to the X server (`set_input_focus` + `configure(stack_mode=Above)` +
`sync`) and sleeps 0.2 s. On 410 gestures that is 82 s and 410 round trips. Meanwhile the window is one: the program's modal
forms are egui layers INSIDE the same X window, and there is no point raising it between two consecutive clicks. `raise_once()` (R.68) raises the window if this has not yet been done, and `resize()`
already resets the flag. What remains is to replace the unconditional `focus()` in the gestures with `raise_once()` and
to reset the flag where the X window could really have changed: the return of the window geometry on
entry to the stand, the own process, a resize.

The risk that hides here is named in item 199: the window of the previous scenario could have remained
on top. In the stand there is one process, and a scenario with "its own process" gets a screen for itself — but
this is verified not by reasoning, but by comparing the verdicts of all 45 scenarios.

### 4. The X driver stops sleeping on behalf of the harness, and the guard begins to see it

`kt_live.py` passes `probe.move/button/key` its own pause (almost zero) — as written in the
docstring of `button()`. The defaults of `ui-probe.py` itself are NOT changed: the script remains a manual
tool ("run it and look"), where there is no one to wait for the frame.

The scope of the guard `live-sleeps.py` is extended to `ui-probe.py` — with one caveat, which must be written down so that it is not "fixed" back: for `move`/`button`/`key` the sleep lives in
the DEFAULT VALUE of the argument, and not in the call, so the guard checks the defaults too.

### 5. The sleep budget: a static limit at the guard, the actual sum — in the report

Two different things, and they must not be confused:

- **The guard counts the DECLARED seconds** — the sum of the constants of all fixed pauses of the harness
  and the driver — and fails when the limit is exceeded. This is checked without a run, does not depend on the machine and does not depend on how many times a gesture was called. A ratchet of the same kind as
  `UNTOUCHED_LIMIT` of the coverage report (R.68).
- **The harness counts the ACTUAL sleep** of the run (the sum over `pause()`) and prints it next to
  the price of the scenario. This number is for the report and for the next measurement, not a verdict: a verdict that
  depends on the machine's load is a flake by construction.

The alternative "fail the run when the actual sleep is exceeded" was rejected for exactly this reason:
with four workers the sleep does not grow, but the waits do grow, and the threshold would begin to catch
the load instead of a regression.

### 6. The screen point is read from X; `xwd` remains the reference and the fallback

`pixel`, `pixel_settled` and `ink_box` take the rectangle via `get_image` of python-xlib
(the same `Display` that the driver already has open) instead of `xwd` plus two `convert`.

The cross-check is mandatory and not a matter of taste: for the eframe window there is a 32-bit visual, and reading the root window
by ImageMagick had already returned BLACK where there was a picture (R.33 item 228) — the scenario then compared black with black and "passed". Therefore, the first time in a run the harness reads the point
by BOTH paths on one frame: if they match — the direct path goes on; if they do not — the whole run
reads by the reference and SAYS so. The same technique as "CPU — byte reference, GPU — by
tolerances" (invariant No. 7), and it also protects against silent blackness.

`shot()` is not touched: the artifacts and the `PATH` steps are pictures for a person, their price
is justified, and `finish.sh` prints their list.

### 7. Acceptance — verdicts, not seconds

None of the decisions above is considered working until the verdicts converge: a full run of all
45 scenarios gives exactly the same verdicts as before the round, and three runs in a row go without
re-runs. Time is the second check, and it must be measured in THE SAME way as R.68: the stage table of
`finish.sh` and `target/live-out/times.txt`.

## Risks / Trade-offs

- **Waiting is subtler than the egui mechanics — and this has already broken a round** (R.68, pitfall 2: the thresholds of
  the drag, the double click and the menu animation). → The list of remaining pauses is closed by decision 2
  and is held by the guard; each replacement is checked by comparing the verdicts of all 45 scenarios, not
  of one.
- **An event that egui will not generate hangs the wait.** → A `soft` outcome for each gesture
  wait, and it is NOT silent: a named pause in the budget and visible to the guard.
- **The removed `focus()` returns "the click went to a foreign window" (item 199).** → The flag is reset
  on entry to the stand, in one's own process and after a resize; the comparison of verdicts catches the rest.
- **Direct reading of a pixel silently returns black** (R.33 item 228 — exactly this class). → A comparison with
  `xwd` on one frame at the start of the run, a fallback to the reference with an announcement.
- **The sleep will become smaller, but the run will not**, because the wait takes its place. → This is a
  legitimate outcome, and it is also a result: blind waiting is changed into sighted waiting. The numbers will go into
  the report as they are, without fitting.
- **The static sleep budget can be circumvented by hiding a pause in a new function.** → The guard counts
  the pauses across the whole tree of the live check (the harness, the scenarios, the driver), and not by a list of names.

## Migration Plan

1. Measurement and accounting: the harness counts the actual sleep and prints it; the guard is extended to the X driver and gets a static budget with the current number — first a ratchet, then the acceleration (the same order as in R.68).
2. Gestures: `wait_seen` instead of pauses, `raise_once` instead of `focus()`, the whole line typed at once; the driver is passed its own pause.
3. Reading the screen point through X with a comparison against the reference.
4. Comparison of the verdicts of all 45 scenarios and three runs in a row; the sleep budget is lowered to what is achieved.

Rollback: each step is reversible separately — the pauses return as numbers, `raise_once` is changed back to `focus()`, the pixel reading is switched to the reference with one line (this path remains in the code as a fallback).
