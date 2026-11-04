## ADDED Requirements

### Requirement: A kernel runs compute shaders through the host's door
A kernel SHALL be able to name WGSL compute shaders and buffers as a job the host runs on its own
compute device (Vulkan, DX12 or Metal, whatever backend the interface uses). The host SHALL report a
shader it cannot compile as an error, without a panic. With no compute device `host()` SHALL give
nothing, and the kernel SHALL say so instead of drawing something else.
Why: the owner, C2 of kkt-26.11.03 — the math runs on Vulkan rather than on hand-written copies; a
plugin still sees nothing of wgpu.

#### Scenario: A job runs and reads back
- **WHEN** a job doubles the words of a buffer in a compute shader, on a machine with a compute device
- **THEN** the buffer read back holds the doubled words, and a second job with the same text reuses
  the compiled shader
- **TEST** t1502 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: A shader the device refuses is an error
- **WHEN** a job names a shader that does not compile
- **THEN** `run` returns an error naming it, and the next valid job on the same host runs
- **TEST** t1503 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The Datamosh encoder keeps its quality on the compute device
- **WHEN** a moving textured picture is encoded in a closed loop at QP 24
- **THEN** the reconstruction's luma PSNR holds above 40 dB on every P-frame, and most blocks of the
  moving area carry the true motion
- **TEST** t1504 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: Datamosh with no compute device leaves the frame and says why
- **WHEN** the stream stage runs with no compute host installed
- **THEN** the frame is the source frame and the status names the missing device
- **TEST** t1505 (`src/plugins/fx-video-codec/tests/no_device.rs`)
