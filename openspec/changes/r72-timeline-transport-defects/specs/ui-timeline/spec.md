# Spec Delta

## ADDED Requirements

### Requirement: Event gesture zones follow the event's size on screen
On an event the trims SHALL be tested before the level line, so a press in the outer columns of an
event (its bottom corners included) always starts a length drag. The level line's grab SHALL scale with
the row's height on screen. On a row too low or an event too narrow for three separate zones, there
SHALL be no level gesture at all: only move, trim and the corner fades. The thresholds are set in
design.md and named next to their constants. Moving an event by its body SHALL never change its gain or
opacity. The hover cursor and hint SHALL name the zone that a press would take.
Why: the owner, `ktprompts_v26.10` USABILITY 5.1–5.3: "so many times I wanted to quickly move events,
and the volume change keeps triggering". The level line was tested first and at a fixed 5 px. On a
16–30 px row that covered a third of the row, including the corners.

#### Scenario: The bottom corner is a trim, a low row has no level gesture
- **WHEN** an audio event with gain −6 dB on an 84 px row is pressed 2 px inside its right bottom corner and dragged; separately — the same event on a 24 px row is pressed on its level line in the middle and dragged sideways; separately — on a 300 px wide event at 84 px the level line is dragged
- **THEN** the first changes the length, not the gain; the second moves the event and its gain is unchanged; the third changes the gain
- **PATH** zoom the rows out until they are low → drag an event by its middle → it moves, the volume stays; zoom back in → drag the bottom right corner → the event gets longer; drag the level line in the middle → the volume changes
- **TEST** t1038 (`src/apps/kt-app/src/layout_tests.rs`), t1039 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: An event's gain goes down to silence, and a non-default level is written on the event
The event's gain SHALL span from +6.02 dB down to silence. The level line's bottom and the menu's
number field's floor SHALL give silence. The upper half of the line keeps today's mapping, so 0 dB
stays at the middle. A gain or opacity other than the default SHALL be written as a small chip after
the source name in the event's label row (for example `−12 dB`, `40 %`, `mute`). It SHALL use no new
strip (rule 13), be truncated with the name by the shared rule, and be absent at the default.
Why: the owner, `ktprompts_v26.10` USABILITY 6: "let the gain go down to mute, not only to −6 … show
somewhere on the event what gain/opacity is set". The track's fader reached −60 dB, the event's line
stopped at −6.02 dB, and the line's height was the only indicator.

#### Scenario: Down to mute, and the chip says so
- **WHEN** an audio event's level line is dragged to the bottom; separately — a video event's opacity is set to 40 % in the menu; separately — both are reset
- **THEN** the event renders silence (see `audio-render`), its label row shows `mute`; the video event shows `40 %`; after reset there is no chip
- **PATH** drag an audio event's level line all the way down → the label reads "name · mute", play → silence → double-click the line → back to 0 dB, the chip is gone
- **TEST** t1040 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A range marker's end is grabbed where it is drawn and does not snap to itself
The end of a range marker SHALL be grabbed on the ruler at the nearest end within the grab radius. On
the ruler it wins over a coincident region edge. Below the ruler a coincident region edge keeps
priority, and the hover hint SHALL say which edge a press takes. A drag of the end SHALL NOT snap to
the marker's own start or its own old end.
Why: the owner, `ktprompts_v26.10` USABILITY 7 and question 9 of R.71: "something is wrong with the
second marker … the end of the range marker". A marker made over the region lies exactly on the
region's end. Its end snapped back to that position and could be taken only on the ruler, without a
hover cue.

#### Scenario: The end of a marker made over the region moves
- **WHEN** a region 2–6 s, "Marker over the region" makes a 2–6 s range; its end is dragged on the ruler from 6 s to 7 s in three steps; separately — hovered at the end
- **THEN** the range is 2–7 s (no snap back to 6 s), the region is unchanged; the hover shows the resize cursor and the hint of the marker's end
- **PATH** select a region → right-click the ruler → "Marker over the region" → hover the right flag of the marker → the hint says "marker end" → drag it right → the marker grows, the region stays
- **TEST** t1042 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Vertical zoom keeps the row under the mouse
A vertical zoom gesture SHALL keep the same row, at the same fraction of its height, under the
pointer. This SHALL hold on rows of different heights (service rows, per-row overrides). The scroll
range SHALL be computed from the real row heights.
Why: the owner, `ktprompts_v26.10` TIMELINE 1: "when you scale vertically the vertical position flies
down to the lowest track". The anchor assumed equal row heights and the clamp pinned the view to the
bottom.

#### Scenario: The row under the pointer stays under it
- **WHEN** a timeline with two morph rows, a camera row and four clip rows (one with a height override), the pointer over the third clip row, vertical zoom in and out by five steps each
- **THEN** after every step the row under the pointer is the same row, its fraction within ±1 px
- **PATH** point at the third track → Ctrl+Shift+wheel up → the tracks grow, the same track stays under the mouse → wheel down → they shrink around it
- **TEST** t1043 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The scrollbar and both jump zones can be taken
The timeline's vertical scrollbar SHALL be a no-resize rect. Its grab SHALL be wider than its drawing,
and a click on its track SHALL page. The right jump zone SHALL end before the scrollbar. The left jump
zone SHALL begin after the track header gutter and keep its full width on every row, badges included.
In a jump zone a click SHALL jump even over an event. A quick second click there SHALL be a second jump,
not a double click that opens the event. Dragging an event that starts in the zone SHALL still move it.
Why: the owner, `ktprompts_v26.10` TIMELINE 2–4: the scrollbar "is very hard to find, either the window
resize or the jump zone triggers"; "the added track buttons broke half of the jump zone on the left";
"if an event is in the jump zone, it opens for some reason". This is the theme of r.12/r.13 ("resize
steals clicks", t101) returning on a new element.

#### Scenario: The scrollbar at the window edge, the jump over an event
- **WHEN** the timeline is flush with the window's right edge; the scrollbar is pressed and dragged; separately — the left jump zone is clicked on a row where badges are; separately — an event lying in the right jump zone is clicked twice within 0.2 s
- **THEN** the scrollbar scrolls and no `BeginResize` is sent; the left click jumps the view; the two clicks give two jumps and no FX window
- **PATH** maximise the window → grab the timeline's scrollbar at the right edge → the tracks scroll, the window does not resize → click near the left edge on a track with buttons → the view jumps left → click twice near the right edge on an event → the view jumps twice, no window opens
- **TEST** t1044 (`src/apps/kt-app/src/layout_tests.rs`), t1045 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Empty rows carry no captions
An empty service row (morpher, camera) and an empty clip row SHALL draw no text inside the row. The
action of adding a morpher or a camera SHALL stay in the row's menu, and its description in the
status-line hint on hover. A test SHALL fail when any catalog text is painted inside an empty row.
Why: the owner, `ktprompts_v26.10` TIMELINE 4: "meaningless obvious captions over the tracks 'Add
morpher here' and the camera one too, I ask to forbid agents to make dumb captions". The prose rule
(item 17 of R.7, checklist rule 9) was broken again, so the rule becomes a test.

#### Scenario: An empty timeline paints no captions in rows
- **WHEN** a project with one empty morph row, one empty camera row and one empty video row is drawn
- **THEN** no painted text lies inside the rows' rectangles; hovering the morph row shows its hint in the status line; its menu has "Add morpher layer"
- **PATH** add a morph track → the empty row is plain, no caption → hover it → the status line explains it → right-click → "Add morpher layer"
- **TEST** t1046 (`src/apps/kt-app/src/layout_tests.rs`)
