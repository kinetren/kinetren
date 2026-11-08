# Proposal: builder-cost-guard

## Why

`engine-audit` (`docs/engine-audit.md` §0, found-defects row 151) measured the whole-engine build on
the relinked copy of the owner's project: `AudioRenderer::new` 393–488 ms in release against 1 ms on
the heavy showcase of 1357 events. All three debugger stops sat in `warp::sign_knots`: a speed curve
with a shaped segment is sampled into a point every 5 ms (`rate_profile_keys`), and for each point the
function asked three readings of the map that each rebuilt the whole segment list — O(K²) per event.
Five events of his project carry such curves (one 360 s long). Every edit pays it, and every consumer
builds (2 audio + 2–3 video builds per edit, rows 110, 111, 117, 150).

The owner's go: registry `docs/ktprompts/kkt-26.11.08-index.md` H14 ("в.1. Да"), before
`edit-without-rebuild`.

The mechanism is a CLASS (`docs/engine-audit.md` M2, "an O(whole) helper inside a loop over its
parts"): it is invisible on hand-made data and explodes on generated data, and nothing counted it.

## What Changes

- `sign_knots` and `span_over` lay the map out once per call and read it by halving; the bits are the
  former ones (frozen oracles in the test).
- Every builder and per-build pass is walked for the same mechanism — work super-linear in events,
  keys, samples, placements or reel depth — measured on the relinked copy and on synthetic worst
  cases; what is super-linear is fixed, the rest is listed with numbers in `design.md`.
- **A counting guard**: `kt_time::cost`, a thread-local meter of the elements whole-structure walks
  touch, and tests that build a project ×1, ×4, ×16 along each axis (events, curve length,
  transitions, placements, morpher layers) and hold the builders' counts to a declared bound. Red on
  the former code; no clock in the check (t1671).

## Capabilities

- `time-model`: the warp map's readings for a whole curve are linear in the curve.
- `audio-render`, `video-plan`: a build's work is bounded linearly in the project.
- `compositions`: a reel's placements cost the reel once, not once per placement.

## Impact

`kt-time` (new `cost` module), `kt-model` (`warp.rs`, transition lookups), `kt-engine` (builders),
tests t2060–t2079.
