## REMOVED Requirements

### Requirement: V-fps counts new pictures of the moment asked
**Reason**: it counted a held stream frame apart ("held" frames a second); no held frame is shown any
more (kkt-26.11.10b K1/K5), so V-fps counts complete new pictures and there is no held count.
**Migration**: "V-fps counts complete new pictures of the moment asked"; t2358 is rewritten to it.

### Requirement: The preview marks a held stream frame
**Reason**: no composite shows a held stream frame any more (`video-plan`, "No stage runs over a
stand-in input"); the owner asked for the catch-up to be written where Loading is (kkt-26.11.10b K2).
**Migration**: "Loading names a stream catch-up where it is written"; t2359 is replaced by t2415.

## ADDED Requirements

### Requirement: V-fps counts complete new pictures of the moment asked
The performance panel's V-fps SHALL count the shown frames that are a complete new picture of the moment
the preview asked for; an incomplete composite (a frame still loading, a stream stage catching up) SHALL
NOT count, whether or not it is drawn.
Why: found-defects 393 — his trace: 114 shows in 6 s (19/s), 111 of them a held stream frame, 45
distinct pictures; the counter said 20 where "at most 5 are new". Since catch-up-shows-loading no held
frame is shown, so there is no held count either.

#### Scenario: Only complete new pictures count
- **WHEN** the app takes a second of incomplete composites of advancing moments, then a second of complete new moments, then the last moment again
- **THEN** the first second's V-fps is 0, the second's counts the composites, and the repeated moment is no new picture
- **PATH** play over a Datamosh event whose history is behind → the toolbar reads 0 vfps and the preview says "Loading — catching up: N"; when the history catches up → the vfps rise and the scrim goes
- **TEST** t2358 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Loading names a stream catch-up where it is written
While the frame on screen is incomplete because a stream stage's history is behind, the Loading scrim
(the owner's rule: an incomplete frame past 180 ms, the last full picture dimmed under it) SHALL write,
in the place and style of "Loading", "Loading — catching up: N", N the positions the history still lacks
for the frame asked, updated as composites land; any other incomplete frame keeps plain "Loading". No
marker SHALL be drawn in the preview's corner. The text SHALL ask for no frame at rest.
Why: kkt-26.11.10b K2 — "catchung up надпись ну очевидно можно в том же месте где loading писать".

#### Scenario: The scrim names the catch-up
- **WHEN** the app takes an incomplete composite whose stream stage lacks 37 positions and 180 ms pass, then an incomplete composite with nothing short, then a complete one, then sits at rest
- **THEN** the scrim's text is "Loading — catching up: 37", then "Loading", then no scrim; nothing is written in the preview's corner; at rest no frame is asked for
- **PATH** play over a Datamosh event whose history is behind → the picture dims and "Loading — catching up: N" stands in the preview's centre, N falling as the history walks → the scrim goes when the moment's own picture lands
- **TEST** t2415 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A person plays into a Datamosh region that is catching up
- **PATH** open the showcase with its mosh cut → the preview stands before the cut → press Space → the cut's frames are not ready: the preview dims with "Loading — catching up: N" in its centre and no corner note → the history catches up and the mosh plays → pause on an uncomputed moment further on → the same Loading text, then the picture
- **WHEN** the live scenario runs on our fixtures
- **THEN** the screenshots of each step are saved, the dump's Loading text names the catch-up while the stream is behind, and no corner marker is drawn
- **TEST** t2416 live `datamosh.py` (`src/scripts/live/datamosh.py`, via `src/scripts/live-check.sh`)
