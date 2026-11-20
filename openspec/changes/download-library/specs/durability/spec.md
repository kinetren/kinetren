## ADDED Requirements

### Requirement: A library item is installed or removed whole, and a title names its font pack
An install SHALL become visible only by a rename of a complete, verified folder. A removal SHALL rename the
folder away before deleting it. A killed run SHALL leave nothing that `resolve` returns, and its leftovers
SHALL be removed at the next start.

A title that uses a font pack's face SHALL name the pack and its version in the project. With the pack
missing:
- the preview SHALL draw the title's box with a marker and the pack's name;
- the export SHALL refuse naming the pack, unless the person chooses to render with the fallback for that
  render.

A title with no pack named SHALL be unchanged.
Why: a half-installed item loaded as a whole one is the silent failure F-EXT-006 found in Kdenlive; a title
drawn in a substituted font is a silent change of the export's bytes (invariant 11, F-PRJ-003;
download-library D2, D7).

#### Scenario: A killed install leaves nothing loadable
- **WHEN** an install is killed after unpacking and before the rename, and a removal is killed after its rename
- **THEN** `resolve` returns no folder for the first and none for the second, and the next start removes both leftovers
- **TEST** t3862 (`src/apps/kt-app/src/library/tests.rs`)

#### Scenario: A title remembers its font pack
- **WHEN** a title in a CJK pack face is saved and opened on a store without the pack, then exported, then exported with the fallback chosen, then the pack is installed
- **THEN** the project names the pack and version; without it the preview marks the box, the plain export refuses naming the pack, the fallback export runs; after the install the title renders in its face and the project's text did not change
- **TEST** t3863, t3864, t3865 (`src/libs/kt-text/tests/font_packs.rs`)
