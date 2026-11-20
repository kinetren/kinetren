## ADDED Requirements

### Requirement: The moving backgrounds
The Background group SHALL offer a moving gradient, plasma, particles and a pattern (stripes, dots, a grid
or waves). Each SHALL move by itself with the material time at its speed, draw the same frame for the
same moment, be opaque, and take its colours and numbers into the picture. Every row SHALL be captioned
in all five catalogs.
Why: `v26.10` G5 — the owner's backgrounds group: "moving gradients, particles, plasma, patterns", not
like in the 90s.

#### Scenario: Each moves, repeats and obeys its rows
- **WHEN** each background at speed 0.5 is drawn at 0 and 1 s and at 0 again; the gradient and the plasma with three red colours; the pattern with each choice; the particles with 4 and 200 discs
- **THEN** each is opaque, differs between 0 and 1 s and repeats at 0; the red colours give a red frame; the four patterns differ; 200 particles light more of the frame than 4
- **PATH** Edit → Create a generator… → Background → Plasma → press Play → the plasma flows; its window → the colours and the speed change it
- **TEST** t3707 (`src/libs/kt-media/tests/gen_backgrounds.rs`), live scenario `src/scripts/live/gen_backgrounds.py` (`./scripts/live-check.sh`)

#### Scenario: Reached from Edit and captioned
- **WHEN** each is created from Edit › Create a generator › Background; the catalogs are read
- **THEN** each is one picture event of its kind; every number, flag and choice row, every colour and the pattern's four choices are captioned in all five languages
- **PATH** Edit → Create a generator… → Background → Moving gradient / Plasma / Particles / Pattern
- **TEST** t3708 (`src/apps/kt-app/src/gen_audio_tests.rs`)
