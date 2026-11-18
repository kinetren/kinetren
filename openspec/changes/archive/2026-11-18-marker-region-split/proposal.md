# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 33, registry U7): "оно еще позволяет "marker over the
region..", он почему-то меняет уже выделенный регион, это полезно, надо вынести в отдельный пункт и
нормально его назвать, а этот пункт надо чтобы всегда регион создавал новый, а не менял текущий".
R.72 left the split to him (`r72-timeline-transport-defects`, found-defects "Changes left
unfinished"); his answer of 2026-11-07, в.4 "а": split it into "always a new one" and "stretch the
existing one". Today the region-edge menu's "Marker over the region" does three things under one
name: a new range marker, stretching a point marker at the region's start, removing an equal range.

## What Changes

- "Marker over the region" ALWAYS makes a new range marker over the region. An equal range is removed
  (the toggle M has since R.24). A different marker at the region's start is NOT changed: the model
  keeps one marker per tick, so the item refuses and the notice names the other item (rule 16).
- A new item "Stretch the marker over the region", offered only when a marker at the region's start
  has another length: it takes the region's length, its name kept.
- The M gesture is untouched (its R.24 behaviour, pinned by t142).

## Capabilities

- `ui-timeline`: ADDED requirement "Marker over the region is always new; stretching is its own item".

## Impact

`src/apps/kt-app/src/timeline.rs` (the region-edge menu), `app.rs` (three methods), five catalogs (two
keys each). Test t3104; live scenario `marker_region_split`. Closes the leftover of
`r72-timeline-transport-defects`.
