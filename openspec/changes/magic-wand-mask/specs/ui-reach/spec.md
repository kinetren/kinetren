## ADDED Requirements

### Requirement: The wand is used by clicks on the preview
The preview's Mask menu and an event's mask lists SHALL offer "Magic wand"; with it armed a click SHALL make a mask with
that seed, further clicks add seeds, Ctrl+click adds a subtract seed, Backspace drops the last one and Esc ends; a seed
SHALL be moved by dragging it and the tolerance set by dragging the ring around the selected seed, one undo step a
gesture, keyed where the row has a curve. The region's outline SHALL be shown over the paused frame and follow the gesture.
The mask's row SHALL show tolerance, "every matching pixel", the colour space and "fill holes". The item SHALL be absent
from a track's list and refused with its reason on a composition instance.
Why: UI rules 13, 17, 22 — the masks' place and gestures.

#### Scenario: Select the sky by mouse
- **WHEN** the preview is right-clicked on the sky, Mask → "Magic wand" chosen and the sky clicked, the ring dragged outward, a cloud Ctrl+clicked, then Ctrl+Z pressed
- **THEN** a mask with one seed appears and its outline runs along the horizon; the tolerance row and the outline grow while dragging; the cloud leaves the region; undo brings it back; the whole gesture never waits for a picture
- **PATH** right click on the sky → "Mask" → "Magic wand" → the cursor is the tool's → click on the sky → an outline along the horizon, a seed with a ring → drag the ring outward → the outline grows into the haze, "Tolerance" follows in the mask's row → Ctrl+click a cloud → the outline goes round it → Ctrl+Z → the cloud is back inside
- **TEST** t3539 (`src/apps/kt-app/src/mask_tests.rs`), t3540 (live scenario `wand`)
