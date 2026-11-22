## ADDED Requirements

### Requirement: The project's program stage normalizes the mix and holds the true-peak ceiling
The mix SHALL end in the project's program stage — a gain and a true-peak limiter at the project's ceiling — in the
player's render and in the export alike; "Match the program" SHALL measure the integrated loudness of the whole mix
without the stage on a background pass and set the gain to reach the project's target, recording the measured value and
the revision; after an edit the stage SHALL report that it changed since measured; the player's monitoring volume SHALL
stay outside the project and every measurement.
Why: the owner, kkt-26.11.20b B6 — "это должно быть на уровне проекта нормализация … проффесиональный путь".

#### Scenario: Match the program, then edit
- **WHEN** a mix measured at −19.5 LUFS with peaks at +1 dBTP is matched to a −14 LUFS target with a −1 dBTP ceiling, exported, then an event is made 6 dB louder
- **THEN** the gain is +5.5 dB; the export measures −14.0 LUFS (±0.2) with the true peak ≤ −1 dBTP; after the edit the stage reports "changed since measured" and its gain is unchanged; the preview and the export of the same range are byte-equal in audio
- **TEST** t3919 (`src/libs/kt-engine/tests/program_loudness.rs`)

### Requirement: Clips are matched to the project target by role, short clips measured together
"Match loudness" SHALL set each chosen event's gain so that its own integrated loudness through its chain reaches the
project target plus its role's offset; events shorter than 3 s on one track from one source SHALL be measured as one
material and get one correction; an event with no loudness SHALL be left and named; the whole match SHALL be one undo
step of ordinary gains.
Why: Premiere's Auto-Match and Resolve's Normalize Audio Levels; Premiere's flaw with short clips.

#### Scenario: Dialogue lines of different loudness and a short one
- **WHEN** five dialogue events at −30, −22, −18, −26 and a 1 s line at −16 LUFS, and a music event, are matched with the target −14 and the music offset −8
- **THEN** each long line measures −14.0 (±0.3), the music −22.0 (±0.3); the 1 s line takes the correction of its group with its neighbours, not +2 dB of its own; one Undo restores every gain
- **TEST** t3920 (`src/libs/kt-engine/tests/program_loudness.rs`)
