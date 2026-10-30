# Spec Delta

## ADDED Requirements

### Requirement: A track is reordered by dragging its header, and the header says so
Pressing a track header's free area (outside its badges and its height edge) and dragging SHALL
reorder the track (one undo step, the same body as the menu's reorder). Hovering that area SHALL show
the grab cursor and the status-line hint "drag to reorder". The test SHALL drag from the places a
person grabs: the name and the empty part above the badges. It SHALL assert the new row order.
Why: the owner, `ktprompts_v26.10` TIMELINE 6: "I would like to drag the order of tracks with the mouse
at the side". The drag exists, but nothing tells the person, no test holds it, and the R.62 badges
took most of the header.

#### Scenario: Dragging a header moves the track
- **WHEN** three video tracks; the second track's header is pressed on its name and dragged below the third in three steps; separately — hovered
- **THEN** the order is 1, 3, 2 and one undo restores it; the hover shows the grab cursor and the hint
- **PATH** hover a track's name at the left → the cursor becomes a hand, the status line says "drag to reorder" → drag it down past the next track → a blue line shows the place → release → the track is there
- **TEST** t1047 (`src/apps/kt-app/src/reach_tests.rs`)
