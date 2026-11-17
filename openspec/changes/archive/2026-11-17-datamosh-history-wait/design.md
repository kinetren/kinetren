# Design: datamosh-history-wait

## D0. What his trace says (`fails/26.11.16/problem_region_26_11_09/new`, read only)

kt26.11.16-8c4ca513, Ryzen 7 7700X, its iGPU through RADV (Mesa 25.0.7). One card ring (ring 1) carries the
display, the compute host, the front engine and the side engine; only that ring admits.

- `trace gpu` said near jobs' buffers came back "p50 607 ms p95 1990 ms" after their end. A near job of 208
  dispatches was cut into 33 pieces over 678 ms; each piece's `gpu_start` was written at the job's end,
  each with its OWN instant (a microsecond apart). The query grouped a job's pieces by that instant, so no
  two pieces ever grouped, every piece counted as a job's last one, and "after end" read the time from
  each piece's end to the job's end — the admission waits between pieces. Read per job (the pieces a
  job's span holds on its thread): `now` p50 36.4 ms p95 49.8, `near` 37.4 / 54.2, `background` 36.3 /
  52.7, against a clock error of 12–39 ms (the calibration's half-bracket). Row 693 was the query's.
- On the CPU clock alone (no calibration) a job's final submission to its buffers back took p50 52 ms in
  every class, against 6–12 ms of device run. That remainder is real, but the record could not split it
  into "queued on the card" and "after the end": the calibration's bracket was as wide as the waits it was
  to measure, because the calibration waited for EVERYTHING submitted to the compute device (D2).
- Row 662, in that job: the pieces went two at once, then the third waited 33–55 ms, released each time
  7–12 ms after the display's composite was submitted; ~10 ms of compute was admitted per ~55 ms display
  cycle. Over the run: 4623 near admissions waited more than 3 ms; in 4322 of them the prediction at the
  start did not fit the bound, and the wait outlasted what the prediction needed by p50 22 ms (p90 55;
  `now` 31 ms). So the ring's view of what was in flight stayed stale for tens of milliseconds after the
  card was done.
- The frames on screen waited for "the device finishing the frame before" p50 54 ms, with a floor at
  ~32 ms, on composites that cost the device a few milliseconds.

## D1. `trace gpu` reads a job's pieces as one job

A `gpu_start` belongs to the `compute_job` span that holds its instant on its thread; a record outside
every span is a job of its own (older traces, the test of t2362). The job's buffer wait is its records'
instant less its LAST piece's device end. His trace re-read: the numbers of D0. t2863.

## D2. Every wait names its own submission

The class: a wait for "everything submitted" (`PollType::wait_indefinitely`) waits also for every
submission any other thread makes to the device before the wait is issued, and holds the device's
snatch lock (read) through it. On the window's device the present takes that lock for WRITING, so a
long wait there also holds the UI's present, and every later submit or poll of the device queues behind
the waiting writer. stream-round-trips D9 (row 661) fixed it in the compute job alone.

`crate::wait_for(device, sub)` waits for one submission; `crate::wait_all` remains where waiting for
everything is the point, and is counted (`waits_for_all`, t2864 holds the frame paths at zero).

## D3. A ring's waiter sees a completion on any device in flight

A waiter that could not be admitted completed the OLDEST piece with a blocking wait on that piece's
device, with no timeout when the prediction could not make room. The devices of one card finish out of
each other's order (the compute host, the front engine, the side engine and the display are four devices
of one card on his machine); the room may come from a piece of another device first, and was seen only
when the oldest one ended. Now the waiter blocks on a device only when every flight is on ONE device and
nothing but a completion makes room (its queue completes in order: the oldest is exact), and then with no
timeout; otherwise it polls every device in flight at each round and sleeps on the ring's condvar (which
any completion, seen by anyone's poll, wakes) for at most `WAIT_TICK` (2 ms).

Never a `Wait` with a timeout: the first form of this change waited on the oldest piece for a tick, and
a full test run lost a loop-fill lane thread (t988 hung at exit) to wgpu 30's assert in `maintain` — "If
the queue is empty, the current submission index (46) should be at least the wait submission index (50)":
a timed wait that times out reads the fence, another thread's poll retires the submission meanwhile, and
the first finds the queue empty below its index. The old waiter's timed waits (up to 20 ms) carried the
same hazard, more rarely. t2865: a long piece on
one device, a short one on another, the ring's three lower pieces uncompleted — the waiter is admitted
when the short one ends (150 ms here), not after the long one (red without the change).

Rejected: a larger lower-class piece (`piece_limit` = the bound instead of half). The trace shows near
waits outlasting the PREDICTION, not pieces too small for it; changing the cut would move what the
display sees in front of its present, and his next trace has to say first whether D2–D4 close it.

## D4. The present's checks stop growing

`wait_presented` checked after the frame's measured time, then at waits doubling from a quarter of it,
without a cap: a frame ended late was seen up to its lateness again later (a frame done at 33 ms of a
1 ms guess was seen at ~64 ms). The waits now stop at the frame's own time, at least 4 ms
(`present_waits`, t2866); t2260 still holds a few checks per present.

## D5. Benches (release, our showcase with Datamosh, `/dev/shm`)

`kt-cli showcase … --mosh`, `kt-cli preview mosh.kt 10 14 --size 640x360 --loop 40 --trace`, three
interleaved before/after pairs on a machine at load 24–26 (shared): kept 69/83/74 moments before against
77/70/66 after, V-fps 23.9–24.3 in all six, process CPU 0.7–0.8 cores in all; `trace gpu`: a job's buffers
after its end p50 0.5–2.3 ms in both. Within noise, as expected: on llvmpipe every device is a ring of its
own and the loop fill runs one lane, so neither the cross-device waiter (D3) nor waits behind other lanes
(D2) occur here. The proof is t2864/t2865 (red without the change) and his next trace: `trace gpu` near
admission share against 486 %, buffer wait per job against 37 ms with the narrower clock error.

Load at rest: nothing here runs without work. The 2 ms tick runs only while a submission waits for
admission with flights on several devices; `wait_presented` only while a frame is in flight.

## Class walk: every wait for a device's work

| Path | Device | Was | Now |
|---|---|---|---|
| compute job buffers (`run_job`) | compute | own submission (D9) | own (`wait_for`) |
| compute clock calibration | compute | everything | own (D2) |
| compute shader warm-up (`prepare`) | compute | everything, then own | own |
| engine read-back frame (`execute`): loop fill lanes, effect cells, reel strips, posters, slow route, bytes preview | side / offline / display | everything | own (D2) |
| export ring (`collect`) | export | own | unchanged |
| export single frame (`execute_once`) | export | through `execute` | own |
| held front frame (`execute_held`) | compute | never waits | unchanged |
| present (`wait_presented`) | display | callback + uncapped checks | capped (D4) |
| pipeline warm-up draw (`warm.rs`) | any, the display's on a card | everything | own (D2) |
| display read for tests (`read_target`) | display | everything | own |
| arbiter `Submitted::wait` | any non-surface | own | unchanged |
| arbiter admission waiter | all of a ring | blocking on the oldest | ticks with several devices (D3) |
| `park`, `finish_present` | any | everything | everything, counted |
| pass-time unit tests, gpu_errors tests | test devices | everything | unchanged (tests) |
| thumbnails, analysis (kt-app) | side engine | through `execute` | own |

## Callers of what moved

- `crate::wait_for` / `wait_all`: `engine.rs`, `warm.rs`, `compute.rs`.
- `present_waits` (new, pub): `GpuFxEngine::wait_presented`, t2866.
- `trace_query::job_spans`: `gpu` only.
