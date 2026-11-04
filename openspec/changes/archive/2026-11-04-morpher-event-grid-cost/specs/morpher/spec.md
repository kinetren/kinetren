# Spec Delta

## ADDED Requirements

### Requirement: An automated event grid in a morpher costs one cell per frame
A morpher layer whose chain holds an event grid with automated column and row counts SHALL
NOT rebuild the renderer, the effective events or the automation plan while the automation
moves. Per frame it SHALL cost one geometry series with the sample cost of the tile rule (capability
`video-fx`) and, where the layer's strength is 0, nothing of the layer. The
rendered frames SHALL be byte for byte the frames before this change, in preview and export.
Why: the owner's exact case (ktprompts v26.10.18 item 8): "an event grid with automation 0 →
12×30 → 12×10 → 0 in the morpher manages to drop fps". The bound turns "is it simple?"
into a number a test holds.

#### Scenario: The owner's automation, counted
- **WHEN** a 4 s event (960×540 synthetic pattern) is partly covered by a morpher block with an event grid whose cols/rows are automated 1×1 → 12×30 → 12×10 → 1×1 (the owner's "0" clamps to 1), with fades at the block edges, and the range is rendered frame by frame through the preview path and exported
- **THEN** the renderer is built once (`prefetch_rebuilds` unchanged during the range); per frame the counters show at most one geometry series for the grid with a sample cost of at most one cell × 16 plus the outer ring plus one copy per frame pixel; frames where the strength is 0 show zero layer passes; every exported frame equals the preview frame and the frame of the same project rendered by the code before this change (a hash of the range recorded in stage 1)
- **TEST** t946 (`src/libs/kt-engine/tests/morph_grid_cost.rs`)
