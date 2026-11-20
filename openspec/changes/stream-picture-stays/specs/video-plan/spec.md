## ADDED Requirements

### Requirement: A picture made on the composing device is composed where it lies
The trace SHALL record every stream-stage or compute-generator frame read back to be composed: its bytes,
the processor time of its conversion and the bytes uploaded again. `kt-cli trace … loop` SHALL sum them
per lap.

A stream stage whose picture the compute host keeps on its device, at the layer's decode size, SHALL hand
the plan that buffer (a held layer input, `LayerInput::Held`), not bytes. An executor on that device SHALL
convert it into the layer's picture with one pass in the processor conversion's integer arithmetic
(`kt_plugin_api::stream::planes_to_rgba`), so the composite is byte-equal to the bytes path. Any other
executor SHALL read the planes back through the compute host and convert them on the processor. The held form SHALL
NOT change any key, the plan's completeness or the coverage.

Why: found-defects 663 — every shown Datamosh frame was read back as planes, converted on the processor
and uploaded again, between two devices of the same chip (stream-picture-stays D1, D3).

#### Scenario: The crossing is in the trace
- **WHEN** a Datamosh loop plays three laps with two devices, and its trace is read
- **THEN** each lap names the frames crossed, their megabytes and the conversion's processor time, and a project with no stream stage crosses nothing
- **TEST** t3812 (`src/libs/kt-engine/tests/stream_cross.rs`), t3813 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: A held layer composes as the bytes did
- **WHEN** a Datamosh frame inside the mosh is planned, then composed by an engine on the compute host's device and by one on another device
- **THEN** the layer's input is held; the first converts it on the device and reads nothing back, the second reads it back; the two composites are equal byte for byte and differ from the unmoshed frame
- **TEST** t3814 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: The planes are converted as the processor converts them
- **WHEN** planes of every byte value, padded 16×16 to 80×48, cropped to even and odd widths and heights down to 1×1, are composed held on the shared device and as the processor's conversion
- **THEN** the composites are equal, converted on the device with nothing read back
- **TEST** t3815 (`src/libs/kt-gpu/tests/stream_held.rs`)

#### Scenario: Another device reads back
- **WHEN** the composing executor is not on the compute host's device
- **THEN** it reads the planes back through the compute host, and the composite is the bytes path's
- **TEST** t3816 (`src/libs/kt-gpu/tests/stream_held.rs`)

### Requirement: One device per adapter where the window can compute
Where the window's backend has compute (Vulkan, DX12, Metal) and its adapter is the compute host's, the
window's device (eframe's, asking the pass timer's features, which are the compute host's) SHALL be the
process's one context, and the compute host SHALL open over that same device and queue before anything else
opens it; the preview's engine is built on the same context. On GL, or with the two on different adapters, the compute host SHALL stay a
device of its own, and the machine facts in the trace SHALL say which and why. A device loss SHALL poison
the preview and the compute host together; the interface's save and the close path's durability SHALL NOT
need the device.

Why: the codec and the window ran on two devices of one chip, and the picture crossed between them (found-
defects 663); the compute-backends decision keeps one API, wgpu, so one device per adapter is the way to
share (stream-picture-stays D2).

#### Scenario: The compute host over an existing device
- **WHEN** a window's device is shared and the compute host opens, and an engine on it leaves a frame on the device
- **THEN** the host's device is the window's (the same `Arc`), and a compute job reads the held frame as it lies — the bytes `execute_once` reads back
- **TEST** t3817 (`src/libs/kt-gpu/tests/compute_shared.rs`)

#### Scenario: The setup picks one device or two
- **WHEN** the decision is asked for a Vulkan window on the adapter the host picks, a GL window, a hybrid pair whose window is on the integrated chip, no adapter for the host, and a lone integrated chip
- **THEN** the first and the last share, the others do not, each with its reason (the trace's `gpu.compute.shares_display`, the run journal's GPU line); the host's pick skips WARP and GL and prefers a card
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
