## ADDED Requirements

### Requirement: The program's own lesson copy makes nobody wait for the disk
"Next lesson" on an edited lesson copy, a lesson's first copy and its reading SHALL wait for no write: the
text goes to the disk behind, without an fsync, and a reading of the copy meanwhile sees it.

Why: found-defects 842 — the next lesson opened 31.8 s after the click, the time of one fsync.

#### Scenario: The disk answers nothing
- **PATH** Finish a lesson → press Next lesson while the disk is busy → the next lesson is there at once → Back to the former one: the edits are kept
- **WHEN** the save lane is held and the person presses Next lesson
- **THEN** the next lesson opens within the click's frames and the copy holds the edit
- **TEST** t3266 (`src/apps/kt-app/src/reach_tests.rs`)
