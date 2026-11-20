# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 16, row G5, his answer of 2026-10-30 to question 6): very
few generators — all four groups, modern looks; the backgrounds group is "moving gradients, particles,
plasma, patterns". The program had a static linear gradient, a four-corner gradient, a checkerboard and
a noise field.

## What Changes

- Four moving backgrounds in the Background group, each drawn on the GPU from a few numbers and moving
  by itself with the material time at its speed:
  - **Moving gradient** — three colour blobs on slow orbits blended softly (a mesh gradient);
  - **Plasma** — travelling sine fields through a palette of three colours, with a scale;
  - **Particles** — soft bokeh discs drifting up over a backdrop: count, size, softness, strength, seed;
  - **Pattern** — stripes, dots, a grid or waves at an angle, drifting: cells, fill ratio, softness.
- Their colours have the eyedropper; every row is captioned in the five catalogs.
- Not here: style presets per kind (a generator preset store) — the next step.

## Capabilities

- `generators`: ADDED requirement "The moving backgrounds".

## Impact

`src/plugins/generators-basic` (four kinds, four WGSL texts), `kt-plugins-builtin` (`BUILTIN_IDS`),
kt-app catalogs ×5 and the label tables, F-FX-007's catalogue line; t1048's rows, t1163's count; tests
t3707–t3708; live `gen_backgrounds`.
