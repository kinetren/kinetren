---
id: F-TXT-009
title: Shapes and plates — polygon, star, arrow, border frame, lower-third plate, progress bar
status: in-progress
priority: B
cost: M
category: titles/generators
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-plugins-builtin, kt-app]
invariants: [7, 8, 11]
ui_rules: [4, 9, 13, 17]
round: features-slow
point: change gen-shapes
tests: [t3709, t3710]
depends_on: [F-TXT-003]
trackers: []
services: []
checked: 2026-11-20
---

# F-TXT-009 — Shapes and plates — polygon, star, arrow, border frame, lower-third plate, progress bar

## Essence

The owner (`v26.10` G5, his answer of 2026-10-30, question 6): the shapes-and-plates group — "star/
polygon/arrow, lower thirds, frames, progress bar". The program had a rounded rectangle, an ellipse and
a line.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Media Generators › Solid Color, Credit Roll; Shapes via the Bézier mask of a solid | no polygon or arrow generator; shapes are masks |
| Premiere | 26 | Essential Graphics: Rectangle, Ellipse, Polygon (pen); Motion Graphics templates for lower thirds and progress bars | shapes are layers of a graphic clip; lower thirds and progress bars are templates |
| Resolve | 21 | Fusion: sRectangle, sStar, sNGon, sEllipse (shape system); Text+ lower thirds | the shape tools have sides/points and inner radius; lower thirds are templates |
| Final Cut Pro | 12 | Generators › Elements: Shapes (Star, Arrow, Polygon …), Lower Thirds titles | a Shapes generator with a shape menu, fill/outline and drop shadow |
| After Effects | 26 | Shape layers: Polystar (polygon/star with inner/outer radius), Rectangle; Progress via Trim Paths | Polystar is one tool for both; a progress bar is a trimmed path or a template |

## The trick behind it

- A star is a polygon with a second radius (AE's Polystar, Resolve's sStar): the inner radius is the one
  number that makes it a star — so here it is its own kind with that number, the polygon has none.
- A lower-third plate and a progress bar are two-colour shapes (a bar and its accent; a track and its
  fill): each colour has its own alpha.
- A plate's reveal is a NUMBER a curve animates (as the title's reveal), not a fixed animation — the
  person keys its in and out.
- The progress bar fills with the material time over its fill time: the most common use is "a bar that
  runs for N seconds" with no keyframes.

## Pros

- Six shapes with antialiased edges on the GPU, all with the eyedropper, positions as fractions of the
  frame (the preview's size does not move them).

## Cons

- No outline-only mode or drop shadow on the shapes yet (the title has them); a plate without text — the
  text is a Text generator on the track above.

## What users say

NO QUOTES: no third-party complaint was searched; the owner's own list (G5) is the signal.

## How it fits Kinotrain

CAPABILITY: openspec/specs/generators/ — the contract is in `spec.md`, the "why" in `design.md`.
Six kinds in `src/plugins/generators-basic/src/lib.rs`, their WGSL in `wgsl/` over the shared `sdf.wgsl`
(distances, `cover`, `over`); the generator window gives each colour group its own alpha
(`gen_ui::color_alpha_of`).

## Estimate

| What | Answer |
|---|---|
| Scope | M — one stage |
| Where the main work is | `src/plugins/generators-basic` |
| Project format | does not touch (plugin rows) |
| Tests that will have to be written | each stands where its rows say over a transparent frame; the star's valley; the plate's reveal; the bar fills with time; created from Edit and captioned |
| Depends on | F-TXT-003 |
| Breaks compatibility | no |

## Verdict

Take: six GPU shapes and plates; outline mode and a shadow later.

## Sources

- https://helpx.adobe.com/after-effects/ (Polystar, Trim Paths, access date 2026-11-20)
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_Manual.pdf (Fusion shape tools sStar, sNGon, access date 2026-11-20)
- https://support.apple.com/guide/final-cut-pro/ (Generators › Shapes, access date 2026-11-20)
- https://helpx.adobe.com/premiere/desktop/ (Essential Graphics shapes, access date 2026-11-20)
