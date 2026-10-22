# Spec Delta

## MODIFIED Requirements

### Requirement: Ducking — a snapshot of the envelope, not a live sidechain
Auto-ducking SHALL be computed by an explicit command: an honest render of the source track (gains,
fades, FX accounted for) → speech intervals → an ordinary volume curve on the Gain slot of the music events, all in one history step. An event's sound MUST NOT become a function of another track in real time — until change `sidechain-ducking-compressor` (the owner's decision in R.63: a live sidechain compressor on the track, shown by the same compressor widget).
The analysis runs in the background with its own fetcher; the result is applied only if the model and the media have not changed.
The tool SHALL be called by its industry name in all catalogues — "Ducking (background attenuation under speech)" / "Audio ducking"; the parameters (threshold, depth, attack, release, source track) SHALL BE REMEMBERED between calls and sessions in the user store (`user_store`) and substituted at the next call; the call SHALL live in the track window (its chain) as an action on THIS background track — there MUST NOT be a separate ducking window; the result is ordinary level curves of the events, which are visible, editable, and recomputed by a repeat call with the same (remembered) parameters.
Why: a live sidechain needs a second input on the effect and the bus (XL work), while a snapshot is visible to the eye, is edited by hand, and is rolled back with one undo. R.63 (the owner): "Duck under a track" — "what is under the track, what needs ducking?? is this Audio-Ducking? why didn't you name it properly"; "why the hell do I have to remember every time which parameters I entered last time"; "this could obviously be done more conveniently via fx-audio on the whole track".

#### Scenario: Music is ducked under the voice
- **WHEN** on the voice track there are two "words", and on another — music; ducking is invoked with a depth of 9 dB
- **THEN** the intervals are found to within a step, four keys per interval, the music under speech is quieter by 9 dB, outside speech it is unchanged, undo returns the model byte-for-byte
- **TEST** t341 (`src/libs/kt-engine/tests/ducking.rs`)

#### Scenario: Ducking in the background and stale protection
- **WHEN** ducking is started on a long edit, and while the computation runs the model or the media changes
- **THEN** the window stays alive (the computation runs in the background), and the stale result is not applied
- **TEST** t420, t421 (`src/apps/kt-app/src/layout_tests.rs`, moved to the track window)

#### Scenario: Name, parameter memory, call from the track window
- **WHEN** ducking is invoked from the window of a background audio track with changed threshold and depth; the window is closed and opened again; the program is restarted (user_store in memory under cfg(test) and in the file in a live window); the item is searched for across all five catalogues
- **THEN** the fields show the previous values, not the defaults (on the old code — `DuckParams::default()`); the catalogues contain the word «ducking»/"ducking" and do NOT contain «under a track»/"under a track"; the program has no window with id `duck`; the glyphs are in the font
- **TEST** t756 (`src/apps/kt-app/src/layout_tests.rs`), t756b (`src/apps/kt-app/src/user_store.rs`), `i18n_tests.rs`, `font_tests.rs`
