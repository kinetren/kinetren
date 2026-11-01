## ADDED Requirements

### Requirement: A barrier pass may render at its own size, and a barrier may read the passport values
A barrier's GPU graph MAY contain a pass rendered at its own size (`Node::PassAt`, both sides in
1..=the intermediate-buffer ceiling); the graph's output SHALL still be at the frame size, and the
registry SHALL refuse a pass-at-size of zero or above the ceiling as it refuses such a resample. The
host SHALL count such a pass in the uniform arena and in the intermediate-size check like any pass.

A barrier kernel MAY read its passport values directly (`frame_values`, `gpu_values`) when its
parameters do not fit the eight cooked numbers. The default forms SHALL cook the values and call the
eight-number kernel, and the host SHALL call the value forms, so every existing barrier keeps its
bytes and its graph.
Why: the VHS tape simulation (kkt-26.11.01 V1) runs on a signal grid of the standard, independent of
the frame size (a 640-wide preview has fewer texels than the grid), and has a deck's worth of physical
quantities (~30).

#### Scenario: A pass at its own size is checked like a resample
- **WHEN** test barriers are registered with a graph containing a pass at 1024×576 that ends with a
  frame-size pass; a pass at 0×576; a pass above the ceiling; and a graph ending on the 1024×576 pass
- **THEN** the first is accepted; the second and third are refused naming the size; the last is
  refused for ending off the frame size
- **TEST** t1187 (`src/libs/kt-plugin-api/tests/barrier_registry.rs`)

#### Scenario: The value forms keep every barrier's bytes
- **WHEN** every registered barrier runs through the host's value form and through its cooked form at
  its defaults and its top
- **THEN** the frames are byte-equal and the graphs equal
- **TEST** t1187 (`src/libs/kt-plugin-api/tests/barrier_registry.rs`), t1051, t1052
