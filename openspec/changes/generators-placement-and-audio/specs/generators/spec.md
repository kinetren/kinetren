## ADDED Requirements

### Requirement: Sound kinds — synthesized from the sample number, no picture, AES17 levels
The sound generator kinds (`tone`, `audio_noise`, `sweep`, `beep`, `silence`, family
`generators-audio`) SHALL have no picture: their source reports `has_video: false` and
`has_audio: true`, so creation makes an audio clip only. Each burst SHALL be a pure function of
(the event's values with curves resolved at the burst's tick, the burst index): a seek, a straight
play and an export give the same bytes, and a burst does not depend on the bursts read before it.
A sine's level SHALL be its peak in dBFS (AES17: a full-scale sine is 0 dBFS); a noise's level
SHALL be its RMS on the same scale (RMS = peak/√2 of the equal sine). Every tone SHALL take its
phase from the global sample number through the ONE sine text the SMPTE tone uses
(`kt_plugin_api::generator::sine`): a 1 kHz `tone` at −20 dBFS equals the SMPTE tone byte for byte.
Noise SHALL be counter-based (white; pink — Voss–McCartney held rows; brown — rows weighted by
2^(k/2)), seeded, mono or decorrelated stereo. A sweep's phase SHALL be the integral of its
frequency (linear or logarithmic), repeating every duration. A beep SHALL be the tone gated on
`(n mod period) < length`. Silence SHALL be zeros. `channels` left/right SHALL leave the other
channel silent.
Why: the owner — "of course the audio ones are needed — you hide some generators in the tests —
pull them out" (kkt-26.11.01, answer p.2); a stateful noise filter would give other samples after
a seek than in a straight play (invariant 11), and the naive `sin(2π·f(t)·t)` sweep runs twice as
far and clicks.

#### Scenario: One sine for the tone and SMPTE
- **WHEN** a `tone` at 1000 Hz, −20 dBFS, both channels and an SMPTE source with the tone on at −20 dBFS read bursts 0, 7 and 9000
- **THEN** the bytes are equal; SMPTE's and the leader's bursts equal their pre-change references
- **TEST** t1155 (`src/libs/kt-media/tests/gen_sound.rs`)

#### Scenario: A sound kind has no picture and is seek-stable
- **WHEN** each sound kind's source is opened with non-default values; bursts 0…63 are read in order, then in reverse and at random through a fresh source
- **THEN** `has_video` is false, `has_audio` true; every burst is byte-identical between the orders and the sources; no sample is NaN or outside ±1
- **TEST** t1156 (`src/libs/kt-media/tests/gen_sound.rs`)

#### Scenario: Tone level, frequency and channels
- **WHEN** a `tone` at 440 Hz, −6 dBFS is read for one second; then with `channels` left and right
- **THEN** the peak is 10^(−6/20) within 1e-4, the zero crossings count 880 ± 1; left writes only L, right only R
- **TEST** t1157 (`src/plugins/generators-audio/tests/sound.rs`)

#### Scenario: Noise colour, level and seed
- **WHEN** white, pink and brown `audio_noise` at −20 dBFS is read for 8 seconds, the power density is measured in octave bands from 125 Hz to 8 kHz; the seed and the stereo flag are changed
- **THEN** the RMS is −20 dBFS AES17 within 0.3 dB; the density falls 0 ± 1 dB per octave (white), −3 ± 1 dB (pink), −6 ± 1.5 dB (brown); another seed gives other bytes; stereo off gives L = R, on gives a correlation below 0.05
- **TEST** t1158 (`src/plugins/generators-audio/tests/sound.rs`)

#### Scenario: A sweep follows its curve without phase jumps
- **WHEN** a log and a linear `sweep` 100 Hz → 6400 Hz over 6 s is read
- **THEN** the zero crossings of every 100 ms window equal, within 2, twice the cycles the curve integrates — `f₀·k^(t/T)` (log), `f₀+(f₁−f₀)t/T` (linear); no sample step inside a sweep exceeds `2π·f(t)/48000·amp` by more than 2 %; the sweep repeats after 6 s
- **TEST** t1159 (`src/plugins/generators-audio/tests/sound.rs`)

#### Scenario: Beep is the gated tone, silence is zeros
- **WHEN** a `beep` 1 kHz, period 1 s, length 40 ms and a `tone` 1 kHz with the same level are read for 3 s; a `silence` is read
- **THEN** the beep equals the tone where `(n mod 48000) < 1920` and is zero elsewhere; silence is all zeros with `has_audio` true
- **TEST** t1160 (`src/plugins/generators-audio/tests/sound.rs`)

#### Scenario: A sound generator plays in a render
- **WHEN** a project with a `tone` event at 2–4 s on audio track 1 and a `beep` event on audio track 2 is rendered offline by the audio renderer
- **THEN** the output holds the tone between 2 and 4 s at its level (peak within 0.01 dB), silence before and after, the beep's pips at its period; two renders are byte-identical
- **TEST** t1167 (`src/libs/kt-engine/tests/gen_sound_render.rs`)

#### Scenario: The menu groups the sound kinds under Audio
- **WHEN** the generator menu rows are listed
- **THEN** 24 rows: the 19 former ones in their order, then `tone`, `audio_noise`, `sweep`, `beep`, `silence`, all in the Audio group, each with a catalog name in five languages and a label for every row
- **TEST** t1163 (`src/apps/kt-app/src/gen_audio_tests.rs`)

## MODIFIED Requirements

### Requirement: Generator UI — one list of kinds, the event under the cursor, the form at the chain head
Creating a generator SHALL be available from the Edit menu, from a clip track's right-click menu
(its header and its body) and from the bin menu through one list of kinds (the entries do not
enumerate the kinds themselves); the File menu SHALL NOT offer it. Edit and the bin SHALL create at
the cursor; the track menu SHALL list the kinds that land on that track's kind (picture kinds on a
video track, sound kinds on an audio track) and SHALL create AT THE CLICKED PLACE — that track and
the clicked time (a header click takes the cursor). Creation puts the media AND a clip in one step
of history and opens the form. The form of the generator/title SHALL live
as the first crumb of the event window (the head has no switch); all entries
are routed into it; the change of kind is its own step of history; a session of the form is one
undo step; an edit SHALL reach the model and the source, and undo too. Fractions of the frame SHALL
be shown as percentages (storage — a fraction), a double click on a label resets the
parameter; every parameter has a label, every flag has a control; the colours
of the title are shown as real (direct fields, unmultiplied components). Safe zones
SHALL be drawn over the preview while the title is open; dragging the title in the preview SHALL
write the position with the same command and the same gesture as the form. The bin card SHALL
show the name of the kind, hide a zero duration, count the "×N" places, and build
a real thumbnail without a file; an empty default frame is labelled with the name of the kind;
the event is labelled with the name of the kind and is not repainted on every keystroke.
Why: the owner did not find the single entry (right click on the empty background of the bin);
the title window was a dead end without a kind picker; a double click placed two clips overlapping.
The File entry of R.55 was the wrong place (owner, kkt-26.11.01 item 4 and answer p.2: "WHY THE
HELL LEAVE IT IN FILE?!"): a verb that makes something on the timeline lives in Edit and on the
track a person clicked.

#### Scenario: One list of kinds for all entries
- **WHEN** the File menu, the Edit menu, the clip track menu and the bin menu are checked
- **THEN** File has no generator item; Edit, the track menu and the bin call the common generators menu and do not enumerate `GeneratorKind::all()` themselves
- **PATH** click "File" → no "Create a generator" row; click "Edit" → "Create a generator" → the groups, Audio among them; right click on a track → the same item first; right click on the bin's empty background → the same item
- **TEST** t560 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Edit creates at the cursor
- **WHEN** Edit → "Create a generator" → Audio → Tone is picked with the cursor at 3 s
- **THEN** one history step adds the media and ONE audio clip at 3 s on audio track 1 (no video clip), and the form opens on it
- **PATH** click "Edit" → hover "Create a generator" → hover "Audio" → click "Tone" → an audio event "Tone" appears under the cursor on the first audio track, its form opens at the head of the effects window
- **TEST** t1164 (`src/apps/kt-app/src/gen_audio_tests.rs`)

#### Scenario: The track menu creates at the clicked place
- **WHEN** a right click lands on the body of audio track 2 at 6 s and "Create a generator" → Beep is picked; then on the body of video track 2 at 2 s and Solid colour is picked
- **THEN** the beep's audio clip starts at 6 s on audio track 2, the solid's video clip at 2 s on video track 2; the audio track's list holds only sound kinds and the video track's only picture kinds
- **PATH** right click on the second audio track at 6 s → "Create a generator" → only the Audio group → click "Beep" → the event appears exactly where the click was → right click on the second video track at 2 s → "Create a generator" → picture groups, no Audio → click "Solid colour" → the event appears there
- **TEST** t1165 (`src/apps/kt-app/src/gen_audio_tests.rs`)

#### Scenario: Creation from the bin and placement on the timeline
- **WHEN** a kind is selected in the menu; a double click is made on the card
- **THEN** the media, the source, the card and the clip under the cursor appear in one step of history, the form is open; a double click gives one clip, not two
- **TEST** t477, t512, t514 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: An edit reaches the model and is rolled back
- **WHEN** a value is changed in the form; then undo; then 30 edits in a row
- **THEN** the model and the frame of the source are updated, undo returns the picture, the session is one step, zero restarts of the source
- **TEST** t478, t502 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Routing of the form and changing the kind
- **WHEN** the form for `gen:text` and for a solid is opened; the kind is changed in the picker
- **THEN** each kind gets its own form, a change of kind is its own step of history
- **TEST** t487, t511 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Percentages, reset, labels, colours
- **WHEN** a fraction of the frame is drawn in the form; a label receives a double click; parameters and flags are enumerated; a title with colours is opened
- **THEN** "7.0 %" is shown for a stored 0.07; the parameter is reset to the default of the spec; each parameter has a label, the flag has a control; the colours of the form equal the model
- **TEST** t516, t517, t542 (`src/apps/kt-app/src/layout_tests.rs`), t508, t509 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Safe zones and dragging in the preview
- **WHEN** a title is open; the block is dragged with the mouse in the preview
- **THEN** the 5 %/10 % safe zones are visible only while the title is open; the position is written by the same command and gesture — one undo step
- **TEST** t489, t495 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Card and event
- **WHEN** a generator lies in the bin without a file, stands in N places; an event is on the timeline
- **THEN** the thumbnail and probe are built without a file and without a DB, the kind "video without audio", the zero duration is hidden, "×N" is shown; the event is labelled with the name of the kind
- **TEST** t476 (`src/apps/kt-app/src/thumbs.rs`), t513, t515 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A sound kind's card is named, not black
- **WHEN** a sound generator lies in the bin
- **THEN** its probe is "audio without video", and its card writes the kind's name over the empty frame
- **PATH** Edit → "Create a generator" → Audio → "Audio noise" → the bin shows a card reading "Audio noise", not a black rectangle
- **TEST** t1166 (`src/apps/kt-app/src/gen_audio_tests.rs`)

#### Scenario: The generator entity in the "no dead ends" mechanism (see capability `ui-reach`)
- **WHEN** `TlEntity::Generator` appears with the verb "open parameters"
- **THEN** the verb table, the absence of orphaned commands and the panel's response to a right click cover the generator with its own row
- **TEST** t252, t253, t395 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Generator kinds are plugins; the pseudo-path names the plugin id
Each generator kind SHALL be a generator plugin of the closed built-in set (capability
`plugins`) whose id equals today's kind tag (`solid`, `text`, `gradient`, `corners`, `checker`,
`noise`, `smpte`, `timecode`, `transparent`, `rect`, `ellipse`, `line`, `leader`, `table`,
`window`; and the sound kinds `tone`, `audio_noise`, `sweep`, `beep`, `silence` of
`generators-placement-and-audio`), so `gen:<kind>` paths and the event's values keep their text. The generator menu SHALL
list the registry's generator rows (a plugin may declare several rows, as the test tables do
today) and MUST NOT enumerate kinds of its own. A generator plugin's frame SHALL be a pure
function of (the event's values with curves resolved at the source's tick, the tick, the
requested quality), and its in-place retune SHALL keep the existing strict contract: applied with
`info()` unchanged, or "recreate". A path naming an id this build does not register SHALL open as
a placeholder source that names the id — not as a broken path, and not through FFmpeg — and its
event's values SHALL be kept and written back unchanged.
Why: generators were the owner's first example of what should be developed in parallel
(ktprompts P3: "move generators and effects into plugins"); the frame-as-pure-function rule is
what makes the generator's cache bar honest (invariant 4), so it moves with the kind into the
manifest instead of being re-promised by each crate.

#### Scenario: A moved generator keeps its path, values and frame
- **WHEN** the generator families have moved into plugin crates (stage 5); a project with every
  menu row, with curves on two of them, is written by the frozen pre-move serializer and by the
  new build, and the generator frames at three ticks and two qualities are hashed
- **THEN** the texts and the `gen:` paths are byte-identical, the frame hashes equal the pre-move
  references (the title's own golden too), the menu offers the same 19 rows in the same order, followed by the five sound rows,
  and a retune of a colour is applied in place
- **TEST** t973 (`src/libs/kt-model/tests/plugin_format.rs`), t560 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A generator whose plugin is missing is a named placeholder
- **WHEN** a project holds media `gen:wobble_grid` with an event carrying values for it, and no
  plugin `wobble_grid` is registered; the project is opened, played and saved
- **THEN** the factory raises a placeholder source labelled with the id, FFmpeg and the sidecar DB
  are not touched, one notice names the id, and the saved file keeps the path and the values
- **TEST** t973b (`src/libs/kt-media/src/factory.rs`)
