## ADDED Requirements

### Requirement: An event window says which event it edits
The effects window SHALL be titled by the event's name (a title's first words, a generator's kind, a reel's name, a
file's base name) and the window's kind, the transition window by the events it joins, the morpher window by its row and
start, and each SHALL show its number in the body in weak text; the per-chain fold SHALL be drawn without an indent strip; copy and paste of a chain
and of an envelope SHALL be icon buttons with their hints.
Why: the owner, v26.10 W3, T8, T7.

#### Scenario: A title by name, the number in the row
- **WHEN** the effects of an event of `interview.mp4` (number 2149) are opened, and those of a linked pair
- **THEN** the titles read "interview.mp4 — Effects" and "interview.mp4 + sound — Effects"; "#2149" is drawn after "Video" at the chain's head; the folds have no left strip; the copy/paste controls are icons with hints
- **PATH** double-click the interview event → the window's title reads "interview.mp4 — Effects" → "#2149" in grey beside "Video" at the head of its chain → hover the copy icon → the status bar says "Copy the chain"
- **TEST** t3940 (`src/apps/kt-app/src/reach_tests.rs`), t3941 (`src/apps/kt-app/src/reach_tests.rs`), live `window_chrome`
