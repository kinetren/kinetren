## ADDED Requirements

### Requirement: A landed save clears only what the file holds
When Ctrl+S's write lands, the view-state asterisk SHALL clear only if the view state now equals the
one the file was written with.
Why: class-walk-revision — one-scheduler made the write async; a bin or view edit made during it
moves no `model_rev`, and the landed save cleared its asterisk though the file lacked it.

#### Scenario: A view edit during the write keeps its asterisk
- **WHEN** an edited project's Ctrl+S is writing and the person toggles the bin's free layout
- **THEN** the write lands with the content asterisk gone and the view asterisk kept; a second Ctrl+S clears it
- **PATH** an edited project → Ctrl+S → at once the bin's free-layout toggle → the write lands, the title keeps its asterisk; Ctrl+S again → the asterisk goes
- **TEST** t2452 (`src/apps/kt-app/src/ui_wait_tests.rs`)

### Requirement: Closing while playing lets the close's jobs run
Closing the window SHALL stop playback before it waits for the autosave and the poster, so the
player's pause vote does not hold the jobs it waits for.
Why: class-walk-revision — both are Background jobs; closed while playing, the close waited out each
bound (5 s, 3 s) with the jobs never started.

#### Scenario: The close drops the playing vote first
- **WHEN** an edited project plays long enough for the insurance autosave to be posted, and the window is closed
- **THEN** the playing vote is down when the close has run
- **PATH** an edited project playing for over a minute → the window's cross → the window goes at once; the recovery file holds the edit
- **TEST** t2453 (`src/apps/kt-app/src/ui_wait_tests.rs`)
