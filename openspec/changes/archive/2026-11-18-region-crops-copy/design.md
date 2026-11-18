# Design

## D1. "Crops" = crosses without covering

`r.intersects_clip(c) && (r.start > c.start || r.end < c.end)` for any of the selected unit's events. A
covering region gives the same piece as the whole event, so the old branch and its status line stay;
a region apart from the event leaves the whole event (his words).

## D2. One crop for everything

`ops::copy_clips_in(p, ids, r)`: the selected events plus their link groups, each cut by `clip_slice`
(the piece of Split, punch-out and `copy_region_block`: reverse, material anchor, key shift), rebased to
the earliest crop. The extras follow `block_extras` over the crop's window, the same rule
`copy_clips` applies to the whole events' window.

## D3. t926 changes by his decision

t926's second case set the region by double-clicking a range marker lying inside one event and expected
the whole event: that is exactly "a region that crops the selected event", which B10 decides the other
way. The case now expects the part under the marker; its first case (three events — the region block)
is unchanged.

## Class walk

The mechanism: one copy target chosen without looking at how the range meets it. Walked over the
paths that take "the selection or the region": Copy (fixed here); Delete — Del with a region deletes
the crossing events whole (`delete_region`) and Cut out the region interior cuts
the interior from every track (by name: the region's interior); Paste puts the block; the FX chain copy
copies a chain, not a range; Collapse into a reel takes the selection. Only Copy had the gap.
