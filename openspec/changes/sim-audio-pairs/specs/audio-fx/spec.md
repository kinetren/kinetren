## ADDED Requirements

### Requirement: A simulated device is one across an A/V link group
When a paired kind's slot is added to, edited on or removed from an event, the model SHALL, in the
same undo step, give every linked event of the other kind (video ↔ audio) the counterpart slot: the
n-th slot of a paired kind matches the n-th slot of its counterpart; values and curves copied, a
missing one appended at the chain's end, an extra one removed. The slot's bypass and masks SHALL stay
each side's own. A morpher patch SHALL hand the counterpart of each enabled paired slot, with its
curves, to the events of the other kind, unless the patch holds that slot itself.
Why: kkt-26.11.03b D1 — adding VHS Tape to an event with audio must put its sound on that audio,
visible and switchable off; presets, paste, automation and morpher batches go through the same doors.

#### Scenario: VHS Tape on a linked pair
- **WHEN** VHS Tape with head A clogged is set on a video event linked to an audio event with a gain;
  undone and redone; the sound slot's tracking edited; a tracking curve put on the picture's slot; the
  sound switched off; a second tape added; every tape removed
- **THEN** the audio chain is `gain, vhs_tape_sound` with the clogged head; undo removes both halves;
  the picture follows the sound's tracking; the curve reaches the sound slot only; the picture stays
  on; two sounds follow two tapes; the gain alone remains
- **TEST** t1461 (`src/libs/kt-model/tests/sim_pairs.rs`)

#### Scenario: A morpher layer's tape reaches the sound
- **WHEN** a patch carries VHS Tape with a tracking curve; then bypassed; then holding the sound itself; a patch carrying only the sound
- **THEN** its audio chain is `vhs_tape_sound` with the curve in the same position; bypassed — none; held — one; the video chain of the sound-only patch is `vhs_tape`
- **TEST** t1461b (`src/libs/kt-model/tests/sim_pairs.rs`)

#### Scenario: The deck's sound on the linked audio in the effects window
- **PATH** a video event linked to its audio → VHS Tape added to the picture → the effects window
  shows both halves' chains, each with the deck face and its Sound section (Hi-Fi / Normal / Mix,
  mono/stereo linear tracks, Dolby B, the audio head) → click "Normal (linear)" on the audio half's
  face → both halves switch to the linear track
- **WHEN** the person adds VHS Tape to the picture and presses "Normal (linear)" on the audio's face
- **THEN** the audio chain holds `vhs_tape_sound`, two Sound sections are drawn, and both halves'
  `audio_out` is Normal in the MODEL
- **TEST** t1468 (`src/apps/kt-app/src/vhs_tape_tests.rs`), live `vhs_sound` (t1469, `src/scripts/live/vhs_sound.py`)

### Requirement: The VHS deck's sound is the same deck's linear track and Hi-Fi FM
`vhs_tape_sound` SHALL simulate the two VHS audio systems of the deck `vhs_tape` simulates, from the
same rows and the same timeline time, never by a look-alike filter:
- the linear track — a stationary head at the tape's speed (PAL SP 23.39 mm/s, NTSC SP 33.35 mm/s,
  LP ½, EP ⅓): the record level and the bias on the tape's transfer curve, the gap, Wallace spacing and
  coating thickness losses at λ = v / f against a playback EQ designed for the nominal head at SP,
  hiss from the track width (mono 1.0 mm, stereo 0.35 mm) and the tape's C/N, oxide defects at the
  picture's rate passing the head for `dropout_us · v_write / v_tape`, the capstan's displacement in
  the phase of the picture's 5.3 Hz flutter term and as wide as the jitter row, Dolby B;
- Hi-Fi — FM carriers (PAL 1.4/1.8 MHz, NTSC 1.3/1.7 MHz, ±150 kHz) behind emphasis and a 2:1
  compander, read at each picture line by the head the picture's geometry names, with the spacing and
  gap losses at the carrier's wavelength, the heads' outputs and the tracking coverage; the picture's
  own dropout field cuts the carrier; FM noise from the C/N; the switching pulse holds the sample of
  the head switch; with the DOC on the carrier detector mutes a lost carrier;
- a still tape plays nothing.
Why: kkt-26.11.03b D1; F-FX-012 — the defect must be heard where it is seen.

#### Scenario: A clogged head is seen and heard in the same field
- **WHEN** head A's spacing is 3 µm: the picture of one frame, and a 1 kHz tone through Hi-Fi
- **THEN** the picture loses field A and keeps field B; the sound is lost while head A reads (from
  the switch point before field A) and plays while head B reads; a clean deck gives the tone back
- **TEST** t1462 (`src/plugins/fx-video-tape/tests/sound.rs`)

#### Scenario: A dropout is a click at its line
- **WHEN** 40 dropouts/s of 40 µs with the DOC off, a 700 Hz tone through Hi-Fi
- **THEN** most samples on the lines the picture's line table drops are broken, and more than 95 % of the rest are clean
- **TEST** t1463 (`src/plugins/fx-video-tape/tests/sound.rs`)

#### Scenario: LP dulls the linear track
- **WHEN** 6 kHz and 500 Hz tones through the linear track at SP and at LP
- **THEN** at LP 6 kHz falls at least 6 dB more than 500 Hz; SP is flat within 1.5 dB
- **TEST** t1464 (`src/plugins/fx-video-tape/tests/sound.rs`)

#### Scenario: The capstan wobbles the linear track
- **WHEN** a 1 kHz tone through the linear track with 0.3 µs of jitter
- **THEN** the tone's delay follows sin(2π(5.3 t + 0.37)) with a correlation above 0.9 and a width of 0.3 · 43.2 samples within 25 %
- **TEST** t1465 (`src/plugins/fx-video-tape/tests/sound.rs`)

#### Scenario: The sound is a function of the rows and the timeline
- **WHEN** the same rows and input are played twice at one timeline seat, and once 4801 frames later
- **THEN** the two runs are byte-equal and the shifted one differs
- **TEST** t1466 (`src/plugins/fx-video-tape/tests/sound.rs`)

### Requirement: The stream's sound is a real audio codec broken by the picture's schedule
`datamosh_sound` SHALL code the event's sound with a real lossy codec (`kt_plugin_api::audio_codec`:
an MDCT of 2048/1024, per-line backward-adaptive prediction with resets, Rice-coded lines, a bit
reservoir, a decoder that checks what it parses and conceals what it cannot decode), and SHALL break
that stream by the datamosh's own rows on the picture's own schedule (`mosh::Sched` over the audio
event's layout, on the project frame grid): the encoder resets at every I-frame of the picture's GOP;
an I-frame the `mosh` gate deletes is deleted with its audio packet, so the decoder conceals that frame
and every frame that borrowed bits from it, misses the reset, and the residual lands on the wrong
prediction until an intact I-frame; `bloom` repeats the run's first P-frame's packet; `freeze` holds the
predictor's history; `residual` scales the residual applied; `loss` flips bits in hit packet slots;
`qp` is the finest quantiser step. Motion vectors have no audio meaning (foreign rows). The predictor
SHALL stay inside its stability triangle and the decoder inside its range: a broken stream smears and
saturates at full scale, it never runs away.
Why: kkt-26.11.03b D1; F-FX-013 — a cut mosh in the picture is a broken audio frame at the same moment.

#### Scenario: The codec's transform and its code are exact
- **WHEN** a block goes through the fast DCT-IV, a signal through MDCT and overlap-add, lines through the Rice code
- **THEN** the DCT equals the direct sum, the signal comes back exactly, the lines come back and their size is predicted
- **TEST** t1470, t1470b, t1470c (`src/libs/kt-plugin-api/src/audio_codec.rs`)

#### Scenario: The intact stream plays the sound back
- **WHEN** a moving tone through the stream with no breakage: with prediction, without it, and at `qp` 51
- **THEN** no frame is concealed, the SNR is above 25 dB, the coarser quantiser costs fidelity
- **TEST** t1471 (`src/plugins/fx-video-codec/tests/sound.rs`)

#### Scenario: A deleted I-frame breaks the sound until the Reset
- **WHEN** `mosh` is drawn on from 0.6 s to 1.8 s of an event starting at 3.0 s (GOP 24, 25 fps: the I-frame at 3.96 s is deleted) and a Reset key rises at its end
- **THEN** up to the audio frame of 3.96 s the output equals the intact stream byte for byte; frames are concealed there; the sound stays broken (error > 0.05) until the Reset; after it the error is below 0.01
- **TEST** t1472 (`src/plugins/fx-video-codec/tests/sound.rs`)

#### Scenario: Bloom, loss and the residual act as in the picture
- **WHEN** `bloom` is on; `loss` is 0.4 (and 0); `residual` is 0.25
- **THEN** the bloom's repeated packet moves the sound away from the intact stream; the loss breaks frames and at 0 changes no byte; the cut residual lets the prediction run (five times the error)
- **TEST** t1473, t1474, t1475 (`src/plugins/fx-video-codec/tests/sound.rs`)

#### Scenario: The stream is a function of the rows and the timeline
- **WHEN** the same rows and input are played twice
- **THEN** the two runs are byte-equal, concealment included
- **TEST** t1476 (`src/plugins/fx-video-codec/tests/sound.rs`)
