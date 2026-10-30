# Spec Delta

## ADDED Requirements

### Requirement: An event gain at the floor is exact silence
An event's gain at the floor of its range SHALL multiply the event's samples by exactly zero. "Almost
silence" is not allowed. Every gain above the floor SHALL keep today's dB-to-linear mapping, byte for
byte (invariant 11).
Why: the owner asked for "gain down to mute" (`ktprompts_v26.10` USABILITY 6). A floor at −60 dB is
audible on loud material and is not what "mute" means.

#### Scenario: The floor renders zeros, the rest is unchanged
- **WHEN** an event with a 0 dBFS sine is rendered at the gain floor, at −6 dB and at 0 dB
- **THEN** the floor gives all-zero samples; −6 dB and 0 dB are byte-equal to the render before this change
- **TEST** t1011 (`src/libs/kt-engine/tests/clip_gain_floor.rs`)
