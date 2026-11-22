## ADDED Requirements

### Requirement: Auto-ducking follows the owner's sections — content keyed by several tracks, titles from the edit
An audio track's Auto-ducking SHALL have two rules: during content, the track goes down by the set amount under the sum
of every event on the ticked key tracks and of every event of the ticked roles (the keyed track excluded), through the
one compressor; during titles, the track is raised by the set amount, or muted, or left, by a gain envelope computed
from the title spans (Text generator events and events tagged "Title") with fades, a pure function of the edit; a track
set to play only in titles SHALL be unmuted inside title spans and muted outside. The person's own level keys on the
track's events SHALL add to the automatic gain in dB.
Why: the owner, v26.10 lines 84–88 — "во время контента звук контента и задний звук, а во время заголовка … либо
автоматом усиление заднего звука или мут заднего звука и размут заголовочного звука"; B5 — several tracks, hand edits.

#### Scenario: Intro, title, content, title, content
- **WHEN** a background music track has content ducking −12 dB keyed by two speech tracks and "Raise +6 dB" during titles; the edit runs intro, title 1 (a Text event), content (speech on both tracks), title 2, content 2; the project is rendered, a title is moved 3 s, and rendered again
- **THEN** under speech on either track the music is 12 dB down (±0.2), in title spans it is +6 dB (±0.2) with fades, in the intro it is unchanged; after the move the raised span moves with the title; seek renders equal continuous renders byte for byte
- **TEST** t3911 (`src/libs/kt-engine/tests/ducking_sections.rs`)

#### Scenario: Mute the background, unmute the title sound
- **WHEN** the rule is "Mute" and a stinger track is set to play only in titles
- **THEN** inside title spans the background is silent and the stinger plays; outside them the stinger is silent
- **TEST** t3912 (`src/libs/kt-engine/tests/ducking_sections.rs`)

#### Scenario: A hand key survives the automation and a re-edit
- **WHEN** the person lowers the music by 4 dB at one spot with a level key, then moves a speech event elsewhere
- **THEN** at that spot the music is the automatic gain −4 dB, before and after the move; elsewhere the automation alone
- **TEST** t3913 (`src/libs/kt-engine/tests/ducking_sections.rs`)

#### Scenario: A key cycle is refused by name
- **WHEN** a track keys on itself through a role its own events carry, and two tracks key on each other
- **THEN** both commands are refused with `SidechainCycle` naming the track; the project is unchanged
- **TEST** t3914 (`src/libs/kt-model/tests/roles.rs`)
