# kt-time

> The crate's contracts and mandatory tests — `openspec/specs/{time-model}/spec.md`; the reasons for the decisions — `design.md` next to it.

The time model of the whole engine. Zero dependencies (except serde).

## Invariants
- A tick = 1/90000 s, `i64`. All internal time and the entire project format — ticks only. Milliseconds exist only at the UI boundary (`Tick::as_ms`).
- `AUDIO_BURST_TICKS = 1920` — an exact integer; the boundaries of audio bursts are converted one-to-one into sample numbers (test `audio_burst_step_is_exact`). Do not change 90000/48000/1024 separately — they are coordinated.
- `FpsGrid::new` panics on an fps that does not divide 90000 — the frame grid MUST be integer.
- **`test_deadline()` / `TEST_WAIT` are the bound of a test's wait, anywhere in the workspace** (test-stability, t1671). They live here because every crate that tests a wait depends on this one; nothing in a product path may use them.
- **`cost` is the walk meter of builders** (builder-cost-guard, t2061–t2067): a thread-local count of the elements a walk over a whole structure touched (`Cost` rows, append only). A pure function of the project, never a clock; it steers nothing. A new walk over something a project can make long (events, keys, sampled points, placements, reels) adds what it visited, so `kt-engine/tests/build_cost.rs` sees it.
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
