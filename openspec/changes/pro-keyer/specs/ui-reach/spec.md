## ADDED Requirements

### Requirement: The Pro Keyer is tuned by stages on its panel
The Pro Keyer's window SHALL show its stages as a pipeline (Plate, Screen, Core, Edge, Shadow, Spill, Out),
each with its rows and an eye that shows that stage in the Viewer; "Analyze" SHALL set the screen and clips
from five frames in one undo step; "Use this frame as the plate" SHALL hold the frame under the playhead.
Why: F-MSK-004 trick 1 (a key is tuned by sight); the device-like panel rule.

#### Scenario: Key an uneven shot by mouse
- **WHEN** Pro Keyer is added, "Analyze" pressed, the Edge stage's eye clicked, the edge band widened, the playhead moved to a subject-free moment and "Use this frame as the plate" clicked
- **THEN** the key is set and the status line says what; the preview shows the edge matte; widening the band brings back hair; the plate stage shows the held frame and the corners clean up
- **PATH** effects window → picker → "Pro Keyer" → the pipeline panel → click "Analyze" → the background goes → click the eye on "Edge" → the preview shows the edge matte → drag "Edge band" right → hair appears → move the playhead to an empty frame → click "Use this frame as the plate" → the Plate tile shows it → click the eye on "Out" → the composite
- **TEST** t3589 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `pro_keyer` (t3590)
