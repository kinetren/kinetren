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
| J2 | 2 | on `problem_region_26_11_09` at 720 lines the preview hangs with no "Loading" and little processor load, plays in some places and not others; a small region starts to move only after many laps; the counter says 20 V-fps where at most 5 are new | partial: `stream-history-on-device` — the Datamosh history walks on the device in batches (one wait per up to 32 positions, not two per position), the arbiter admits a piece against the time in flight, V-fps counts new pictures (held frames apart), a held frame says "catching up", the trace gives a job's device start on the CPU clock (t2350–t2363); not measured on a card (found-defects 406) | found-defects 400–406 | stream-history-on-device |
| J3 | 2 | a pause shows "Loading" and only then computes the region properly; the processor is barely used, as if it computed in real time with sleeps | partial: `stream-history-on-device` — the per-position round trips are gone (the walk is batched on the device), on llvmpipe the walk is device-bound (found-defects 400) | found-defects 400, 406 | stream-history-on-device |
| J4 | 3 | the cache looks underused: the status bar says 39/535 MB while the program takes 4.7 GB; why 535 and not more; the preview quality barely changes it; Diagnostics says 100 % (535/537) | partial: diagnosed from his trace — the status bar is audio/video MB, not used/limit; the decoded-frame cache never asks the ledger to grow (found-defects 396, 397) | found-defects 396, 397 | diagnosed |
| J5 | 4 | logs and traces are in `kt.9/fails/26.11.10` | partial: read (copied to the orchestrator's scratchpad, his folder untouched) | J1–J4 | read |
| J6 | 6 | yes: open the sound device on Play (found-defects 274) | partial: decided; goes to an executor | found-defects 274 | decided |
| J7 | 10 | after a jump forward, while "Loading", the second green strip on the Datamosh event creeps up to the playhead: is it computing what he never watched, and why, with gop=15? | partial: `stream-history-on-device` — a re-walk after a jump or an eviction starts at the latest checkpoint (every 16 positions, pinned outside the LRU); the first visit still walks from the anchor — the effect's physics with mosh on (found-defects 395) | found-defects 395 | stream-history-on-device |
