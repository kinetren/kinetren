## ADDED Requirements

### Requirement: A marker's note is written by content and is OTIO's comment
A marker SHALL carry a note: any text, several lines, outside the namespace of names. The project file
SHALL be written as format 5 when, and only when, a marker has a note, and a file without notes SHALL
keep its bytes. A move and a paste of a marker SHALL carry its note. The OTIO exchange door SHALL read
`Marker.2.comment` as the note and write the note as the comment.
Why: `v26.10` line 34, the owner: "нужен механизм заметок(чтобы прям можно было текст писать
многострочный с пометками и т.д.)"; F-COL-020: a remark written into the name broke the address.

#### Scenario: A note is kept, moved, pasted and written
- **WHEN** a range marker gets a three-line note, a second marker the same note, the first is moved
  and a region over it is copied and pasted, and the second's note is cleared and undone
- **THEN** the note is on the marker and not its name, two markers may share it, a note on no marker
  is refused, the file is format 5 and reads back field by field, a file without notes writes no note
  field, the move and the paste carry the note, and the undo brings it back
- **TEST** t3111 (`src/libs/kt-model/tests/project_file.rs`)

#### Scenario: A marker comment is its note both ways
- **WHEN** a foreign `.otio` marker with a two-line comment is imported, its note edited and then
  cleared, and a note made in the editor is exported and read back
- **THEN** the comment is the note; the export writes the edited note, nothing when cleared, and the
  editor's note as the comment, which comes back as the note
- **TEST** t3112 (`src/libs/kt-otio/tests/convert.rs`)
