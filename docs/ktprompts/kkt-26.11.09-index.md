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
| I2 | 2 | a heavy Datamosh on the region `problem_region_26_11_09` (event 2149: Camera ISP → Datamosh gop 66, 720 lines, search 64 → VHS Tape) leaves the preview on "Loading", even on a pause | no: defect, `video-plan`; his journal on the pause (+193…+201 s): Datamosh compute 13–24 jobs a second at ~8.5 ms each and 1–6 composites a second for 10 s, the frame never completes; not reproduced headless on llvmpipe (`pause-scan` 4423–4426 s settles in ≤1.2 s, `preview` of the region 97 % cache hits) — the live player's other consumers (prefetch keeps running on the pause) are the suspect | I5–I8 (he declined the bench and the version bisect; the telemetry must explain it) | telemetry first |
| I3 | 3 | a low playback rate does not clear "Loading" either | no: same as I2 | I2 | on his word |
| I4 | 4 | the project and logs are in `kt.9/fails/26.11.09` | partial: read; relinked copy made (his file untouched) | I2 | read |
| I5 | 6 | "нет": the frame does load in the end, after two to five seconds | partial: the bench and bisect proposal withdrawn; the wait itself (I2) is to be explained by the telemetry (I7) and fixed after | `structured-telemetry` | running |
| I6 | 7 | the logs are a mess, unstructured — is it really convenient to work in them? | partial: the journal is ASCII lines meant for a reader; it cannot say why a frame waited | `structured-telemetry` | running |
| I7 | 8 | telemetry such that the orchestrator understands what happens without shuffling versions or benches | partial: change `structured-telemetry` — a structured trace with causes, queried by a tool, cheap at rest | `structured-telemetry` | running |
| I8 | 9 | the cache claim is wrong: passing a small piece a hundred times still does "loading" | no: defect, `video-plan`/`audio-render` — a covered loop with a heavy chain is not kept; explained by I7's trace, fixed in its second stage | `structured-telemetry` | running |
