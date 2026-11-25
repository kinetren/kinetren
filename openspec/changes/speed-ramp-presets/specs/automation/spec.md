# Spec Delta

## ADDED Requirements

### Requirement: Speed ramps keep the event's material
The speed row SHALL offer six ramps; applying one SHALL write its curve over the event and SHALL keep the material span the
event reads, so its first and last frames and its neighbours stay; it SHALL be one undo step and an ordinary editable
curve; a ramp that cannot keep the span SHALL be refused with the reason.
Why: the owner's K7; a ramp that moved the last frame would break the edit around it.

#### Scenario: The span holds for every ramp
- **WHEN** each ramp is written on a 3 s event at 1×, at 0.5× and reversed, and on one with a later cut
- **THEN** the integral of the curve over the event equals the former mean × length within one tick; the last frame is the same; the keys lie at the event's fractions
- **TEST** t4284 (`src/libs/kt-model/tests/speed_ramp.rs`)

#### Scenario: A ramp past the limit is refused
- **WHEN** Bullet is asked for an event whose mean is 20×
- **THEN** it is refused with the reason and nothing is written
- **TEST** t4285 (`src/libs/kt-model/tests/speed_ramp.rs`)

#### Scenario: A person ramps an event without a speed effect
- **WHEN** they choose Hero from the event's menu
- **THEN** the event gains the speed slot with the curve and one Ctrl+Z removes both
- **PATH** right-click an event → "Speed ramp" → "Hero" → the event shows its curve and the preview slows in the middle; play to the end → the last frame is the same as before; Ctrl+Z → the event has no speed effect
- **TEST** t4286 (`src/apps/kt-app/src/reach_tests.rs`)
