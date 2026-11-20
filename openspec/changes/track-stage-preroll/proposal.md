# Proposal: track-stage-preroll

> DESIGN (spec session). Code starts after the owner answers Q1 and the orchestrator gives the go. Tests
> reserved: t3361–t3366. Base: `orch-26-11-01` at `f1f063c4`.

## Why

found-defects 420 (kept-mix-settle's class walk): "a seek into a track's reverb or delay tail starts it
cold". An event's chain pre-rolls on a real seek (`hard_seek`: `max(tail, 100 ms) + look-ahead` rendered
into the void, audio-render "There are two breaks — seek and splice"); a track stage (R.62: the sum of a
track's events through the track's chain) has no pre-roll. So a seek, a sign flip of the rate, or an export
RANGE that starts where a track's reverb or delay still rings plays silence there, where continuous play
rings. kept-mix-settle stopped keeping that wrong start (`cold_after_seek`, t2373), but it is still heard,
and an export of a region differs from the same region inside the whole export. The row's own verdict:
"a stage pre-roll changes the bytes of every such seek and export range: decide with the export reference".

## What Changes

- **A track stage pre-rolls on a real seek like an event chain**: reset, then its lane's sum over
  `[head − tail − la, head)` runs through its chain into the void, the events of that lane warmed at the
  pre-roll's start by their own `hard_seek`. A seam (a loop wrap, an event moved during play) stays a seam.
- **An export range starting mid-ring sounds as the same range inside the whole export** (the audio of a
  range = the whole export's samples of that range, byte for byte). The full export from 0 is unchanged
  (nothing rings before 0): `GOLDEN_AUDIO` holds.
- **`cold_after_seek` becomes 0 where the pre-roll ran**, so the kept mix keeps the start again.
- The pre-roll's cost is counted (`warmup_misses` as today, a trace record with the stage's frames).

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `audio-render`: track stages pre-roll on a real seek; an export range equals its part of the whole export.

## Impact

- `src/libs/kt-engine/src/audio.rs` (`render_stages`, the seek path, `cold_after_seek`), `fxstore.rs` (the
  stage's reset), `mix.rs` (no change of rule: `cold_after_seek` answers 0 after a pre-roll).
- Invariants 1–3 unchanged (the stage's state is reset on a seek only, as today's buffers; a model edit
  does not touch it), 6 (the pre-roll runs on the render thread like event pre-rolls, never in the
  callback), 11 (deterministic: the pre-roll length is a function of the chain).

## Questions for the owner

**Q1. How long may Play wait after a seek into a long track reverb?**
Today: after a seek into a track's reverb tail (a reverb on the whole track, not on an event) the tail is
silent until the next sound enters it; the render of that region alone also lacks it. A correct start must
first compute the seconds before the seek point through the track's chain, as event reverbs already do
(their tail, at least 0.1 s). A track reverb of 4 s on release costs about 0.1–0.4 s of computing before the
first sound after such a seek (more in debug); a seek where nothing rings costs nothing.
- **a) Always the full tail**, in the preview and in the render: every start sounds as continuous play.
  *(Recommended: the same rule as event effects, and the render of a region then equals its part of the
  whole render.)*
- **b) Full tail in the render, at most 0.5 s in the preview**: the preview starts faster, a longer tail
  starts partly cold there (the kept mix does not keep that start).
