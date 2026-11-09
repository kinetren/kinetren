## ADDED Requirements

### Requirement: A paused export waits on an event
While the render window's Pause holds, the export's frame loop and its pipeline's gate SHALL wait on a
condition that the pause, the resume, the cancel and the writer notify, never on a timer.
Why: row 271 — a paused export slept 40 ms and its pipeline waited 50 ms in a loop.

#### Scenario: A paused export does not wake until resumed
- **WHEN** an export is paused for half a second and resumed
- **THEN** its pause wait wakes at most once (the resume) and the export finishes with the same frames
- **TEST** t2261 (`src/libs/kt-engine/tests/export_pause.rs`)

### Requirement: A render's decoder settings are its own
A render SHALL open its sources with the decoder thread and RGBA thread counts it chose for its size and
the free memory; a source opened at the same time by anyone else (the preview, a thumbnail) SHALL take
the process's defaults. The settings are scoped to the thread that opens the render's sources, not set
process-wide.
Why: row 94 — the render window set the process-wide counts around its opening, and a second decoder
of a file opened meanwhile took the render's settings, or reset ones.

#### Scenario: Another thread's open takes the defaults
- **WHEN** a thread opens inside a render's settings while another thread asks at the same time, and the first asks again after its scope
- **THEN** the first sees the render's counts inside, the other and the first after it see the defaults
- **TEST** t2262 (`src/libs/kt-ffmpeg/src/lib.rs`)
