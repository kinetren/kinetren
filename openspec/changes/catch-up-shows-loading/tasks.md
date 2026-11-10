# Tasks: catch-up-shows-loading

## 1. No held frame in the engine (K1, K5, D1, D2)

- [ ] 1.1 `video_stream::run`: a frame short of more than `compose_steps` is `(None, 0)` playing or paused; record the shortage in the root renderer's `stream_short` (max over the plan); `StreamPacing::hold`, `set_stream_hold`, `stream_held` and the player's set go
- [ ] 1.2 `GpuStages.stream_short`, `Composed.catching_up`; the player and compose thread carry it; nothing kept or noted for a short frame
- [ ] 1.3 `StreamKernel::held` leaves the contract (and fx-video-codec's impl); `mosh::held` stays for t2270
- [ ] 1.4 Tests t2410, t2411, t2412, t2413, t2414 (decode_storm.rs); t2361 replaced; t2042/t2043/t2360 adjusted for the pacing struct
- [ ] 1.5 Commit, target-prune

## 2. Stand-ins found by the class walk (D5, D6)

- [ ] 2.1 `layer_src`: a frame pair lacking a side gives no frame (incomplete); t2418 (video_warp.rs)
- [ ] 2.2 `compose_offline_frame` keeps only complete composites; reel strip and poster open a missing file as the placeholder; t2419
- [ ] 2.3 Commit, target-prune

## 3. Loading names the catch-up (K2, D3, D7)

- [ ] 3.1 App: `preview_catching_up`, the scrim text `perf.loading_catching_up` ("Loading — catching up: {0}"), probe `preview.catching_up`; corner marker, `vheld`, `video_frames_held`, `perf.catching_up`, `perf.v_held` go; five catalogs
- [ ] 3.2 V-fps counts complete new pictures; t2358 rewritten; t2415 replaces t2359; t100/t125/i18n tests
- [ ] 3.3 Trace: `loading.b` positions; `kt-cli trace loading` names the catch-up; held constants kept for older traces; t2417
- [ ] 3.4 Commit, target-prune

## 4. Live

- [ ] 4.1 `datamosh.py`: play into the moshed cut → Loading with the catch-up, pause on an uncomputed moment → same; screenshots per step (t2416)
- [ ] 4.2 Live `idle` (rest costs nothing) and `heavy_loop`
- [ ] 4.3 Commit, target-prune

## 5. Measure and close

- [ ] 5.1 Release: `kt-cli showcase --mosh` and the relinked copy of his project — `kt-cli preview` V-fps / incomplete before and after over the moshed region; `pause-scan`
- [ ] 5.2 Workspace tests of touched crates (nextest, bounded), clippy
- [ ] 5.3 found-defects 403 closed, rows 460+ for what is left; What's new line; crate CLAUDE.md notes; registry K1/K2/K5 moved
- [ ] 5.4 Archive (specs merged, design into capability design.md); commit, target-prune
