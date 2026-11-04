# Proposal

## Why

`docs/found-defects.md` row 11: the first play after opening starves audio, +51k underrun frames on
release, measured before the prefetch work and not since. Re-measured in the real release program
under Xvfb, playing at once after opening, counting from the first moment, showcase at 640×360:

- on the code before `preview-throughput`: 98,511–126,903 frames on every start;
- after it: 0 on 3 starts of 4, and on the fourth exactly 32,803 frames in one burst at the first
  moment. The old code shows the same 32,803 on its worst run.

So most of row 11 was the decoder's seeks, closed by `preview-throughput`. What remained is a race:
the render fills the whole ring in one pass the moment Play lands, ahead of the decoders. Two events
sound at that point, and the ring holds 16,384 frames: 2 × 16,384 + a remainder, 0.34 s of silence.

## What Changes

- A start (opening, a seek, a change of direction, a loop flip) waits until the bursts of a whole
  ring and the look-ahead margin are in the cache, at most `START_AUDIO_GRACE` (500 ms). The clock
  stands meanwhile, as it already does before the first frame.
- The output callback does not count a pass with nothing to play before that first frame as an
  underrun: it is a state, not a failure.
- `kt-cli preview --cold` plays with no warm-up.

## Capabilities

### Modified Capabilities

- `transport`: what happens between Play and the first frame.

## Impact

`src/libs/kt-engine/src/player.rs`, `src/apps/kt-app/src/audio_out.rs`, `src/libs/kt-engine/src/preview_bench.rs`,
`src/apps/kt-cli`. Invariant 6 holds: the wait lies before the first frame, where time already stands.
Playback starts up to 500 ms later when its audio is not there, and only then.
