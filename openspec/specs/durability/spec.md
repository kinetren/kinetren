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
- **THEN** the recovery file is orders of magnitude smaller than the same project in the exchange format, and the write fits in the interval without interface lag
- **TEST** t815 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Live SIGKILL
- **WHEN** an edit by a real gesture → background write → `SIGKILL` → restart
- **THEN** the project file is untouched, the program itself offers recovery, and the work is in place — and this twice over
- **TEST** live `recover.py` (`src/scripts/live/recover.py`, via `src/scripts/live-check.sh`)

### Requirement: Recovery is offered by time and placed into the editor
At startup and by the menu item "Recover unsaved…" the program SHALL collect all candidates (recoveries next to the project, including the files of second copies, and unnamed work) and offer recovery of the project only if it is STRICTLY newer than the project; unnamed work is always offered, the freshest first.
The dialog shows the path and the time; "Recover" puts the work INTO THE EDITOR with an asterisk and MUST NOT consume the source; "Delete" removes only this file; "Not now" touches nothing. An empty list on a direct request from the menu SHALL say so in words.
Why: the chain "argument → last_project → autosave" chose by order, not by time — "opened A, New, an hour of work, a crash" was lost silently; a comparison without strictness would call for recovering what is already in the file after every Ctrl+S.

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

### Requirement: Unsaved work is not silently discarded; Drop is the last line
"New", "Open", and closing the window with unsaved work SHALL ask with one guard that has THREE answers (save / do not save / stay); "Save and continue" MUST NOT continue if the save did not succeed. Closing the window is intercepted in the standard way (`CancelClose`). The application's `Drop` SHALL write the recovery in cases where `on_exit` is not called (re-creation of the application in place, unwinding of the stack from a panic), and its body MUST NOT contain a single `unwrap`; a poisoned guard is no reason not to write. A clean project is not hindered by the guard.
Why: the right not to ask was legitimate while autosave wrote over the user's file; the owner removed that — together with the right to stay silent. A `bool` instead of three answers lost the edit on the "Cancel" button; a panic inside `Drop` during unwinding is an `abort`, which takes away exactly the file the whole thing was for.

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
The program SHALL keep a journal `kt-app.log` in the data directory: append, do not overwrite, rotate at 1 MiB in one step, and a failure to write the journal MUST NOT bring down the program. The panic hook SHALL leave a report with the version, thread, message, backtrace with names, and a dump of the "black box", without touching the model; the next start reports the crash exactly once. The "black box" MUST be readable even under a poisoned mutex.
Why: the window's `on_exit` no longer exists, and a `.desktop` with `Terminal=false` sends stderr nowhere — that is exactly where the final record lives; the hook is called in the panicking thread (invariants 2 and 6 forbid going into the engine), and a second `unwrap` inside the hook is an `abort` with no report.

#### Scenario: The journal appends and rotates
- **WHEN** two records are written, then the file goes past the threshold and one more is written
- **THEN** both lines are in place with dates; after the threshold the old content goes to `.log.1` and the new starts from zero; an unavailable path is silently skipped
- **TEST** t354 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: The crash report is complete and is given once
- **WHEN** the hook forms the report and the program starts after the crash
- **THEN** the report holds the version, thread, message, function names, and the dump of the box; the second start stays silent about the crash, and the report lies in `.seen`
- **TEST** t375 (`src/apps/kt-app/src/journal.rs`), t374 (`src/apps/kt-app/src/journal.rs`), t373 (`src/libs/kt-engine/src/flight.rs`)

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
- **TEST** `src/scripts/finish.sh` (workshop/SIMD stages, log `target/finish-*-fail.log`)

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
