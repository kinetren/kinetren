## ADDED Requirements

### Requirement: Disk copies are switched beside the quality and asked for on the card
A switch beside the preview quality SHALL offer "As in the preview" (the default), "1/2", "1/4" and "Off", and SHALL
say how many of the project's files have a ready copy and their size on disk; a bin card SHALL show its copy state
(ready with its size, making with percent, none) and offer "Make a disk copy", "Use a camera's copy…" and "Remove disk
copies"; an item that cannot apply (a generator, a still, a file without video) SHALL be disabled with its reason. No
text of the interface SHALL say "proxy" in any language.
Why: the owner, A10 — a switch "как в превью" beside, never "прокси"; UI rules 13, 16, 27.

#### Scenario: A heavy file gets its copy and plays from it
- **WHEN** a costly 4K clip is placed and played at preview 1/4, the switch is opened, and a generator's card is right-clicked
- **THEN** the clip's card shows a percent, then the copy badge with its size; the switch reads "As in the preview · 1/1 · <size>"; the generator's "Make a disk copy" is greyed with its reason
- **PATH** drop the 4K clip on the timeline → play at 1/4 → the clip's bin card shows 0…100 % → a disk badge with "48 MB" → click the switch beside the quality → its menu shows "As in the preview ✓ · 1/2 · 1/4 · Off" and "1 of 1 files, 48 MB" → right click on a generator's card → "Make a disk copy" greyed with "a generator has nothing to decode"
- **TEST** t3447 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `disk_copies` (t3448)

#### Scenario: No "proxy" anywhere in the interface
- **WHEN** the five catalogs are scanned
- **THEN** no entry contains "proxy" or "прокси" (or the word in the other three languages' catalogs)
- **TEST** t3450 (`src/apps/kt-app/src/i18n_tests.rs`)
