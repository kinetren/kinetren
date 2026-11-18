## ADDED Requirements

### Requirement: The generator and transition kind pickers are galleries of the frame after the click
The kind picker of a generator event (its window's head) and of a transition (its window) SHALL show
every kind as a cell under its family heading, as the effects picker does: the frame the preview shows
after the click on that cell, on the project the player plays — a generator kind as the event at the
cursor with that kind at its defaults, a transition kind as the cut at the middle of its window with
that kind — rendered by the effect cells' service with its misclick guard, cache and pause while
playing; a cell not yet computed SHALL say Loading, a sound generator's cell SHALL show its name
without a picture. The current kind SHALL be outlined; a click on a cell SHALL pick it and close the
gallery. The gallery SHALL be as wide as the window it opens from and as tall as the screen allows.
Why: `v26.10` line 17, the owner: "нет превью генераторов при выборе в окне, у переходов так же нету …
в половине программы нормальный интерфейс (как у эффектов …) а у половины обрубки".

#### Scenario: A kind candidate is what the click puts
- **WHEN** a transition kind candidate and a generator kind candidate are put into a project
- **THEN** the transition has that kind with no curves, the generator event that kind at its defaults
  with no curves, the events are unchanged otherwise and the track is soloed; a transition or an
  event the project does not hold gives no scene
- **PATH** in a transition's or a generator event's window, click the Kind field → each cell shows the
  frame with only that kind changed, the other events and effects as they are
- **TEST** t3108 (`src/apps/kt-app/src/fx_thumbs.rs`)

#### Scenario: A transition kind's cell is the cut after the click
- **WHEN** the cells of Iris and Crossfade are rendered for a cut at the middle of its window
- **THEN** each is byte for byte the frame the offline path composes with that kind there, and the
  two differ
- **PATH** open a transition's window, click the Kind field → the Iris cell shows the round iris
  between the two clips, the Crossfade cell their mix, as the preview shows each after the click
- **TEST** t3109 (`src/apps/kt-app/src/fx_thumbs.rs`)

#### Scenario: The transition kind picker is a gallery of cells
- **WHEN** a transition's window is open past the misclick guard and its Kind field is clicked, then
  the Iris cell is clicked
- **THEN** every visible kind is a picture cell, the current one has a cell, the cells asked the
  service; after the click the transition is Iris and the gallery is closed
- **PATH** double-click a transition → its window; click the Kind field → a gallery of pictures by
  family, each the cut at the middle of the window with that kind ("Loading" for the first second);
  click the Iris picture → the window shows Iris, the gallery is gone
- **TEST** t3110 (`src/apps/kt-app/src/layout_tests.rs`)
