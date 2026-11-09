# Keeping the work safe

## Purpose

The editor's work is not lost on a power cut, nor on a crash or killing of the program, nor with a second copy on the same project, nor by the "Cancel" button: the project file is changed only by an explicit save; everything else goes to the recovery file and is offered back.

## Requirements

### Requirement: Atomic project write is honest to power loss
Any write of the project file (GUI and kt-cli) SHALL go through one home — `kt_otio::write_atomic` — in the order "write and flush the TEMPORARY file → rename → flush the directory". The temporary name MUST be appended to the FULL name of the project and carry the process number. A failure of directory sync MUST NOT count as a write failure: the file is in place and intact, and the caller learns of the degradation through a flag and says so once per session.
Why: `write + rename` without sync survives a process crash but not a power cut — the rename may land in the journal before the data, and the project file in place is left zero bytes long; exFAT and network shares cannot sync a directory, and turning that into an error would mean claiming the save did not happen.

#### Scenario: The sync order is observed by injection, not by fault injection
- **WHEN** the project is written through a substituted file system that records the calls
- **THEN** the call log is exactly "write+sync `<name>.<pid>.tmp` → rename → sync dir", there is no temporary file left after the write, and swapping `sync_dir` before `rename` turns the test red
- **TEST** t351 (`src/libs/kt-otio/tests/io.rs`), t150 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Directory not synced — the write still succeeds
- **WHEN** `sync_dir` answers `Unsupported`
- **THEN** the write returns success with `dir_synced == false`; the file is intact and readable
- **TEST** t353 (`src/libs/kt-otio/tests/io.rs`)

#### Scenario: kt-cli writes with the same code and does not overwrite without `--force`
- **WHEN** `kt-cli new/convert/demo/roundtrip` receives the path of an existing file
- **THEN** without `--force` it refuses with a hint; with `--force` it is an atomic write with no leftover temporary file
- **TEST** t387 (`src/apps/kt-cli/tests/cli.rs`)

### Requirement: The write outcome is a type, and a failure does not look like success
A guarded write SHALL return an explicit outcome (`Wrote { dir_synced }` / `SkippedNewerOnDisk`), and each caller MUST handle both; a refusal by the guard or an I/O error MUST NOT clear the asterisk, change the file, or print "saved". A save error is a warning (Warn), and its repeat MUST turn the counter of unread notifications red.
Why: a `bool` with `.map(|_| ())` turned a guard refusal into success — the asterisk went out, and there was no file with the edits; collapsing duplicates by an early return silenced the only channel of the message exactly when the error repeated.

#### Scenario: A guard refusal leaves the asterisk and the file
- **WHEN** on disk there is a revision "from the future" and the user presses Ctrl+S
- **THEN** the file does not change, `dirty_content` remains, and a notification is present that does not look like success
- **TEST** t355 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The revision guard does not go backward
- **WHEN** a write with an old revision arrives after a write with a new one
- **THEN** the old one does not touch the file, the new one writes; the background autosave does not overwrite a fresh Ctrl+S
- **TEST** t199 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A repeated warning is visible
- **WHEN** the same warning arrives three times
- **THEN** there is one line with `repeats == 3` and `notices_unread == 3`, the text without "×N", and an error from Ctrl+S arrives as Warn
- **TEST** t356 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Autosave writes to a recovery file, only Ctrl+S changes the project
Background writing SHALL go to `<full project name>.recovery.kt` next to the project (an unnamed project — to the data directory) and MUST NOT touch the user's file or clear the asterisk. The recovery file SHALL be of the same format as the project: writing megabytes of the exchange format every half minute is both interface lag and a window of loss on a crash. A successful save SHALL remove only ITS OWN recovery. Exiting the program SHALL write the recovery, not the project; a clean exit takes its own recovery away. Drift of the viewport (zoom, scroll, cursor) MUST NOT start a write. There are TWO revision guards — one for the project, one for the recovery. The gate "not during a gesture and not during playback" SHALL yield after two quiet intervals.
Why: the owner's decision — there must be a SECOND state on disk, otherwise there is nowhere to roll back to; a shared guard raised the bar with background writes, and the next Ctrl+S turned out "stale"; a substitution of the extension merged `film.otio` and `film.mp4.otio` into one recovery; a zoom every 30 s ran a full serialization without a single edit. R.70: the owner's autosave wrote 115 MB, because the project lived in the exchange format and the body of the reel was repeated in every instance.

#### Scenario: Background write does not touch the project and does not clear the asterisk
- **WHEN** there is a content edit and the autosave interval has passed
- **THEN** the project file is byte for byte unchanged, `dirty_content` stays lit, and the recovery holds the model with the edit
- **TEST** t358 (`src/apps/kt-app/src/layout_tests.rs`), t194 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The recovery name is appended to the full name
- **WHEN** the projects are `film.kt` and `film.mp4.kt`
- **THEN** they have different recovery files next to the project; for an unnamed one — the data directory
- **TEST** t357 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Ctrl+S removes only its own recovery
- **WHEN** its own and someone else's recovery lie next to each other and the user saves
- **THEN** its own is removed, the other's is intact, and the `recovery_written` mark is cleared
- **TEST** t359 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Viewport drift does not write, an edit does
- **WHEN** only the zoom and scroll changed on a clean project
- **THEN** no background write starts; after a content edit it starts
- **TEST** t360 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The recovery guard does not block saving
- **WHEN** the background recovery write has raised its own guard
- **THEN** the project guard stays at zero, and Ctrl+S of the same revision passes
- **TEST** t361 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Edits inside a gesture advance the revision
- **WHEN** the FX window or a drag of an edge edits the model via `apply_coalesced`
- **THEN** the revision grows and autosave does not count the edit as saved
- **TEST** t200 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Autosave of a large project stopped being an event
- **WHEN** a project with a reel in several instances (a scene that gives megabytes in the exchange format) is written by autosave
- **THEN** the recovery file is orders of magnitude smaller than the same project in the exchange format, and the program's own work on the write, counted on the writing thread's processor clock, stays under 2 s even in the debug build; the time a disk takes over its syncs does not count, and it holds up the autosave thread, not the interface
- **TEST** t815 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Live SIGKILL
- **WHEN** an edit by a real gesture → background write → `SIGKILL` → restart
- **THEN** the project file is untouched, the program itself offers recovery, and the work is in place — and this twice over
- **TEST** live `recover.py` (`src/scripts/live/recover.py`, via `src/scripts/live-check.sh`)

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

### Requirement: Unsaved work is not silently discarded; Drop is the last line
"New", "Open", and closing the window with unsaved work SHALL ask with one guard that has THREE answers (save / do not save / stay); "Save and continue" MUST NOT continue if the save did not succeed. Closing the window is intercepted in the standard way (`CancelClose`). The application's `Drop` SHALL write the recovery in cases where `on_exit` is not called (re-creation of the application in place, unwinding of the stack from a panic), and its body MUST NOT contain a single `unwrap`; a poisoned guard is no reason not to write. A clean project is not hindered by the guard. Work the program does by itself — a reel's preview, a poster, an analysis — is not an edit and MUST NOT mark the project dirty: a project opened and left untouched gets no question and leaves no recovery file.
Why: the right not to ask was legitimate while autosave wrote over the user's file; the owner removed that — together with the right to stay silent. A `bool` instead of three answers lost the edit on the "Cancel" button; a panic inside `Drop` during unwinding is an `abort`, which takes away exactly the file the whole thing was for. A finished reel preview set the convenience flag, so every project with a reel asked "save your changes?" untouched (found by the README pipeline, 2026-11-03).

#### Scenario: "New" and "Open" ask
- **WHEN** the project is dirty and the user presses "New" or "Open"
- **THEN** the question is asked; "Stay" changes nothing; "Do not save" lets go; a clean project gets no question
- **TEST** t369 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A failed save stops the transition
- **WHEN** the answer is "save", and the write was refused
- **THEN** the work stays in place, the asterisk is lit, and a warning is present
- **TEST** t370 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Window close is intercepted
- **WHEN** the window is closed on a dirty project
- **THEN** exit is not permitted until an answer is given; "Stay" keeps the program; "Do not save" allows exit without a second question
- **TEST** t371 (`src/apps/kt-app/src/layout_tests.rs`), live `unsaved.py` (`src/scripts/live/unsaved.py`)

#### Scenario: Drop writes the recovery even under a poisoned guard
- **WHEN** an application with a dirty model is destroyed, and the guard's mutex is poisoned by a panic
- **THEN** the recovery is written with the same work, and the project file is not created; a clean application writes nothing
- **TEST** t372 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A background reel preview is not an edit
- **WHEN** a project with a reel is opened and nothing is touched until its reel preview has arrived; then "New" is pressed, and the application is destroyed
- **THEN** both dirty flags are clear, the guard lets "New" through with no question, and no recovery file is left
- **TEST** t1413 (`src/apps/kt-app/src/layout_tests.rs`), live `readme_shots.py` (`src/scripts/live/readme_shots.py`, the welcome step: a template replaces the untouched showcase without a question)

### Requirement: Project lock — flock on an adjacent file, a second copy degrades honestly
An open project SHALL take the core lock on a file in the data directory: `flock` on Linux,
`LockFileEx` on Windows (the key — a hash of the absolute path; the path inside). The key on Windows MUST
be computed from a normalized path: without the `\\?\` prefix, with one kind of separator and without
regard to case, because `C:\Proj\a.kt` and `c:\proj\A.kt` are one file. The process number of the holder
SHALL lie in a separate file next to the lock: the Windows lock is mandatory, and
a locked file cannot be read by a second copy. The lock is placed not on the project itself and
not globally on the program. A second copy MUST NOT be forbidden: it gets a warning with
the number of the other process (or without a number, if its file could not be read), ITS OWN
recovery file, ITS OWN temporary file, and one question at the first Ctrl+S of the session. kt-cli warns
about the busy state before writing, but does not refuse.
Why: a global lock would have killed live scenarios and "opened a second time to look"; a lock
on the project's inode would fall off after the first atomic `rename`; a file with a pid without a
kernel lock would leave a permanent block after `SIGKILL` (the pid file here is only a reference, the kernel holds
the lock); a shared tmp of two copies would replace the project with the half-written contents of
the other. On Windows the lock was a stub "always free", and two copies wrote one project.

#### Scenario: The lock is named and survives an atomic save
- **WHEN** the project is busy, then atomically overwritten
- **THEN** the second handle gets `Busy` with the pid, a neighbouring project is taken freely, after the overwrite the lock is held, after drop it is released
- **TEST** t362 (`src/apps/kt-app/src/project_lock.rs`), t362b (`src/apps/kt-app/src/project_lock.rs`)

#### Scenario: The second copy has its own tmp and recovery
- **WHEN** the temporary name and the recovery target are computed while the project is busy
- **THEN** the tmp contains the pid and the full project name; the recovery of the second copy differs from the owner's and remains `.otio`
- **TEST** t363 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The first Ctrl+S on an occupied project asks once
- **WHEN** the project is busy with a neighbour and the user saves
- **THEN** a notice with the pid, a question; "don't save" leaves the file and the asterisk; "save anyway" writes and no longer asks again
- **TEST** t364 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The lock key is normalized, the pid is read while the lock is busy
- **WHEN** the lock key is computed for `C:\Proj\a.kt`, `c:\proj\A.kt`, `\\?\C:\Proj\a.kt` and `C:/Proj/a.kt` by the Windows rule (a pure function, checked on any OS); on Windows the project is held by one handle and taken by a second
- **THEN** all four keys are equal, and by the Linux rule `/p/a.kt` and `/p/A.kt` differ; the second handle gets `Busy` with the holder's pid, read from the pid file
- **TEST** t852 (`src/apps/kt-app/src/project_lock.rs`)

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

#### Scenario: The first panic keeps the report
- **WHEN** a process panics on one thread, then on another, then in a destructor during the unwind
- **THEN** `kt-app-crash.txt` begins with the first panic's report and holds two "a later panic" sections with their thread and message (before: each panic rewrote the file and the live `render` report showed only "panic in a destructor during cleanup", the EGL error that started it gone)
- **TEST** t2089 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: A crash the window showed is not announced again
- **WHEN** a report marked `shown:` and then an unmarked one are taken at start
- **THEN** both are taken (moved to `.seen`, handed to telemetry); the bell announces only the unmarked one
- **PATH** Kinetren crashes → the crash window → Restore the project → the editor opens without a second "the previous session ended abnormally" line in the bell
- **TEST** t1078 (`src/apps/kt-app/src/journal.rs`)

### Requirement: An event number is not reused; the envelope of a born event is inherited
An event number SHALL be issued from ONE place, by a session mark that does not drop after deletion and undo; copies of the formula "max + 1" outside that home MUST NOT exist. A derived event (a half of a cut, a chunk of a bake, a paste) SHALL receive the parent's envelope whole — the kinship is reported by the model as a batch of edits — and references to material (`available_range`) live on the MEDIA as the fallback path for born events and MUST NOT overwrite the clip's own envelope. Markers are inherited whole; the WRITER distributes them across the material window.
Why: a reused number hung someone else's envelope (name, markers, references of a deleted one) on a newborn, and numbers are what link groups and morphers go by; the two halves of one cut went into a foreign editor in DIFFERENT coordinates (from an hour and from zero); markers laid out at a cut were lost under undo.

#### Scenario: Numbers are not reused after deletion and undo
- **WHEN** two clips are created and deleted, and a third is created and undone
- **THEN** each next number is strictly greater; a repeated request without taking one returns the same number
- **TEST** t376 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Number arithmetic in one home
- **WHEN** the UI files of the application are scanned
- **THEN** the formula "max over clips + 1" occurs only in the home of the rule
- **TEST** t379 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A newborn clip is written in the neighbour's material coordinates
- **WHEN** a clip with an hour of embedded timecode is cut in half and saved
- **THEN** both halves are placed from the hour, and not the right half from zero
- **TEST** t377 (`src/libs/kt-otio/tests/edit_roundtrip.rs`)

#### Scenario: The media envelope does not overwrite the clip's envelope
- **WHEN** a clip has several references to material and its own `active_key`
- **THEN** after the round trip both references and the active key are preserved
- **TEST** t378 (`src/libs/kt-otio/tests/edit_roundtrip.rs`)

#### Scenario: Name, foreign effects and metadata are inherited
- **WHEN** a clip with a foreign effect and a foreign metadata namespace is cut
- **THEN** both halves carry the name, the effect and the metadata
- **TEST** t380 (`src/libs/kt-otio/tests/edit_roundtrip.rs`)

#### Scenario: The writer distributes the markers
- **WHEN** there are three markers (before, on the boundary, after the cut), and the child's envelope carries all three
- **THEN** in the file each half has only the markers of its own window; the boundary marker goes to the right half
- **TEST** t381 (`src/libs/kt-otio/tests/edit_roundtrip.rs`)

#### Scenario: Inheritance arrives on a live gesture
- **WHEN** a cut and a paste of a copy are done through the application
- **THEN** the newborns get the parent's name in `Retained`
- **TEST** t382 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Workspace ≠ project
The width of the meters panel, zoom, scroll and cursor SHALL be stored in the user store (`workspace.json`: the width is shared, the view is per project), while in `.otio` they remain passthrough: what was read is what gets written. An empty store MUST yield to the values from the file; a filled store takes precedence over the file; a store of a future version is read but not overwritten. Track names and heights, groups and the bin layout SHALL remain in the project. Convenience edits (region, loop, zoom, bin layout, track name) MUST NOT give an asterisk; content edits do.
Why: you open a colleague's project and lose your own width; you save it and impose yours on them; passthrough keeps byte stability and old builds working; the boundary "convenience/content" moves along with the features and is held only by a test.

#### Scenario: Foreign values are not overwritten by ours
- **WHEN** the foreign meter width and view were read from the file, and we have our own
- **THEN** in the saved file the foreign values are untouched; ours are in the store
- **TEST** t390 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Migration without loss, and a future version
- **WHEN** the store is empty / filled / broken / of version 99
- **THEN** silent / the round trip is exact and another project's view is not substituted / does not crash / is read and not overwritten; track names, heights and the bin layout travel in the project
- **TEST** t391 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The dirtiness boundary
- **WHEN** the region, loop, zoom, bin layout or a track name change, and then a marker is set
- **THEN** there is no asterisk before the marker and there is one after it; Esc clears the region without touching the loop
- **TEST** t96 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Long background work is finite and cancellable
A finite background task (render, analysis, index) SHALL have progress, cancellation and pause, and its handle MUST stop the thread on `Drop`; a service (a queue that lives with the project) is a separate type. Cancellation MUST clear everything: "running" means running.
Why: a task that pretended to be a service raised a thread on every call and appended to the orphaned `Arc`s of a closed project; within one day of writing, the flag and the handle were not separated, and the cost grew. The services themselves are the `media-index` capability.

#### Scenario: Task scaffolding
- **WHEN** a task reports progress, is paused, cancelled, and dropped
- **THEN** progress arrives, the pause holds, cancellation releases the pause, and `Drop` stops the thread
- **TEST** t398 (`src/apps/kt-app/src/bg.rs`), t398b (`src/apps/kt-app/src/bg.rs`)

#### Scenario: Cancellation clears everything
- **WHEN** there are several requests in the analysis queue and a cancellation arrives
- **THEN** the queue is empty, and the "running" flag matches the real thread
- **TEST** t410 (`src/apps/kt-app/src/analysis.rs`)

### Requirement: Work ends with `finish.sh`, and the proof is a live scenario
Any work on the program SHALL end with `src/scripts/finish.sh` and the version `vYY.MM.DD-hash` in the report; the script's output MUST NOT be filtered, and the tree is not edited while it runs. The verdict of each verification stage MUST be taken from the command that does the checking (the cargo return code), not from the one that prints. For each point of the owner's complaint there SHALL be a live scenario on a virtual screen (the real binary, real gestures), and the test asserts the observable consequence from the point where a person acts. A failing test that depends on a pause or on the order of a background thread, and a `SIGBUS` without a single line of test on the first run after a rebuild, MUST be rerun separately before they are considered a regression.

**The script is the ONLY point of verification, and it names its own cost.** The workshop test run SHALL be a stage of this same script, not a separate command of the agent: otherwise the budget and the verdict split into two unrelated halves, and the kt-gpu stage duplicates what the workshop has already run, and therefore MUST be removed (the SIMD stages remain — they have their own environment). Each stage SHALL print its own time, and the script — a TOTAL; a total longer than **five minutes** (the budget set by the owner) SHALL be marked in the report together with the name of the stage that consumed it. The budget and the timing SHALL be held by a guard, not by the agent's memory.

**Live scenarios are run by several workers.** The run SHALL hand out scenarios to workers, each of which has its OWN virtual screen and its OWN user store; the number of workers SHALL be limited so that the load does not exceed the number of cores of the machine (one kt-app in software rendering takes about seven). A scenario that failed in the parallel run SHALL be automatically rerun ALONE; only a scenario that fails again counts as red, and the report SHALL name the scenarios that passed only on the second attempt. A run without parallelism SHALL remain available through one key.
Why: `cargo test` builds a separate binary and does not rebuild `target/debug/kt-app` — the owner twice ran the monthly build and counted the defects as alive ("you did not verify" ×3); `if cargo test | tail` was green on any failure; eleven "green" items of the round turned out to be broken in the window, because the harness checked the wiring rather than the consequence; `SIGBUS` is a swap of the binary under a mapped image on a separate partition, not a defect. R.68: a verification cycle cost 21 minutes (live scenarios 1012 s, workshop 259 s, build ~100 s), and a check that costs this much itself creates the temptation to skip it — that is, it works against the rule for which it was set up. The ban on "two threads" was lifted by the owner by direct decision ("Yes, with load limiting and automatic rerun of the red") together with the mechanism that removes its cause: a red under load of ~14 now differs from a defect by a repeated solo run.

#### Scenario: Stage verdict from cargo
- **WHEN** the test stage of `finish.sh` fails
- **THEN** "WARNING: … red" is printed, along with the names of the failed tests and the path to the full log; replacing `cargo` with a failing one makes the stage red
- **TEST** `src/scripts/finish.sh` (workshop stage, log `target/finish-*-fail.log`)

#### Scenario: Live data-safety scenarios
- **WHEN** `src/scripts/live-check.sh` starts Xvfb with a separate `XDG_DATA_HOME` and `KT_AUTOSAVE_SECS`
- **THEN** `recover.py` (SIGKILL → offer to recover, twice) and `unsaved.py` (the close button asks, "Stay" holds, "Do not save" does not touch the file) are green; screenshots and `.otio` are in `target/live-out`
- **TEST** live `recover.py`, `unsaved.py` (`src/scripts/live/recover.py`, `src/scripts/live/unsaved.py`)

#### Scenario: The cost of each stage and the five-minute budget
- **WHEN** `finish.sh` finishes a run; separately — a run in which a stage artificially lasts longer than the budget
- **THEN** a table "stage — seconds" is printed with ALL the stages the script executed, and the total; with a total above 300 s the report carries a mark with the name of the most expensive stage; not one executed stage is left without a time line
- **TEST** t800 (`src/apps/kt-app/src/reach_tests.rs`, calls the timing guard `src/scripts/finish-budget.py --selftest` and checks the script's list of stages against the list of measurements)

#### Scenario: A red in parallel is rerun alone
- **WHEN** in a parallel run a scenario fails the first time and passes the second; separately — it fails both times
- **THEN** in the first case the run is green, and the report says the scenario passed on the second attempt alone; in the second case it is red, with the scenario's name and the path to the screenshots; a solo run under one key reruns nothing
- **TEST** t801 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-check.sh` on a substitute scenario directory; without Xvfb the test is skipped with the named reason)

#### Scenario: Workers do not see each other
- **WHEN** the run goes with several workers
- **THEN** each scenario has its own screen number and its own user-store directory; no scenario received a neighbour's store; and the number of simultaneously live kt-app instances did not exceed the declared worker limit
- **TEST** t802 (`src/apps/kt-app/src/reach_tests.rs`, a substitute scenario prints `DISPLAY`/`XDG_DATA_HOME`; without Xvfb it skips with the named reason)

### Requirement: Per-user data lives in `kinetren`; the former directory is moved once
The application's per-user data directory SHALL be `$XDG_DATA_HOME/kinetren` (falling
back to `~/.local/share/kinetren`) on Linux and `%APPDATA%\kinetren\data` on Windows (the
Roaming known folder, the same directory eframe uses for its own storage). It covers
recovery files, the journal and crash report, project locks, presets, the workspace and UI
state. Large regenerable downloads (the FFmpeg build) SHALL go to the Local known folder on
Windows (`%LOCALAPPDATA%\kinetren\`), not to Roaming. The resolver MUST NOT fall back to the
working directory: if no per-user directory can be resolved, the program SHALL say so once
and use the system temporary directory. On Linux, if the directory does not exist and the
former `kinotrain` directory does, the first start SHALL move the former one into place
before anything is read or written. This happens once, and nothing is lost or duplicated.
If the move fails (for example, across filesystems), the content SHALL be copied and the
former directory left in place. If both directories exist, the new one wins and the former
one is not touched. The move MUST NOT run on Windows, where no former install exists.
`kt-cli` SHALL resolve the directory with the same code as the app and look for project
locks there.

Why: the full rename must not lose the user's presets, workspace, recovery copies or
locks. On Windows `HOME` is usually unset, and the former fallback `"."` put the data
directory into the working directory. Started from Explorer, that can be `System32`, where
every write fails silently. Started from a source tree, the migration would have renamed
any `kinotrain` folder found there.

#### Scenario: First start moves the former directory; later starts do not touch it
- **WHEN** the data directory is resolved with only `kinotrain/` present (holding a preset file and a lock); then again; then with both present
- **THEN** the first call leaves `kinetren/` with the same files and no `kinotrain/`; the second changes nothing; with both present the new one is used and the former is untouched
- **TEST** t830 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: kt-cli and the app agree on the lock directory
- **WHEN** the app takes the lock of a project, and `kt-cli` checks the same project
- **THEN** `kt-cli` reports the holder's process, i.e. both use `kinetren/locks`
- **TEST** t831 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: Directory per OS, never the working directory
- **WHEN** the resolver runs with injected environments: Linux with `XDG_DATA_HOME`; Linux with only `HOME`; Linux with neither; Windows with Roaming and Local known folders and no `HOME`; Windows with known folders unavailable; each time with a `kinotrain/` folder in the working directory
- **THEN** the results are `$XDG_DATA_HOME/kinetren`, `~/.local/share/kinetren`, the temp directory (with a warning), `%APPDATA%\kinetren\data` with downloads under `%LOCALAPPDATA%\kinetren`, and the temp directory (with a warning); the `kinotrain/` folder in the working directory is untouched in every case
- **TEST** t851 (`src/libs/kt-dirs/tests/dirs.rs`)

### Requirement: Atomic replace on Windows survives a briefly held target
On Windows the rename step of `kt_otio::write_atomic` SHALL replace the project with
`MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` and SHALL retry on a
sharing violation or access denied within a budget of about one second, with growing
pauses. Write-through SHALL count as the directory sync (`dir_synced == true`), so the
"not synced" notice does not appear on every Windows save. When the budget runs out, the
write MUST fail with the reason, and the temporary file is removed. The same replace is used
for the journal rotation and the user store.
Why: on Windows, replacing a file fails while another process holds it open without
delete sharing: an antivirus scanning the just-written file, a sync client, an editor.
Without a retry, Ctrl+S, autosave and the exit save fail at random. A directory cannot be
opened for `fsync` there; write-through is the platform's way to make the rename durable.

#### Scenario: Retry within the budget, then an honest failure
- **WHEN** the replace runs through an injected file system that answers "sharing violation" twice and then succeeds; then one that answers it for longer than the budget
- **THEN** the first write succeeds with `dir_synced == true` after exactly three attempts; the second returns an error that names the sharing violation, the project is unchanged and no temporary file remains
- **TEST** t853 (`src/libs/kt-otio/tests/io.rs`)

### Requirement: The owner's claims are indexed by the registry; the guard keeps it complete and honest
The owner's claims file SHALL be stored verbatim (`docs/ktprompts/<version>.txt`) and have a
registry `docs/ktprompts/<version>-index.md`, where EACH numbered item of the file has
a row with a state from the closed set: `done` (with the round and the test number),
`partial` (what exists, what is missing), `no` (a defect — with a capability; a feature — with a note in `docs/features`
or the mark "no note"), `question` (with the text of the question to the owner), `future`. A row
SHALL have a stage (a round number or "later"). The guard SHALL fail if a source item
has no row, a state is outside the set, a `done` has no test `tNNN` existing in the
sources, or a `question` has no text; the guard SHALL be part of `finish.sh`. The owner's answer to a
question SHALL be recorded verbatim in `docs/user-feedback.md`, and move the row from
`question` to a decision. An item that the OTIO exchange file cannot express SHALL go into
the registry section "what export loses".
Why: the owner asked "to make notes for agents and tests, so as not to return to the
same kinds of problems"; without a registry the next round would take from the file what is already done
(generator automation, freeze frame, moving away from OTIO) or would do the opposite of what was
decided (the format name, the import door). A rule in prose has already failed here twice
("do not make obvious labels" — checklist item 17 of R.7, and "Add morpher here").

#### Scenario: The registry is complete and honest
- **WHEN** the registry `v26.10` is checked; a row of one item is temporarily deleted; separately — a `done` row is given a non-existent test; separately — a `question` row is left without text
- **THEN** on the whole registry the guard is green and prints the count by state; each of the three corruptions makes it red with the item's address
- **TEST** t823 (`src/apps/kt-app/src/reach_tests.rs`), `src/scripts/ktprompts-index.py`

#### Scenario: A closed item refers to a live test
- **WHEN** a registry row is in the state `done R.NN, tNNN`
- **THEN** the function `fn tNNN_` exists in the sources of `src/`, otherwise the guard is red — as with the registry of the migration of live scenarios
- **TEST** t823 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Every claims file has its registry
- **WHEN** the guard runs over `docs/ktprompts/` holding `v26.10.txt`, `v26.10.18.txt` and `kkt-26.10.27.txt`; separately — a fourth source is added without a `-index.md`
- **THEN** each of the three sources is checked against its own registry and the counts are printed per source; the source without a registry makes the guard red with its file name
- **TEST** t823 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Between-projects records — recent projects, posters, templates, tour progress
The user store SHALL keep, outside every project and in the per-user data directory: the recent
projects list (path, name, last-opened time, the template it was made from), bounded to 12 entries,
newest first, with "Clear the list" in the welcome window; one poster image per recent entry, stored
under the data directory's own posters folder and keyed by the project's path, NEVER written next to
the project or into its sidecar index; the person's project templates (name, frame width and height,
frame rate), saved from Project settings with "Save as template…"; the tour progress (finished steps
per lesson); and the start choice ("Show the welcome window at start"). The poster SHALL be rendered
by the offline path — its own short-lived sources, a finite cancellable job, never during playback —
when a named project is saved or closed, from the frame at the playhead (or at a tenth of the
project's length when the playhead is at zero); the welcome window SHALL only read finished poster
files and show a placeholder until one exists. The former single `last_project` key SHALL seed the
recent list once. Each record SHALL follow the store's existing rules: a broken file is moved aside to
`.broken` and the record starts empty; a file of a future version is read but not overwritten; under
tests the store lives in memory or in the directory the test names.
Why: the welcome window (capability `ui-reach`) needs a list that F-HLP-027 found missing ("exactly
one path is stored, and it is invisible"), with a limit and a "clear" (UI rule 20: nothing grows by
itself); a poster in the project's folder or sidecar would make the program write files the person
did not ask for into his project tree, and the reel-preview disk tier lives in the sidecar precisely
because it is the project's content cache, not the program's (R.58). Templates were the fifth
between-projects record F-HLP-027 deferred to the shared store (first-wave conclusion 2).

#### Scenario: The recent list is bounded, keeps a missing file and never touches the project tree
- **WHEN** in a temporary data directory 14 projects are saved and closed in turn (each poster job
  run to completion), the 3rd project's file is then deleted, the store is reloaded, the list is
  cleared; separately the store starts with only a `last_project` value in the eframe storage;
  separately a broken `recent.json` and one of version 99 are read
- **THEN** the list holds the 12 newest, newest first; each has a poster file under the data
  directory's posters folder and no new file appears in any project folder or sidecar; the deleted
  project stays listed with its path and is reported missing; "Clear" empties the list and removes
  the posters it owned; the `last_project` start yields a list of one; the broken file is moved to
  `.broken` and the list starts empty; the version-99 file is read and left unwritten
- **TEST** t985 (`src/apps/kt-app/src/user_store.rs`)

#### Scenario: Templates, tour progress and the start choice survive a restart
- **WHEN** a user template "Shorts 60" (1080×1920, 60 fps) is saved from Project settings, two steps
  of lesson 1 are finished, "Show at start" is unchecked; the store is reloaded; then "Start over" is
  used on the tour
- **THEN** after the reload the template is listed after the built-in ones with its size and rate, the
  progress shows two finished steps, the start choice is off; "Start over" clears the progress and
  nothing else
- **TEST** t987 (`src/apps/kt-app/src/user_store.rs`)

### Requirement: Each live guard runs in its own output folder
The guards of the live framework that run `scripts/live-check.sh` (t801, t802) SHALL each pass
their own `KT_LIVE_OUT` (`target/live-guard-<test>`), never a shared one. Why: `live-check.sh`
deletes `verdicts/` of its output folder at its start, and the tests run in parallel. On CI of
94e36e9e t802 started while t801 was re-running its red scenario and wiped the verdict:
"the re-run scenario stayed red", with an empty verdict.

#### Scenario: Two guards run at once and keep their verdicts
- **WHEN** t801 (a re-run of a red scenario) and t802 (three isolated workers) run in parallel
- **THEN** t801 reads its re-run verdict `ok` from `target/live-guard-t801/verdicts`, and t802
  reads its workers' `env.json` from `target/live-guard-t802`
- **TEST** t801, t802 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A canonical path is shown as a person reads it
A path made canonical for display or as a key next to `std::path::absolute` spellings SHALL go
through `kt_dirs::canonical`, never the bare `std::fs::canonicalize`: on Windows the verbatim
prefix is dropped where the plain spelling names the same file (`\\?\C:\x` → `C:\x`,
`\\?\UNC\h\s\x` → `\\h\s\x`) and kept where it would not (past `MAX_PATH`, not a drive or UNC
path). The rule is `kt_dirs::plain_path`, pure over `Os`. `user_store::project_key` — the key of
a Recent row and its poster, and the path its welcome card shows — uses it. Why: on windows-latest
t986 failed "each card names its folder": the card named `\\?\C:\…`, a folder nobody typed.

#### Scenario: The verbatim prefix is dropped where it means nothing
- **WHEN** `plain_path` reads a verbatim drive path, a verbatim UNC path, a plain path, a
  `\\?\Volume{…}` path, a verbatim path past `MAX_PATH`, and a Linux path spelled the same; and
  `canonical` resolves an existing folder
- **THEN** the first two lose the prefix, the rest come back unchanged, and `canonical` gives an
  absolute path without the prefix that canonicalises to the same file
- **TEST** t1062 (`src/libs/kt-dirs/tests/dirs.rs`)

#### Scenario: A recent card names the folder without the prefix
- **WHEN** the welcome window shows two recent projects of one folder
- **THEN** both cards name the folder in its canonical spelling, and no label carries `\\?\`
- **TEST** t986 (`src/apps/kt-app/src/reach_tests.rs`)

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

### Requirement: A test never touches the person's data directory
A test process SHALL NOT read or write the person's data directory (the journal, the crash report,
recovery, project locks, the workspace, presets, recent projects and posters, the render log,
eframe's storage, FFmpeg downloads). In a test build the data directory SHALL be a folder of the
test process in the system temp directory — or the absolute `KINETREN_DATA_DIR` the test that
started the process gave it — and every path of the program's data SHALL be derived from that one
directory. No code outside the one resolver SHALL ask for the person's directories directly.
Why: until 2026-11-01 the tests wrote into the owner's real `~/.local/share/kinetren`: his journal
was full of test lines (`RENDER render log not written (/tmp/kt-t959-…)`) and rotated his own records
away at 14:09, next to 7934 lock files, a recovery file and a workspace the tests had written.

#### Scenario: A test's data directory is its own
- **WHEN** a kt-app test resolves the data directory and the paths derived from it, and writes a line
  into the journal
- **THEN** the directory is not, and does not contain or lie inside, the directory the resolver gives
  for the person; it lies in the temp directory; the journal, crash report, recovery, lock, FFmpeg
  download root, render log and user store paths lie inside it; the line is in the test's journal and
  not in the person's
- **TEST** t1310 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: Only the resolver asks for the person's directories
- **WHEN** the kt-app sources are scanned for direct requests of the person's directories (the
  `kt-dirs` data, local and download roots, eframe's storage directory, `HOME`, `APPDATA`,
  `XDG_DATA_HOME`, a home-directory lookup)
- **THEN** the only uses are the resolver itself, the test comparing it with eframe's own choice, and
  the FFmpeg download root outside a test build
- **TEST** t1311 (`src/apps/kt-app/src/journal.rs`)

### Requirement: A native crash leaves a report that names its thread and its stack
The editor and kt-cli SHALL put a handler on SIGSEGV, SIGBUS, SIGILL, SIGFPE and SIGABRT that, at
the signal and using only async-signal-safe means (system calls, fixed static buffers, the
unwinder primed at start; no allocation and no lock of the program), writes a raw report: the
build, the signal with its code and fault address, the process id, the faulting thread's name and
id, its program counter, its stack (the unwinder's frames; when the unwinder stops within three
frames, the code addresses found on the stack), the name of every thread, and the executable
mappings. The handler SHALL then restore the default action and re-raise, so the process dies by its
own signal; an alarm SHALL bound it. A healthy process SHALL symbolize the raw report from the
binaries' line tables (a frame with none keeps `module+offset`): the editor's supervisor right after
the child's death, into `kt-app-crash.txt` with a `summary:` line (signal, thread, the top frame and
the first frame of our code), the stack, the threads and the crashed run's last telemetry-journal
lines (its RUN line found by the child's pid); the crash window SHALL show the summary and those
journal lines; with no supervisor, the next start SHALL do the same before it takes the report;
kt-cli SHALL start its own `crash-report` and print the report on stderr. The raw report SHALL be
taken once (it moves to `.seen`), and the journal's "the previous run crashed" record SHALL carry
the summary. A stack overflow MAY end without the report (no alternate signal stack, see design).
Why: the owner, kkt-26.11.08 H5: "краш скудный" — his report of a SIGSEGV named the signal and a log
of one header line; H4: "debug сборка как-то обильно падает на SIGSEGV", and no report said where.

#### Scenario: A fault in a named thread is named with its function, file and line
- **WHEN** a child process with the handler installed writes through a null pointer in a thread named `kt-crash-probe`, beside three named threads
- **THEN** it dies by SIGSEGV; the symbolized report says SIGSEGV, code 1 (address not mapped), address 0x0, thread `kt-crash-probe`; its frame 0 is `kt_crash::testing::kt_crash_probe_segv (libs/kt-crash/src/lib.rs:…)`, the caller is further down, every thread is listed with the faulted one marked; the summary begins `SIGSEGV in thread "kt-crash-probe" at kt_crash::testing::kt_crash_probe_segv`
- **TEST** t2080 (`src/libs/kt-crash/tests/handler.rs`)

#### Scenario: An abort names the code that aborted
- **WHEN** a child process aborts in a named thread
- **THEN** it dies by SIGABRT; the report says SIGABRT, sent by the process itself, names the thread and the aborting function through the C library's frames
- **TEST** t2081 (`src/libs/kt-crash/tests/handler.rs`)

#### Scenario: The raw format parses and stays exact without line tables
- **WHEN** a raw report whose mapping is a missing file is symbolized
- **THEN** its fields are read, each frame keeps `module+offset` from the mapping's file offset, and a legacy hash suffix is dropped from a name
- **TEST** t2083 (`src/libs/kt-crash/src/symbols.rs`)

#### Scenario: kt-cli prints its own crash report
- **WHEN** `kt-cli crash-probe` faults on a thread named `kt-cli-probe`
- **THEN** kt-cli dies by SIGSEGV, its stderr carries the symbolized report (signal, thread, the probe function with file and line, the faulted thread marked) and a last line naming the report file in the temporary folder, whose first line is `kt-cli <full version>`
- **TEST** t2082 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: The editor's report carries the summary, the stack and the run's journal
- **WHEN** the editor's child installs the telemetry journal and the handler, records an action and faults in a thread named `t2084-fault`, and the supervisor gathers the crash with the child's pid
- **THEN** the report holds the exit line, `summary: SIGSEGV in thread "t2084-fault" at kt_crash::testing::kt_crash_probe_segv…`, the stack with file and line, the faulted thread marked and the run's journal lines with the action; the window's lines start at this pid's RUN line; the raw report moved to `.seen`
- **PATH** Kinetren dies by a fault → the crash window opens → under "The program was ended by the signal SIGSEGV (segmentation fault)." a monospace line names the thread and the function, and "The last lines of the log:" lists what the program and the person did last (the telemetry journal) → "Open the log folder" → `kt-app-crash.txt` holds the stack
- **TEST** t2084 (`src/apps/kt-app/src/supervisor.rs`)

#### Scenario: Without a supervisor the next start takes the raw report
- **WHEN** a child faults with no supervisor gathering it and the next start absorbs the raw report
- **THEN** `kt-app-crash.txt` exists with the summary and the run's journal lines, the raw report is gone, and the report is taken unmarked (the bell announces it)
- **TEST** t2085 (`src/apps/kt-app/src/supervisor.rs`)

### Requirement: The person's media and save gestures make no blocking call on the UI thread
Ctrl+S SHALL write the project — its text, the write and both fsyncs — on the scheduler's save lane;
the UI SHALL take the outcome within a few milliseconds or in a later frame, and the unsaved mark
SHALL clear only if no edit came while the file was written; a save in flight is finished before
the program exits. A file brought onto the timeline, a card placed from the bin, Undo and Redo of a
media's arrival and a file that came back SHALL open their file, swap or release a decoder on the
media lane, and the gesture lands when it answers. Autosave writes on a background lane that waits
out playback. On these gestures the UI thread SHALL make no blocking call.
Why: `found-defects.md` row 98 (Ctrl+S stood 6.7 s on a busy disk) and row 148 (the UI thread opened
files and joined a decoder on media gestures); the owner: "No interface action waits".

#### Scenario: Media and save gestures block nothing
- **WHEN** a real file is brought onto the timeline, placed again from the bin, its arrival undone twice and redone twice, and the project saved with Ctrl+S
- **THEN** every gesture lands (the clips, the media, the file on disk, no unsaved mark) and the UI thread counted no blocking call; one inline open on the UI thread turns it red
- **TEST** t2236 (`src/apps/kt-app/src/ui_wait_tests.rs`)

### Requirement: An action that waits for a save goes on when the write lands
Every path that saves and then leaves the project — "Save and continue" of the unsaved-work question
before New, Open, Import, a dropped project, a start tile or a recent card, and quitting; "Next lesson"
on an edited lesson copy — SHALL run its action when the save's write has LANDED, never right after
posting it, and never by waiting on the UI thread. A gesture that leaves the project while a Ctrl+S of
the same revision is still writing SHALL wait for that write instead of asking about work being saved.
A save that fails, is skipped for a newer file on disk, is cancelled (Save As) or is declined (the busy
project) SHALL drop the waiting action: the project stays with its unsaved mark. An edit made while
the file was written is asked about again before the action goes on.
Why: one-scheduler moved the write onto the save lane, so `save()` returns with the unsaved flags still
up. The live `lessons` walk met "Unsaved work" over the program's own lesson copy after "Next lesson"
(red twice), and "Save and continue" silently stayed on the project; the harness took the answer inside
`save()` and was green.

#### Scenario: Next lesson opens when the copy is written
- **WHEN** lesson 1's copy carries an edit and the save's answer comes in a later frame, and the person presses Next lesson
- **THEN** no frame shows the unsaved-work question, lesson 2 opens once the write lands, and lesson 1's copy on disk holds the edit; the code that opened the next lesson right after posting the save turns it red
- **PATH** finish lesson 1 → press Next lesson → no "Unsaved work" window → lesson 2 opens once the copy is written
- **TEST** t2305 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Save and continue goes on after the write
- **WHEN** an edited project answers "Save and continue" before opening a recent project, the answer of the save coming in a later frame
- **THEN** the project stays while the write is in flight, then the chosen project opens with no second question, and the first project's file holds the edit
- **PATH** an edited project → a Recent card → "Unsaved work" → "Save and continue" → the chosen project opens once the write lands
- **TEST** t2306 (`src/apps/kt-app/src/ui_wait_tests.rs`)

#### Scenario: A gesture after Ctrl+S waits for its write
- **WHEN** Ctrl+S is still writing and the person chooses File › New
- **THEN** no unsaved-work question is asked, the empty project opens when the write lands, and the file holds the edit
- **PATH** an edited project → Ctrl+S → at once File › New → no "Unsaved work" window → the empty project once the write lands
- **TEST** t2307 (`src/apps/kt-app/src/ui_wait_tests.rs`)

#### Scenario: A failed write drops what waited for it
- **WHEN** the project's folder cannot be written and File › New follows "Save and continue", or follows a Ctrl+S in flight
- **THEN** the write's failure is warned, nothing opens, the work and its unsaved mark stay, and nothing is left waiting
- **PATH** an edited project in an unwritable folder → File › New → "Save and continue" → a warning, the project stays with its asterisk
- **TEST** t2308 (`src/apps/kt-app/src/ui_wait_tests.rs`)
### Requirement: A live try starts clean and a red try keeps its evidence
Before each try of a scenario the live bench SHALL empty the scenario's folder, so nothing an earlier
run left reaches this one, and the stand SHALL drop its dump's companions (`.crash.json`, `.load.json`)
when it starts. The lone rerun of a red scenario SHALL keep the red try whole in
`<scenario>/first-try/`, and every try SHALL keep the lines it printed in its folder (`said.txt`); the
report names where the first try is. The program's geometry dump and the bench's command file
SHALL live on tmpfs, emptied when a program starts, and be copied beside the screenshots when the
scenario ends; a red scenario of the stand gets the stand's journal and trace beside it.
Why: `renderregion` went red twice on a `geom.json.crash.json` a build of the day before had left
(found-defects 165), and the rerun overwrote the red try's screenshots, dumps and log, so a red that went
green alone left nothing to read (found-defects 163). On the working disk a write of the dump stood
10–20 s while builds synced it, and `space_hold` read a head standing still for 17 s of a playback
that ran.

#### Scenario: A stale file does not reach the run, and the retry keeps the first try
- **WHEN** a run of `_flaky` (red the first time) and `_red` (red always) starts on folders holding a
  file of an earlier run
- **THEN** the earlier file is gone before the first try; `_flaky` passes on the retry; both folders hold
  the first try in `first-try/` (its mark, its lines, its screenshot) beside the retry's own, and the
  report names `first-try`
- **TEST** t2210 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-check.sh` on stand-in
  scenarios; without Xvfb the test is skipped with the named reason)
