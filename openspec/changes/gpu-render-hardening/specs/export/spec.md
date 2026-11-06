## MODIFIED Requirements

### Requirement: A software adapter composes an export on several devices
An export on a software adapter SHALL compose on a device per frame worker: an eighth of the cores, 2
to 4 of them, as many as the available memory holds for the frame size, and at least one. One device
SHALL serve a real card. The frames written SHALL NOT depend on how many devices composed them. The
video decoders of a render SHALL open with four threads, and a render short of memory SHALL run tight:
two decoder threads, half the frame cache, four encoder threads. A render SHALL NOT die of memory;
speed is secondary.
Why: each device keeps its own textures and shaders and adds 1.3 GB at 1080p, and seven decoders at
auto held 2.7 GB of frames for nothing: the showcase went from 17 GB to 8.7 GB and from 144 s to 130 s.

#### Scenario: Several devices write the frames of one
- **WHEN** one project is exported through one engine and through three, on the same adapter
- **THEN** the frames written are byte for byte equal
- **TEST** t1573 (`src/libs/kt-gpu/tests/export_ring.rs`)

#### Scenario: The device count follows the memory and the frame
- **WHEN** the count is asked with plenty of memory, 10 GB, 4 GB, 1 GB at 4K, or memory unknown
- **THEN** it is the ceiling, less than the ceiling, one, one (never none), and two; a bigger frame gets
  no more devices than a smaller one; little memory gets two decoder threads, any other four
- **TEST** t1642 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The memory is read
- **WHEN** the program asks what it may take on Linux or Windows
- **THEN** it gets a number; on Linux a control group's limit counts
- **TEST** t1643 (`src/libs/kt-gpu/tests/compute_host.rs`)
