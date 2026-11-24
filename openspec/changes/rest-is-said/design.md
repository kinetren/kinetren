# Design: rest-is-said

## Context: what each row shows (read, not run)

- **961, t2140 — the cause is in the code, not in the machine.** `trace::flush` (`trace.rs:1326`) waits `FLUSH_LIMIT`, 2 s, for
  the writer and then returns the same `()` as when it succeeded. The writer creates a directory, prunes and writes files
  (`Writer::open`); t2140 and t2142 ran in two processes of one run and both took ~9.8 s, so what they shared is the disk. A
  writer that stands 2 s in one file call leaves t2140 reading a file without its records, and it asserts 300 of them. Proof
  for the executor, no load needed: `testing::pause_writer` held 3 s around the flush makes the present t2140 red every run
  (not proved here that it WAS this: the message is lost).
- **966, t2044.** 600 display moments in 10 s were all handed out (the display thread had its time) and none was complete: the
  history of ~110 positions did not walk in the 10 s the bench gave (`loop_for`). The same test was red before at 57
  frames instead of ~200 (row 762), so the margin is under 10 s alone and gone at a load of 75. It is either M2 or a walk
  that never ends; a bench bounded by seconds cannot tell them apart. With D3 it can: it comes, or the guard ends the
  test with the walk's own state.
- **293, 424, t2044's second assertion** ("every decoded frame is read before the cache drops it"): not explained by any shape.
  It stays open, with D5's evidence to read.
- **967, 950, 828 (b), 958 (`idle`), 961 (t2142)**: `closes` is counted inside the closing pass, the pump is idle by its processor
  share, a counter moves when a segment is committed, a job moves its counter when it ENDS. All M1. `durable-moment-key`'s
  branch already folds `settled` into `audio_out.rs`'s `parked` for 967: kept, and replaced by D1's answer.
- **958, `node_graph` and `datamosh`**: "not analysed" and the runs' dumps are gone. Their steps are gestures (Viewer, the FX window), so
  M3 is a guess. Nothing is designed for them; D1 and D5 make the next red readable.
- **825, 827**: cured by taking both durations in one window. The rule is in `distribution` already. 374 and 375 (device time
  of a software rasterizer) wait for the same cure and are not this change.

## D1. `kt_time::rest`: one statement of rest

`rest::Actor::new(name)` registers a thread or a service; it holds one atomic: `Working`, `Parked`, `Due(at)` (parked with a timed
wake that will do designed work, as the output thread's `PAUSE_CHECK`). A global `EPOCH` moves on every change to
`Working` and on every hand-over of work (`Wake::notify`, `Lane::post`/`kick`, a `DecodeRequest` submitted, `ring` push of the
trace). `Parked` is written by the actor as its last act before it blocks, after every flag and counter of the pass; a
hand-over that lands between the two moves `EPOCH`, so the rest is not believed.

- `rest::busy() -> Vec<Busy { name, state, since }>`; `rest::reached(deadline) -> Result<Rest, Vec<Busy>>`: no actor `Working` or
  `Due`, and `EPOCH` equal before and after the look. `Rest { epoch }`; `rest.still() -> Result<(), Vec<Woke { name, at }>>`: nothing
  moved since, or who did (each actor keeps the time and cause of its last wake).
- **A quiet window is `Rest` + a sleep + `still()`.** The sleep's length is then a claim about the program only ("nothing
  in 300 ms"), never about when the program got there. The sleep stays: a timer nobody registered is found by it.
- Actors: the scheduler (one, from `Stats`: running, waiting, pump due), `prefetch`, `engine-build`, `audio-render`, `audio-output`, each
  `MediaWorker`, the compose queue, `render-cache` and its reader, the compute host's walk, `pipeline-warm`, the trace
  writer (its own atomic, outside `EPOCH`: recording is not work), `geom-dump`, `load-watch` (excluded by name: its second is
  its design, and it records nothing at rest).
- Cost: one relaxed store and one fetch-add per park and wake, on threads that just took a lock or a syscall. The
  audio render thread parks as today; the cpal callback registers nothing.
- The kt-app dump gains `"rest": ["render-cache", …]` (empty: at rest). `idle.wait_quiet` asks it first, then keeps its
  counters and processor share as the check that nothing unregistered runs. `sound_paused`, `rc_settled`, `posters_busy`,
  `analysis` stay for the scenarios that ask one of them by name.

Rejected: *a longer quiet before every window* (moves the threshold of the same race); *reading the threads' state in
`/proc`* (t2142 does: Linux only, and a thread asleep in a disk call is `D` or `S` like a parked one); *a virtual clock for the whole
engine* (the decoder, the device and the disk do not run on it).

## D2. A wait that gives up says it

`trace::flush() -> bool`, `#[must_use]`: `false` at `FLUSH_LIMIT` (the product's callers, the exit and a crash, print it into
their own line). `trace::testing::flush()` waits to `test_deadline()` and panics naming the writer's state and what is
unwritten; the seven test callers use it. `Lane::wait_idle` and `rest::reached` are `#[must_use]`. `preview_bench`'s "a start that never
sounds" exit sets `BenchReport.gave_up = Some(why)`; a test asserts it is `None`.

## D3. A bench ends by what it showed

`BenchOptions.end: End::{ Once, Wall(Duration), Shown(u64) }` replaces `loop_for`. `Shown(n)`: complete new frames; the guard is
`test_deadline()`, and at the guard the report says `gave_up` with the walk's positions, the decoder's queue and `rest::busy()`.
`kt-cli preview --loop N` stays `Wall`: it is a measurement.
- t2044: `Shown(24)` (three laps of eight). Its claim "the loop shows frames" becomes "it comes to show them"; the bound on
  decoded pictures is a count and stays; the unread assertion stays.
- `player_dynamic.rs:667` (6 s) and `mix_player.rs:126` (8 s) are read by the executor: `Shown` where the claim is a state,
  `Wall` with a reason in t1671's list where the seconds are the claim.

## D4. The stretch: M1 found at once

`KT_REST_STRETCH_MS=n` (read once, 0 by default; any build): an actor sleeps `n` ms right before it writes `Parked` or `Due`, that is
after every flag of its pass. A test that opens its window at a flag then measures the stretched pass and is red every
run; one that asks `rest::reached` is green at any `n`. `scripts/finish.sh` does not run it; `make test-stretch` does (n = 400), and the
executor's report lists what it found. A test of a person-facing bound (t1671's list) is exempt by name.

## D5. A red keeps its evidence

- `.config/nextest.toml`: `[profile.default.junit] path = "junit.xml"` with failure output stored. nextest overwrites it every run,
  so `scripts/finish.sh` copies it to `target/reds/<date>-<commit>.xml` when the run is red and prints each failed test's last
  assertion in its summary; `scripts/red.sh` (one line) does the same copy after a hand run, and the executors'
  rule in `docs/orchestrator.md` names it: copy before the rerun.
- A test that gives up prints the load average, `rest::busy()` and the machine's available memory (one helper,
  `kt_time::testing::context()`, used by `test_deadline`'s waits in `rest`, D2 and D3).
- Live: a step that fails writes the dump's `rest` and the last 200 lines of the stand's log beside the screenshot.

## D6. Row 960: the near part waits for a frame

`prefetch_loop` (`player.rs:3469`): while `stream_pending` a paused preview passes every 100 ms, whatever arrives. The far part has
the rule (`far_walk_again`); the near part gets it in the form that suits a pause, with a wake instead of a clock:
1. A paused pass that left the near part pending, walked no position and saw no frame inserted since the pass before is
   STALLED.
2. Before it parks it arms `cache.on_next_insert` (idle-cpu) with the loop's `wake`, then reads `video_inserted` again; moved:
   it passes at once (the insert fell before the arming).
3. It parks by `wake.wait_since(seen)`, no clock. The next frame wakes it; so do a play, a seek, an edit, a quality, a
   replaced source, as today.
`on_next_insert` holds ONE waiter and the app's paused preview uses it (`app.rs:7709`): it becomes a list with a key
(`"preview"`, `"prefetch"`), each key replacing its own.
A decoder that is slow is not stalled for long: each frame it lands is a wake. A source that never answers costs the
passes until the requests are out, then none. While playing nothing changes (a pass every 100 ms).

## D7. The class walk: where else the program or a test is anchored to something else

| Place | Shape | Here |
|---|---|---|
| `trace::flush` in tests (7) | M2 | D2 |
| `preview_bench` ends (3 tests) | M2 | D3 |
| `audio_out` tests (`parked`, 5 users) | M1 | D1 |
| kt-time `sched/tests.rs` t2230 (rows 373, 650) | M1: the order of two threads' pushes is asserted; kicks race the pump | D1's stretch shows it; the cure is the scheduler's own start order (`SchedStart` records), no rest |
| kt-engine `render_cache.rs` tests (14 sleeps), `render_ahead.rs` (9), `player_dynamic.rs` (18) | M1, M3 candidates | D4 says which |
| kt-app `layout_tests.rs` (46 sleeps) | harness frames, mostly; M3 candidates | D4; `h.run_steps` where the sleep is a wait |
| `decode_storm.rs` t2414, t2459 (row 772) | a cold start EXPECTED to be incomplete: on a loaded machine the history may be whole before the first look | t2459's guard asserts the scene, so a red is the scene, not the code; the source's gate (`Gate`, t2041) holds the decoder until the first composite: the test no longer races |
| live `wait_quiet` (idle, gpu_record, render_cache) | M1 | D1 |
| live gestures (M3: 783, 817, 254) | cured one by one by publishing `double_click_s`, input marks | unchanged |
| exit and close waits (`shutdown::WAIT`) | bounded on purpose, and they record `left_behind` | unchanged: they already say it |
| product waits that give up: `first_callback` (1 s), `close` (`recv_timeout` 2 s, `audio_out.rs`) | M2 in the product: a ring not handed back becomes `None` and the next Play reports it | unchanged, reported at the next Play |
| `poster` waits, `offline-park`, the GPU arbiter's takes | read: each records its wait | unchanged |

## D8. The order

1. D2 and D5 (small, they change what the next red says). 2. D3. 3. D1's registry with the actors one by one. 4. D4's run and
its list. 5. D6 after `durable-moment-key` (the prefetch loop).

## Risks

- **An actor forgotten.** Then `reached` says rest too early, as today. The stretch does not reach it either; the quiet window's
  sleep and `idle`'s processor share do. t4117 holds the list: every `thread::Builder::new().name(…)` of the workspace's
  product code is registered or named with its reason.
- **The stretch slows every park**, so `make test-stretch` is a run of its own.
