## ADDED Requirements

### Requirement: The shape parameters of one segment and of a selection are edited by fields
The automation widget SHALL let a person select segments of the ACTIVE curve: a right click on a
segment selects that segment alone (unless it already belongs to the selection), Ctrl+click on a
segment toggles it, Shift+click selects the run from the last clicked segment to this one, and a drag
that starts on an empty spot of the graph (not on a key, not on a shape handle, not on the ruler)
selects every segment its time span touches (with Ctrl — added to the selection). Selected segments
SHALL be tinted on the graph. A plain click on an empty spot SHALL keep its meaning (a key is placed)
and SHALL clear the selection; switching the active curve SHALL clear it too.

Under the "Shape of all segments" row, the curve block SHALL show, while a selection exists, a row
for the SELECTED segments with the same controls: the shape and its parameters (tension; periods,
amplitude, profile). Its caption SHALL name the segment's bounds for one segment and the count for
several. The fields SHALL show the segments' own values; a field whose values differ across the
selection, and the shape when the kinds differ, SHALL show the mixed mark "—" (and no parameter
fields while the kinds differ). A typed number SHALL be set on every selected segment; dragging a
field SHALL shift every selected segment by the same amount, each kept within that parameter's
bounds; picking a shape SHALL set it on every selected segment and SHALL keep the parameters of the
segments that already have that shape. A segment formula keeps its text field of R.67 when exactly
one formula segment is selected. A right-click shape choice on a segment that belongs to the
selection SHALL apply to the whole selection, and the menu header SHALL say how many segments it
touches. The "all" row SHALL be the same control over all segments of the curve: it keeps its look,
and when the segments differ it SHALL show the mixed mark instead of the first segment's shape.

Every edit through these controls SHALL be ONE command for all the segments it touches (never one
per segment); inside the FX window it joins the window's history step like any other edit of the
window. The controls SHALL write ONLY on a human gesture (a drag, focus with an edit, a pick from the
list): a value from the file outside a field's bounds SHALL be shown as it is and MUST NOT be clamped
into the model by merely drawing the field.
Why: kkt-26.11.01 item 10 (the owner): "I want to set the zigzag's parameters and there is nowhere to
write them; if I set the zigzag for the whole curve, then there is… and what if I have 100 segments
with parameters?" One segment had only the R.67 drag handle (no number, no profile), several had
nothing. The "all" row's `DragValue`s with a range clamped an out-of-range value (the model takes
periods up to 1024, the field 0.25…256) and reported "changed" each frame — the R.67 class "a write
without a gesture".

#### Scenario: One segment shows and edits its own parameters
- **WHEN** a curve has four keys with a zigzag (periods 4, amplitude 2) on the first two segments and a linear third; the person right-clicks the second segment, picks its own shape in the menu (the menu closes, nothing changes), and types 9 into the periods field of the "selected segment" row, then Enter
- **THEN** the row's caption names the second segment's bounds, its fields show 4 and 2; after the edit the second segment has periods 9, the first keeps 4, the third is linear; the edit is one command
- **PATH** FX window → a curve with zigzag segments → right click on the second segment → its own shape in the menu (nothing changes, the menu closes) → under "Shape of all segments" the row "Selected segment 0.667–1.333 s" with Zigzag, Periods 4, Amplitude 2 → click Periods, type 9, Enter → only that segment's zigzag gets denser on the graph
- **TEST** t1115 (`src/apps/kt-app/src/segfield_tests.rs`)

#### Scenario: A selection shows mixed values and a typed number lands on all of it
- **WHEN** three zigzag segments with periods 2, 4, 6 and equal amplitude are selected by right click on the first and Ctrl+click on the other two; a fourth zigzag segment is not selected; the person types 5 into the periods field
- **THEN** the caption says three segments; the periods field shows "—", the amplitude field shows the common value; after typing all three have periods 5, the fourth keeps its own; the change arrives as one command in one frame
- **PATH** right click on segment 1 → its own shape in the menu → Ctrl+click segments 2 and 3 → they are tinted, the row says "Selected segments: 3", Periods shows "—" → click Periods, type 5, Enter → the three zigzags become equal, the fourth is untouched
- **TEST** t1116 (`src/apps/kt-app/src/segfield_tests.rs`)

#### Scenario: A drag on a mixed field shifts by one amount, a shape pick keeps same-kind parameters
- **WHEN** a selection of two smooth segments and one linear one (the kinds are mixed); then a selection of three smooth segments with tension 1.0, 2.0 and 3.9 whose tension field is dragged right by 20 px; then the shape "Zigzag" is picked for a selection that holds one zigzag (periods 7) and one smooth segment
- **THEN** with mixed kinds the shape shows "—" and there is no parameter field; after the drag the first two tensions grew by the same amount and the third stopped at the bound 4; after the pick both are zigzags, the former zigzag keeps periods 7, the other takes the default zigzag
- **PATH** select three segments → the shape shows "—" → with three smooth segments selected the Tension field shows "—" → drag it right → the segments steepen together, the steepest stops at its bound → pick Zigzag in the row's shape list → both segments tremble, the old zigzag keeps its density
- **TEST** t1117 (`src/apps/kt-app/src/segfield_tests.rs`)

#### Scenario: Sweep, range and a plain click
- **WHEN** a curve of six segments; the person drags on an empty spot of the graph across segments 2–4; then Shift+clicks segment 6; then clicks an empty spot without modifiers
- **THEN** after the sweep exactly segments 2, 3, 4 are selected and no key moved or was added; after Shift+click segments 4…6 are selected; the plain click places one key and clears the selection
- **PATH** drag across the middle of the graph → three segments tinted, the row says 3 → Shift+click the last segment → the tint runs from the fourth to the sixth → a plain click → a new key appears, the tint and the row disappear
- **TEST** t1118 (`src/apps/kt-app/src/segfield_tests.rs`)

#### Scenario: The segment menu applies to the selection it belongs to
- **WHEN** three segments are selected; the person right-clicks one of them and picks "Hold"; then right-clicks an unselected segment
- **THEN** the menu header names three segments and all three become Hold; the second right click selects only the clicked segment
- **PATH** select three segments → right click on one of them → the header says "3 segments" → Hold → the three become steps → right click on another segment → only it is tinted
- **TEST** t1119 (`src/apps/kt-app/src/segfield_tests.rs`)

#### Scenario: The shape fields write only from a gesture
- **WHEN** a curve whose segments carry a zigzag with periods 500 (legal in the model, above the field's 256) is shown with the "all" row and a selection row for 60 frames, nothing touched
- **THEN** the curve in the model is unchanged and the fields show 500; on the code before this change the "all" row wrote 256 into every key on the first frame (the red proof)
- **PATH** there is no gesture: the window is merely open for sixty frames and the test compares the model before and after — that a field does not write by itself is checked without a mouse
- **TEST** t1120 (`src/apps/kt-app/src/segfield_tests.rs`)
