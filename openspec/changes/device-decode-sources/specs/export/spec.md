## ADDED Requirements

### Requirement: The export's bytes do not depend on where its sources decode
An export SHALL write the same bytes whether its sources decode on the device or on the processor, and on
one device from run to run whatever fitted the surfaces' bound. Its stage profile SHALL state the pictures decoded
on the device and on the processor and the transfer time.
Why: invariant 11; device-decode-sources D6 — the device picture is the processor's picture (media-index),
so the choice must be invisible in a render.

#### Scenario: Device and processor exports are equal
- **WHEN** a two-second project over a fake-driver file is exported with the device, with `KT_DECODE_DEVICE=off`, and with the bound refusing every pool
- **THEN** the three outputs are byte-for-byte equal, and the stage profile names the device's pictures in the first and none in the other two
- **TEST** t3307 (`src/libs/kt-engine/tests/export_options.rs`)
