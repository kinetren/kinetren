# Tasks: track-stage-preroll

> IMPLEMENTATION-READY (2026-11-22). The owner answered Q1 a (kkt-26.11.20 A15: always the full tail). Tests:
> t3361–t3366, t4103–t4104. Code references checked against orch `873934f7`. Read first: `design.md` D1, D1a,
> D2, D4; `src/libs/kt-engine/CLAUDE.md`; `openspec show audio-render --type spec`. Build-queue area: AUDIO
> (`audio.rs`, `fxstore.rs`, `mix.rs`) plus the start hold in `player.rs`. Commit per section; target-prune.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D6, D1a, class walk D5), delta `audio-render`
- [x] 0.2 the owner's answer Q1 a; found-defects 420 points here

## 1. The pre-roll

- [ ] 1.1 `FxStateStore.stages_warm` (`fxstore.rs`): false by default, cleared by `reset_all` (`:517–544`), set
      by `NestStream::rewind` (`:229`), by `MixPlay::warm` (`mix.rs:709, 717`) and by the pause meter
      (`player.rs:841`)
- [ ] 1.2 `render_stages` (`audio.rs:1405–1482`): on entry with the flag false, per stage whose lane rings in
      `[range.start − P_st, range.start)`: `sum_lane` (`:1487`) in 1024-frame pool chunks through the stage's
      processor into a discarded scratch; for a keyed stage, `P_st` also covers the key chain's tail,
      look-ahead and slot delay, and the key lane runs through the branch copy (`:1460–1462`) into
      `process_keyed` (`:1470`); then set the flag
- [ ] 1.3 remove `cold_after_seek` (`audio.rs:1047–1055`); `MixPlay::fx_reset` (`mix.rs:751–754`) sets
      `keep_from = head`; update the kt-engine `CLAUDE.md` lines on `cold_after_seek` ("a stage has no pre-roll
      — t2371, t2373") and the `mix::UNSETTLED` comment in `kt-time/src/trace.rs`
- [ ] 1.4 the start hold (`player.rs:2793–2822`) adds `(head − P_max)..head` to its windows when moving forward
- [ ] 1.5 `play::STAGE_PREROLL` in `kt-time/src/trace.rs` (beside `play::SEEK` `:481`; b = frames, c = µs);
      `kt-cli trace summary` sums it

## 2. Tests

- [ ] 2.1 t3361, t3362, t3364, t4103 (`tests/track_mix.rs`): byte-equal tests use a delay without feedback
      (t2376's way), a reverb case within tolerance
- [ ] 2.2 t3363 (`tests/golden_export.rs`): a range export equals its part of the whole export; GOLDEN_AUDIO
      (`:309`) unchanged
- [ ] 2.3 t3365 (`tests/mix_settle.rs`): rewrite t2373 to the warm start (its local `seek` helper at `:135`)
- [ ] 2.4 t3366 (`tests/loop_fx.rs`): a sign flip pre-rolls, laps count none after the first (t166/t167 green)
- [ ] 2.5 t4104 (`kt-cli/src/trace_query/tests.rs`)
- [ ] 2.6 green: t7, t166, t167, t732, t733, t734, t911, t2370–t2372, t2375, t2376, GOLDEN_AUDIO;
      `cargo test -p kt-engine -p kt-cli`; clippy

## 3. Close

- [ ] 3.1 `kt-cli probe … --time` on a release build over a seek into a 4 s track reverb: the pre-roll's cost
      in design D2
- [ ] 3.2 the idle check on the heavy showcase (pause, the mouse over a static place, an effects window open):
      the pause meter does not pre-roll
- [ ] 3.3 What's new line ("a seek into a track's reverb or delay now rings as in continuous play"); commit;
      target-prune; the report's "Найдено, не исправлено"
