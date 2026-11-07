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

### Requirement: At rest the work counters stand still
On a pause with nothing moving, on a project that carries heavy effects, the work counters
(`kt_time::work`) SHALL stand still once what the opening started is over: no UI frame, composite,
device submit, decode order, prefetch pass, thumbnail or composition preview. The verdict is the
counters: a machine with no card rasterizes in software, and every frame looks expensive there.
Why: the owner, 2026-11-07 (E1): on a pause, with nothing happening, the program held the whole
processor. Every green test ran an empty or a light project.

#### Scenario: A pause on the heavy project
- **WHEN** the heavy project (`kt-cli showcase --heavy`) has gone quiet after opening, and nothing is touched
- **THEN** every counter stands still
- **PATH** open the project and touch nothing → the picture stands, and nothing runs behind it
- **TEST** t1830, t1833 (`src/apps/kt-app/src/idle_tests.rs`)
