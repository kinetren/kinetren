# Spec Delta

## ADDED Requirements

### Requirement: Conform rule, leading node and reel settings in the file — v11 by content
A file SHALL raise the envelope version to v11 only when it carries at least one of: a project conform
rule other than "fit"; a leading "Geometry" node with a rule other than "follow the project", or with non-identity
parameters; the reel's own raster or fps; the project's explicit-settings flag. A leading node with defaults MUST NOT be
written — an event without a node in the file receives one on read. The old `Effect` of kind `kt:Pan` SHALL be read
into the event's leading node (mode → rule, offset and zoom → position and zoom) one-way and
never written again. One version per round R.62: the change `track-object-and-track-fx`
extends the v11 predicate with its own fields.
Why: a file without new features must remain byte-stable at its previous version (the cascade over the
fixtures), and migration on read is the only legitimate path for old files.

#### Scenario: The round trip of v11 and the byte-stability of the old
- **WHEN** a project without new features is saved; a project with an explicit event rule; a project with the project rule "fill"; a reel with its own raster; then a file with the old `kt:Pan` is read
- **THEN** the first keeps its previous version and is byte for byte (t433b); the others are v11 with an idempotent round trip; `Pan` is read into the leading node with the same numbers and does not reappear on writing
- **TEST** t728 (`src/libs/kt-otio/tests/convert.rs`), t433b (`src/libs/kt-otio/tests/convert.rs`)
