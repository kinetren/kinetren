## MODIFIED Requirements

### Requirement: An impossible action is an error; paired ways are identical; names say what and where
A gesture's stop (neighbour, occupied place) SHALL be shown as an error: the obstacle
is highlighted in red, a hint in error style overrides hover, and if the obstacle lies
outside the view — the side, the distance and a red arrow at the edge of the row. The obstacle
lives for exactly one frame. An action available both by gesture and by menu item SHALL have one
body and take the context into account the same way. The menu item name SHALL say WHAT and WHERE
will happen, and remain distinguishable in every language.
Why: "the hint is pale and faded… everywhere such situations are made as
errors"; "marker here with a selected region must produce a region marker";
"Remove crossfade… turns out to be the fadeout from the end that removes it". Two paths to
one action SHALL also share one catalog name, with the chord in the chord column, not in the
translated text: the Edit menu's "Split / cut region" promised a cut-out that neither it nor `S`
ever did (owner's decision 2026-10-26, change `split-menu-label`: rename, keep the action).

#### Scenario: Stop against a neighbour outside the view
- **WHEN** an event is moved onto an occupied place, the obstacle is not visible
- **THEN** the hint in error style names the side and the distance, a red arrow at the edge of the row; a visible obstacle is highlighted
- **TEST** t172 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: "Marker here" = gesture M
- **WHEN** a region is selected and the menu item "Marker here" is invoked (including a click on the region edge)
- **THEN** a range marker is placed over the region, as with gesture M; the item name changes with the context
- **TEST** t170 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Transition menu items are distinguishable in all languages
- **WHEN** the labels "remove fade / crossfade" and "fade in / fade out" are read in each catalog
- **THEN** the four actions (fade/crossfade × our start/our end) have different labels, and "at the start / at the end of the event" is present
- **TEST** t171 (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: Overlap transition is reachable by the same gestures
- **WHEN** right-click on the overlap zone of two events; double-click on the window of an overlapping transition
- **THEN** the menu offers "Transition from…" (the neighbour ladder is equal to the joint one), double-click opens the properties
- **TEST** t466 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The Edit menu's split is the event menu's split and the S key
- **WHEN** a region is drawn and the playhead stands inside an event with nothing selected; Edit → "Split at playhead" is clicked; the edit is undone and `S` is pressed instead; the label and chord of the Edit item and of the event menu's split item are read in each of the five catalogs
- **THEN** both paths split the same events at the same tick and leave the region's interior in place; the Edit item shows the event menu's label with `S` in the chord column; no catalog's split label contains a chord; the toolbar scissors button's hint is the split label followed by its key
- **PATH** a region over part of an event, the playhead inside the event → Edit menu → the item reads "Split at playhead" with S on the right → click → the event is cut in two at the playhead, the region still covers both halves → Ctrl+Z → the event is whole again → S → the same cut at the same place
- **TEST** t870 (`src/apps/kt-app/src/layout_tests.rs`)
