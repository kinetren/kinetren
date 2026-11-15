## ADDED Requirements

### Requirement: One resolved chain per layer and moment; carriers by row
A layer with live curves SHALL resolve its chain once per material tick and share it with every
plan of that moment (`LayerChain::Shared`, the layer's bounded `ChainMemo`); a layer without live
curves SHALL borrow its chain. Transitions, cameras, generators and masks SHALL apply their curves
by rows compiled once per build (`RowCurves`, `MasksPlan`), with the bits of the by-name path; a
frame sets no parameter by name.

#### Scenario: Three plans, one copy
- **WHEN** the probe plan and two composites of one moment of an animated layer are built, then a composite of the next frame
- **THEN** the chain is copied once for the first moment and once for the next, and it carries the curve's value
- **TEST** t2666 (`src/libs/kt-engine/tests/frame_cost.rs`)

#### Scenario: Carriers by row, same bits
- **WHEN** 50 frames of a scene with an animated wipe, a camera curve and an animated mask are planned, and the compiled forms are compared with `kind_at`, `params_at` and `masks_at` over 4 s
- **THEN** no parameter is set by name, and every compiled value equals the by-name value
- **TEST** t2667 (`src/libs/kt-engine/tests/frame_cost.rs`)
