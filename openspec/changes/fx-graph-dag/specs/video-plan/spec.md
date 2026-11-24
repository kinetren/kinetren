## ADDED Requirements

### Requirement: A branching chain is lowered by the one chain planner into a straight program
A chain whose slots name their inputs (a branch, a Merge) SHALL be planned by `plan_chain` and by
nothing else: it SHALL lower the graph, in slot order, into the chain's ops plus `Hold` (keep the
current frame), `Load` (make a held frame current) and `Merge`, and every such position SHALL be a
segmentation stop, so that geometry series, point-wise runs and K× barriers fuse only within the
linear runs between them and the plan of a run equals the plan of a line made of the same slots. The
GPU SHALL execute the lowered program as it executes a chain; at most `MAX_HELD` frames SHALL be
held at once. A chain whose every slot takes the slot before (a line) SHALL lower to exactly the
ops, keys and bytes it had before branches existed. The leading Geometry
node SHALL stand before the first branch. A graph without a time kind in an arm SHALL read one source
at one material moment, so the frames asked, the coverage strip and the stream history are those of
the line. The Viewer SHALL cut a chain to the chosen node's ancestors.
Why: invariant 7 — one composition path; a second planner for graphs would be "the preview shows one
thing, the export writes another".

#### Scenario: A line is the chain, byte for byte
- **WHEN** every chain of the golden projects and the showcase is lowered and executed with an empty input field and through the graph path
- **THEN** the ops, the plan keys, `durable_hash` and the frames are equal to the line's; GOLDEN_VIDEO holds
- **TEST** t3200 (the goldens GOLDEN_VIDEO, t26, t306 in `src/libs/kt-gpu/tests` and `src/libs/kt-engine/tests/golden_export.rs`, unchanged)

#### Scenario: Highlights blurred and laid back, in the preview and the export
- **WHEN** an event's graph is Source → blur → Merge(Screen) over Source through the Source's luma, composed cold and warm, by a fresh renderer, and by the testkit's one-frame chain
- **THEN** the four frames are byte-for-byte equal on one device, and differ from the plain and the blurred picture
- **TEST** t3201 (`src/libs/kt-engine/tests/fx_graph_render.rs`)

#### Scenario: A matte made from the picture
- **WHEN** a Merge mixes a blurred arm over the source through the source's luma, with mix 1 and 0.5, and through the inverted luma
- **THEN** every byte is the integer mix of the source and the blur by the coverage luma × mix; the inverted luma gives the complement
- **TEST** t3202 (`src/libs/kt-engine/tests/fx_graph_render.rs`)

#### Scenario: A run between stops is planned as its line
- **WHEN** graphs with geometry series, a K× barrier and point-wise runs inside arms are lowered
- **THEN** each run's plan equals the plan of the same slots as a line; a stop zeroes the K accumulators; a merge runs at 1×
- **TEST** t3203 (`src/libs/kt-fx/tests/graph_plan.rs`)

#### Scenario: A graph with no time kind in an arm reads as the line
- **WHEN** a slowed event with a zooming lead branches after the lead, and the same event branches before any geometry
- **THEN** the frames asked by the order equal the line's; the first keeps its decode step, the second gets step 1; a mask on a slot inside an arm takes the slot's own input as its dry side, a masked slot of a read's line cuts inside the read, and the picture's masks cut before the branch (within one level at their soft pixels: a Merge keeps straight 8-bit alpha)
- **TEST** t3205, t3206 (`src/libs/kt-engine/tests/fx_graph_render.rs`)

#### Scenario: A register is used again, and the arms are prewarmed
- **WHEN** a chain of five merges one after another is composed, one of its arms making no pass; a chain with two arms of point-wise effects is prewarmed
- **THEN** the picture is that of the arms computed apart and the same twice; a held texture goes back to the pool when its register takes another value; the run shapes are each arm's, the ids hold the Merge, a dangling slot brings none
- **TEST** t3243 (`src/libs/kt-engine/tests/fx_graph_render.rs`), t3242 (`src/libs/kt-engine/tests/prewarm_reach.rs`)

#### Scenario: The keys see the wiring
- **WHEN** an arm is rewired from one node to another with the same slots and values
- **THEN** `fx_chain_hash`, the GPU plan key and `durable_hash` move; a line's keys equal the keys before this change
- **TEST** t3208 (`src/libs/kt-engine/tests/fx_graph_render.rs`)

### Requirement: A time kind inside an arm is a second read of the layer
A warp, a stream stage or an edge reading standing in an arm (not before the first branch) SHALL open
a read: a derived clip of the same event whose chain is that slot's ancestors with the slot as its
Output, reading exactly as an event with that chain would (its own material moment, decode step,
stream history keyed by event and read). The layer's program SHALL take the read's finished frame in
place of that slot's output. The order (`layers_at`, `wanted_frames`) SHALL list every read's frames,
the event's coverage strip SHALL draw a row per read and show the event warm only where every read is,
and a layer SHALL have at most `MAX_READS` reads.
Why: the owner's answer Q3 b (kkt-26.11.20 A6), "Разрешить в ветке" at the cost of a second read; a
read shaped as today's clip keeps invariants 4, 5 and 8 per read instead of inventing a second
reading machine.

#### Scenario: The read law
- **WHEN** an event merges an arm holding Slowed (0.5×) over an arm holding a blur, and separately two copies of the event are stacked, the upper with the Slowed path as a line, the lower with the blur
- **THEN** the frames are byte-for-byte equal on one device, in the preview and in the export
- **TEST** t3226 (`src/libs/kt-engine/tests/fx_graph_render.rs`)

#### Scenario: The order and the strip see both reads
- **WHEN** a slowed arm is added to an event that reads frames 0..48 on its trunk
- **THEN** `wanted_frames` lists the trunk's frames and the arm's half-rate frames; the prefetch warms both and the composite misses none; the strip draws two rows; removing the arm returns the line's order exactly
- **TEST** t3227 (`src/libs/kt-engine/tests/fx_graph_render.rs`), t3299 (`src/apps/kt-app/src/node_editor_tests.rs`)

#### Scenario: Datamosh in an arm keeps its own history
- **WHEN** Datamosh stands both on the trunk of one event and in an arm of another event over the same file, played across a seek
- **THEN** each keeps its own history keyed by event and read; the arm's frames equal those of an event whose chain is the arm's path (the event before the cut and the slot in front included), after playing and after a seek; the trunk's edit asks no history again; the run ahead, the checkpoints, the memo's want and a position's bytes count the read's stage, two stages as two
- **TEST** t3228 (`src/libs/kt-engine/tests/stream_stage.rs`)

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
- **TEST** t3204 (`src/libs/kt-engine/tests/fx_graph_render.rs`)
