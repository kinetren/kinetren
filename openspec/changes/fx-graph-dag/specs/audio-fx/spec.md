## ADDED Requirements

### Requirement: The sound chain executes a graph, its branches aligned by look-ahead
A sound chain whose slots name their inputs SHALL be executed by the event's processor in slot order
on buffers from the render thread's pool: a held output goes to every slot that names it, a Merge
sums Background and mix × Foreground (Sum) or crossfades them (Crossfade) in that fixed order, and
before a Merge the input with less look-ahead SHALL be delayed by the difference so both sides are
aligned; the chain's look-ahead and its tail SHALL be those of the path to the Output. A
compressor's key socket MAY take an arm of its own chain. A warp, the stream's sound or an
edge reading inside an arm SHALL read its own source window (a sound read, as the picture's read),
consuming it as the chain's input consumes the source today; a branch on the sound lane is accepted
only from v2c on. A line SHALL compute exactly the samples it
computed before; a warm render SHALL allocate nothing. A rewire SHALL be a structural change of the
chain as a reorder is, and a value edit SHALL reach `set_params`.
Why: invariants 1–3 and 11; the layer strength's dry ring is the same alignment for one branch.

#### Scenario: A line is the chain, sample for sample
- **WHEN** every audio golden and the showcase render through the graph path with empty input fields
- **THEN** the samples equal the line's; GOLDEN_AUDIO holds; four warm seconds allocate nothing
- **TEST** t3212 (`src/libs/kt-engine/tests/audio_graph.rs`), GOLDEN_AUDIO (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: Parallel compression and a look-ahead arm
- **WHEN** an event sums its source with a compressed arm, then with an arm through a look-ahead limiter
- **THEN** the sum equals the source plus the arm computed alone, sample for sample, in both orders of building; the limiter arm's delay is matched on the dry side so a click lands on the same frame in both; two runs give the same bytes
- **TEST** t3213, t3214 (`src/libs/kt-fx/tests/audio_graph.rs`)

#### Scenario: A compressor keyed by its own filtered copy
- **WHEN** a compressor's key socket takes a high-pass arm of its own input, on a signal with a loud bass and a quiet hi-hat
- **THEN** the gain reduction follows the hi-hat, not the bass; the key from another track still works as before
- **TEST** t3215 (`src/libs/kt-fx/tests/audio_graph.rs`)

#### Scenario: Slowed in a sound arm reads its own window
- **WHEN** an event sums its dry source with an arm holding Slowed at 0.5×
- **THEN** the arm equals the event whose chain is that arm's path, sample for sample; the dry side is today's line; two runs give the same bytes
- **TEST** t3229 (`src/libs/kt-engine/tests/audio_graph.rs`)

#### Scenario: Tails and state follow the graph
- **WHEN** a reverb stands in one arm and a dangling node in another; the arm is rewired; a value is edited
- **THEN** the tail is the reverb's; the dangling node does not ring; the rewire rebuilds as a reorder does, the value edit keeps the state
- **TEST** t3216, t3217 (`src/libs/kt-engine/tests/audio_graph.rs`)
