## ADDED Requirements

### Requirement: A camera with a history is a stream stage whose bytes do not depend on the path to them
A slot that keeps a history by its values SHALL be hosted as a stream stage over the event's project frames, its anchors counted from the event's cut, and
SHALL give a frame the same bytes whether it is reached by steady play, a seek, reverse play, a fresh process or an export. An event that continues the take
of the one before it SHALL take its warm-up from it. Steady forward play SHALL cost one step per frame and a seek at most the warm-up plus the anchor grid.
Why: invariants 7 and 11; a split of one take changes nothing a person cut.

#### Scenario: Steady, seek, reverse and export agree
- **WHEN** a slowed and a reversed event with the camera's 3DNR on are composed at frame 100 by steady play from frame 0, by a cold seek, by reverse play from 140, in a fresh process and by an export
- **THEN** the five frames are byte-equal on one device; the seek walked at most W + G frames; steady play ran one step per frame
- **TEST** t3546 (`src/libs/kt-engine/tests/temporal_stage.rs`)

#### Scenario: The anchor change is invisible
- **WHEN** a static noisy scene is composed across an anchor boundary with α = 0.9
- **THEN** the frames on both sides differ from an uncut recursion run from frame 0 by at most one code value, and from each other by no more than the noise of the source
- **TEST** t3547 (`src/libs/kt-engine/tests/temporal_stage.rs`)

#### Scenario: A split does not restart the filter
- **WHEN** an event with the camera's 3DNR is split in the middle, then the right half's camera rows are changed, then the halves are moved apart
- **THEN** after the split every frame equals the whole event's; after the change and after the move the right half starts cold at its cut
- **TEST** t4253 (`src/libs/kt-engine/tests/temporal_stage.rs`)
