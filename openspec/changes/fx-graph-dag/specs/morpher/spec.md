## ADDED Requirements

### Requirement: A patch's graph is spliced after its target's Output
A morpher patch's chains SHALL be allowed the same inputs and Merge nodes as an event's; spliced into
a target, the patch's Source SHALL be the target's Output, its inputs offset by the target's length,
and the layer strength's dry side SHALL stay the target's Output. A paired device's halves SHALL stay
mirrored in presence, values and curves; their wiring SHALL be each lane's own. Removing a slot that
another slot reads SHALL re-wire the reader to the removed slot's own input, in the same step.
Why: the layer is "the same edit on everything below"; a branch inside it must mean the same on every
target, and the pair rule shares the device, not its place in a graph.

#### Scenario: A branching layer over a branching event
- **WHEN** a layer whose patch merges a blurred arm over its input lies over an event whose own chain branches; the patch's VHS Tape picture half is moved into an arm; then removed
- **THEN** the effective chain is the event's graph followed by the patch's with offset inputs, and renders as the two applied in turn; the sound half stays on the trunk of the sound chain and mirrors values; the removal takes both halves and re-wires the reader
- **TEST** t3222, t3223 (`src/libs/kt-model/tests/sim_pairs.rs`)
