# Registry of the owner's list `kkt-26.11.09`

Source — [`kkt-26.11.09.txt`](kkt-26.11.09.txt) (verbatim, two messages of 09.11.2026). Columns and
states — as in [`v26.10-index.md`](v26.10-index.md). His run: release kt26.11.09-85ea51da (stage 7:
compiled-curves, pipeline-prewarm) on AMD Radeon (RADV RAPHAEL_MENDOCINO, Vulkan). His files:
`/media/e/developer/kt.9/fails/26.11.09/` (`haski.kt`, `telemetry-journal.log`, `app.ron`); the
relinked copy: `/media/e/developer/kt.9/repro/26.11.09/haski-relinked.kt`.

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| I1 | 1 | the program feels "lazy" now | partial: the same mechanism as I2; his journal shows the UI thread free (frames 1–3 ms) while the preview never completes | I2 | diagnosed |
| I2 | 2 | a heavy Datamosh on the region `problem_region_26_11_09` (event 2149: Camera ISP → Datamosh gop 66, 720 lines, search 64 → VHS Tape) leaves the preview on "Loading", even on a pause | partial: a frame once composed under the chain is shown from the kept composite with no history, decode or plan (the trace of our heavy loop: every kept frame used to plan the Datamosh stage, 15 ms, and come out short where the memo had dropped the history); the first visit of a paused frame still computes its history — row 173, split by his next trace | `structured-telemetry` t2145, t2151, live `heavy_loop`; found-defects 173 | stage 2 done |
| I3 | 3 | a low playback rate does not clear "Loading" either | partial: a slow rate takes the same kept path (I2); the first lap at a slow rate still computes | I2 | stage 2 done |
| I4 | 4 | the project and logs are in `kt.9/fails/26.11.09` | partial: read; relinked copy made (his file untouched) | I2 | read |
| I5 | 6 | "нет": the frame does load in the end, after two to five seconds | partial: the bench and bisect withdrawn; his 2–5 s is the first visit's history (row 173), which his next run's `kt-cli trace loading` splits per position into device, wait and processor | `structured-telemetry`; found-defects 173 | telemetry done |
| I6 | 7 | the logs are a mess, unstructured — is it really convenient to work in them? | done structured-telemetry: typed records in `<data>/trace`, read by `kt-cli trace`, the journal kept for people + t2140–t2143 | `structured-telemetry` | done |
| I7 | 8 | telemetry such that the orchestrator understands what happens without shuffling versions or benches | done structured-telemetry: frames, decodes, cache, stream stages, compute, kept composites and mix, playback, jobs, edits and the machine joined by ids; `kt-cli trace loading/loop/during/slow/frame` + t2144, t2146–t2149 | `structured-telemetry` | done |
| I8 | 9 | the cache claim is wrong: passing a small piece a hundred times still does "loading" | done structured-telemetry: a kept moment is shown before anything is asked (probe plan, `StreamKernel::key`), a re-decoded frame keeps its serial, the slow route keeps, the look-ahead skips kept moments + t2145, t2150–t2152, live scenario `heavy_loop` | `structured-telemetry` | done |
| I9 | 11 | start `edit-without-rebuild` and `kernel-specialization` | partial: `edit-without-rebuild` archived 09.11 — one shared build per project version off the UI thread, per-event derived state memoized, a region change builds nothing; slider step on the relinked copy 87.8 → 1.4 ms UI, 197 → 40 ms CPU (t2000–t2008, live `edit_drag`); `kernel-specialization` archived 09.11 — kernels skip what stands still, same bytes (smoothers, filter/EQ re-tune on change, compressor below threshold exact, mixdown constant-weight runs, meter and peaks via `fast!`; t2170–t2176) | `edit-without-rebuild`, `kernel-specialization` | running |
| I10 | 13 | when these finish, go on with the others in the table | partial: standing go for the order table of `docs/engine-audit.md` — next `one-scheduler`, `gpu-arbiter`, `stream-layout-per-build`, `memory-ledger`, `fused-pointwise-runs`, the local cost queue, test hygiene; started as slots free, ≤3 at once | `one-scheduler`, `gpu-arbiter`, `stream-layout-per-build`, `memory-ledger`, `fused-pointwise-runs` | standing |
| I11 | 15 | do not wait for him: start the next rows yourself as executors finish | partial: the standing go of I10 needs no click of his on any turn; on a turn an event started, the orchestrator still cannot start a session (the app refuses), so it starts the waiting ones on the next turn it can — `one-scheduler`, `fused-pointwise-runs`, `test-hygiene` started 09.11 | `one-scheduler`, `fused-pointwise-runs`, `test-hygiene` | standing |
