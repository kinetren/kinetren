# Tasks: kept-mix-settle

## 1. Diagnose
- [x] 1.1 His trace at +240.149: replays, the reset chunk, the kept second lap (design Context)
- [x] 1.2 The same sequence on the relinked copy through `MixPlay`: 742/15606 → 15606/15606

## 2. The kept mix settles (row 390)
- [x] 2.1 `AudioRenderer::settle_frames`, `cold_after_seek`
- [x] 2.2 `MixPlay`: continued / window warm-up, `keep_from`, `fx_reset(renderer, head)`; trace aux `warm`, `unsettled`; `kt-cli trace loop` names them
- [x] 2.3 t1932 to the warmed reference; t2370–t2373 (store), t2374 (the player, 230 ms loop at the device) — each red on the old exit
- [x] 2.4 Class walk (design D4): key branches reset (t2375), a reel's move pre-rolls its stages (t2376)

## 3. The output device opens on Play (row 274)
- [ ] 3.1 `Player::wake()` public; the output thread parks on it
- [ ] 3.2 `audio_out`: the output thread, `ReturnOnDrop`, open on play, close after rest, flush while closed
- [ ] 3.3 Trace `device_open`/`device_close`/`device_error`; the error to the bell
- [ ] 3.4 Tests t2380–t2384 with a device double (open on play, close at rest and park, the ring survives, a flush while closed, an open error retried)
- [ ] 3.5 Live `idle`: the audio threads judged (no wake on a pause); `sched_pause`, `loop`, `space_hold` green
- [ ] 3.6 Measure open → first callback here (trace of the live run)

## 4. Contracts
- [ ] 4.1 kt-engine and kt-app CLAUDE.md; root CLAUDE.md invariant 2/6 lines
- [ ] 4.2 found-defects: 390 and 274 leave; rows 420–439 for what is left
- [ ] 4.3 What's new lines; registry rows J1, J6
- [ ] 4.4 Tests of the specs, `cargo test` of what changed, clippy, target-prune
