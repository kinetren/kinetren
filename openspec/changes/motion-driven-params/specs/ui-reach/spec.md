## ADDED Requirements

### Requirement: "The speed it is played at" in a curve's "Driven by…" menu
A curve's "Driven by…" menu SHALL offer "The speed it is played at" with "Speed change", "Speed" and "Acceleration", each writing the source with its
default formula in one undo step; the source's name SHALL stand in the curve's legend, the attack, release and full-at beside its formula; a row that cannot SHALL say why.
Why: the owner's TD2 — the container is known, nothing to pick.

#### Scenario: Wake a fisheye on the composition's speed changes
- **WHEN** inside a reel the person opens the Lens strength curve's "Driven by…" menu, chooses "The speed it is played at" → "Speed change", leaves the reel and plays the braking instance
- **THEN** the legend reads "Strength ← speed change"; the fisheye rises where the instance brakes and fades after, though the composition stands
- **PATH** enter the «Letters» reel → effects window → the Lens "Strength" curve → "Driven by…" → "The speed it is played at" → "Speed change": the legend reads "Strength ← speed change" → Esc to the root → Space over the braking instance: the fisheye rises and fades
- **TEST** t3951 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `motion_link`
