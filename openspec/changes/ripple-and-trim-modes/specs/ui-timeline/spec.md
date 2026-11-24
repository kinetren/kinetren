## ADDED Requirements

### Requirement: One shift rule moves everything after an edit point
A ripple SHALL be computed by one rule from an edit point, a signed length and a scope (the affected
tracks or all tracks, with or without markers and named areas), and SHALL move, in one undo step,
every event of a track in scope that starts at or after the point (each link group once), the morpher
blocks, camera events, foreign placeholders and the editing end that start at or after it, and, with
markers in scope, the markers and named areas — moved whole when they start after the point and
stretched at their end when they span it. The boundary SHALL be the same tick on every track. A
ripple SHALL act within the reel the edit was made in; a locked track SHALL stay and be named, and a link group
with a member on a locked track SHALL refuse it. A
ripple that would overlap an event spanning the point on a track in scope, put a marker on the tick of
an unmoved marker, or overlap a foreign placeholder SHALL be refused as a whole, naming the obstacle;
the project SHALL then be unchanged. Events SHALL keep their identities: no event is removed and
re-created by a ripple.
Why: F-TL-001 — Vegas's per-track boundary desyncs a pair that is a frame off; invariant 3 — the
effects' state is diffed by event id.

#### Scenario: A rippled delete moves the tail and keeps the transitions
- **WHEN** an event with a transition between its two right neighbours, a range marker after it, a named area over the tail, a morpher block, a camera event and a foreign placeholder later on, is deleted with ripple over all tracks and markers
- **THEN** every later object moved left by the event's length in one undo step; the transition survived with its offsets; the morpher's windows (`ref_windows`) cover the same material as before; Undo restores the project byte for byte
- **TEST** t3415 (`src/libs/kt-model/tests/ripple.rs`)

#### Scenario: A link pair moves once and in sync
- **WHEN** a video/audio pair whose audio starts one frame earlier than its video lies after the edit point and the ripple closes 2 s
- **THEN** both move by exactly 2 s once (not twice through `MoveClip`'s group rule), the one-frame offset is kept
- **TEST** t3416 (`src/libs/kt-model/tests/ripple.rs`)

#### Scenario: A spanning event refuses the ripple with its name
- **WHEN** a ripple over all tracks would close a gap while a music event on another track spans the edit point
- **THEN** the batch is refused with the spanning event named, the project is unchanged, no undo step is written
- **TEST** t3417 (`src/libs/kt-model/tests/ripple.rs`)

#### Scenario: Markers move in a safe order and never merge
- **WHEN** ten adjacent point markers after the edit point are rippled left by less than their spacing, then a ripple would put one on the tick of a marker before the point
- **THEN** the first ripple moves all ten without a `MarkerExists` refusal; the second is refused naming both markers
- **TEST** t3418 (`src/libs/kt-model/tests/ripple.rs`)

#### Scenario: A locked track and an expanded composition
- **WHEN** a ripple over all tracks is made at the root with a locked music track and a composition expanded in place after the point, then inside that band, then with a pair whose audio is on the locked track
- **THEN** the first moves every unlocked root row, the instance as one event and nothing inside it, and names the locked track; the second moves the composition's own later events only; the third is refused naming the pair
- **TEST** t4239, t4240 (`src/libs/kt-model/tests/ripple.rs`)

#### Scenario: The handles come back before the tail moves
- **WHEN** an event joined to its left neighbour by a crossfade is deleted with ripple
- **THEN** the left neighbour gets its handle back and the tail lands exactly at the left neighbour's restored end, with no gap and no overlap
- **TEST** t3419 (`src/libs/kt-model/tests/ripple.rs`)

### Requirement: A rippled region cut keeps the remainders' identities
The region cut with ripple SHALL cut a spanning event by splitting it at the region's end and trimming
the left part to the region's start (the left part keeps its id, the right is the split half) and then
ripple by the region's length; events wholly inside are removed. The gap-keeping region cut SHALL stay
as it is.
Why: invariant 3 — a re-created remainder loses its reverb tail and stream history.

#### Scenario: The remainders keep their effect state
- **WHEN** a region over the middle of a slowed-reverb audio event is cut with ripple and the audio is rendered across the join and after it
- **THEN** the left remainder keeps its `ClipId`, the right is a `Split` half with the link group, the effects' state objects of the left part are the same objects (no reset), the result equals a manual split–trim–move
- **TEST** t3420 (`src/libs/kt-model/tests/ripple.rs`), t3421 (`src/libs/kt-engine/tests/ripple_render.rs`)

### Requirement: Post-edit ripple and the auto-ripple mode
After a delete, cut, region cut, trim or paste, one command SHALL ripple by the time that edit freed or
took, in the chosen scope, inside the same undo step; with no such last edit it SHALL be disabled with
its reason. The auto-ripple mode SHALL be off by default, cycle off → affected tracks → all tracks,
draw the selection outline and the playhead red while on, and make delete, cut, region cut, paste and
edge trims ripple in the same batch; a move of an event SHALL NOT ripple. The mode SHALL live in the
person's workspace, not in the project, and start off with the program.
Why: the owner's ★1 ("auto-ripple кнопкой … с красным индикатором"); rule 20 — the shift happens only on
an explicit command or a visibly armed mode.

#### Scenario: Ripple after the fact joins the edit's undo step
- **WHEN** an event is deleted (a gap remains), then the post-edit ripple over affected tracks is given, then Undo once
- **THEN** the tail closes the gap; one Undo restores both the event and the tail positions; after any other command, or after the Undo, the item is disabled with its reason
- **PATH** click an event → Delete → a gap → Edit → "Ripple after the edit" → "Affected tracks" → the tail closes → Ctrl+Z → the event and the gap are back as before the delete
- **TEST** t3422 (`src/apps/kt-app/src/reach_tests.rs`), t4241 (`src/libs/kt-model/tests/ripple.rs`)

#### Scenario: The armed mode is visible and ripples a region cut
- **WHEN** the auto-ripple button is pressed once, a region is drawn over the middle of an interview event and `C` is pressed, then the button is pressed twice more
- **THEN** after the first press the selection outline and the playhead are red and the status line says how to turn it off; `C` removes the region and closes the hole; after the third press the mode is off and the outline is the normal colour; the project file never holds the mode
- **PATH** click the auto-ripple button → outline and playhead turn red → drag a region over the event → press C → the hole closes, the tail moved left → click the button twice → normal colours
- **TEST** t3423 (`src/apps/kt-app/src/reach_tests.rs`), t3424 (`src/apps/kt-app/src/layout_tests.rs`), planned live scenario `ripple` (t3425)

### Requirement: Roll, slip and slide are drags with their own stops
A Shift- or Alt-drag at the joint of two butt-joined events SHALL trim both by the same amount; a Shift- or
Alt-drag of the body SHALL move the material under a fixed event (and its link group) and SHALL stop at the
material's start and known end with the stop drawn as an error; a Ctrl+Shift- or Ctrl+Alt-drag of the body
SHALL move the event and trim only its butt-joined neighbours. The modifiers SHALL be those held at the press. Keys glued to the
material SHALL move with the frames on a slip. Each SHALL be one undo step.
Why: F-TL-002 — slip first, slide only explicit (it changes what was not dragged).

#### Scenario: Slip keeps place and length and stops visibly
- **WHEN** an event with a key at a material frame is slipped by +1 s, then past the end of its 10 s material
- **THEN** start and duration are unchanged, `source_in` grew by 1 s, the key stays on its material frame (it moved left on screen with the frames); the second drag stops at the material's end with the edge drawn red and the hint in the error style
- **PATH** hold Shift and drag the event's body right by 1 s → the event stays in place, the picture inside moves, the key mark moves with the frames → drag on past the material's end → the drag stops, the event's edge turns red, the status line says the material ended
- **TEST** t3426 (`src/libs/kt-model/tests/ripple.rs`), t3427 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Slide trims only butt-joined neighbours
- **WHEN** the middle of three butt-joined events is slid right by 0.5 s, then an event with a gap before it is slid
- **THEN** the left neighbour grew and the right shrank by 0.5 s, the total length is unchanged; in the second case the event moves and the non-touching neighbour is unchanged
- **TEST** t3428 (`src/libs/kt-model/tests/ripple.rs`)

#### Scenario: Roll at a plain cut
- **WHEN** the joint of two butt-joined events without a transition is rolled by 10 frames, then past the right event's material start
- **THEN** both trimmed by 10 frames, the joint moved; the second roll is clamped at the material with the stop shown
- **TEST** t3429 (`src/libs/kt-model/tests/ripple.rs`)

#### Scenario: The three drags by mouse
- **WHEN** the middle of three butt-joined events is slid by Ctrl+Shift-drag, the joint after it rolled by Shift-drag, and a Shift+click, a Ctrl+click and a Shift+double click are given after
- **THEN** each drag is one undo step with its own result; the clicks select, toggle and make a region as before
- **PATH** hold Ctrl+Shift and drag the middle event right: it moves, its neighbours give way, the ends stand → Ctrl+Z → hold Shift and drag the edge between the second and the third: the cut moves, nothing else → Ctrl+Z → Shift+double click on the event: a region over it
- **TEST** t4242 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Close the gaps of the selected events
The close-gaps command SHALL move each selected event left to the end of the previous event on its
track, in one undo step; with an empty selection it SHALL apply to the track under the playhead; there
SHALL be no command that closes every gap of the project at once.
Why: F-TL-008 — the Vegas form by selection; a project-wide tidy is ripple over all tracks without a sign.

#### Scenario: Gaps of a selection close
- **WHEN** three events with gaps between them on one track are selected and gaps are closed
- **THEN** they stand butt-joined from the first one's start; unselected events of other tracks did not move; one Undo restores them
- **TEST** t3430 (`src/libs/kt-model/tests/ripple.rs`)
