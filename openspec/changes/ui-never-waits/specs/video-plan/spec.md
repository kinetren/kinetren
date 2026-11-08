## ADDED Requirements

### Requirement: The kept moments are checked against an edit off the UI thread
The bars' "plays without work" layer SHALL be read on the UI thread from the map of kept moments
alone. Moments presented before an edit SHALL be checked against the current plan on the preview's
own thread, between frames, a few per pass and yielding to a frame that is asked; the executor's memo
SHALL be asked with the map free. While moments wait, the query says the check is pending and the
preview's thread wakes the UI when it moves.
Why: the check builds frame plans, and a plan with a stream stage runs its kernel (compute jobs read
back with a blocking poll); `holds` takes the engine's lock, which the preview's thread holds for a
whole composite. Both were asked by `coverage_runs` on the UI thread.

#### Scenario: The bars while the executor's memo is held
- **WHEN** a presented moment is on the bars, the executor's memo is held, the project is edited, and the bars are asked and frames asked three times
- **THEN** every query returns at once, says the check is pending and no longer draws the moment as kept; once the memo answers the moment comes back
- **TEST** t1843 (`src/libs/kt-engine/tests/compose_queue.rs`)
