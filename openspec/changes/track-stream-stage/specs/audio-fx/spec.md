## MODIFIED Requirements

### Requirement: A simulated device is one across a video track and its sound track
A video track SHALL name at most one audio track of its own track list as its sound track, and an
audio track SHALL be the sound of at most one video track. When a video track's chain gets its first
paired slot and the track has no sound track, the model SHALL link the audio track of the same
number if it exists and is free; a link the person cleared SHALL NOT be re-made while the device
stays. A paired slot added to, edited on or removed from a linked video track's chain, or its
counterpart on the sound track's chain, SHALL be mirrored onto the other chain by the event rule (the
n-th slot of a paired kind matches the n-th counterpart, values copied, a missing one appended, an
extra one removed), in the same undo step; each half's bypass SHALL stay its own. Changing the link
SHALL move the halves; clearing it or removing the sound track SHALL take them off. The sound half
SHALL run in the audio track's stage, seated at timeline frame 0 with the project's frame grid, so it
derives the device at the timeline moment the picture half reads. A stream device (Datamosh) SHALL
pair on a track like any device: its sound half SHALL break the sound track's mix on the picture's
schedule over the SAME track layout the picture's stream stage uses (`video-fx`, "A stream stage on a
video track decodes the track's composite"), so a deleted keyframe, a bloom or a freeze falls at the same
timeline moment in both; between the layout's segments the codec's predictors reset.
Why: kkt-26.11.18b line 29, "Дать звук отдельно"; CLAUDE.md — a simulated device that records
picture and sound simulates both, driven by the same device state. Datamosh was kept out while its
picture did nothing on a track (found-defects 820); track-stream-stage D6 gives it one.

#### Scenario: VHS Tape on a video track sounds on its sound track
- **WHEN** VHS Tape is put on Video 1's chain with Audio 1 present; its tracking is edited; the sound half's audio output is set from the audio track; the sound half is switched off; the tape is removed
- **THEN** Video 1's sound track is Audio 1 and Audio 1's chain holds `vhs_tape_sound`; the tracking reaches it; the audio output reaches the picture half; switched off it stays in the chain bypassed and the picture half stays on; removal takes both; each step is one undo step; Camera ISP put with Datamosh gives Audio 1 `camera_isp_sound` and `datamosh_sound`, in the order of their picture halves
- **TEST** t3230 (`src/libs/kt-model/tests/sim_pairs.rs`)

#### Scenario: The link moves, clears and stays one-to-one
- **WHEN** Video 1 with a tape is linked to Audio 2; then cleared; a value of the tape is edited; Video 2 tries to name Audio 1 while Video 1 does; Audio 1 is removed while linked
- **THEN** the half moves from Audio 1 to Audio 2; clearing takes it off and the edit does not re-link; the second link is refused; removing the sound track clears Video 1's link
- **TEST** t3231 (`src/libs/kt-model/tests/sim_pairs.rs`)

#### Scenario: The track's deck is heard where the picture drops
- **WHEN** an audio track's chain carries the tape's sound half with a head clogged over a tone event that starts off the tape's field grid, at a 30 fps project; then the half is switched off
- **THEN** past the first quarter the track's sound equals the same half carried by the event itself to float rounding (the stage sums and chunks its own way), so the deck is read at the same timeline moment; switched off the track sounds the dry tone byte for byte
- **TEST** t3232 (`src/libs/kt-engine/tests/sim_pairs.rs`)

#### Scenario: A track's Datamosh breaks its sound where the picture breaks
- **WHEN** a video track linked to Audio 1 carries Datamosh deleting the keyframes at its two cuts, over a tone on Audio 1; then the sound half is switched off
- **THEN** the sound track's output smears from each cut's audio frame to the next intact keyframe's, at the timeline moments of the picture's deleted keyframes; switched off it is the dry tone byte for byte
- **TEST** t3391 (`src/libs/kt-engine/tests/sim_pairs.rs`)

#### Scenario: Datamosh pairs on a track in the model
- **WHEN** Datamosh is put on Video 1 linked to Audio 1, its `freeze` is edited, then it is removed
- **THEN** Audio 1's chain holds `datamosh_sound` with the edit mirrored, and removal takes it, each in one undo step
- **TEST** t3392 (`src/libs/kt-model/tests/sim_pairs.rs`)
