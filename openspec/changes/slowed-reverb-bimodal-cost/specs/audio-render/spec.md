## ADDED Requirements

### Requirement: A chain's cost does not depend on the process run
The render of the same chain over the same input SHALL cost the same class of time in every process run of the same build: no effect's cost may depend on where the allocator placed its buffers, or on floating-point state the engine does not set. When the cost depends on the machine (for example, the core type the render thread lands on), the engine does not pin threads; the requirement then records it as a property of the machine, with the experiment that reproduces it. The fix of a placement or number-format cause SHALL keep the sound byte for byte.
Why: one Slowed+Reverb slot cost ~104 ms or ~415 ms per 10 s at random from one run to the next on identical code and input. That is a 4× cost a person pays by chance, and on a loop it decides whether the audio render keeps ahead of the callback (invariant 6).

#### Scenario: The cause of the slow mode is gone and stays gone
- **WHEN** the reverb is built, renders a long decaying tail over a click stimulus, and its state is inspected
- **THEN** the property the investigation found to cause the slow mode does not occur: no two comb/allpass line heads alias within 4 KiB when the cause is placement, or the reverb state holds no subnormal values when the cause is number format. A release measurement printed by the test's `--ignored` twin shows one mode over repeated process runs
- **TEST** t991 (`src/libs/kt-engine/tests/cost.rs`)

#### Scenario: Removing the slow mode does not move the sound
- **WHEN** the fix is in, and the audio references and the per-kind sound table run
- **THEN** `GOLDEN_AUDIO`, every row of the per-kind table and the reverb tail formula are unchanged
- **TEST** t11 (`src/libs/kt-engine/tests/golden_export.rs`), t973c (`src/libs/kt-engine/tests/plugin_sound.rs`), t6 (`src/libs/kt-engine/tests/fx.rs`)
