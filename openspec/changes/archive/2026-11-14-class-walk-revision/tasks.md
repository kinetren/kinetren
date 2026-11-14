# Tasks: class-walk-revision

## 1. The walks

- [x] 1.1 Walk the eleven archived changes' callers, consumers and siblings in the code (four read-only walks), re-read every miss
- [x] 1.2 The table in `design.md`; a pointer section in each archived change's `design.md`

## 2. Fixes (t2450–t2459) and row 463

- [x] 2.1 Run prewarm by the frame's pieces (t2450)
- [x] 2.2 The stream view per renderer (t2451)
- [x] 2.3 A landed save compares the view state (t2452); the close pauses the player first (t2453)
- [x] 2.4 `reel_envelope` waits while held (t2454); the strip raises generator sources (t2455)
- [x] 2.5 Pen and search count every press; the preview click window (t2456, t2457)
- [x] 2.6 `kt_time::mem::room()` for the stream memo and the frames loop count (t2458)
- [x] 2.7 The bench / `kt-cli preview` / `pause-scan` name a catch-up (t2459); `stream-cost` sizing, `pause-scan` decoder threads, `EXPORT_STREAM_MEMO_BYTES`
- [x] 2.8 Row 463: t2230 frees one worker (40/40 stress)
- [x] 2.9 Every new test red on the old code (each fix reverted in turn)

## 3. Records

- [x] 3.1 found-defects rows 510–524; row 463 out
- [x] 3.2 `docs/engine-audit.md` "Class-walk revision 2026-11"; `docs/orchestrator.md` brief rule (the CALLERS walk)
- [x] 3.3 nextest of kt-time, kt-engine, kt-gpu, kt-cli, kt-app; clippy; live `idle` and `masks`
- [x] 3.4 Commit, target-prune; archive
