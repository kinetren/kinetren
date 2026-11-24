## ADDED Requirements

### Requirement: The project's program stage normalizes the mix and holds the true-peak ceiling
A document MAY have a program stage — a gain and a true-peak limiter at the project's ceiling; with none, the output
SHALL be byte for byte what it was. The stage SHALL stand at the OUTPUT of the mix: after the kept mix and the nested
reels, before the player's monitoring gain, in the forward play and in the export alike; its parameters SHALL NOT enter
the scene hash or drop a kept burst. It SHALL be fed the mix its look-ahead early, primed after a seek, through the
loop's wrap. A reel opened for editing, a reel's envelope, the golden stream, the analysis jobs and the probe SHALL run
no stage; a reverse play and a paused level SHALL apply the gain and a clamp at the ceiling. Its detector SHALL be the
meter's own interpolator. The monitoring volume SHALL stay outside the project and every measurement.
Why: the owner, kkt-26.11.20b B6 — "это должно быть на уровне проекта нормализация … проффесиональный путь". A stage
inside the renderer would drop every kept burst at each change of its gain.

#### Scenario: Match the program, then edit
- **WHEN** a mix measured at −19.5 LUFS with peaks at +1 dBTP is matched to −14 LUFS with a −1 dBTP ceiling, exported, then an event is made 6 dB louder and later a title on a video track is moved
- **THEN** the export measures −14.0 LUFS (±0.2) with the true peak ≤ −1 dBTP and the gain is at least +5.5 dB; after the louder event the stage reports "changed since measured" with its gain unchanged; moving a title alone reports nothing; the forward play and the export from the same start are byte-equal
- **TEST** t3919 (`src/libs/kt-engine/tests/program_loudness.rs`), t3919b (`src/libs/kt-engine/tests/program_player.rs`: the play against the export)

#### Scenario: No stage, no change
- **WHEN** the golden audio project is rendered with no program stage, with one at 0 dB over a mix whose true peak is below the inner ceiling, and the latter cut in pieces of 1/1024/7919 frames
- **THEN** the first equals the golden bytes; the second equals the first sample for sample; the slicing changes nothing
- **TEST** t4200 (`src/libs/kt-engine/tests/program_loudness.rs`)

#### Scenario: The ceiling holds for peaks between samples
- **WHEN** a sine at a quarter of the sampling frequency with a 45° phase, a full-scale square, isolated clicks, a burst out of silence and noise at +12 dB run through the stage at a −1 dBTP ceiling, in pieces of 1/1024/7919 frames and on the baseline path
- **THEN** the meter reads at most −1.0 dBTP for each and the pieces and the path change no bit; a signal under the inner ceiling passes unchanged, and after a held peak the release ends on exactly the signal; a gain change makes no step
- **TEST** t4201 (`src/libs/kt-fx/tests/program.rs`)

#### Scenario: A gain change keeps the kept mix and the loop
- **WHEN** a covered loop plays from the kept mix through a stage and the stage's gain is changed by 3 dB, the stage taken away and given back while it plays
- **THEN** no burst is dropped, no chunk renders live, the output rises by 3 dB over the smoothing time with no step at the wrap above the material's own
- **TEST** t4202 (`src/libs/kt-engine/tests/program_player.rs`)

### Requirement: Clips are matched to a target, short clips measured together
"Match loudness" SHALL set each chosen event's gain so that its own integrated loudness through its chain reaches the
chosen target; events shorter than 3 s on one track from one source SHALL be measured as one material and get one
correction; an event with no loudness SHALL be left and named; one whose level ends at its largest (+6.02 dB) short of the target SHALL be named; the whole match SHALL be one undo step of ordinary gains.
Why: Premiere's Auto-Match and Resolve's Normalize Audio Levels; Premiere's flaw with short clips.

#### Scenario: Dialogue lines of different loudness and a short one
- **WHEN** five dialogue events of different loudness, two 1 s lines 10 dB apart of one track and source and an event of silence are matched to −14, and a music event to −22
- **THEN** each long line measures −14.0 (±0.3), the music −22.0 (±0.3); the two short lines take one correction, neither its own; the silence is left and named; one Undo restores every gain
- **TEST** t3920 (`src/libs/kt-engine/tests/program_loudness.rs`)

#### Scenario: The measuring jobs yield to playback and to edits
- **WHEN** "Match the program" runs, Play starts and stops, then an event's level changes before the job ends
- **THEN** the job stands while playing and goes on after; its result is dropped with the notice that the sound changed; no gain is written
- **TEST** t4203 (`src/apps/kt-app/src/program_tests.rs`)
