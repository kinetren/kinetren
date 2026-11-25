## ADDED Requirements

### Requirement: On a card the export composes on the compute host's device
When an export composes on one card device, that device SHALL be the compute host's (the shared device
where one device per adapter holds, or the compute host's own in a headless run), so a stream stage's frames
reach the composite held, not read back. On a software adapter the export SHALL keep its several devices.
The frames SHALL equal those of the two-device export.

Why: found-defects 663 — the export's stream frames crossed devices like the preview's
(stream-picture-stays D4).

#### Scenario: A card's export crosses nothing and writes the same frames
- **WHEN** a project with Datamosh is exported with the export engine on the compute host's device (the host holding pictures), and again on a device of its own; and the decision is asked for a card, a software adapter, a host not holding pictures and `KT_VIDEO_ENGINES`
- **THEN** the first crosses no stream frame (no `stream_cross` record), the second reads each held picture back, and the two exports' raw frames are equal; only the card holding pictures composes on the host's device (kt-cli asks the host to hold them on a card, `hold_pictures_for_export`)
- **TEST** t3820 (`src/libs/kt-engine/tests/stream_stage.rs`), t3818 (`src/libs/kt-gpu/tests/compute_shared.rs`)
