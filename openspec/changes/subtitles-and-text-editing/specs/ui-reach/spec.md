## ADDED Requirements

### Requirement: Text and captions are reached where the person works
The Text panel SHALL be a tab beside the bin; "Transcribe" SHALL be in the bin card's and the audio
event's right-click menus; a subtitle track SHALL be created from the track header's menu, by importing
SRT/VTT from File and by "Create captions from text" in the Text panel; a caption SHALL be edited in
place by double-click on its box.
Why: UI rule 22 (the place order) and rule 17.

#### Scenario: From a file to burned-in captions by mouse
- **WHEN** an audio event is right-clicked → "Transcribe", the Text panel's "Create captions" is clicked, a caption is double-clicked and retyped, the render window's sidecar box is ticked
- **THEN** the panel fills with words; a subtitle track appears with captions under the preview; the retyped text shows in the frame; the render writes the `.srt`
- **PATH** right click on the audio event → "Transcribe" → the card shows progress → the Text tab fills → click "Create captions" → a subtitle track row with boxes, captions on the preview → double-click a box → type → the frame shows the new text → File → Render → tick "Sidecar SRT" → Render → the .srt beside the file
- **TEST** t3473 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `captions` (t3474)

#### Scenario: Captions by hand
- **WHEN** a subtitle track is made from the track header's menu, a caption is drawn by a double-click on its row, typed, trimmed and restyled, and an SRT is imported from File
- **THEN** the caption is on the preview as it is typed, the style acts on the whole track, the import makes its own track
- **PATH** right click on a track header → "Add a subtitle track": a row with a text icon → double-click on the row: a box with a cursor → type: the text is on the preview → drag the box's right edge → the track window's "Style": bigger, with an outline: every caption changes → File → Import → Subtitles…: a second subtitle row with boxes
- **TEST** t4252 (`src/apps/kt-app/src/reach_tests.rs`)
