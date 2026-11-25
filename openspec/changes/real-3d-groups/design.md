# Design: real-3d-groups

> IMPLEMENTATION-READY (spec-engine, revision 2, 2026-11-25, orch `fc60a5d9`). The owner answered Q1 (a), Q2 (a) (kkt-26.11.20b B10). Code is named by function and file.

## Context (what exists)

- **Pan3d** is a geometry plugin (`plugins/fx-video-distort/src/pan3d.rs`): it takes a canvas-sized picture as a quad `w × h` world pixels and gives the inverse homography of `pan3d_forward(camera, out, pos, rot, scale, anchor,
  quad)` to the fused geometry pass. World: project px × `world_scale`, X right, Y down, Z into depth, rotations ZXY; the camera at its auto distance shows a layer at z = 0 one to one. Both are frozen by the file format.
- **The camera** is resolved once a frame by one rule (`resolve_cam3d_in`): the chain's pinned one, else the active one.
- **The plan**: `build_plan` walks a lane's active events and pushes `PlanLayer::{One, Dissolve}`; a track with a chain, a level, a blend or masks is one Nest layer (`VideoTrackStage`, `track_stage_layer`).
- **The executor**: every texture is `Rgba8Uint` with no sampler; `resolve_plan` makes `Job::{Single, Dissolve}` with `ChainSpec` (its source may be a nested job list). `FrameTol::GEOM` is the silhouette's tolerance.
- **Masks** are rasterized on the processor (`kt_fx::mask`), evaluated in the plan; `Mask` has mode, invert, opacity, feather, expansion, curves.

## D1. The group

A run of neighbouring video lanes whose tracks have `three_d` is one group; a lane without it ends the run. `build_reel` lays the groups out once per build (`VideoGroup3d { lanes, per lane: events range, LayerIndex, the track's chain and masks }`). `build_plan`, at the first active
event of a group's lane, pushes ONE `PlanLayer::Group { members, cam, blend, alpha_k }` in the group's place and skips the others, as a track stage does. A group of one lane is a group.

**What the group takes from its tracks**: the blend of its top track with a blend other than Normal, else Normal (the group is one layer of the 2D stack; a track's own blend inside a group has nobody under it at its depth) — the track window says so on the others. Each track's level
multiplies its members. A track's chain and masks run on each member's picture before it is placed. A stream device on a 3D track's chain is refused with its reason (`track-stream-stage`'s stage is the track's composite, which a 3D track has not).

## D2. A member

```rust
struct Member<'a> { plan: FramePlan<'a>, place: Place3d, mesh: MeshRef, alpha_k: u32 }
```
- `plan`: ONE layer of the event as the 2D stack would draw it over nothing (its chain, conform, masks, fades, opacity), at `M ×` the canvas, with the event's Pan3d slot replaced by the identity geometry (`stream::identity_slot`'s way: the indices stay). The track's chain, where there is one, wraps it as a Nest layer.
- `place`: the rows of the event's LAST active Pan3d of the trunk at the frame (curves resolved): position, rotation, scale, anchor; none → the defaults. **Where it stands in the chain does not matter in a group**: the transform is the layer's, the other slots run on its picture in their order. The
  last Pan3d of the track's chain is a parent transform for all of the track's members.
- **The camera** is one for the group: the programme's active one at `t`. A camera pinned in a member's Pan3d is not read there, and its row says "the group's camera draws it".
- **A transition pair** is one member: its picture is the pair's `Dissolve` (both events' Pan3d removed), its place the two places' rows mixed by the transition's progress (position, scale, anchor straight; angles straight in degrees, as the rows are curves). A morpher's pieces, overlapping
  events and reads need nothing of their own.
- **Its size**: `M` = `ss_taps` of the largest magnification of the event over its length (the same sampling as `clip_head_zoom`, with the member's place and the group's cameras), one per event, capped by `MAX_INTER_DIM`: a plane brought three times nearer is drawn from a picture three times larger, as Pan3d alone
  does. It is the event's decode quality too: `layers_at`, `wanted_frames`, `video_decode_qualities` ask `quality × M` (invariant 4). A 3D switch is an edit of the plan.

## D3. Meshes

- **Plane**: the canvas quad.
- **Extruded mask**: every picture mask of the event with `extrude > 0`, mode Add and not inverted gives one prism: its outline at the frame (curves resolved, the rasterizer's own flattening at 0.25 px of the project), caps at ± half the extrusion, sides between. The picture is the event's WITHOUT those masks; the other masks cut
  it as today. Feather, expansion and opacity of an extruded mask are not geometry: its row says so. An outline that crosses itself has no inside to triangulate: the event is a plane cut by the mask as today, and the mask's row says "it crosses itself: no thickness".
- Triangulation: ear clipping in f64 over the outline's points in order, ties by the lower index (deterministic); texture coordinates are the point's place on the canvas; sides take the picture half a pixel inside the edge. Built on the processor, kept by the hash of the evaluated outline and the extrusion (an LRU of 64).
- Vertices are f32 world positions; the matrix is made in f64 and rounded once.

## D4. The pass (kt-gpu `mesh.rs`)

1. Each member's jobs are resolved and drawn into its canvas (`Rgba8Uint`, straight), as a nest's are.
2. One small pass per member writes it premultiplied into an `Rgba8Unorm` texture with a full mip chain (filtered halving draws): a plane far away is minified, and without mips it shimmers.
3. One render pass per group: `Rgba8Unorm` and `Depth32Float` at 4 samples (both are guaranteed at 4 by WebGPU), resolved to one sample, then written back to the pool's `Rgba8Uint` straight, as `Unpremul` does.
   - **The matrix** is `pan3d_forward`'s model, view and projection with the third column kept: a new `pan3d::matrix(camera, out, place) -> [[f64; 4]; 4]`, from which `pan3d_forward` is derived (one text; t298 and t187 hold the homography's bits). Depth is reversed and infinite: `near / w`, near = 1 project px; an orthographic
     camera writes its `v.z` scaled over ±10⁵ px.
   - **Two draws of every member**, in the order of their distance: first the fragments with alpha 255, depth test and write, front to back; then the rest, back to front by the mesh's centroid (ties: the lane order, then the event's start), depth test, no write, premultiplied over. AE's order; two translucent
     members that cross are sorted as wholes, and the note says so.
   - Texture level and bilinear are the sampler's (trilinear, no anisotropy): `FrameTol::MESH`, measured on the software adapter against its stored frames and on a card, is set by the executor at ×2 of what is seen, as every class was; two runs on one device are byte-equal.
4. The result is the layer: `alpha_k`, the blend and the stack go on as for `Job::Single`.

A device that refuses 4 samples of either format (none known) runs one sample and says so in the machine facts. The pipelines are compiled with the project (`prewarm`: "a project with a group"), timed by `Slot`, cut by the arbiter at member boundaries. Memory: 4 × 8 bytes a pixel of the canvas (66 MB at 1080p), pooled,
counted `Transient`; 4K on a small card: the tight render's rule (`kt_gpu::memory::tight`) counts it before the start.

## D5. Every reader of the plan

`PlanLayer::Group` is hashed by `put_plan` (members' plans recursively, places bit by bit, the meshes' hashes, the camera), walked by `FramePlan::{starved, durable}`, `render_cache::{plugins_of, put_rows}`, `picture_key` (cached-final-render), `frame_meta`, the cost estimate, `absent_layers`, the stream walkers (a member's event may carry its own
Datamosh: its picture is an ordinary layer), `kt-cli probe` ("group of 3: …").

## D6. Parallax split

"Split into depth layers…" on a video event with N ≥ 1 picture masks: N + 1 new video tracks over its own, switched 3D, named after the masks; on each a copy of the event (curves, transitions' places and the morpher's reach as a paste gives) with one mask, the last with all of them inverted; Pan3d added at the end of each chain, depths 0, 200, … project px, scaled so that each
keeps its size from the default camera; the former event goes; its sound partner keeps its link to the nearest layer. One undo step. Refused with a reason: no masks, a locked track, a reel's public parameters addressing the event.

**The depth handle**: a selected member shows a vertical handle at its centre; a drag moves Pan3d's `pos_z` (added to the chain by the same step when there is none), with the curve's key rule of every gizmo (`geomctl`).

## D7. Model, format, interface

- `Track.three_d: bool` in `TrackProps`; `Mask.extrude: f64` a mask row (curves, links). `PROJECT_FILE_VERSION_GROUP3D`, by content. OTIO: both in `metadata["kt"]`, a row in "What OTIO export loses".
- A track's header has the switch (video tracks; a bracket joins the headers of a group); the node editor of an event on a 3D track marks Pan3d "places the layer".
- A project with no 3D track takes none of these branches: keys and bytes as before (GOLDEN_VIDEO).

## D8. Class walk — "a mapping that must be forward, not inverse"

| Mapping | Invertible per pixel? | Path |
|---|---|---|
| 2D affine, corner pin, Pan3d outside a group | yes | inverse UV (unchanged) |
| crossing planes, extruded shapes | no | D4 |
| lens and distort effects on a member | yes | in its picture, before the placing |
| volumetric transitions | yes (panels) | unchanged; between two members: in the pair's picture |
| a reel instance on a 3D track | — | a member like any event; the reel's own groups are drawn inside it |
| a generator, a title | — | a member |
| the preview's frame and safe areas, hit-testing of events (`centerctl`, the Viewer) | 2D rects | a member's hit test is its projected quad; the Viewer's cut shows the member's picture before the placing |
| thumbnails, strips, posters | the same plan | — |
| masks drawn on the preview for a member | canvas of the event | drawn through the member's projection; editing an extruded outline is done flat (the mask panel's own view), said there |
| particles, lights, models (C, D) | no | the same pass, later |

## D9. Idle

A group with no animation is a kept composite; meshes are cached by content; nothing at rest; the depth handle is drawn from numbers in the plan.

## Rejected

- **Replacing the 2D engine with the 3D one** — the owner: "не усложнив код"; 2D layers keep their fused path.
- **A ready-made 3D engine** (bevy, three-d) — a second device and resource model.
- **Integer textures in the mesh pass** — no blending, no resolve, no filtering: each would be written by hand for one pass.
- **Per-pixel sorting of translucent members (linked lists)** — an unbounded buffer per pixel for a case AE does not sort either.
- **A pinned camera per member** — two cameras in one depth buffer have no common depth.
