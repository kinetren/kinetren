# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 34, registry U8): "нужен механизм заметок(чтобы прям можно
было текст писать многострочный с пометками и т.д.)". A marker had one text field, its name, which is
also the address of named ranges and areas; a remark written there broke the namespace and could not
hold several lines. A foreign `.otio` marker's `comment` was kept only as passthrough, invisible and
uneditable. F-COL-020's verdict took exactly this half: a marker's text separate from its name.

## What Changes

- `Marker.note`: multi-line text, outside the name namespace; `Command::SetMarkerNote`; a move and a
  paste carry it.
- The `.kt` file is format 5 when a marker has a note (an older build refuses instead of dropping it).
- OTIO: `Marker.2.comment` is the note both ways (it was passthrough only).
- UI: "Note…" in the marker's right-click menu opens a multi-line field at the flag (Enter — a new
  line; Ctrl+Enter or a click elsewhere keeps it as one undo step; Esc cancels); a marker with a note
  shows the note glyph after its name; hovering the flag reads the note in the status line.

## Capabilities

- `project-format`: ADDED requirement "A marker's note is written by content and is OTIO's comment".
- `ui-timeline`: ADDED requirement "A marker's note is typed at its flag and read on hover".

## Impact

`kt-model` (`project.rs`, `model.rs`, `ops.rs`, `project_file.rs`), `kt-otio/src/convert.rs`,
`kt-app` (`timeline.rs`, `app.rs`, `icons.rs`, five catalogs), `docs/guide/shortcuts.md`. Tests
t3111–t3113; live scenario `marker_notes`.
