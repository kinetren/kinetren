# Time model — why it is this way

## What hurt and for whom

The old engine (C++ v1) kept time in milliseconds and in the source's frame rate. Two
consequences cost audible defects: the warp path counted in the file's sample rate, and
for 44.1 kHz every ~230 ms the source's burst dropped out (periodic clicks); the ms
buckets of coverage did not match frame boundaries. The owner demanded "no milliseconds in
the format".

## Decisions and why

- **90 000 ticks per second.** The smallest convenient number divisible by 24, 25, 30, 60 and
  by the 48000/1024 burst step (1920). Hence tick↔sample = 15/8 exactly, and a frame of any
  rate is an integer number of ticks. Float time does not accumulate error, because there is none.
- **Burst = 1024 frames = 1920 ticks.** The size was chosen for the cpal callback ring and for
  the cache (`kt-cache` keys by burst index). The three constants are linked by a formula;
  changing one breaks the one-to-one mapping.
- **Negative ticks are valid.** Indexing down to zero is needed by reels (the zero of a reel is
  not a wall, capability `compositions`) and by seek's pre-roll; the floor is taken via
  `div_euclid`, not truncation.
- **The tick is the format unit.** `.otio` stores only ticks (capability `project-format`).

## Rejected and why

- Time as f64 seconds — rejected: error accumulates on long timelines and results are
  non-deterministic across platforms (capability `video-plan`, the determinism invariant).
- Milliseconds as in v1 — rejected: 1 ms is a multiple of neither a 24/30 fps frame nor a burst.

## Non-obvious details

- 48 kHz frames ↔ ticks are mutually exact ONLY for frame counts that are multiples of 8
  (ticks that are multiples of 15): `from_audio_frames(f).to_audio_frames() == f` ⇔ `f % 8 == 0`.
  Clip boundaries live in ticks; `AudioRenderer` (`src/libs/kt-engine/src/audio.rs`) floors
  them into frames once. A test that sets clip boundaries in frames not multiples of 8 would get
  a seam shifted by 1 sample — and that is not an engine defect.
- The playhead in the UI lives in AUDIO FRAMES, not in ticks (capability `transport`): the clock
  is the cpal callback, and its quantum is frames.
- `FpsGrid::new` panics on an fps that does not divide 90000 — deliberately: a fractional grid
  would be a silent format error.

## Deferred

Nothing.
