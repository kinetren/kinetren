## ADDED Requirements

### Requirement: The compute host reuses a job's buffers without showing it
The host SHALL hand a finished job's buffers to later jobs of the same size and usage, SHALL keep at
most 1 GiB of them, and a buffer a job asks for as zero SHALL be zero whatever job used it before.
Why: a kernel asks for the same set every frame; the pool must stay invisible to the bytes
(project rule on caches).

#### Scenario: A zero buffer stays zero after reuse
- **WHEN** a job fills a buffer, and the next job asks for a zero buffer of the same size
- **THEN** the buffer read back holds zeros
- **TEST** t1570 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The pool is bounded
- **WHEN** the same job runs twice, then jobs hold more than the bound
- **THEN** the second run does not grow the pool, and the pool never passes 1 GiB
- **TEST** t1571 (`src/libs/kt-gpu/tests/compute_host.rs`)

### Requirement: The Datamosh decoder is a compute shader
The block copy and the residual SHALL run as a compute job with no CPU copy: unrestricted vectors
clamped at the frame edge, the chroma vector halved and floored, the residual scaled by the gain and
floored, the result clamped to bytes.
Why: the owner, C2 of kkt-26.11.03; `predict` and `reconstruct` were the last single-thread stage of
a clean decode.

#### Scenario: The decoder's arithmetic
- **WHEN** a picture decodes with known vectors, a residual of both signs and the gain 128
- **THEN** each pixel is the copy at the clamped position plus the floored scaled residual
- **TEST** t1572 (`src/plugins/fx-video-codec/tests/codec.rs`)
