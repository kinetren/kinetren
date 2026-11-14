## ADDED Requirements

### Requirement: The trace names the machine the load report described
Every run's trace SHALL carry, as machine facts at the head of every segment, what the load report's
header said: each adapter the machine offers (`adapter.<i>`: name / kind / backend / vendor:device),
the cards the monitors hang off (`monitors`, "unknown" where the system does not say), the power
choice the UI and offline work run on (`power`), the run's start in local time with milliseconds and
its offset (`clock`), and the FFmpeg the run loaded (`ffmpeg`). The editor SHALL record the devices at
its start from the one adapter listing it makes before the window's device opens; `kt-cli --trace`
SHALL record them too. `kt-cli trace machine` SHALL print the run's label and its start (UTC) before the facts,
`summary` SHALL end with the same lines, and `dump` SHALL start with one JSON line holding them. No
`load-report.txt` SHALL be written. The Diagnostics load section's button SHALL open the trace
folder `<data>/trace` (made if absent).
Why: the owner, kkt-26.11.14 M3: "the load report moves into the trace; `load-report.txt` goes; the
button opens the trace folder". found-defects 487: the header's facts were only in a file nothing
read.

#### Scenario: The machine facts from what was read
- **WHEN** the facts are built from an integrated Radeon and llvmpipe, the monitor on the Radeon, LowPower; and from nothing, HighPerformance
- **THEN** `adapter.0` and `adapter.1` name each with kind, backend and PCI ids, `monitors` names the Radeon, `power` LowPower; with nothing, only `monitors` "unknown" and `power`
- **TEST** t2570 (`src/libs/kt-gpu/src/context.rs`)

#### Scenario: The trace's clock is local time with its offset
- **WHEN** the start 2025-11-07 07:00:00.250 UTC is written at +03:00, -05:30 and 0
- **THEN** it reads 10:00:00.250+03:00, 01:30:00.250-05:30 and 07:00:00.250+00:00
- **TEST** t2571 (`src/apps/kt-app/src/tele_journal.rs`)

#### Scenario: The query shows the machine
- **WHEN** a run carries the clock, the cores, two adapters, the monitors and the power choice
- **THEN** `machine` names the run, its UTC start and every fact; `summary` carries the same lines; `dump`'s first line is JSON with the facts, then one line a record
- **TEST** t2572 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: The button opens the trace folder
- **WHEN** Diagnostics is open and its load section's button is clicked with no trace folder yet
- **THEN** the button reads "Open the trace folder", the folder exists after the click, and no `load-report.txt` is in the data directory
- **PATH** open Diagnostics (toolbar) → the Performance section shows the processor's second and the button "Open the trace folder" → click it → the file manager shows `<data>/trace`
- **TEST** t2573 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: The real program's trace names its machine
- **WHEN** the live `idle` scenario opens Diagnostics on the heavy showcase and clicks the button
- **THEN** the program asked to open `<data>/trace`, `kt-cli trace <data>/trace machine` names `adapter.0`, `monitors`, `power` and `clock`, and no `load-report.txt` exists
- **PATH** the heavy showcase open → toolbar "Diagnostics" → "Open the trace folder" → the trace folder opens
- **TEST** live `idle` (`src/scripts/live/idle.py`)

## MODIFIED Requirements

### Requirement: The load watch never calls uncounted work rest
A second in which a thread other than the watch's own holds 10 % of a core or more while no work counter
moves SHALL NOT read "at rest": the journal's LOAD record says "NO COUNTER MOVED" and
Diagnostics says that no counter moved. Model edits, project swaps and engine builds SHALL have work
counters (`model_edits`, `project_swaps`, `engine_builds`). The live scenario `idle` SHALL fail a pause
that holds such a thread.
Why: the owner's report read "at rest" at 196 % (its lines 482 and 512) while prefetch and the audio
render rebuilt the engine on every mouse move: the counters did not see that work, and the verdict
believed them. A tool made for proof must not hide what it was made to show.

#### Scenario: A busy thread with no counter
- **WHEN** a second has prefetch at 95 % and the audio render at 92 % of a core and every counter still, and another has only the watch's own tick
- **THEN** the first is not rest and reads "NO COUNTER MOVED", the second is rest
- **TEST** t1880 (`src/apps/kt-app/src/load_watch.rs`)

## REMOVED Requirements

### Requirement: The load report is the journal's last quarter of an hour
**Reason**: the owner's decision kkt-26.11.14 M3: `load-report.txt` goes. Its body was the journal's
own records (each with its clock time, in `telemetry-journal.log` and mirrored into the trace); its
header is the trace's machine facts now. t1967 goes with it.
**Migration**: read `kt-cli trace <data>/trace machine|summary|dump`, or `telemetry-journal.log`.
