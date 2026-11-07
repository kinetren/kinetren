# Design: audio-fx-cost

## The mechanism

The owner's example names one place ("врубаю vhs tape на видео - терпимо, добавляю на аудио - всё"); the
mechanism behind it is an audio kernel that does more per sample than its sound needs. Two forms of it
were found, and both act in every kernel that has the pattern, not in VHS alone:

1. **A library call per sample on the shipped build.** The release build is baseline x86-64 (SSE2, R.32:
   a CPU without AVX must run it). SSE2 has no instruction for `floor`/`ceil`/`round`/`trunc`, so LLVM
   lowers each one to a call into glibc through the PLT, and `f64::fract` is a `trunc`. Before this
   change the disassembly of the release binary showed such calls in the per-block body of ten of the
   21 audio kernels — VHS Tape's sound had fifteen call sites (the FM phase wrap, the sub-sample
   phase, `sin_cyc`'s fold, the tape position, the ring read, the dropout cells), chorus/flanger/
   vibrato/delay their fractional delay reads, bitcrusher its hold grid, pitch, the camera microphone
   and the stream's sound their grids. A call per sample also keeps every loop around it scalar.
2. **Pure per-sample math redone while nothing changes.** A smoother at rest returns the same bits
   sample after sample, and `10^(dB/20)` of them is the same value: the limiter took two `pow` per
   sample, the gate two, the compressor one, distortion three transcendental functions, bitcrusher a
   `pow`. VHS Tape's sound took a `pow(x, −0.5)` per sample per channel in its compander (exactly a
   reciprocal square root), shaped a linear track nobody listens to with an `exp` per sample, walked 33
   dropout cells (three hashes each) per sample per track for an answer that changes once a millisecond,
   and shifted a 65-tap history by one per sample.

## What was done

- `kt_plugin_api::cpu::run_fast` / `fast!`: the per-block body of every audio plugin's `process` is
  compiled twice — as built, and inside a `#[target_feature(enable = "avx2")]` function — and the AVX2
  copy runs where `is_x86_feature_detected!("avx2")` says so (once per process; `KT_SIMD=0` forces
  the baseline, as kt-fx's SIMD kernels; `with_baseline` does it for one thread, for t1981). The
  closure is `#[inline(always)]`: a closure with two callers is not inlined by itself, and a body left
  out of line keeps the baseline code (the first try showed exactly that in the disassembly).
- VHS Tape's sound: the Hi-Fi receiver's eight sub-samples are split into passes — the modulator's
  phase (a running sum) one by one, then the carrier, noise and detector of each sub-sample side by
  side, then the two sums in the original order; `atan2_rad` answers the origin with a select at the
  end instead of an early return (the WGSL mirror keeps its return: the same value, and a GPU's 0/0 is
  not defined); `powf_neg_half` replaces the compander's `pow`; the linear track records its raw input
  and shapes it (`tanh`) only when that track is read, with the drive it was recorded at; the dropout
  field keeps the starts of the cells around the current one; the loss filter's history is a ring
  written twice so its window is one slice; a mono deck reads its one track's dropout once; the
  passport is built once, not per automation chunk; the displacement helpers are `#[inline]`.
- `dsp::Memo`: the last key's bits and the value of a pure function of it — gate, limiter,
  compressor (make-up), distortion (drive, tone), bitcrusher (steps).
- `math::powf_neg_half`: `1/sqrt(x)` in `f64`; `powf` itself outside the positive finite numbers and
  where the `f64` value lies within 2⁸ ulps of a boundary between two `f32` roundings. Both `pow`'s
  `f64` result (libm's FreeBSD `e_pow`, "nearly rounded") and `1/sqrt` lie within about an ulp of the
  true value, so away from a boundary they round to one `f32`. t1982 checks all 2 139 095 040 positive
  finite floats (7 s on 16 threads).
- `kt_fx::audio::cost` + `kt-cli fx-cost`: the measurement, ONE text for the tool and the guard.

## Measurements

Release build, this machine (KVM guest, AVX2; other sessions' builds pushed the load to 15–60, so every
number is best of 7 and read against the reference kernel measured alternately in the same run). Each
cell: share of real time of one instance on 5 s of the stereo stimulus, then the cost in reference
kernels (eight cascaded biquads per channel). "Before" is db6df6eb's plugins built with this change's
measuring code; "baseline" is this change with `KT_SIMD=0`. Heavy settings: `cost::heavy`.

| kind | defaults: before | after (AVX2) | after (baseline) | heavy: before | after (AVX2) | after (baseline) |
|---|---|---|---|---|---|---|
| gain | 0.000% · 0.01 | 0.001% · 0.01 | 0.000% · 0.01 | — | — | — |
| slowed_reverb | 0.083% · 1.78 | 0.082% · 1.78 | 0.082% · 1.76 | 0.087% · 1.87 | 0.089% · 1.89 | 0.089% · 1.89 |
| filter | 0.018% · 0.39 | 0.024% · 0.52 | 0.018% · 0.40 | — | — | — |
| eq | 0.009% · 0.19 | 0.009% · 0.19 | 0.009% · 0.19 | 0.119% · 2.56 | 0.042% · 0.91 | 0.119% · 2.58 |
| distortion | 0.185% · 3.96 | 0.017% · 0.35 | 0.015% · 0.32 | — | — | — |
| bitcrusher | 0.160% · 3.42 | 0.018% · 0.37 | 0.022% · 0.48 | — | — | — |
| stereo_width | 0.000% · 0.01 | 0.000% · 0.01 | 0.000% · 0.01 | — | — | — |
| tremolo | 0.040% · 0.86 | 0.041% · 0.89 | 0.040% · 0.86 | — | — | — |
| auto_pan | 0.041% · 0.88 | 0.043% · 0.92 | 0.041% · 0.89 | — | — | — |
| vibrato | 0.158% · 3.38 | 0.109% · 2.34 | 0.158% · 3.39 | — | — | — |
| chorus | 0.336% · 7.09 | 0.308% · 6.62 | 0.336% · 7.20 | 0.486% · 10.19 | 0.444% · 9.43 | 0.490% · 10.44 |
| flanger | 0.161% · 3.37 | 0.134% · 2.87 | 0.156% · 3.36 | 0.163% · 3.36 | 0.136% · 2.86 | 0.156% · 3.34 |
| phaser | 0.296% · 6.23 | 0.290% · 6.18 | 0.289% · 6.24 | 0.368% · 7.70 | 0.344% · 7.33 | 0.348% · 7.41 |
| delay | 0.071% · 1.46 | 0.064% · 1.37 | 0.067% · 1.44 | 0.080% · 1.67 | 0.068% · 1.46 | 0.078% · 1.65 |
| compressor | 0.192% · 4.03 | 0.186% · 4.00 | 0.186% · 4.00 | 0.217% · 4.66 | 0.212% · 4.57 | 0.209% · 4.48 |
| gate | 0.276% · 5.81 | 0.027% · 0.58 | 0.023% · 0.50 | — | — | — |
| limiter | 0.288% · 5.99 | 0.054% · 1.16 | 0.053% · 1.13 | 0.285% · 5.97 | 0.054% · 1.16 | 0.052% · 1.11 |
| pitch_shift | 0.000% · 0.01 | 0.000% · 0.01 | 0.001% · 0.01 | 0.615% · 13.24 | 0.605% · 13.03 | 0.614% · 12.97 |
| vhs_tape_sound | 2.514% · 52.77 | 1.053% · 22.19 | 1.915% · 40.13 | 3.680% · 77.02 | 1.445% · 30.52 | 2.269% · 47.91 |
| datamosh_sound | 0.643% · 13.15 | 0.626% · 13.09 | 0.626% · 13.06 | 0.620% · 13.03 | 0.629% · 12.66 | 0.619% · 12.94 |
| camera_isp_sound | 1.189% · 24.74 | 1.229% · 25.57 | 1.226% · 25.55 | 1.301% · 27.14 | 1.313% · 27.35 | 1.522% · 31.71 |

Reading the table: VHS Tape's sound 52.8 → 22.2 at its defaults, 77 → 30.5 heavy (of that, the AVX2
path gives 40 → 22: its FM loop runs side by side; the rest is the removed per-sample work, which the
baseline path keeps too). Gate 5.8 → 0.6, limiter 6.0 → 1.2, distortion 4.0 → 0.35, bitcrusher 3.4 →
0.4 on both paths. Vibrato, chorus, flanger, delay, EQ heavy: 7–65 % less on the AVX2 path. Camera ISP's
sound, Datamosh's sound, pitch, the phaser, the compressor, Slowed+Reverb: unchanged — their cost is the
simulation or the effect itself (below). Filter's 0.39 → 0.52 is noise at 0.02 % of a core.

His project (`haski-relinked.kt`, copied to the scratch folder, release):
- `kt-cli fx-cost --project … --clip 2133` — his event with Slowed+Reverb + Camera ISP sound + VHS
  Tape sound at his settings (NTSC, EP, 123 dropouts/s of 60 µs): the chain 83.7 → 51.4 reference
  kernels, 3.9 → 2.5 % of a core; VHS Tape's sound alone 54.7 → 23.2.
- `kt-cli probe … 4632 4652 --time` (that event's range, warm cache): the audio render 881 → 597 ms
  for 20 s (4.4 → 3.0 % of real time), the loop 3500 → 2493 ms for 80 s.
- His `load-report.txt` during playback shows the `audio-render` thread at 2–6 % of a core; the cores
  that ran flat out were the UI thread and prefetch (G1, G5, G7: `ui-frame-cost`, `input-without-work`)
  and the renderer builds (`cached-range-plays`). The effects' cost was real and is now lower, but on
  his machine it was not the thing that made the program unusable — the report says so plainly.

## Decisions

- **Same bytes, everywhere.** The owner: "мы сейчас не трогаем vhs-и всякие фильтры по существу". No
  optimisation here moves a sample: t973c's table and t1981's (printed at db6df6eb before any change)
  hold on the AVX2 and the baseline path. Each transformation is the same operations on the same values:
  a loop split into passes adds its sums in the original order; a table or a memo holds what the
  expression gave for the same bits; a cache is keyed by everything its answer depends on (the dropout
  cells by the cell and the law, rebuilt when a row changes); a deferred `tanh` uses the drive the
  sample was recorded at.
- **The dispatch is a second compile of the same text**, not hand-written SIMD: nothing to keep
  bit-equal by hand, every kernel gets it with one line, and a new kernel gets it the same way. AVX2
  without FMA: Rust never contracts `a * b + c`, never reorders a float sum.
- **The guard is a ratio.** CLAUDE.md, `test_deadline`: a wall-clock limit is no check. t1980 counts each
  kind in units of a reference kernel measured alternately in the same run, best of three; the bounds
  are twice what the test profile measured (the test profile measures within a third of release).

## Tried and dropped (no measurable gain, so not kept)

- The AAC codec's `encode`/`decode` and the noise suppressor's frame through `fast!`, pitch's grain
  functions inlined into its body: Camera ISP 25 → 23–25, Datamosh 13 → 12.6, pitch 13 → 13.
- The suppressor's 512-sine window as a table and its buffers kept: no change (libm's `sin` is cheap
  next to the FFT).
- The rate control's residual `spec − p` taken once instead of per probe: the compiler already did it.

## The class, walked (the owner's rule)

Every audio effect's process path (21 kinds, `FxInstance::all()`):

| kind | what its block does per sample | found | done |
|---|---|---|---|
| gain, stereo_width | a multiply / a mid-side mix | nothing | dispatch |
| filter, eq | biquads, coefficients per block | nothing | dispatch (EQ heavy 2.6 → 0.9) |
| distortion | `pow` (drive), `pow`+`exp` (tone) per sample, `tanh` | pure math redone at rest | `Memo` ×2 |
| bitcrusher | `pow` (steps), `floor` (hold grid) | both | `Memo`, dispatch |
| tremolo, auto_pan | an LFO: an `f64` `sin` per sample | the LFO's `sin` is the effect (its phase moves every sample) | dispatch |
| vibrato, chorus, flanger | LFO `sin`, a fractional delay read (`floor`) | `floor` via PLT | dispatch |
| phaser | LFO `sin`, `pow`, `tan` per sample (the allpass frequency follows the LFO) | per-sample coefficients ARE its sweep; a per-block update would move bytes | not changed: found, not fixed |
| delay | a fractional read | `floor` via PLT | dispatch |
| compressor | `pow` (make-up), `log10`+`pow` of the envelope | make-up redone at rest; the envelope's two are the signal | `Memo` (make-up) |
| gate | two `pow` of the threshold | redone at rest | `Memo` |
| limiter | two `pow` (threshold, ceiling); a monotone-queue window max (O(1)) | redone at rest | `Memo` |
| pitch_shift | WSOLA grains / a phase vocoder per hop | the work is the effect; no per-sample library call left in its body | dispatch |
| slowed_reverb | freeverb combs/allpasses; the warp is the host's sinc (kt-fx) | nothing | dispatch |
| vhs_tape_sound | 8× FM per channel, the linear track, the compander | all of form 1 and 2 above | see "What was done" |
| datamosh_sound | the AAC-like codec per 1024 samples, the picture's schedule per frame | the codec's rate-control bisection and FFT are the simulation | dispatch |
| camera_isp_sound | port/capsule filters, AGC, a 512-point STFT suppressor, the device codec | the STFT and the codec are the simulation | dispatch |

The rest of the path, beyond the kernels:

- **Bypassed slots** — `ClipEvent::active_fx` drops them before the engine; no node, no work.
- **Video slots in an audio chain** — `AudioNode::build` gives them no node; no work.
- **Silence and tails** — a node runs on silence only inside `tail()` after the event (the gate in
  `produce` zeroes a whole chunk in the tail without the warp, R.43). VHS Tape's and the microphone's
  noise on silence is their sound (tape hiss, capsule self-noise), not idle work; neither declares a
  tail, so nothing runs past the event.
- **Parameter smoothing** — `SmoothedParam::tick` per sample is a multiply-add; the cost was what was
  computed FROM it (form 2), now memoised.
- **Automation** — `set_targets` once per 1024-frame chunk and only with curves (R.47); VHS Tape's sound
  rebuilt its passport (a vector of 40 rows) there per chunk — now once per process. The camera
  microphone's `set_targets` looks rows up by name per chunk (17 lookups, 47 times a second): small,
  left. The LFO's phase with a `rate_hz` curve walks the curve's keys per sample (`param_integral`):
  O(keys), a handful of comparisons for the usual 2–10 keys — found, not fixed (a cursor would make it
  O(1)).
- **The node graph per block** — `ClipProcessor::produce` dispatches one virtual call per node per
  1024-frame chunk; the twin and the fold copy a chunk only with a pitch's echo or a loop.
- **The paired device's shared state** — VHS Tape's sound derives the deck per tape frame
  (`Deck::words`, cached by frame) and per picture line (`line_read`, cached by line): removing the line
  reads entirely changed nothing measurable. The stream's sound builds the picture's schedule per audio
  frame (47 a second, with a small `HashMap`): small. The microphone reads only the sensor's seed.
- **An instance per placement** — the store keeps a processor per event (invariant 3); the
  `cached-range-plays` change made a reel placement's inner states born at the first read. Not this
  change's.

## Found, not fixed

- The phaser recomputes its allpass coefficient (`sin`, `pow`, `tan`) every sample: 6.2 reference
  kernels. A per-block (or per-16-samples) update would cut it several times but moves bytes and the
  sweep's smoothness — a sound decision, the owner's ("не трогаем по существу").
- The compressor's `log10` of the envelope per sample is skippable below the threshold (exactly, with a
  margin argument); 4 reference kernels, left for want of an exhaustive test.
- The LFO's phase walks the rate curve's keys per sample (above).
- Camera ISP's and Datamosh's sound are the AAC-like codec's rate control (up to eight quantise +
  count passes per 1024-sample frame) and the suppressor's FFT: their cost is the simulation as built.
