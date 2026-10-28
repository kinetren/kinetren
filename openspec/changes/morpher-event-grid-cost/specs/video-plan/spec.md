# Spec Delta

## ADDED Requirements

### Requirement: A strength slot at zero skips the layer, and the frame cost is counted
On a frame where the layer's strength slot evaluates to exactly 0, the plan executor SHALL
NOT execute the chain slice between the slot's snapshot and the slot. It SHALL return the snapshot (the dry
side), which is byte for byte what the mix at 0 gives today, on the CPU and on the GPU. The
plan executor SHALL count, for each composed frame, the passes run, the sub-samples taken (the
pure sample cost of each geometry series), the snapshots and the mixes. It SHALL also count the
compose time. The counters are integers in the frame path: no allocation, no lock, and the
same counting on both backends.
Why: ktprompts v26.10.18 item 8. Where a block's fades and gates put the layer's strength at
0, the executor still computed the whole event grid and then blended it away
(`src/libs/kt-engine/src/video.rs`, the `LayerMix` slices; the GPU Snapshot/Mix ops). There
was no number for "how much does this frame cost", so the question "do we compute extra?"
could only be answered by reading the code.

#### Scenario: Strength zero computes nothing of the layer
- **WHEN** an event with its own effect is covered by a layer whose chain is an event grid 12×30, with strength 0, 0.5 and 1.0 on successive frames, on the CPU and on the GPU
- **THEN** at 0 the frame equals the frame without the layer byte for byte (as t267 says) and the counters show zero passes and zero sub-samples for the layer's slice; at 0.5 and 1.0 the frames are unchanged from today; the counters at 1.0 show no snapshot and no mix
- **TEST** t945 (`src/libs/kt-engine/tests/morph_slice_render.rs`)
