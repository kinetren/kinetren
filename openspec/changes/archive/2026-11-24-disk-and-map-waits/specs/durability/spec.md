## ADDED Requirements

### Requirement: The journal and the store's small files wait for no disk on the UI thread
A line of `kt-app.log` SHALL be stamped when it is said and written by a thread of its own, in order; the
workspace, the recent list, the templates, the tour's progress and the start choice SHALL be written by a
thread of their own, the newest text of a file replacing one still waiting, and a read of such a file
SHALL see what is on its way. The exit, a panic and a forced exit SHALL wait for them within a bound.
Why: found-defects 833 — a locked track's refusal held a frame for 5.2 s on a busy disk, and the hint it
had to show came and went inside that frame.

#### Scenario: A line is said while the disk answers nothing
- **WHEN** the disk stands and more lines are said than the journal keeps waiting; then the disk answers
- **THEN** every call returns; the file has the lines in order and says how many were dropped
- **TEST** t3727 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: A small file is saved while the disk answers nothing
- **WHEN** a file is being written on a standing disk and another is saved twice; then the disk answers
- **THEN** the saves return, a read gives the newest text, and the file gets that text once, synced since
  the older one had to be
- **TEST** t3728 (`src/apps/kt-app/src/later.rs`)

#### Scenario: The refusal of a locked track is read at once
- **WHEN** an event of a locked track is dragged
- **PATH** lock the lower track → drag its event: it stays, and the status line names V2 and its lock at once
- **THEN** the status line names the track
- **TEST** live `track` (`src/scripts/live/track.py`)
