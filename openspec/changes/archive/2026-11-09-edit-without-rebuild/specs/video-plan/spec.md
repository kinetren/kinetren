## ADDED Requirements

### Requirement: The preview's video renderer is the shared build's
The preview composer, prefetch and the UI's readers SHALL use the video renderer of the player's
shared build; a video layer reused from the last build SHALL equal the freshly derived one (the plan
key, the decode step and the stream stage included).
Why: the UI thread built a `VideoRenderer` per edit (found-defects row 150, 19–36 ms on the owner's
project), and prefetch built another.

#### Scenario: The video side of an incremental build
- **WHEN** the edits of t2004 are applied in turn
- **THEN** the incremental video build's state text (every layer, the cameras, the stages, the nested reels) equals the fresh build's
- **TEST** t2004 (`src/libs/kt-engine/tests/shared_build.rs`)
