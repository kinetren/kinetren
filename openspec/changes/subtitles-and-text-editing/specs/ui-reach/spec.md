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
- **TEST** t3473 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/captions.py` (t3474)
