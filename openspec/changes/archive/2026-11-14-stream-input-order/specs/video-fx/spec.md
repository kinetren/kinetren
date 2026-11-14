## ADDED Requirements

### Requirement: A stream stage reads the output of the slots in front of it
The slots of a chain in front of a `Stream` slot that change the picture SHALL run on every frame of
the main stream (the event's own source and its history in the event before) before the stage's
kernel reads it, with their curves at the position's material time, at the stream's size; the chain
after the stage SHALL NOT run them again. The leading Geometry node stays after the stage (it places
the event's picture on the canvas). The stream's addresses SHALL name what feeds it: an edit of a
slot in front SHALL invalidate the history, an edit of a slot after the stage SHALL NOT. With no
device for the front, the frame SHALL be missing, never the decoded picture in its place. The second
input (another track) is read as decoded (found-defects 540).
Why: found-defects 484 — the order the person built (camera_isp → datamosh → vhs_tape) was not the
order run: the kernel read the decoded frame and Camera ISP ran over the mosh (`stream-input-order`).

#### Scenario: A slot in front feeds the stream
- **WHEN** shot B's chain is posterize → datamosh, composed at a frame inside the mosh
- **THEN** the frame equals the mosh of sources posterized before decoding (both shots, the history in the event before included), the slot ran on the stream's frames, and the frame differs from the chain datamosh → posterize
- **TEST** t2470 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: An edit in front invalidates the history, one after does not
- **WHEN** a frame inside the mosh was composed with the chain posterize(3) → datamosh → posterize(5) on a shared memo, and the order is asked again by renderers of posterize(3) → datamosh → posterize(4) and of posterize(4) → datamosh → posterize(5)
- **THEN** the first asks no frame of shot B, the second the whole history (7 or more)
- **TEST** t2471 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: An export runs the slot in front
- **WHEN** a range with posterize → datamosh is exported in one go and in two pieces, each with a fresh renderer, memo and cache
- **THEN** the pieces equal the whole, and the whole differs from the export of datamosh → posterize
- **TEST** t2472 (`src/libs/kt-engine/tests/stream_stage.rs`)
