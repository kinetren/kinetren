## ADDED Requirements

### Requirement: An offline frame job keeps its own stream history and waits out a hold inside a frame
The reel strip's and the poster's renderer SHALL keep its stream stages' history in a memo of its own,
as an export does, never the preview's. Its stream pre-roll SHALL stop when its job is held (a playback
or an export's vote), not only when it is cancelled, keeping what it computed; the job SHALL wait the
hold out between the pre-roll's batches, never inside a device job, and SHALL go on from the memo after
it; a cancel while held SHALL end it.
Why: stream-memo-budget (rows 519, 514) — the jobs kept `VideoRenderer::new`'s default memo, sized for
the player's loop and not counting them, so a strip of a Datamosh evicted the player's history; and the
pre-roll's stop check answered only the cancel, so the history batches inside one frame ran on through
playback while the frame loop around them waited.

#### Scenario: The job's renderer has its own memo
- **WHEN** a plain renderer and an offline job's renderer are built for one project
- **THEN** the plain one shares the preview's memo; the job's does not, and holds the export's bound
- **TEST** t2493 (`src/apps/kt-app/src/reel_preview.rs`)

#### Scenario: The pre-roll stops on a hold and goes on after it
- **WHEN** a pre-roll's job becomes held during its first run and is released by another thread; separately, a job is cancelled while held
- **THEN** the stages' stop check answers the hold during that run; the pre-roll runs a second time, never while still held; the cancelled one runs once
- **TEST** t2494 (`src/apps/kt-app/src/reel_preview.rs`)
