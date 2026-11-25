## ADDED Requirements

### Requirement: The preview zooms and pans past the frame, and its handles live on a pasteboard
The preview SHALL show the canvas at Fit or at a step from 25 % to 400 % of the project's pixels and pan it, without
composing: the presented picture is drawn scaled. Around the canvas a neutral pasteboard SHALL hold the transform frame
with its rotation handle, masks, effect handles and the title's frame wherever they fall, drawn and grabbed there, outlines
and handles only. The canvas's rectangle SHALL come from one function for the picture and every control. The zoom SHALL NOT
be stored: a project opens at Fit. At Fit the preview SHALL be drawn as before. Zooming and panning on a pause SHALL cost no
composite and no decode.
Why: the owner's B9 — "нет возможности отзумиться в превью чтобы поправить большой зум, или задать поворот"; A26 —
"Контуры; не запоминать".

#### Scenario: A 3× pan is edited at 33 %
- **WHEN** an event's Geometry zoom is 3, the preview is zoomed out to 33 % with Ctrl+wheel, a corner handle and the rotation handle are dragged on the pasteboard, then Fit is pressed
- **THEN** the whole frame and its handles stand around the small canvas; the drags change the zoom and the angle to the numbers the form shows, one undo step each; Fit returns the preview
- **PATH** select the zoomed event → its frame runs off the preview → Ctrl+wheel down over the preview → the canvas shrinks to 33 %, grey around it, the frame and the handles visible → drag a corner → the zoom field follows → drag the rotation handle → the picture turns → Shift+0 → Fit
- **TEST** t3597 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `viewer_zoom`

#### Scenario: Nothing is composed for a look
- **WHEN** a paused heavy moment is zoomed through every step, panned by the wheel and a middle drag, and left alone
- **THEN** no composite, no decode request and no compose request is counted, and after the last gesture no frame is drawn
- **PATH** pause on a heavy moment → Ctrl+wheel through the steps → the same picture smaller and larger, no Loading → drag with the middle button → it moves → hands off → nothing is drawn
- **TEST** t3599 (`src/apps/kt-app/src/idle_tests.rs`)

#### Scenario: The point under the pointer stays, and the canvas does not get lost
- **WHEN** Ctrl+wheel zooms in over a corner of the picture, the canvas is panned as far as it goes, the preview is resized and detached, the project is reopened
- **THEN** the project pixel under the pointer is the same at each step within a screen pixel; a tenth of the canvas stays visible in every case; the reopened project is at Fit
- **PATH** Ctrl+wheel up over the picture's corner → that corner grows in place → wheel down → the picture moves down → a middle drag to the right → it stops with a tenth in view → File → Open → the preview is whole
- **TEST** t4229 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A mask and a title are edited outside the canvas
- **WHEN** at 50 % a rectangle mask is drawn from the pasteboard across the picture's edge and a vertex of a path outside the canvas is dragged; a title's block half outside is dragged; at 400 % a title is dragged
- **THEN** the mask's numbers go past the frame as drawn, the vertex moves, the title's position follows the same numbers as at Fit; the title is laid out once a frame at the Fit size
- **PATH** Ctrl+wheel down → right click → Mask → Rectangle → press on the grey, release inside → the mask's outline crosses the edge → open the title's window → drag its frame on the grey → "Position" follows
- **TEST** t4230 (`src/apps/kt-app/src/mask_tests.rs`)
