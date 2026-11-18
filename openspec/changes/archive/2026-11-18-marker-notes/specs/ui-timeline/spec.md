## ADDED Requirements

### Requirement: A marker's note is typed at its flag and read on hover
The marker's right-click menu SHALL offer "Note…", which opens a multi-line field at the marker's
flag: Enter SHALL start a new line, Ctrl+Enter or a click elsewhere SHALL keep the text as one undo
step, Esc SHALL leave the note as it was. A marker with a note SHALL show the note glyph after its
name on the time-label row, and hovering its flag SHALL read the note in the status line.
Why: `v26.10` line 34, the owner: "нужен механизм заметок(чтобы прям можно было текст писать
многострочный…)".

#### Scenario: A note is typed at the flag and read on hover
- **WHEN** a marker's menu gives Note…, two lines are typed with Enter between them and a click lands
  elsewhere; the flag is hovered; Note… is opened again, more is typed and Esc pressed
- **THEN** the field is multi-line, nothing is written while typing, the click keeps both lines as one
  undo step, the note glyph is drawn, the status line reads "take 3 · warmer", and Esc changes nothing
- **PATH** right-click a marker's flag → Note… → a multi-line field under the flag; type two lines
  (Enter between them) and click elsewhere → the note glyph stands after the flag; hover the flag →
  the status line reads both lines; Note… again, type more, Esc → the note as it was
- **TEST** t3113 (`src/apps/kt-app/src/layout_tests.rs`)
