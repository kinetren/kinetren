## ADDED Requirements

### Requirement: A generator kind may synthesize its sound; there is one sine text
A generator kernel SHALL either declare no sound, a level with an optional gate played by the
host's sine (`tone`/`gate`), or a synthesizer (`Synth`) that fills one interleaved stereo burst
of `AUDIO_BURST_FRAMES` frames from the event's values and the GLOBAL sample number of its first
frame; a synthesizing kind has no picture. The registry SHALL refuse, by id, a kind that declares
both a synthesizer and a tone. The sine and the dBFS-to-amplitude conversion SHALL live once, in
`kt_plugin_api::generator` (`sine`, `db_amp`), called by the host's tone and by every kernel that
plays a sine, through the frozen math (`kt_plugin_api::math`). The built-in set SHALL include the
`generators-audio` family after `generators-test`.
Why: a level and an on/off cannot express a frequency, a noise colour or a sweep; two sine texts
would drift in phase and in bytes (the rule of R.53), and a platform `sin` would differ across
machines (t1066).

#### Scenario: Two answers to "what does it sound like" are refused
- **WHEN** a generator kind declaring both a synthesizer and a tone is registered; then a kind with a synthesizer only
- **THEN** the first is refused with an error naming its id and the conflict, the second is registered and its kernel reports the synthesizer
- **TEST** t1162 (`src/libs/kt-plugin-api/tests/registry.rs`)

#### Scenario: The host's tone and the tone kind share the sine
- **WHEN** the SMPTE tone (host `tone_1k`) and the `tone` kind at 1 kHz read the same bursts at the same level
- **THEN** the bytes are equal
- **TEST** t1155 (`src/plugins/generators-audio/tests/sound.rs`)
