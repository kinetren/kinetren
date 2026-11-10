# Tasks: local-cost-queue

## 1. Audio local costs (rows 210, 211, 311, 312, 313, 314)

- [x] 1.1 Before-digests of the curved/switched forms printed on the base tree; t2315 written red-free on the base
- [x] 1.2 Limiter ratio memo (210) — measured 3 % slower, reverted; row closed as no cost; t1981, t2315
- [x] 1.3 Camera mic: µ-law constant + level table (211, t2316), suppressor window table and kept buffers, rows by index (312); t1981, t2315
- [x] 1.4 VHS sound: FIR cosine table, dropout cache by law and stepped (311; the Deck copy stays, measured cheap); t1981, t2315
- [x] 1.5 Pitch WSOLA window table and candidate list (313); t1981, t2315
- [x] 1.6 Codec scratch, no clones, Datamosh/mic callers (314); t2317, t1981, t2315
- [x] 1.7 Measure before/after (`kt-cli fx-cost`, `kt-cli probe --time` on the heavy showcase, release); commit, target-prune

## 2. GPU local costs (rows 136, 137, 138)

- [x] 2.1 `kt-cli pass-cost` (a chain's passes by the pass timer, two forms interleaved, minima, same-bytes check)
- [x] 2.2 Camera ISP sensor-grid passes (136); t2310, t2311, t1217, t1430
- [x] 2.3 Radial blur spin table on the device (137) — measured more costly, rejected (design D2); t2312 not kept
- [x] 2.4 Uniform writes per piece and bind-group reuse (138); t2313, t2314, t2023, t2253
- [x] 2.5 Measure before/after; commit, target-prune

## 3. Row 118 and the class walk

- [x] 3.1 The first lap on the heavy showcase through the trace (`kt-cli preview`, `kt-cli trace`); local fixes or rows 350+
- [x] 3.2 Class walk across every kernel; list into design.md; rows 350–369 for what is left
- [x] 3.3 Live `idle` and `heavy_loop` (rest costs nothing); t1217, t2023, the touched capabilities' TEST lines, clippy

## 4. Close

- [x] 4.1 found-defects rows removed/added, engine-audit row 11 marked done with the numbers, crate CLAUDE.md notes
- [ ] 4.2 Archive (specs merged, design "why" into capability design.md); commit, target-prune (finish.sh is the orchestrator's)
