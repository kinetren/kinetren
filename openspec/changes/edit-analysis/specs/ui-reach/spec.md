## ADDED Requirements

### Requirement: The analysis acts only when told, and shows first
Mark beats, Mark cuts, Split at cuts, Cut on the beat and Still stretches SHALL each draw what they will do before doing it
and act in one undo step; a place they cannot act on (the material ends) SHALL be left and named. Split at cuts SHALL leave
the linked sound whole unless the person ticks it.
Why: rule 20; Premiere's detector cuts the linked sound without a word.

#### Scenario: Correct a tempo, cut on it
- **WHEN** a grid at half tempo is doubled from the header's menu and four butt-joined events are cut on the beat, one of them without material left
- **THEN** the ticks double, three joints stand on beats, the fourth stays and the notice names it; one Undo returns all
- **PATH** right click a beat tick → "Tempo ×2" → the ticks double → select four picture events → right click → "Cut on the beat…" → lines show where the joints go → OK → the joints move → the notice reads "1 joint left: its footage ends" → Ctrl+Z
- **TEST** t4225 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Split a finished film at its cuts
- **WHEN** Split at cuts runs on an event with five cuts and a dissolve, its sound linked
- **THEN** the picture is six events and the sound one; ticked, both; the dissolve is drawn as a span and not split
- **PATH** right click the film's event → "Cuts" → "Split at cuts…" → lines show on the event → OK → six events, the sound whole → Ctrl+Z → tick "and the linked sound" → OK → six and six
- **TEST** t4221 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `cuts_motion`

#### Scenario: Still stretches
- **WHEN** the edit holds a 12 s static face under a title, a 12 s panning shot and a freeze of 6 s, and stretches of at least 5 s are asked
- **THEN** the face and the freeze are shown, the pan is not; Mark puts two range markers
- **PATH** Edit → "Still stretches…" → bands appear over the face and the freeze → "Mark" → two range markers → Ctrl+Z
- **TEST** t4222 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Find moments ranks the footage
A tab beside the media bin SHALL list the spans of the chosen sources ranked by the person's weights of motion, loud sound,
speech and a steady camera, each row showing what made it; a click SHALL scrub it, a drag SHALL place it, and "Instead of
this" SHALL offer spans of the selected event's length, a synced camera's same moment first, placed above it without
removing anything.
Why: "можно определять что где-то есть более выразительное лицо, или движение или т.д."

#### Scenario: The lively span comes first
- **WHEN** three sources (a still interview, a street, a dancer) are ranked with motion only, then with steady camera raised
- **THEN** the dancer's active span is first; the shaken street shots fall below it; no file is decoded on the second ranking
- **PATH** click the "Moments" tab beside the bin → tick three sources → after the bars fill, rows appear, the dancer first → drag "Steady camera" up → the rows reorder at once → click a row → its card scrubs to the span → drag the row to the timeline → the event stands under the mouse
- **TEST** t4223 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `find_moments`

#### Scenario: Instead of this
- **WHEN** a dull 4 s event of camera A, synced with camera B, is selected and "Instead of this" is pressed and the first row replaced
- **THEN** the first row is camera B at the same moment, 4 s long; a new event stands on the track above, the old one is unchanged; one Undo
- **PATH** right click the dull event → "Find something livelier…" → the Moments tab opens, its first row camera B → "Put above" → an event appears above → Ctrl+Z
- **TEST** t4224 (`src/apps/kt-app/src/reach_tests.rs`)
