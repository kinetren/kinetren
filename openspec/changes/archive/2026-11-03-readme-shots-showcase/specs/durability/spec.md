## MODIFIED Requirements

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
