## ADDED Requirements

### Requirement: An effect reads the speed of the time map that plays it
A link SHALL be able to read the outer speed of its owner — the product of the rates of the time maps that play it (its
event's warp for the event's effects, the covering morpher layer's warp, the warp of every enclosing reel instance) —
its acceleration per timeline second, and a speed-change value (a peak-hold of the absolute acceleration with an attack
and a linear release), each a pure function of the edit at the moment: the same in playback, scrubbing, a seek, a loop
and an export.
Why: the owner, v26.10 lines 160–164 and kkt-26.11.20b B7 — "Скорость монтажа"; "не хочется руками вылавливать и
подстраиваться под уже существующие кривые тормажения-скольжения которые находятся у другого события".

#### Scenario: The fisheye wakes on the instance's braking
- **WHEN** a Lens strength inside a reel follows the outer speed change, the instance's speed goes 1 → 0.3 over 1 s and back; the range is rendered, then sought into the release and rendered again
- **THEN** the strength is 0 while the speed is steady, rises during the change and falls linearly over the release; each value equals the closed form; the seek render equals the continuous one byte for byte; the preview equals the export
- **TEST** t3947 (`src/libs/kt-model/tests/motion_links.rs`), t3948 (`src/libs/kt-engine/tests/motion_links.rs`)

#### Scenario: Nested maps multiply
- **WHEN** an event at speed 2 sits in a reel whose instance plays at 0.5 inside another instance braking from 1 to 0.5
- **THEN** the outer speed is 2 · 0.5 · r(t) at every tick, and its acceleration follows the product rule
- **TEST** t3949 (`src/libs/kt-model/tests/motion_links.rs`)
