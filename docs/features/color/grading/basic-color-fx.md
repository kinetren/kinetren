---
id: F-CLR-007
title: Basic color correction as an event — ColorGrade, Duotone, Posterize
status: done
priority: A
cost: M
category: color/grading
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-fx, kt-gpu, kt-app]
invariants: [7, 11]
ui_rules: []
round: "R.47"
point: 332
tests: [328, 329, 330]
depends_on: []
trackers: []
services: []
checked: 2026-09-14
---

# F-CLR-007 — Basic color correction as an event — ColorGrade, Duotone, Posterize

## Essence

Four knobs per event — brightness, contrast, saturation, hue — plus two stylisations by brightness: duotone (two colours: shadows and highlights; sepia and "invert" are its special cases) and posterize. This is NOT full grading (the wheels LGG — F-CLR-001, curves — F-CLR-002): it is what 90 % of clips are fixed with in 10 seconds.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Brightness and Contrast / HSL Adjust / Black and White | a set of simple OFX effects on top of the full Color Grading window |
| Adobe Premiere | 26 | Lumetri Basic (Exposure/Contrast/Saturation/Tint) | the "basic" section of Lumetri — the first tab, before the wheels and curves |
| DaVinci Resolve | 21 | Adjustment knobs of the Edit page | simple sliders on the Edit page, full grade on Color |
| Final Cut Pro | 12.3 | Color Board / Color Adjustments | a simplified panel with global knobs; the duotone effect — through Effects → Duotone |
| After Effects | 26 | Brightness & Contrast / Hue-Saturation / Tint / Posterize | effect by effect; Tint = our Duotone (map black/white to color) |

## The trick behind it

The order of operations inside a "simple" correction is standardised de facto: saturation and hue are computed BEFORE contrast and brightness (in the luma-chroma space), otherwise contrast repaints the hues — and all five follow this sequence in their basic panels. The second detail: AE's Tint (our Duotone) is a lerp by LUMA between two colours, and with amount<1 it gives "colourisation on top" — sepia of any density with one knob; the separate "Sepia"/"Invert" effects are absent from mature sets precisely because they are points in the space of Tint. The third: Posterize in AE computes levels PER CHANNEL (coloured steps), not by luma — by luma you get a "poster", by channels — "a print-style poster", and the standard is channels.

## Pros

- Point-wise, without neighbours and without pixel parameters: they are absorbed into the merged geometry pass (R.45) — grading INSIDE `[grid, grade, zoom]` does not break the series and does not blur.
- Resolution-invariant: preview and export coincide by construction, no K×-barriers are needed.
- Duotone with amount covers sepia, tinting and (with inverted colours) inversion — three "effects from the list" in one.

## Cons

- Posterization inside a geometry series goes through supersampling — the steps are averaged over the taps (softer than a "hard poster"); an honest even quantisation — at the barrier position (the leading/trailing slot).
- A hue rotation in RGB (around the grey axis) is not a colorimetric HSL: at the extreme angles saturated colours leave the orbits of a "designer" HSL. For a basic knob this is an accepted price (Hue in half a dozen OFX sets is built the same way); a full qualifier — F-CLR-003.

## What users say

NO QUOTES: the reconnaissance was carried out from the official documentation and help of the five editors, from the model's memory (knowledge cut 2026-01); a verbatim forum fragment with a link, without live access to the thread, cannot be reproduced — the facts of the table are verifiable against the vendors' manuals listed in the "Sources" section.

## How it fits Kinotrain
CAPABILITY: openspec/specs/video-fx/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Basic colour correction — point-wise tap operations in the chain plan; the order of operations and the luma weights are part of the golden cycle of the registry.

## Estimate

| What | Answer |
|---|---|
| Size | M |
| Where the main work is | kt-fx (taps + kernels), kt-gpu (3 passes + tap arms) |
| Project format | does not touch |
| Tests to write | golden cycle; absorption into the run + "taste" (desaturated / two colours / ≤N levels); conformance classes |
| Depends on | locks of the first video wave (tap code, 4 bits) |
| Breaks compatibility | no |

## Verdict

Take into R.47: ColorGrade (brightness/contrast/saturation/hue, with the order "saturation and hue before contrast"), Duotone (two eyedroppers + amount; sepia and invert are presets of the space, not separate effects), Posterize (per channel, 2–32). The wheels, curves and qualifier remain F-CLR-001/002/003 — this note is not their competitor, but their "first tab".

## Sources

- https://helpx.adobe.com/premiere/using/color-workflows.html (accessed 2026-09-14)
- https://helpx.adobe.com/after-effects/using/color-correction-effects.html (accessed 2026-09-14)
- https://www.blackmagicdesign.com/products/davinciresolve (accessed 2026-09-14)

**Implemented in R.47** — item 332 (t330).
