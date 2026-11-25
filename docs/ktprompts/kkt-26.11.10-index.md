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
| J1 | 1 | a short loop (230 ms) starts with silence again — on `rendereg` with effects, not on `зацикленность_18` nor `problem_region_26_11_09` | done kept-mix-settle: leaving the kept mix the effects go on through the replayed frames (or warm their settle length through the loop) before a chunk is kept, so a lap rings the lap before it; his trace's sequence on the relinked copy keeps the continuous lap byte for byte + t2370–t2374 (t2374 at the device); whether what he heard was only this — found-defects 423 | `kept-mix-settle` | done |
| J2 | 2 | on `problem_region_26_11_09` at 720 lines the preview hangs with no "Loading" and little processor load, plays in some places and not others; a small region starts to move only after many laps; the counter says 20 V-fps where at most 5 are new | done stream-history-on-device, gpu-throughput, catch-up-shows-loading, t2355, t2251, t2410; his card is C1 | found-defects 400–406 | done |
| J3 | 2 | a pause shows "Loading" and only then computes the region properly; the processor is barely used, as if it computed in real time with sleeps | done stream-history-on-device, gpu-throughput, t2355, t2432: the walk is batched on the device and goes ahead on a pause | found-defects 400, 406 | done |
| J4 | 3 | the cache looks underused: the status bar says 39/535 MB while the program takes 4.7 GB; why 535 and not more; the preview quality barely changes it; Diagnostics says 100 % (535/537) | done frames-cache-growth: the preview's frames cache asks the memory ledger for its look-ahead and loop at the preview quality and gives it back to a render; the status bar reads frames held/bound and the program's memory, Diagnostics names the caches and the rest + t2390–t2399, live `diagnostics` | `frames-cache-growth` | done |
| J5 | 4 | logs and traces are in `kt.9/fails/26.11.10` | partial: read; nothing to build | J1–J4 | read |
| J6 | 6 | yes: open the sound device on Play (found-defects 274) | done kept-mix-settle: an `audio-output` thread opens it on Play and closes it a second after the play stopped; a pause wakes nothing of the sound + t2380–t2384, live `idle`; 2026-11-15 the owner kept play-start-and-prefetch's form: the device opens on the first Play, then pauses at rest and Play resumes it (user-feedback 2026-11-15) | `kept-mix-settle` | done |
| J7 | 10 | after a jump forward, while "Loading", the second green strip on the Datamosh event creeps up to the playhead: is it computing what he never watched, and why, with gop=15? | done stream-history-on-device, t2360: a re-walk starts at the latest checkpoint; the first visit walks from the anchor (the effect's physics) | found-defects 395 | done |
| J8 | 12 | too many questions to him; debug tests burn the processor; no guard against long computations — something burned the CPU for ~10 hours | done test-stability, one-process-tests, t1671: every test waits with a deadline, nextest kills a test past 15 minutes; the rule is in every brief | `.config/nextest.toml` | done |
