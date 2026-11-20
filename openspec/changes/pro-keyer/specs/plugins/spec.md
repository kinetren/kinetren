## ADDED Requirements

### Requirement: A barrier may declare a held frame of its own source
A barrier plugin MAY declare a held frame — a frame of its layer's own source at a fixed material tick, read
by its pass graph as a second input; the plan SHALL ask and read it by the same function that decides the
layer's frames, count it in the chunk budget, warm it like any frame, and put its content into the layer's
key; a barrier without one SHALL keep its plan and bytes.
Why: invariant 4 (the order equals the read); the frame-blending precedent.

#### Scenario: The plate frame is requested and read as one
- **WHEN** a Pro Keyer with a held plate at material 2.0 s plays from 10 s, is exported in chunks, and the plate tick is moved
- **THEN** the prefetch and the composite ask the same plate key, the export chunk holds it, a moved tick changes the layer key and the picture; a barrier without a held frame has the plan it had
- **TEST** t3588 (`src/libs/kt-engine/tests/held_frame.rs`)
