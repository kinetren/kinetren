# Tasks: test-hygiene

## 1. The bench keeps the evidence

- [x] 1.1 `worker.py`: the scenario folder emptied before a try; on `KT_LIVE_TRY=2` the red try moves
  to `<scenario>/first-try/`; `said.txt` per try; the stand drops `geom.json.*` on start
- [x] 1.2 `live-check.sh`: the retry passes `KT_LIVE_TRY=2`, the report names `first-try`
- [x] 1.3 `_flaky`/`_red` stand-ins; t2210 (red on the former worker)

## 2. Input read by a slow frame (product)

- [x] 2.1 `Controls::pressed` → `Pressed` in order; `handle_keys` runs every press; Esc/Enter per press
- [x] 2.2 Space: edges in order, the hold proven by a frame start, a release of an unproven press a tap,
  the double window widened by `KeyClock::busy`
- [x] 2.3 egui's click windows widened by the busy frame (`KeyClock::widen_clicks`)
- [x] 2.4 t2211, t2212 (both red on the former code), t2213, t2214
- [x] 2.5 the dump's `keys_gated`; `save_now`'s red names it (row 47's next step)

## 3. Clocks in the workshop

- [x] 3.1 t1913/t1914 interleave their rounds (`frame_costs`)
- [x] 3.2 t1671 scans test code for `.elapsed()` bounds and `thread_ns()`; its list names the
  telemetry/update-check timeouts and the frame-cost rounds
- [x] 3.3 the long-frame note's run-queue wait (`frame_cost::run_delay_ns`), t2215; `ui_wait` judges the rest

## 4. The start's pass (product, row 158)

- [x] 4.1 reproduce t1935 under load with its structured trace (`KT_TEST_TRACE`); the bench prints its
  device thread's lateness
- [x] 4.2 the render thread commits a start's pass whole; the seam `hold_start_bursts_for_tests`; t2216
  (red on the former push)

## 5. Live scenarios read consequences

- [x] 5.1 `forms`, `stills`, `loop`, `readme_shots`, `unsaved`, `space_hold`, `fxpreset`
- [x] 5.2 `heavy_loop`, `cached_range`: the trace's reasons; `idle.wait_quiet` asks the threads too

## 6. The bench's channel and gestures (from the proof runs)

- [x] 6.1 the dump and `ctl` on tmpfs (`_channel.py`); the stand's journal and trace kept with a red
- [x] 6.2 `poll_step` waits for a frame the program draws by itself; `focus()` asks the X server; a
  closing program and a stopped stand are gone when their window is; `_wait_ctl` waits while the
  stand draws; a click is dated when read; `_probe_rect` takes a rectangle that held two dumps
- [x] 6.3 scenarios red in the proof runs read consequences (idle, decode_storm, nestparam, stills,
  generator, morphgrid, morph, gpu_record, lessons, track)

## 7. The recheck of kept moments (product, rows 133, 170)

- [x] 7.1 no frame polls `final_pending`; t2217 (red on the former code)

## 8. Proof and records

- [x] 8.1 six proof runs of three full live sets beside `cargo test --workspace`: the workshop green in
  all; first-try reds 12–14 per proof at the start (and in the baseline), 4–5 in the last three; the
  listed rows' scenarios green on the first try except `formula` (found-defects 254) and one
  `readme_shots` key (found-defects 255)
- [x] 8.2 `docs/found-defects.md`: closed rows leave, rows 250–255 added; `docs/engine-audit.md` row 12
- [x] 8.3 `cargo test` (the proof runs' workspace runs); clippy; target-prune; archive
