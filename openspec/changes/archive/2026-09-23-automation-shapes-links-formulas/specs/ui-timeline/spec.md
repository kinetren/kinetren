# Spec Delta

## ADDED Requirements

### Requirement: Shapes, derived graph and formula in the automation widget
The segment shape SHALL be edited from the same automation widget as the keys: by a right click on the SEGMENT between two keys (neighboring segments are not touched), with the shape's parameters shown next to it; the same actions are available for a selection of several segments. The graph SHALL draw the shape as it sounds and looks in the render, by the same value code — there is no second drawing formula. A curve with a DERIVED source (a link or a formula) SHALL be drawn distinguishably from a keyed one and MUST NOT be edited with the mouse: it has no keys, and dragging a point would silently create a third truth about the value. Such a curve SHALL show what drives it — the source address or the formula text — and have one "unlink" action that turns it into ordinary keys at the current values. The formula text is entered in a field with on-the-fly checking: a parse error is shown with its position and is not applied to the model. The default form and the "apply to all" command are reachable from the same place as the other actions on the curve. All new controls SHALL have a hint in the status line.
Why: the owner asked that "the resulting graph is also drawn, but not edited with the mouse"; an editable derived graph would be exactly the "mute dead end" for which the project paid seven rounds.

#### Scenario: The shape is edited per section, neighbours intact
- **WHEN** a shape is chosen by a right click on a segment, then its parameter is edited, then the shape is applied to all segments of the curve
- **THEN** only the chosen segment changes and only its parameters; the graph is redrawn by the same value function the render uses; "to all" is one undo step
- **TEST** t698 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A derived curve is visible but not edited with the mouse
- **WHEN** the handle's source is a link; a drag and a double click are made on its line; then "unlink" is pressed
- **THEN** the line is drawn distinguishably and labelled with its source; neither the drag nor the double click changed the model; "unlink" gave ordinary keys at the current values in one step
- **TEST** t706 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The formula field explains the error and does not spoil the model
- **WHEN** text with an unclosed parenthesis is entered in the formula field, then one with an unknown name, then a correct one
- **THEN** the first two show an error with a position in the status line and do not touch the model; the correct one is applied; the field has a hint in the status line
- **TEST** t709 (`src/apps/kt-app/src/layout_tests.rs`)
