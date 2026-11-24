## ADDED Requirements

### Requirement: Effect handles come from declared kinds of rows
An effect MAY declare that rows of its passport are a centre, a radius, an angle or a quad on the frame; the preview SHALL
draw and accept these handles for the selected slot of an event's, a morpher's and a track's open window, on a pause, each
gesture one undo step editing the key at the cursor where the row has a curve, with the number the form shows. A
declaration naming a row the passport lacks SHALL be refused at registration. Every built-in row that is a place on the
frame SHALL be declared.
Why: F-UI-011 trick 2 — handles from types, so every effect is controllable on screen; the owner's B9 — "нет нормальнго
управления эффектом в превью".

#### Scenario: Twirl and Corner Pin by their handles
- **WHEN** a Twirl slot is selected and its circle and dial are dragged, the dial past a full turn; a Corner Pin slot's four corners are dragged onto a quad; a Vignette on a track is selected in the track's window and its two rings dragged
- **THEN** the rows follow, the twirl's angle past 360°, the corners match the points; the track's chain takes the radius and the softness; each drag is one undo step; during playback nothing is drawn
- **PATH** select the event → effects window → click the Twirl node → a circle and a dial stand on the preview → drag the circle outward → "Radius" follows and the picture twists wider → drag the dial round twice → "Angle" reads past 360 → click the Corner Pin node → four corners → drag each → the picture is pinned there
- **TEST** t3598 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A declaration is checked
- **WHEN** plugins declaring a radius over a missing row, a choice row and a centre that is not declared are registered, and the built-in set is read
- **THEN** each is refused naming the row; the built-in declarations are those of the list in the design, and every smooth row named `center_*`, `radius` on a frame unit, or a corner pair is among them or on the list of exceptions with its reason
- **TEST** t4231 (`src/libs/kt-plugins-builtin/tests/handles.rs`)
