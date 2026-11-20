# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 16, row G5, his answer of 2026-10-30 to question 6): the
shapes-and-plates group — "star/polygon/arrow, lower thirds, frames, progress bar". The program had a
rounded rectangle, an ellipse and a line.

## What Changes

- Six kinds in the Shapes group, transparent, antialiased on the GPU, positions in fractions of the frame:
  **Polygon** (sides, size, rotation), **Star** (points, inner radius, size, rotation), **Arrow** (from,
  to, thickness, head), **Border frame** (inset, thickness, corner radius), **Lower-third plate** (a bar
  with an accent stripe; its Reveal is a number a curve animates), **Progress bar** (a track and a fill
  that fills by itself with the material time over its fill time).
- Every colour has the eyedropper AND its own alpha: the window gave every colour group the first
  group's alpha row, so a second colour with alpha (the plate's accent, the bar's fill) wrote the
  first one's transparency. Each group now finds its own `<stem>_a`.

## Capabilities

- `generators`: ADDED requirement "The shapes and plates".

## Impact

`src/plugins/generators-basic` (six kinds, `sdf.wgsl` and six WGSL texts), `kt-plugins-builtin`,
kt-app `gen_ui.rs` (`color_alpha_of`), catalogs ×5, the label table, F-FX-007's line, t1048 rows,
t1163's count, t508's alpha coverage; tests t3709–t3710; live `gen_shapes`.
