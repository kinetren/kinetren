# Design

## D1. A field of the marker, not a second entity

F-COL-020 (and F-TL-007 before it) decided: the remark lives ON the marker. `note` is a plain string,
serialized only when non-empty, so every file without notes keeps its bytes. It is not in
`taken_names`: two markers may carry the same note. Markers stay addressed by time (R.35 item 4); a
note does not reopen that decision.

## D2. Format 5 by content

`version_for` writes 5 only when a note exists (`has_marker_notes`; markers live at the root only).
An older build reads a marker object without the field and would drop the note silently on its next
save; with 5 it refuses and names the version. No legacy fix is needed: a file below 5 has no notes.

## D3. OTIO comment IS the note

The reader puts `comment` into `note`; `MarkerExtra` keeps only metadata and extra. The writer writes
the model's note; when a note is empty it falls back to a comment still retained by a project written
before this change (its passthrough held it) — a project that never showed the comment does not lose
it on its next export. No row in "What OTIO export loses": OTIO expresses it.

## D4. Typed at the flag, read on hover

The same inline-field pattern as the marker's name (R.10 item 1), multi-line: Enter is a new line, so
the field is kept by Ctrl+Enter or by a click elsewhere — a typed note is not thrown away by a stray
click; Esc cancels (the field's own check and the Esc cascade's step 0). One `SetMarkerNote` when it
closes (one undo step, nothing written while typing; an unchanged text writes nothing). The note glyph
(Phosphor `note`) stands after the name on the time-label row (rule 13: no new strip); the text is
read in the status line on hover (rule 10: one place of hints), its lines joined by " · ". The layer
joins the closed list of overlays (`marker-note-edit`).

## D5. Class walk: text a person writes where only a name fits

The mechanism: a place where a person wants to leave a remark has only a short name that is also an
address. Checked:
- markers — fixed here;
- range markers — the same marker, the same field;
- named areas — a name that is an address; an area has no note (no request; the morpher picker
  addresses them by name);
- events — the name is the source's; an event note is a candidate (Premiere/Resolve have clip
  markers; not asked);
- tracks — a name only; not asked;
- the bin's media and groups — names only; not asked;
- reels — the name; their public parameters have free author text already.
"с пометками" is read as free text with lines; a checklist or styled text is not done and would be a
question to him.
