## ADDED Requirements

### Requirement: "Follow the outer speed change" from a parameter's link menu
A parameter's link menu SHALL offer "Follow the outer speed change", "Follow the outer speed" and "Follow the outer
acceleration", each writing a link with the default mapping in one undo step, with attack, release and full-at shown on
the link's row.
Why: the owner's TD2 — the container is known, nothing to pick.

#### Scenario: Wake a fisheye on the composition's speed changes
- **WHEN** inside a reel the person opens the Lens strength's link menu, chooses "Follow the outer speed change", leaves the reel and plays the braking instance
- **THEN** the strength row reads "linked: outer speed change"; the fisheye rises where the instance brakes and fades after
- **PATH** enter the «Letters» reel → effects window → right click the Lens "Strength" label → "Follow the outer speed change" → the row reads "linked: outer speed change" → Esc to the root → Space over the braking instance → the fisheye rises and fades
- **TEST** t3951 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `motion_link`
