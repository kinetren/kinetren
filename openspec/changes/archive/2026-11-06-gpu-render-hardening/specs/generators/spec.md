## ADDED Requirements

### Requirement: A generator that costs more than a compute frame is a shader
The kinds line, ellipse, rect, leader, gradient, corners and noise SHALL draw on the compute device,
and with no device SHALL give a transparent frame of their size, without a panic. A kind that costs
less than a compute frame with its readback SHALL stay on the CPU.
Why: 1.4 ms against 92 ms on the same 1080p frame, on llvmpipe; the owner, C2 of kkt-26.11.03.

#### Scenario: No device
- **WHEN** a shader kind is asked for a frame with no compute device
- **THEN** the frame is transparent, of its size
- **TEST** t1645 (`src/libs/kt-plugin-api/tests/registry.rs`)
