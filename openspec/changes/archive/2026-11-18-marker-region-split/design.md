# Design

## D1. What "always new" means under one marker per tick

`Command::AddMarker` refuses a tick that already has a marker. So "always new" cannot put a second
marker beside a point marker at the region's start; it refuses with an error notice that names
"Stretch the marker over the region" (rule 16: what is in the way and where to go). An equal range is
still removed: that is the toggle the M gesture has, and a second equal range is impossible anyway.

## D2. The stretch item is offered only when it does something

`KtApp::marker_to_stretch(r)` — a marker at `r.start` whose length is not the region's. With none, the
item is not drawn (rule 9: no dead control).

## D3. M stays

The owner's words are about the menu item. M's R.24 rule ("with a region and the cursor in it, m gives
marked_range"; a repeat toggles) and its stretch of a point marker at the start are pinned by t142,
which is frozen. The paired-ways rule (17) is about one action; the two items are now two actions, and M
keeps the combined one it always had. If he wants M to stop stretching, that is a question to him.

## Class walk

The mechanism: one menu name doing different things by hidden state. Walked over the ruler's and the
region's items: "Marker here" (a point, or removing the nearest — the M magnet; its name says
"here"); "Clear the region" (one result); the marker menu's Rename / Delete / range items (one result
each); the empty-space "Delete clips in the region" / "Cut out the region interior" (one result each).
Only "Marker over the region" changed what it did by what stood at the start.
