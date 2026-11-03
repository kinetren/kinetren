## MODIFIED Requirements

### Requirement: Every window is resizable, its body fills the area, and it remembers its size
Every window of the registry that is not a modal question SHALL be resizable in width and in height,
with a minimum size below which its form stops being usable and no maximum other than the screen.
When the person enlarges a window, its body SHALL take the added room: the body's scroll SHALL reach
down to the action row (or to the window's bottom edge when there is none) and span the window's
width; lists, tables, galleries and previews SHALL grow with it, and text SHALL re-wrap to the new
width. No window body SHALL be capped by a fraction of the screen or a fixed number of points, and
no column inside a window SHALL have a fixed width, unless the guard lists that size with its reason
(a numeric field sized for its longest value, a thumbnail cell). A modal question (the recovery offer,
the question on close) is declared with its reason and is exempt. The size the person gave a window
SHALL be kept across openings: a window opened per object (effects, morpher, transition, camera,
track) SHALL open at the size its kind was last given, and a single window at its own last size, in
the same run and after a restart. A resizable window SHALL still not grow by itself (t918, t923). A window whose content arrives after it opens (the finds of the project search) SHALL open SIZED, with room for that content, not fitted to its empty form: a fitted window fills the height it measured at opening and clips what comes later.
Why: kkt-26.11.01 item 12, the owner: "if you make the Guide window bigger its elements will not
take the whole area, and again you look through a needle's eye on a big window … and so everywhere
… I am tired of reminding"; "some windows grow in width but not in height". The body was capped at
0.62× the screen and shrunk to its content, half of the windows were not resizable at all, and each
earlier fix was of one window — the class came back in the next. One rule in the window shell and one
guard over all windows instead.

#### Scenario: Every window grows when its corner is dragged and its body fills it
- **WHEN** every window the registry can open is opened in a 1920×1200 harness and its bottom-right
  corner is dragged out to make it about 1500×1000 points
- **THEN** the window's frame grew to the dragged size (it is resizable and nothing caps it), and its
  body's scroll reaches within the action row of the window's bottom and spans its width; a modal
  question is listed with its reason (≥ 25 characters) instead
- **PATH** open the effects window of an event → a window of about 580×520 with the chain, the
  parameters and Done/Cancel → drag its bottom-right corner down and right → the window follows the
  pointer, the chain and parameters stretch across, the empty room is inside the scroll above
  Done/Cancel, not around the window's content
- **TEST** t1125 (`src/apps/kt-app/src/window_fill_tests.rs`)

#### Scenario: A window keeps the size the person gave it
- **WHEN** the effects window of one event is enlarged and closed, and the effects window of another
  event is opened; separately the Guide is enlarged, closed and reopened
- **THEN** the second effects window opens at the enlarged size, and the Guide reopens at its own
  enlarged size
- **PATH** open the effects window of event 1 → drag its corner to make it large → close it → open
  the effects window of event 2 → it opens just as large; Help → Guide → enlarge → close → Help →
  Guide → it is as large as it was left
- **TEST** t1126 (`src/apps/kt-app/src/window_fill_tests.rs`)

#### Scenario: A fixed size in a window carries a reason
- **WHEN** the kt-app window code is scanned for screen fractions, fixed scroll heights and fixed
  column widths (`content_rect() … *`, `max_height(<number>)`, `set_width(<number>)`,
  `set_min_width`/`set_max_width(<number>)`, `desired_width(<number>)`)
- **THEN** every match is listed in the guard's table with a reason of at least 25 characters, and
  the table holds no entry the code no longer has; the 0.62× screen cap of the window body is gone
- **PATH** (not a UI scenario: a source scan that keeps fixed sizes out of the windows; the
  consequence the person sees is held by t1125)
- **TEST** t1127 (`src/apps/kt-app/src/window_fill_tests.rs`)

#### Scenario: The project search shows what it finds
- **WHEN** the search window is opened on a project with three markers whose names start with "DR",
  and "dr" is typed after it opened
- **THEN** the finds lie inside the search window's frame (before: the window stayed the height of
  its empty field and the finds were laid out below its edge, unseen)
- **PATH** press Ctrl+F → a window titled "Search the project" with an empty field and room under it
  → type "char" → the media 01-charge.webm and 05-charlotte-by-night.webm and every event that uses
  them are listed under the field, the first one highlighted
- **TEST** t1414 (`src/apps/kt-app/src/layout_tests.rs`), live `readme_shots.py`
  (`src/scripts/live/readme_shots.py`, the search step)
