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
| J1 | 1 | a short loop (230 ms) starts with silence again — on `rendereg` with effects, not on `зацикленность_18` nor `problem_region_26_11_09` | question: why does the loop's first burst come out silent on this region only? — diagnosed from his trace (the audio start of a loop, the kept final mix, the parked render thread of `one-scheduler`) | his trace | diagnosing |
| J2 | 2 | on `problem_region_26_11_09` at 720 lines the preview hangs with no "Loading" and little processor load, plays in some places and not others; a small region starts to move only after many laps; the counter says 20 V-fps where at most 5 are new | question: what does each lap wait on, and what does V-fps count? — diagnosed from his trace (the look-ahead paused during playback, the GPU arbiter's bound, the stream history's pacing) | his trace | diagnosing |
| J3 | 2 | a pause shows "Loading" and only then computes the region properly; the processor is barely used, as if it computed in real time with sleeps | question: why does the paused computation use so little processor? — diagnosed from his trace as J2 | his trace | diagnosing |
| J4 | 3 | the cache looks underused: the status bar says 39/535 MB while the program takes 4.7 GB; why 535 and not more; the preview quality barely changes it; Diagnostics says 100 % (535/537) | question: what does each number count, and why does the frames' share not grow? — decoded frames only vs the ledger's tenants; why the two numbers disagree | his trace, `memory-ledger` | diagnosing |
| J5 | 4 | logs and traces are in `kt.9/fails/26.11.10` | partial: read (copied to the orchestrator's scratchpad, his folder untouched) | J1–J4 | read |
| J6 | 6 | yes: open the sound device on Play (found-defects 274) | partial: decided; goes to an executor | found-defects 274 | decided |
