# Proposal: transitions-w5

> DESIGN ONLY (spec session, 2026-11-25). Wave W5 of F-FX-007 (`openspec/specs/video-fx/design.md` D2), registry
> `kkt-26.10.27` K7, which no change covered. Tests reserved: t4275–t4278.

## Why

Transitions are half of the owner's K7 complaint (`kkt-26.10.27`): we have few iris shapes, no warps, and `push`/`slide`
have no motion blur.

## What Changes

- `iris`: heart, star, hexagon, appended at the end of its shape list.
- Three new kinds: `lens_warp`, `ripple_warp`, `twirl_warp` (styled family).
- `push` and `slide`: a `blur` row (default 0, the former bytes).

Not here: `luma` driven by a user's image. A transition binds two textures (`samp(0u)`, `samp(1u)`); a media row and a
third binding are a change of the transition contract and go with the media-in-transitions question (F-CMP-002).

## Capabilities

### Modified Capabilities

- `transitions`: the shapes, the kinds, the row.

## Impact

- `src/plugins/transitions-basic` (iris.wgsl, push.wgsl, slide.wgsl), `src/plugins/transitions-styled` (three new files),
  `BUILTIN_IDS`, catalogs ×5; R.51's closed-form tests. No format version change (a choice value and an omitted row).
