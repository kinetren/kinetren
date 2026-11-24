---
id: F-CMP-005
title: Real 3D scenes — intersecting layers, depth, meshes and parallax shapes
status: candidate
priority: B
cost: XL
category: compositing/3d
editors: [after-effects-26, resolve-21, nuke-17, vegas-pro-2026, premiere-26]
we_have: partial
crates: [kt-model, kt-engine, kt-gpu, kt-fx, kt-app]
invariants: [7, 11, 12]
ui_rules: [17, 22]
round: null
point: null
tests: []
depends_on: [F-CMP-004]
checked: 2026-11-20
---

# F-CMP-005 — Real 3D scenes — intersecting layers, depth, meshes and parallax shapes

## Essence

Our 3D is Pan3d: one layer at a time is placed in perspective by a camera, by an inverse homography in the fragment
shader, and layers are stacked by track order. That cannot show two layers that CROSS each other in depth, a layer
behind another that is above it on the timeline, a shape with thickness, or a parallax made of many cut-out planes
at different depths. The owner names the limit himself: the engine rests on the invertibility of a per-pixel UV map,
and real 3D (meshes that overlap) needs forward mappings — without losing speed and without growing the code.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26 | 3D layers, cameras, lights; the Advanced 3D renderer; 3D model import (glTF/OBJ) | consecutive 3D layers form one 3D group rendered together with a depth buffer — they intersect and sort per pixel; a 2D layer between them splits the group (a "render break"); since AE 24 the Advanced 3D renderer draws imported models with image-based lighting [NOT VERIFIED live] |
| DaVinci Resolve | 21 (Fusion) | Shape3D, ImagePlane3D, Merge3D, Camera3D, Renderer3D, FBX/USD import | a node graph builds a 3D scene from image planes and meshes; one renderer node draws it to an image with depth (OpenGL or software) [NOT VERIFIED live] |
| Nuke | 17 | Card, ScanlineRender, the new 3D system (USD) | cards (textured planes) and geometry in a scene, rendered by a scanline renderer with a camera; projections of footage onto geometry for parallax [NOT VERIFIED live] |
| VEGAS Pro | 2026 | 3D Track Motion, compositing parent/child | layers in 3D space per track with a parent's mode; no mesh, no per-pixel intersection between tracks [NOT VERIFIED live] |
| Premiere | 26 | Basic 3D (an effect) | one layer rotated in perspective; no scene [NOT VERIFIED live] |

## The trick behind it

**1. A 3D group is rendered together, then composited as ONE layer.** AE's rule: neighbouring 3D layers are drawn
into one image with a depth buffer (so they intersect and sort per pixel), and that image enters the 2D stack where
the group stands. A 2D layer between them ends the group. Everything 2D — blend modes, track effects, morphers —
keeps working around the group.

**2. A layer in 3D is a textured mesh, and a plane is the simplest mesh.** The layer's composited picture (after its
own effects) is the texture; the geometry is a plane, an extruded shape (a mask path given depth), or an imported
model. Forward rasterization with a depth buffer handles any of them; the inverse UV path handles only the plane.

**3. Parallax is many planes at different depths seen by one moving camera.** Cut a photo into layers by masks, push
each to a depth, move the camera — the "2.5D" look. With real depth sorting the cut-outs occlude each other
correctly when the camera moves.

## Pros

- Layers that cross, shapes with thickness, true parallax, models — without a separate 3D program.
- The 2D engine stays as it is; the 3D group is one more kind of layer source.

## Cons

- Forward rasterization differs across GPUs by an LSB or two at edges: determinism is "within tolerance", as the GPU
  composite already is (invariant 11 for video).
- Lights and shadows are a large second stage.

## What users say

The owner, `docs/ktprompts/v26.10.txt` line 152: "внедрить настоящий 3d не потеряв в производительности и не
усложнив код - что-то выкинув. Ну и тот же эффект параллакса - хотелось бы и кастомными шейпами делать"; lines
121–124: a feature study of professional editors in assembling complex 3D scenes, their bug trackers, what people
ask for and complain about. Earlier (R.29): "в направлении After Effects … и 2d и 3d в проекте".

NO QUOTES: no web access in this spec session (2026-11-20). The search the owner asked for (R2: dump the bug trackers
of AE, Fusion, Nuke, Blender's compositor) needs the tracker tools of `docs/features/_trackers.md` with network access;
it is a task for a session that has it. The editor rows are [NOT VERIFIED] for the same reason.

## How it fits Kinotrain

### What already exists

- Pan3d and cameras (R.29): `CameraEvent` on camera tracks, `Cam3d` resolved by one rule
  (`src/libs/kt-engine/src/video_plan.rs`), the inverse homography in the fused geometry pass
  (`src/libs/kt-fx/src/geom.rs` ↔ `geom_fused.wgsl`).
- Layers composited by track order with weights (`frame_plan`), masks with paths (`src/libs/kt-model/src/mask.rs`).
- kt-gpu render pipelines and pooled targets (`src/libs/kt-gpu/src/engine.rs`).

### What is missing

- A 3D group, a depth buffer, forward-rasterized meshes, extruded shapes, a model loader.

### What hinders

- The fused geometry pass assumes one layer → one inverse map; a group must be planned as one source with many
  members.

### Which invariants it touches

- **7** — the group is a node of the one plan; preview and export draw it the same way.
- **11** — video within `FrameTol` across devices, byte-equal on one device.
- **12** — a static 3D group is a kept composite like any other.
- CAPABILITY: openspec/specs/video-plan/

## Estimate

| What | Answer |
|---|---|
| Scope | XL |
| Where | kt-model (3D switch on a track, depth, extrusion), kt-engine (the group in the plan), kt-gpu (the depth render pass), kt-app (the switch, depth gizmo) |
| Project format | yes, by content |
| Tests | two crossing planes intersect along the line; a 2D layer splits the group; parallax occludes correctly |
| Depends on | F-CMP-004 (the composite as a graph) |
| Breaks compatibility | no (Pan3d stays) |

## Verdict

Take it in stages: (1) a 3D group of planes with a depth buffer under the existing cameras (layers intersect and
sort per pixel); (2) extruded mask shapes as parallax pieces with thickness; (3) imported models (glTF); lights and
shadows after. The bug-tracker study (R2–R3) is a separate task for a session with network access.

## Sources

- `docs/ktprompts/v26.10.txt` lines 121–124 (R1–R4), 152 (E1); registry `v26.10-index.md` R1–R4, E1.
