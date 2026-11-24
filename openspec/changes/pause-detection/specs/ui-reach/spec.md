## ADDED Requirements

### Requirement: Pauses are shown before anything is done
"Pauses…" SHALL be offered on selected audio events, a track header and the region. While its window is open the pauses
found SHALL be drawn on the events and follow the rows; a click on one SHALL exclude it. Mark, Split and Delete SHALL act
on the pauses shown, one undo step each; nothing SHALL be placed or cut before that, and nothing SHALL come back after.
Delete SHALL be disabled with its reason while ripple is not available.
Why: F-AUD-008 — the threshold is always wrong somewhere; a hundred wrong cuts cost more than one look.

#### Scenario: Look, drop one, mark the rest
- **WHEN** the person opens "Pauses…" on an interview event, raises the shortest pause, clicks one band and presses Mark, then Undo
- **THEN** the bands follow the row, the clicked band is hollow, the markers stand on the others, Undo removes them all
- **PATH** right click the interview's sound → "Pauses…" → bands appear over the event and the window reads "37 pauses, 1 min 12 s" → drag "Shortest pause" to 0.8 s → fewer bands → click a band → it turns hollow → "Mark" → range markers stand on the ruler → Ctrl+Z → they are gone
- **TEST** t4208 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `pauses`

#### Scenario: The window open at rest
- **WHEN** the heavy showcase is paused with the window open over a 40-minute event and the mouse moves over the timeline
- **THEN** no analysis job runs, the overlay mesh is not rebuilt, and only hover frames are drawn
- **PATH** open "Pauses…" on the long event → wait → move the mouse over the tracks → nothing but the hover changes
- **TEST** t4209 (`src/apps/kt-app/src/frame_cost_tests.rs`)
