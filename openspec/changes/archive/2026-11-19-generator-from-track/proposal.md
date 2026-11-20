# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 12, row G1): "я не могу пкм по треку сделать и там создать
его … Не вижу смысла делать выпадающие меню с перечислением всех видов генераторов это утомительно.
Кстати у переходов в ПКМ тоже такой же громоздкий бред. Достаточно в самом окне менять тип." A track's
right-click menu already had "Create a generator…" (generators-placement-and-audio), but as a submenu
of groups and kinds; the event's "Transition with next/previous" was a submenu of 33 kinds. Since G6
(kind-picker-previews) both windows pick the kind in a gallery of pictures. The orchestrator relayed
the decision: his words decide it (option a).

## What Changes

- A track's right-click "Create a generator…" is ONE plain item: the track's first kind (a solid
  colour on a video track, a tone on an audio track) at the click, and its window opens, where the
  gallery changes the kind.
- An event's "Transition with next/previous" is ONE plain item each: a dissolve with the neighbour, and
  the transition's window opens with its gallery.
- Lesson 3 says "choose Transition with previous: a dissolve is made and its window opens".
- Edit's and the bin's "Create a generator…" keep their list of kinds (not in the decision; a
  question for him, see design).

## Capabilities

- `ui-reach`: ADDED requirement "A track's and an event's right-click menus create, the window picks
  the kind".

## Impact

`kt-app`: `gen_ui.rs` (`generator_item_for`, the track list gone), `timeline.rs` (the two items),
`lessons.rs` and five catalogs (lesson 3), tests t1165 and t1089 changed by this decision, t560's track
check; new t3127. Live `genaudio`, `lessons` retargeted.
