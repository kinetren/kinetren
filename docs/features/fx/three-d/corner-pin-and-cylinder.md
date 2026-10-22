---
id: F-FX-004
title: 3D transforms — Corner Pin and Cylinder
status: done
priority: A
cost: M
category: fx/three-d
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-fx, kt-gpu, kt-app]
invariants: [7, 11]
ui_rules: []
round: "R.47"
point: 335
tests: [328, 329, 331, 333]
depends_on: []
trackers: []
services: []
checked: 2026-09-14
---

# F-FX-004 — 3D transforms — Corner Pin and Cylinder

## Essence

Corner Pin — fitting a frame "by four corners": a TV screen in the frame, a poster on a wall, a screencast in a laptop — the main tool for inserting an image into perspective. Cylinder — wrapping the frame onto a cylinder (a ribbon, a drum, a "convex screen"). A continuation of the R.29 course "effects in 2D and 3D versions": we already have Pan3d — these are the next two 3D effects.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | Deform / Picture-in-Picture (corners) | a four-cornered deformation of the event; historically "Pan/Crop + Track Motion" |
| Adobe Premiere | 26 | Corner Pin (Distort) | four animatable points; the classic for "a screen in the frame" |
| DaVinci Resolve | 21 | Corner Pin (Edit/Fusion) | simplified on the Edit page, full with tracking in Fusion |
| Final Cut Pro | 12.3 | Distort (corners in the inspector) | the four corners are a property of the clip's transform, not an "effect" |
| After Effects | 26 | Corner Pin / CC Cylinder | Corner Pin is the target of Mocha tracking (tracker data is exported directly into its points); CC Cylinder — radius/rotation, light |

## The trick behind it

Corner Pin is the only "3D" effect that does NOT need a camera: four points fully define the homography (the projective map of a plane), and all the perspective is already encoded in them — therefore AE makes it the target of planar tracking (Mocha writes directly into the corners), and not a 3D layer. The second detail: a degenerate quad (three points on a line, a self-intersection) in mature implementations gives emptiness/transparency, not NaN garbage — the degeneracy is checked before inverting the matrix. The third: CC Cylinder wraps the frame ORTHOGRAPHICALLY along one axis (an asin unwrap), and the edges of the frame remain edges — no cropping; this makes the effect suitable for a "ribbon", not only for a ball.

## Pros

- Corner Pin for us is almost free: the homography already exists (the Proj primitive of the inverse map of Pan3d), only the builder "4 points → matrix" is needed; the GPU shader is not touched at all.
- Both are concatenated with the rest of the geometry (R.44): `[grid, corner_pin, zoom]` — one pass, the corners are animated by curves like any parameters.
- A foundation for future planar tracking (MSK): the target — the corners of Corner Pin, as in AE.

## Cons

- Eight animatable parameters — a long grid in the editor (Pan3d has nine; there is a precedent).
- Cylinder without light/shading (unlike CC Cylinder) — a flat unwrap; light is not in this pass.

## What users say

NO QUOTES: the reconnaissance was carried out from the official documentation and help of five editors from the model's memory (knowledge cut-off 2026-01); a verbatim forum fragment with a link, without live access to the thread, cannot be reproduced — the facts of the table are verifiable from the vendors' manuals listed in the "Sources" section.

## How it fits Kinotrain
CAPABILITY: openspec/specs/video-fx/ — the contract in `spec.md`, the "why" and the paths in the code in `design.md`.
CornerPin and the cylinder are inverse mappings in the geometry series; Pan3d and they are computed on the CPU (reference) and the GPU (within the GEOM tolerance).

## Estimate

| What | Answer |
|---|---|
| Size | M (Corner Pin — S, Cylinder pulls in the F-FX-002 primitive) |
| Where the main work is | kt-fx/geom.rs (builder + mode) |
| Project format | not touched |
| Tests to write | default == identity bytes; markers "corners → positions"; a degenerate quad is transparent deterministically; the edges of the cylinder are invariant |
| Depends on | F-FX-002 (Cylinder — a Warp-primitive mode) |
| Breaks compatibility | no |

## Verdict

We take both in R.47: CornerPin — through the existing Proj (without WGSL changes; the default corners pass the step bit-exactly), Cylinder3d — a mode of the Warp primitive (an orthographic asin unwrap along X; the edges remain edges). The light/shading of the cylinder and binding to a tracker — not in this pass (tracking is the MSK category).

## Sources

- https://helpx.adobe.com/after-effects/using/distort-effects.html (access date 2026-09-14)
- https://borisfx.com/products/mocha-pro/ (access date 2026-09-14)
- https://www.blackmagicdesign.com/products/davinciresolve (access date 2026-09-14)

**Implemented in R.47** — docs/porting-notes.md items 333/335 (t331/t333); the further life of the feature is there.
