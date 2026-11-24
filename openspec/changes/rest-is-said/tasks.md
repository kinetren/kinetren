# Tasks: rest-is-said

> IMPLEMENTATION-READY (spec-engine, 2026-11-24, orch `286c406c`). Tests t4117–t4124. Areas: TESTS, PLAYER (the prefetch
> loop's near part), `audio_out.rs`, `render_cache.rs` (park points). Section 1 and 3 may run beside
> `durable-moment-key` and `fx-graph-dag` v2d; 2 and 4 after `durable-moment-key` merges. Commit per section.

## 0. Notes first

- [x] 0.1 the rows read, proposal, design (D1–D8, class walk D7), deltas `distribution`, `transport`
- [ ] 0.2 the proof of t2140: `pause_writer` held 3 s around today's `flush`, red every run (the output into the report)

## 1. A wait says it gave up; a red keeps its message

- [ ] 1.1 `trace::flush -> bool` `#[must_use]`, `trace::testing::flush`; the seven test callers; t4119
- [ ] 1.2 nextest `junit`, `finish.sh` (the copy, the last assertion of each red), `scripts/red.sh`; `testing::context()`;
      the rule's line in `docs/orchestrator.md` (the orchestrator's text); t4122

## 2. The statement of rest

- [ ] 2.1 `kt-time/src/rest.rs` (D1): `Actor`, `EPOCH`, `reached`, `still`, `busy`; t4117, t4118
- [ ] 2.2 the actors, one commit a crate: `sched.rs`; kt-engine `player.rs` (four threads), `worker.rs`, `compose_queue.rs`,
      `render_cache.rs`; kt-plugin-api's compute walk; kt-gpu `compiled.rs`; kt-app `audio_out.rs`
- [ ] 2.3 `audio_out.rs`'s tests (t2380, t2643, t2644 and the other users of `parked`) from `Rest`; the writer's `/proc` reading of t2142 and
      t1964 replaced; the kt-app dump's `rest`, `idle.wait_quiet`, `gpu_record`, `render_cache`; t4120
- [ ] 2.4 the stretch (D4), `make test-stretch`; every red it shows cured or rowed; the list in the report

## 3. The bench

- [ ] 3.1 `BenchOptions.end`, `BenchReport.gave_up`; t2044 by `Shown(24)`; the two other `loop_for` users read; t4121
- [ ] 3.2 t2414 and t2459 behind the source's gate (row 772)

## 4. Row 960

- [ ] 4.1 `on_next_insert` keyed (t1836 widened); the near part's stall (D6); t4123; t3822's scene as a person has it: t4124

## 5. Close

- [ ] 5.1 rows 958 (`idle`), 960, 961, 966, 967 closed, 772 closed or re-read; `distribution/design.md` gains the four shapes;
      `scripts/live` README line on `rest`; target-prune; the report's "Найдено, не исправлено"
