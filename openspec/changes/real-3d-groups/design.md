# Design: real-3d-groups

> DESIGN ONLY, revision 1 (2026-11-20), base `941b25cd`. Q1–Q2 in `proposal.md`; defaults recommended.

## Context

- Pan3d (R.29, R.44): a layer's picture is placed by the camera through an inverse homography inside the fused
  geometry pass (`kt-fx/src/geom.rs` ↔ `geom_fused.wgsl`); the camera is resolved by one rule
  (`video_plan.rs`, `Cam3d`, spec "The camera is resolved by one rule per programme").
- `frame_plan` stacks layers by track rank with weights; morphers are layers; tracks have chains and masks.
- kt-gpu: render pipelines, pooled targets, `FrameTol` tolerances for the GPU composite.

## D1. Groups in the plan

A run of consecutive video tracks with `three_d` (in display order) is one group; a 2D track ends it. `frame_plan`
emits `Layer::Group3d { members: Vec<Member>, cam: Cam3d }` at the group's place; each `Member` = the event's 2D chain
plan (its own effects, masks, conform) ending in a texture + its transform (position x/y/z, rotation, scale, anchor:
the Pan3d rows on that event or its track) + its mesh kind. Track effects of a 3D track apply to the member's texture
(before 3D), track blend modes are not available inside a group (the group's image has the group's first track's
blend into the 2D stack) — named in the track window.

## D2. Meshes

- Plane: 4 vertices in world pixels from the placement rectangle (the one placement function) × the transform.
- Extruded mask: the mask's path flattened to a polygon (the rasterizer's own flattening, 0.25 px), triangulated
  (ear clipping, deterministic order), front and back caps at ±extrude/2 and side quads; the texture maps to the
  front cap by the layer's UV, sides take the edge colour (clamp). Built on the CPU per mask content hash.

## D3. The pass

One render pass per group: a Depth32Float target and an RGBA target from the pool; opaque-alpha fragments write depth;
translucent ones are drawn after, sorted back-to-front by their centroid depth (stable by member order), with depth
test and no write — AE's order. The result is a premultiplied RGBA layer in the 2D stack. Determinism: integer
vertex order and fixed sort keys; across devices within `FrameTol::BILINEAR` (the composite's tolerance).

## D4. Parallax split

"Split into depth layers…" on an event with N masks: N + 1 events on new 3D tracks (one per mask, the rest on the
last), each with the event's chain and one mask, depths spread 0…(N·200 px), one undo step; the preview shows a depth
handle per member while the group is selected.

## D5. Class walk — "a mapping that must be forward, not inverse"

| Mapping | Invertible per pixel? | Path |
|---|---|---|
| 2D affine, corner pin, Pan3d alone | yes | inverse UV (unchanged) |
| crossing planes | no (two layers at one pixel by depth) | D3 |
| extruded shapes | no | D3 |
| lens/distort effects | yes (per-pixel UV) | unchanged |
| a particle generator (future) | no | the same mesh pass (named for later) |

## D6. Idle

A group with no animation is a kept composite; meshes are cached by content; nothing at rest.

## Rejected

- **Replacing the 2D engine with the 3D one** — the owner: "не усложнив код"; 2D layers keep their fused path.
- **A ready-made 3D engine** (bevy, three-d) — a second device, a second resource model; one mesh pass in kt-gpu is
  smaller than an integration.
