## ADDED Requirements

### Requirement: The RGB to YUV conversion of an export is exact and runs on the composing device
An export SHALL convert its RGBA composite to the encoder's pixel format (yuv420p, NV12, yuv420p10le,
p010le, yuv422p10le) by one integer formula — the chosen matrix and range in Q15, chroma sited left
(horizontal [1 2 1], vertical [1 1] for 4:2:0; horizontal only for 4:2:2), the edge repeated — computed on
the device that composed the frame, and SHALL read back the encoder's planes, not RGBA. A path with no such
pass SHALL convert by the CPU reference of the same formula. The stream SHALL be tagged with the left
chroma location besides its matrix and range. The planes SHALL be a function of the RGBA only, equal on
every device and machine.
Why: found-defects 741 — the owner's trace: copy out 2.2 ms a frame (12 % of the wall) and the CPU convert
26 % of it; export-host-copy D2, D3. swscale's RGB input path has no documented filter to reproduce on a
device, and its C and SIMD paths differ.

#### Scenario: The formula's values
- **WHEN** black, white, the six primaries and secondaries, and a one-pixel stripe pattern are converted by the reference for BT.601 and BT.709, limited and full, 8 and 10 bit, 4:2:0 and 4:2:2
- **THEN** each Y, Cb and Cr equals the value computed by hand from the Q15 coefficients, the stripes give the left-sited chroma, and odd widths and heights repeat the edge
- **TEST** t3312 (`src/libs/kt-gpu/src/yuv.rs`)

#### Scenario: The device pass equals the reference byte for byte
- **WHEN** composites of odd and even sizes are converted by the pass on the test device into each of the five layouts
- **THEN** the planes read back equal the CPU reference's byte for byte
- **TEST** t3313 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: The export reads back planes and stays reproducible
- **WHEN** a two-second project is exported pipelined and serially to libx264 (yuv420p), and pipelined twice to a forced NV12 encoder
- **THEN** the pipelined and serial files are equal, the two NV12 runs are equal, the bytes read back per frame are the layout's (1.5 per pixel), and the convert stage only copies planes
- **TEST** t3314 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The ring counts the layout's bytes
- **WHEN** the ring depth is asked for 1080p RGBA, yuv420p and p010le readbacks
- **THEN** each depth is `RING_BYTES` over the layout's frame bytes, within `RING_MIN..=RING_MAX`
- **TEST** t3315 (`src/libs/kt-gpu/tests/export_ring.rs`)

#### Scenario: A path without the pass writes the same bytes
- **WHEN** one frame is converted by the reference on the processor (the GL bytes path's way) and by the pass on the device
- **THEN** the planes are equal
- **TEST** t3318 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The file says where its chroma sits
- **WHEN** an export with BT.709 limited is read back by the output probe
- **THEN** it reports the matrix, the range and the left chroma location, and a saturated red comes back within 6 codes through the decode that reads the tags
- **TEST** t3319 (`src/libs/kt-engine/tests/render_settings.rs`)
