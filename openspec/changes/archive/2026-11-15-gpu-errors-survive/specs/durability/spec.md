## ADDED Requirements

### Requirement: A GPU error never ends the program
Every wgpu device the process creates or receives (the window's device from eframe, every headless
device, the export's devices, the compute host, the D3D12 check) SHALL carry an uncaptured-error
handler and a device-lost callback that write the error (its role, kind and cause chain on one line;
the first twenty per role in full, then every hundredth with its count) to the program's log and
telemetry journal, or to stderr in kt-cli, and never panic. The window SHALL survive a refused
`Surface::configure`: the display engine's frames and every other submission on the window's device
off the UI thread SHALL wait, at most 250 ms, until the UI has painted twice, so egui's reconfigure at
the next paint finds the queue idle, and the window takes its new size while a loop plays.
Why: the owner, kkt-26.11.15 item 1 (N1): kt26.11.10 died on a double click on the window title
while a region looped; the report was wgpu's default handler panicking on "In Surface::configure —
Failed to wait for GPU to come idle", the preview's thread having submitted on the same device.

#### Scenario: An uncaptured error is written, not a panic
- **WHEN** a headless device we opened is asked for a buffer whose usage validation refuses, outside any error scope
- **THEN** the process goes on, the headless role's error count grows by one, and the device serves the next buffer and submission with no new error (before: wgpu's default handler panicked)
- **TEST** t2720 (`src/libs/kt-gpu/tests/gpu_errors.rs`)

#### Scenario: A refused reconfigure holds the display for two UI frames
- **WHEN** the display role's handler sees a `Surface::configure` error, then UI frames tick; and when another error or another role's configure error comes
- **THEN** the display is held for exactly two ticks; other errors hold nothing; a held frame waits at most 250 ms when the UI stops ticking
- **TEST** t2722 (`src/libs/kt-gpu/src/errors.rs`)

#### Scenario: An error line carries its cause on one line
- **WHEN** a validation error with a multi-line cause chain is described
- **THEN** the line names the kind, holds the whole chain joined on one line, and is cut at 600 bytes on a character boundary
- **TEST** t2723 (`src/libs/kt-gpu/src/errors.rs`)

#### Scenario: Maximizing and resizing while a loop plays
- **PATH** open the demo project (generators, no video) with a 2 s loop → press Space → the loop plays → double-click the free part of the title strip: the window fills the screen and the loop goes on → double-click again: the window comes back to its size → four more toggles, then four resizes by the window's edge: each time the window takes the size and the loop still plays → the program never closed; the log holds a "display device: validation error … Surface::configure" line where the queue was busy
- **TEST** t2724 live scenario `src/scripts/live/resize_loop.py`
