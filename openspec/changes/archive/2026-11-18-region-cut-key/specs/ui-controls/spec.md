## ADDED Requirements

### Requirement: The region's interior is cut out by a key and the Edit menu
"Cut out the region interior" SHALL be a command of the key table, bound to `C` in our layout and
unbound in the borrowed layouts, and SHALL have an Edit menu item with its chord; the key, the Edit
item and the empty-space item SHALL run one body, which cuts out the interior of the region from
every event it crosses and leaves the rest in place, as one undo step. Without a region the key and
the Edit item SHALL say so and change nothing.
Why: the owner, `v26.10` line 35 — "нужна функция cut(ножницы) региона и чтобы еще с клавиатуры
можно было (кнопка "C" свободна?)"; the action lived only in the empty-space menu (F-TL-015).

#### Scenario: C and Edit › Cut out the region interior give the same cut
- **WHEN** a region 20 000–40 000 ticks lies over an event and `C` is pressed; the cut is undone and Edit › "Cut out the region interior" (chord `C`) is clicked
- **THEN** no material is left inside the region, the remainders end and start at its edges, one undo restores the events, and the Edit item gives exactly the cut `C` gave
- **PATH** draw a region over part of an event (I, O) → press C → the event's middle is gone, both remainders stay where they were → Ctrl+Z → the event is whole → Edit menu → "Cut out the region interior" with C on the right → click → the same gap
- **TEST** t3102 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: No region, other layouts
- **WHEN** there is no region and `C` is pressed; the key list of every borrowed layout is read
- **THEN** the project is unchanged and the notice says "Nothing to cut out: draw a region first"; our layout prints `C` for the command, every borrowed layout prints no key
- **PATH** clear the region → press C → nothing is cut, the status line says to draw a region first
- **TEST** t3103 (`src/apps/kt-app/src/layout_tests.rs`)
