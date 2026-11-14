## ADDED Requirements

### Requirement: The insurance autosave reaches the recovery file while playing
An autosave due after two intervals of playback or a gesture SHALL run on a lane no pause vote holds,
off the audio and the interface threads, so the recovery file holds the edit while the playback goes on.
An autosave posted on a pause and then held by a playback for another interval SHALL be taken by that
lane; the write SHALL happen once, by whichever lane starts it first.
Why: stream-memo-budget (row 513) — the "write anyway after two intervals" gate posted the job on the
Background lane the player's vote holds, so nothing reached the recovery file until a pause: a crash
during a long playback lost every edit since it began.

#### Scenario: Written while playing
- **WHEN** an edited project plays past two autosave intervals; separately, an autosave is posted with a playback's vote raised and stays held for another interval
- **THEN** the recovery file holds the edit while the vote is still up, in both cases
- **TEST** t2495 (`src/apps/kt-app/src/ui_wait_tests.rs`)
