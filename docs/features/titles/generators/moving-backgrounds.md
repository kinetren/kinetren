---
id: F-TXT-008
title: Moving backgrounds — a mesh gradient, plasma, particles, patterns
status: in-progress
priority: B
cost: M
category: titles/generators
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-plugins-builtin, kt-app]
invariants: [7, 8, 11]
ui_rules: [4, 9, 13]
round: features-slow
point: change gen-backgrounds
tests: [t3707, t3708]
depends_on: [F-TXT-003]
trackers: []
services: []
checked: 2026-11-20
---

# F-TXT-008 — Moving backgrounds — a mesh gradient, plasma, particles, patterns

## Essence

The owner (`v26.10` G5, his answer of 2026-10-30 to question 6): "very few generators", all four groups,
"not like in the 90s" — modern looks, styles per kind. The backgrounds group: "moving gradients,
particles, plasma, patterns". The program had a static linear gradient, a four-corner gradient, a
checkerboard and a noise field.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Media Generators › Color Gradient, Noise Texture, Checkerboard | static gradients with keyframes; motion comes from animating the points |
| Premiere | 26 | Essential Graphics templates; Color Matte | a motion background is a downloaded Motion Graphics template |
| Resolve | 21 | Fusion generators: Background, FastNoise, Plasma, Particles (pEmitter) | node tools; Plasma and FastNoise animate by a seethe/phase rate; particles are a full system |
| Final Cut Pro | 12 | Generators › Backgrounds (Blobs, Curtains, Organic, Underwater …), Textures | dozens of animated backdrops, each with a few controls and a colour preset menu |
| After Effects | 26 | Fractal Noise, Turbulent Noise, CC Particle World, Gradient Ramp, 4-Color Gradient | effects on a solid; evolution by expressions or keyframes |

## The trick behind it

- A modern backdrop moves BY ITSELF at a speed: FCP's Blobs or Resolve's Plasma have a rate, not
  keyframes; the person picks colours and a speed, not a path.
- A soft "mesh" gradient (several colour blobs blended by distance) reads as modern; a linear ramp reads
  as the 90s.
- Every frame is a function of the material time, so a backdrop under a cut repeats exactly — the GPU
  draws it per frame from a few numbers.

## Pros

- Four backdrops that move with no keyframes, all on the GPU, all with colours from the eyedropper.

## Cons

- No style presets yet (FCP's colour menus): a generator preset store is the next step.

## What users say

NO QUOTES: no third-party complaint was searched; the owner's own list (G5) is the signal.

## How it fits Kinotrain

CAPABILITY: openspec/specs/generators/ — the contract is in `spec.md`, the "why" in `design.md`.
Four kinds in `src/plugins/generators-basic/src/lib.rs` with WGSL `pixel` texts under its `wgsl/`
(`meshgrad`, `plasma`, `particles`, `pattern`), `live` (they move with `pts`), opaque.

## Estimate

| What | Answer |
|---|---|
| Scope | M — one stage |
| Where the main work is | `src/plugins/generators-basic` |
| Project format | does not touch (plugin rows) |
| Tests that will have to be written | each moves and repeats per moment; colours and rows reach the picture; created from Edit and captioned |
| Depends on | F-TXT-003 |
| Breaks compatibility | no |

## Verdict

Take: four GPU backdrops with a speed and colours; style presets next.

## Sources

- https://support.apple.com/guide/final-cut-pro/ (Generators › Backgrounds, access date 2026-11-20)
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_Manual.pdf (Fusion generators: Plasma, FastNoise, pEmitter, access date 2026-11-20)
- https://helpx.adobe.com/after-effects/ (Fractal Noise, CC Particle World, access date 2026-11-20)
- https://www.vegascreativesoftware.info/us/vegas-pro-help/ (Media Generators, access date 2026-11-20)
