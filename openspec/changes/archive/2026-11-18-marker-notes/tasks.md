# Tasks

Tests t3111–t3113 (the `features-slow` block). Branch `marker-notes` from `orch-26-11-01` (dde924f1).

## 1. Model, file, door

- [x] 1.1 `Marker.note`, `Command::SetMarkerNote`, a move and a paste carry it
- [x] 1.2 `.kt` format 5 by content (`has_marker_notes`, `PROJECT_FILE_VERSION_NOTES`)
- [x] 1.3 OTIO `comment` ↔ note (`convert.rs`); t3111, t3112

## 2. UI

- [x] 2.1 Note… in the marker's menu, the multi-line field at the flag, the glyph, the hover's status line, Esc's step 0, the overlay in the closed list
- [x] 2.2 Catalogs ×5, `docs/guide/shortcuts.md`; t3113

## 3. Live, docs, close

- [x] 3.1 Live scenario `scripts/live/marker_notes.py`, five screenshots
- [x] 3.2 kt-model + kt-otio 310/310, kt-app guards 136/136, clippy clean in the touched files; rest cost: the glyph is drawn only for a marker with a note, its probe name only built under a dump
- [x] 3.3 What's new; registry U8 → done; F-COL-020 verdict; archive
