# Tasks: durable-history-and-mix

> IMPLEMENTATION-READY (2026-11-22; revised 2026-11-24 for the owner's D6, re-read 2026-11-25 at orch `fc60a5d9`: D5, D10). `project-cache-folder` and
> `persistent-cache` are in. After `track-stage-preroll` (`mix.rs`). Tests: t3336, t3337, t3346–t3352, t4145. Read first: design D1, D2, D5, D10; `src/libs/kt-engine/CLAUDE.md`;
> `openspec show audio-render --type spec`. Build-queue areas: CACHE (`mix/`), AUDIO (`mix.rs`), PLAYER. Commit per section; target-prune after each.

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

- [ ] 2.0 `MediaSource::sound_identity`, `MediaParts` keeps it at a raise and through `swap_picture`; `audio-path.txt`, `AUDIO_PATH_VERSION` (a guard like t3808's); t4145
- [ ] 2.1 `Unit.data` becomes `Arc<[f32]>`; `MixCoverage` carries the snapshot of whole unwritten units; the `mix-disk` lane; the step of `run_prerender` (D5) writes runs (raw f32, a
      manifest per run) to `mix/v<FORMAT>/` through the folder's header and `ask_room`
- [ ] 2.2 the prefetch thread asks the lane; the ring; the drain in `audio_render_loop` into `MixStore::write` as first recordings; the render thread never opens a file; t3346–t3350, t2684 green
- [ ] 2.3 the bars' scan at rest adds restorable bursts; the export never reads the tier; t3351, t3352
- [ ] 2.4 `cargo test -p kt-engine -p kt-cli`; clippy; the idle check on the heavy showcase (after the writes,
      nothing at rest); kt-engine `CLAUDE.md` ("the render thread never reads a file; a new kept result names its
      durable salt"); What's new line ("a restart no longer renders the covered sound again"); found-defects 770
      and 771 moved; commit; the report's "Найдено, не исправлено"
