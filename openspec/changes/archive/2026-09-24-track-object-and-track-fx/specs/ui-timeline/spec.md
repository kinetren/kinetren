# Spec Delta

## ADDED Requirements

### Requirement: Track on screen — badges, properties window, chain, lock as a refusal
The track header SHALL carry a row of badges M / S / lock / FX under the name (service rows — only the lock); the width of the header strip SHALL be fixed by the gallery of the row and not change with state (rule 3), the badge's hit zone is at least 14 px (rule 1); the badge works repeatedly and during playback. The same verbs SHALL be in the right-click menu and on the hotkeys ALT+M / ALT+S — one body (rule 17); the bare M and S remain with the marker and the cut (the owner's bindings), and the target of the key is the track of the SELECTED event, without a selection — the row under the pointer. The "Track properties" window SHALL edit the name, level, blend mode, switches AND THE CHAIN in one session and in one undo step, hand back focus and close with an Esc cascade. The track chain SHALL be drawn with the same strip widget as for an event and a morpher; the track has no automation widget, and envelopes arriving with an inserted chain SHALL be rejected with a reason and an address of where to put them. A gesture over an event of a locked track SHALL be shown as an error with the track highlighted, not silent. The name and height of a track SHALL go through undo.
Why: "Dead ends: you can create, you cannot control" — seven rounds; rule 13 forbids a full-height strip for three buttons, so the badges stand under the name in the place already occupied.

#### Scenario: The badges are hit, do not jump, work repeatedly
- **WHEN** from the person's point M, S, lock, FX of a clip track and the lock of a morph row are pressed twice in a row, including the edge of the badge at the window edge; states change
- **THEN** each press reaches the model (the window edge does not steal clicks), the width of the header strip does not change, a repeated press clears the state, the service row has no M/S/FX, ALT+M and ALT+S do the same and do not take away the marker and the cut
- **TEST** t742 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The track properties window
- **WHEN** the window is opened twice; name, level, mode, switches are edited; "Cancel"
- **THEN** the edits are one undo step, "Cancel" rolls back everything, focus is handed back, the limits come from the passport
- **TEST** t743 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The track chain in its window
- **WHEN** the track window is open; a chain WITH ENVELOPES is inserted into the strip
- **THEN** the strip is the same as for an event, the effect is in the model in one step, the curves are dropped with a named reason and an address, not silently; the captions come from the catalogs, the glyphs are in the font
- **TEST** t744 (`src/apps/kt-app/src/layout_tests.rs`), `i18n_tests.rs`, `font_tests.rs`

#### Scenario: A lock refuses a gesture
- **WHEN** an event on a locked track is dragged, trimmed, cut, deleted; its mute is edited
- **THEN** each gesture is an error with the track highlighted and a reason, the model is untouched; the mute is applied
- **TEST** t745 (`src/apps/kt-app/src/layout_tests.rs`)
