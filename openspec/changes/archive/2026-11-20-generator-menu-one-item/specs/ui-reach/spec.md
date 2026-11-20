## ADDED Requirements

### Requirement: Edit and the bin create a generator by the selected track
The Edit menu's and the bin background menu's "Create a generator…" SHALL be one plain item with the
same body as a track's item. It SHALL create the first kind that lands on the track of the earliest
selected event (a solid colour on a video track, a tone on an audio track), at the cursor on that track.
With nothing selected it SHALL create a solid colour on video track 1. Its window SHALL open, and the
kind SHALL be changed in the window's gallery. A kind change, from the gallery or from the bin card,
SHALL keep the generator's sound pair in step in the same history step: a kind that sounds beside its
picture SHALL have its linked sound event on audio track 1, and a kind with no sound SHALL have none.
Why: the owner, kkt-26.11.20 line 4 (A1): "Один пункт, по дорожке (Recommended)".

#### Scenario: Edit and the bin by the selected track
- **WHEN** nothing is selected and Edit → "Create a generator…" is pressed; then an event on video track 2 is selected and the bin's background menu's item is pressed
- **THEN** neither menu lists kinds; a solid colour stands at the cursor on video track 1, then one on video track 2; the first opens its window
- **PATH** nothing selected → Edit → "Create a generator…" → a solid colour on V1 at the cursor, its window open; click an event on V2 → right click the bin's empty space → "Create a generator…" → a solid colour on V2 at the cursor
- **TEST** t3712 (`src/apps/kt-app/src/gen_audio_tests.rs`), t1164 (`src/apps/kt-app/src/gen_audio_tests.rs`), live scenario `src/scripts/live/genaudio.py` (`./scripts/live-check.sh`)

#### Scenario: A kind change keeps the sound pair
- **WHEN** Sync flash is picked in a new generator's Kind gallery; then Safe areas; then Ctrl+Z; then the bin card's Change kind → Safe areas
- **THEN** the flash has its beep event on A1, linked, under it; Safe areas leave the picture event alone; one Ctrl+Z gives both back; the card's change takes the beep again
- **PATH** Edit → "Create a generator…" → Kind → Sync flash → a beep event on A1 under the flash → Kind → Safe areas → the beep is gone → Ctrl+Z → the flash and its beep → right click the bin card → Change kind → Signal check → Safe areas → the beep is gone
- **TEST** t3713 (`src/apps/kt-app/src/gen_audio_tests.rs`), t3702 (`src/apps/kt-app/src/gen_audio_tests.rs`), live scenario `src/scripts/live/gen_technical.py`
