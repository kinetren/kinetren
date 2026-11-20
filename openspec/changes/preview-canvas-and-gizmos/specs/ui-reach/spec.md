## ADDED Requirements

### Requirement: Effect handles come from declared kinds of rows
A passport row MAY declare an on-screen handle kind — a radius around a named centre, an angle around a named
centre, or a corner of a quad — and the preview SHALL draw and accept such handles for the selected slot, each
gesture one undo step editing keys on a curved row and giving the number the form shows; the zoom steps, Fit and
the pan SHALL be rows of the controls table.
Why: F-UI-011 trick 2 — handles from types, so every effect is controllable on screen.

#### Scenario: Twirl and Corner Pin by their handles
- **WHEN** a Twirl slot is selected and its radius circle and angle dial are dragged; a Corner Pin slot's four corners are dragged onto a quad
- **THEN** the radius and angle rows change by the dragged amounts and the picture follows; the four corner rows match the dragged points; each drag is one undo step
- **PATH** select the event → effects window → the Twirl slot → a crosshair, a circle and a dial appear on the preview → drag the circle's rim outward → the twirl widens, the radius field follows → drag the dial → the twist grows → select the Corner Pin slot → four corner handles → drag each onto the screen's corners in the picture → the layer is pinned there
- **TEST** t3598 (`src/apps/kt-app/src/reach_tests.rs`)
