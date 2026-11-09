# Design: kernel-specialization

## Context

`docs/engine-audit.md` §3 took JIT tier T2 for the audio kernels: one Rust source, several monomorphic
variants, the variant chosen at run time by state ("a parameter standing still, a weight of exactly 1, no
curve") or by instruction set. audio-fx-cost had already built two of its tools: `cpu::fast!` (an AVX2 copy
of a kernel's block) and `dsp::Memo` (a bits-keyed last value). This change applies the state half across
the class and adds the missing instruction-set copies to the analysis paths.

## The class and its mechanism

**A kernel does per-sample or per-block work whose inputs have not changed.** The inputs of a re-tune, of
a level conversion, of a crossfade weight are often bit-identical from one sample (or block) to the next;
the kernel recomputes them anyway because it was written as one loop for the moving case.

## Decisions

### D1. "Stands still" is a fixed point, not an equality with the target

`SmoothedParam::tick` is `c += (t − c)·α`, a pure function of `(c, t, α)`. When one tick leaves `c`'s bits
unchanged, every later tick does too, until `t` moves. That is what `still()` checks — a tick computed and
not committed. `c == t` would be wrong both ways: a glide stops a few ulps short of its target when the
step rounds to nothing (it is still, but `c ≠ t`), and `−0` with target `−0` ticks to `+0` (equal, but not
still). `advance(n)` stops ticking at the first tick that changes nothing. No new field: the struct, its
`Clone` and every `fork` stay as they were.

### D2. A skipped re-tune is keyed by the bits of everything the coefficients depend on

`Biquad::tune` is a pure function of `(kind, f, q, gain)`. The filter keeps `Biquad::tune_key` of its last
tune, the EQ the bits of its seven values; equal bits → the coefficients already in the biquads are the
ones the tune would write. `reset()` zeroes the filter state, never the coefficients, so the key survives
a seek; `fork` clones both.

### D3. The compressor below its threshold: a safe level, not a comparison in dB

Below threshold the general path computes `over = max(20·log10(env) − thr, 0) = +0`, so `gr = +0` for every
ratio ≥ 1, `10^(−0/20) = 1` exactly (pow of a zero exponent), and the gain is the make-up gain to the bit.
Deciding "below" without the `log10` needs a level in the envelope's own domain:
`below_threshold(thr) = 10^(thr/20) · 0.999`, memoized per threshold. The 0.0087 dB margin is ~1000× the
rounding of the `f32` functions, so `env < level` implies `over = +0` without relying on the monotonicity of
`flog10` near the edge; near and above the threshold the general formula runs. t2172 walks 3 200
thresholds (−100…+20 dB plus random ones) × 8 192 envelopes just under and spread below each level. A NaN
threshold gives a NaN level, the comparison fails, the general path runs.

Ducking since R.65 IS this compressor with a key: a silent key decays the envelope under the level, so the
"no reduction" stretches of a ducked track take the skip.

### D4. The mixdown in regime runs

`fade_weight · overlap_at · gain` has a finite set of regimes per event: before a fade-in (`rise(0)`), in a
fade window (ramp), outside (1); inside an overlaid window (1), under 0/1, 2 (ramp) or ≥ 3 (1/n)
overlapping events; past the FX tail edge (1). `weight_run` finds the next boundary of any regime after
`f` (fade and overlaid windows, the lane cover's bounds, the tail edge) and says whether the regime is a
constant. A constant run takes the weight ONCE, by the same expression, at its first frame — the value
every frame of the run would compute — and multiplies; a ramp run keeps the per-frame expression. A
`debug_assert` compares the weight at the run's last frame. A degenerate fade window (`w1 ≤ w0`) counts as
a ramp. Both mixdown paths (plain events, events with effects) use it; the order of the f32 sums is
untouched.

### D5. Analysis at the CPU's level

The loudness meter and `PeakAcc` run their bodies through `fast!` (the AVX2 copy; same bytes — no FMA, no
reordered sums). The true peak's tail is a doubled ring: each sample is written at `pos` and `pos + 8`,
so the last eight samples, oldest first, are one slice — the same values in the same order as the rotated
array. The interpolator table is fetched once per `feed`, not through a `OnceLock` per tap. `PeakAcc` folds
a block's run of frames into a fixed-size array; integer min/max give the same values in any grouping.

### D6. One reference switch

`cpu::with_general` (a thread-local, read once per block) runs every "stands still" variant's general
path — the code as it was. Each specialized path is held to it AND to digests printed on the tree before
the change (t2171, t2173–t2175); a deliberate fault in the constant runs or in the compressor skip turned
both red (checked by mutation while writing them). The memo-style changes that are exact by construction
and need no switch (distortion's compensation inside its drive memo, the doubled rings) are held by the
stored digests alone.

### D7. ARM and the frozen path

No intrinsic was added. `fast!` is x86-only by construction (other architectures run as built; aarch64's
baseline has NEON). The new code calls only `FrozenMath` functions and IEEE-exact operations (`sqrt`, `/`).

## The class walked (every place where it can act the same way)

| Where | What was found | Verdict |
|---|---|---|
| filter | biquad re-tune (`sin_cos`, `pow`) per 128 frames at rest | **taken** (D2): 0.52 → 0.34 ref (release, fx-cost) |
| EQ | six re-tunes per 128 frames at rest | **taken** (D2): heavy 0.90 → 0.55 ref; at rest 40 % of the general path (t2176) |
| distortion | `1/√g` per sample; three ticks + two memo checks per sample at rest | **taken**: compensation in the drive memo, constant block: 0.34 → 0.30 |
| bitcrusher | three ticks per sample at rest | **taken** (constant block): 0.37 → 0.27; the hold-grid `floor` per sample is the position's function, stays |
| stereo width | a tick per frame at rest | **taken** (constant block); width 1 already passed through |
| gain | automated gain/pan tick per frame on a flat curve | **taken** (constant multiply); the static path already existed |
| gate | detector, hysteresis, ramps per sample: state | checked, nothing (threshold memo since audio-fx-cost) |
| limiter | sliding max per sample: state; `ceil/thr` division per sample | checked; the division is small, not taken |
| compressor | `log10`+`pow` per sample below threshold (row 129) | **taken** (D3): quiet material 1.46 → 0.58 ref (release, t2176); above threshold the formula is the sound |
| tremolo, auto-pan, vibrato, chorus, flanger | LFO moves every sample; phase by prefix integral (compiled-curves) | checked, nothing stands still |
| phaser | per-sample coefficients (row 128) | the owner's decision, untouched |
| pitch shift | per-grain work; the seam list's `remove(0)` per seam | checked, nothing per sample |
| delay | coefficient per block; fractional read per sample | checked, nothing |
| slowed reverb | freeverb state per sample; plain path exists | checked, nothing |
| VHS sound | per-block constants hoisted by audio-fx-cost; per sample the device's state | checked, nothing |
| Datamosh sound | per-frame codec work; per-block buffer push/drain | checked, nothing |
| camera microphone | two 63-sample history shifts per sample on the decimating codecs; `log10(256)` per kept µ-law sample | **taken**: doubled rings (−2 % of the effect at codec 2); the µ-law constant is small, not taken |
| mixdown, plain and with effects | `fade_weight` + `overlap_at` (binary search) per sample at weight 1 | **taken** (D4) |
| crossfades, fades, overlaid transitions | the same weights | **taken** (D4) |
| track stages | the same kernels through `TrackProcessor` | covered by the kernels |
| kept final mix | a replay copy | checked, nothing |
| meters (`Meters::fold`) | a max per chunk, ~47 calls/s | checked, nothing measurable |
| LUFS, true peak | scalar `f64` at the baseline level, a rotate per sample, a `OnceLock` per tap | **taken** (D5): 50 → 25 ms per minute of audio (release) |
| peaks pyramid | scalar fold at the baseline level | **taken** (D5): 7.2 → 4.1 ms per minute |
| sync | works on the pyramid; a full search only at a level of 16–31 blocks, then ±4 | checked, nothing |
| ducking | the sidechain compressor (R.65) | covered by D3 |

## Measured

Release, `kt-cli fx-cost --secs 5`, ratio to the reference kernel, base d5d5b6b9 against this change in the
same session: filter 0.52 → 0.34, EQ heavy 0.90 → 0.55, EQ default 0.18 → 0.01, distortion 0.34 → 0.30,
bitcrusher 0.37 → 0.27; every other kind within noise (compressor 4.06 at defaults: its stimulus is above
the threshold). The owner's chain (`--project haski-relinked.kt --clip 2133`: slowed reverb, VHS sound,
Datamosh sound) has no kernel that stands still: `probe 4632 4652 --time` 460 → 449 ms per 20 s, loop
1815 → 1819 ms per 80 s — no change. The engine-audit's guess (51 → ~40 reference kernels on his chain)
does not hold for this chain: its cost is the two simulations, whose per-sample work is the device.

## Risks

- A future edit of a kernel's general loop that forgets its constant twin: t2171 and t2176 catch a
  divergence in bytes and a variant that is no longer taken.
- `still()` costs one tick's arithmetic; it is read per block, never per sample.
