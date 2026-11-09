# Tasks: one-scheduler

## 1. The executor

- [x] 1.1 `kt_time::sched`: classes, lanes (one-shot and pump), pool sized from cores, pause votes, tokens, counters, trace records (t2230–t2234)
- [x] 1.2 `kt_time::block`: blocking calls on marked threads, counted and traced (t2235)
- [x] 1.3 `kt-cli trace during` names jobs by class and lane, pauses, blocking calls

## 2. The services onto it

- [x] 2.1 Thumbnails (pump, sources dropped at the end of a run, no 2 s timer), row 174
- [x] 2.2 Effect cells (pump, no 1 s / 25 ms timers), row 114 (caps go round)
- [x] 2.3 Posters, the poster loader, reel previews, analysis (`BgJob` on a lane, `BgCtl` waits on the gate)
- [x] 2.4 The index service (pump, no 200 ms), the media watch (event-driven, no 900 ms)
- [x] 2.5 The player's pause vote; the export's vote

## 3. Timers at rest

- [x] 3.1 Audio render and prefetch park with no timeout on a pause
- [x] 3.2 The load watch parks after a quiet stretch (row 122)
- [x] 3.3 Telemetry's heartbeat sleeps on a quiet session
- [x] 3.4 Row 140 checked after play→pause on the live stand (`sched_pause`): a settled pause draws no frame

## 4. The UI thread's waits

- [x] 4.1 Ctrl+S and autosave on the save lane (row 98)
- [x] 4.2 Media opens on gestures and the decoder join as jobs (row 148)
- [x] 4.3 The blocking-call guard over the gestures (t2236)

## 5. Proof and archive

- [ ] 5.1 Live scenario `sched_pause`: play while thumbnails and analysis are pending; PATH line
- [ ] 5.2 Trace of a pause and CPU at rest on the heavy showcase, before/after
- [x] 5.3 The class walk in `design.md`; specs; What's new; found-defects rows; archive
