## ADDED Requirements

### Requirement: The memory numbers say what they count and name the whole process
The status bar SHALL show the decoded frames held against the bound they may grow to now, and the
program's resident memory, with a hint saying what each is; it SHALL NOT show a pair a person reads as
used/limit when it is two different caches. The Diagnostics window SHALL show the decoded frames'
occupancy of their bound now, the caches' line (kept composites, history, frames, the kept mix, device
buffers, the room) and under it the program's memory: what the caches hold of it and the rest, named as
the decoders, the graphics driver and the interface. The process's memory SHALL be read at most once a
second, in frames drawn anyway.
Why: found-defects 397 — "cache: 39/535 MB" was audio/video and read as used/limit, Diagnostics said
"100 % (535/537)" of a bound that never moved, and 4.7 GB of the program were named nowhere
(kkt-26.11.10 J4).

#### Scenario: The status bar says frames against their bound and the program
- **WHEN** the editor is open with a project
- **THEN** the status bar shows "frames: V/cap MB" with the cache's bytes and its bound now, and "memory: P MB" with the process's resident memory
- **PATH** look at the status bar → "frames: 0/512 MB · memory: 600 MB"; hover either → the hint says the frames are decoded frames against the bound that grows with the loop and quality, and the memory is all the program holds; play a loop → the bound grows past 512
- **TEST** t2397 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: Diagnostics names the whole process
- **WHEN** the Diagnostics window is open
- **THEN** under the caches' line a line gives the program's memory, the caches' part and the rest, the rest being the program less the caches
- **PATH** open View → Diagnostics… → under the cache occupancy bar "Caches: kept … · mix … MB", then "Program: P MB — caches C, the rest R (decoders, graphics driver, interface)"
- **TEST** t2398 (`src/apps/kt-app/src/diag_tests.rs`)
