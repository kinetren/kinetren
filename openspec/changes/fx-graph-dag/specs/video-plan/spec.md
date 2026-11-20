## ADDED Requirements

### Requirement: A branching chain is lowered by the one chain planner into a straight program
A chain whose slots name their inputs (a branch, a Merge) SHALL be planned by `plan_chain` and by
nothing else: it SHALL lower the graph, in slot order, into the chain's ops plus `Hold` (keep the
current frame), `Load` (make a held frame current) and `Merge`, and every such position SHALL be a
segmentation stop, so that geometry series, point-wise runs and K× barriers fuse only within the
linear runs between them and the plan of a run equals the plan of a line made of the same slots. The
GPU SHALL execute the lowered program as it executes a chain; at most `MAX_HELD` frames SHALL be
held at once. A chain whose every slot takes the slot before (a line) SHALL lower to exactly the
ops, keys and bytes it had before branches existed. The warp, the stream stage, the leading Geometry
node and the decode scale step SHALL belong to the trunk (the slots before the first branch): a layer
reads one source at one material moment, so the frames asked, the coverage bars and the stream
history are those of the line. The Viewer SHALL cut a chain to the chosen node's ancestors.
Why: invariant 7 — one composition path; a second planner for graphs would be "the preview shows one
thing, the export writes another". The trunk keeps invariants 4, 5 and 8 unchanged.

#### Scenario: A line is the chain, byte for byte
- **WHEN** every chain of the golden projects and the showcase is lowered and executed with an empty input field and through the graph path
- **THEN** the ops, the plan keys, `durable_hash` and the frames are equal to the line's; GOLDEN_VIDEO holds
- **TEST** t3200 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: Highlights blurred and laid back, in the preview and the export
- **WHEN** an event's graph is Source → luma key → blur → Merge(Screen) over Source, composed for the preview cold and warm, by a fresh engine and in an export at the same moment
- **THEN** the four frames are byte-for-byte equal on one device, within the composite's tolerance of a reference computed from the two layers of the Merge law, and a second run gives the same bytes
- **TEST** t3201 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: A matte made from the picture
- **WHEN** a Merge mixes a blurred arm over the source through the luma of a third arm, with mix 1 and 0.5, and an arm of another size
- **THEN** the dark areas show the source and the bright the blur in proportion; mix 0.5 halves the coverage; the Foreground is fitted to the Background's size
- **TEST** t3202 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: A run between stops is planned as its line
- **WHEN** graphs with geometry series, a K× barrier and point-wise runs inside arms are lowered
- **THEN** each run's plan equals the plan of the same slots as a line; a stop zeroes the K accumulators; a merge runs at 1×
- **TEST** t3203 (`src/libs/kt-fx/tests/geo_fuse.rs`)

#### Scenario: The trunk keeps reading as the line
- **WHEN** a slowed event with a zooming lead branches after the lead, and the same event branches before any geometry
- **THEN** the frames asked by the order equal the line's; the first keeps its decode step, the second gets step 1; a mask on a slot inside an arm takes the slot's own input as its dry side
- **TEST** t3205 (`src/libs/kt-engine/tests/decode_scale.rs`), t3206 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: The keys see the wiring
- **WHEN** an arm is rewired from one node to another with the same slots and values
- **THEN** `fx_chain_hash`, the GPU plan key and `durable_hash` move; a line's keys equal the keys before this change
- **TEST** t3208 (`src/libs/kt-engine/tests/fx_auto_render.rs`)

### Requirement: A Merge is the layer composite's operation — the Merge law
A Merge node SHALL fit its Foreground to its Background's size and blend it by the layer blend
modes' own kernels (`Normal` = `alpha_over`, the others `blend_over`), then mix the result with the
Background by `mix` and an optional matte through the layer-strength mix pass. An event whose graph
merges arm B over arm A with mode M at mix 1 SHALL render the picture of two copies of the event on
adjacent tracks, the lower with chain A and the upper with chain B and blend mode M, both at full
level. A graph SHALL NOT take an input from another layer.
Why: one set of kernels called from two places, not two composition codes (invariant 7); the
compositions' law E is the precedent.

#### Scenario: One event with a Merge equals two stacked layers
- **WHEN** for each of the nine modes an event merges a graded arm over a blurred arm, and the two-layer stack of the same chains with that blend mode is composed
- **THEN** the frames are byte-for-byte equal on one device
- **TEST** t3204 (`src/libs/kt-gpu/tests/parity.rs`)
