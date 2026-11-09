## ADDED Requirements

### Requirement: The executor binds and writes uniforms per frame, not per pass
The GPU executor SHALL write a frame's pass uniforms into its arena with one queue write per submitted
piece (a piece the arbiter cuts carries its own passes' uniforms before it is submitted), and SHALL reuse a
bind group when the same layout meets the same textures again, keeping at most a bounded set and never a
texture its owner released beyond the frame in which it was released. The frames SHALL be the same bytes
as with a write and a bind group per pass, preview and export, cut into pieces or not; after warm-up a
frame creates no GPU resource (t2023).
Why: found-defects row 138 (gpu-pass-cost): a bind group and a uniform write per pass on the composing
thread.

#### Scenario: Batched uniforms and reused bind groups keep the bytes
- **WHEN** a chain of geometry, point-wise runs, barriers (blur, glow, Camera ISP, VHS Tape) and a transition is composed per frame, per piece with the arbiter cutting at every pass, and through the export ring, with the per-pass form and the batched form
- **THEN** every frame is equal byte for byte between the forms
- **TEST** t2313 (`src/libs/kt-gpu/tests/local_cost.rs`)

#### Scenario: The steady state creates no bind group
- **WHEN** a heavy chain plays the same 24 frames twice after warm-up
- **THEN** the second lap creates no bind group, and the cache holds no more entries than its bound
- **TEST** t2314 (`src/libs/kt-gpu/tests/local_cost.rs`)
