## ADDED Requirements

### Requirement: A pass computes per pixel only what varies per pixel
A video effect's pass SHALL take once, at its entry, what is the same for every pixel of the pass (the
input textures' sizes, scales, periods), and once per window row or column what varies only by row or
column (an optical-black row term, a mosaic phase, a wrapped coordinate), and SHALL read a neighbourhood
it filters several times only once. A compute kernel SHALL spread its work over invocations (a workgroup
per block, an invocation per output) rather than loop over a block in one invocation with large private
arrays. Such a rewrite keeps the same operations on the same values in the same order: the effect's
tolerances against its stored frames and its pinned bytes hold.
Why: gpu-pass-cost, measured with the GPU record: Camera ISP's raw-front evaluated two hashed row/column
terms for each of up to 29 window reads (37 ms a 1080p frame on llvmpipe, 15 after); Datamosh's residual
ran a thread per 8×8 block with three 64-float arrays, which a card spills to scratch.

#### Scenario: The Camera ISP keeps its picture
- **WHEN** the ISP's GPU graph runs after the rewrite of its helpers, raw-front, raw NR and YUV
- **THEN** it agrees with the CPU kernel within its tolerance and its consequences hold
- **TEST** t1217 (`src/libs/kt-gpu/tests/camera_isp.rs`)

#### Scenario: Datamosh keeps its bytes
- **WHEN** the residual runs a workgroup per block and the planes go up four samples to a word
- **THEN** the pinned digest of sixteen moshed frames is unchanged
- **TEST** t1239 (`src/plugins/fx-video-codec/tests/codec.rs`)
