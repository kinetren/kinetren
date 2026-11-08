## ADDED Requirements

### Requirement: A kernel names its passes and its dispatches
A barrier kernel MAY name each of its `shaders()` with `BarrierKernel::pass_names` (same order, same
length); the host calls an unnamed pass `p<i>`. A compute `Step` SHALL carry a `name`. The names are what
the telemetry journal's GPU record shows (`vhs_tape:s5-head-read`, `datamosh:search`,
`generator:noise`); a host node inside a barrier's graph is named after the barrier (`vhs_tape:resample`).
The multi-pass simulations (VHS Tape, Camera ISP) name every pass.
Why: gpu-pass-cost — a record that says "p6" tells the owner nothing.

#### Scenario: Every tape pass has its name
- **WHEN** a frame with VHS Tape is timed
- **THEN** no pass of the tape is called `p<i>`, and its resample to the grid is `vhs_tape:resample`
- **TEST** t2020 (`src/libs/kt-gpu/tests/pass_time.rs`)
