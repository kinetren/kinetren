## ADDED Requirements

### Requirement: Check the edit, with jumps to every finding
Edit → "Check the edit" SHALL list, without decoding, flash frames shorter than the threshold, gaps where no
picture event is active, events on missing media or with missing plugins, and events barred by the fit rule;
the render window SHALL show the same list when it opens and the export's picture findings after it; a click
on a finding SHALL move the playhead to it and select its event; "Mark all" SHALL add range markers named by
kind in one undo step; nothing SHALL be changed by itself.
Why: F-EXP-009 trick 1 — half of the mistakes are in the edit; rule 20.

#### Scenario: A flash frame and a gap found and marked
- **WHEN** an edit with a one-frame leftover after a cut and a 10-frame gap is checked from the Edit menu, the flash row clicked, then "Mark all"
- **THEN** the list shows a flash at its time and a gap; the click puts the playhead on the leftover and selects it; "Mark all" adds two range markers; one Undo removes both
- **PATH** click "Edit" → "Check the edit" → the Diagnostics window's Edit check section lists "flash 1 frame at 00:00:12:03" and "gap 10 frames" → click the flash row → the playhead jumps, the leftover is selected → click "Mark all" → two range markers on the ruler → Ctrl+Z → gone
- **TEST** tPENDING-9 (`src/libs/kt-model/tests/lint.rs`), tPENDING-10 (`src/apps/kt-app/src/reach_tests.rs`)
