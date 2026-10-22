# Spec Delta

## MODIFIED Requirements

### Requirement: Format — `Stack` as the child of `Track`, sidecar is advisory
In the EXCHANGE file an instance SHALL be written as a `Stack` child of a `Track` with the body of the reel
inlined into each instance; on reading, the instances collapse by the address of the reel, and
diverged bodies SHALL become separate reels with a notice. The sidecar of the reels (the number of
tracks, the order of rows, morphers, cameras, the layer map) SHALL be advisory: if it is missing or
does not match — a notice, and the edit is read from the file. Foreign nested `Stack` and `Track` inside a
`Track` SHALL be read as real reels. The version cascade MUST NOT move files without
attachments.
In the PROJECT file a reel SHALL be stored ONCE with its body, and an instance SHALL be a
reference to its address; there is no inlining there. The difference is one of addressing, not of taste: the exchange
needs self-containment (the reader does not know our conventions), the project needs non-repetition.
Why: JSON OTIO has no object identity; a silent choice of one of two diverged bodies
is the loss of someone else's work; an old build would read the attachment as a placeholder and, on rewriting,
would turn the live edit into dead raw data. R.70: the same fact — the absence of object
identity — makes inlining for the PROJECT a multiplicative cost: the owner's three reels
produced 15 220 clips and 115 MB.

#### Scenario: The round trip of a file with an attachment
- **WHEN** a project with a reel is saved and read in ITS OWN format; separately — it is exported to OTIO and read by import
- **THEN** in both cases the reel, its instances and their ids survive the round trip, and idempotency is preserved; in the project the body of the reel is written once, in the export — inlined into each instance
- **TEST** t431 (`src/libs/kt-otio/tests/convert.rs`), t810 (`src/libs/kt-model/tests/project_file.rs`)

#### Scenario: Diverged bodies and an advisory sidecar
- **WHEN** a foreign editor changed one instance and did not change the other; the sidecar is missing
- **THEN** the second body becomes a separate reel with a notice; without the sidecar the edit is read from the tracks
- **TEST** t432 (`src/libs/kt-otio/tests/convert.rs`), t433 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The version cascade does not move old files
- **WHEN** all committed `.otio` files without attachments are re-saved by export
- **THEN** byte for byte and with the same version
- **TEST** t433b (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Foreign attachments are real reels
- **WHEN** `nested_example.otio` is read
- **THEN** three reels, not a single placeholder, and the round trip is idempotent
- **TEST** t437 (`src/libs/kt-otio/tests/convert.rs`)
