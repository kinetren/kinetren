## ADDED Requirements

### Requirement: Masks in the file — format 2 by content; the OTIO envelope v14
The masks of events, effect slots and tracks SHALL be written in the `.kt` file as fields of the model.
The file SHALL be written as format 2 when any event or track has a mask and as format 1 otherwise, so
a project without masks is byte for byte as before and a build that reads only format 1 refuses a
file with masks as "written by a newer Kinetren" instead of dropping them. The reader SHALL read
formats 1 and 2. The OTIO door SHALL carry the masks in the `kt` envelope of the clip and of the track,
raising the envelope to v14 by content; other programs see the picture without masks (a row in "What
OTIO export loses").
Why: a silent loss of masks in an older build is the "silent corruption" the version refusal exists
for; the version by content keeps old files byte-stable.

#### Scenario: Model round trip, validation, byte stability
- **WHEN** a project with picture, slot and track masks is written and read; a project without masks is written; malformed masks (a duplicate id, a two-vertex path, a curve on a missing row, masks on an audio event, a curve on a track mask) are validated
- **THEN** the round trip is identical; the maskless text is byte for byte the earlier one; each malformed case is rejected with its named error
- **TEST** t1260 (`src/libs/kt-model/tests/masks.rs`)

#### Scenario: The version by content
- **WHEN** a project with a mask and one without are written; a format-2 text is read; a format-3 text is read
- **THEN** the first carries version 2, the second 1; format 2 reads; format 3 is refused as too new
- **TEST** t1264 (`src/libs/kt-model/tests/masks.rs`)

#### Scenario: Masks through the OTIO door
- **WHEN** a project with masks is exported to `.otio` and imported back
- **THEN** the envelope is v14, the masks come back equal; a project without masks keeps its earlier envelope version
- **TEST** t1265 (`src/libs/kt-otio/tests/masks.rs`)
