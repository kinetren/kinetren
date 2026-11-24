## ADDED Requirements

### Requirement: A pause is what does not rise above the recording's own floor
Pauses SHALL be found by sound alone, without a recognizer. The floor SHALL be learned from the recording itself, per
speech band and moment (a low percentile over a sliding window), and a moment SHALL be sound when its bands rise above
their floor by the sensitivity, with hysteresis; sounds shorter than a given length SHALL be absorbed, pauses shorter than
a given length dropped, and the margins before and after speech kept. The decision SHALL be a pure function of the file's
series and peaks, the same on every machine. The file SHALL be judged, not the event after its effects.
Why: the owner, v26.10 line 89 — "по звуку можно тоже анализ пауз сделать отдельно, но надо учесть и про шумное аудио".

#### Scenario: Clean speech
- **WHEN** pauses of at least 400 ms are asked on a fixture of synthetic speech with two gaps of 600 ms and one of 200 ms
- **THEN** exactly the two long gaps are found, each edge within 10 ms after the margins
- **TEST** t3466 (`src/libs/kt-media/tests/pauses.rs`)

#### Scenario: The same speech under noise
- **WHEN** pink noise 10 dB under the speech is added, then noise whose level rises by 12 dB in the middle, then a 100 Hz hum louder than the speech
- **THEN** the same two pauses are found in each, edges within 40 ms, with the default rows; a fixed threshold that finds the clean fixture's pauses finds none or all in each (the check of the fixture)
- **TEST** t4205 (`src/libs/kt-media/tests/pauses.rs`)

#### Scenario: Breaths, merging and music
- **WHEN** a 90 ms click stands inside a 900 ms pause; the same file is asked with the merging row at 50 ms; a file with constant music under the speech is asked
- **THEN** one pause, then two; the music file answers "never falls to its floor" and finds nothing
- **TEST** t4206 (`src/libs/kt-media/tests/pauses.rs`)

#### Scenario: Two speakers, a reversed and a slowed event
- **WHEN** two tracks are chosen whose speakers alternate, one of the events reversed and one at half speed
- **THEN** only the moments where both are silent are pauses, mapped to the timeline within one frame; "each track apart" gives each track's own
- **TEST** t4207 (`src/libs/kt-media/tests/pauses.rs`)
