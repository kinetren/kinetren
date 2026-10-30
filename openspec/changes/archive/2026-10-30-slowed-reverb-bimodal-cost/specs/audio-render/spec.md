## ADDED Requirements

### Requirement: A chain's cost does not depend on the process run
The render of the same chain over the same input SHALL cost the same class of time in every process run of the same build: no effect's cost, and no source's cost, may depend on where the allocator placed its buffers, on floating-point state the engine does not set, or on bytes a value does not use (the payload a dataless enum variant leaves from its stack frame). When the cost depends on the machine (for example, the core type the render thread lands on), the engine does not pin threads; the requirement then records it as a property of the machine, with the experiment that reproduces it. The fix of a placement, number-format or unused-bytes cause SHALL keep the sound byte for byte.
Why: one Slowed+Reverb slot cost ~104 ms or ~415 ms per 10 s at random from one run to the next on identical code and input. That is a 4× cost a person pays by chance, and on a loop it decides whether the audio render keeps ahead of the callback (invariant 6). The investigation found it in the synthetic source the measurement read: matched per sample, its `Sine` arm's `sin()` was hoisted out of the match and ran for `Clicks` on leftover payload bytes, a subnormal in the slow runs (design, Finding).

#### Scenario: The cause of the slow mode is gone and stays gone
- **WHEN** a `Clicks` synthetic source is built with a subnormal (6.77e-310, the value observed in the slow runs) in its pattern's unused bytes, and 2000 bursts are read from it and from the same source with clean bytes, interleaved, best of 5
- **THEN** both give the same bytes, and the dirty source costs less than 2× the clean one (before the fix: 6.8× in release, 5.5× in the dev profile). A release measurement printed by the test's `--ignored` twin shows the lone Slowed+Reverb slot at one cost with a clean and with a dirty payload, and one mode over repeated process runs
- **TEST** t991 (`src/libs/kt-engine/tests/cost.rs`)

#### Scenario: Removing the slow mode does not move the sound
- **WHEN** the fix is in, and the audio references and the per-kind sound table run
- **THEN** `GOLDEN_AUDIO`, every row of the per-kind table and the reverb tail formula are unchanged
- **TEST** t11 (`src/libs/kt-engine/tests/golden_export.rs`), t973c (`src/libs/kt-engine/tests/plugin_sound.rs`), t6 (`src/libs/kt-engine/tests/fx.rs`)
