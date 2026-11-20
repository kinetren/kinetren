# Design

## D1. One item, the window chooses

The track item creates `GeneratorKind::all()`'s first kind that lands on the track (`lands_on`): a
solid colour on video, the first sound kind on audio — at the clicked track and time, as before. The
existing `create_generator_at` opens the generator window; its Kind field is the gallery of G6. The
transition item makes a crossfade (the default window as before: up to 0.5 s each side when butted, the
overlap zone when overlapping) and opens `open_transition_editor`. One step of history each.

## D2. Tests changed by this decision

t1165 asserted the track's submenu of groups; it now asserts one item, the event at the click, the
track's first kind and the open window — the place and the track it guarded stay. t1089 (lesson 3)
clicked Crossfade in the submenu; it now clicks the item. t560's track check names the new item.

## D3. Not taken: Edit and the bin

His sentence names the right-click menus of a track and of transitions. Edit and the bin have no
track and no time; a plain item there would create a video kind, and the gallery shows all kinds on a
video event, so a tone from Edit would mean turning a picture event into a sound one. They keep their
list; whether they become one item too is a question for him.

## D4. The gallery holds the event's stream (found by the live run)

With the track item, a sound event's kind is changed in its window — and the gallery listed every kind,
twenty picture kinds first: the sound kinds stood under the popup's fold, the live `genaudio` could not
reach Beep train. A picture kind cannot become a sound event (nor the reverse), so the gallery holds
only the kinds that land on the event's stream (`lands_on`). Test t3128.

## Class walk

The mechanism: a menu enumerating kinds that a window already picks. Checked: the track's right-click
(changed), the event's transition items (changed), Edit and the bin (D3), the transition window's own
kind field and the generator window's (galleries, G6), the effects picker (a gallery, R.54 — choosing
WHICH effect to add is the action itself, not a kind to change later), the fade items (no kind), the
morpher's source (a file, not a kind).
