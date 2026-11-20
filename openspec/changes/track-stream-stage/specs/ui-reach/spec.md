## REMOVED Requirements

### Requirement: Datamosh is refused on a track with its reason
**Reason**: a track stream stage exists (track-stream-stage, `video-fx`); the refusal kept a working device off the track.
**Migration**: replaced by "Datamosh lands on a track and its sound half follows"; t3237 is rewritten to the new consequence (t3393), not weakened.

## ADDED Requirements

### Requirement: Datamosh lands on a track and its sound half follows
The track window SHALL let Datamosh land on a video track's chain from the picker, a paste or a preset, like
any effect. With a sound track linked, the sound half SHALL appear on it in the same undo step. The track's
picture SHALL show the mosh in the preview, or Loading while its history is walked; never a frame of
another moment.
Why: found-defects 820 — the device was refused only because the engine had no track stream stage
(track-stream-stage).

#### Scenario: Datamosh picked in a track's window
- **PATH** right click Video 1's header → Track properties → open the graph's picker → type "mosh" → click Datamosh → the graph holds a Datamosh node and the Audio 1 lane shows its sound node → the preview says Loading, then shows the track moshed → Ctrl+Z: both nodes go
- **WHEN** the person picks Datamosh in a video track's window, with Audio 1 linked, then undoes
- **THEN** Video 1's chain holds Datamosh and Audio 1's `datamosh_sound`; the preview shows the mosh once its history is walked; one undo removes both
- **TEST** t3393 (`src/apps/kt-app/src/node_editor_tests.rs`); planned live scenario `track_mosh`

#### Scenario: The track mosh costs nothing at rest
- **PATH** the heavy showcase with Datamosh on Video 1 → Track properties open → the mouse rests over the preview, then moves over the graph's nodes → nothing redraws by itself once the history is walked
- **WHEN** a paused project with a track mosh stands with the track window open and the mouse moving over it
- **THEN** after the walk no frame is asked at rest, a hover asks no layout, and no stream position is walked again
- **TEST** t3394 (`src/apps/kt-app/src/idle_tests.rs`)
