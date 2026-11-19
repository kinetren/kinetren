## ADDED Requirements

### Requirement: A bin card lands with the scrubbed moment under the mouse
A bin card dragged onto a track SHALL place the whole file with the file moment its press was on (on
the scrub strip) under the mouse. Holding Alt over the track SHALL put the event's start under the
mouse, Alt+Shift its end; the modifiers SHALL be read every frame, so pressing or releasing them
mid-drag moves the landing before the drop. Before the release the drop ghost SHALL be the event —
its length at the start the drop will use, with a tick at the mouse — and the status line SHALL name
which point lands under the mouse and the keys for the others, in all five catalogs. While carried, the card's strip
SHALL show the pressed moment, and the card SHALL NOT write its own hover text.
Why: `v26.10` MW1 and his answer U9 of kkt-26.11.18b: "целиком, момент под мышью. а если прожали alt
то начало события будет под курсором … чтобы уже визуально до вставки была голограма события".

#### Scenario: The scrubbed moment lands under the mouse
- **WHEN** a 12 s file's card is pressed at the middle of its strip and dropped on V1 at 20 s
- **THEN** the event is the whole file and starts so that its 6 s moment is at 20 s (within the magnet); the status line said the moment lands under the mouse
- **PATH** hover the card, the timecode reads the middle → press there and carry to V1 → the ghost is 12 s long with its middle at the mouse, the status line names the moment, Alt and Alt+Shift → release → the event stands where the ghost was
- **TEST** t3117 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/bin_drag_moment.py` (`./scripts/live-check.sh`)

#### Scenario: Alt and Alt+Shift pressed mid-drag move the landing
- **WHEN** the same drag has Alt pressed once the card is over the track; then again with Alt+Shift
- **THEN** with Alt the event starts at the mouse, with Alt+Shift it ends there; the status line names the start, then the end
- **PATH** carry the card over V1 → press Alt → the ghost jumps to start at the mouse → release → the event starts there; again with Alt+Shift → the ghost ends at the mouse → release
- **TEST** t3118 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The ghost is the real event
- **WHEN** the card is over the track before the release; the catalogs are read
- **THEN** the ghost's width is the file's length on the view's scale; the carried card's strip shows the pressed moment's timecode; the three landing texts exist in all five catalogs and name Alt
- **PATH** carry the card over V1 → the green ghost is as long as the file, a white tick at the mouse
- **TEST** t3119 (`src/apps/kt-app/src/layout_tests.rs`)
