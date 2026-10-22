# Proposal

## Why

R.68 taught the live check to wait for the PROGRAM rather than the clock, but only for the probes: `wait_frame`, `wait_seen` and `settle` are built and work when a scenario looks for a widget in the dump. The GESTURES THEMSELVES still sleep blindly, and that is **387 s out of 1582 s** of a sequential live run (24 %): `click` — 0.78 s, `key` — 0.77 s, `drag` — 2.55 s, typing a line — 0.77 s PER CHARACTER. Alongside this, reading the screen: about 341 times per run the harness takes a full 1920×1080 frame via `xwd` and parses it with two ImageMagick processes just to learn the colour of ONE pixel.

It is not only about minutes. A fixed pause does not stretch under load — this is exactly the blindness that, in R.59 §468, made green scenarios "turn red all at once", and because of which the ban on parallelism survived nine rounds. While the last 410 waits stand on the clock, the class of false reds is only half closed.

The measurement revealed two more things, and both show that the rule does not hold:

- **the guard `live-sleeps.py` does not see `src/scripts/ui-probe.py`** — the mouse and keyboard driver sleeps inside itself (`move` 0.05 s, `button` 0.09 s, `key` 0.05 + 0.12 s), and 123 s of the 387 live OUTSIDE the scope of the rule "pause only with a reason". The harness already knows how to pass its pause to the driver — the docstring of `button()` says exactly this, "the harness passes almost zero and waits for the FRAME itself" — but not a single call does it;
- **reasons in words turned out to be not enough.** All 265 s inside the harness itself are legitimate pauses with an explanation longer than forty characters. Prose did not hold: the same lesson as the window registry (R.66) and the PATH guard (R.67) — an exception must be MEASURABLE.

The choice of round belongs to the owner (2026-09-29, `docs/user-feedback.md`, R.69): from the deferred list he named this item first.

## What Changes

- **A gesture waits for the READ INPUT, not the clock.** `click`, `right_click`, `double_click`, `drag`, `key`, `wheel` and `hover` take the `seen()` mark before the gesture and wait with `wait_seen`: the program itself says how many input events it has read. The mechanism already exists and is in use — it has simply not been carried over to the gestures. Pauses that have no observable consequence (the egui double-click time threshold, the step of a drag between positions, the autosave clock) remain and stay enumerable.
- **The X driver stops sleeping on behalf of the harness.** The calls `probe.move`/`probe.button`/`probe.key` from `kt_live.py` pass their own pause (almost zero), as was intended; the built-in defaults of `ui-probe.py` are kept for a manual run from the command line, where there is no one to wait for.
- **The window is raised when the WINDOW CHANGES, not on every gesture.** `raise_once` was written in R.68 and is not connected: `click`/`key` call `focus()` unconditionally, which is 0.2 s plus a round trip to the X server for each of the 410 gestures.
- **A line is typed whole.** `type_text` sends all the characters and waits ONCE: egui reads input events per frame, and the whole line lands in one frame. The decision is recorded in R.68 (`design.md`, decision 2) and has not been implemented.
- **Reading a screen point stops being a screenshot.** `pixel`/`pixel_settled`/`ink_box` read a rectangle through X (`get_image` via python-xlib) instead of `xwd` + two `convert` calls. `xwd` remains the REFERENCE: the eframe GL window lives in a 32-bit visual, and ImageMagick on the root window already returned BLACK (R.33 item 228) — the new path is accepted only after a check against the reference on the same frame. `shot()` for artifacts and for the steps of `PATH` remains as it is.
- **Sleep gets a BUDGET, not only a reason.** The harness counts the total fixed sleep of a run and prints it; the guard `live-sleeps.py` is extended to `ui-probe.py` and fails when the declared limit is exceeded — a ratchet of the same kind as `UNTOUCHED_LIMIT` in the coverage report.

Not included: reducing the set of checks (the rule "tests from the specs are not weakened" outranks the budget), changes to the product (`kt-app` does not change — the input and frame counters already exist), analysis of the fixed costs of a scenario (project startup, `wait_loaded`, fixture generation) and speeding up the workshop run — both are named as the floor in `durability/design.md` and are handled separately.

## Capabilities

### New Capabilities

(none — the rules of the live check are already described by the requirements of `ui-reach`)

### Modified Capabilities

- `ui-reach`: the requirement "Live stand: the process is reused, the wait goes by the FRAME, not by the clock" — waiting for an observable event is extended from the probes to the gestures themselves and to the X driver; "enumerable" acquires a measurable meaning (a declared budget of total sleep instead of a reason in words alone); the scope of the guard is extended to `ui-probe.py`; reading a screen point goes through X, while the snapshot via `xwd` remains the reference for the check.

## Impact

- `src/scripts/live/kt_live.py` — gestures, `raise_once`, `type_text`, pixel reading, accounting of total sleep.
- `src/scripts/ui-probe.py` — calls from the harness pass their own pause; the defaults for a manual run do not change.
- `src/scripts/live-sleeps.py` — the guard's scope includes the X driver; the sleep budget as a ratchet.
- 45 scenarios `src/scripts/live/*.py` — the verdicts of all 45 must match the current ones (this is the acceptance test). The claim that "by construction, no edits are required" was NOT confirmed: two scenarios needed an explicit `settle()` — `forms` counts the fields right after the animated collapse, `analysis` compares the SCREEN right after hovering. Previously a fixed pause inside the gesture was hiding this; now "the program has read the gesture" and "the picture has settled" are different waits, and the second is requested by whoever reads the dump or the pixels directly.
- `openspec/specs/ui-reach/{spec,design}.md`; new tests from **t807**.
