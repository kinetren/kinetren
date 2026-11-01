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
- **WHEN** every registered barrier with at most eight passport rows runs through the host's value form and through its cooked form at
  its defaults and its top
- **THEN** the frames are byte-equal and the graphs equal
- **TEST** t1187 (`src/libs/kt-fx/tests/barrier_plugin.rs`), t1051, t1052

### Requirement: A layer strength over a step that destroys alpha keeps each side's alpha on both executors
When a layer strength (a mix of the chain before a step with the chain after it) stands over a step
whose relation to alpha is "destroys", the GPU executor SHALL mix the dry side with its OWN alpha
(the chain's fold at the snapshot) and the wet side with its own, and the mixed frame SHALL have
alpha when either side has it — as the CPU's frames do. The end-of-chain flag SHALL NOT stand for
the dry side.
Why: found by the VHS tape simulation (vhs-tape-simulation), the first barrier that declares
"destroys": t1052 ran it under a strength of 0.5 on a frame with alpha, and the GPU forced the dry
side opaque (it passed the chain's final flag for both sides) and then declared the mixed layer
opaque, so the composite skipped the alpha-over — 86 % of the samples differed from the CPU.

#### Scenario: A destroying barrier under a strength on a frame with alpha
- **WHEN** every registered barrier runs under a layer strength of 0.5 on a frame with gradient alpha, on both executors
- **THEN** the GPU result is within the kind's tolerance class of the CPU, the alpha channel included
- **TEST** t1052 (`src/libs/kt-gpu/tests/barrier_graph.rs`)
