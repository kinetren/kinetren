# Design: vhs-tape-simulation

## Context

The physical chain and its numbers are in the note `docs/features/fx/simulate/vhs-tape-simulation.md`
(F-FX-009, steps 1–16). This document decides how that chain runs inside our engine:

- a video kind is a barrier plugin: a CPU `frame` kernel (the byte reference), an optional SIMD twin
  behind the host's one switch, a GPU pass graph (`kt_plugin_api::barrier`);
- GPU passes are fragment passes over `Rgba8Uint` textures; until now a plugin pass always rendered
  at the frame size (`kt-gpu` engine, `FxOp::PluginBarrier`), and a pass gets 16 uniform words;
- a kernel receives eight cooked numbers (`Cooked = [f64; 8]`);
- invariant 11: no platform libm on a frozen path (t1066), the grain pinned to `burst_index`.

## Goals / Non-Goals

Goals: the look comes out of the modelled process, with physical parameters; CPU is the reference,
the GPU within a measured tolerance; deterministic; every existing barrier keeps its bytes.

Non-goals: decoding real RF (vhs-decode does it); sync pulses, vertical interval, CTL track and
servo dynamics as signals; the audio tracks; nonlinear emphasis; S-VHS/ED modes (the standard table
has room for them).

## Decisions

### D1. A signal grid of the STANDARD, not of the frame

The simulation runs on a fixed grid per standard, sampled at **4 fsc**:

| | fs | samples per line W (active A + margins M) | lines H |
|---|---|---|---|
| PAL | 17.734475 MHz | 1135 = 184 (sync + back porch) + 922 (52 µs) + 29 (front porch) | 576 |
| NTSC | 14.318182 MHz | 910 = 135 + 754 (52.66 µs) + 21 | 480 |

4 fsc carries the FM carrier and its first sidebands (PAL 4.8 MHz white + ~3 MHz video < 8.87 MHz
Nyquist), the colour-under and the chroma; it is the sample rate of digital composite (D2) and of
the capture cards vhs-decode targets in its lower range. The grid is the WHOLE line: the blanking
before the active part is as long as on a real line (10.4 µs PAL), because the de-emphasis reaches
~7 µs back. A first version had 2.9 µs margins and the line-start transient of the demodulator
smeared 4 % into the picture's left edge (t1180 caught it); a real deck never shows that, since its
blanking is long. The blanking holds the black level (no sync pulse, D9).

Why not the frame's pixels: the physics would depend on the preview size (a 640-wide preview would
sample the 4.8 MHz carrier at 12 MHz), and preview ≠ export. Why no sync pulse in the blanking: it
adds work and nothing visible once the time base is modelled per line (D8). Rows are FRAME lines;
row parity is the field, so field A (even rows) is head A's track and field B head B's.

The frame enters through the host's resample to A×H (premultiplied first when it has alpha: the
tape records what a camera saw over black) and leaves through a bilinear read of the grid at the
frame size. A K× frame (`res_scale`) changes nothing in the grid: the look is K-invariant by
construction.

### D2. The stages (one CPU function = one GPU pass)

| # | Stage | Reads | Output (per grid sample) |
|---|---|---|---|
| R | host resample to A×H (+ premultiply) | frame | RGB of the standard grid |
| C | parameter upload (3 passes of 16 words) | — | a 48×1 parameter texture |
| L | line table: head, coverage, gains, noise σ, dropout, ACC, time-base error, the two channel kernels of the row | C | 16 + 2·81 columns × H |
| 1 | record luma: Y (BT.601) → LPF 3 MHz ∗ pre-emphasis (τ₁ 1.3 µs, ×5) → white/dark clip → instantaneous frequency → phase increment | R | u32 increment (DDS) |
| 2 | block sums of 32 increments | 1 | u32 |
| 3 | phase = prefix sum along the line, from a seeded start phase | 2 | u32 phase |
| 4 | write: cos(phase) + colour-under (U/V LPF 0.5 MHz on the rotated 627/629 kHz carrier, PAL V switch), through the tape's saturation in its in-band form (D9) | 3, R | magnetisation m |
| 5 | read: own-track and neighbour-track convolutions with the row's kernels × coverage × head efficiency × dropout envelope + Gaussian noise (+ the switching burst) | 4, L | RF r |
| 6 | FM demodulation: Hilbert FIR → discriminator arg(z_n·z̄_{n−1}) → level; RF envelope | 5 | level, envelope (2×u16) |
| 7 | dropout compensation (the line before in the same field when the envelope is low) → LPF ∗ de-emphasis | 6 | luma |
| 8 | colour-under demodulation with the track's rotation, PAL V switch undone, ACC | 5, L | U, V (2×u16) |
| F | frame: time-base shift per row, bilinear read, the 1 H (NTSC) / 2 H (PAL) comb on chroma, BT.601 back to RGB, opaque | 7, 8, L | the frame |

The DDS phase (stages 1–3) is a **u32 fixed-point** accumulator: wrapping addition is exactly
associative, so the GPU's block-sum prefix and the CPU's running sum give the same phase for the
same increments, and the FM never drifts between backends. The prefix is split into blocks of 32
so a GPU texel reads at most 32 + W/32 values (no 11-pass Hillis–Steele scan, no O(W) loop).

Intermediate values are packed into the four bytes of a texel: an f32 by its bits (stages 3–5, 7),
two u16 fixed-point numbers (6, 8; `round_ties_even` on the CPU = WGSL `round`).

### D3. The channel as integrations, not as a look

The read signal of a row is a convolution with two kernels the line table builds per row:

- **own track**: Lorentzian of half-width a = (d_rec + d_play)/v (spacing loss e^{−2π d f/v}; the
  record head's spacing and the playback head's add) ⊗ a box of the gap transit time g/v (gap loss
  sinc(π g f/v));
- **neighbour track** (other azimuth): Lorentzian of (d_rec,neighbour + d_play) ⊗ a box of
  (g + w·tan 12°)/v, w the strip of the neighbour under the head: the azimuth loss
  sinc(π w tan12° f/v) is exactly the transfer of that box — the integration along the tilted gap.

Each tap is the EXACT bin integral of Lorentzian ⊗ box: a second difference of
J(t) = (t·atan(t/a) − ½a·ln(a² + t²))/π, the antiderivative of the Lorentzian's CDF. 81 taps (±40).
A first version sampled the integrand at 4 points per bin: a gap box narrower than a sample (0.3 µm
is 1.1 samples) then held 2 of the 4 points and the own kernel's DC gain came out 1.35, which
skewed every azimuth ratio by 2.7 dB (t1184 caught it). The second difference cancels in f32 far
from the centre, but there the taps are ~1e-6 and the error is absolute.
The gains are coverages: the overlap of the head [o − w_h/2, o + w_h/2] with the own track, the
neighbours (±1, the other azimuth, the other field's row) and the next-but-one (±2, the same
azimuth, the same row), divided by the SP pitch (the reference signal), times the head's efficiency.
Noise: tape noise ∝ √(width read) × efficiency, amplifier noise constant (70/30 % of the noise at
the reference C/N).

Geometry per row: position along the field u; offset o = tracking + skew·(u − ½) (+ pitch·(u − ½)
for a two-head STILL: the stopped tape lets the scan cross one pitch per field). Head widths: a
two-head drum has one compromise pair at 0.65 × SP pitch; a four-head drum has an SP pair at the SP
pitch and an LP pair at 1.15 × LP pitch used at LP/EP. Four-head STILL reads both fields from track
A with heads of A's azimuth (the second pair) and no crossing — the note marks the pairing as not
confirmed by search; the modelled consequence is the field still without a noise bar.

### D4. Fixed filters are constants of the standard

The record luma kernel (LPF 3 MHz ±12 ∗ pre-emphasis, 61 taps), the playback luma kernel (LPF ∗
de-emphasis 5 τ₁, 140 taps), the chroma LPF (0.5 MHz, 49 taps) and the Hilbert FIR (±15, Blackman)
depend only on the standard. They are designed ONCE in f64 with `kt_plugin_api::math` and rounded to
f32; the CPU uses those values and the WGSL text is generated from the same values (`{:?}` of f32
round-trips exactly), so both executors convolve with bit-identical taps.

### D5. Our own f32 cos/sin/atan2 in the per-sample code

The per-sample functions (the carrier cos/sin in cycles, the discriminator's atan2, the kernel
taps' atan2) are OUR polynomials, written twice (Rust f32, WGSL) as the same sequence of f32 ops.
Reason: WGSL's `atan2`/`cos` accuracy is implementation-defined (up to thousands of ULP), so the
builtin would make the GPU result depend on the vendor; the CPU side must not call std either
(t1066). The f64 design-time code uses `kt_plugin_api::math`.

### D6. A pass at its own size (`Node::PassAt`)

`Node::PassAt { shader, inputs, u, f, w, h }` renders the plugin's pass at `w×h` (both in
1..=MAX_INTER_DIM). `check_graph` tracks the size as for a resample; the graph must still END at the
frame size. The engine acquires the target at that size; the arena prediction is unchanged (one
slot per non-Save node); the TooLarge pre-check counts its size. Rejected: packing the grid into
frame-sized textures (a 640×360 preview has fewer texels than the grid; the layout would differ per
size).

### D7. Raw passport values (`frame_values` / `gpu_values`)

A deck has ~30 physical quantities; `Cooked` holds 8. Two default methods on `BarrierKernel` take
the passport values and, by default, cook them and call `frame`/`gpu` — so every barrier keeps its
bytes and its code. The host (kt-fx `apply_barrier`, kt-gpu `chain_ops`, the registry's graph check)
calls the `_values` forms. Rejected: packing several quantities into one f64 (unreadable, and a
quantisation the passport does not show).

### D8. Time is the only state

A barrier has no memory between frames, and the simulation keeps none: the tape position is the
timeline position. Track index = 2·⌊t·frame_rate_std⌋ + field; tape noise, dropouts (per line a
Bernoulli of rate·T_field/lines, start, length from a counter hash) and the start phase of the FM
are `lowbias32` hashes of (track, row, sample, salt); the time-base error is wow (the drum rate,
25/29.97 Hz) + flutter (5.3 Hz) + a random per-line part, with the slow phases taken in f64 from the
absolute time and uploaded as fractions of a cycle. So any frame is computed alone, in any order,
on either backend.

The neighbour track's CONTENT is the other field of the same frame (a real tape has the field
20 ms older there); exact for still pictures. The ±2 track's content is the same field.

### D9. Simplifications (and why they are harmless or deliberate)

- Linear emphasis only; nonlinear emphasis (SP 0.5/LP 0.1 scaling in vhs-decode) is left out — it
  changes the noise weighting of fine detail, not any mechanism the parameters name.
- The record head's spacing is applied as a read-side loss of the track (it adds into the
  Lorentzian), not as a record-field distortion before saturation.
- The playback equaliser is ideal (the inductive +6 dB/oct of the head is assumed equalised); the
  FM system is insensitive to it, the chroma level is held by ACC.
- No sync, burst or vertical interval: the time-base error is applied per row in the final read;
  the colour-under is demodulated with the reference (the deck's APC locks to the played-back burst),
  so chroma keeps its phase under a jittering time base — the physical behaviour (note, trick 3).
- Tape noise is white at RF (not shaped by the head kernel).
- **The tape's saturation is applied in its IN-BAND form.** A memoryless transfer
  x/(1 + k|x|) on the 4 fsc grid makes the carrier's third harmonic (≈ 13 MHz), which a real head
  never reads, but which on the grid ALIASES back to ≈ 4.8 MHz, beats with the carrier and leaves a
  deterministic error in the luma (measured: it hid the 2 dB LP noise difference, t1189). So the
  stage applies what the transfer does in band: the carrier's first-harmonic gain
  a₁ = (1/π)∮ f(cos φ) cos φ dφ, and the chroma's gain with the carrier as its AC bias
  g = (1/2π)∮ f′(cos φ) dφ (the bias effect proper), the chroma compressed by its own level. Both
  integrals are taken once per frame in f64.
- PAL colour-under is taken as 40.125 fH continuous with the line count; SECAM is not modelled.

### D10. Cost and threads

Per sample the CPU runs ~650 multiply-adds (stage taps 61 + 2·65 + 2·81 + 2·16 + 140 + 2·65). The
CPU kernel splits rows into bands over `std::thread::scope` (each sample is independent, so the
bytes do not depend on the band split). The GPU runs 14 passes plus the resample.

Measured once on a release build (2026-11-01, 32 threads, the machine at load average ≈ 30 from the
other sessions, so these are upper bounds), the grid simulation plus the read-out into 1920×1080:

| | ms per frame |
|---|---|
| PAL, defaults (the first run, kernels designed) | 209 |
| PAL, worn tape, two decks | 91 |
| NTSC, defaults | 65 |

So the CPU path is an export path (≈ 10 frames/s), the GPU path is the preview's. No SIMD twin
yet: the barrier contract makes it optional, and the row bands already use every core. A twin is
a straightforward follow-up — eight neighbouring OUTPUT samples per AVX2 register, each tap sum in
the scalar order without FMA, so the bytes stay the scalar's (t1053 would hold it). The cost does
not depend on the frame size beyond the read-out: the grid is the standard's.

### D11. The panel

The FX window draws a deck face above the parameter grid: a drum circle with two or four heads
coloured by condition (clean/dirty/clogged/chipped from spacing and efficiency), the speed switch,
the tracking knob, the deck count; it is built from egui's painter and the shared widgets and edits
the same passport values (no second store). Secondary: the parameter grid alone is complete.

### D12. A defect the first "destroys" barrier found: the strength mix's alpha on the GPU

No barrier before declared `AlphaRel::Destroys` (edges stopped in R.53). Under a layer strength
the GPU's `FxOp::Mix` passed the chain's END flag as the alpha of both the dry and the wet side,
and the mix did not touch the fold: after a destroying step the dry side was forced opaque and the
mixed layer declared opaque, so the composite dropped its alpha-over (t1052: 86 % of the samples
off). The CPU mixes two frames, each with its own flag, and the result has alpha when either
has. Now `Snapshot` and `Mix` carry the fold at their points (`None` → the chain input's flag),
and a mix sets the fold to dry ∨ wet. For every earlier kind the result is unchanged: without a
destroying step between them the two flags were equal.

## Risks / Trade-offs

- FM demodulation near threshold is chaotic: a 1-ULP difference flips a click. The CPU↔GPU
  tolerance is measured (t1186) and set at ×2 of the measurement as a named `FrameTol` class.
- CPU cost is far above any other effect (hundreds of ms per frame without threads); the GPU path is
  the default in preview.
- Thumbnails render the whole SD simulation; they are cached (R.54) and pinned to `burst_index`.
