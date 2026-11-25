# Registry of the owner's list `kkt-26.11.09`

Source — [`kkt-26.11.09.txt`](kkt-26.11.09.txt) (verbatim, two messages of 09.11.2026). Columns and
states — as in [`v26.10-index.md`](v26.10-index.md). His run: release kt26.11.09-85ea51da (stage 7:
compiled-curves, pipeline-prewarm) on AMD Radeon (RADV RAPHAEL_MENDOCINO, Vulkan). His files:
`/media/e/developer/kt.9/fails/26.11.09/` (`haski.kt`, `telemetry-journal.log`, `app.ron`); the
relinked copy: `/media/e/developer/kt.9/repro/26.11.09/haski-relinked.kt`.

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| I1 | 1 | the program feels "lazy" now | done structured-telemetry, t2145, live `heavy_loop` (the mechanism of I2) | I2 | done |
| I2 | 2 | a heavy Datamosh on the region `problem_region_26_11_09` (event 2149: Camera ISP → Datamosh gop 66, 720 lines, search 64 → VHS Tape) leaves the preview on "Loading", even on a pause | done structured-telemetry, stream-history-on-device, t2145, t2151, t2355: a kept moment is shown with no history; the first visit's history walks on the device in batches | `structured-telemetry` t2145, t2151, live `heavy_loop`; found-defects 173 | done |
| I3 | 3 | a low playback rate does not clear "Loading" either | done structured-telemetry, t2145, t2150 (as I2) | I2 | done |
| I4 | 4 | the project and logs are in `kt.9/fails/26.11.09` | partial: read; nothing to build | I2 | read |
| I5 | 6 | "нет": the frame does load in the end, after two to five seconds | done structured-telemetry, stream-round-trips, t2148, t2710: the trace splits the first visit per position; its cost is the walk's | `structured-telemetry`; found-defects 173 | done |
| I6 | 7 | the logs are a mess, unstructured — is it really convenient to work in them? | done structured-telemetry: typed records in `<data>/trace`, read by `kt-cli trace`, the journal kept for people + t2140–t2143 | `structured-telemetry` | done |
| I7 | 8 | telemetry such that the orchestrator understands what happens without shuffling versions or benches | done structured-telemetry: frames, decodes, cache, stream stages, compute, kept composites and mix, playback, jobs, edits and the machine joined by ids; `kt-cli trace loading/loop/during/slow/frame` + t2144, t2146–t2149 | `structured-telemetry` | done |
| I8 | 9 | the cache claim is wrong: passing a small piece a hundred times still does "loading" | done structured-telemetry: a kept moment is shown before anything is asked (probe plan, `StreamKernel::key`), a re-decoded frame keeps its serial, the slow route keeps, the look-ahead skips kept moments + t2145, t2150–t2152, live scenario `heavy_loop` | `structured-telemetry` | done |
| I9 | 11 | start `edit-without-rebuild` and `kernel-specialization` | done edit-without-rebuild, kernel-specialization, t2000, t2173 | `edit-without-rebuild`, `kernel-specialization` | done |
| I10 | 13 | when these finish, go on with the others in the table | done one-scheduler, gpu-arbiter, stream-layout-per-build, memory-ledger, fused-pointwise-runs, local-cost-queue, test-hygiene, t2230, t2250, t2270, t2294, t2190, t2211 | `one-scheduler`, `gpu-arbiter`, `stream-layout-per-build`, `memory-ledger`, `fused-pointwise-runs` | done |
| I11 | 15 | do not wait for him: start the next rows yourself as executors finish | partial: a standing rule (the orchestrator starts the next rows itself) | `one-scheduler`, `fused-pointwise-runs`, `test-hygiene` | standing |
