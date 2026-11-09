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

### D9. The bench's own channel leaves the working disk

The program's geometry dump and the bench's command file lived in the run's folder on the working
disk. A write and rename there stood 10–20 s while builds and the sidecars synced the disk (the dump
thread's comment already recorded it), and a scenario then read a dump that many seconds old:
`space_hold`'s first hold played 17 s (the journal: 10 s → 72 s) while the dump showed the head
standing. `_channel.py` puts both on tmpfs (`$XDG_RUNTIME_DIR/kinetren-live/<run>`), emptied when a
program starts (a new program runs the last command it finds), and copies the dump and its companions
beside the screenshots when the scenario ends. A red stand scenario also gets the stand's journal and
trace (`stand-*`): the stand's next start wipes them.

### D10. A gesture waits for what the program says, not for a guess of it

- The double click after a click waits out twice the double-click window the program published
  (`double_click_s`): D4 widened it, and the script's own estimate fell short of the triple window
  (`formula` red in all three sets of the first proof run).
- `focus()` waits until the X server names the window focused: `decode_storm`'s Space reached no
  window (no PLAY in the run's journal).
- `poll_step` waits for one more program frame, so N attempts are N frames, not a stale clock
  (`stills`, `generator`).
- `idle` sweeps until frames have seen the pointer and waits for a frame after the sweep before
  judging rest: a stalled program read the sweep's moves seconds late, in the next window.
- `decode_storm` waits for the paused frame to come before judging the pause, and for a new frame
  instead of reading V-fps over 6 s.
- A program of the scenario's own, and the stand stopped for one, are gone only when their WINDOW is
  gone: the editor is the supervisor's child and dies after it, and `probe.window()` takes the first
  window of the class. The next program's Space, Ctrl+F and drag went to the dying window
  (`cached_range`, `readme_shots`, `welcome`, `lessons` in the second proof run).
- `_wait_ctl` waits while the stand draws: a fixed 30 s ran out while the stand, alive, opened a
  scenario's project after a heavy one (`formula`, `geomctl`, `fade`).
- `poll_step` waits for a frame the program draws by itself, never a poked one: `morphgrid` counts
  the frames after Stop.

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
| `cached_range` "a pause runs nothing" | a quick run | 166 UI frames in 4 s after a quiet pause; the trace shows no input and no work | rest checks print the repaint causes of the window (D9's evidence); not repeated since |
| `formula` | proof run 1, all three sets | D4's wider window made the script's triple wait short | D10 |
| `idle` | proof run 1 | the sweep's moves read seconds late, in the rest window (14 of 17 frames had input) | D10 |
| `decode_storm` | proof run 1 | Space sent before the window had the focus; the pause judged between two retries; V-fps over a fixed 6 s | D10 |
| `nestparam` | proof run 1 | the placed clips read from a stale dump; a bin menu that did not open (cause not seen: the stand's journal was wiped) | `app.until`; D9 keeps the stand's journal for the next red |
| `stills` | proof run 1 | the start's reading came from a frame in passage | the width must hold for two readings |
| `generator` | proof run 1 | the undo's 20 attempts ran out | D10 `poll_step` |
| `space_hold` | a quick run | the dump stood 17 s on the disk while the program played | D9 |
| `formula` | proof run 3, all three sets | the click on Done was read by a frame seconds after it was sent (a save's fsync held the UI 2–3.6 s, found-defects 252); measured from the send, the double click fell in egui's triple window, and a triple click on an event closes the window its double click opened | `click()` dates a click when the program has read it |
| `morph` | proof run 3 | the brightness read right after the click | `app.until` |
| `gpu_record` | proof run 3 | the first second's composite took the slow-chain route (counted "offline") | a second of the playback counts preview frames |

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

- Ctrl+S holds the UI thread for seconds on a busy disk (found-defects 252).
- Frames at ~100 a second with no input beside the index service, unverified (found-defects 253).

- `heavy_loop` lap 20 of the baseline: one moment composed because "layer 6 (source 5) is another
  stream frame" than the lap before: a Datamosh stage's output for one moment differed across laps.
  Under load the stage shows held frames (flagged and not kept); this one was not flagged.
  found-defects 250.
- At a display rate below the frame rate a covered loop never becomes fully kept: only shown moments
  are kept. A person on a slow machine sees "Loading" moments compose for the first time lap after lap.
  Composing the unshown moments of a loop in the background is a scheduler feature
  (`one-scheduler`), not this change's. found-defects 251.
