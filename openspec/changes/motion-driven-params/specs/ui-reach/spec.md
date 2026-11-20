## ADDED Requirements

### Requirement: "Follow speed change of…" from a parameter's link menu
A parameter's link menu SHALL offer "Follow speed change of…", which picks an event, a composition instance, a morpher
block on the timeline or the playhead, and writes a link with the default mapping in one undo step; the playhead
entry and a link to it SHALL be marked "preview only".
Why: the owner's TD2; rule 16 — a difference from the render is said, not hidden.

#### Scenario: Wake a fisheye on a composition's speed changes
- **WHEN** the person opens the Lens strength's link menu, chooses "Follow speed change of…", clicks the composition on the timeline and plays
- **THEN** the strength row shows the link "speed change of «Letters»" with attack and release; the fisheye grows during the composition's braking and fades after
- **PATH** effects window → right click the Lens "Strength" label → "Follow speed change of…" → the cursor becomes a picker → click the «Letters» composition on the timeline → the row reads "linked: speed change of «Letters»" → Space → the fisheye rises where the letters brake and fades out
- **TEST** t3950 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `motion_link`
