## ADDED Requirements

### Requirement: A start's first pass reaches the output whole
After a start (opening, a seek, a change of direction, a loop flip) the render thread SHALL hand its
first pass to the ring in one commit, so the callback, which primes on the first frames in the ring and
runs the clock from then on, never begins on a pass that is still being rendered. Time stands before
the first frame (invariant 6), so this costs no sync; the start waits for the pass as it waits for its
sound (cold-start-audio).
Why: t1935 went red four times under loaded full runs with two callback underruns (found-defects 158).
Its structured trace at load 192 placed them in the first lap: the start's pass rendered 17 bursts live
in 83 ms and pushed them one by one, the callback primed on the first, and a render thread put off
between two bursts let it eat them. The bench's device thread was on time (≤ 29 ms late), so the cause
was the start, not the bench; a person's machine under a build plays the same start.

#### Scenario: A start's pass held back by a slow render thread
- **WHEN** every burst of a start's pass holds the render thread 30 ms (a burst is 21 ms of sound)
- **THEN** the device never finds the ring empty; with the pass pushed burst by burst it found it empty
  for 5122 frames
- **TEST** t2216 (`src/libs/kt-engine/tests/start_pass.rs`)

#### Scenario: The covered loop under load
- **WHEN** t1935 runs beside 128 busy loops on 32 cores
- **THEN** no callback underrun; the bench prints how late its device thread woke, so a red names
  whether the device or the render thread failed
- **TEST** t1935 (`src/libs/kt-engine/tests/mix_player.rs`)
