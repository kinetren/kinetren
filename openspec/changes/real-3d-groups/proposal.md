# Proposal: real-3d-groups

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` R1–R4 (no) and E1 (future). Note F-CMP-005.
> Tests reserved: t3952–t3959.

## Why

The owner, `v26.10` line 152: "Сейчас наш композиционный "движок" основывается на факте обратимости per-pixel
UV-маппинга во фрагментном шейдере. Это накладывает свои ограничения, например, если понадобится необратимые прямые
отображения (настоящие 3d-меши с перекрытием). Хотелось бы сохранить некоторую скорость-гибкость движка и внедрить
настоящий 3d не потеряв в производительности и не усложнив код - что-то выкинув. Ну и тот же эффект параллакса -
хотелось бы и кастомными шейпами делать." And lines 121–124: a study of how professionals assemble 3D scenes.

Today Pan3d places ONE layer in perspective by an inverse homography; layers stack by track order, so two layers
cannot cross in depth, a layer cannot pass behind another that is above it in the track list, and a cut-out shape has
no thickness.

## What Changes

- **3D tracks and 3D groups** (After Effects' rule): a video track gets a "3D" switch. Neighbouring 3D tracks form one
  **3D group**; the group's layers are drawn together by the active camera into one image with a depth buffer, so
  they intersect and sort per pixel by depth, not by track order; the group's image enters the 2D stack at the
  group's place. A 2D track between 3D tracks splits them into two groups (shown on the track headers).
- **Each layer of a group is a textured mesh**: its picture after its own effects is the texture; the geometry is a
  plane placed by the layer's 3D position, rotation and scale (the Pan3d rows, now read as the layer's transform),
  or an **extruded mask** — a mask path given a depth, so a cut-out has thickness and sides (the owner's "кастомными
  шейпами" parallax).
- **Forward rasterization in kt-gpu**: one render pass per group with a depth target, the existing cameras (one rule
  per programme), the same tolerances as the GPU composite. The inverse-UV path stays for every 2D layer and for
  Pan3d outside a group; nothing in the 2D path changes.
- **Parallax in one gesture**: "Split into depth layers…" on an event cuts its picture by its masks into layers of a
  new 3D group, each pushed to a depth the person sets on the preview.
- **Later stages, named**: imported models (glTF) as group members; lights and shadows; depth of field. The bug-tracker
  study the owner asked for (R2–R3) needs network access and is a separate task.

## Capabilities

### Modified Capabilities

- `video-plan`: the 3D group as a node of the plan; forward rasterization with depth; a 2D track splits groups.
- `ui-reach`: the 3D switch on a track header, the group mark, the depth gizmo, "Split into depth layers…".
- `project-format`: the 3D switch and extrusion rows by content.

## Impact

- kt-model: `Track.three_d: bool`; `Mask.extrude: f64` (depth in project pixels, 0 = flat); the layer transform rows
  read from Pan3d's passport when the track is 3D; groups derived from the track order (no stored group).
- kt-engine: `frame_plan` emits `Layer::Group3d { members, camera }` for a run of 3D tracks; each member's 2D chain is
  planned as today and ends in a texture.
- kt-gpu: a mesh pipeline (vertex + fragment, a Depth32 target from the pool), plane and extruded-path meshes built on
  the CPU per revision (cached by the mask's content), drawn back-to-front for alpha with depth test, then resolved to
  the group's RGBA.
- kt-app: the switch, the group's bracket on the headers, the depth handle on the preview, the split gesture, i18n ×5.
- Format: the next free `.kt` version by content. OTIO: a planned row (a foreign editor sees 2D layers).
- Idle: a still group is a kept composite like any layer.

## Questions to the owner

Decided (2026-11-22, `kkt-26.11.20b` B10, line 22): "A+B; переключатель дорожки (Recommended)" — Q1 (a),
intersecting planes and extruded shapes in this change, models and lights later; Q2 (a), a track switch makes the
group.

The questions as asked:

**Q1. What comes first?** Your words: "настоящий 3d … не потеряв в производительности и не усложнив код". The steps:
planes that intersect (A), extruded shapes for parallax (B), imported models (C), lights and shadows (D).
- (a) **A and B in this change, C and D later** (*recommended*: they reuse the layers you already have — pictures and
  masks — and need one new pass).
- (b) A, B and C (glTF models) together.

**Q2. What decides the 3D group — the track or the event?** In After Effects it is the layer's switch; our tracks
already hold the order and the track effects.
- (a) **A track switch: neighbouring 3D tracks form a group** (*recommended*: the group is visible on the headers; an
  event inherits its track).
- (b) An event switch: any event can be 3D, groups formed per moment.
