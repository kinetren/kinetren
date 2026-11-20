## ADDED Requirements

### Requirement: The project file is parsed once and written without a copy
Reading a `.kt` file SHALL check the version from the marker before the body and SHALL build the model from the text
in one typed pass, without an intermediate document tree; writing SHALL serialize the project borrowed, without
cloning it; the bytes written SHALL be unchanged by this.
Why: the owner, C2a — "нужен замер, чтобы поискать узкие места"; the double parse held the whole file twice.

#### Scenario: Allocations of a read stay near the model's size
- **WHEN** projects of 10 k and 100 k events are written and read back under the counting allocator
- **THEN** a read allocates at most 3× the bytes of the model it builds and grows at most 12× between the two sizes; a write allocates no copy of the project; the bytes equal the former writer's
- **TEST** t3924 (`src/libs/kt-model/tests/project_scale.rs`)

#### Scenario: A newer version is refused before the body is read
- **WHEN** a file with a version above the reader's and a malformed body is read
- **THEN** the refusal is `TooNew` with the version, not a JSON error
- **TEST** t3925 (`src/libs/kt-model/tests/project_scale.rs`)

### Requirement: A repeated effect chain is written once and referred to by its content
The `.kt` writer SHALL write every effect chain (slots, bypass flags, curves, master switch) that occurs on at least
two carriers once, in a `chains` table at the end of the file keyed by the hash of its canonical text, and each
carrier SHALL refer to it by that key; a chain that occurs once SHALL be written in place; the reader SHALL expand
every reference into an independent copy, and an unknown key SHALL be a named error with the carrier's address. The
version SHALL rise by content only when the table is not empty.
Why: the owner, C2c, option A — "10тыс раз поставлю событие с 100 одинаковыми эффектами … сколько места экономит и
скорости чтения"; in the program every copy stays independent.

#### Scenario: Ten thousand copies of a hundred effects
- **WHEN** an event with 100 effects and curves is copied 10 000 times at different positions and sources, written, read back, and one copy's effect is edited and written again
- **THEN** the file is at most 1/20 of the size without the table; the read-back model equals the original; the edit changes only that copy in the model and a few lines in the file
- **TEST** t3926 (`src/libs/kt-model/tests/project_scale.rs`)

#### Scenario: By content, deterministic, refused by an older build
- **WHEN** a project where no chain repeats is written; then a project with repeats is written twice and offered to a reader of the previous version; then a file with a reference to a missing key is read
- **THEN** the first file is byte-equal to the former writer's; the two writes are equal and carry the raised version; the old reader refuses naming it; the broken reference is an error naming the event
- **TEST** t3927 (`src/libs/kt-model/tests/project_scale.rs`)

#### Scenario: OTIO export writes every chain in full
- **WHEN** the project of t3926 is exported to `.otio` and imported back
- **THEN** every event carries its full chain; the import equals the model
- **TEST** t3928 (`src/libs/kt-otio/tests/project_scale.rs`)

### Requirement: The program is measured at a million events
`kt-cli scale` SHALL build synthetic projects of a given size and shape (flat, nested, copies) and report wall time,
allocations and peak memory of read, open, write, edit, undo, the middle view, the full view, the bin and the OTIO
export; the paths of the project file and the model SHALL grow at most linearly under the counting guard.
Why: the owner, 2026-10-30, question 3 — "сотни тысяч - миллион, нужен замер".

#### Scenario: The tool runs and reports every path
- **WHEN** `kt-cli scale --events 10000 --shape copies` runs
- **THEN** it prints one row for each of the nine paths with time, allocations and memory, and exits 0
- **TEST** t3929 (`src/apps/kt-cli/tests/scale.rs`)

#### Scenario: Linear growth of the file and model paths
- **WHEN** the flat and nested shapes of 10 k and 100 k events are read, validated, written and indexed for the middle view under the counting guard
- **THEN** walked elements and allocated bytes grow at most 12× for a 10× project; the middle view's index query walks at most 2× more at 100 k than at 10 k
- **TEST** t3930 (`src/libs/kt-engine/tests/build_cost.rs`), t3931 (`src/libs/kt-model/tests/project_scale.rs`)
