## ADDED Requirements

### Requirement: A picture made on the composing device is composed where it lies
The trace SHALL record every stream-stage or compute-generator frame read back to be composed: its bytes,
the processor time of its conversion and the bytes uploaded again. `kt-cli trace … loop` SHALL sum them
per lap.

A stream stage or a compute generator whose output lies on the device of the executor composing the frame
SHALL hand the plan that buffer (a held layer input), not bytes. The executor SHALL convert it into the
layer's picture on the device with the processor conversion's integer arithmetic, so the composite is
byte-equal to the bytes path. Any other executor SHALL read the buffer back as before. The held form SHALL
NOT change any key, the plan's completeness or the coverage.

Why: found-defects 663 — every shown Datamosh frame was read back as planes, converted on the processor
and uploaded again, between two devices of the same chip (stream-picture-stays D1, D3).

#### Scenario: The crossing is in the trace
- **WHEN** a Datamosh loop plays three laps with two devices, and its trace is read
- **THEN** each lap names the frames crossed, their megabytes and the conversion's processor time, and a project with no stream stage crosses nothing
- **TEST** t3812 (`src/libs/kt-engine/tests/stream_cross.rs`), t3813 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: A held layer composes as the bytes did
- **WHEN** a Datamosh layer and a compute generator are composed on the compute host's own device, held, and on it again through the bytes path
- **THEN** the two composites are equal byte for byte and the held one read nothing back
- **TEST** t3814 (`src/libs/kt-gpu/tests/stream_held.rs`)

#### Scenario: The planes are converted as the processor converts them
- **WHEN** decoded planes of even and odd widths and heights are converted on the device and on the processor
- **THEN** the RGBA bytes are equal
- **TEST** t3815 (`src/libs/kt-gpu/tests/stream_held.rs`)

#### Scenario: Another device reads back
- **WHEN** the composing executor is not on the compute host's device
- **THEN** the stage gives bytes, as before, and the composite is the same
- **TEST** t3816 (`src/libs/kt-gpu/tests/stream_held.rs`)

### Requirement: One device per adapter where the window can compute
Where the window's backend has compute (Vulkan, DX12, Metal) and its adapter is the compute host's, the
program SHALL open the window's device itself through kt-gpu's one instance, with the compute host's and the
pass timer's features, hand it to the window (eframe's existing-device setup), and install the compute host
over the same device and queue. On GL, or with the two on different adapters, the compute host SHALL stay a
device of its own, and the machine facts in the trace SHALL say which and why. A device loss SHALL poison
the preview and the compute host together; the interface's save and the close path's durability SHALL NOT
need the device.

Why: the codec and the window ran on two devices of one chip, and the picture crossed between them (found-
defects 663); the compute-backends decision keeps one API, wgpu, so one device per adapter is the way to
share (stream-picture-stays D2).

#### Scenario: The compute host over an existing device
- **WHEN** a device is opened and the compute host installed over it, then a Datamosh walk runs
- **THEN** the walk's frames are held on that device and equal the walk on a device of its own
- **TEST** t3817 (`src/libs/kt-gpu/tests/compute_shared.rs`)

#### Scenario: The setup picks one device or two
- **WHEN** the window's setup is resolved for a Vulkan adapter with compute, for GL, and for a hybrid pair whose window is on the other chip
- **THEN** the first is one device shared, the others two, each named in the machine facts
- **TEST** t3818 (`src/libs/kt-gpu/tests/compute_shared.rs`)

#### Scenario: A lost shared device poisons both, and the work is saved
- **PATH** a project with Datamosh plays → the device is lost: the preview says it has no picture and a notice rings once → Space stops → Ctrl+S: the title loses its asterisk
- **WHEN** the shared device is lost during a walk
- **THEN** the preview says it has no picture, the compute host refuses new jobs, and Ctrl+S writes the project
- **TEST** t3819 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: One device crosses nothing over a loop
- **WHEN** `kt-cli preview` plays a Datamosh loop with one device per adapter
- **THEN** the trace's crossed bytes are 0 and the frames equal the two-device run's
- **TEST** t3821 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: A paused Datamosh loop costs nothing on one device
- **PATH** the heavy showcase with a Datamosh loop → Space plays a lap → Space stops → the mouse rests, then moves over the preview → nothing redraws or walks by itself
- **WHEN** the heavy showcase with a Datamosh loop stands paused with one device per adapter, the mouse moving over the preview
- **THEN** once the history is walked no frame is asked at rest and no compute job runs
- **TEST** t3822 (`src/apps/kt-app/src/idle_tests.rs`)
