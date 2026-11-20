## ADDED Requirements

### Requirement: The track window edits a video track and its sound track in one graph
A video track's window SHALL show, while the track has a sound track, ONE node editor: the video
track's chain on the picture lane and the sound track's chain on the sound lane, a switch per lane at
its In node, a device's halves in one column with each half's own switch. A "Device sound" row SHALL
pick the sound track among the free audio tracks of the reel or none, shown only while the chain holds
a simulated device or a link exists. The sound track's edits SHALL be part of the window's session
and of its Cancel.
Why: kkt-26.11.18b line 29 and fx-graph D10 — the pair is edited where the device is; rule 17, one
widget; rule 9, no dead control.

#### Scenario: The tape on a track and its sound half's switch
- **PATH** right click Video 1's header → Track properties → the track window opens → add VHS Tape from the graph's picker → the graph shows the picture lane and the sound lane with the tape's two halves in one column, and the row "Device sound: Audio 1" appears → click the sound half's switch → the half is off, the picture half stays on → pick "None" in the row → the sound lane goes, the row stays
- **WHEN** the person puts VHS Tape on a video track in its window, switches the sound half off, then clears the link
- **THEN** the model's Audio 1 chain holds the half bypassed while Video 1's tape is on; clearing removes the half; Cancel restores both tracks
- **TEST** t3235 (`src/apps/kt-app/src/node_editor_tests.rs`), live `track_sound` (`src/scripts/live/track_sound.py`)

#### Scenario: The track window costs nothing at rest
- **PATH** a video track carrying VHS Tape and Camera ISP, linked to Audio 1 → Track properties → the track window's graph shows both lanes → the mouse rests, then moves over the nodes → nothing redraws by itself
- **WHEN** the track window of a video track with a device and its sound track is open on a pause, with the mouse moving over it
- **THEN** no frame is asked at rest and a hover asks no layout
- **TEST** t3236 (`src/apps/kt-app/src/idle_tests.rs`)
