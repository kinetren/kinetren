## MODIFIED Requirements

### Requirement: A simulated device is one across an A/V link group
When a paired kind's slot is added to, edited on or removed from an event, the model SHALL, in the
same undo step, give every linked event of the other kind (video ↔ audio) the counterpart slot: the
n-th slot of a paired kind matches the n-th slot of its counterpart; values and curves copied, a
missing one appended at the chain's end, an extra one removed. The slot's bypass and masks SHALL stay
each side's own. A morpher patch SHALL hold both halves as its own slots by the same rule (capability
`morpher`, "A patch holds both halves of a paired device, each switchable"); no half SHALL be derived
at render time.
Why: kkt-26.11.03b D1 — adding VHS Tape to an event with audio must put its sound on that audio,
visible and switchable off; presets, paste, automation and morpher batches go through the same doors.
kkt-26.11.18b item 3 — the derived half of a morpher could not be switched off.

#### Scenario: VHS Tape on a linked pair
- **WHEN** VHS Tape with head A clogged is set on a video event linked to an audio event with a gain;
  undone and redone; the sound slot's tracking edited; a tracking curve put on the picture's slot; the
  sound switched off; a second tape added; every tape removed
- **THEN** the audio chain is `gain, vhs_tape_sound` with the clogged head; undo removes both halves;
  the picture follows the sound's tracking; the curve reaches the sound slot only; the picture stays
  on; two sounds follow two tapes; the gain alone remains
- **TEST** t1461 (`src/libs/kt-model/tests/sim_pairs.rs`)

#### Scenario: A preset or a pasted chain carries the pair
- **WHEN** a chain block holding VHS Tape with a curve is appended to the picture's chain; a block holding the sound to the audio's
- **THEN** the audio holds the sound with the curve; the picture holds the tape
- **TEST** t1461c (`src/libs/kt-model/tests/sim_pairs.rs`)

#### Scenario: A morpher layer's tape reaches the sound
- **WHEN** a patch carries VHS Tape with a tracking curve; then the picture slot bypassed; then the sound half bypassed instead; a patch given only the sound
- **THEN** its audio chain is `vhs_tape_sound` with the curve in the same position; picture bypassed — the sound stays; sound bypassed — none on the audio, the tape on the video; the sound-only patch holds `vhs_tape` on its picture chain too
- **TEST** t1461b (`src/libs/kt-model/tests/sim_pairs.rs`, rewritten: the derivation is gone and "bypassed — none" now reads per half)

#### Scenario: The deck's sound on the linked audio in the effects window
- **PATH** a video event linked to its audio → VHS Tape added to the picture → the effects window
  shows both halves on the graph's two rails, tied, each with the deck face and its Sound section
  (Hi-Fi / Normal / Mix, mono/stereo linear tracks, Dolby B, the audio head) → click the sound half →
  click "Normal (linear)" on its face → both halves switch to the linear track
- **WHEN** the person adds VHS Tape to the picture and presses "Normal (linear)" on the audio's face
- **THEN** the audio chain holds `vhs_tape_sound`, two Sound sections are drawn, and both halves'
  `audio_out` is Normal in the MODEL
- **TEST** t1468 (`src/apps/kt-app/src/vhs_tape_tests.rs`), live `vhs_sound` (`src/scripts/live/vhs_sound.py`)
