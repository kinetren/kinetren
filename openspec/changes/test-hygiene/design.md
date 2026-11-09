# Design: test-hygiene

## Context

Row 12 of `docs/engine-audit.md`'s order table, mechanism M11: "tests that depend on the wall clock or
on load". The owner's rule (CLAUDE.md, t1671): a red under load is a cause to find, not noise to rerun.
The lone rerun of `live-check.sh` (R.68) stays; what changes is that nothing it reruns goes unexplained.

The evidence: the finish.sh logs of the last stages (finish-5…9, read only), and this change's own
loaded runs — three full live sets in parallel with `cargo test --workspace` (load up to 142), t1935
in a loop beside a full `cargo nextest`, and t1935 beside 128 busy loops on 32 cores (load 124–192).
Every first-try red of those runs is in the walk below, with its cause.

## The mechanism

A check is red under load and green alone when it measures, or waits by, a clock the load stretches.
Three forms of it were found, and one product defect each form had been hiding:

1. **The program judged input by frame times.** egui hands a frame's events without their moments; a
   press read in a frame happened during the frame before. On a 600 ms frame the program merged and
   misread input: a person on a loaded machine (a build running, software rendering) suffers the same.
2. **The scenario read before the consequence.** A dump written before the gesture landed, a "quiet"
   judged on counters while a job was in flight, a polling loop that counted attempts.
3. **A wall-clock or processor-time number compared across moments of a loaded machine.**

## Decisions

### D1. The bench keeps the evidence and starts clean (rows 163, 165)

`worker.py` empties a scenario's folder before a try; on the lone retry (`KT_LIVE_TRY=2` from
`live-check.sh`) the red try's folder becomes `<scenario>/first-try/`. Each try writes what it said into
`said.txt` beside its screenshots. The stand drops its dump's companions (`geom.json.*`) on start.
Rejected: keeping the folder and deleting known files (`.crash.json`) one by one — every new artefact
would be the next stale one. The cost of a clean folder is the sidecar index of the fixtures being
rebuilt: not measurable in the run time (synthetic media).

### D2. Every key a frame reads runs, in order (rows 99, 166)

`Controls::pressed` returns `Pressed`, a fixed array of the frame's commands in event order (no
allocation, invariant 12), and `handle_keys` runs each. A set made → → → ← nothing (+1 −1, the
`space_hold` red of kt26.11.09-d13a8fbc exactly) and two Ctrl+Z one undo. Window openers and Save run
once a frame; a command that opens a text field ends the frame's keys, as a person's next keys would
have gone into the field. Esc and Enter run once per press.

### D3. Space: presses in order, a hold proven, the double window widened by the busy frame (row 166)

- Presses and releases are handled in the order read: two taps in one frame are the double press.
- A hold is proven by a frame that began `SPACE_HOLD_MS` after the frame that read the press and found
  no release (`handle_keys` runs before `tick_space_boost`). A release of an unproven press is a tap.
  Before, a release read by a frame past the threshold did NOTHING: neither a tap (`elapsed ≥ 300`) nor
  the end of a hold (never marked held).
- The double-press window is measured between frame starts and widens by `KeyClock::busy`: the smoothed
  gap before a frame the previous one asked for at once (≤ 33 ms repaint: playback, a held key, the
  frame after a gesture). An idle wait never enters it, so a resume 2 s after a pause stays a resume.
  On a card the widening is ~16 ms; on 450 ms frames a quick pair stops as the person meant.
- Rejected: event timestamps. winit drops X11's event time, and eframe delivers a slow frame's events
  together after it; there is no moment to read. Rejected: "two presses read in neighbouring frames are a
  double press" — a program idle after a pause draws no frames, and a resume seconds later would be read
  in the next frame.

### D4. egui's click windows widen by the same busy frame (row 166, `formula`, `lessons`, `forms`)

egui stamps a click with the frame that read it. `KeyClock::widen_clicks` sets
`max_double_click_delay = 0.3 + busy` and `max_click_duration = 0.8 + busy` each frame they change. The
harness keeps egui's windows (`KeyClock::widen` false under `cfg(test)`): its clock is a fixed
`step_dt` while `busy` is the wall clock, and t2214 checks the setting itself. The live framework's
double click waits out the triple window `0.6 + 3 × frame` since the window grew.

### D5. A long frame's note names its run-queue wait; `ui_wait` judges the rest

`frame_cost::run_delay_ns` reads `/proc/thread-self/schedstat` (an fd kept per thread, no allocation);
the long-frame note adds "N ms of it waiting for a processor". `ui_wait` subtracts it. The red at load
142 was a 410 ms frame with 10 ms of work. A lock or a file wait stays counted whole.

### D6. t1913/t1914 interleave their rounds; t1671 sees processor clocks and constant bounds (row 126)

`frame_costs` takes the scenes' rounds in turns, seven of them, each scene's cheapest: the two numbers
meet the same moments of the machine. They stay a processor-time check (the frame cost IS what they
guard; t1912 counts the visits deterministically beside them) and are listed in t1671. t1671's scan
adds, for test code only, any `.elapsed()` bound other than the ten-minute guard and any `thread_ns()`
reading; the telemetry and update-check bounds (the request timeout a blocked call would take) are
listed as limits that are the check. Rejected: counting document copies instead of timing — a counter
in `TimelineModel::document()` misses `project().clone()` and every walk, which is exactly what the
timing catches.

### D7. A start's first pass reaches the ring whole (row 158)

The structured trace of a red t1935 (load 192, recorded with `KT_TEST_TRACE`) put the two underruns in
the first lap: the start's pass rendered 17 bursts live in 83 ms, pushing each, the callback primed on
the first burst, and the render thread was put off between bursts. The bench's device thread was on
time (≤ 29 ms late, now printed by `BenchReport::device_late_max_ms`), so the first hypothesis of the
row — the bench's own ordinary-priority device thread catching up — was wrong, and the instrumentation
that disproved it stays as a printed diagnostic, judging nothing. The render thread now collects the
start's pass in a buffer allocated once and commits it with one `write_chunk_uninit`; the callback sees
nothing of the pass, then all of it. Time stands before the first frame, so it costs no sync; the start
waits ~5 ms longer in release. The seam `hold_start_bursts_for_tests` (one ring's worth of bursts, zero
in the product) makes the preemption happen on any machine: t2216 is red on the old push (5122 frames
of underrun) and green now. This is not t1935's test race: a person's machine under a build plays the
same start.

### D8. Scenarios read consequences

`forms`, `stills`, `loop`, `readme_shots`, `unsaved` read with `app.until` or assert the consequence
over the whole history; `space_hold` double-taps as a person does (two presses 0.1 s apart, no wait
between); `fxpreset` takes the program's probe as the proof the window opened (a split double click
selected the event and changed the screen by 1 % too); `heavy_loop`/`cached_range` judge by the
trace's reasons; `idle.wait_quiet` asks the program's threads too.

## The walk (what was checked, and found)

### Rows of `docs/found-defects.md`

| Row | Cause | Fix |
|---|---|---|
| 163 | the retry rewrote the scenario folder | D1, t2210 |
| 165 | the folder kept files of earlier runs (`geom.json.crash.json` of a build of the day before) | D1, t2210 |
| 166 `space_hold` | → → → ← read in one frame cancelled (a set); two taps merged; a tap read late did nothing | D2, D3; t2211–t2213; scenario double-taps as a person |
| 166 `unsaved` | moved-clip read from a dump written before the release landed; the rest of its reds followed (nothing to save, no question) | D8 `app.until` |
| 166 `formula` | a double click split by a slow frame into two single clicks | D4, t2214 |
| 99 | the one-press-per-frame read of Space and of commands | D2, D3 |
| 101 | `space_hold` (above) and `readme_shots` (the bin width read from a stale dump; a double click) | D4, D8 |
| 47 | not reproduced in five finish logs nor three loaded live sets; the last guess (a text field holds the keys) is now in the dump (`keys_gated`) and in the red's message, the window's opening is proven by its probe | diagnostics, D8; stays open with its next step named |
| 126 | two scenes timed one after the other at different moments of a loaded machine | D6 |
| 133, 170 | "quiet" judged on counters while a job was in flight: a reel preview or a recheck that ends past three still seconds lands in the "rest" window | `wait_quiet` asks the threads too; not reproduced in this change's runs, so the cause is the likeliest one, not a proven one |
| 158 | a start's pass handed to the callback burst by burst (product) | D7, t2216 |

### Live scenarios red on the first try

| Scenario | Seen in | Cause | Fix |
|---|---|---|---|
| `loop` | finish-5, finish-9 | asserted the FIRST playhead read after Space was already the region start; the reads before Space took effect stand at 5 s | assert no read between 5 s and the region start |
| `readme_shots` | finish-9 | bin width read right after the drag | `app.until` |
| `renderregion` | finish-6 | stale crash file (row 165) | D1 |
| `hwencode`, `render`, `renderregion` | finish-5 | red twice: the libva driver crash in a test process (row 159, fixed by `render-crash-and-recovery` since) | none here |
| `formula`, `lessons`, `forms` | this change's baseline | double click split by a slow frame | D4 |
| `forms` | baseline (also red alone) | field count read after `settle` | `app.until` |
| `stills` | baseline | 20 attempts of a fixed step | `app.until` |
| `heavy_loop` | baseline | at a display rate below the frame rate each lap shows a different subset of frames; a moment shown for the first time is composed once ("for the first time (kept since)" ×9 over 6 s) | the trace's reasons: nothing kept is composed again |
| `ui_wait` | baseline | a 410 ms frame of 10 ms work at load 142 | D5 |
| `cached_range` | (same check as `heavy_loop`) | | the trace's reasons |

### Workshop tests with a clock or a sleep

- t1671's list before: `sidecar_stall` (3 s inside the 5 s busy timeout), `video_stall` (250/900 ms,
  the ring is the yardstick). Unchanged.
- Found by the wider scan: `telemetry.rs` (enqueue, exit) and `update_check.rs` (start/exit): each
  bound is the request timeout a blocked call takes; listed. `frame_cost_tests.rs`: D6; listed.
- `preview_throughput.rs:487` `wall_s >= 0.9` is a lower bound load cannot break. `video_stall.rs:80`
  compares two moments' order. `kernel_specialization`/`build_cost` count, never time.
- Sleeps in harness tests (t1620, t1621, t2212, t2213) are the input's timing, not a bound; each
  outcome holds at any load, since load only lengthens a frame and the rules judge by frame starts.
- t1935: D7. The baseline loaded `cargo test --workspace` was otherwise green.

### Found, not fixed

- `heavy_loop` lap 20 of the baseline: one moment composed because "layer 6 (source 5) is another
  stream frame" than the lap before: a Datamosh stage's output for one moment differed across laps.
  Under load the stage shows held frames (flagged and not kept); this one was not flagged.
  found-defects row 250.
- At a display rate below the frame rate a covered loop never becomes fully kept: only shown moments
  are kept. A person on a slow machine sees "Loading" moments compose for the first time lap after lap.
  Composing the unshown moments of a loop in the background is a scheduler feature
  (`one-scheduler`), not this change's. found-defects row 251.
