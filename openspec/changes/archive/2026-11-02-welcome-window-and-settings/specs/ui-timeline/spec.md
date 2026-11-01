## MODIFIED Requirements

### Requirement: The performance panel tells the person the truth
The underrun counters SHALL be labelled (render / callback); callback starvation
is also shown in milliseconds of silence; "FPS" means UI frames, video is discussed in
V-fps and "V-skipped cells". The SPEED SHALL be judged by the release build:
the dev profile is built with `opt-level = 1`, and the tempo measurement is `kt-cli probe --time`.
The performance panel SHALL be a section of the Diagnostics window, not a box of the left column:
the column belongs to the media bin, and the always-visible layer is the status line (vfps, cache,
underruns in ms). The Diagnostics window SHALL carry all three graphs — Quality (FPS, V-fps,
A-hit, V-hit), FFmpeg activity (Total, Open, Read, V-dec, A-dec) and Cache churn (evict, miss,
no-read) — next to every line the box had, the cache occupancy bar and the problem log; the
graphs SHALL share the window's height, so a larger window shows larger graphs. A graph SHALL
be redrawn from what was built for its last sample: a frame without a new sample rebuilds
nothing, and an open Diagnostics window SHALL NOT make an idle program repaint more often.
Why: the user read `Underruns X/Y` as L/R; in the debug build the audio render
of a dense edit ran at 112 % of real time against 10 % in release. The owner, kkt-26.11.01
item 14: "Perfomance надо переместить в окно diagnostics, чтобы там три графика было" — the box
squeezed the bin all the time, and diagnosing a slow spot meant looking in two places.

#### Scenario: Underrun labels and silence in ms
- **WHEN** the performance panel and the status line show starvation
- **THEN** both counters are labelled, the callback silence is expressed in milliseconds
- **TEST** t289 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Tempo is measured by the tool, not by eye
- **WHEN** one needs to answer "does the audio render keep up with real time"
- **THEN** `probe --time` prints the fraction of real time and `OVER BUDGET`, not a UI impression
- **TEST** t290 (`src/apps/kt-cli/tests/cli.rs`, capability `transport`)

#### Scenario: The left column is the bin's, the panel is in Diagnostics
- **WHEN** the editor is open with a project and the Diagnostics window is closed
- **THEN** no "Performance" title, graph or perf line is painted in the left column; the bin's
  rectangle reaches the bottom of the column; the gauge button of the toolbar and View →
  Diagnostics… both open the window
- **PATH** look at the left column → only the media bin, down to the bottom; click the gauge
  button in the toolbar → the Diagnostics window opens with its "Performance" section at the top;
  close it with the cross; View → Diagnostics… → the same window opens again
- **TEST** t1145 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: Nothing of the former box is lost
- **WHEN** the Diagnostics window is open
- **THEN** it paints the three graph titles (Quality, FFmpeg activity, Cache churn) and every
  line the box had — cache MB, underruns render/out, preview size, clips and media, latency and
  callback, V-shown and V-skip/back, the FX backend, the compose cost, the FFmpeg totals — and the
  "Save the playback journal" button; the "Economy UI" switch lives only in File › Preferences…, with its hint
- **PATH** open View → Diagnostics… → the window shows, top to bottom: the "Performance" rows in
  two columns, the GPU/FX/compose/FFmpeg lines and "Save the playback journal", the cache
  occupancy bar, the three graphs one under another, the problem log; File › Preferences… → hover "Economy UI"
  → the status line says what the switch does
- **TEST** t1146 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: The graphs grow with the window
- **WHEN** the Diagnostics window is dragged by its bottom-right corner to a larger size
- **THEN** each of the three graphs gets taller, all three and the log stay inside the window,
  and the window keeps the size it was given over the next frames (it does not grow by itself)
- **PATH** open Diagnostics → drag its bottom-right corner down and right → the three graphs
  stretch to the new height, the log stays at the bottom; leave the mouse → nothing moves
- **TEST** t1147 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: An open Diagnostics window does not heat an idle program
- **WHEN** the Diagnostics window is open and nothing plays or moves
- **THEN** the frame asks for its next repaint no sooner than the idle tick (≥ 100 ms) and the
  graphs rebuild nothing between frames; during playback they follow the samples
- **PATH** open Diagnostics and leave the mouse → the graphs stand still, the program is idle;
  press Space → the lines run; Space again → they stop
- **TEST** t1148 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: A graph is drawn from its last sample's build
- **WHEN** a rolling graph is drawn several times without a new sample, then gets one sample,
  then is drawn in a larger rectangle
- **THEN** the repeated draws reuse the built lines (no rebuild), the new sample rebuilds once,
  the larger rectangle rebuilds once; the auto-scale moves only on a sample, so ten draws of the
  same data paint the same scale
- **PATH** open Diagnostics and press Space → the lines step on every half-second sample and hold
  still between samples; the scale eases to a new peak over a few samples, as fast at the idle
  tick as at 60 fps
- **TEST** t1149 (`src/apps/kt-app/src/graphs.rs`)
