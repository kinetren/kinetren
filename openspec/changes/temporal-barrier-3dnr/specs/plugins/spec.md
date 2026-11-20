## ADDED Requirements

### Requirement: A temporal kind is a recursion over its own state with declared anchors
A video plugin MAY declare the planner class `Temporal` and provide a temporal kernel: optional spatial
passes before (run on every history frame), a step from the current input and its previous state to the
new state and output, optional spatial passes after (run on the shown frame), a warm-up length and an
anchor grid. Its output at material frame N SHALL be a pure function of its rows and of the inputs from the
warm-up start of N's anchor to N, the state at an anchor being a cold recursion over the warm-up frames
before it. The registry SHALL refuse a `Temporal` class without a temporal kernel, a temporal kernel on
another class, and a kernel that keeps state with a zero warm-up.
Why: the camera-isp design's option B — the owner chose the real recursive filter on the GPU.

#### Scenario: The registry refuses a malformed temporal plugin
- **WHEN** a plugin declares `Temporal` without a kernel, a point-wise plugin offers a temporal kernel, and a kernel with state declares warm-up 0
- **THEN** each registration is refused naming the declaration; a well-formed temporal plugin registers
- **TEST** t3545 (`src/libs/kt-plugin-api/tests/registry.rs`)
