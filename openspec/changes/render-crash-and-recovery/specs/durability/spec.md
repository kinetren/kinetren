## MODIFIED Requirements

### Requirement: The log and crash report — a channel for when there is no one to show it to
The program SHALL keep a journal `kt-app.log` in the data directory, ONE FILE PER RUN: each start
SHALL move the previous run's journal to `kt-app.log.1` (shifting `.1`…`.3` to `.2`…`.4`; four
previous runs are kept) and begin the new one with a header line naming the build version, the OS
and architecture, the FFmpeg major (or that FFmpeg is missing), the process id and the start time
in local time with its UTC offset. Every record SHALL be stamped in local time with the offset
(`YYYY-MM-DD HH:MM:SS+HH:MM`). Within a run the journal appends; past 1 MiB it shifts the same fan.
A failure to write the journal MUST NOT bring down the program. The panic hook SHALL leave a report
with the version, thread, message, backtrace with names, and a dump of the "black box", without
touching the model; the next start takes the report exactly once (it moves to `.seen` and goes to
telemetry), and announces it in the bell only when no crash window showed it (the report carries
no `shown:` line). The "black box" MUST be readable even under a poisoned mutex.
Why: the window's `on_exit` no longer exists, and a `.desktop` with `Terminal=false` sends stderr
nowhere — that is exactly where the final record lives; the hook is called in the panicking thread
(invariants 2 and 6 forbid going into the engine), and a second `unwrap` inside the hook is an
`abort` with no report. The owner (kkt-26.11.01 item 9): "it seems the log from an old run stayed,
i.e. we have no log rotation" — the journal appended across runs in UTC without saying so, and the
lines of the crashed run could not be told from the others.

#### Scenario: The journal appends and rotates
- **WHEN** two records are written, then the file goes past the threshold and one more is written
- **THEN** both lines are in place with dates; after the threshold the old content goes to `.log.1` and the new starts from zero; an unavailable path is silently skipped
- **TEST** t354 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: Each run has its own journal with a header
- **WHEN** a run writes two records, then a second run starts and writes one, then five more runs start
- **THEN** after the second start `kt-app.log` holds the second run's header and record only and `kt-app.log.1` the first run's; the header names the version, the OS, the FFmpeg major, the pid and a time with an offset; after the seven runs `kt-app.log.4` exists and `.5` does not; the tail of the last run returns only lines after the last header
- **PATH** start Kinetren → work → close it → start it again → open the log folder: `kt-app.log` begins with a header line of today's run, the previous run is in `kt-app.log.1`
- **TEST** t1075 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: The stamp is local time with its offset
- **WHEN** one moment is stamped with the offsets +03:00, −05:30 and 0
- **THEN** the stamps read the shifted wall-clock time followed by `+03:00`, `-05:30` and `+00:00`, and the date rolls over when the offset crosses midnight
- **PATH** open the log folder → `kt-app.log` → every line starts with the local time and its offset, e.g. `2026-11-01 18:10:12+03:00`
- **TEST** t1079 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: The crash report is complete and is given once
- **WHEN** the hook forms the report and the program starts after the crash
- **THEN** the report holds the version, thread, message, function names, and the dump of the box; the second start stays silent about the crash, and the report lies in `.seen`
- **TEST** t375 (`src/apps/kt-app/src/journal.rs`), t374 (`src/apps/kt-app/src/journal.rs`), t373 (`src/libs/kt-engine/src/flight.rs`)

#### Scenario: A crash the window showed is not announced again
- **WHEN** a report marked `shown:` and then an unmarked one are taken at start
- **THEN** both are taken (moved to `.seen`, handed to telemetry); the bell announces only the unmarked one
- **PATH** Kinetren crashes → the crash window → Restore the project → the editor opens without a second "the previous session ended abnormally" line in the bell
- **TEST** t1078 (`src/apps/kt-app/src/journal.rs`)

### Requirement: Recovery is offered by time and placed into the editor
At startup and by the menu item "Recover unsaved…" the program SHALL collect all candidates (recoveries next to the project, including the files of second copies, and unnamed work) and offer recovery of the project only if it is STRICTLY newer than the project; unnamed work is always offered, the freshest first.
The dialog shows the path and the time; "Recover" puts the work INTO THE EDITOR with an asterisk and MUST NOT consume the source; "Delete" removes only this file; "Not now" touches nothing. An empty list on a direct request from the menu SHALL say so in words.
"Recover" SHALL build the editor from the recovery file as an Open does: the player and every
source are raised from the restored document (nothing of the document it replaces may answer at
the same media number), and the document takes the name the recovery belongs to — that project,
or none for unnamed work. A lesson sandbox SHALL NOT become `last_project`: it is not stored, and
one stored by an older build is not opened at start.
Why: the chain "argument → last_project → autosave" chose by order, not by time — "opened A, New, an hour of work, a crash" was lost silently; a comparison without strictness would call for recovering what is already in the file after every Ctrl+S. The owner's crash (kkt-26.11.01 item 9): an imported, unsaved project was restored into the opened lesson — the model was swapped but the lesson's sources stayed at the same media numbers (the edit played SMPTE bars and noise), and the title, the lock and Ctrl+S kept pointing at `lesson-5.kt`, which had come back only because the sandbox was stored as `last_project`.

#### Scenario: Selection by mtime
- **WHEN** the recovery is older than the project / newer than the project / unnamed
- **THEN** not offered / offered / always offered and first, if it is fresher
- **TEST** t365 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Unnamed work is visible even when a project is open
- **WHEN** `last_project` exists, and unnamed work lies in the data directory
- **THEN** the recovery window opens; an empty list from the menu gives a notification
- **TEST** t366 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Recovery is repeatable and does not write to the project
- **WHEN** the person recovers a candidate, then cancels and recovers it again
- **THEN** the model is in the editor, the asterisk is lit, and the project file and the source are untouched both times
- **TEST** t367 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: "Delete" and "Not now"
- **WHEN** there are two candidates; "Delete" on the first, then on the second
- **THEN** exactly the selected one is removed; the window lives while candidates remain and closes on an empty list; "Not now" leaves the file
- **TEST** t368 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A lesson sandbox is never the last project
- **WHEN** the editor stands in a lesson sandbox and eframe asks it to save; and, separately, the start plan reads a `last_project` that is a sandbox path (an older build stored it)
- **THEN** no `last_project` entry is written for the sandbox; the start plan opens nothing from it and, with a recovery pending, offers the recovery over an empty editor
- **PATH** Help › lessons → lesson 5 → File › Import older… a project → the program closes abnormally → start Kinetren → the recovery window opens over an empty editor, not over `lesson-5`
- **TEST** t1076 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Recovery raises the restored document's own sources
- **WHEN** a lesson-like project whose media 1 is a generator is open, and the unnamed recovery holds a different document whose media 1 is a file; the person presses "Recover"
- **THEN** the player's media 1 is the recovery's file, not the generator; the editor is unnamed (no `lesson` path, title, lock or save target), the asterisk is lit, and the recovery file is untouched; a project's recovery restores under that project's name
- **PATH** the recovery window → Restore into the editor → the timeline shows the restored edit with its own pictures in the preview (no SMPTE bars, no noise), the title is the untitled project's
- **TEST** t1077 (`src/apps/kt-app/src/layout_tests.rs`)

## ADDED Requirements

### Requirement: A crash is shown right after it happens, by a supervisor process
`kinetren` SHALL run as a thin supervisor that starts the editor as its child (the same binary,
`KT_SUPERVISED=1`) and waits; the supervisor initialises no window, GPU, FFmpeg or telemetry until
the child ends. A child that exits with code 0, or is stopped on request (SIGTERM, SIGINT, SIGHUP),
SHALL end the supervisor without a window. Any other exit — a panic (code 101), an abort, a fatal
signal, a non-zero code, a Windows exception code — is a CRASH: the supervisor SHALL take the fresh
panic report (written after the child started) or write one itself naming the exit, mark it
`shown:`, and open the crash window at once. The window SHALL show what happened in words (the
panic message, or the signal or code with its meaning), the build version, the report's path and
the journal lines of the crashed run, with the buttons **Restore the project** (starts the editor
again; it offers the recovery and says so when there is nothing to recover), **Open the log
folder** and **Close** outside the scrolling body. On Linux the editor SHALL die with the
supervisor. `KT_NO_SUPERVISOR=1` runs the editor in the first process. The supervisor and the
window add no link-time dependency (invariant 10).
Why: the owner, 2026-11-01: "I want the crash window right after the crash, not who knows when".
A dying process cannot show a window reliably: the hook runs on the panicking thread inside the
event loop that is unwinding, and a signal or an abort never reaches a hook. Before this change
the report waited for the next start and a non-panic death left nothing.

#### Scenario: A child's panic is detected and its report taken
- **WHEN** the supervisor runs a child that writes a journal line and panics with the panic hook installed (its data directory a temporary one)
- **THEN** the verdict is a crash with code 101, the panic message is the child's, the report is the child's own (version, backtrace) and now carries `shown:`, and the journal tail holds the child's line
- **PATH** Kinetren closes on an internal error → at once a small "Kinetren closed unexpectedly" window shows the error message, the version and the last log lines
- **TEST** t1072 (`src/apps/kt-app/src/supervisor.rs`)

#### Scenario: Signals and exits are classified
- **WHEN** the supervisor runs a child that aborts, one killed by SIGKILL, one stopped by SIGTERM, and one that exits cleanly (Linux)
- **THEN** the abort is a crash named SIGABRT with a report the supervisor wrote; SIGKILL is a crash whose words mention the system ending the program (often memory); SIGTERM and the clean exit open no window; the pure classification also names the Windows codes 0xC0000005, 0xC00000FD and 0xC0000409
- **PATH** the system kills Kinetren (out of memory) → the crash window says the system ended the program, with the last log lines
- **TEST** t1073 (`src/apps/kt-app/src/supervisor.rs`)

#### Scenario: The crash window says what happened and offers the way back
- **WHEN** the crash window is drawn for a panic with a journal tail, and the buttons are clicked
- **THEN** it shows the headline, the panic message, the version, the report path and the tail lines; Restore the project, Open the log folder and Close are visible below the scrolling body; Restore returns "restart", Close returns "close"
- **PATH** the crash window → read the error and the log lines → press Restore the project → the editor starts again and the recovery window offers the unsaved work
- **TEST** t1074 (`src/apps/kt-app/src/supervisor.rs`)
