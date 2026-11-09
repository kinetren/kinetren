# Tasks

## 1. Baseline (before numbers)

- [ ] 1.1 `kt-cli showcase --mosh` (Datamosh on the montage) and a baseline trace of `kt-cli preview` on it (release, llvmpipe): compute jobs and device submits per second by cause (frame vs prefetch pass), recorded in design.md D-measure
- [ ] 1.2 `kt-cli gpu-share` bench skeleton (one device: display + offline + compute host) measuring present latency with the current code; numbers into design.md D-measure

## 2. Classes and the ring (kt-time, kt-gpu)

- [ ] 2.1 `kt_time::sched::current_class`/`with_class`/`ClassGuard`, set by the worker around a lane job; trace rows `GpuAdmit`, `GpuPiece`, `StreamStop`; work counter `PresentCheck` — t2256
- [ ] 2.2 `kt_gpu::arbiter`: rings (D1), admission (D2), bound (D3), enable switch for the bench — t2250, t2251, t2252
- [ ] 2.3 Engine: register every device; per-label measured pass time; pieces in `execute`/`submit` on a displayed ring (D4) — t2253
- [ ] 2.4 Compute host: admission by class, pieces between dispatches; a constructor on a given device for the bench — t2255, t1239 unchanged
- [ ] 2.5 `wait_presented` without polling (D8) — t2260
- [ ] 2.6 Tags: compose thread now, prefetch near, export offline, media worker per request; the editor's display registered — t2254 bench green, `cargo test -p kt-gpu -p kt-engine`; commit "gpu-arbiter 1"

## 3. Stop check (plugin API)

- [ ] 3.1 `StreamInput::stop`; Datamosh asks it before each history position — t2257
- [ ] 3.2 Host stop scopes: compose thread, prefetch advance, reel/poster pre-roll — t2258, t2259; t1239, t2040–t2045 green; commit "gpu-arbiter 2"

## 4. Export pause and decode settings

- [ ] 4.1 Export pause on the gate, pipeline gate without timeout — t2261
- [ ] 4.2 `kt_ffmpeg::with_decode_settings`, render window — t2262; commit "gpu-arbiter 3"

## 5. Query, measurement, checks

- [ ] 5.1 `kt-cli trace gpu` — t2263
- [ ] 5.2 After numbers: `kt-cli gpu-share` both modes, `kt-cli preview` on the mosh showcase (submits/s by class); design.md D-measure
- [ ] 5.3 `cargo test` of touched crates (workspace for core crates), clippy, live `idle`, `sched_pause`, `heavy_loop`, `ui_wait`; CPU at rest on the heavy showcase
- [ ] 5.4 docs: kt-gpu CLAUDE.md, kt-plugin-api/kt-engine notes, found-defects (rows 94, 134, 147, 149, 271, 272 moved; new 290–309), What's new line, t1671 list; target-prune; commit "gpu-arbiter 4"

## 6. Archive

- [ ] 6.1 `/opsx:archive`, specs merged, design records into `openspec/specs/{video-plan,plugins,export,telemetry}/design.md`, audit table row 5 done; commit "gpu-arbiter 5". (`finish.sh` is the orchestrator's, per stage.)
