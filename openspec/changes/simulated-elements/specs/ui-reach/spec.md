## ADDED Requirements

### Requirement: A simulation is added to a set from its owner's window
The window of an owner that produces elements — the title form, the mask panel, an emitter's form — SHALL have a
"Simulation" section: the granularity of the set (for a title: letters, words, lines), the solver from the `sim`
family with its rows in the one parameter grid, and its drivers chosen from the outer speed, a link, a track's sound, a
tracker or another set; while the window is open the preview SHALL draw a faint overlay of the elements' homes.
Why: the owner's F2 and D3 — the system is reached where its elements live, not through one form of one generator.

#### Scenario: Make a crawl's letters sway when it brakes
- **WHEN** the person opens a crawl title's form, adds a simulation on letters with the spring solver driven by the outer speed, and plays a range where the crawl's Slowed&Reverb rate drops
- **THEN** the letters lag and overshoot around the braking and settle; the solver's rows are in the grid with curve buttons
- **PATH** double-click the crawl title → its form → the "Simulation" section → "Add" → "Letters", solver "Spring", driver "Edit's speed" → faint home marks appear on the preview letters → Space over the braking → the letters run ahead and settle back
- **TEST** t3972 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `simulated_elements`

#### Scenario: A mask follows a tracker with inertia
- **WHEN** the person opens a tracked mask's panel and adds a simulation on its vertices with the spring solver
- **THEN** the mask's edge lags behind a fast move of the tracked point and settles; the overlay shows the tracked homes
- **PATH** select the masked event → the mask panel → the "Simulation" section → "Add" → "Vertices", solver "Spring" → the tracked homes show as dots → play over the fast move → the edge trails the dots and settles
- **TEST** t3988 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: An emitter's particles are drawn by one pass
- **WHEN** the person adds the Emitter generator, sets a sprite and a sound-driven burst, and plays
- **THEN** particles spawn on the beats and fall with gravity; the performance panel shows one sprite pass for the emitter
- **PATH** Generators → "Emitter" → drop on a track → its form → sprite "Dot", burst driver "Track 2 sound" → Space → particles burst on the beats and fall
- **TEST** t3989 (`src/apps/kt-app/src/reach_tests.rs`)
