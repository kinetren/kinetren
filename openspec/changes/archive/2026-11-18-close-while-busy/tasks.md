# Tasks

## 1. Diagnosis and proposal

- [x] 1.1 The owner's traces (`fails/26.11.18/{new,old,render_simple_region}`): no close recorded in any run, prefetch passes up to 37 s joined by the player's drop; proposal, design (class walk), spec deltas (durability, transport, telemetry); t2950–t2959 and rows 750–759 reserved in the shared notes
- [x] 1.2 Live scenario `close_busy` on the old code (Xvfb, llvmpipe): the close ended there — the owner's iGPU path is not reproduced; the scenario guards the bound and the trace's record of the close

## 2. Bounded, recorded waits

- [x] 2.1 kt-time: `shutdown` (`join`, `wait_until`, `record`, `mark`, `left_behind`), `Ev::Shutdown = 120` with its kinds, `Ctx::cancel_check`, `testing::name_of`; t2950, t2953
- [x] 2.2 kt-engine: `Player::drop` bounded joins; `MediaWorker::shutdown_until` and the stop between chunks; the producer's moment under `stop::CLOSED`; t2952
- [x] 2.3 kt-gpu: the exit hook's wait bounded (10 s) and recorded
- [x] 2.4 kt-app: `note_close_asked`, `on_exit`'s waits recorded, `exit_watch` armed after the recovery write (`_exit`), trace flushed after `on_exit` and after eframe; `AudioOutput` and `FxThumbService` drops bounded; `file.quit` probe; t2951
- [x] 2.5 kt-cli: `trace … shutdown`; t1671's list names the bounds that ARE the checks

## 3. Found on the way

- [x] 3.1 Row 750: the thumbnail pump and the media watch woke the UI under their locks (ABBA with egui's context lock, epaint's 10 s panic at the start); every `wake::` site walked

## 4. Verification

- [x] 4.1 `close_busy` (t2954) green: paused 2.6 s, playing 3.7 s, File → Quit 5.1 s, each ended by itself, the trace names the close
- [x] 4.2 nextest of kt-time, kt-engine, kt-cli; the kt-app tests named above; clippy
- [x] 4.3 Docs: crate CLAUDE.md (kt-time, kt-engine, kt-app), found-defects 750–751, What's new, the T1 row of `kkt-26.11.18-index.md`
