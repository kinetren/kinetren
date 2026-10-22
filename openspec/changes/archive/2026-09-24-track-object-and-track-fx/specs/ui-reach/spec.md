# Spec Delta

## ADDED Requirements

### Requirement: A track has the full set of verbs, paired paths share one body
A track SHALL be a timeline entity with the full set of verbs (rename, delete, insert above/below, reorder, mute, solo, lock, level, blend mode, chain, properties), each reachable from the badge, the menu, and the key where the path is declared; the set is checked by a machine against the paired entities; the live scenario SHALL assert the consequence (the picture changes on mute and on solo, the lock refuses a drag and names the track, a vignette placed in the track window lies ON THE TRACK, not on the event, the render window lists the mute). The meters of a silent file would say nothing — the sum with level and mute is held by t732 on the engine side.
Why: the tests of R.38 checked the reachability of what was done, not the consequence; the rule of R.40 — probe from the point where the person acts.

#### Scenario: Verbs and paths
- **WHEN** the badges, the menu, the keys and the track properties window are scanned
- **THEN** the set of verbs is complete and matches for the root and inside a reel; paired paths lead to one command
- **TEST** t741 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A live scenario of a track
- **WHEN** `src/scripts/live/track.py` mutes a track, sets solo, locks and drags an event, applies a vignette from the track window, opens the render window
- **THEN** the color in the preview changes to the color of the track under the muted one and back; a drag on the locked one does not move the event, and the status bar names it; the vignette ends up in `kt.tracks`, not on the event; the render window shows the line about muted tracks
- **TEST** `src/scripts/live/track.py` (live scenario, `./scripts/live-check.sh`)
