## ADDED Requirements

### Requirement: One local journal, stamped at the moment of the event
The program SHALL keep one journal, `telemetry-journal.log` in the data directory, that never leaves the
machine and that the usage telemetry never reads. Every record SHALL carry the local wall-clock time
with milliseconds and its offset, and the seconds since the process started, both taken when the event
happened by the code that saw it, not when the record is written or read. A record SHALL reach the file
before the next thing happens: one writer thread, blocked while nothing comes, appends each record with
no user-space buffer, so a killed process loses nothing it recorded; a panic flushes it. Past 2 MiB the
journal SHALL rotate into `.1` and `.2`. While nothing happens the journal SHALL cause no wake-up of
any thread. It SHALL hold no media content and no path beyond a file name.
Why: the owner, kkt-26.11.07b G6/G10: "сделал отложенную запись и я не могу тебе сказать точное
время"; "Не догадался сделать общую телеметрию - взять playback journal и втроить в журнал телеметрии,
куда и load-report и playback и действия будут входить"; "телеметрия != трекинг использования в posthog".

#### Scenario: A record carries the time of its event
- **WHEN** an event is recorded at 10:00:00.250 by a fake clock and the clock has moved an hour before the line is written and read
- **THEN** the line says 10:00:00.250 and the monotonic offset of the event
- **TEST** t1960 (`src/apps/kt-app/src/tele_journal.rs`)

#### Scenario: A killed process keeps what it recorded
- **WHEN** a child process records lines, its writer has written them, and the child is killed with SIGKILL
- **THEN** the file holds every line whole, with no explicit flush or close
- **TEST** t1962 (`src/apps/kt-app/src/tele_journal.rs`)

#### Scenario: Rotation bounds the journal
- **WHEN** records pass the rotation size several times
- **THEN** there are at most the journal and `.1`, `.2`, each under the size plus one line, and the newest record is in the journal
- **TEST** t1963 (`src/apps/kt-app/src/tele_journal.rs`)

#### Scenario: At rest the journal costs nothing
- **WHEN** the journal's writer has nothing to write for a second and a half
- **THEN** its thread made no context switch in that time
- **TEST** t1964 (`src/apps/kt-app/src/tele_journal.rs`)

### Requirement: The journal holds the load, the playback, the notes and the person's actions
Into the journal SHALL go: each busy second of the load watch (threads, work counters, "NO COUNTER
MOVED") and the return to rest; every record of the playback black box as it is pushed (play, pause,
seek, loop region, direction, wrap, stall, underrun snapshot), a run of repeats as one record with
its count and the time of its last repeat; a summary at each stop of playback (length, V-fps, new
frames, skipped cells, rollbacks, underruns); the Diagnostics notes (underruns, eviction spikes,
no-read drops, low video hit, offline media) raised by the load watch at the second they happen; every
`kt-app.log` line, notice and export log line; and the person's actions: an edit by command name and
target, undo, redo, a window opened or closed, the effect selected in a chain, a project opened or
saved (by file name), and a mark he sets with Ctrl+Alt+M or the Diagnostics button, with an optional
note. Every existing record of the program SHALL either be folded in or have its reason to stay apart
written in the capability's design.
Why: the owner could not say what he did at "487 s НАВЕРНОЕ"; the black box, the load report and the
Diagnostics log answered three halves of one question.

#### Scenario: An action shows up with its name
- **WHEN** an edit, an undo, a window opened and closed, a playback started and stopped and a mark with a note pass through the app's doors
- **THEN** the journal holds each in order with its name: the command, "undo", the window's title, PLAY and PAUSE with the summary, MARK with the note
- **TEST** t1961 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: The playback black box goes into the journal with its repeats closed
- **WHEN** the flight ring gets a play, a hundred identical wraps and a pause
- **THEN** the sink hands over play, one wrap, the run of a hundred with the time of its last repeat, and pause
- **TEST** t1965 (`src/libs/kt-engine/src/flight.rs`)

#### Scenario: A note is stamped at the second it happened
- **WHEN** the load watch's probe sees two new callback underruns and a no-read drop in one sample, with no UI frame
- **THEN** the journal has NOTE records stamped with that sample's time
- **TEST** t1966 (`src/apps/kt-app/src/load_watch.rs`)

### Requirement: The load report is the journal's last quarter of an hour
`load-report.txt` SHALL be the export of the journal's last 15 minutes, every kind, oldest first, each
record with its clock time, under the machine header. It SHALL be written by itself after three seconds
of a whole core (at most once in five seconds) and by the Diagnostics button; the load watch SHALL write
each busy second into the journal at that second.
Why: the owner's report said "487 s ago" from the moment it was written, so the seconds moved under
him between two reads.

#### Scenario: The report says clock time and holds the actions
- **WHEN** the journal holds a load second, an edit and a mark, and the report is written later
- **THEN** each appears with its own clock time, none says "s ago"
- **TEST** t1967 (`src/apps/kt-app/src/load_watch.rs`)

## MODIFIED Requirements

### Requirement: The load watch never calls uncounted work rest
A second in which a thread other than the watch's own holds 10 % of a core or more while no work counter
moves SHALL NOT read "at rest": the journal's LOAD record and the report say "NO COUNTER MOVED" and
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
