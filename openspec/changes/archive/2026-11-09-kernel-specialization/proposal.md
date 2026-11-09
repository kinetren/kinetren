# Proposal: kernel-specialization

## Why

The owner, 2026-11-09 (registry `docs/ktprompts/kkt-26.11.09-index.md`, I9): "запускай edit-without-rebuild
и kernel-specialization". `docs/engine-audit.md` judged JIT for every computation ("про JIT я вообще-то имел
ввиду не только шейдеры") and took tier T2 for the audio kernels: the same Rust compiled into "stands
still" variants chosen at run time, run-time machine code rejected (row 7 of its order table, mechanism M7).

The class: **a kernel does per-sample or per-block work whose inputs have not changed.** The filter and
the EQ re-tune their biquads (a `sin_cos` and a `pow` each) every 128 frames whether or not a value moved;
the compressor takes `log10` and `pow` of its envelope every sample even below its threshold, where the
answer is "no reduction" (found-defects row 129); a smoother at rest still ticks; the mixdown computes a
crossfade weight (branches, a `sqrt`, a binary search of the overlap map) per sample also where the weight
is exactly 1; the loudness meter shifts its true-peak tail by a rotate per sample and runs its scalar `f64`
filters at the baseline level, like the peaks pyramid; the camera microphone's resampler shifts two
63-sample histories per sample.

## What Changes

- **Smoothers know they stand still.** `SmoothedParam` remembers the target at which a tick changed nothing
  (a fixed point of a pure function of the current value, the target and the coefficient) and answers from
  it; `advance(n)` gives the value after `n` ticks without running them at rest. A block whose smoothers
  all stand still takes the kernel's constant-parameter path.
- **A re-tune only when its inputs changed** (the `Memo` pattern on the biquad's inputs): filter, EQ.
- **Compressor below threshold, exactly**: under a safe level computed once per threshold the gain is the
  make-up gain to the bit; the `log10`/`pow` run only near and above the threshold. Distortion's level
  compensation joins its drive memo; the automated gain at rest multiplies by a constant.
- **Mixdown in runs**: a block is split into runs of constant weight (a plain multiply) and ramp runs (the
  per-sample formula), for events with and without effects.
- **Analysis at the CPU's level**: the loudness meter and the peaks pyramid through `fast!`; the true peak
  reads a doubled ring instead of rotating its tail; the interpolator's table is fetched once per feed.
- **The camera microphone's resampler** reads doubled rings instead of shifting its histories.
- **One reference switch**: `kt_plugin_api::cpu::with_general` runs every kernel and the mixdown on this
  thread on its general path. Every specialized path is held byte for byte to it (t1981's pattern).
- Tests t2170–t2176.

## Impact

- `kt-plugin-api` (`dsp::SmoothedParam`, `dsp::Biquad`, `cpu::with_general`), `fx-audio-tone`,
  `fx-audio-dynamics`, `fx-video-camera` (mic), `kt-fx` (`audio::loudness`), `kt-media` (`index::PeakAcc`),
  `kt-engine` (`audio.rs` mixdown).
- No sound change, no format change, no new dependency: invariant 11 holds byte for byte (t973c, t1981, the
  golden audio and the new two-path tests). Phaser's per-sample filter (row 128) stays: the owner's decision.
