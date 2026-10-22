---
id: F-FX-003
title: Radial/Zoom Blur and Drop Shadow
status: done
priority: B
cost: M
category: fx/blur
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-fx, kt-gpu, kt-app]
invariants: [7, 11]
ui_rules: []
round: "R.47"
point: 334
tests: [328, 329, 332]
depends_on: []
trackers: []
services: []
checked: 2026-09-14
---

# F-FX-003 — Radial/Zoom Blur and Drop Shadow

## Essence

Two directional derived blurs: a blur "from the centre" (zoom — rays outward, spin — along an arc) for speed/impact/sleep, and a shadow under a layer with
alpha — the basic tool of styling titles and keyed (cut-out)
objects. Both are built on our existing blur.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Radial Blur / Drop Shadow (Video FX) | Radial has a choice of zoom/spin and a centre; Drop Shadow — offset/colour/blur |
| Adobe Premiere | 26 | Directional/Radial Blur / Drop Shadow | Drop Shadow honestly states in the help "works on alpha — is not visible on an opaque clip" |
| DaVinci Resolve | 21 | Radial Blur / Zoom Blur / Drop Shadow (ResolveFX) | zoom and spin are separate effects; the shadow has an independent blur quality |
| Final Cut Pro | 12.3 | Directional (no radial among the standard ones) / no separate Drop Shadow effect | the shadow is built into titles and the transform (the Drop Shadow section of the inspector) |
| After Effects | 26 | Radial Blur / Drop Shadow | the reference for both; Radial Blur has Amount+Center+Type(Spin/Zoom); Drop Shadow — light/distance/softness |

## The trick behind it

Radial blur is everywhere implemented with a finite number of samples along the ray/arc
(AE even shows "anti-aliasing: low/high" — this number of taps), and the number of
taps is FIXED and does not depend on the amount — otherwise the cost would jump with the
handle; therefore our N=16 is stated in both executors — this is the norm of the industry, not a simplification. Drop Shadow is an effect ABOUT ALPHA: Premiere documents directly "not visible on a full-frame opaque clip" — a correct implementation begins with an early exit on the absence of alpha, and an honest
shadow is "a blurred copy of the alpha, tinted and placed UNDER the layer"
(order under, not over). FCP keeps the shadow in the transform inspector, not as an
effect — because 90 % of its uses are titles.

## Pros

- Both reuse what is ready: radial — bilinear samples (bilinear_into),
  the shadow — the mip downscale of the blur; not a single new mathematical primitive.
- The fixed N=16 makes the view invariant to the K× resolution barrier
  for free (the fractions of the frame are scaled by the buffer itself).
- A shadow under a key (`[greenscreen, drop_shadow]`) — a ready scenario
  for styling cut-out objects, which our key lacked.

## Cons

- 16 samples per pixel — the most expensive CPU/llvmpipe barrier among the new ones;
  the cost is measured and recorded.
- A shadow on a layer without alpha is meaningless — the early exit must be
  agreed between the CPU (per frame) and the GPU (per chain accumulator),
  a divergence = different pictures from the backends.

## What users say

NO QUOTES: the reconnaissance was carried out from the official documentation and help of five editors from the model's memory (knowledge cut-off 2026-01); a verbatim forum fragment with a link, without live access to the thread, cannot be reproduced — the facts of the table are verifiable from the vendors' manuals listed in the "Sources" section.

## How it fits Kinotrain
CAPABILITY: openspec/specs/video-fx/ — the contract in `spec.md`, the "why" and the paths in the code in `design.md`.
The radial blur and the shadow are kinds of the registry with a golden row; the shadow requires an alpha sample in the thumbnail.

## Estimate

| What | Answer |
|---|---|
| Size | M |
| Where the main work is | kt-gpu (2 passes), kt-fx (2 kernels) |
| Project format | not touched |
| Tests to write | the shadow is visible only with alpha (a pair of cases + a passthrough hash); the centre of the radial is sharp; conformance of zoom/spin; t303 invariance |
| Depends on | the locks of the first video wave |
| Breaks compatibility | no |

## Verdict

We take both in R.47: RadialBlur (amount/centre/spin flag, N=16 stated in
both executors), DropShadow (offset in whole px, radius, opacity, tint,
an early exit without alpha, composite UNDER). The directional (linear) blur —
we do not take: it overlaps the radial zoom in terms of its place in editing, and it can be
added later as a mode.

## Sources

- https://helpx.adobe.com/after-effects/using/blur-sharpen-effects.html (access date 2026-09-14)
- https://helpx.adobe.com/premiere/using/effects-reference.html (access date 2026-09-14)
- https://www.blackmagicdesign.com/products/davinciresolve (access date 2026-09-14)

**Implemented in R.47** — docs/porting-notes.md item 334 (t332); the further life of the feature is there.
