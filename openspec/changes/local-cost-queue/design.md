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

### D2. Radial blur's spin table on the device (row 137)

The uniform of a barrier pass is 64 bytes of `u`/`f`: no room for 32 numbers plus the centre, and the
geometry pass's 1024-byte layout belongs to geometry and point-wise runs. A CPU table (frozen `fsin`)
would also change the bytes: the device's `sin` is not the frozen one. So the table is made ON THE DEVICE
by a 32×1 `PassAt` that evaluates the same expression (`0.6·amount·(t − 0.5)`, `t = f32(i)/15`) and
writes the f32 bits of each sine and cosine into a texel; the main pass reads them from `T1`. The plugin
contract is unchanged (`PassAt`, `Save`, `Saved` exist since vhs-tape-simulation). Bytes: held by t2312
on the device; a driver that folds `t` at compile time in one shader and divides at run time in the other
could differ (WGSL division is not correctly rounded): the table pass takes `t` from a constant array.

### D3. One uniform write per piece, bind groups reused (row 138)

Each pass site writes its uniform with `queue.write_buffer` and creates a bind group. Writes are staged in
a frame-local byte vector covering the arena's used slots and flushed once before each submission
(`cut_point`'s piece submit and the frame's own). Bind groups go into a small map keyed by (layout, the
three views) — `wgpu::TextureView` is `Hash + Eq` by id — kept per engine, emptied of entries not used in
the last frame at each top-level frame, and bounded (64): a released texture lives at most one frame more.
The arena buffer is bound with a dynamic offset, so its slot is not in the key; a grown arena empties the
map.

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

### D5. Row 118

Measured on the heavy showcase with the structured trace; what is local to the composite path is fixed
here (D1–D3 are its candidates), the remainder written down. Filled in during apply.

## Class walk (what was checked)

Filled in during apply: every audio kind's `process`/`set_targets`, every video barrier's graph and WGSL,
the generators, the transitions, the executor's pass sites.

## Risks / Trade-offs

- [The device evaluates the table's `sin` differently from the in-pass `sin`] → t2312 on llvmpipe; a
  device where it differs falls inside `FrameTol::BILINEAR` of the stored frames (t181).
- [A cached bind group keeps a texture alive] → one frame at most, bounded entries; memory-ledger's
  accounts see the release a frame late at worst.
