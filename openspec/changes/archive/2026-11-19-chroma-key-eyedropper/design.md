# Design

## D1. The screen, not the composite

The pick reads the pixel of the SCREEN under the pointer (X11 `GetImage` on the root at the pointer's
root position, Windows `GetPixel` on the screen DC), not a pixel of the preview's composite. What a
person points at is what they see; the composite stays on the GPU (preview-throughput D2) and a
one-pixel readback would be a new path through the display. The screen also answers for a bin
thumbnail or a colour in another of the program's windows — "или еще где-либо". The display's own
transforms are nil for the preview (the display pass reproduces the bytes, t1525): the live scenario
reads the teal fixture back within one level.

## D2. A mode that eats one click

Armed, the app draws one transparent `Area` over the whole window in the Foreground order; the click
lands on it, so nothing under it acts (the preview would toggle playback). The resize zone at the
window edge is off while armed. The layer is in `no_teaching_layer_over_the_ui`'s closed list.

## D3. Thread-locals, as hint_sink

The colour rows are drawn inside window closures without `&mut KtApp`. The pipette arms an
`egui::Id` of its row in a thread-local; the layer writes `(id, rgb)` back; the row takes it on the
next frame and writes through its own path (the plugin slot's rounded rows, a generator's channels,
a composition's value) — so a pick is the same edit as a typed colour, one history step. Esc is a
step of the Esc cascade, before the window on top.

## D4. Not taken

- A single pixel, no averaged rectangle: the average belongs with B3's automatic analysis.
- Wayland: no screen read for a client; the bell names it (the portal's PickColor is a later step).
- Outside the program's window: a pointer grab would be needed.

## Class walk

The mechanism: a colour is set by numbers or the system picker only. Every colour button of
kt-app was walked; t3122 holds the list by the sources: `widgets::rgb_button` (chroma key, outline,
duotone ×2, glow/drop-shadow tint, a transition's colour/tint), `widgets::color_row` (generators,
plugin colour groups), `widgets::color_row_rgba` (a title's fill, outline, shadow, box), a
composition's colour parameter (`pubparam`). Masks have no colour; the mask overlay's display colours
are not values.
