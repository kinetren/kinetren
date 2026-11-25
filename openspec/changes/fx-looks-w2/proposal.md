# Proposal: fx-looks-w2

> DESIGN ONLY (spec session, 2026-11-25). Wave W2 of F-FX-007 (`openspec/specs/video-fx/design.md` D2), registry
> `kkt-26.10.27` K7, which no change covered. Tests reserved: t4267–t4274.

## Why

The owner, `kkt-26.10.27` K7: the catalogue has far fewer effects than what people use. W2 of the ranked waves is the
Retro, Glitch, Cinematic and Blur tabs: block glitch, film damage, light leaks, halation, a lens blur, a directional
blur, a sepia and grain looks.

## What Changes

- Four new kinds (one kernel each, WGSL only since the CPU path went): `glitch_blocks`, `film_damage`, `lens_blur`
  (barriers) and `light_leak` (a positioned tap). `film_damage` holds gate weave as a row: one film path.
- No kind where a look of ours exists: halation is a `glow` variant, the directional blur a row of `radial_blur`, sepia
  a `duotone` variant, Movie Grain a `noise` variant, Cinematic a `color_grade` variant.
- All of it is an imitation of a look, said so in the picker's keyword and the note: no film simulation is offered.

## Capabilities

### Modified Capabilities

- `video-fx`: the kinds, the modes, the variants.

## Impact

- `src/plugins/fx-video-stylize` (glitch_blocks, film_damage, light_leak), `src/plugins/fx-video-blur` (lens_blur, the
  radial mode), `BUILTIN_IDS` +4, catalogs ×5, F-FX-007 rows. No format change but a new optional row of `radial_blur`
  (omitted at its default, so a saved file reads as before).
