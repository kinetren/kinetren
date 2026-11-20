## ADDED Requirements

### Requirement: A temporal slot is a history stage whose bytes do not depend on the path to them
The plan SHALL host a temporal slot through the same history machinery as a stream stage — the memo, its
anchor checkpoints, walks after a seek, the slots in front of it as its input, pacing and Loading, kept
composites by the kernel's key, the export's pre-roll — over the MATERIAL frames of the event's media, and
SHALL produce for a frame the same bytes whether it is reached by steady play, a seek, reverse play, a
fresh process or an export. Steady forward play SHALL cost one recursion step per frame and a seek at most
the warm-up plus the anchor grid.
Why: invariants 7 and 11; the history must not depend on where playback started.

#### Scenario: Steady, seek, reverse and export agree
- **WHEN** a slowed and a reversed event with the camera's 3DNR on are composed at frame 100 by steady play from frame 0, by a cold seek, by reverse play from 140, in a fresh process and by an export
- **THEN** the five frames are byte-equal on one device; the seek walked at most W + G frames; steady play ran one step per frame
- **TEST** t3546 (`src/libs/kt-engine/tests/temporal_stage.rs`)

#### Scenario: The anchor change is invisible
- **WHEN** a static noisy scene is composed across an anchor boundary with α = 0.9
- **THEN** the frames on both sides differ from an uncut recursion run from frame 0 by at most one code value, and from each other by no more than the noise of the source
- **TEST** t3547 (`src/libs/kt-engine/tests/temporal_stage.rs`)
