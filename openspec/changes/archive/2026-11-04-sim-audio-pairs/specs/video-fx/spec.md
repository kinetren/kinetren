## ADDED Requirements

### Requirement: A simulation of a device that records sound names its audio half
Every simulation video kind whose device records sound (`vhs_tape`, `datamosh`, `camera_isp`) SHALL
declare its audio pair, and its sound rows SHALL stand at the end of the passport, never read by the
picture: the picture keeps its bytes and a file without them keeps its text. A new simulation effect
of such a device is not done until its pair exists, or its note says why the device has no sound.
Why: kkt-26.11.03b D1 — "again the agents did not think that VHS Tape must have a paired audio filter,
Datamosh too"; the root CLAUDE.md rule written with it.

#### Scenario: Every built-in simulation of a sound-recording device is paired
- **WHEN** the built-in set is assembled
- **THEN** `vhs_tape` names `vhs_tape_sound`, `datamosh` names `datamosh_sound`, `camera_isp` names `camera_isp_sound`, and no declared pair is missing (the assembler refuses otherwise)
- **TEST** t1460 (`src/libs/kt-plugin-api/tests/pairs.rs`), the assembler's check in `kt_plugins_builtin::builtin`
