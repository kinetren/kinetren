## ADDED Requirements

### Requirement: A track's and an event's right-click menus create, the window picks the kind
A clip track's right-click "Create a generator…" SHALL be one item without a list of kinds: it SHALL
create the track's first kind (a picture kind on a video track, a sound kind on an audio track) at the
clicked track and time and open its window, whose gallery changes the kind. An event's "Transition with
next/previous" SHALL each be one item: a dissolve with that neighbour as one history step, its window
open. A generator window's kind gallery SHALL hold only the kinds of its event's stream. Lesson 3
SHALL teach this path.
Why: `v26.10` line 12 (G1): "Не вижу смысла делать выпадающие меню с перечислением всех видов
генераторов … у переходов в ПКМ тоже такой же громоздкий бред. Достаточно в самом окне менять тип".

#### Scenario: A generator from a track
- **WHEN** audio track 2 is right-clicked at 6 s and "Create a generator…" pressed; then video track 2 at 2 s
- **THEN** the menu lists no kinds; a sound event stands at 6 s on audio track 2, a solid colour at 2 s on video track 2; each opens its window
- **PATH** right click on a track → "Create a generator…" → the event at the click, its window open → its Kind field → the gallery
- **TEST** t1165 (`src/apps/kt-app/src/gen_audio_tests.rs`), live scenario `src/scripts/live/genaudio.py` (`./scripts/live-check.sh`)

#### Scenario: A transition from an event
- **WHEN** an event butted to the next is right-clicked and "Transition with next" pressed
- **THEN** the menu lists no kinds; a dissolve joins them as one history step; its window is open
- **PATH** right click on the event → "Transition with next" → the dissolve, its window with the Kind field open
- **TEST** t3127 (`src/apps/kt-app/src/layout_tests.rs`), t1089 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/lessons.py`

#### Scenario: A sound event's gallery is the sound kinds
- **WHEN** a generator is created from the audio track's menu and its Kind field opened
- **THEN** the gallery shows the five sound kinds and no picture kind
- **PATH** right click on the audio track → "Create a generator…" → its window → Kind → Tone, Audio noise, Sine sweep, Beep train, Silence
- **TEST** t3128 (`src/apps/kt-app/src/gen_audio_tests.rs`)
