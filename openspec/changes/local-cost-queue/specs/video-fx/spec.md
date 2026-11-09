## ADDED Requirements

### Requirement: Camera ISP computes a sensor pixel once at any K
At K > 1 (the frame K× the sensor's grid) Camera ISP SHALL render every pass that works on the sensor's
grid at the sensor's size, and only the graph's last pass (or a pass on the frame's own grid: the
unprocessing, the scene view) at the frame's size, reading the sensor-sized input at the pixel's sensor
position. The output SHALL be the same bytes as the form that rendered every pass at the frame's size, on
the same device, for every preset, layout and demosaic, on opaque and alpha frames, at K 1 to 4; at K = 1
the graph does not change.
Why: found-defects row 136 (gpu-pass-cost): behind a K× barrier every sensor pixel was computed K² times
(16× at K = 4) in up to ten passes.

#### Scenario: The sensor-grid form is the frame-grid form's bytes
- **WHEN** the defaults, every preset and every layout × demosaic run on opaque and alpha frames behind a zoom that gives K = 2, 3 and 4, in the sensor-grid form and in the frame-grid form, on one device
- **THEN** the two frames are equal byte for byte, and the sensor-grid graph renders its sensor passes at ⌈frame/K⌉
- **TEST** t2310 (`src/libs/kt-gpu/tests/camera_isp.rs`)

#### Scenario: The sensor-grid form costs less on the device
- **WHEN** the defaults run at K = 2 at 960×540 logical (1920×1080 frame) on a software device, both forms interleaved, minima of the pass timer
- **THEN** the camera's passes together take at most 60 % of the frame-grid form's device time (measured 0.40: 314.8 → 127.2 ms on llvmpipe)
- **TEST** t2311 (`src/libs/kt-gpu/tests/camera_isp.rs`)
