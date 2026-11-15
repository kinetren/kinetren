## ADDED Requirements

### Requirement: A pass does not repeat per fragment what is one value for the frame, where the device keeps the bytes
A pass SHALL take once per fragment a value it asked per tap (the composite's side sizes), and SHALL
compute on the device, once per frame, a per-pixel function of a byte that a pixel asks many times
(Camera ISP's source transfer under defocus: a 256-texel table made by the same `decode`), where the
frame stays the former bytes on the same device. Per-pixel math of per-frame constants that would move
bytes if moved to the CPU stays where a measurement shows no cost (the transitions' `sin(π·k)`, spin's
rotation, Camera ISP's per-frame scalars).
Why: found-defects rows 361, 362, 363 (per-sample-costs).

#### Scenario: The curve table is the per-tap form's bytes
- **WHEN** Camera ISP runs with defocus for every source curve, two radii, opaque and alpha, at K 1 and 2, with the table and under `cpu::with_general` (the per-tap form)
- **THEN** the frames are equal byte for byte, and the graph holds the table pass only with defocus
- **TEST** t2683 (`src/libs/kt-gpu/tests/camera_isp.rs`)
