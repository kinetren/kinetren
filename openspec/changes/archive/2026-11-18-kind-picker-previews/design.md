# Design

## D1. One service, one more candidate form

A kind cell is the effect cell's job with another candidate: the scene (the project the player plays,
the place, the moment) and, instead of effects appended at the place, a `Swap` that puts a kind in
place — `Swap::Gen(clip, kind)` sets the event's generator to the kind's defaults with no curves (what
`fit_gen_to_media` makes of a kind change), `Swap::Transition(id, kind)` sets the transition's kind
with no curves (what the picker's click does). The cut's or the event's video track is soloed, as for
an effect cell. Everything else is the service's: invariant 5's offline path on its lane, the misclick
guard (nothing opened in the first 800 ms: Loading), the RAM cache, the pause while playing, nothing
partial shown. Rejected: a second service for kinds — a second queue, cache and set of open files for
the same frames.

## D2. The middle of the window

A transition has no single picture: at either edge of its window every kind is the same frame A or B.
The cell is the middle of the window (`window.start + len/2`), where both sides show and kinds differ;
the cursor is not followed (it is usually outside the window). A still does not show motion — the
direction of a wipe reads from its shape. A generator's cell is at the cursor clamped into the event,
the effects picker's moment, frozen while playing.

## D3. The pickers stay the same controls

Both stay a `ComboBox` with the current kind's name (the harness tests t227/t511 and the lessons'
live scenario find them so); only the popup's content is a gallery. `CloseOnClickOutside` and an
explicit close on a pick (the R.54 rule), the width of the host window's body with a floor of four
cells, the screen-bounded height (`popup_list_height`). A cell below the fold is neither drawn, asked
nor published (a published rectangle there would be clicked past the popup); its name stays in the
accessibility tree, so a popup's sizing pass keeps it findable. The generator tables keep sharing a
tag as before: picking another table of the same tag changes nothing, as with the list.

## D4. Class walk: a picker of a kind with no picture

The mechanism: a choice of a VISUAL kind offered as words where the effects picker shows the result.
Checked:
- effects picker (event, morpher, track) — cells since R.54;
- generator kind in the event window — fixed here;
- transition kind in its window — fixed here;
- "Transition with next/previous" in the event's right-click menu — a context menu of names for a fast
  pick by habit; a gallery there would be a picture popup inside a menu, so it stays names, and the
  window it opens shows the gallery;
- "Create a generator…" in the bin, the main menu and a track's menu — no event exists yet, so there
  is no frame after the click; the bin card shows the created kind's frame right after;
- blend modes of a track and a layer — a combo of names; a cell per mode is a candidate for a later
  change (a mode shows only over another layer);
- the morpher's source kind and the geometry node's fit — names of behaviours, not looks.
