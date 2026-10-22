# Spec Delta

## MODIFIED Requirements

### Requirement: Autosave writes to a recovery file, only Ctrl+S changes the project
Background writing SHALL go to `<full project name>.recovery.kt` next to the
project (an unnamed project — to the data directory) and MUST NOT touch the user's file or clear the asterisk. The recovery file SHALL be of the same
format as the project: writing megabytes of the interchange format every half minute is
both interface lag and a window of loss on a crash. A successful save
SHALL delete only ITS OWN recovery; exiting the program SHALL write
the recovery, not the project; a clean exit takes its own recovery with it. Drift of the
viewport (zoom, scroll, cursor) MUST NOT trigger writing. There are TWO revision
guards — one for the project, one for the recovery. The gate "not during a gesture and not during playback" SHALL yield after two quiet intervals.
Why: the owner's decision — there must be a SECOND state on disk, otherwise there is nowhere to roll back to; a shared guard raised the bar with background writing, and the next
Ctrl+S turned out to be "stale"; substituting the extension merged `film.otio` and
`film.mp4.otio` into one recovery; zoom every 30 s ran a full serialization without a single edit. R.70: for the owner, autosave wrote 115 MB, because the project
lived in the interchange format and the body of the reel was repeated in each instance.

#### Scenario: Background write does not touch the project and does not clear the asterisk
- **WHEN** there is a content edit and the autosave interval has passed
- **THEN** the project file is byte for byte the same as before, `dirty_content` is lit, and the recovery holds the model with the edit
- **TEST** t358 (`src/apps/kt-app/src/layout_tests.rs`), t194 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The recovery name is appended to the full name
- **WHEN** the projects `film.kt` and `film.mp4.kt`
- **THEN** they have different recovery files next to the project; for an unnamed one — the data directory
- **TEST** t357 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Ctrl+S removes only its own recovery
- **WHEN** its own and someone else's recovery lie side by side and the user saves
- **THEN** its own is deleted, the other's is intact, and the `recovery_written` mark is cleared
- **TEST** t359 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Viewport drift does not write, an edit does
- **WHEN** only the zoom and scroll changed on a clean project
- **THEN** background writing does not start; after a content edit — it starts
- **TEST** t360 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The recovery guard does not block saving
- **WHEN** the background recovery write has raised its own guard
- **THEN** the project's guard remained at zero, and Ctrl+S of the same revision passes
- **TEST** t361 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Edits inside a gesture advance the revision
- **WHEN** the FX window or a drag of an edge edits the model via `apply_coalesced`
- **THEN** the revision grows and autosave does not consider the edit saved
- **TEST** t200 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Autosave of a large project stopped being an event
- **WHEN** a project with a reel in several instances (a scene that gives megabytes in the interchange format) is written by autosave
- **THEN** the recovery file is orders of magnitude smaller than the same project in the interchange format, and the write fits into the interval without interface lag
- **TEST** t815 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Live SIGKILL
- **WHEN** an edit by a real gesture → background write → `SIGKILL` → restart
- **THEN** the project file is untouched, the program itself offers to recover, the work is in place — and this twice over
- **TEST** live `recover.py` (`src/scripts/live/recover.py`, via `src/scripts/live-check.sh`)
