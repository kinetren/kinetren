## ADDED Requirements

### Requirement: Every entity with values has copy, built-in and own presets
Effect slots, chains, curves, transitions, generator values, mask lists, cameras, morpher patches and the
Geometry node SHALL each have a clipboard block and a preset kind in the user store (one file per kind, the
library's rules for raw, read-only and broken files); pasting a block or a preset SHALL be one undo step that
places keys by the entity's own time domain; a curve preset SHALL be stored in relative time of its span and
fitted to the target row's range, and refused across units with the reason.
Why: the owner, v26.10 A3 — "у всех есть возможность копирования кривых, выбор дефолтных пресетов, и
пользовательских пресетов".

#### Scenario: Curve and transition presets round-trip
- **WHEN** an opacity curve is saved as a preset, applied to a gain row and to a longer event's opacity, and a transition with curves is copied and pasted onto another transition, then the store file is read back
- **THEN** the opacity preset fits the longer event's span with the same shape; applying it to gain is refused naming the units; the transition keeps its kind, values and curves from its window start; each paste is one undo step; the files round-trip and a broken file goes to `.broken`
- **TEST** tPENDING-4 (`src/libs/kt-model/tests/presets.rs`), tPENDING-5 (`src/apps/kt-app/src/user_store.rs`)

### Requirement: Take from event is a checklisted paste
"Take from event…" SHALL let the person click a source event, transition, camera block or track header and
choose in a checklist which of its parts to take (picture chain, sound chain, their curves, geometry, masks,
level, generator values), with keys kept in timing or stretched to the target; the result SHALL apply to the
target or every selected compatible event in one undo step, skipping and counting what a target cannot take;
for a single curve "Link instead" SHALL make a live link instead of a copy.
Why: the owner, v26.10 A2 — "без отдельного сохранения в архив - взять … из другого события".

#### Scenario: Take a chain and its curves, but not the masks
- **WHEN** three selected events take from a source event its picture chain with curves (stretched) but not its masks, and one of the three is an audio event
- **THEN** the two video events get the chain and the curves stretched to their lengths and keep their own masks; the audio event is skipped and the status line counts it; one Undo removes all
- **TEST** tPENDING-6 (`src/libs/kt-model/tests/presets.rs`)
