# Tasks: scheduler-followups

## 1. The player builds its first version off its caller (D1)

- [x] 1.1 `Player::new_deferred` (kt-engine `player.rs`): publish an empty project's build, hand the real project to `engine-build` as version 1; `Player::new` unchanged
- [x] 1.2 t2331 (`src/libs/kt-engine/tests/player_open.rs`): a marked thread counts no build, Play before the build holds time, the first sound is the project's
- [x] 1.3 `cargo test -p kt-engine` (transport/audio-render TEST lines), clippy; commit "scheduler-followups 1"

## 2. Project open off the UI thread (D2, D3, D4)

- [x] 2.1 `build_player` → `new_deferred` with no sources; the load job on the `media` lane (decode threads, sources, lengths, missing) landing as `MediaThen::Load`
- [x] 2.2 `open_job` / `Opening` / `take_opening`: the file read and the lock on the lane; every UI open path posts it (Open, recent/start tile, drop, insert-as-sequence, import, live rig, lessons); "Opening …" hint
- [x] 2.3 `replace_with` releases the old player and sound output on an `app-drop` thread
- [x] 2.4 t2330 (open and replace block nothing), t2332 (edit/Undo/path change/Save during the load) in `ui_wait_tests.rs`
- [x] 2.5 the kt-app suite (reach_tests, layout_tests, ui_wait_tests, durability TEST lines), clippy; commit "scheduler-followups 2"

## 3. Ctrl+S under a busy disk (252)

- [x] 3.1 t2333: a background writer with fsyncs in the project's directory, Ctrl+S with zero grace — no blocking call, the asterisk clears in a later frame; fix what remains if red
- [x] 3.2 commit "scheduler-followups 3"

## 4. The side engine and the bounded offline ring (D6, row 290)

- [x] 4.1 `GpuFxEngine::sibling`, `kt_gpu::side_engine()`, `park_offline_engine` parks both; fx_thumbs, reel_preview, posters use it
- [x] 4.2 `Ring` counts preview servers in `bounded`/`cut_before`; `GpuFxEngine::serve_preview`; kt-app's slow-route setter calls it
- [x] 4.3 t2337, t2338 (`src/libs/kt-gpu/tests/arbiter.rs`); the wait measured (trace `FrameWait` lock / `GpuAdmit`) before/after
- [x] 4.4 `cargo test -p kt-gpu` (video-plan TEST lines t2250–t2256), clippy; commit "scheduler-followups 4"

## 5. The loop fill (D5, row 251)

- [x] 5.1 `loop_fill.rs`: the `near` service, the walk ahead of the playhead, warm check, compose via the fill executor, keep, skip set; `Player::set_fill_gpu`; the prefetch thread kicks it; `KT_LOOP_FILL=off`
- [x] 5.2 trace: `job::LOOP_FILL`; `kt-cli trace loop` names fill composites apart
- [x] 5.3 kt-app and `kt-cli preview` inject `side_engine()` as the fill executor
- [x] 5.4 t2334, t2335, t2336 (`src/libs/kt-engine/tests/loop_fill.rs`)
- [x] 5.5 release build, llvmpipe: `kt-cli preview --loop` + `kt-cli trace loop` with the fill and with `KT_LOOP_FILL=off`; numbers into design.md
- [x] 5.6 `cargo test -p kt-engine`, clippy; commit "scheduler-followups 5"

## 6. Proof and records

- [ ] 6.1 live scenario `open_heavy` (`src/scripts/live/open_heavy.py`) with screenshots per step; live `idle`, `heavy_loop`, `sched_pause` green
- [x] 6.2 design.md: the class walk's results and the measurements; found-defects: rows 251, 270, 290 removed, 370+ for what is left; whats-new line; crate CLAUDE.md notes (kt-app open path, kt-engine first build, kt-gpu side engine)
- [ ] 6.3 `src/scripts/specs-user-path.py --check`, `cargo test --workspace` of touched crates, clippy; commit "scheduler-followups 6"; target-prune
- [ ] 6.4 archive (`/opsx:archive`), commit; `finish.sh` is the orchestrator's per stage
