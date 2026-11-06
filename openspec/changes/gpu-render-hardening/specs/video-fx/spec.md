## ADDED Requirements

### Requirement: The rules of the kernels hold on the GPU path
A barrier SHALL keep its look at K (the view moves by a mean of at most 2.5). The second-wave barriers
SHALL keep their meaning (a flat pixelate cell, a shadow in a keyed zone, a two-level halftone). A
thumbnail SHALL be the fit plus the chain plus the fit to the cell, and its grain SHALL follow the tick.
A grid's cost SHALL be what its program states, and a tile SHALL be refused where it would not be exact.
Mask modes SHALL combine by their table, with invert and opacity.
Why: the owner allowed changing references, not leaving rules unchecked.

#### Scenario: A barrier at K keeps its look
- **WHEN** a barrier runs on a frame 2× and 4× the logical size, the plan confirming K
- **THEN** the picture lies within the thresholds of the 1× one
- **TEST** t303 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The second-wave barriers mean what they say
- **WHEN** sharpen, pixelate, chromatic aberration, radial blur, drop shadow and halftone run
- **THEN** each shows its own consequence
- **TEST** t332 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: A thumbnail is fit plus chain
- **WHEN** a thumbnail job renders
- **THEN** its cell equals fit + chain + fit, on every run
- **TEST** t544e (`src/apps/kt-app/src/fx_thumbs.rs`)

#### Scenario: A thumbnail's grain follows the tick
- **WHEN** a noise thumbnail renders twice at one tick and once at another
- **THEN** the first two are equal and the third differs
- **TEST** t544f (`src/apps/kt-app/src/fx_thumbs.rs`)

#### Scenario: A grid's cost is counted
- **WHEN** grids of 1×1, 2×2, 12×10 and 12×30 run
- **THEN** the program states the sub-samples and the executor reports the same
- **TEST** t942 (`src/libs/kt-fx/tests/grid_cost.rs`)

#### Scenario: A tile is refused where it would not be exact
- **WHEN** a geometry step or a point-wise op follows the grid, or a cell takes exactly 3 taps
- **THEN** the program is not marked; a marked one costs less than the full pass
- **TEST** t943 (`src/libs/kt-fx/tests/grid_cost.rs`)

#### Scenario: Mask modes
- **WHEN** two masks combine in each mode, with invert and opacity
- **THEN** the coverage follows the table
- **TEST** t1268 (`src/libs/kt-fx/tests/masks.rs`)
