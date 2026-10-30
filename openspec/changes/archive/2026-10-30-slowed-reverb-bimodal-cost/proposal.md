## Why

Rendering one clip through the core Slowed+Reverb effect costs either ~104 ms or ~415 ms per
10 s of audio on a release build, switching between the two from one process run to the next on
identical code and identical input. It was found by the release speed check of
`plugin-api-stage1` stage 3 (a scratch test: `AudioRenderer::render_frames_with`, 10 s of
`synth:clicks`, a single `slowed_reverb` slot with every number row at 70 % of its range, best of
5 renders per process). Measured on two trees: c53131f gave 104 / 415 / 103 ms, ed78f71 gave
415 / 414 / 104 ms. The other 17 audio kinds were stable to ±1 % in the same runs. The chain of
all 18 kinds together stayed at ~628 ms.

A 4× cost that a person pays at random is a playback defect, not a curiosity. On a loop with a
few slowed events it decides whether the audio render keeps ahead of the callback (invariant 6:
starvation is heard as silence). Because the data is deterministic and only the PROCESS changes,
the cause is something a process gets differently each run. The owner asked for a spec before the
investigation (2026-10-30).

## What Changes

- Find the cause and name it with numbers. Suspects, in order:
  1. **Memory placement of the reverb's lines.** 4K aliasing or cache-set conflicts between the
     eight comb and four allpass buffers per channel, which `Vec` places at addresses ASLR
     varies per run.
  2. **The core type the render thread lands on**, if the machine is hybrid.
  3. **Subnormal floats in the comb/allpass feedback.** `DelayLine` flushes them, the freeverb
     lines do not; this matters if FTZ/DAZ differ per thread.
  4. **The rubato warp at rate 12.8.**
- Fix it WITHOUT moving a byte of the sound: `GOLDEN_AUDIO` (t11), t973c and the tail test t6 stand.
- Make the cost reproducible in a test where a deterministic proxy exists: for example, the reverb
  lines' layout or the count of subnormal states. Wall-clock timing is not a CI assertion here.
- Write down what the measurement found in `audio-render`'s design and, if it is a code rule, in
  kt-fx's `CLAUDE.md`.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `audio-render`: a requirement that the cost of a chain does not depend on the process run (the
  same render, the same cost class), with the test chosen by the investigation.

## Impact

- `src/libs/kt-plugin-api/src/dsp.rs` (Comb, Allpass), `src/libs/kt-fx/src/audio/nodes.rs`
  (Freeverb, ReverbNode), possibly `src/libs/kt-fx/src/audio/warp.rs` (the rubato resampler
  setup).
- Release builds are measured with `cargo test --release` and `kt-cli probe … --time`, never debug
  (item 301).
- No format change, no UI change, no new dependency unless the investigation proves one is needed.
