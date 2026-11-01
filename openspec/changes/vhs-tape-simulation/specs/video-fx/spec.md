## ADDED Requirements

### Requirement: The VHS tape simulation writes the signal onto a modelled tape and reads it back
The kind `vhs_tape` SHALL compute its picture by the physical chain of a VHS recorder and player
(`docs/features/fx/simulate/vhs-tape-simulation.md`), not by image operations:
- the frame is resampled onto the standard's signal grid at 4 fsc (PAL 1135×576, NTSC 910×480);
- the luma is pre-emphasised (τ 1.3 µs, ×5), clipped and frequency-modulated between the sync tip
  and white carriers (PAL 3.8–4.8 MHz, NTSC 3.4–4.4 MHz) by a phase accumulator;
- the chroma is band-limited and recorded as colour-under (626.953 / 629.371 kHz) with the per-track
  rotation (PAL 0 / −90° per line, NTSC −90 / +90°) under the FM, through tape saturation;
- each row is read by the head of its field through the channel of that head: spacing loss, gap loss
  and, for the strip of the neighbour track under the head, the azimuth loss of the ±6° gaps, with
  gains given by the head-to-track geometry (track pitch per speed, head widths of a two- or four-head
  drum, tracking offset, scan-angle mismatch, still), plus tape and amplifier noise and dropouts;
- the output switches heads at the switching point before the field's end;
- playback FM-demodulates, compensates dropouts, de-emphasises, demodulates the colour-under with the
  track's rotation undone, applies ACC and the 1 H (NTSC) / 2 H (PAL) comb, and reads the rows with
  the time-base error of the tape unless a TBC is on.
Its parameters SHALL be physical quantities in their units (µm of spacing, gap, tracking and skew;
µs of dropout length, switching skew and jitter; dB of head output and C/N; per head and per drum;
the standard, speed, deck count, head count, transport, TBC, DOC and comb). With one deck the same
drum records and plays; with two, the record drum's head defects are written into the tape and the
play drum adds its own. The output is opaque. The imitation `vhs` stays unchanged.
Why: kkt-26.11.01 V1, the owner: "really take a VHS tape, really write a magnetic signal onto it,
then really read it back"; answer 4: "you … completely ignore the complexity of the effects'
physical mechanics". The root rule: an imitation is not a simulation.

#### Scenario: Without impairments the FM round trip gives back the luma
- **WHEN** a luma ramp and a bar pattern pass through the chain with ideal heads (no spacing, a
  minimal gap, the head exactly on its track, no noise, no dropouts, no chroma, no saturation)
- **THEN** the demodulated, de-emphasised luma equals the record-side band-limited luma within 1.5 %
  of the white level away from the clipped edges, and the FM carrier measured on the written signal
  is the standard's sync-tip and white frequency within 1 %
- **TEST** t1180 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: Colour-under halves nothing of the luma and cuts the chroma
- **WHEN** a horizontal sinusoid sweep is put into the luma, then into U, and measured at the output
- **THEN** the luma passes 1.0 MHz within 3 dB of its 0.2 MHz level; the chroma at 1.0 MHz is at
  least 12 dB below its 0.2 MHz level (the colour-under bandwidth ~0.5 MHz)
- **TEST** t1181 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: A clogged head kills every other field
- **WHEN** the playback head A's spacing is 3 µm (head B clean)
- **THEN** the even rows (field A) are noise — their error against the clean run is at least 8×
  that of the odd rows — while the odd rows stay within the clean run's error
- **TEST** t1182 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: The tracking knob moves the noise bar
- **WHEN** two decks with a scan-angle mismatch (skew) play with the tracking offset at two values
- **THEN** the noise per row has a bar where the head leaves its track, and the bar's position moves
  in the direction and by the amount the geometry predicts (Δu = −Δtracking / skew, within 0.06 of a
  field); at a centred tracking with no skew there is no bar
- **TEST** t1183 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: Azimuth rejects the neighbour's FM and lets its chroma leak, which the comb cancels
- **WHEN** the neighbour-track kernel of a row with a strip w of the neighbour under the head is
  measured at the FM and colour-under frequencies, and a picture whose two fields differ is played
  with the head half on the neighbour, with the comb on and off
- **THEN** the kernel's transfer follows sinc(π·w·tan 12°·f/v) within 1.5 dB at both frequencies
  (≥ 20 dB more loss at 4.3 MHz than at 0.63 MHz); the neighbour's luma leaks less than its chroma;
  the comb reduces the chroma leak by at least 10 dB
- **TEST** t1184 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: Time is the only state
- **WHEN** the same frame is rendered twice at one timeline position, at two positions, and a sequence
  of positions in reverse order
- **THEN** one position gives identical bytes, two positions give different noise and dropouts, the
  reverse order gives the same bytes per position; the kernel calls no platform transcendental
- **TEST** t1185 (`src/plugins/fx-video-tape/tests/physics.rs`), t1066

#### Scenario: The GPU graph is within its tolerance of the CPU
- **WHEN** the kind runs on the GPU executor at its defaults and at each preset, PAL and NTSC, at K=1
  and K=2, on a frame with alpha
- **THEN** the result is within `FrameTol::SIGNAL` of the CPU, and the graph is deterministic on the
  device
- **TEST** t1186 (`src/libs/kt-gpu/tests/vhs_tape.rs`)

#### Scenario: Still on two heads shows a bar, on four heads it does not
- **WHEN** the transport is still on a two-head and on a four-head drum
- **THEN** the two-head picture has a noise bar inside the field (the scan crosses one pitch); the
  four-head picture has none and its two fields carry the same lines
- **TEST** t1188 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: LP is noisier, not softer
- **WHEN** the same picture is played at SP and at LP on a two-head drum
- **THEN** the noise at LP is at least 2 dB higher, and the luma response at 1 MHz is the same within
  1 dB (the writing speed, hence the wavelength, is unchanged)
- **TEST** t1189 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: A defect of the recorder survives a clean player; a chipped head flickers
- **WHEN** two decks play, the RECORD drum's head A clogged and the play drum clean; and one deck with
  head B chipped (output −6 dB, a 0.6 µm gap)
- **THEN** field A is noise in the first case although the playing heads are clean; in the second the
  odd rows are noisier than the even rows by at least 6 dB
- **TEST** t1190 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: The TBC removes the line jitter and the switching skew, not the switching noise
- **WHEN** a picture with vertical edges is played with jitter and switching skew, with and without a
  TBC, and the switching point is moved
- **THEN** without the TBC the edge position varies between rows and the rows after the switching
  point are displaced by the skew; with the TBC both are gone; the rows affected follow the
  switching point
- **TEST** t1191 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: The dropout compensator hides dropouts with the line before
- **WHEN** a worn tape with many dropouts is played with the compensator on and off
- **THEN** the error of the rows with dropouts is at least 2× lower with the compensator on
- **TEST** t1192 (`src/plugins/fx-video-tape/tests/physics.rs`)

#### Scenario: The effect window shows the deck face
- **WHEN** the effects window of an event with the VHS tape simulation is opened, and the head
  selector picks head B and sets it to "chipped"
- **THEN** above the parameters a deck face shows the drum with its heads, the speed switch, the
  tracking knob and the deck count; the chosen head is drawn in the chipped colour and the passport
  values of that head changed to the chipped preset's
- **PATH** select an event → Effects (+ effect) → pick "VHS tape (simulation)" → the effects window
  shows a deck face (a drum with two heads A and B, SP/LP/EP, a tracking knob) over the parameter
  grid → click head B on the drum → the head selector names B → pick "Chipped" → head B turns the
  chipped colour, its output and gap fields change
- **TEST** t1193 (`src/apps/kt-app/src/vhs_tape_tests.rs`)
