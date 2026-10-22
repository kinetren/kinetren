---
id: F-CMP-001
title: Concatenation of transforms
status: done
priority: A
cost: XL
category: compositing/transforms
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, nuke-17]
we_have: yes
crates: [kt-fx, kt-gpu, kt-engine, kt-model]
invariants: [7, 11]
ui_rules: []
round: R.44
point: 307
tests: [296, 297, 298, 299, 300, 301]
depends_on: []
trackers: []
services: []
checked: 2026-09-13
---

# F-CMP-001 — Concatenation of transforms

## Essence

Consecutive geometric operations (shift, zoom, mirror, grid, 3D perspective) are executed as ONE resampling of the source frame, rather than as a chain of rasterizations "each effect into its own buffer". For the editor: "shrink with a 2×2 grid, then zoom back in" returns a sharp picture — and not the blur of enlarged pixels of an intermediate raster.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | none | Pan/Crop, Track Motion and every OFX effect is a separate raster stage of the pipeline; a chain of transforms does not collapse |
| Adobe Premiere | 26.x | none | a clip has exactly one built-in transform (Motion); there is no concept of a transform chain; a nested sequence is a raster at the size of the sequence (the blur of nests — F-TL-004) |
| DaVinci Resolve | 21 | partly (Fusion) | not on the Edit page; inside Fusion transform nodes are concatenated on the Nuke model (per the documentation, not checked live) |
| Final Cut Pro | 12.3 | none | a built-in transform, one per clip; a compound clip is rasterized at its own size |
| After Effects | 26.x | Collapse Transformations / Continuously Rasterize | a switch on the layer: the transforms of the precomp and the parent collapse into one resample, but the order of application of the layer's masks/effects also changes |

**Outside the core.** Without Nuke this feature cannot be described: concatenation is its term, and the quality rule is formulated there.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Foundry Nuke | 17.0 | Concatenation | adjacent 2D transform nodes (Transform, Reformat, Tracker, GridWarp, STMap…) collapse into one operation; filtering is done once |

## The trick behind it

Concatenation in Nuke is not a speed optimization but a QUALITY rule, and it has three non-obvious consequences. (1) The filter for the whole series is set by the LAST node — the filtering settings of the intermediate transforms are ignored, so compositors deliberately choose the filter at the end of the chain. (2) The series is broken by ANY non-transform node between transforms — even Crop, even a colour Grade — and it is broken SILENTLY: there is no indicator; the break is diagnosed by a suddenly blurred picture; hence the discipline "keep all transforms together, colour before or after". (3) AE has an analogue (Collapse Transformations) — a switch, because there the collapse CHANGES THE SEMANTICS (the order of the precomp's masks and effects), and users turn it on and off; Nuke's concatenation does not change semantics and is therefore always on, with no switch. For us this is a direct argument for "merging without a toggle": if the merged result mathematically equals the step-by-step one (minus losses), there is nothing to choose.

## Pros

- Quality: "filtering unavoidably destroys some image detail" (Foundry's reasoning verbatim: "As filtering unavoidably destroys some image detail, it's
  important to concatenate as many nodes as possible in order to preserve
  image quality"), and a series of N transforms pays this price once, not N times.
- Speed: one resampling instead of N full rasterizations — a merged series is CHEAPER than the step-by-step one (fewer passes and less memory traffic).
- Accuracy: intermediate 8-bit quantizations disappear between the steps.

## Cons

- It works only for operations with a closed mapping "output pixel → entry point of the input"; any pixel operation (colour, blur, grain) is a barrier, and in Nuke it breaks the series silently — a subject of complaints and training breakdowns.
- The AE variant (collapse) pays with a change in the semantics of the order of masks/effects — see the quotes below: "effects … will no longer work quite the same".

## What users say

> Notice how the 3 nodes handling image transformation are all grouped
> together? Nuke is smart enough to combine the Reformat, Transform & Tracker
> nodes into one process, so we only filter our image once.
> — Ben McEwan, benmcewan.com, 22.04.2019,
> https://benmcewan.com/blog/2019/04/22/back-to-basics-a-brief-lesson-on-image-filtering-node-concatentation/

About the price of the AE variant (a thread already pinned in F-TL-004):

> The downside is that effects applied to the precomp layer will no longer
> work quite the same.
> — Walter Soyka, Creative COW, 04.05.2018,
> https://creativecow.net/forums/thread/collapse-transformations-continuous-rasterise-rem/

> Continuous Rasterize can definitely make things go wonky sometimes and
> should be avoided at the design stage whenever possible.
> — Gary Reisman, Creative COW, 03.05.2018,
> https://creativecow.net/forums/thread/collapse-transformations-continuous-rasterise-rem/

## How it fits Kinotrain
CAPABILITY: openspec/specs/video-plan/ — the contract is in `spec.md`, the "why" and code paths are in `design.md`.
Consecutive geometric FX are executed in one pass of inverse mappings; the plan of the chain is built by `plan_chain`, and the GPU mirror is edited in the same commit.

## Estimate

| What | Answer |
|---|---|
| Size | XL — a round with a revision of invariants 7 and 11 |
| Where the main work is | kt-fx (the common inverse-mapping program), kt-gpu (the merged WGSL pass), kt-engine (the post-FX cache key, removal of GpuRequired) |
| Project format | not touched (FxInstance and parameters do not change) |
| Tests that will have to be written | byte-parity of series of length 1; CPU↔GPU conformance of merged series; "a net-unit chain preserves detail"; edge rules; camera-stale cache |
| Depends on | — |
| Breaks compatibility | the picture of multi-geometry chains changes (sharper); sanctioned by R.44 |

## Verdict

We take it (the owner's decision, R.44): merging ALWAYS and without a switch — a series of length 1 without minification is byte-for-byte equal to today's pass, so
there is no question of "when to enable". Together with it: Pan3d on CPU (removing the "3D only on GPU" of R.29) and supersampling by the Jacobian under minification.
DONE in R.44 (items 306–310, tests t296–t301, t66d): recalculation of the goldens was not needed — box supersampling at ×2 arithmetically matched the old bilinear (item 309).

Stages (1) and (2) are DONE in R.45 (items 311–315, tests t302–t306): point-wise Greenscreen/Noise are absorbed into the merged pass as tap ops, non-local
Blur/Glow/Edges/VHS are executed in K× when "compression above + zoom below" (K = quantum of min(zoom of the tail, compression), ceiling constant 4096).

Stage (3) is DONE in R.46 (items 317–323, tests t308–t318): at a net zoom of the chain above 100 % the pre-FX frame is decoded at S×Quality (S — an integer step
1..4 per clip: how much the chain asks × how much the native gives in reserve ×
the buffer ceiling). The squeeze-cap of R.45 finally got an honest source
of detail — the step seeds the compression accumulator. The size of the decode became part of
the ORDER `layers_at`/`wanted_frames`: otherwise prefetch would warm one thing and the composite would read another (t247d on the size axis). Measure: on a 512×288 file in
a 128×72 project with zoom ×2, the contrast of the finished frame grew from 0.0 to 223.0
out of ≈255 (t315, red-proof on the commit before the round).

THE VERDICT IS CLOSED IN FULL: all three stages are done. Only an
optional optimization remains — AVX2 of the merged pass per the 1.3× rule.

## Sources

- https://learn.foundry.com/nuke/content/comp_environment/transforming_elements/node_concatenation.html
  (Foundry documentation "How Nodes Concatenate"; accessed 2026-09-13)
- https://benmcewan.com/blog/2019/04/22/back-to-basics-a-brief-lesson-on-image-filtering-node-concatentation/
  (accessed 2026-09-13)
- https://www.cgchannel.com/2026/02/foundry-releases-nuke-17-0-nukex-17-0-nuke-studio-17-0/
  (Nuke version 17.0, 26.02.2026; accessed 2026-09-13)
- https://creativecow.net/forums/thread/collapse-transformations-continuous-rasterise-rem/
  (quotes about AE collapse; first pinned in F-TL-004)
- The Vegas/Premiere/FCP rows — from the structure of their pipelines and documentation, not checked live; the Fusion concatenation — from Blackmagic documentation, not checked live.
