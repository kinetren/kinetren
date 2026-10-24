---
id: F-FX-002
title: Distortions — Lens, Twirl, Ripple, Kaleidoscope, Chromatic Aberration
status: done
priority: A
cost: L
category: fx/distort
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-fx, kt-gpu, kt-app]
invariants: [7, 11]
ui_rules: []
round: "R.47"
point: 335
tests: [328, 329, 332, 333]
depends_on: []
trackers: []
services: []
checked: 2026-09-14
---

# F-FX-002 — Distortions — Lens, Twirl, Ripple, Kaleidoscope, Chromatic Aberration

## Essence

Non-linear deformations of the frame: lens barrel/pincushion distortion, a whirlpool, a radial wave, a kaleidoscope — plus chromatic aberration (coloured edges "like glass"). For the editor this is the glitch aesthetic, clip transitions "through distortion", imitation of optics and correction of real lenses (k1 with a minus sign).

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Spherize / Swirl / Wave (Video FX) | an OFX set; the parameters are animated by the event's keyframes |
| Adobe Premiere | 26 | Lens Distortion / Twirl / Wave Warp | Lens Distortion is used in both directions — for stylisation and for correction; Wave Warp animates itself (the phase runs) |
| DaVinci Resolve | 21 | Lens Distortion / Warper / Ripples (ResolveFX) | Warper is a general grid warp; Lens Distortion has an "Undistort" tick (the reverse sign) |
| Final Cut Pro | 12.3 | Fisheye / Twirl / Water Pane; Kaleidoscope | the standard Kaleidoscope with the number of sectors and the angle |
| After Effects | 26 | Optics Compensation / Twirl / Wave Warp / CC Kaleida | Optics Compensation — the reference for lens correction (FOV model); Twirl — angle+radius around a point |

## The trick behind it

The whole family consists of INVERSE coordinate maps: for every output pixel one computes where to take the colour from; therefore in compositors (AE, Nuke) they are concatenated with transforms without intermediate rasterisations — exactly our model of the merged geometry pass, and we have five effects that fit into ONE new primitive with modes. The second detail: in Twirl the angle decays from the centre to the radius quadratically (θ·(1−r)²) — without decay you get a "twisted pancake" with a ragged edge, and all mature implementations decay. The third: the Ripple in AE/Premiere "lives" — the wave's phase animates itself; a static wave looks like glass, not water, therefore the phase must be an animatable parameter. The fourth: chromatic aberration is NOT a geometric effect (the channels move differently, and one inverse map per pixel cannot express that), but a barrier with three samples; in glitch packs it is always placed AFTER the distortions.

## Pros

- One primitive (normalised space + modes) — five effects; each next one is only a formula in four copies of the loop.
- The inverse map is concatenated: `[grid, twirl, zoom]` — one pass, without the smearing of intermediate rasters (the legacy of R.44).
- Lens — a polynomial without trigonometry: CPU↔GPU conformance in the ordinary GEOM class.

## Cons

- The trigonometric modes (twirl/ripple/kaleidoscope) are computed in f64 on the CPU and f32 on the GPU: a shift of the map by hundredths of a pixel gives flips of texels — a new tolerance class by measurement (WARP) is needed, and this is the price of the whole family.
- PerPixel supersampling runs the inverse map three times per pixel — on software GPU (llvmpipe) transcendental functions are noticeably more expensive than affine ones.

## What users say

NO QUOTES: the reconnaissance was carried out from the official documentation and help of five editors from the model's memory (knowledge cut-off 2026-01); a verbatim forum fragment with a link, without live access to the thread, cannot be reproduced — the facts of the table are verifiable from the vendors' manuals listed in the "Sources" section.

## How it fits Kinotrain
CAPABILITY: openspec/specs/video-fx/ — the contract in `spec.md`, the "why" and the paths in the code in `design.md`.
The Warp family — primitives of the chain plan (taps); chromatic aberration — three taps; GPU conformance by the `FrameTol` tolerance.

## Estimate

| What | Answer |
|---|---|
| Size | L — the first non-linear primitive paves the road |
| Where the main work is | kt-fx/geom.rs + geom_fused.wgsl (in one commit), WARP measurement |
| Project format | not touched |
| Tests to write | identity at neutral parameters (bytes); markers by formula; the sector invariant of the kaleidoscope; conformance of the WARP class; PerPixel-ss enabled |
| Depends on | the locks of the first video wave |
| Breaks compatibility | no |

## Verdict

We take all five in R.47: LensDistort (k1/k2, minus = correction), Twirl (quadratic decay), Ripple (an animatable phase — a parameter under a curve), Kaleidoscope (sectors + angle), ChromaticAberration (a barrier, three samples). One universal primitive with modes instead of five kinds — the road is paid once. Resolve's grid Warper — we do not take it (a free grid is a separate big feature).

## Sources

- https://helpx.adobe.com/after-effects/using/distort-effects.html (access date 2026-09-14)
- https://helpx.adobe.com/premiere/using/effects-reference.html (access date 2026-09-14)
- https://www.blackmagicdesign.com/products/davinciresolve (access date 2026-09-14)

**Implemented in R.47** — items 334–335 (t332/t333).
