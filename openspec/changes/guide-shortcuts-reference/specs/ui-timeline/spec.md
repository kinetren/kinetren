## ADDED Requirements

### Requirement: A chord with Alt or Ctrl does not also fire its bare letter
A hotkey that is a bare letter (split `S`, marker `M`, region `I`/`O`, loop `L`,
navigation `A`/`D`) SHALL act only when neither Alt nor Ctrl is held. A chord that adds
Alt or Ctrl to that letter (`Alt+S` solo, `Alt+M` mute, `Ctrl+S` save, `Ctrl+L` go to
timecode) SHALL perform only its own action. Shift is not covered by this rule: `Shift+A`/`Shift+D`
are the same jump with the region edge dragged along.
Why: the split condition excluded only Ctrl, so `Alt+S` toggled solo AND split the event
under the playhead in the same frame. The t742 check of `Alt+S` asserted only the solo, so
the split went unnoticed. Found while building the full shortcuts reference (2026-10-26).

#### Scenario: Alt+S only solos
- **WHEN** an event is selected and the playhead is inside it; `Alt+S` is pressed; then `Alt+M`
- **THEN** the event's track is soloed, then muted; the number of events and the selected event's range are unchanged; a bare `S` afterwards still splits it
- **PATH** a clip is selected, the playhead stands inside it → Alt+S → the S badge in the track header lights up, the clip stays whole (no cut line at the playhead) → Alt+M → the M badge lights up, the clip is still whole → S → the clip is cut in two at the playhead
- **TEST** t868 (`src/apps/kt-app/src/layout_tests.rs`)
