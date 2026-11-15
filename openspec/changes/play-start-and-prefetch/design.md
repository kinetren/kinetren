# Design: play-start-and-prefetch

## Context

Row 525: t2335 (`kt-engine/tests/loop_fill.rs`) waited its whole 600 s deadline under load on the
gpu-throughput branch, at stage-11 (de81f203) and after; no thread dump, ptrace being off for foreign
processes. Row 516: `preview_bench` modelled a device that is always open, so nothing measured what
Play waits for since kept-mix-settle opens the device on Play (7–20 ms here, 4.7 s and 23.4 s in a
parallel live run, row 425).

## D1. Row 525 is the test's own race, fixed before this change

`ptrace_scope` is 1 here, so gdb may trace its own child: `repro.sh` (the session's scratchpad) starts
N busy loops, runs the test binary under `gdb -batch` and interrupts it past a threshold, printing
every thread's stack.

- The test as it was when the row was written (count read AFTER the seek): 8 of 12 runs hung at load
  48. Stacks: the prefetch thread parked in `Wake::wait_since` (correct — the state did not move), the
  test in `passes()`. On a pause a seek owes exactly one pass; under load it landed before the count
  was read, and the test then waited for a pass nothing would ask for.
- ec6be337 (followups-races, merged after the row) reads the count before the seek. It was on a
  branch parallel to cfb992e8, the commit that recorded row 525, so the row was never re-checked.
- The current test: 12 of 12 at load 48, 30 of 30 at load 96 (a run takes up to 80 s there, so a
  60 s threshold at first looked like hangs — every stack showed the prefetch thread parked between
  passes and the test waiting for the next one). With the prefetch loop logging every decision, no
  `passes_since` waited over 30 s.

Nothing in the prefetch path loses a wakeup: it takes `seen` before it looks at the state and parks
on `wait_since(seen)`, so a notify between the look and the wait moves the count it waits on. The
seek bumps the generation before it notifies, and the generation is in the pass state.

## D2. The class walk: every hand-off to a parked thread

The mechanism of row 525 as suspected is a lost wakeup: a state change and its notify landing between
a thread's look at the state and its wait. Every place a thread parks on another's news, checked:

| Hand-off | Where | Verdict |
|---|---|---|
| render / prefetch / audio-output park on `Wake` | `player.rs` `Wake`, `prefetch_loop`, `audio_render_loop`, `audio_out.rs` | sound: `seen` before the look, `wait_since(seen)` |
| prefetch's timed pass while playing or a state moved | `prefetch_loop` | sound: a timer, the next pass reads the state |
| the build's want, published build, gate | `builds.rs` | sound: changes under the lock; `shutdown` takes each lock before notifying |
| **the compose thread's stop** | `compose_queue.rs` `Composer::stop` | **lost**: the flag set and notified without the ask lock; a stop between the look and the wait parked the thread for good (it is not joined, it held the player's preview parts) — fixed, t2640 |
| the compose thread's ask and nudge | `Composer::ask`, `nudge` | sound: the ask is replaced under the lock; the nudge takes the lock before notifying |
| a finished frame to the UI | `Composer::take` | sound: timed, under the lock |
| the media worker's queue, retune, close | `worker.rs` | sound: every change under the queue lock, notify after |
| generation cancels | `Worker::cancel_generations_below`, `withdraw_video` | sound: under the queue lock; a dropped look-ahead request is asked again by the next pass, which the moved state makes |
| the scheduler's gate (checkpoint, `wait_gate`), lane idle, workers, `Answer` | `kt-time/src/sched.rs` | sound: the pause mask is swapped under the scheduler lock; `cancel`, `notify_gate` take it; lane state under the lane lock |
| the loop fill's kick | `loop_fill.rs` → `Lane::kick` | sound: the scheduler's |
| **the trace writer's park** | `kt-time/src/trace.rs` `wake` | **lost, rarely**: Dekker's handshake (writer: store `PARKED`, look at the ring; producer: publish, look at `PARKED`) with only the writer's side `SeqCst`; a store followed by a load of another location may be reordered (x86's store buffer), and the writer could park over a record until the next one — fixed with a `SeqCst` fence before the producer's load. No test can force the reordering; a flush or the next record always drained it, so nothing was lost for good |
| kt-gpu's warm worker idle, frame done, exit guards | `kt-gpu/src/compiled.rs`, `exit.rs` | sound: under their mutexes |
| kt-gpu's arbiter rings | `kt-gpu/src/arbiter.rs` | sound: timed waits (2 ms) |
| **the seek's flush and the start's pass** | `Player::seek`, `audio_render_loop`, the callbacks | **lost pass**: the consumer drains `flush` when it next runs; a render thread that already took the new generation could write the start's whole pass before that drain, which then dropped it — the start lost its first ring and played ahead of the clock by it. Worse, `Player::seek` stored the flag AFTER the generation, so the render thread could see the new generation without the flag. Fixed (D3), t2641, t2644 |
| the journal writer, export pipeline channels | `tele_journal.rs`, `export_pipeline.rs` | sound: blocking `recv` on channels, disconnect on drop |
| **this change's own paused stream: a seek during the pause's check** | `audio_out.rs` `output_loop` | **lost drain** (found by t2644's second round, red on it): the pass that marked the pause checked parked on a `seen` taken after the seek, so the flush stayed pending and the start held for good — that pass now loops to look again |
| a crash report's bystander threads (a test child) | `kt-crash/tests/handler.rs` | **naming race**: a thread names itself when it starts, and the probe faulted before a bystander ran (t2080 red in the full parallel run) — the child now waits for each to start |

The callers of what moved: `AudioShared::flush` is set by `Player::seek` and the render thread (a
change of direction, a loop flip) and drained by `audio_out::Sink::fill`, the output thread while
closed or paused, `preview_bench`'s device, and two tests' devices (`mix_settle_player.rs`,
`loop_disarm.rs`) that leave `drains_flush` unset; `Device::open` is called only by `output_loop`
and implemented by `CpalDevice` and the test double; `BenchOptions` is built by `kt-cli preview` and
tests through `..Default::default()`; `play_name` is `kt-cli trace`'s.

## D3. A start waits for the drain of its flush

`Player::seek` stores `flush` before it bumps the generation, so a render thread that sees the new
generation sees the flag (release/acquire through the generation). While `starting` and the flag is
pending, the start holds (`START_HOLD` b = 3) — time stands before the first frame, so this costs no
sync (invariant 6). The hold applies only when the consumer says it drains flushes itself
(`AudioShared::drains_flush`: the app's output thread, the bench's device); a test that pops the ring
by hand leaves it unset and sees the former behaviour. Draining with the device closed, the output
thread notifies the player's wake, so a paused render thread fills the start at once.

## D4. Play's first sound is measured

`Player::play` stores `play_at_ns`; the render thread, on the first frames a callback handed out after
a Play, writes `play` aux `first_sound` (`b` µs) and `AudioShared::first_sound_us`. It is measured
where the clock is: the frames consumed are what the device took. `preview_bench`'s device double
opens `device_open` after it sees Play (a pause closes it); `kt-cli preview --device-open-ms N`
prints "play → first sound"; `kt-cli trace summary` prints p50 and max of the first sounds, the
device's opens and resumes.

## D5. Play does not reopen the device

Measured with a cpal 0.16 probe on this machine (PipeWire through ALSA), alone and beside ~100 busy
loops: an open plus its first callback took 28–62 ms alone and 46–112 ms under load; a paused stream
made 0 callbacks and 1–7 context switches in 3 s in our process and 2–4 in PipeWire's, the same as a
closed one (0–26); a resume returned in 20 µs and its next callback came 27–45 ms later (the device
buffer playing out silence written before the pause).

So the output thread pauses the stream at rest (after `CLOSE_AFTER_REST`) instead of dropping it, and
Play resumes it. The first Play still opens the device: the owner decided "open the sound on Play"
(kkt-26.11.10 J6), and an open ahead of it is not his decision. Safeguards:

- a backend whose `pause` fails is closed at once; one whose callbacks keep coming over `PAUSE_CHECK`
  (300 ms) is closed after it — the program at rest costs nothing whatever the backend. `OutputCtl::paused`
  says a pause was CHECKED (the tests wait for it, never for a sleep of `PAUSE_CHECK`), `closes` counts
  drops;
- a paused stream holds the ring, so a seek while paused is drained by a short resume (one callback of
  silence while the transport stands), then paused again; the render thread is woken to fill the start;
- a resume that fails drops the stream; the next pass opens a new one.

Rejected: opening the device when a project opens (not the owner's decision, and an exclusive raw ALSA
device would be held while nothing plays); a longer `CLOSE_AFTER_REST` (an open stream costs wakeups,
row 274); a wall-clock stand-in clock until the device opens (invariant 6: the callback is the clock,
and the first sound would be lost, not late).

## D6. Measurements

Release `kt-cli preview`, the heavy showcase (`kt-cli showcase --heavy`: 401 events, 25 transitions)
from /dev/shm, 5–8 s at 640×360; Play → first sound as the render thread measured it (the bench's
device calls back every 2048 frames, 42.7 ms, so an open lands on the next period):

| device opens in | alone | beside 96 busy loops (32 threads), two runs |
|---|---|---|
| 0 ms (always open, the former bench) | 28.5 ms | 130.0, 86.3 ms |
| 20 ms (an open on this machine) | 84.9 ms | 166.3, 138.3 ms |
| 2000 ms (row 425's loaded opens) | 2046.0 ms | 2037.5, 2042.7 ms |

No callback or render underrun in any run. The open adds itself plus up to a device period; under
load the start's pass and a period cost the rest. The live `idle` scenario (the real kt-app on
PipeWire): the device's open with its first callback 25.8 ms, Play → first sound 69.5 ms; on the
pause after the play no thread of the sound woke over 5 s with the stream paused. A paused stream's
resume (the cpal probe, D5): 20 µs, next callback 27–45 ms, alone and beside ~100 busy loops.
