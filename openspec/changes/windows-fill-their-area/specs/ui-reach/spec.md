## ADDED Requirements

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
the same run and after a restart. A resizable window SHALL still not grow by itself (t918, t923).
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

### Requirement: The Guide draws real tables and separate index cards and uses its whole window
A table on a guide page SHALL be drawn as a grid table: a header row in its own shade, a rule between
the columns and between the rows, alternating row stripes, and column widths taken from the content —
a column is never narrower than its longest word, and the width left over is shared in proportion to
how much text each column holds; cells wrap within their column. Every "I want to…" entry SHALL be its
own card: the "what" on its first line, the "where" below it in a weaker style, a frame around the
card and a gap between cards, the whole card clickable. The Guide's two sides SHALL share the window:
the index side a proportion of the width with a floor and a ceiling, the page the rest, and both
lists SHALL reach the window's bottom.
Why: kkt-26.11.01 item 12, the owner: "in the Guide all the tables are crooked too, their first column
is narrow and in the end it looks cheap"; "the text under "I want to…" visually runs together, though
it turns out to be separate sections"; "the Guide window looks unfinished". The table was a
two-column egui grid of wrapping labels (the first column collapsed to its narrowest word and wider
tables lost columns), and the entries were frameless wrapping buttons with one arrow between the two
halves.

#### Scenario: A guide table has columns from its content
- **WHEN** the Guide opens the "Keyboard and mouse" page in a 1600×1000 window, and again in a
  narrow one
- **THEN** each table's columns match its header's count; no column is narrower than its longest
  word; the first column of the keys table is at least as wide as its widest key cap and no row's
  first cell wraps when the window is wide; rules are painted between columns and rows and the header
  row has its own fill; in the narrow window the cells wrap and the table stays within the page
- **PATH** Help → Guide → click "Keyboard and mouse" → each table has a shaded header, the key column
  is as wide as its longest key, the action column takes the rest, lines between the cells → make
  the window narrow → the action texts wrap inside their column, the table does not run past the page
- **TEST** t1128 (`src/apps/kt-app/src/window_fill_tests.rs`)

#### Scenario: "I want to…" entries are separate cards
- **WHEN** the Guide opens with an empty search
- **THEN** every index entry is drawn in its own framed rectangle, the rectangles do not overlap and
  are separated by a gap, the "what" text lies above the "where" text inside each, and a click
  anywhere on a card opens its page at its heading
- **PATH** Help → Guide → the left side lists "I want to…" cards: "Play a part in a loop" with "drag
  on the ruler, then L" under it, each in its own frame → click the card's lower line → the timeline
  page opens at "Markers and the region"
- **TEST** t1129 (`src/apps/kt-app/src/window_fill_tests.rs`)

#### Scenario: The Guide's sides grow with the window
- **WHEN** the Guide is opened at its default size, then its corner is dragged out to about 1500×1000
- **THEN** the index side grew wider within its ceiling, the page grew by the rest, and both scrolls
  reach the window's bottom in both sizes
- **PATH** Help → Guide → drag the corner far out → the cards column widens a little, the page takes
  the rest of the width, both lists run to the bottom of the window
- **TEST** t1130 (`src/apps/kt-app/src/window_fill_tests.rs`)

### Requirement: The effect picker is one gallery that grows with its window
The "+ effect" picker SHALL show its content in ONE scroll: the presets as a row of cells first, then
the effects by category, each effect a card (its name and its variant cells) and the cards flowing
into as many columns as the gallery is wide. Its width SHALL be the width of the window it opens
from (never less than the gallery's floor), so an enlarged effects window gives a wider gallery with
more cards per row, and its height SHALL be the room the screen has below the button, never a fixed
number. What a person types into the search SHALL NOT change the popup's width.
The same screen-bounded height SHALL apply to the media picker's list.
Revision (R.54, `GALLERY_W` "fixed by the gallery of the longest content"): the width is no longer a
constant; it follows the host window, which keeps the R.54 point — nothing jumps while typing.
Why: kkt-26.11.01 item 12, the owner: "the effects window is small by itself, and when you want to add
an fx the choice of fx is split further into two sections, and there too it is a needle's eye". The
picker was two scrolls of 160 and 450 points in a 520-point popup, whatever the window or the screen.

#### Scenario: The picker gallery follows the window and the screen
- **WHEN** the effects window of a video event is opened at its default size and the picker is
  opened; then the window is enlarged to about 1400 points wide and the picker is opened again; a
  query is typed
- **THEN** the picker has one scroll; its width is the host window's content width in both cases
  (the second is wider) and does not change while typing; a row of the wide gallery shows more
  effect cards than a row of the narrow one; the popup's bottom is within the screen and lower than
  500 points below its top when the screen allows
- **PATH** open the effects window of a video event → click "+ effect" → a gallery as wide as the
  window, presets then effects in one list → close it, drag the window's corner to make it wide →
  click "+ effect" → the same gallery, wider, with more effect cards per row, reaching down the screen
- **TEST** t1131 (`src/apps/kt-app/src/window_fill_tests.rs`)
