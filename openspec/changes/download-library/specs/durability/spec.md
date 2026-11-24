## ADDED Requirements

### Requirement: A library item is installed or removed whole, and a title names its font pack
An install SHALL become visible only by a rename of a complete, verified folder. A removal SHALL rename the
folder away before deleting it. A killed run SHALL leave nothing that `resolve` returns, and its leftovers
SHALL be removed at the next start.

A title that uses a font pack's face SHALL name the pack and its version in the project. With the pack
missing:
- every picture of the title SHALL show its box hatched with the pack's name;
- the export SHALL refuse naming the pack and the titles, unless the person chooses the substitute font for that
  render, which draws the title in it, never the hatched box, and is named in the result.

The file the face comes from (a pack's or a system family's) SHALL be part of the identity of the title's frames in
memory and on disk. A title with no pack named SHALL be unchanged, and a system family that is missing SHALL be
substituted with a notice as before.
Why: a half-installed item loaded as a whole one is the silent failure F-EXT-006 found in Kdenlive; a title
drawn in a substituted font is a silent change of the export's bytes (invariant 11, F-PRJ-003;
download-library D2, D7).

#### Scenario: A killed install leaves nothing loadable
- **WHEN** an install is killed after unpacking and before the rename, and a removal is killed after its rename
- **THEN** `resolve` returns no folder for the first and none for the second, and the next start removes both leftovers
- **TEST** t3862 (`src/libs/kt-library/tests/library.rs`)

#### Scenario: A title remembers its font pack
- **WHEN** a title in a CJK pack face is saved and opened on a store without the pack, then exported, then exported with the fallback chosen, then the pack is installed
- **THEN** the project names the pack and version; without it the preview marks the box, the plain export refuses naming the pack, the fallback export runs; after the install the title renders in its face and the project's text did not change
- **TEST** t3863 (`src/libs/kt-text/tests/font_packs.rs`), t3864 (`src/libs/kt-engine/tests/export.rs`)

#### Scenario: A frame of one face is never shown for another
- **WHEN** a title in a pack face is composed and prerendered without the pack, the pack is installed, removed, and a title in a system family is composed before and after that font is installed
- **THEN** each composite and each stored frame is keyed by the face it was drawn with; none is shown for the other
- **TEST** t3865 (`src/libs/kt-engine/tests/moment_key.rs`)
