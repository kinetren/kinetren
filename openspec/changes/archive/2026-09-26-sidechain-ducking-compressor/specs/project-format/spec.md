# Spec Delta

## ADDED Requirements

### Requirement: Compressor key input in the file — v12 by content
The sidechain key SHALL be written in the compressor's parameters as the address of a TRACK (`TrackId`), not as its number: the number is the row rank, and inserting a track above would reassign the key to someone else's signal. A compressor WITHOUT a key MUST NOT write the field at all — a project file without a sidechain remains byte for byte the same and keeps the previous envelope version. A file carrying at least one assigned key SHALL raise the envelope version to v12: an old build would silently lose the key and would sound different, and the version is the only honest way to tell it so.
Reading SHALL be tolerant: a key pointing at a track that is not in the file is read as "no key" (the device is an ordinary compressor), the project opens, and there is no error. The "read → write" cycle with a key SHALL be idempotent.
Why: the envelope rule is that the version follows the CONTENT and is one per round; an address by row rank had already led to the state being reassigned to a neighbour (R.62, the `TrackId` key in the FX state).

#### Scenario: v12 round trip and byte stability of a file without a sidechain
- **WHEN** a project with a compressor without a key is saved; then the same project with an assigned key; then a file is read where the key points at a deleted track
- **THEN** the first is the previous version and byte for byte; the second is v12 with an idempotent cycle and the same `TrackId` after reading; the third opens, the key is read as missing, and writing does not revive it
- **TEST** t773 (`src/libs/kt-otio/tests/convert.rs`)
