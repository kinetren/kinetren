# Proposal: test-hygiene

## Why

Row 12 of the order table in `docs/engine-audit.md` (mechanism M11), started on the owner's standing go
of 2026-11-09 (`docs/ktprompts/kkt-26.11.09-index.md` I10, I11: "когда эти закончат - продолжай другие
делать по таблице"). CLAUDE.md: "A red under load is a cause to find, not noise to rerun: a retry of a
test is not a fix" (t1671). Several stage checks this week passed only because `live-check.sh` reran a
red scenario alone, and the rerun overwrote what the red try had left.

The class: a check red only under a loaded full run and green alone, and a bench that destroys the
evidence of its own red. Rows 47, 99, 101, 126, 133, 158, 163, 165, 166 and 170 of
`docs/found-defects.md`, plus every live scenario red on the first try in the last finish.sh logs and
in this change's own loaded runs.

## What Changes

- **The bench keeps the evidence and starts clean** (rows 163, 165): a scenario's folder is emptied
  before a run, the lone retry keeps the red try in `<scenario>/first-try/`, and each try's lines are in
  its folder (`said.txt`). t2210.
- **Keys a slow frame reads all run, in order** (rows 99, 166): the frame's commands were a set, so
  → → → ← read together moved nothing and two Ctrl+Z undid one edit. t2211.
- **Space keeps its meaning when read by a slow frame** (row 166): two presses in one frame are the
  double press; a hold is proven by a later frame that still finds the key down, so a tap read late is
  a tap, not nothing; the double-press window widens by the busy frame the program measured. t2212,
  t2213.
- **egui's click windows widen by the busy frame** (row 166, `formula`, `lessons`, `forms`): a double
  click whose two clicks were read a slow frame apart was two single clicks. t2214.
- **A long frame's journal note says how much of it waited for a processor**; `ui_wait` judges the rest.
  t2215.
- **t1913/t1914 interleave the scenes' rounds** (row 126) and stand on t1671's list as a processor-time
  check; t1671 now also refuses an `.elapsed()` bound of a test against anything but the ten-minute
  guard, and any processor-time reading in a test, outside its list.
- **t1935's bench tells its own device thread's stall from the render's** (row 158): an underrun the
  bench's ordinary-priority device thread made by waking later than the ring lasts is the machine's.
- **Live scenarios read consequences** (`forms`, `stills`, `loop`, `readme_shots`, `unsaved`,
  `space_hold`, `fxpreset`), **judge a covered loop by the trace's reasons** (`heavy_loop`,
  `cached_range`) and **wait for a quiet program by its threads too** (`idle`, rows 133, 170).
- Nothing is retried or weakened: the lone rerun of `live-check.sh` (the owner's R.68) stays, and the
  proof of this change is three full live sets in parallel with a workspace test run, without a lone-retry
  pass for these scenarios.

## Capabilities

### Modified Capabilities

- `durability`: the live bench's folders and the first try's evidence.
- `ui-timeline`: keys and clicks read by a slow frame.
- `transport`: Space read by a slow frame.
- `telemetry`: a long frame's run-queue wait.
- `distribution`: t1671's wider scan and list.
- `audio-render`: t1935's bench attribution.

## Impact

`src/scripts/live-check.sh`, `src/scripts/live/{worker,kt_live,idle,space_hold,forms,stills,
loop,readme_shots,unsaved,heavy_loop,cached_range,ui_wait,fxpreset,_flaky,_red}.py`,
`src/apps/kt-app/src/{app,controls,frame_cost,timeline}.rs` and their tests,
`src/libs/kt-engine/src/preview_bench.rs`, `src/libs/kt-engine/tests/mix_player.rs`,
`src/apps/kt-cli/tests/dist_guard.rs`.
