## ADDED Requirements

### Requirement: The preview zooms and pans past the frame, and its handles live on a pasteboard
The preview SHALL show the canvas at Fit or at a chosen zoom from 25 % to 400 %, and pan it, without
recomposing: the presented picture is drawn scaled. Around the canvas a pasteboard SHALL hold the transform
frame, its rotation handle, effect handles and mask vertices wherever they fall, grabbable there; the canvas
rectangle for the picture and every handle SHALL come from one viewer transform. Zooming and panning on a pause
SHALL cost no composite and no decode.
Why: the owner's B9 — "нет возможности отзумиться в превью чтобы поправить большой зум, или задать поворот".

#### Scenario: A 3× pan is edited at 33 %
- **WHEN** an event's Geometry zoom is 3 (its frame far larger than the canvas), the preview is zoomed out to 33 % with Ctrl+wheel, the corner handle and the rotation handle are dragged on the pasteboard, then Ctrl+0
- **THEN** the whole frame and its handles are visible around the small canvas; the drags change the zoom and the angle by the same numbers the form shows, one undo step each; Ctrl+0 returns to Fit; no composite or decode was counted for the zoom and pan
- **PATH** select the zoomed event → its frame runs off the preview → Ctrl+wheel down over the preview → the canvas shrinks to 33 %, grey around it, the frame and handles visible → drag a corner handle → the zoom changes, the field follows → drag the rotation handle → the picture turns → Ctrl+0 → Fit
- **TEST** t3597 (`src/apps/kt-app/src/layout_tests.rs`), t3599 (`src/apps/kt-app/src/idle_tests.rs`)
