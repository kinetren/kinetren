---
id: F-FX-001
title: Stylization — Vignette, Pixelate, Halftone, Sharpen
status: done
priority: A
cost: M
category: fx/stylize
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-fx, kt-gpu, kt-app]
invariants: [7, 11]
ui_rules: []
round: "R.47"
point: 334
tests: [328, 329, 330, 332]
depends_on: []
trackers: []
services: []
checked: 2026-09-14
---

# F-FX-001 — Stylization — Vignette, Pixelate, Halftone, Sharpen

## Essence

Four "looks" at the frame: darkening of the corners (vignette), mosaic (pixelation of faces and numbers — and a style), newspaper halftone, sharpness (unsharp). For the editor these are the most common stylisations after
colour correction; pixelate, moreover, is a working tool for "blurring out a face".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Vignette / Pixelate / Sharpen (Video FX) | OFX effects of the event; Vignette has an oval with feathering, Sharpen has one handle |
| Adobe Premiere | 26 | Vignette (Lumetri) / Mosaic / Unsharp Mask | the vignette lives INSIDE Lumetri (colour correction), mosaic and sharpness are separate effects |
| DaVinci Resolve | 21 | Vignette / Mosaic Blur / Sharpen (ResolveFX) | Sharpen has a radius and "coring" (a threshold below which one does not touch — noise protection) |
| Final Cut Pro | 12.3 | Vignette / Censor (Pixelate) / Sharpen | Censor is a ready tool to "blur out" with a shape; halftone is in the set of Comic styles |
| After Effects | 26 | no standard Vignette / Mosaic / CC Half Tone / Unsharp Mask | the vignette is made with a mask on a Solid — the absence of a standard one is considered a known oddity; Half Tone is from the Cycore set |

## The trick behind it

Sharpen in all mature implementations is not a convolution "+sharpness", but an unsharp mask on a BLURRED copy: out = src + (src − blur(src))·amount, and the radius is the radius of that very blur; therefore the sharpness scales with the resolution just like the blur, and Resolve has a coring threshold against sharpening the noise. Pixelate in FCP is "Censor": the product understands that the main scenario is blurring out, and gives a shape/tracking, not just a cell. An honest halftone measures the LOCAL brightness under the dot (the average of the cell); a cheap one — a threshold on the coordinate; the difference is visible on gradients: with the cheap one the dots do not grow smoothly. The vignette of Premiere lives inside colour correction — that is, it is applied AFTER the grade, and this is a deliberate default order.

## Pros

- Pixelate is an integer copy of pixels: byte-for-byte equality between CPU and GPU is achievable (class EXACT), a rare case among stylisations.
- Unsharp reuses our mip downscale of the blur — the sharpness gets K×-invariance of the kind for free (the same law step·K).
- The vignette is a per-point effect with a coordinate: it is absorbed into the merged geometry pass and "zooms" together with the picture inside the chain.

## Cons

- Halftone is a threshold effect: inside a geometry series the supersampling averages the dots; its honest place is a barrier (after the geometry), and this is fixed by the classification.
- Sharpen raises noise and ringing on contours at large amounts — a property of unsharp; the coring is left for later.

## What users say

NO QUOTES: the reconnaissance was carried out from the official documentation and help of five editors from the model's memory (knowledge cut-off 2026-01); a verbatim forum fragment with a link, without live access to the thread, cannot be reproduced — the facts of the table are verifiable from the vendors' manuals listed in the "Sources" section.

## How it fits Kinotrain
CAPABILITY: openspec/specs/video-fx/ — the contract in `spec.md`, the "why" and the paths in the code in `design.md`.
Stylisation — the per-point and barrier kinds of the registry; the vignette — a separate stage of the plan.

## Estimate

| What | Answer |
|---|---|
| Size | M |
| Where the main work is | kt-fx (kernels), kt-gpu (3 passes + taps) |
| Project format | not touched |
| Tests to write | golden cycle; conformance classes; K×-invariance (t303); "blocks are constant", "dispersion grows", "two levels of raster" |
| Depends on | the locks of the first video wave (tap code 4 bits) |
| Breaks compatibility | no |

## Verdict

We take all four in R.47: Vignette — a per-point tap with the stage coordinate; Pixelate — an EXACT barrier (the cell ×K); Halftone — a barrier with a pattern in logical coordinates (threshold THRESHOLD); Sharpen — an unsharp mask over the mip of the blur (step·K). The shape/tracking of "Censor" — we do not take (masks are a separate category MSK); the coring of Sharpen — later, on a complaint.

## Sources

- https://www.blackmagicdesign.com/products/davinciresolve (access date 2026-09-14)
- https://helpx.adobe.com/premiere/using/effects-reference.html (access date 2026-09-14)
- https://support.apple.com/guide/final-cut-pro/welcome/mac (access date 2026-09-14)

**Implemented in R.47** — docs/porting-notes.md items 332/334 (t330/t332); the further life of the feature is there.
