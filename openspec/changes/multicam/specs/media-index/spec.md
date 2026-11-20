## ADDED Requirements

### Requirement: Angle cells decode below everything else
The angle viewer's cells SHALL compose each angle through the one plan at a coarse size, after the
program frame and at the background class, and ask their decodes at the lowest priority with a
look-ahead no longer than half a second; a starved cell SHALL show its previous frame dimmed and SHALL
never delay the program frame; on a pause each cell SHALL be composed once per moment.
Why: invariants 4, 5 and 12 — N cells must not starve the program picture or heat the system.

#### Scenario: Cells starve before the program picture
- **WHEN** a 16-angle multicam plays with the viewer open on a throttled decoder
- **THEN** the program frame's V-fps equals the run without the viewer within 5 %; cells drop frames instead; on a pause each cell composes once and no further work is counted
- **TEST** t3510 (`src/libs/kt-engine/tests/multicam.rs`)
