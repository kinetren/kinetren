# Design: per-sample-costs

## Context

Rows 128 and 350–363 of `docs/found-defects.md` were left by audio-fx-cost, kernel-specialization and
local-cost-queue's class walks. All audio is byte for byte (invariant 11, `kt_plugin_api::math` on the
frozen paths); the audio render's hot path allocates nothing and takes no lock; video stays within
`FrameTol`, and here every GPU change was held EXACT on the device.

The mechanism (the class): an inner loop recomputes per element what is constant over a larger unit
(a block, a frame, a cell, a channel pair), allocates per unit, or repeats one channel's work for the
other.

## Decisions

### D1. A cheaper frozen function is the function to the bit, proved on every float (row 128)

Phaser's sweep is per sample by the owner's sound decision; per block would change the sound. What
costs is `10f32.fpowf(ph·d)`: libm's `pow` is 22 ns, `exp` 3 ns (release, this machine).
`math::exp10f(y)` takes `exp(y·ln 10)` in `f64` and rounds once; the product's rounding stays within
2⁶ `f64` ulps for `|y| ≤ 37`, `pow`'s within one, so both round to the same `f32` unless the value is
within 2¹⁰ ulps of a rounding boundary (the low 29 bits near the half) — there, and outside
`|y| ≤ 37`, `powf` answers. t2682 compares it with `powf(10, y)` on all 2³² floats (10 s). The same
call serves the compressor's per-sample gain above the threshold and the distortion's drive.
`tan`/`sin` of the sweep stay: 4–5 ns each, no exact cheaper form.
Rejected: a `log10` twin for the compressor's detector (log10 8.5 ns, log 5.2 ns) — a second proof for
3 ns a sample above the threshold only; written down here.

### D2. Kept buffers, never `to_vec` per chunk (rows 350–352)

- The warp keeps its interleaved input, planar input and planar output; rubato's
  `process_into_buffer` is what `process` called after allocating — the same arithmetic.
- `Pass::run` copies the dry side into a buffer `ClipProcessor::scratch` keeps (main stream and twin);
  the twin's chunk the same. They are handed back after the chunk.
- `FxStateStore::pool()`: an event's chain output, the laps' sum and ring, a stage's sum, the key
  signals and the active-event lists are taken and given back within one render call (bounded,
  32 of each). The store is the render thread's own (invariant 2), passed `&mut` already; a
  thread-local would break on a nested reel's re-entry.
- The reel maps (`nest_prerolls`, `nest_keys`) are pure functions of the build: a `OnceLock` on the
  renderer (shared across threads, so no `Cell`). The "heard" set is the pooled index list.
- A sub-mix burst the ring let go and nobody holds is filled again (`NestStream::burst`); its samples
  are zeroed as a new one's are. A ring grows to its cap (block + pre-roll, a project function) first.
- `ClipProcessor::source_offset` (the warp map's reading at a seam) laid its segments out per call: a
  one-entry memo keyed by (local, rate bits, clip length), emptied when the curves or the chain change.
  A loop wraps to the same frame every lap. Found by t2684, not in the rows.
t2684 counts allocations of the render thread over 4 s of warm bursts, linear and looped, on a project
with every path: zero (the base tree: 1 509 linear, 32 looped).

### D3. Per-sample constants once, the same values (rows 353–360)

- 353: after decimation by m the anti-image FIR's history holds the literal `+0.0` between kept
  samples. Summing only the kept taps (polyphase, slot j holds frame n − (HIST − 1 − j)) gives the
  same non-zero sum (adding ±0 to a non-zero sum is exact); the forms can differ only in the sign of
  an exact zero, so an exact zero takes the full sum.
- 354: the rows are written in place; `Deck::renew` is `Deck::new` without a new vector; the mic's
  curved rows are looked up by index once.
- 355: `Biquad::tune_like` copies coefficients; the same inputs give the same bits.
- 356: `wrap_next` (compare-and-reset) for the combs, allpasses, delay lines, the pre-delay (every
  channel's line has one length, so `pre_pos` wraps at it), the mic's delay and the dry ring;
  `read_frac` truncates (`d ≥ 1`, so truncation is the floor, and the SSE2 build's `floor` is a
  libc call) and wraps by one add. A NaN delay behaves as before (index 0).
- 357: the dropout mask of a frame's sub-samples once, before the channel loop. Divisions by
  per-block constants as reciprocals would move bytes: not done.
- 358: the gust envelope `0.02·w³·gust` once per sample for both channels (the product kept in its
  order), the two level hashes once per 0.4 s cell.
- 359: the vocoder's analysis writes every sample of `time`, so the zero fill goes; `window[i]·(1/N)`
  is a table of the same products. Its per-bin divisions as reciprocals would move bytes: not done.
- 360: row k of pink/brown noise is hashed when `n >> k` moves; the sum keeps its order.

### D4. GPU: what the device repeats per fragment (rows 361–363)

- 361: the composite's `samp` asked `textureDimensions` five times a tap; `take_dims` takes both sides
  once a fragment, in the transition branches only (the plain composite asks none). A/B on llvmpipe:
  22 kinds × 3 phases hash-identical; whip 15.3 vs 14.1 ms, zoom blur 17.0 vs 18.8 ms a 1080p frame —
  within noise there (llvmpipe's query is a constant read); a card's is an instruction.
- 362: with defocus, s00 decoded 17 taps × 3 channels through `pow`. A 256×1 pass (`s12_curve`) writes
  `decode(x, curve)` as the f32's bytes, with `decode` moved into `camera_common.wgsl` so both passes
  compile one text; s00 reads it (u1.y = 1, T2). `cpu::with_general` keeps the per-tap form. EXACT on
  llvmpipe for every curve, two radii, opaque/alpha, K 1 and 2 (t2683). `kt-cli pass-cost`, 1080p,
  blur 4, minima of 9 blocks: sRGB s00 31.6 → 28.5 ms, BT.709 44.2 → 26.6, gamma 2.2 44.0 → 27.0, the
  table pass 0.4 ms. Linear sources and no defocus take no table.
- 363: replacing the per-pixel `sin(π·k)` of whip and zoom blur and spin's two rotation trig calls by
  constants changed a 1080p frame by nothing measurable (whip 14.7 → 13.9, zoom blur 15.4 → 15.8,
  spin 6.1 → 6.2 ms — noise). Moving them into uniforms would move bytes (device trig vs frozen CPU
  math) for nothing: not taken. Camera's s01/s08 `sqrt(hd2)`, `lens_fit`/`isp_crop` and s06's dye
  ratio are one scalar op among dozens a pixel — same verdict by the same measure.

## Measurements (release, this machine shared; ratios to the reference kernel, `kt-cli fx-cost`)

| kind | default before → after | heavy before → after |
|---|---|---|
| phaser | 6.31 → 4.03 | 7.54 → 5.08 |
| compressor | 4.06 → 2.19 | 4.63 → 2.91 |
| chorus | 6.78 → 5.46 | 9.73 → 7.88 |
| flanger | 3.02 → 2.18 | 3.02 → 2.17 |
| vibrato | 2.40 → 2.03 | — |
| delay | 1.36 → 0.98 | 1.44 → 0.98 |
| slowed_reverb | 1.76 → 1.68 | 1.92 → 1.82 |
| vhs_tape_sound | 22.83 → 20.11 | 31.10 → 28.50 |
| camera_isp_sound | 22.52 → 21.92 | 24.67 → 23.84 |
| the rest | unchanged | unchanged |

`kt-cli probe heavy.kt 0 104 --time` (the heavy showcase, 1 357 events): linear 849/777 ms base,
731/873 ms new for 104 s — 1 % of real time either way; the showcase's sound is light and the
difference is inside the run-to-run spread.

Processor load while playing the heavy showcase (`kt-cli preview heavy.kt 20 30 --size 960x540`, release,
llvmpipe, two runs each, interleaved): base 1 122 % / 1 154 % of a core, V-fps 1.0 / 1.0; new 1 167 % /
1 204 %, V-fps 2.3 / 2.1 — the software rasterizer takes the machine either way, and the frames shown
doubled (the event under Camera ISP with lens blur 1.2 takes the table). Audio 484 bursts live, no
underrun in any run. The live `idle` scenario on the same project: a pause 1 % of a core, the effects
window paused 1 %, the mouse over the preview 317 %, over the effects window 193 % (llvmpipe drawing the
interface); `heavy_loop`: from lap 6 every frame and burst from what is kept.

## Class walk (what was checked)

Audio kinds (`process` and what it calls per sample/chunk):
- gain, stereo_width, bitcrusher (memoized steps), gate, limiter (memos; its per-sample ratio stays,
  local-cost-queue D-limiter), distortion (now `exp10f` on the glide path) — clean.
- filter, eq — tune once per channel pair (355); the re-tune key from kernel-specialization stays.
- tremolo, auto_pan, vibrato — one LFO `sin` a sample, the substance; vibrato/chorus/flanger read
  through `DelayLine` (356).
- chorus — one `sin` per voice a sample (distinct phases): the substance.
- phaser — D1. compressor — D1; its `log10` above the threshold stays (D1).
- delay — `DelayLine` (356); its damping `fexp` is per block.
- slowed_reverb — Freeverb's 24 divisions a frame (356), the warp's buffers (350).
- pitch_shift — WSOLA (local-cost-queue fixed it), vocoder (359); `ring_at`/`acc_at` index by constant
  powers of two (a mask).
- vhs_tape_sound — 354, 357; `% RING`/`% TAPS` are constants (the compiler's multiply), left.
- camera_isp_sound — 353, 354, 356 (its `Delay`), 358; the white wind and hiss hashes are per sample
  by nature.
- datamosh_sound — allocates only on a jump (its input grid restarts); codec scratch done by
  local-cost-queue.
- generators-audio — noise (360); tone/sweep/beep one `sin` a sample.
- layer_mix, the dry ring (356), the twin (351), the loudness meter (`% TP_TAPS` constant) — clean.
- the mixdown (352) and the warp (350); a seek's warm-up scratch is per seek, left.
Shaders:
- composite (361); geom_fused, grid_tile, layer_mix, mask, resample, mirror, premul/unpremul — no
  per-fragment query beyond one `textureDimensions`, no transcendental of a uniform.
- Camera ISP s00–s11 — `begin()` caches sizes (gpu-pass-cost); s00 (362); s02's `sin` is per pixel
  noise; s06's `pow(1 + k·r²)` depends on the pixel's radius; s01/s08/s06 per-frame scalars (363).
- VHS Tape — `tape_dims()` caches sizes; `up.wgsl` one query a fragment; `lt.wgsl`'s per-tap cosine
  runs on the per-line table pass (a column of lines, not pixels).
- distort warps (twirl, ripple, kaleidoscope) — trig of the pixel's own angle/radius: the substance.
- radial_blur — measured by local-cost-queue D2 (a table costs more).
- stylize, color, blur, codec, generators — no per-fragment query or uniform-only transcendental found.
- transitions (32 kinds) — 363; `samp` (361).

The callers walk (`grep` of the touched functions across `src/`): `consumed_abs` (kt-fx windows, the
seam memo only), `Freeverb::process` (slowed_reverb only), `DelayLine::read_frac` (vibrato, chorus,
flanger, delay), `Biquad::tune` (tone family; `mag_db` reads the coefficients `tune_like` copies),
`active_clips` (render, sum_lane, nest release; the raw reverse path keeps its own list — reverse
preview, no FX state, left), `NestStream::push` (the producer only), `decode` (s00, s12),
`t_dims` (composite only). `exp10f` is `powf(10, ·)` everywhere it replaced it.

## Risks

- A future edit that writes a non-zero between kept samples of the mic's interpolator history breaks
  353's premise; t2315/t2171 digests (codecs 1, 2) catch it.
- The pool's buffers are sized by the request each time; a caller forgetting to give one back only
  allocates again (t2684 catches it).
