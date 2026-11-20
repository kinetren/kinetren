## ADDED Requirements

### Requirement: The trace says who decoded each picture
Every `Decoded` record SHALL carry its decoder's kind (processor or device). Every device decoder's open
or refusal SHALL be a record with the file, the device, the pool's bytes and the reason of a refusal
(ledger, flavour, crop, proof, fault). The transfer time of device pictures SHALL be summed per source.
`kt-cli trace summary` SHALL print the pictures decoded on the device with the transfer's p50, those on the
processor and why. `kt-cli probe <project> <from> <to> --decode-check` SHALL decode the range's pictures
both ways on this machine and count those that differ.
Why: owner, 2026-11-09 — diagnose from the telemetry of his run; the gain of the device depends on his
chip's transfer cost, which this VM cannot measure (device-decode-sources D7, Q1).

#### Scenario: `trace summary` reads the decode split
- **WHEN** a run holds forty device pictures with transfer times, ten processor pictures, a refusal by the ledger and one by the proof
- **THEN** `trace summary` prints "40 on the device", the transfer p50, "10 on the processor" and both reasons
- **TEST** t3308 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: The decode check counts differing pictures
- **WHEN** `probe --decode-check` runs over a fake-driver file, and again with the driver told to change samples after the first keyframe
- **THEN** the first reports zero differing pictures, the second the pictures after the keyframe
- **TEST** t3311 (`src/apps/kt-cli/tests/cli.rs`)
