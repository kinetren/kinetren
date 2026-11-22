## ADDED Requirements

### Requirement: The Physics animator is chosen in the title form
The title form's animation presets SHALL include "Physics" with its behaviour (Sway, Hang, Drag, Fall) and its rows
(stiffness, damping, spread, threshold, gravity) in the one parameter grid; while the form is open, a faint spring line per
letter SHALL show on the preview.
Why: the owner's F2; the title form is where a title's motion is set.

#### Scenario: Make a crawl's letters sway when it brakes
- **WHEN** the person picks "Physics" in the title's animation presets, keeps "Sway", and plays a range where the crawl's Slowed&Reverb rate drops
- **THEN** the letters lag and overshoot around the braking and settle; the rows are in the grid with curve buttons
- **PATH** double-click the crawl title → its form → the animation presets → "Physics" → "Sway" is selected, rows appear → faint spring lines on the preview letters → Space over the braking → the letters run ahead and settle back
- **TEST** t3972 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `letter_physics`
