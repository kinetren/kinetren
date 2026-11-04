## ADDED Requirements

### Requirement: A software adapter composes an export on several devices
An export on a software adapter SHALL compose on a device per frame worker, a quarter of the cores and
2 to 8 of them, and one device SHALL serve a real card. The frames written SHALL NOT depend on how many
devices composed them.
Why: llvmpipe runs one frame per queue; the owner's machine has no card, and the render there must
not pay for the CPU composite's deletion.

#### Scenario: Several devices write the frames of one
- **WHEN** one project is exported through one engine and through three, on the same adapter
- **THEN** the frames written are byte for byte equal
- **TEST** t1573 (`src/libs/kt-gpu/tests/export_ring.rs`)
