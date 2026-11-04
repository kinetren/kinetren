# Spec Delta

## ADDED Requirements

### Requirement: A geometry series that ends in an event grid computes one cell
When the event grid is the LAST geometry step of a fused series, and no point-wise tap that
takes its coordinate after the grid is absorbed into it, the series SHALL compute ONE tile.
The tile is computed with the same inverse map and the same supersampling rule (`ss_taps`,
cap 4×4). The pass then SHALL replicate the tile into every cell and compute fully only the pixels
whose sub-samples cross the grid's outer edge. The field outside the grid stays as before.
The replicated frame SHALL be byte-for-byte the frame of the full pass on the CPU. The GPU
executes the same tile rule and SHALL stay within the existing tolerance class against the CPU. Any series that
does not meet the condition (a geo step after the grid, a tap whose stage follows the grid, a
perspective program) SHALL keep the full pass. There is no switch and no quality mode.
The sample cost of a series SHALL be a pure function of the program: the number of
sub-samples the executor will take for one frame. Tests and the cost counters read it.
Why: ktprompts v26.10.18 item 8: "an event grid … drops fps; it is basically a simple pixel
shader — so do we have extra computations?" There was no per-cell work, but there was
per-cell REPETITION. Every output pixel of a 12×30 grid at 960×540 averaged 16 sub-samples,
so 8.3 M sub-samples were taken per frame for 360 copies of one tile of 80×18 pixels. The owner's rule
"no switches" (R.44) holds because the bytes do not change.

#### Scenario: The cost of a grid is the cost of one cell
- **WHEN** the sample cost is computed for a 960×540 frame through a grid of 1×1, 2×2, 12×10 and 12×30 cells, before and after the tile rule
- **THEN** before: 1×, 4× and 16× the frame's pixel count (518 400, 2 073 600, 8 294 400, 8 294 400 sub-samples); after: for 12×10 and 12×30 no more than one cell's pixels × 16 plus the pixels of the outer ring × 16 plus one copy per frame pixel, at least 8× below the full pass; 1×1 and 2×2 are unchanged
- **TEST** t942 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The copied cells are the computed cells, byte for byte
- **WHEN** a sweep of grids (cols 1–16 × rows 1–32, centred and not, on 960×540, 320×180 and an odd 333×187 frame), with a pan, a lead Geometry node and an absorbed point-wise effect BEFORE the grid, is executed by the tile rule and by the full pass on the CPU; and series with a zoom or a mirror AFTER the grid
- **THEN** every frame of the tile rule equals the full pass byte for byte (taps 1, 2, 3 and 4 per axis are covered); the series with a step after the grid take the full pass; `src/libs/kt-fx/src/geom.rs` and `src/libs/kt-gpu/src/shaders/geom_fused.wgsl` carry the same condition
- **TEST** t943 (`src/libs/kt-fx/tests/geo_fuse.rs`, `src/libs/kt-fx/src/geom.rs`, `src/libs/kt-gpu/src/shaders/geom_fused.wgsl`)

#### Scenario: The GPU replicates the same tile
- **WHEN** the sweep's grids 2×2, 3×3, 12×10 and 12×30 are executed on the GPU and on the CPU
- **THEN** the GPU frames are within the tolerance class of t296 against the CPU frames, and the GPU runs one tile pass plus one replicate pass for the series instead of one full pass
- **TEST** t944 (`src/libs/kt-gpu/tests/parity.rs`, `src/libs/kt-fx/src/geom.rs`, `src/libs/kt-gpu/src/shaders/geom_fused.wgsl`)
