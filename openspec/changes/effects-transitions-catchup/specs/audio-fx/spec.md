# Spec Delta

## ADDED Requirements

### Requirement: Audio catch-up — presets over existing nodes before new nodes
The audio part of the catch-up (wave W7 of F-FX-007, "voice presets": telephone, megaphone, radio,
robot, chipmunk/deep) SHALL be delivered first as chain presets and built-in variants over the
existing audio kinds; a new audio node SHALL be added only where no chain of existing nodes
expresses the sound (today: a ring modulator for "robot"), as an audio plugin with declared state,
tail and look-ahead (capability `plugins`) that falls under the parameterized harness without a
test of its own. The note SHALL list every built-in audio id.
Why: CapCut's voice changers are presets of filters, pitch and distortion; we have every one of
those nodes and the preset library that carries curves (F-AUT-004). A node per preset would grow
the harness and the catalogs for sounds the chain already makes.

#### Scenario: The audio list matches and the wave declares what is new
- **WHEN** the guard compares the audio list of F-FX-007 with the audio kinds of the registry and
  reads wave W7 of the change design
- **THEN** the list equals the registry (18 ids today); W7 names its presets by the existing ids
  they chain and names exactly the nodes it adds, each with the family crate it lives in
- **TEST** t978 (`src/apps/kt-cli/tests/fx_catalogue.rs`), t982 (`src/apps/kt-cli/tests/fx_catalogue.rs`)
