# kt-time

> The crate's contracts and mandatory tests — `openspec/specs/{time-model,telemetry}/spec.md`; the reasons for the decisions — `design.md` next to it.

The time model of the whole engine. Zero dependencies (except serde).

## Invariants
- A tick = 1/90000 s, `i64`. All internal time and the entire project format — ticks only. Milliseconds exist only at the UI boundary (`Tick::as_ms`).
- `AUDIO_BURST_TICKS = 1920` — an exact integer; the boundaries of audio bursts are converted one-to-one into sample numbers (test `audio_burst_step_is_exact`). Do not change 90000/48000/1024 separately — they are coordinated.
- `FpsGrid::new` panics on an fps that does not divide 90000 — the frame grid MUST be integer.
- **`test_deadline()` / `TEST_WAIT` are the bound of a test's wait, anywhere in the workspace** (test-stability, t1671). They live here because every crate that tests a wait depends on this one; nothing in a product path may use them.
- **`cost` is the walk meter of builders** (builder-cost-guard, t2061–t2067): a thread-local count of the elements a walk over a whole structure touched (`Cost` rows, append only). A pure function of the project, never a clock; it steers nothing. A new walk over something a project can make long (events, keys, sampled points, placements, reels) adds what it visited, so `kt-engine/tests/build_cost.rs` sees it.
- **`trace` is the structured trace of a run** (structured-telemetry, capability `telemetry`, t2140–t2143):
  typed 64-byte records (`Ev` documents each field), a lock-free ring a thread stores into without a
  lock or an allocation (after its first record, which names the thread), one writer that parks when a
  drain finds nothing, bounded ring files under the data directory. Rules: a new event is APPENDED to
  `Ev` (the numbers are in files) with its fields documented and its row in the change's class walk;
  the audio CALLBACK records nothing (the render thread may); a text record (`push_text`, `machine`,
  `journal`) locks and allocates — never from the render thread's loop; nothing records at rest (a
  hover UI frame must not); a thread that does work for a frame sets `with_cause(frame)`. The questions
  live in `kt-cli trace` (`apps/kt-cli/src/trace_query.rs`), not here.
- **`sched` is the one executor of background work** (one-scheduler, capabilities `media-index`,
  `telemetry`, t2230–t2234): classes in a fixed order, lanes (FIFO, one job at a time; one-shot or a
  pump), a pool of a quarter of the cores (3 to 6) that keeps one worker for `Answer`, pause votes
  (`Vote`, dropped with their owner), tokens, per-class counters, `SchedQueue/Run/Cancel/Pause`
  records. Workers and checkpoints wait on condvars: nothing here may wake on a clock. Std only.
- **`block` counts blocking calls on the threads that must not wait** (t2235): the UI and the audio
  render thread mark themselves; a call that can stand on the disk or a build opens `wait(kind)`.
- **`work::park_until_work`** parks a watcher until any counter moves; `add` pays one relaxed load
  while nobody is parked (t2241).
- **`mem` is the memory ledger** (memory-ledger, capability `media-index`, t2290, t2291): a keeper of
  pictures, mixes or device buffers opens an `Account` of its `Tenant` with its BASE (never cut) and
  reports `set(bytes)` (atomics: no lock, any thread). Growth above the bases is granted from ONE
  budget, half of (available + growth held), in `Tenant`'s order — kept composites, stream history,
  decoded frames; unknown memory or a live `Claim` (an export) grants nothing. A grant that shrinks
  calls the kind's `on_shrink` hooks after the lock and never for the kind that asked: a keeper asks
  under its own lock, and its hook takes it. A sizing runs only when a want or a claim changes, and
  records `mem_budget`/`mem_grant` only when something moved. A new cache that holds pictures, mixes
  or device buffers opens an account; one that can use more than its base asks a want.
  `Ledger::sample` (frames-cache-growth, t2395) writes `mem_sample` (the process's resident memory by
  the host's `set_resident_probe`) and `mem_used` per tenant only for what moved by a MB; the caller
  samples once per BUSY second (the app's load watch, `kt-cli preview`), never at rest.
- Negative ticks are valid (floor semantics via `div_euclid`) — indexing down to zero relies on them.
- **Subtlety**: the 48 kHz frames ↔ ticks are mutually exact only for frames that are multiples of 8 (ticks that are multiples of 15): `from_audio_frames(f).to_audio_frames() == f` ⇔ `f % 8 == 0`. Clip boundaries live in ticks; the engine floors them into frames once (kt-engine::AudioRenderer). In tests that set clip boundaries in frames, use multiples of 8 — otherwise the seam would drift by 1 sample (see tests/audio_engine.rs).

## Must not
- Add float time to the public API (except explicit `*_secs_f64` for UI/debugging).
- Add dependencies.

## The Windows GNU `round` (windows-build)
- `src/libm_fix.rs` (only `cfg(all(windows, target_env = "gnu"))`) defines `round`/`roundf`:
  mingw's own return 1 for `round(0.49999999999999994)`, and every `.round()` of the CPU
  reference went through them. kt-time is in every binary, so the linker takes these first.
  Do not move the module to a crate that some binary does not link.
