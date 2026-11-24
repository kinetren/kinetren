# Tasks: durable-moment-key

> IMPLEMENTATION-READY (spec-engine, 2026-11-24, orch `887c2972`). Read first: design D1–D6; `kt-cache`, `kt-engine`,
> `kt-gpu` `CLAUDE.md`. Tests t4108–t4116. Areas: CACHE, PLAYER, STREAM (`video_stream.rs` `ident`), GPU (`plan_key`
> only). Best taken by the executor of `project-cache-folder`, before its section 3: both write `durable_hash`.
> Commit per section; target-prune after each.

## 0. Notes first

- [x] 0.1 proposal, design, class walk, deltas `video-plan`, `media-index`
- [ ] 0.2 the "before" runs of the measurement on a release build, saved beside this file's report
- [ ] 0.3 t4108 written first and seen RED for the design's reason (the probe key of an evicted moment)

## 1. The identity (kt-cache, kt-media, the worker)

- [ ] 1.1 kt-cache: `SourceIdent`, `adopt`, `forget`, `get_video_leaf`, the identity on worker inserts and its refusal;
      `invalidate_source` keeps the identity; t4110
- [ ] 1.2 kt-media: `MediaSource::identity` and its four answers; the stat once at open, and compared when
      `cursor_for` opens another decoder by the path (D1, row 1065); the generators' after a retune; `src_rev`'s text in one function both use; t4111
- [ ] 1.3 kt-engine `worker.rs`: `adopt` in `spawn_with_stats` and after an applied retune, the identity passed to every
      insert (`:669`, `:677`, the audio ones); `player.rs:2245` and `:2286`; t211, t496, t496a, t497, t2152 green as they are
- [ ] 1.4 `cargo test -p kt-cache -p kt-media -p kt-engine`; clippy; commit

## 2. One key

- [ ] 2.1 `video_plan.rs`: `LayerSrc.leaf`, `LayerBlend.leaf`, `FramePlan.durable`; the literals of tests and
      `chain_plan_with` take `leaf: 0`; `layer_src` (`video.rs:2569`, `:2592`) by `get_video_leaf`
- [ ] 2.2 `render_cache.rs`: `moment_key`, `disk_key(plan) -> Option`; `Leaves`, `with_leaves`, `note_leaf`,
      `probe_durable`, `FileIds`, `files`, `restored_key` deleted; `restore_from_disk`, `kept_picture`, `scan`, `one` follow;
      t2981 rewritten (the identities adopted in the caches, the scenario's outcomes kept); t4115
- [ ] 2.3 kt-gpu `engine.rs:4336`: `plan_hash` and its helpers deleted, `plan_key` over `moment_key`; the two
      "hash it in both" rules of the CLAUDE.md files deleted; t2294, t1457, t1574, t1607, t250 green as they are
- [ ] 2.4 t4108 green; t4109 (the retune race)
- [ ] 2.5 `cargo test -p kt-engine -p kt-gpu`; clippy; commit

## 3. The stream stage

- [ ] 3.1 the snapshot in `ClipStream::new`, `ident`, the reads that refuse, `key_only` with the cache; t4116
- [ ] 3.2 t2145, t2150, t2151, t2270, t2978 green as they are; commit

## 4. The window

- [ ] 4.1 `player.rs:3575–3632`: the held set for the size is the window's; `size_frames_cache`'s recount rule;
      t4114; t3605 green as it is
- [ ] 4.2 a moment waiting for its recheck is held for 200 ms, the prefetch nudges `compose_queue`'s recheck, the
      order from the playhead outward (`player.rs:1942`); t4113
- [ ] 4.3 t4112 (the loop end to end, row 965's as a test)
- [ ] 4.4 the measurement's table in the report and in `video-plan/design.md`; the audit's cut 1 text and row 965's
      cause corrected; rows 1061–1063 closed
- [ ] 4.5 What's new ("a loop that plays from its finished frames no longer keeps its sources in memory"); the crate
      notes; commit; the report's "Найдено, не исправлено"
