## ADDED Requirements

### Requirement: A reel strip row carries the version of what its cells hold
The disk tier's strip row key SHALL be (reel, edit, media fingerprint mixed with `STRIP_ROW_VERSION`). A row
written under another version SHALL be a miss, rendered again once and replaced. The version SHALL move
whenever what a strip's cell holds changes for the same edit and media.
Why: found-defects 524 — builds between catch-up-shows-loading D6 and t2455's fix wrote rows whose generator
cells were empty, and the key (reel, edit, media) never moved for them; treating a row with an empty cell as a
miss would re-render a reel whose cell is legitimately empty at every opening.

#### Scenario: A row of an older version is a miss
- **WHEN** a strip row is written under the media fingerprint alone (the version before), then under the current key
- **THEN** the first is not read back under the current key; the second is
- **TEST** t2519 (`src/apps/kt-app/src/reel_preview.rs`)
