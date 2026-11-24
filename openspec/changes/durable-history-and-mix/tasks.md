# Tasks: durable-history-and-mix

> IMPLEMENTATION-READY (2026-11-22; revised 2026-11-24 for the owner's D6, kkt-26.11.22 line 9: no history on
> disk, the mix tier only). Starts after `project-cache-folder` sections 1 and 3 (the folder, the header,
> `ask_room`, the key without the build). No longer waits for `stream-picture-stays` (the STREAM half is gone).
> Tests: t3336, t3337, t3346–t3352 (t3338–t3345 and t4105 freed). Code references checked against orch
> `873934f7`, the mix half at `af14db35` (design D10). Read first: design D1, D2, D5, D10;
> `src/libs/kt-engine/CLAUDE.md`; `openspec show audio-render --type spec`. Build-queue areas: CACHE (`mix/`),
> AUDIO (`mix.rs`), PLAYER. Commit per section; target-prune after each.

## 0. Notes first

- [x] 0.1 proposal, design (D1, D2, D5–D8, D10, Withdrawn), deltas `audio-render`, `telemetry`
- [x] 0.2 the owner's answers: A14 (build now, the cache folder) for the mix; D6 (final frames only) withdrew the
      history tier; rows 770 (closed by D6) and 771 point here

## 1. The restart line (telemetry)

- [ ] 1.1 a restart recorder over the first minute after open: history positions walked (`walked()`
      `stream.rs:124`, `positions_walked()` `:141`) with device time, covered bursts rendered again, bursts
      restored; `kt-cli trace summary` line (`trace_query.rs:466`); t3336
- [ ] 1.2 record kinds for mix runs in `kt_time::trace::render_cache` (`trace.rs:631`), with their bytes;
      `kt-cli trace cache` (`trace_query.rs:130`) prints them; t3337

## 2. The mix tier

- [ ] 2.1 `Unit.data` (`mix.rs:56`) becomes `Arc<[f32]>`; `MixCoverage` (`:499`) carries a snapshot of the
      unwritten units; the player's thread writes runs (raw f32, a manifest per run) to `mix/v<FORMAT>/` at
      rest (the prerender's quiet rule, `render_cache.rs` `quiet_left` `:925`), through `project-cache-folder`'s
      header and `ask_room`, salted by the store's salt and the audio file identities of the widened range
      (`audio.rs:1012`)
- [ ] 2.2 the loader on `prefetch_loop` (`player.rs:3400`, beside `mix.budget.ask` `:3522`) sends restored
      runs over a second `rtrb` ring; `audio_render_loop` (`:2442`) drains it at the top of a pass into
      `MixStore::write` (`mix.rs:303`) as first recordings; the render thread never opens a file;
      t3346–t3350
- [ ] 2.3 the bars' scan at rest adds restorable bursts; the export never reads the tier; t3351, t3352
- [ ] 2.4 `cargo test -p kt-engine -p kt-cli`; clippy; the idle check on the heavy showcase (after the writes,
      nothing at rest); kt-engine `CLAUDE.md` ("the render thread never reads a file; a new kept result names its
      durable salt"); What's new line ("a restart no longer renders the covered sound again"); found-defects 770
      and 771 moved; commit; the report's "Найдено, не исправлено"
