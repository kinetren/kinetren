# Spec Delta

## ADDED Requirements

### Requirement: The space bar plays while held from pause, reverses with Ctrl, and releases smoothly
A short press of Space (shorter than the hold threshold of about 0.3 s) SHALL toggle play/pause as
before, and two quick presses SHALL still stop at the region start (R.16: "don't break the functions of
the double and single press"). A press that STARTS while paused and is held past the threshold SHALL
play only while held. The press SHALL be held past the threshold to count, and its release SHALL pause
at the position reached. A press held DURING playback SHALL boost the speed as before (up to 4× over
5 s — limits set by the owner in R.16/R.17). Its release SHALL return to the base speed by a decay
whose time constant is shorter than the rise's, not by a jump; the rise SHALL start without a jump
(an ease-in). Ctrl+Space held SHALL play in REVERSE at the base speed, boosted the same way. The
same pause-on-release rule SHALL apply when it starts from pause. Its release during forward playback
SHALL return to forward base speed. The display of the speed SHALL show the temporary speed while it
lasts.
Why: the owner, `ktprompts_v26.10` USABILITY 2–4, and his answer to question 7 of R.71 ("a threshold
~0.3 s"). The release jumped from 4× to 1× in one frame. A press from pause kept playing after
release, so "look at this spot while I hold" was impossible.

#### Scenario: A short press and a double press keep their meaning
- **WHEN** Space is pressed and released within 0.1 s while paused; separately — pressed twice within 0.3 s while playing
- **THEN** the first plays and keeps playing; the second stops at the region start (at the project start without a region)
- **PATH** paused → tap Space → the playhead runs and keeps running → tap Space twice quickly → the playhead stands at the region start
- **TEST** t1620 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Held from pause plays only while held
- **WHEN** paused, Space is pressed and held 1 s, then released; separately — Ctrl+Space the same
- **THEN** playback starts after the threshold, the speed is boosted while held, and on release the transport is paused at the reached position (not at the start point); with Ctrl the playhead moved BACKWARD and stopped likewise
- **PATH** paused at 00:00:10 → hold Space → after a moment the picture plays and speeds up → release → the picture stops where it was, the play button shows "paused"
- **TEST** t1621 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Ctrl+Space during playback reverses while held
- **WHEN** playing forward at 1×, Ctrl+Space is held 1 s, then released
- **THEN** while held the player's rate is negative (−1× and growing in modulus); after release it is +1× (the base) and playback continues
- **PATH** playing → hold Ctrl+Space → the picture runs backward, the speed strip shows a negative speed → release → it runs forward at 1× again
- **TEST** t1622 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The boost eases in and decays faster than it rose
- **WHEN** the boost is held 5 s during playback and released; the speed is sampled per frame with dt values 1/60, 1/30 and 0.1
- **THEN** the first sampled step above 1× is smaller than the later ones (no jump at the start); after release the speed is back within 2 % of the base in less time than the rise took to reach the same distance; no step diverges at any dt
- **PATH** playing → hold Space → the speed strip rises smoothly → release → it comes down to 1× quickly but without a jump
- **TEST** t1623 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The arrows and Ctrl+click on Back/Forward step one frame
← and → SHALL move the playhead exactly one frame of the project fps grid back and forward. The step
SHALL land on an aligned tick, so a repeated step never sticks. Holding the arrow SHALL repeat the step.
During playback the first step SHALL pause, then step. Ctrl+click on the toolbar's Back and Forward
buttons SHALL run the same body (rule 17). Shift on Back/Forward and Shift+A/D keep the region-edge
drag (R.16). The arrows SHALL not act while a text field has focus.
Why: the owner, `ktprompts_v26.10` TIMELINE 11, and his answer to question 8 of R.71 ("Ctrl instead of
Shift? Or just arrows, if that is how other editors do it?"). The program had no frame step at all
(F-TL-003).

#### Scenario: One frame per press, from any place the person acts
- **WHEN** at a 30 fps project, paused at frame 100, → is pressed three times, ← once; separately — Ctrl+click on Forward; separately — → during playback
- **THEN** the playhead is at frame 102 (aligned), after Ctrl+click at frame 103; during playback the first → pauses and steps one frame from the paused position
- **PATH** paused → press → → the picture advances by one frame, the timecode +1 frame → press ← → back one frame → Ctrl+click the Forward button → one frame forward
- **TEST** t1624 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A playback stall is one episode and reaches the bell
The engine SHALL record ONE stall per episode: a record when the playhead has stood still for 1 s while
playing at a nonzero rate, and no further record until the playhead moves again. At rate 0 (a freeze,
the shuttle centre) there SHALL be no stall. The application SHALL post one warning to the notices
(the bell) per new stall episode, naming the time position, in addition to the counter in the
performance panel.
Why: the owner, `ktprompts_v26.10` item 5 of the memory block: "what does 'playback froze: 5' mean? if
it is important it must be in notices, not somewhere in the corners". The engine pushed a record on every
loop pass of one freeze, so the counter overstated it and flooded the 512-entry ring.

#### Scenario: One freeze — one record, one notice
- **WHEN** a synthetic player plays, the head is held still for 3 s, then moves; separately — the rate is 0 for 3 s
- **THEN** the flight ring holds exactly one stall record for the first case and none for the second; the application posts exactly one warning with the position
- **PATH** there is no gesture: the freeze is produced by the test player → the bell gets one red entry "playback froze at 00:01:02"
- **TEST** t1625 (`src/libs/kt-engine/tests/flight_stall.rs`), t1626 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: "Freeze frame here" holds the frame at the playhead and continues after it
The event's menu SHALL have the command "Insert frame hold (freeze frame)" for the event under the
playhead. The name follows the industry: Premiere's "Insert Frame Hold Segment", Resolve's "Freeze
Frame". It SHALL make three pieces in ONE undo step. The piece before the playhead stays unchanged. A
hold of 2 s (or up to the event's end, if shorter) shows the frame at the playhead at rate 0. The piece
after it continues from that same frame. The event's total length SHALL not change, and the material
that no longer fits is cut at the end. No other event SHALL move (rule 20). Linked A/V partners SHALL
follow as for any split, and the sound of the hold is silence. On an event with a speed curve the piece
after the hold SHALL start at the material of the frozen frame.
Why: the owner, `ktprompts_v26.10` "a convenient way to 'stop' an event: I want a freeze frame, and
then continue it" (S1). Rate 0 is a freeze since R.57, but making one took a split, a second split and
a hand-typed speed.

#### Scenario: The freeze keeps the length and continues from the frozen frame
- **WHEN** a 10 s event at constant 1×; separately — one with a speed curve; the playhead at 4 s; the command is run, then undone
- **THEN** three pieces: 0–4 s as before, 4–6 s at rate 0 showing the material of 4 s, 6–10 s starting at the material of 4 s; the neighbour events are where they were; one undo returns the single event byte-equal
- **PATH** right-click the event under the playhead → "Insert frame hold (freeze frame)" → the event shows a 2 s piece with a frozen frame, play → the picture stops for 2 s and goes on from the same frame
- **TEST** t1627 (`src/apps/kt-app/src/reach_tests.rs`), t1638 (`src/libs/kt-engine/tests/clip_gain_floor.rs`)
