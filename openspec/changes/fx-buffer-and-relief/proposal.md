# Proposal: fx-buffer-and-relief

> DESIGN ONLY (spec session, 2026-11-25). Wave W6 of F-FX-007 (`openspec/specs/video-fx/design.md` D2), registry
> `kkt-26.10.27` K7, which no change covered. Tests reserved: t4287–t4291.

## Why

The owner's FX items (ktprompts, FX): "buffer + blend levels" — trails of the picture's own past — and a relief with one
light; the catalogue's Droste zoom.

## What Changes

- `droste` (a warp plugin): a spiral homothety.
- `relief` (a barrier): normals from luminance and one light.
- `trails`: the picture blended over N earlier frames of itself with a level each; the first kind with a window of PAST
  INPUT frames, so it stays a pure function and the memo stays honest.

## Capabilities

### Modified Capabilities

- `video-fx`: the two kernels and the window.
- `plugins`: the `past` declaration.

## Impact

- kt-plugin-api (`Decl.past`), `kt-engine` `video.rs`/`video_stream.rs` (one read per past frame, prefetch's wish list), kt-fx
  `plan.rs` (a merge pass), `src/plugins/fx-video-distort` (droste), `src/plugins/fx-video-stylize` (relief, trails).
- Stage A (droste, relief) is free now; stage B (trails) is after `fx-graph-dag` 5.1 and not beside `shared-events`.
