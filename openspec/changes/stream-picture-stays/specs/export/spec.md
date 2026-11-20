## ADDED Requirements

### Requirement: On a card the export composes on the compute host's device
When an export composes on one card device, that device SHALL be the compute host's (the shared device
where one device per adapter holds, or the compute host's own in a headless run), so a stream stage's frames
reach the composite held, not read back. On a software adapter the export SHALL keep its several devices.
The frames SHALL equal those of the two-device export.

Why: found-defects 663 — the export's stream frames crossed devices like the preview's
(stream-picture-stays D4).

#### Scenario: A card's export crosses nothing and writes the same frames
- **WHEN** a project with Datamosh is exported on a card with the export engine on the compute host's device, and again on a device of its own
- **THEN** the first crosses no stream frame and the two exports' raw frames are equal
- **TEST** t3820 (`src/libs/kt-gpu/tests/compute_shared.rs`)
