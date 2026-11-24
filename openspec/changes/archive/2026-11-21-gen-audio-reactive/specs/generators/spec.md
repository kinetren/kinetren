## ADDED Requirements

### Requirement: An audio-reactive generator listens to a track's files
The Waveform, Spectrum and Level rings generators SHALL draw what the audio track named by their
"Hears track" row sounds like, from an analysis series of each FILE on that track. The series has one
record per audio burst (the RMS of each channel, the peak, 16 log bands and 32 waveform points) and
SHALL be a pure function of the file's decoded sound. It SHALL be computed in the background from a
short-lived source of the job's own, kept behind one store seam (today the sidecar, cleared when the
file changes), and keyed by the file. The generator's state SHALL carry its FEED, built by
`gen_sources`: the pieces of series it hears, in its own material time. A series that arrives, a moved
or replaced event, or another track SHALL retune the source and move the frame's keys (`src_rev`, the
event memo). A render SHALL compute every missing series before its first frame. A frame SHALL read the
values at its moment and the same values held over the last half second, as a fold over the series, so
the picture is a function of the time alone. The visualizer SHALL hear the raw file at the event's
plain material time, not the mix after effects (owner question Q55).
Why: `v26.10` G5, the audio-reactive group; the orchestrator's choice of option (a), 2026-11-20.

#### Scenario: The series is pure and kept by the file
- **WHEN** a 2 s 1 kHz sine is analysed twice, stored in a sidecar, read back, and the file changes
- **THEN** one record per burst, the same bytes twice, the level of a −9 dBFS RMS sine, the store gives the same hash without opening the file, a changed file has none, a stop keeps nothing
- **TEST** t3714 (`src/libs/kt-media/tests/listen.rs`)

#### Scenario: The feed follows the track it hears
- **WHEN** a waveform on V1 over 0..8 s hears A1, where a music event lies at 2..6 s from its 1 s; the series becomes known; the event moves; the waveform hears track 2
- **THEN** no series — no piece; then one piece 2..6 s heard at the file's 1..5 s; the piece and the feed's key follow the event; track 2 hears nothing
- **TEST** t3715 (`src/libs/kt-engine/tests/listen.rs`)

#### Scenario: A reactive frame follows the sound and only it
- **WHEN** each reactive kind hears a sine for 0..2 s and is drawn at 1 s and at 3 s
- **THEN** the two frames differ, the same moment gives the same bytes, the spectrum and the rings light more while it sounds, and a kind hearing nothing draws the silent frame
- **TEST** t3716 (`src/libs/kt-media/tests/gen_reactive.rs`)

#### Scenario: The reactive kinds are created and captioned
- **WHEN** Waveform, Spectrum and Level rings are picked in a new generator's Kind gallery
- **THEN** each is one picture event in the group "Reacts to sound", its window shows "Hears track"; every row, flag, choice and colour is captioned in five languages
- **PATH** Edit → "Create a generator…" → its window → Kind → Reacts to sound → Waveform / Spectrum / Level rings → a picture event at the cursor, its window with "Hears track 1" first
- **TEST** t3718 (`src/apps/kt-app/src/gen_audio_tests.rs`), t1163 (`src/apps/kt-app/src/gen_audio_tests.rs`), live scenario `src/scripts/live/gen_reactive.py` (`./scripts/live-check.sh`)

#### Scenario: A render computes what its generators hear first
- **WHEN** a render prepares a project whose waveform hears a file the registry lacks; then again; then a stopped one; then a missing file
- **THEN** the series is computed once from a source of its own and stored, the generator hears it, the second preparation opens nothing, the stopped one keeps nothing, the missing file is silence
- **TEST** t3719 (`src/libs/kt-engine/tests/listen.rs`)
