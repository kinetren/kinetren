## ADDED Requirements

### Requirement: A build hashes and derives only the events that changed
The player's build of a new project version SHALL key an event's derived state by the event cell's
fingerprint, computed once per version of the event, and the reel resolve SHALL pass an event no layer,
link or override reaches as the same cell; an edit of one event SHALL compute O(1) fingerprints and
derive O(1) events, and the build's output SHALL equal a fresh build's.
Why: found-defects 192 — the incremental build hashed every event's Debug text and resolved the whole root
on every version (49 ms at 13 357 events, linear) (shared-events D3, D4).

#### Scenario: One event's edit hashes one event
- **WHEN** one event's parameter is edited on projects of 1357 and 13 357 events and the next version is built with the memo
- **THEN** the counted fingerprints computed and events derived are the same small number on both sizes
- **TEST** t3371 (`src/libs/kt-engine/tests/build_cost.rs`)

#### Scenario: The resolve passes untouched events through
- **WHEN** the root of a project with one morpher layer over two events is resolved
- **THEN** every event the layer does not reach is the same cell as in the project, and only the reached events are new
- **TEST** t3372 (`src/libs/kt-engine/tests/build_cost.rs`)

#### Scenario: A shared build equals a fresh one
- **WHEN** the edits of t2004 are applied in turn with shared cells and each version is built incrementally and afresh
- **THEN** the audio bytes and the video renderer's derived text are equal
- **TEST** t3373 (`src/libs/kt-engine/tests/shared_build.rs`)

#### Scenario: The edit cost tool reports the drag at a given size
- **WHEN** `kt-cli edit-cost` runs on the showcase at ×1 and ×10
- **THEN** it prints the deep copies and fingerprints per edit beside the times, and the copies per edit do not grow with the size
- **TEST** t3374 (`src/apps/kt-cli/tests/cli.rs`)
