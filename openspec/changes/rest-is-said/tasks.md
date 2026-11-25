# Tasks: rest-is-said

> IMPLEMENTATION-READY (spec-engine, revision 3, 2026-11-25, orch `fc60a5d9`: row 839 added (D9); what `durable-moment-key` fixed is cut). Tests
> t4117–t4124, t4144. Areas: TESTS, PLAYER (the prefetch loop's near part), `audio_out.rs`, `render_cache.rs`, `worker.rs` (one question). **Every section may start now** (`persistent-cache` is in); 2.2 and 4 beside
> `shared-events` are text merges (`player.rs`, `worker.rs` signatures). Commit per section.

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
- [ ] 2.3 `audio_out.rs`'s `settled` and `parked` read `Rest` (their tests unchanged); the writer's `/proc` reading of t2142 and
      t1964 replaced; the kt-app dump's `rest`, `idle.wait_quiet`, `gpu_record`, `render_cache`; t4120
- [ ] 2.4 the stretch (D4), `make test-stretch`; every red it shows cured or rowed; the list in the report

## 3. The bench

- [x] 3.0 t2044 plays on until a frame was shown (`first_shown_by`): built in `durable-moment-key` (966)
- [ ] 3.1 `BenchReport.gave_up` (asserted `None` by every test that reads a report), `BenchOptions.stop_when`; the two other users of `loop_for` end by what
      they played (D3); t4121
- [ ] 3.2 t2414 and t2459 behind the source's gate (row 772)

## 4. Rows 960 and 839

- [ ] 4.0 the proof of 839 (D9): a gated source, today's code, the records and the moment never written (the output into the report)

- [ ] 4.1 `on_next_insert` keyed (t1836 widened); the near part's stall (D6); t4123; t3822's scene as a person has it: t4124
- [ ] 4.2 `MediaWorker::outstanding`, `one()` without the clock, `note_input`'s notify while busy (D9); t4144; t2982–t2989, t3447, t3833 green; the idle check on the heavy showcase while it prepares and after

## 5. Close

- [ ] 5.1 rows 958 (`idle`), 839, 960, 961 closed (966, 967, 969 are), 772 closed or re-read; `distribution/design.md` gains the four shapes;
      `scripts/live` README line on `rest`; target-prune; the report's "Найдено, не исправлено"
