## ADDED Requirements

### Requirement: Input that changes nothing does no work
A frame of drawing, a pointer moving, a window or panel being open or selected, and a gesture that
holds still SHALL change no model, bump no revision, push no undo step, hand the player no project,
rebuild no renderer, wake no prefetch and order no decode. A number field SHALL write only from a human
gesture (a drag, keyboard input, a click on its slider, a reset), never because the widget clamped or
rounded a value on display, and a value its carrier already holds after its own clamp and cast SHALL
be no edit. A colour button SHALL write only a colour that differs from the one held. A live drag
SHALL hand the player a project only on a frame where the value moved.
Why: the owner, kkt-26.11.07b G5/G7: "открываю эффекты события этого, и у меня 100% херачит prefetch
когда я двигаю мышкой", and with VHS Tape and Camera ISP switched off "любое движение мышки … тоже
приводит к нагрузке". His VHS Tape held `dark_clip = -0.8`, the floor; the row is stored as f32, -0.800000012,
under the floor, so the percent field clamped it on every frame and read "changed", the window wrote it
back and each mouse move rebuilt the whole engine: 447 ms of audio renderer on his 1974 events, on the
audio render thread and again on the prefetch thread. Slowed+Reverb, Chromatic Aberration and Camera
ISP at their ceilings did the same.

#### Scenario: No field writes from a frame
- **WHEN** every number row of every effect, transition, generator and track passport stands at its floor and then its ceiling as the carrier stores it, and the shared grid is drawn for frames with the pointer moving over it
- **THEN** no frame reports a change and no value moves
- **PATH** open any effect's window with a number at the end of its range → the field shows it; move the mouse over the field and the grid → the value stays, nothing is edited, Undo has nothing new
- **TEST** t1870 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: An effects window under the pointer changes nothing
- **WHEN** the effects window is open over an event carrying any effect kind, its numbers at a bound, that slot selected (the VHS Tape deck, the Camera ISP pipeline and the codec's stream panel included), the owner's own VHS values first, and the pointer sweeps the window
- **THEN** the app's and the model's revisions, the undo depth and the player's project version stay as they were
- **PATH** double click an event carrying VHS Tape → the effects window opens on its chain; click the VHS Tape slot → the deck face and its grid show; move the mouse over the window → nothing but the hover changes, the processor stays at rest
- **TEST** t1871 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: Every editing window under the pointer changes nothing
- **WHEN** the transition window (every kind, at both bounds), the track window (video and audio, at both bounds), the camera window and the morpher window are open and the pointer sweeps them
- **THEN** no revision, undo step or player project moves
- **PATH** double click a transition → its window opens; move the mouse over it → nothing changes; the same over a track's window, a camera's and a morpher's
- **TEST** t1872 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: A held level drag hands the player nothing
- **WHEN** an event's level line is grabbed and the pointer then holds still or moves along the line for frames
- **THEN** the player is handed a project only on frames where the level moved
- **PATH** grab the level line of a selected event → the line follows the pointer; hold the mouse still → nothing is rebuilt; move it up → the level follows, once per change
- **TEST** t1876 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: A colour written back is the colour held
- **WHEN** an effect's colour row holds any 8-bit colour and the colour button gives it back through its floating-point round trip
- **THEN** the written value equals the held one, so nothing is sent
- **PATH** open Green Screen's window and click its key colour → the colour popup opens; move the mouse over the popup without pressing → the colour stays as it was, nothing is edited
- **TEST** t1877 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: The VHS deck selected, a number at its floor, the pointer moving, on the heavy project
- **WHEN** the heavy showcase is open, a composition's effects window shows its chain, the VHS Tape slot is selected, the dark clip field is dragged to its floor (-80 %), the program has gone quiet, and the pointer sweeps the deck panel
- **THEN** no edit, project swap, engine build, prefetch pass or decode runs, and the prefetch and audio render threads hold under 5 % of a core each, measured by their processor time
- **PATH** right click the composition → Effects… → the window shows Slowed, VHS Tape and Camera ISP; click VHS Tape → the deck face shows; drag the dark clip number far left → it stops at -80 %; move the mouse over the deck → only the hover changes, the engine stays asleep
- **TEST** live `idle` (`src/scripts/live/idle.py`)
