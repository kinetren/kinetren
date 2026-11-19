## ADDED Requirements

### Requirement: Marker over the region is always new; stretching is its own item
The region-edge menu's "Marker over the region" SHALL add a new range marker over the region and
SHALL NOT change a marker that stands at the region's start: an equal range SHALL be removed (the
toggle of the M gesture), a different marker SHALL stay and the refusal SHALL name "Stretch the marker
over the region". That item SHALL be offered only when a marker at the region's start has another
length, and SHALL give it the region's length, keeping its name.
Why: the owner, `v26.10` line 33 — "это полезно, надо вынести в отдельный пункт … а этот пункт надо
чтобы всегда … создавал новый, а не менял текущий"; his answer of 2026-11-07 (в.4 "а").

#### Scenario: New, refused, stretched, toggled
- **WHEN** a region 30 000–90 000 ticks; right click on its start edge → "Marker over the region"; then a point marker stands at 30 000 → the same item; then → "Stretch the marker over the region"; then the menu is opened again and "Marker over the region" is clicked
- **THEN** the first click adds a 60 000-tick range and no stretch item is offered; over the point marker the stretch item is offered, the new item leaves the point marker and the notice names the stretch item; the stretch item gives the marker the region's length and is then not offered; the last click removes the equal range
- **PATH** draw a region → right-click its edge on the ruler → "Marker over the region" → a range marker spans the region → put a point marker at the region's start → right-click the edge → two items now → "Marker over the region" → the status line says a marker is already there and which item changes it → right-click → "Stretch the marker over the region" → the point marker spans the region
- **TEST** t3104 (`src/apps/kt-app/src/layout_tests.rs`)
