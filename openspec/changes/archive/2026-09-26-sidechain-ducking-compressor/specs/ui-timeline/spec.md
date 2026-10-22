# Spec Delta

## MODIFIED Requirements

### Requirement: Track on screen — badges, properties window, chain, lock as a refusal
The track header SHALL carry a row of badges M / S / lock / FX under the name (service lines — only the lock); the width of the header strip SHALL be fixed by the gallery of the row and SHALL NOT change with state (rule 3), the grab zone of a badge is at least 14 px (rule 1); a badge works repeatedly and during playback. The same verbs SHALL be in the right-click menu and on the hotkeys ALT+M / ALT+S — one body (rule 17); bare M and S remain with the mark and the cut (the owner's bindings), and the target of the key is the track of the SELECTED event, and without a selection — the row under the pointer. The "Track properties" window SHALL edit the name, level, blend mode, switches AND THE CHAIN in one session and one undo step, return focus, and close via the Esc cascade. The track's chain SHALL be drawn by the same strip widget as that of an event and a morpher; a track has no automation widget, and envelopes that arrive with an inserted chain SHALL be rejected with a reason and the address where to put them. A gesture over an event of a locked track SHALL be shown as an error with the track highlighted, not pass silently. The name and height of a track SHALL go through undo.

DUCKING in the track window SHALL be shown by TWO COMPRESSORS — by the same parameter widget as any compressor, without its own form made of separate fields: on the left the compressor of THIS track (the background) with the chosen key, on the right — the real compressor of the KEY track, drawn muted and editable right from here; if there is no compressor on the key, it is created by one action and one history step. The choice of the key SHALL be a list of the audio tracks of the same reel with their names; the target track is absent from the list, the tracks that would produce a cycle are absent too, and the reason is named (rule 16). Ducking MUST NOT have its own window, nor a separate set of four numbers. Edits of both compressors SHALL go in the same window session and the same undo step as the other track properties, and the device parameters SHALL live in the project (in the track's chain), not in the user storage.
Why: "Dead ends: you can create it, you can't control it" — seven rounds; rule 13 forbids a full-height strip for the sake of three buttons, therefore the badges stand under the name in a place already occupied. R.63 (the owner): "everywhere, meaningless copy-pastes of the same code pile up, when one can even visually show the user that ducking works in the program, for example, through two compressors"; "this is trivially easier to do through fx-audio on the whole track".

#### Scenario: The badges are hit, do not jump, work repeatedly
- **WHEN** from the person's point M, S, lock, FX of a clip track and the lock of a morph row are pressed, twice in a row, including the edge of a badge at the window border; the states change
- **THEN** each press reaches the model (the window border does not steal clicks), the width of the header strip does not change, a repeated press reverts the state, the service line has no M/S/FX, ALT+M and ALT+S do the same and do not take away the mark and the cut
- **TEST** t742 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The track properties window
- **WHEN** the window is opened twice; the name, level, mode, switches are edited; "Cancel"
- **THEN** the edits are one undo step, "Cancel" reverts everything, focus is returned, the limits come from the passport
- **TEST** t743 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The track chain in its window
- **WHEN** the track window is open; a chain WITH ENVELOPES is inserted into the strip
- **THEN** the strip is the same as that of an event, the effect goes into the model in one step, the curves are discarded with a named reason and address, and not silently; the labels come from the catalogs, the glyphs are in the font
- **TEST** t744 (`src/apps/kt-app/src/layout_tests.rs`), `i18n_tests.rs`, `font_tests.rs`

#### Scenario: A lock refuses a gesture
- **WHEN** an event on a locked track is dragged, trimmed, cut, deleted; its mute is edited
- **THEN** each gesture is an error with the track highlighted and a reason, the model is untouched; mute is applied
- **TEST** t745 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Ducking — two compressors, not its own form
- **WHEN** in the window of the background audio track the key is chosen — the speech track; the speech has no compressor yet; then the threshold on the background and the threshold on the speech are edited; then the window is closed with "Cancel"
- **THEN** two compressors are shown with the same widget (the second muted), the speech compressor is created in one history step, both edits are one window session and one step, "Cancel" reverts both; there is no ducking window of its own, there are no separate ducking fields, the parameters live in the project
- **TEST** t775 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The key list does not offer the impossible
- **WHEN** the key choice is opened on a track that already serves as the key of another track; and on a track where the key points at a deleted track
- **THEN** the list contains neither the track itself nor those that would produce a cycle, and the reason is named in the status line; a dead key is shown as "no key" and is cleared with one action
- **TEST** t776 (`src/apps/kt-app/src/layout_tests.rs`)
