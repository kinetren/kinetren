## ADDED Requirements

### Requirement: The wand is used by clicks on the preview
The preview's right-click Mask menu SHALL offer "Magic wand"; its click SHALL become the first seed;
Shift-click SHALL add a seed and Alt-click a subtract seed; dragging from a seed SHALL set the tolerance
with the paused frame's result shown while dragging; the mask's form SHALL show the wand's rows; a seed's
menu SHALL offer linking it to a tracker point.
Why: UI rules 13, 17 and 22 — the masks' place and gestures.

#### Scenario: Select the sky by mouse
- **WHEN** the preview is right-clicked on the sky → Mask → "Magic wand", the mouse is dragged from the seed to widen the tolerance, a cloud is Alt-clicked, and the matte view is turned on
- **THEN** the sky region appears as a mask; dragging grows it live on the paused frame; the cloud is removed; the matte view shows the sky white with a soft edge
- **PATH** right click on the sky in the preview → "Mask" → "Magic wand" → the sky outline appears → drag from the seed outward → the region grows while dragging → Alt-click on a cloud → the cloud leaves the region → click the matte view toggle → white sky, black rest
- **TEST** t3539 (`src/apps/kt-app/src/mask_tests.rs`), planned live scenario `wand` (t3540)
