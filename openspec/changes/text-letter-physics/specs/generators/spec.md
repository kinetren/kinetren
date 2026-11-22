## ADDED Requirements

### Requirement: A title's letters can be bodies driven by the edit's speed
A title's Physics animator SHALL treat each glyph as a body tied to its laid-out place and driven by the acceleration of
that place under the title's outer time map (its own warp, a morpher layer's warp and the enclosing instances' speeds),
with the behaviours sway, hang, drag and fall; the state at a frame SHALL be integrated from the event's start with a fixed
step of material time using the frozen math, the same bytes on every machine, with or without checkpoints; the result
SHALL reach the title as per-glyph offsets and rotations, drawn by the title's own code.
Why: the owner, v26.10 lines 109–111 — "если управлять например rate таким событием, например, через slowed&reverb - то
можно было бы добавить инерцию буквам"; F-FX-005 (R.59): simulation is a function of the edit.

#### Scenario: A crawl braked by Slowed&Reverb shakes its letters
- **WHEN** a crawling title at constant speed is rendered, then its Slowed&Reverb rate drops from 1 to 0.4 over 0.5 s and back; the range is rendered twice, once continuously and once seeking into the middle
- **THEN** at constant speed every glyph sits at its home; during the braking glyphs move ahead along the crawl and settle within the damping time; the seek render equals the continuous one byte for byte; the frames are equal with checkpoints cleared
- **TEST** t3968 (`src/libs/kt-text/tests/letter_physics.rs`), t3969 (`src/libs/kt-engine/tests/letter_physics.rs`)

#### Scenario: The motion does not depend on the frame rate
- **WHEN** the same edit is rendered in a 25 fps and a 30 fps project
- **THEN** at the same material moments the glyph offsets are equal (within interpolation of 1/240 s)
- **TEST** t3970 (`src/libs/kt-text/tests/letter_physics.rs`)

#### Scenario: Letters fall at a jolt and stay fallen
- **WHEN** the behaviour is fall with a threshold and the instance holding the title brakes hard
- **THEN** glyphs whose jolt passes the threshold detach, fall and rest on the frame's bottom; the others sway back
- **TEST** t3971 (`src/libs/kt-text/tests/letter_physics.rs`)
