## ADDED Requirements

### Requirement: A stream kernel stops between history positions when nobody wants the frame
`StreamInput` SHALL offer `stop()` (default `false`, additive to the plugin API). A stream kernel SHALL
ask it before each history position it computes and, when it answers `true`, return `None` with what it
computed kept in the memo, so a later render resumes there and gives the same bytes. The host answers
from a stop check its caller set: the preview's compose thread stops a frame that a newer ask or an
edit replaced, the prefetch thread an advance the playhead or an edit left, a reel strip's or a poster's
pre-roll a cancelled job. An export never stops.
Why: row 147 — a composite the UI had left ran its stream history to the end (up to 300 frames of
compute jobs): in live `ui_wait` the preview's thread held 4 cores for seconds after the pause.

#### Scenario: A history stops within one position and resumes to the same bytes
- **WHEN** a Datamosh history of 40 positions is rendered with a stop that turns true after the 5th compute job, then rendered again with no stop, and a third time in a fresh memo with no stop
- **THEN** the first render returns `None` after at most one more position, the memo holds what it computed, the second finishes from there, and its bytes equal the third's
- **TEST** t2257 (`src/plugins/fx-video-codec/tests/stop.rs`)

#### Scenario: The preview stops a history it left
- **WHEN** a frame's long stream history runs under the compose thread's stop check, and the check turns true (a newer ask, an edit)
- **THEN** the history stops before its end, the frame is incomplete, the stop is traced as the compose thread's, the memo keeps what was computed, and the next composite resumes to the bytes of an uninterrupted one
- **TEST** t2258 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The look-ahead stops when the playhead leaves
- **WHEN** the prefetch thread's advance of a long history runs under its stop check, and the check turns true (a seek, an edit)
- **THEN** the advance stops at the next position, says it is still pending, and the trace marks the advance stopped; a later advance finishes it
- **TEST** t2259 (`src/libs/kt-engine/tests/decode_storm.rs`)
