## ADDED Requirements

### Requirement: The bottom toolbar row never cuts off a button
When the bottom toolbar row of the timeline does not fit the window's width, the button groups that
do not fit SHALL move, as whole groups and from the row's end, into a "more" (⋯) menu button that
stays at the row's right end; every moved button SHALL work from that menu exactly as from the row
(the same body, the same hint). When the row fits, there SHALL be no "more" button. The transport
group and the timecode SHALL never move. The row's height does not change.
Why: found by the `tour-lessons-fixes` executor while walking the lessons (kkt-26.11.01): the row
needs about 1460 points, and in a narrower window its tail — Render, Search, Diagnostics, Save — was
clipped with no way to reach it; item 12 of the same round is "nothing looks through a needle's eye".

#### Scenario: Render stays reachable in a narrow window
- **WHEN** the main window is 1100 points wide, then 1700
- **THEN** at 1100 the row ends with a "more" button inside the window, every button of the row is
  either inside the window or in that menu, and clicking Render in the menu opens the render window;
  at 1700 there is no "more" button and Render is in the row
- **PATH** make the program window 1100 wide → the toolbar under the timeline ends with "⋯" at the
  right edge, nothing is cut → click "⋯" → a menu row with Undo/Redo, Render, Search, Diagnostics,
  Save → click Render → the render window opens; widen the window to 1700 → "⋯" is gone, Render sits
  in the row
- **TEST** t1132 (`src/apps/kt-app/src/window_fill_tests.rs`)
