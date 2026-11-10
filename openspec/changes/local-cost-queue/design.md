# Design: local-cost-queue

## Context

Row 11 of `docs/engine-audit.md`'s order table: the local M7/M9 costs left after kernel-specialization,
stream-layout-per-build, fused-pointwise-runs, gpu-arbiter and memory-ledger. Each is small and local;
the bar for each is "the same bytes" (invariant 11), held by a test that compares the old form with the
new one, or by digests printed on the tree before.

## Goals / Non-Goals

Goals: remove each listed recomputation or per-unit allocation, measure each before/after on a release
build, pin the bytes. Non-goals: anything that changes a sound or a picture (row 128 Phaser stays the
owner's), cache keeping and eviction (memory-ledger), new architecture.

## Decisions

### D1. Camera ISP's sensor passes at the sensor's size (row 136)

Every pass after the unprocessing computes per SENSOR pixel `(x/K, y/K)` and reads its input at the block's
top-left texel (`at`); only its alpha comes from the source at the frame pixel. So at K > 1 a block's K²
fragments compute one value K² times. The graph now renders those passes as `Node::PassAt` at
⌈frame/K⌉ and leaves only the last pass at the frame's size. Two flag bits of the shared flags word
(K needs bits 0–2 of its eight) tell a pass its grids: bit 6 "the input is on the sensor grid" (`at`
reads texel (sx, sy) directly, the sensor size is the texture's), bit 7 "the output is on the sensor grid"
(the fragment IS the sensor pixel; its alpha is read at the block's top-left frame pixel, the texel the
frame-grid form's next pass would have read). Integer addressing only: the same values feed the same
arithmetic, so the bytes are equal (t2310 compares the forms EXACT on the device). A process-wide switch
`fx_video_camera::set_sensor_grid(false)` keeps the old form for the test and `kt-cli pass-cost`.

Rejected: rendering the sensor passes at the sensor size and resampling at the end with the host's
`Resample` — a bilinear resample changes the bytes (and the alpha path).

### D2. Radial blur's spin: no table (row 137) — measured, rejected

The per-pixel `sin`/`cos` are 4 ms of the 26 ms spin pass at 1080p on llvmpipe (zoom mode, the same 64
texel loads a pixel and no trig: 22.4 ms; `kt-cli pass-cost`, release, minima of seven). The pass is
its bilinear loads.
- A table made on the device (a 32×1 word pass, read per tap from `T1`) kept the bytes (checked EXACT on
  the device) and cost MORE: 26.2 → 27.4 + 0.4 ms. Two loads per tap cost more than the trig they
  replace; on a card `sin`/`cos` are special-function units, cheaper still against a load.
- A table in the uniform (computed on the CPU) cannot keep the bytes: the device's `sin` is not the
  frozen one, and the bilinear weights move at rounding edges. The barrier uniform has no room for 32
  numbers either; the 1024-byte layout belongs to geometry and runs.
Row 137 closes as "not the cost"; the pass stays as it was.

### D3. One uniform write per submission, bind groups reused (row 138)

Each pass site wrote its uniform with `queue.write_buffer` and created a bind group: 2.2 µs and 6.7 µs a
pass on the composing thread (llvmpipe, release, a temporary timer in `pass_which`). Writes are staged in
a frame-local byte vector covering the arena's used slots and flushed once before each submission
(`cut_point`'s piece submit and every frame's own; `binds::Staged`). Bind groups go into a map keyed by
(layout, the views) — `wgpu::TextureView` is `Hash + Eq` by id — kept per engine (`binds::BindCache`),
emptied of entries the previous frame did not use at each top-level frame, and bounded (256): a texture
its owner released lives at most until the next frame starts. The arena buffer is bound with a dynamic
offset, so its slot is not in the key; a new arena empties the map. All four pass sites use it (ordinary,
display, geometry, point-wise run). `GpuFxEngine::per_pass_binds(true)` keeps the old form for t2313 and
`kt-cli pass-cost --form binds`.

**Measured** (release, llvmpipe, a two-layer chain of eleven effects, minima of seven blocks): the
composing thread's CPU time a frame 0.66 → 0.47 ms at 640×360 and 1.62 → 1.29 ms at 1080p (the readback
included); the device time does not change; the same bytes. In a steady run three of four bind groups
come back (t2314: 648 reused, 217 made over 24 moments; the misses are each moment's new uploads).

### D1 measured

`kt-cli pass-cost --fx camera_isp --k 2` (release, llvmpipe, a 1920×1080 frame behind K = 2): the
camera's passes 388.8 → 120.7 ms, the frame 415.7 → 143.5 ms; the unprocessing (the frame's grid) and the
last pass do not change; the same bytes. t2311 holds ≤ 60 % (0.40 in the test profile).

### D4. Audio (rows 210, 211, 311–314)

- Limiter (row 210): a two-key memo of `ceil/thr` measured 1.17 → 1.20 reference kernels (`kt-cli
  fx-cost`, release, three interleaved runs): the division is off the loop's critical path (the
  monotone queue and the gain recursion are), and the memo's compare costs more. Kept as it was.
- Camera mic: `(1+µ).flog10()` once (a `OnceLock`), the 128 levels `(256^(i/127) − 1)` a table indexed by
  the rounded code (the `q` the formula takes is exactly `i/127`); the suppressor's sqrt-Hann window a
  static table, `re`/`im` kept in the state, `min_now` refilled instead of reallocated; rows by a static
  index table built once.
- VHS sound: `design_linear`'s 33×256 cosines and 33 window values a static table (the same `fcos` of the
  same arguments); the dropout cache keyed by its `DropLaw` bits, advanced by dropping cells that left the
  window and adding the new ones (the set of starts is the same, `any` does not depend on order). A new
  `Deck` per curved chunk stays: it is a copy of 39 numbers, and the words and line reads it drops are
  recomputed at the next tape frame anyway (once per 40 ms, about two chunks).
- Pitch WSOLA: a static Hann table; candidate offsets generated in the same order without a `Vec`.
- Codec: stack arrays in `dct4`/`mdct`/`imdct`; no `p.clone()` (disjoint field borrows); a predictor
  reset zeroes in place (it allocated four vectors at every random-access point); the payload allocated
  once at the size the rate control measured; callers pass `&pkt.payload.v`; Datamosh keeps a bits
  buffer and its decoded frames (the copy for its flips is a `memcpy` into kept capacity).

**Measured** (release, `local_cost_print` in `kt-engine/tests/local_cost.rs`: a 4 s event through the
renderer against the reference kernel on the same frames, minima of nine, before/after binaries run
interleaved twice; the renderer's own cost is in every figure, ~2.3 reference kernels):

| case | before | after |
|---|---|---|
| VHS Tape sound, curved gap/spacing/bias | 42.6–43.2 | 36.0–36.3 |
| VHS Tape sound, curved dropouts | 34.6–35.2 | 34.9–35.1 |
| camera mic, AAC / 16 kHz / µ-law / curved | 28.4–29.4 / 27.3–27.6 / 29.1 / 28.9–29.1 | 25.5 / 24.2–24.6 / 25.1–25.2 / 25.5–25.9 |
| Pitch WSOLA, fixed / curved | 19.9 / 20.0–20.1 | 19.0–19.3 / 18.8–19.0 |
| Datamosh sound, predicting | 7.2–7.3 | 7.2–7.3 |
| limiter, curved | 9.3–9.4 | 9.4 |

The dropout cache's stepping does not show at 40 dropouts a second: a walk of 33 cells once a
millisecond is ~1 % of the sound's work. Datamosh's clones were small against its MDCT. The heavy
showcase's audio (no tape or camera sound halves in it) renders 104 s in 458 ms either way
(`kt-cli probe … --time`): nothing of this row reaches it.

### D5. Row 118 — nothing local left on the heavy showcase

`kt-cli preview heavy.kt 20 30 --size 640x360` (release, footage in /dev/shm, the base and this change's
binaries run interleaved twice): V-fps 1.0–2.5 either way, ~420–530 of ~450–550 composites incomplete,
`trace loading` ranks the decoded frames and the cold pipelines first; the passes there are the
simulations' own (VHS Tape's 4 fsc grid, fourteen passes; Camera ISP at K = 1, which D1 does not touch).
The composing thread's encode is 0.10–0.15 ms a composite either way. What is local to the composite
path is D1 and D3; the first lap of this window is the decoder's. Row 118 stays open with that verdict.

### D6. More of the class, taken where the bytes stay and it pays

- The camera microphone's anti-alias FIR (63 products a sample per channel) runs only on the samples the
  device keeps (1 in 3 or 6 at 16 and 8 kHz); its output was read only there.
- Camera ISP's `ix(name)` is a map built once: `Plan::new` asks ~69 names a frame and the microphone ~30
  a chunk, each a scan of ~98 names. (The microphone's own index table from stage 1 went with it.)
- The warp's sinc table (`kt-fx/src/audio/sinc.rs`) is memoized by (length, oversampling, cutoff bits),
  bounded at 32: a warp is built at every seek, fresh entry and lap entry of a warped event, and the
  table took 0.2–0.8 ms (release; 16 384 × three `fcos` and one `fsin` at 128 × 128) on the render
  thread each time; a hit costs ~0.1 µs.
All three are the same function of the same arguments: t1981, t2171, t2315, t1067, t4 hold the bytes.

## Class walk (what was checked)

Mechanism: a kernel computes per element (sample, grain, pixel, chunk, frame, pass) a value constant
over a larger unit, or allocates per unit. Walked by reading (an Explore sweep, then each finding read):
- **Audio kernels**: every `process`/`set_targets`/`apply` of `fx-audio-dynamics` (gain, gate,
  limiter, compressor — memoized already), `fx-audio-tone` (filter, EQ, width, distortion, bitcrusher),
  `fx-audio-modulation` (tremolo, auto pan, vibrato, chorus, flanger, phaser, delay, Slowed+Reverb,
  pitch WSOLA and vocoder), VHS Tape's sound, Camera ISP's microphone, Datamosh's sound, the audio
  generators, the volume transitions; `kt-plugin-api` `dsp` and `audio_codec`; `kt-fx` audio (warp,
  sinc, nodes, loudness); `kt-engine` audio mixdown and `mix.rs`.
- **Video**: every barrier's graph builder (Rust, per frame) and WGSL (blur family, stylize, distort,
  color, tape, camera, codec), the generators' and transitions' WGSL, `composite.wgsl`, `geom_fused.wgsl`,
  `resample.wgsl`, the executor's per-frame code in `kt-gpu/src/engine.rs`.
- **Fixed here**: rows 136, 138, 211, 311–314 and D6. **Closed by measurement**: 137, 210.
- **Found and left** (rows 350–365 of `docs/found-defects.md`): the warp's per-chunk buffers (350), the
  chain's dry/twin chunk copies (351), the mixdown's per-call scratch (352), the mic's polyphase zeros
  (353), the simulations' per-chunk row clones (354), the EQ/filter's per-channel re-tune (355), the delay
  lines' `% len` per sample (356), VHS sound's per-sub-sample divisions (357), the mic's gust hashes
  (358), the vocoder's zero-fill (359), the noise generator's row hashes (360), the composite's repeated
  `textureDimensions` (361), Camera's unprocessing `pow` per tap (362), per-pixel math of per-frame
  constants in transitions and Camera (363, would move bytes), the executor's small per-frame allocations
  (364), the simulations' per-frame plan vectors (365).
- **Clean**: gain, gate, limiter, compressor (memoized), width, distortion, bitcrusher, tremolo, auto
  pan, vibrato and flanger (their per-sample `fsin` is the effect), delay, Slowed+Reverb, the WSOLA search,
  `dsp::Memo`/`SmoothedParam`/`Biquad`/`EnvFollower`/Freeverb's process, the codec's tables, the loudness
  meter, `render_plain_clip`, `weight_run`, `mix.rs`; WGSL of halftone, edges, the distort warps (their
  trig depends on position), Camera's s02/s04/s05/s07/s09/s10 loops, the codec, `geom_fused`, `resample`.

## Risks / Trade-offs

- [A memo grows without bound] → the sinc memo keeps 32 tables, the bind cache 256 groups and a frame.
- [A timing test reads another test's passes] → the pass timer is process-wide; t2311 holds its file's
  lock (red at 0.81 beside t2310, 0.33–0.34 alone).
- [A cached bind group keeps a texture alive] → one frame at most, bounded entries; memory-ledger's
  accounts see the release a frame late at worst.
