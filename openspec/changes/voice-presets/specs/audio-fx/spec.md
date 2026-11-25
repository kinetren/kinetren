# Spec Delta

## ADDED Requirements

### Requirement: Ring modulation is a function of the position
The audio plugin `ring_mod` SHALL multiply the sound by a sine whose phase is the integral of its frequency curve over
the event's position; at mix 0 the output SHALL be the input byte for byte.
Why: a carrier kept as a counter would differ after a seek (invariant 11; the LFO rule).

#### Scenario: A seek and a cold render agree
- **WHEN** `ring_mod` with a frequency curve 30→200 Hz renders 4 s cold, and 2–4 s after a seek, and at mix 0
- **THEN** the overlapping samples are equal, the last is the input
- **TEST** t4279 (`src/plugins/fx-audio-modulation/tests/ring_mod.rs`)

### Requirement: Voices are chain presets
Telephone, Megaphone, Radio, Chipmunk, Deep voice, Small hall and Robot SHALL be factory chain presets over registered
kinds; applying one SHALL be one undo step and SHALL set the whole chain.
Why: the owner's K7; every voice is nodes we have but the robot's.

#### Scenario: The presets apply from the picker
- **WHEN** the person opens the picker on a voice event and clicks "Telephone"
- **THEN** the chain is the preset's nodes, one Ctrl+Z restores the former chain
- **PATH** click "+ Effect" on an audio event → type "phone" → the "Telephone" row stands under the presets; click it → the graph shows the nodes and the voice sounds narrow; Ctrl+Z → the chain is as it was
- **TEST** t4280 (`src/apps/kt-app/src/user_store.rs`)
