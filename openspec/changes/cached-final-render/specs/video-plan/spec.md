## ADDED Requirements

### Requirement: A picture key names a frame by what its kernels are given
`picture_key` SHALL hash the frame's size, its layers' roles and weights, each leaf's name and source time, the chains as their curves
resolved them with their wiring, the masks, cameras and leads as evaluated, and a subtree by its own plan. It SHALL hash the timeline tick only where a slot or a
transition of the plan reads time, and the event's progress only where a slot reads progress; it SHALL hash neither an event's start, length or
`source_in` nor a reel's revision. A plan with a leaf that has no durable name SHALL have no picture key. Whether a plugin reads time or
progress is a fact of the plugin, yes unless it says no.
Why: cached-final-render D2 — the moment's key holds its place, so a trimmed event or a ripple renamed every frame after it and nothing of an earlier render could be used
(`v26.10` X1: "мелкая оплошность (склейка кривая обнаружена)").

#### Scenario: Every field that makes another picture moves the key
- **WHEN** each field a kernel is given is changed alone on a plan of files, a title, a reel, a transition, masks, a camera and a graph, and each plugin that says it reads no time (no progress) is cooked at three ticks (three progresses) over its passport's corners
- **THEN** the key changes for every field; every such plugin's parameters, taps and primitives are byte-equal
- **TEST** t3335 (`src/libs/kt-engine/tests/cached_render.rs`)

#### Scenario: A shot that moved keeps its key
- **WHEN** an event's tail is trimmed by two frames and everything after is moved earlier, in a project of colour-graded shots, a title, and one shot with film grain
- **THEN** every remaining frame of the trimmed shot and of the moved shots has its former picture key at its new place and the same composed bytes; the grain's frames have new keys
- **TEST** t4139 (`src/libs/kt-engine/tests/cached_render.rs`)

### Requirement: An offline render's frames are named by their sources
Every cache an offline render makes (an export, a reel strip, a poster, the effect cells, kt-cli's measures) SHALL adopt the identity of each source it opens before its first frame, so
the names of its stream positions and its moments are those sources' and no other file's.
Why: found-defects 1068 — the memo of the slots in front of a stream stage is the process's, and an export's positions were named by the media id alone: an
export of another project read the first one's pictures.

#### Scenario: Two exports in one process
- **WHEN** two projects whose media has one id and different files, each with a Posterize in front of Datamosh, are exported one after another in one process, then each in a process of its own
- **THEN** each file equals the one made in its own process byte for byte
- **TEST** t4138 (`src/libs/kt-engine/tests/stream_stage.rs`)
