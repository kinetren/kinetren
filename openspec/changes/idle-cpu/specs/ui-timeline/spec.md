## MODIFIED Requirements

### Requirement: Repaint only on motion, the cost of a frame is bounded
The UI SHALL request a frame only while something is MOVING: any animated state
MUST converge and release the repaint. At rest NO frame is requested: a background thread that has
something to show wakes the UI (`wake`), and a state on a clock asks for exactly the frame it needs
(`KtApp::rest_wake`). During playback the delay is 16 ms. The preview's frame key SHALL be a tick
aligned to the project's fps grid, and not a raw tick. In the draw paths there MUST NOT be allocations
or clones of structures per frame: snapshots are `Arc` clones of the caches keyed by the model's
revision, the preview's composition buffer is reused. Thousands of painter primitives (the waveform,
the meters' glow) SHALL be drawn as one mesh. UI caches invalidated by the epoch of the burst cache
SHALL be throttled during playback (the coverage bars — no more often than 4 Hz).
Why: the "burning" meters kept 60 fps while idle and heated the whole system, the raw tick
re-uploaded the preview texture 60 times a second, and the epoch changes with every insertion — during playback
this is a storm of polls under the cache's mutex (invariant 12 of the root CLAUDE.md). A tick of 4 Hz
then drew the whole window for nobody: 36 % of a core at rest on the software rasterizer, and the
owner's "ОНО ТАМ ЧЕ, БИТКОИНЫ БЛЯДЬ СЧИТАЕТ ЧТО ЛИ НА ПАУЗЕ".

#### Scenario: When idle the frames are rare, 60 fps during playback
- **WHEN** the application is idle (paused, nothing moves) or is playing
- **THEN** at rest there are no frames: the last one asks for no next frame at all, a status line and unsaved work ask for exactly their moment, and during playback the delay is 16 ms; pausing the player parks the threads
- **PATH** leave the program with nothing moving → the window stands and draws nothing; press Play → the picture and the meters move at full rate; press Pause → the meters fall and stop
- **TEST** t20 (`src/apps/kt-app/src/app.rs`), t1833 (`src/apps/kt-app/src/idle_tests.rs`), t20 (`src/libs/kt-engine/tests/player_dynamic.rs`)

#### Scenario: The meters converge and release the repaint, and go out when paused
- **WHEN** the sound has stopped or the transport is stopped
- **THEN** the meter's "activity" means the movement of the bars, not "the bar is above the minimum"; the decay converges to the floor within the normal fall time and the repaint is released; the measurement under the cursor holds only while the cursor is moved; the glow of a row depends on its own dB row, not on the level along the whole height
- **TEST** t38, t121 (`src/apps/kt-app/src/meters.rs`)

#### Scenario: The preview key is aligned to the grid, V-fps is honest
- **WHEN** the playhead moves within one frame cell
- **THEN** the preview key does not change and the texture is not re-uploaded; the V-fps counter of the performance panel counts the NEW frames shown, not the ui frames
- **TEST** t39 (`src/apps/kt-app/src/app.rs`)

#### Scenario: The meters' glow is one mesh
- **WHEN** the meters with the zonal glow are drawn
- **THEN** the glow is one vertical mesh with stops at the zone boundaries (not hundreds of ellipses), is drawn BEFORE the LED segments of both bars and does not go onto the panel
- **TEST** t40 (`src/apps/kt-app/src/meters.rs`)

## ADDED Requirements

### Requirement: The program at rest costs nothing, and this is measured on a heavy project
On a pause the work counters (`kt_time::work`) SHALL stand still: no UI frame, composite, device
submit, decode order, prefetch pass, thumbnail, effect cell or composition preview, with the effects
window open or not. A pointer passing over a static place SHALL cause no composite, device submit,
decode order or job of any kind, and SHALL NOT lay out the timeline's events: they are replayed as
ready meshes until what lies under the pointer changes. Any frame with input other than the pointer
drops those meshes. The verdict is the counters and not percent: a machine with no card rasterizes in
software, and every frame looks expensive there.
Why: the owner, 2026-11-07: "ЕСЛИ Я ПРОСТО повожу!!! ПОВОЖУ МЫШКОЙ !!! в любом статичном месте программы ...
Причем это зависит от позиции скрола таймлайна", and "ПОЧЕМУ АГЕНТАМ НЕ ПРИКАЗЫВАЕТСЯ ТЕСТИРОВАТЬ ЭТО?!?"
Every green test ran an empty or a light project.

#### Scenario: A pause, a pointer and an open effects window on the heavy project
- **WHEN** the heavy project (`kt-cli showcase --heavy`) has gone quiet after opening, and the pointer goes over the preview, the status bar and a composition, and its effects window opens
- **THEN** on the pause every counter stands still; over each static place no composite, submit, decode order, thumbnail or job runs, and event passes are at most a third of the frames; the program is at rest again a moment after the pointer stops; the open effects window runs nothing
- **PATH** open the project and touch nothing → the picture stands, Diagnostics would read "at rest"; move the mouse over the preview, the status bar, then a composition → only the outline under the pointer changes; right click the composition → Effects… → the window shows Slowed, VHS Tape and Camera ISP, and nothing runs
- **TEST** live `idle` (`src/scripts/live/idle.py`)

#### Scenario: A hover frame does not lay out the events
- **WHEN** the pointer moves over the timeline with nothing changing under it, then a frame with other input arrives
- **THEN** the pointer's frames lay the events out once, and the other frame lays them out again and keeps nothing
- **PATH** move the mouse along an empty part of a track → only the pointer moves; move it onto an event → that event's outline lights up; press any key → the picture is drawn anew
- **TEST** t1834 (`src/apps/kt-app/src/idle_tests.rs`)
