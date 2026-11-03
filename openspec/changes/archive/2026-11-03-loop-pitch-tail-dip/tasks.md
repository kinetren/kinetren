# Tasks

Test numbers used by this change: t1440, t1441 (and t1442 for the live scenario); t1431–t1434 are
camera-isp-edges' reserve.

## 1. The fix

- [x] 1.1 Reproduce (a probe on a sine, echo → pitch, loop over the event start, both choices): the deepest start window of every lap after the first — "rings on" 0.000, "cut" 0.42–0.49; with a feedback echo still ringing at the wrap 0.01–0.37
- [x] 1.2 `ClipProcessor::lap_entry` / `past_out_point` (the `reentry` rule shared with `fresh_entry`); `read` seeks on a lap entry
- [x] 1.3 `FxStateStore::start_lap` / `laps_mut` / `retire_laps`, `MAX_LAPS`, the spare; `reset_all` and `retain_clips` drop the laps; `render_fx_clip` sums the earlier laps at the tail zone's weight
- [x] 1.4 t1440, t1441 red on the old behaviour (`lap_entry` off), green with it; t1105–t1114, t1330–t1339, t322, t973c, loop_fx, GOLDEN green

## 2. Live and docs

- [x] 2.1 Live scenario: loop + play over an event with an echo before a pitch on our fixture — the cursor wraps, playback keeps going without starvation (the harness hears nothing: the sound is proven by t1440/t1441) — green alone on `KT_LIVE_DISPLAY=:241`, four steps
- [x] 2.2 `docs/guide/whats-new.md` line; kt-fx and kt-engine CLAUDE.md rules; the K15 row of `docs/ktprompts/kkt-26.11.01-index.md`
- [x] 2.3 `cargo clippy --all-targets`, `cargo test --workspace`; archive; `./scripts/finish.sh` and the version
