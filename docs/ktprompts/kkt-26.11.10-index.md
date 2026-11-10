# Registry of the owner's list `kkt-26.11.10`

Source — [`kkt-26.11.10.txt`](kkt-26.11.10.txt) (verbatim, one message of 10.11.2026). Columns and
states — as in [`v26.10-index.md`](v26.10-index.md). His run: release kt26.11.09-87f6f38d (stage 9:
one-scheduler, fused-pointwise-runs, stream-layout-per-build, gpu-arbiter, memory-ledger,
test-hygiene). His files: `/media/e/developer/kt.9/fails/26.11.10/` (`telemetry-journal.log`,
`trace/` — 5 files, 530 k records, 25 min); the project is the 26.11.09 one (relinked copy
`/media/e/developer/kt.9/repro/26.11.09/haski-relinked.kt`).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| J1 | 1 | a short loop (230 ms) starts with silence again — on `rendereg` with effects, not on `зацикленность_18` nor `problem_region_26_11_09` | partial: diagnosed from his trace — a live chunk after an effects reset is kept cold across the wrap and replayed every lap (found-defects 390) | found-defects 390 | diagnosed |
| J2 | 2 | on `problem_region_26_11_09` at 720 lines the preview hangs with no "Loading" and little processor load, plays in some places and not others; a small region starts to move only after many laps; the counter says 20 V-fps where at most 5 are new | partial: diagnosed from his trace — the Datamosh history advances on one thread by round trips (12 % device time), the arbiter waits for each piece, a held stream frame shows with no Loading, V-fps counts tick changes (found-defects 391–394, 398) | found-defects 391–394 | diagnosed |
| J3 | 2 | a pause shows "Loading" and only then computes the region properly; the processor is barely used, as if it computed in real time with sleeps | partial: diagnosed with J2 — on a pause the same walk runs at a shorter round trip and Loading is shown because a held frame is no longer allowed (found-defects 391) | found-defects 391 | diagnosed |
| J4 | 3 | the cache looks underused: the status bar says 39/535 MB while the program takes 4.7 GB; why 535 and not more; the preview quality barely changes it; Diagnostics says 100 % (535/537) | partial: diagnosed from his trace — the status bar is audio/video MB, not used/limit; the decoded-frame cache never asks the ledger to grow (found-defects 396, 397) | found-defects 396, 397 | diagnosed |
| J5 | 4 | logs and traces are in `kt.9/fails/26.11.10` | partial: read (copied to the orchestrator's scratchpad, his folder untouched) | J1–J4 | read |
| J6 | 6 | yes: open the sound device on Play (found-defects 274) | partial: decided; goes to an executor | found-defects 274 | decided |
| J7 | 10 | after a jump forward, while "Loading", the second green strip on the Datamosh event creeps up to the playhead: is it computing what he never watched, and why, with gop=15? | partial: diagnosed — the strip is decoded source frames ordered from the chain's anchor; with mosh on every I-frame is dropped, so gop does not bound the walk (found-defects 394, 395) | found-defects 394, 395 | diagnosed |
| J8 | 12 | too many questions to him; debug tests burn the processor; no guard against long computations — something burned the CPU for ~10 hours | partial: an orphan `kt_time` test binary of `gpu-arbiter`'s removed worktree ran 21 h at 100 % of a core, killed; nextest now kills any test past 15 min (`.config/nextest.toml`); every brief: tests only under nextest or `timeout`, no process left at the report; fixes J1–J6 started without asking (`stream-history-on-device`, `kept-mix-settle`, `frames-cache-growth`) | `.config/nextest.toml` | running |
