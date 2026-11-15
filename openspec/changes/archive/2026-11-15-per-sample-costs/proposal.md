# Proposal: per-sample-costs

## Why

The orchestrator's brief (stage С.1, performance): close the open rows of `docs/found-defects.md`
about work in inner loops — 128 (Phaser's per-sample filter math), 350–360 (the audio render's
per-chunk allocations and per-sample constants), 361–363 (the composite's, Camera ISP's and the
transitions' per-pixel constants). The class is "an inner loop recomputes, per element, what is
constant over a larger unit, allocates per unit, or does a channel's work twice". The change walks
every audio effect and every shader for it (`design.md`, "Class walk").

## What Changes

- **Every audio fix keeps the bytes** (invariant 11): tables, memos and kept buffers hold exactly the
  values the per-element code computed, in the same order of operations; a cheaper form of a frozen
  function is that function to the bit, proved on every `f32`.
- Audio rows:
  - 128 Phaser, and with it the compressor's gain and the distortion's drive: `10^x` through
    `math::exp10f` — `powf(10, x)` to the bit, through `exp` (t2682 exhaustive). The per-sample
    sweep stays per sample (the owner's sound decision is untouched).
  - 350 the warp keeps its chunk buffers (`process_into_buffer`).
  - 351 a strength's dry side and the tail twin copy into kept buffers.
  - 352 the mixdown takes its buffers from a pool in `FxStateStore`, the active-event lists too; the
    reel maps once per build; a sub-mix burst the ring let go is filled again; the warp map's
    reading at a seam is memoized (a loop wraps to the same frame).
  - 353 the camera mic's anti-image filter sums only the kept taps (polyphase), the full sum where
    the result is an exact zero (its sign).
  - 354 the camera mic and VHS Tape's sound renew their rows and deck in place.
  - 355 the EQ and the filter tune one channel and copy the coefficients.
  - 356 delay lines, combs, allpasses and the pre-delay wrap by compare-and-reset; `read_frac`
    truncates (exact for `d ≥ 1`) and wraps by one add.
  - 357 VHS Tape's sound asks the dropout test once a frame, not per channel and sub-sample.
  - 358 the camera mic's gust levels once per cell and once per sample for both channels.
  - 359 Pitch's vocoder does not zero what it overwrites; its synthesis window is pre-scaled.
  - 360 the noise generator hashes a row only when its held value changes.
- GPU rows:
  - 361 the composite takes the two sides' sizes once per fragment (same bytes, checked A/B).
  - 362 Camera ISP with defocus decodes its source's transfer from a 256-texel table the device
    makes by the same `decode` (EXACT against the per-tap form, t2683).
  - 363 measured: the per-pixel trig of per-frame constants is not a cost on the device; moving it
    to the CPU would move bytes for nothing — not taken, the row closes with the figures.
- Tests t2680–t2684; found-defects rows 128, 350–363 leave the list.

## Capabilities

### Modified Capabilities
- `audio-fx`: inner loops do no work they can take once (t2680, t2682).
- `audio-render`: a warm render allocates nothing (t2684).
- `video-fx`: per-fragment constants once; Camera ISP's transfer table (t2683).
- `generators`: noise rows hashed when they change (t2681).

## Impact

kt-plugin-api (`dsp`, `math`), kt-fx (`audio/mod.rs`, `warp.rs`), kt-engine (`audio.rs`,
`fxstore.rs`), kt-gpu (`composite.wgsl`), plugins fx-audio-{tone,modulation,dynamics},
fx-video-{camera,tape}, generators-audio. No format change, no UI change.
