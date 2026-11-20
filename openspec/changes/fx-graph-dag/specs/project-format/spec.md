## ADDED Requirements

### Requirement: An effect graph raises the file by content; OTIO carries it in the envelope
A slot whose inputs are not "the slot before" SHALL write its `in` (main, second, matte) next to its
kind, and a Merge SHALL write the kind `merge`. A project with any of them SHALL be written as the
next project format (8 on this base) and as `metadata["kt"]` v18 in the exchange file; a project of
lines SHALL keep its version and its text byte for byte. An older build SHALL refuse such a file
naming its version. The reader SHALL refuse an input that names a later slot, another lane, a second
input on a kind that has none, a branch before the leading Geometry node, more than `MAX_HELD` held
frames or more than `MAX_READS` reads, with the slot's address. OTIO export SHALL keep the chain an ordered `Effect.1` list and
carry `in` in each effect's `kt` metadata, and import SHALL read it back; the loss SHALL be a row of
"What OTIO export loses".
Why: an older build would open the arms as a line and save them so; the precedents are masks
(format 2) and the device's sound rows (format 4).

#### Scenario: A graph raises the format; a line does not move
- **WHEN** a project of lines is written; a Merge and an arm are added and it is written and read; an input naming a later slot is put in the text by hand
- **THEN** the first text is unchanged and keeps its version; the second is format 8 with `fx_in` and `merge`, reads back equal and is refused by a build that reads 7; the third is refused with the slot's address
- **TEST** t3210, t3209, t3209b, t3209c (`src/libs/kt-model/tests/fx_graph.rs`)

#### Scenario: The OTIO door keeps the wiring
- **WHEN** the branching project is exported to `.otio` and imported
- **THEN** the effects are an ordered `Effect.1` list with `kt:merge`, the envelope is v18, and the imported chains equal the originals with their inputs
- **TEST** t3211 (`src/libs/kt-otio/tests/fx_graph.rs`)

#### Scenario: Copy and paste carry the wiring
- **WHEN** a block of three slots with an arm and a Merge is copied from one event and pasted into another's chain and into a morpher patch
- **THEN** the inputs inside the block are re-based, an input from outside the block becomes the block's input, and one undo removes it
- **TEST** t3224 (`src/libs/kt-model/tests/fx_graph.rs`)
