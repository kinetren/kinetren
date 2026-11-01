# Design: shape-masks

## Context

The reconnaissance (F-MSK-001, F-MSK-002, 2026-09-07) said: no geometry in the model, no
rasterizer, no gesture over the frame, a GPU with one 128-byte uniform and three textures, a
passport that was a static slice. Since then the program changed under the notes:

- the passport is built at run time (R.59), a curve can be a link or a formula (R.61/R.67);
- the R.64 transform frame exists: `kt-app/src/geomctl.rs` maps screen ↔ canvas through
  `kt_fx::geom::geometry_placement` — the same function the composite places the layer with;
- the event has the leading "Geometry" node (`EventGeometry`, R.62) whose doc comment already
  reserves "a place for a mask: crop → mask → placement";
- the track is an object with a chain, a level and a blend mode (R.62B), composited as a nested
  layer (`track_stage_layer`);
- the plan has a wet/dry mechanism — the morpher's `LayerMix { amount, from }`: a snapshot of the
  dry side at `from`, a mix at the slot, on the CPU (`apply_chain_with_mixes` →
  `blend_frames_cpu`) and on the GPU (`FxOp::Snapshot`/`FxOp::Mix`, `layer_mix.wgsl`, T2 unused).

The owner's scope is fixed by the batch (kkt-26.11.01 S1) and the orchestrator's brief: shapes
(rectangle, ellipse, Bézier with the pen on the preview), feather with falloff, expansion,
opacity, invert, four modes, several masks in an ordered list, animation through the existing
properties with the key only by gesture, application to the event's picture, to an effect and to
a track, one composition path with a CPU byte reference and the GPU within `FrameTol`, masks in
`.kt` with old files loading.

## Goals / Non-Goals

**Goals:** the classic mask a person knows from AE/Premiere/Resolve, edited on the picture; a mask
that follows the event through its Geometry node; "limit an effect to a region" without
duplicating the event; addresses for the future tracker; byte stability of everything without
masks.

**Non-Goals:**
- variable feather along the contour (AE Mask Feather tool, Fusion per-point feather) — a second
  geometry with its own keys (F-MSK-001 trick 2); later, the model leaves room (a vertex is a
  struct);
- Lighten/Darken/None modes of AE (F-MSK-001: they differ from Add/Intersect only on soft masks;
  the owner asked for the four classic ones);
- a shared contour object referenced by several slots (F-MSK-002 verdict) — replaced by LINKS: a
  mask row can be driven by a row of another mask (see D6); a "paste as linked" verb is a later
  convenience;
- track mattes (a neighbour layer's alpha, F-MSK-003) and the magic wand (G1);
- curves on track masks: a track has no material, its time domain is the next piece of work
  (automation, "Track properties — static"); a track mask is static and a curve on it is refused
  loudly with the same reason.

## Decisions

### D1. The model: one `Mask` type, three owners

```rust
pub struct Mask {
    pub id: MaskId,                 // u32, unique within its list; never reused in the list
    pub name: String,               // "Mask 1", unique within its list (the UI calls unique_name)
    pub shape: MaskShape,
    pub mode: MaskMode,             // add | subtract | intersect | difference
    pub invert: bool,
    pub opacity: f64,               // 0..1
    pub feather: f64,               // px of the mask's reference frame, 0..1000
    pub falloff: Falloff,           // smooth | linear
    pub expansion: f64,             // px, −1000..1000 (negative — choke)
    pub auto: Vec<ParamCurve>,      // curves on this mask's rows, the owner's domain
}
pub enum MaskShape {
    Rect { cx, cy, w, h, rot, round },   // normalized centre/size, degrees, roundness 0..1
    Ellipse { cx, cy, w, h, rot },
    Path { verts: Vec<Vertex> },          // closed; ≥ 3 vertices
}
pub struct Vertex { pub id: u32, pub x, y, ix, iy, ox, oy }   // tangents RELATIVE to the vertex
```

Owners: `ClipEvent.masks: EventMasks { picture: Vec<Mask>, slots: Vec<Vec<Mask>> }` (the slot lists
parallel to `fx`, shorter = no masks, the same convention as `fx_bypass`/`fx_auto`; ONE field on the
event, so the 180 struct literals of the tests gained one line), `Track.masks`. All fields are
`#[serde(default, skip_serializing_if = …)]`: a project without masks serializes byte for byte as
before (t148 golden, t811/t822).

Why one type: the three places are one concept for a person ("the mask of …"); one passport, one
rasterizer, one widget, one validator. Why per-slot lists rather than one event list with a
"target": a target index would have to be remapped on every reorder anyway, and the person's
picture (Premiere: each effect has its masks) is a list per effect. Why tangents relative to the
vertex: moving a vertex (by hand or by a tracker driving only `x/y`) carries its handles, as in every
editor.

Rejected — masks inside `FxInstance`/`PluginSlot`: it changes the serde form of every slot
(frozen by t148) and makes a mask a property of the effect's kind. Rejected — a mask as an effect in
the chain (VEGAS/Shotcut "mask filter … apply"): its region is a prefix of the chain, two masks
intersect, and "the second of five effects" cannot be said (F-MSK-002).

### D2. Coordinates and units

- **Normalized to the reference frame:** x ∈ 0..1 across its width, y ∈ 0..1 down its height
  (values outside are legal — a mask can extend past the frame). The reference frame of an event
  and of a slot mask is the event's SOURCE frame (the input of the Geometry node, uncropped); of a
  track mask — the project canvas.
- **Rotation and circles are aspect-correct:** shapes are generated in reference PIXELS
  (`x·W_ref`, `y·H_ref`; the display aspect includes the pixel aspect) and only then mapped to the
  raster. An ellipse with `w·W_ref == h·H_ref` is a circle on the screen.
- **Feather and expansion are in px of the reference frame** — what a person sees at 100 %. On a
  raster, they are scaled by the raster's px per reference px (the square root of the affine's
  determinant), so the preview at ¼ and the export at 1/1 give the same look (t1267).

Rejected — raster pixels (the preview size differs from the export, F-MSK-001), native source pixels
for coordinates (a relink or a proxy of another resolution would move the mask).

### D3. Where each mask is applied (the plan)

The engine materializes masks into the event's chain as INTERNAL slots, as it already does for the
leading Geometry node and the morpher's `LayerMix` (`materialize_lead`):

- **The picture:** `FxInstance::MaskAlpha { masks, follow_lead: true }` right AFTER the leading
  Geometry node: the frame's alpha is multiplied by the coverage. The coverage is rasterized in the
  node's OUTPUT raster with the path mapped through the node's placement (`geometry_placement`,
  the very function the composite and the R.64 frame use): this is "crop → mask → placement" with
  the mask rasterized at the output resolution (no resampling of a soft edge), and a mask drawn
  around a face stays on the face when the event moves, scales or rotates.
- **An effect slot `i`:** `FxInstance::MaskMix { from: i, masks, follow_lead }` right after the
  slot: the frame at `from` (the slot's input) is snapshotted, the slot runs, and the two are mixed
  per pixel by the coverage — the existing wet/dry mechanism of `LayerMix` with a coverage instead of
  a scalar. `follow_lead` is true when the slot stands after the leading node (its raster is the
  canvas, the path goes through the placement), false before it (the source raster, the path is
  scaled to it).
- **A track:** `FxInstance::MaskAlpha { masks, follow_lead: false }` appended to the track
  stage's chain (after the track's own effects, before level × alpha and the blend mode);
  `Track::needs_video_stage` is true when the track has masks.

Masks are resolved at the frame's material tick when the layer is built (`Mask::at`), so the slots
carry STATIC shapes; the raw masks with curves enter the post-FX hash (`fx_chain_hash` hashes the
materialized chain's Debug form plus `masks`/`fx_masks`; the track stage's `content` hashes its
masks). An event without masks gets no slots — the chain, the plan, the hash and the bytes are those
of today (t1273).

The planner (`kt-fx::plan`) treats `MaskMix` exactly like `LayerMix` (a stop, a snapshot at
`from`, a mix at the slot) and `MaskAlpha` as a non-geometric barrier: both end a fused geometry
window, so a geometry series is never split by anything but a mask the person placed. A mask stop
at index 0 (a masked slot before everything) forbids a decode step on that clip, as a morpher stop
does.

Why at the executor's raster and not one canvas mask applied at the composite: a slot before the
Geometry node runs on the source raster, a slot after it — on the canvas; the mask must cover the
same CONTENT in both, and the only honest way is to rasterize where it is applied. "Once per frame"
holds per application point: each masked owner is rasterized once per frame per backend.

### D4. The rasterizer (kt-fx `mask.rs`) — one CPU function, the GPU uploads its bytes

`rasterize(masks, affine, w, h) -> Coverage { w, h, cov: Vec<u8> }`:

1. Each shape becomes one closed polyline in raster px: a rectangle with roundness is four lines
   and four quarter-arcs, an ellipse four cubic Béziers (κ = 0.5522847498), a path its cubic
   segments; cubics are flattened by a deterministic subdivision (the number of steps from the
   control-polygon length, ≤ 0.25 px chord error). Rotation uses `kt_plugin_api::math` (frozen
   `fsin/fcos`, t1066); no other transcendental function is called.
2. **Inside** by the nonzero winding rule at pixel centres, by a scanline over the edge table
   (O(rows × crossings)).
3. **Signed distance** to the polyline, computed ONLY in a band around the contour of half-width
   `feather/2 + |expansion| + 1.5` px (each edge visits the pixels of its own expanded bounding box,
   keeping the minimum): O(perimeter × band) instead of O(pixels × edges).
4. **Coverage of one mask:** `d` = signed distance (inside positive) + expansion;
   `c = ramp(d)`, where without feather `ramp = clamp(d + 0.5, 0, 1)` (one-pixel antialiasing)
   and with feather `f` it is `falloff(clamp(0.5 + d / f, 0, 1))` — the feather is centred on the
   contour like AE's; falloff `linear` is the identity, `smooth` is smoothstep. Then invert
   (`1 − c`) and opacity (`× opacity`).
5. **Modes, top to bottom,** as the soft forms of the Boolean operations: add `a + m − a·m` (OR),
   subtract `a · (1 − m)` (AND NOT), intersect `a · m` (AND), difference `a + m − 2·a·m` (XOR). The
   start is 0, except when the first mask subtracts or intersects — then it starts from the full
   frame (AE: "the first Subtract mask subtracts from a full layer").
6. Quantized once: `round(a · 255)`.

All arithmetic is f64 with `sqrt`/`floor` only (exact in IEEE), so the bytes are the same on every
platform (invariant 11) and are frozen by a hash (t1269).

**The apply kernels** (integer, mirrored in WGSL): alpha `a' = (a·m + 127) / 255`; mix
`c' = (dry·(255 − m) + wet·m + 127) / 255` per channel (alpha included), the dry side fitted by
`fit_into` first when the sizes differ, as `blend_frames_cpu` does. The GPU gets the coverage as
an `Rgba8Uint` texture (the byte in `.r`) in T2 of the existing `layer_mix` layout (`m` unused) and a
new `mask.wgsl` for the alpha form; the inputs are the same bytes, the arithmetic is integer, so a mask
adds no divergence: mask-only chains agree with `FrameTol::EXACT`, a masked effect within its own
class (t1274). Rejected — rasterizing on the GPU: no compute, no
storage buffers, three textures; a GPU fill would be a second rasterizer held to a threshold
tolerance (F-MSK-001 "a threshold kernel is the worst case"), while the CPU raster is a few
milliseconds per 1080p frame.

**A memo** (the owner's rule "caches where they pay"): the last coverages, keyed by the hash of the
static masks + affine + size, bounded (16 entries, LRU), invisible to bytes — a static mask on a
playing event is rasterized once, not per frame.

### D5. Animation: the passport and the path keyed as a whole

`Mask::specs()` is built at run time (R.59): `opacity`, `feather`, `expansion` and the shape's
rows — `cx cy w h rot` (+ `round`) for the primitives, `v<id>.x v<id>.y v<id>.ix v<id>.iy v<id>.ox
v<id>.oy` per vertex for a path. A vertex id is a stable address: deleting a vertex never renames
another, so its curves and links stay where they were. `mode`, `invert`, `falloff` are static (a
choice and a flag; a mode that jumps in time has no meaning a person expects).

Curves live in `Mask.auto` (a carrier owns its curves — `TransitionEvent.auto`, `CameraEvent.auto`;
the same `ParamCurve`, not a second key mechanism). The domain of an event and a slot mask is the
event's MATERIAL tick, like `fx_auto`: trim/split/reverse do not move the keys. `Mask::at(tick)`
evaluates the curves by `curve_value`.

**The path is keyed as a whole** (AE's "Mask Path" stopwatch): the mask row's stopwatch puts a key
at the playhead on EVERY shape row; after that, a drag of any point on the preview at another tick
keys every shape row at that tick (the values of the untouched points are their current ones). So
all points share key times and the shape interpolates point by point (F-MSK-001 trick 3:
correspondence by vertex). Inserting a vertex into an animated path writes keys for the new vertex
at every key of the path (the point at the same curve parameter of the split segment at that key),
in the same history step — the shape at the keys does not change. Deleting a vertex of an animated
path deletes its curves. Feather, opacity and expansion have their own curves, edited in the same
`automation::draw` widget of the mask list.

### D6. Links: a mask row as a target and a source (the tracker's door)

`ParamOwner::Mask { slot: Option<usize>, mask: MaskId }` (event picture when `slot` is `None`); the
link path text `self/mask<id>/v3.x` and `self/fx<N>.mask<id>/feather`. `check_links`,
`resolve_derived` and `read_at` walk mask curves next to `fx_auto`/`gen_auto`; a mask curve can be
a link or a formula and is resolved in `morph_pass` with the others (topological order, once per
build). A tracker (T1) will write its track as a property and the mask point will LINK to it — no
new mechanism. Two slots that should share a contour do the same: the second mask's rows link to the
first's.

### D7. Commands, validation, clipboard

- `SetMasks { id, masks: EventMasks }` (the picture and every slot list of the event in one value)
  and `SetTrackMasks { id, masks }` — full replacements (as `SetFxAuto`); a gesture sends them through `apply_coalesced`
  (one history step per drag). `SetFx` trims `fx_masks` to the new chain length (as it trims
  `fx_auto`); the editor's reorder moves the masks with their slots under the window's gesture.
- `validate`/`check_on`: unique ids and names in a list, a path has ≥ 3 vertices with unique ids,
  finite numbers, every curve names an existing row of its mask, masks only on video events and
  video tracks, `fx_masks.len() <= fx.len()`, no masks on the leading Geometry node itself (it has
  the picture masks), and a curve on a track mask → `TrackCurvesUnsupported` (the existing loud
  refusal).
- The clipboard: masks are fields of `ClipEvent` and travel with it; `FxSlotBlock` gains
  `masks` (skip if empty — old clipboard JSON reads), `FxChainBlock` carries them per slot; the
  paste re-hangs their keys on the target's entry point like the slot's curves.
- `Band::after_batch` and `target_rows` get their exhaustive rows; `touches_render_plan` is true.

### D8. The file

`.kt`: the masks are ordinary fields of the serde form (D1). `to_text` writes the version BY
CONTENT: 2 when any event or track has a mask (`Project::needs_format_2`), otherwise 1 — so every
file without masks is byte for byte as before, and a build that reads only format 1 refuses a file
with masks ("written by a newer Kinetren") instead of opening it with the masks silently gone.
`PROJECT_FILE_VERSION` (the highest version read) becomes 2; format 1 reads as before. OTIO: the
clip envelope (`metadata["kt"]`) carries `masks`/`fx_masks`, the track envelope `masks`; the writer
raises the envelope to v14 by content (`clip_needs_v14`). A foreign program shows the event without
its masks — the row in "What OTIO export loses".

### D9. The editor on the preview (kt-app `maskctl.rs`)

- **One state:** `KtApp.mask_ctl: Option<MaskEdit { owner, mask: Option<MaskId>, tool, sel_vertex,
  pen: Vec<Vertex>, drag }>`, `owner ∈ { Event(ClipId), Slot(ClipId, slot), Track(TrackId) }`.
  While it is set, the R.64 frame is not drawn (one set of handles at a time, as Transform/Crop) and
  `maskctl::controls` is drawn instead, from the same single call site in `draw_preview`, after
  the preview click interactor (so the mask handles own the pointer inside their zone; a click
  outside every handle and outside the drawn contours still plays/pauses — t74).
- **Mapping:** event and slot masks go through the leading node's placement (the same call as
  `geomctl::geom_placement_of`, curves sampled at the material tick), a slot before the node and a
  track mask through the plain picture rectangle. There is no second arithmetic.
- **Handles:** a primitive has the 8 scale handles + rotation of the R.64 frame drawn on its own
  box, plus the body move; a path has its vertices (squares), the selected vertex's tangents
  (circles on lines), the contour as one mesh; every mask with feather > 0 has its outer contour
  dashed and one feather handle on it (dragging it outward grows the feather). Grab zones are R.64's
  (`GRAB` = 14 px) and the nearest handle wins.
- **Tools:** Rectangle / Ellipse — a drag draws the shape (Shift — square/circle, Alt — from the
  centre); Pen — click adds a corner vertex, click-drag a smooth one (the drag sets the out tangent,
  the in tangent mirrors it), click on the first vertex or Enter closes and creates the mask, Esc
  drops the unfinished path (and, when nothing is unfinished, leaves mask editing — the Esc
  cascade), Backspace removes the last unfinished vertex. Editing: drag a vertex/tangent/handle/body;
  Alt+click on a vertex — corner ↔ smooth; double-click on a segment — insert a vertex there; Delete
  — remove the selected vertex (a path keeps ≥ 3; the refusal is said as an error, rule 16).
- **Writing:** every drag recomputes from the press and the masks at the press (absolute, R.64
  t760); one `apply_coalesced` gesture token per drag → one history step; a row with a live curve
  gets a key at the cursor (only by the gesture, R.67); an animated path keys every shape row (D5).
- **Playback:** the contours are drawn during playback (they follow the animation); the handles
  are hidden while playing, as the R.64 frame's (its rule: a drag would key a moving tick).

### D10. Where masks live in the interface (rule 22, "do not throw features anywhere")

One widget, `masks::mask_list(ui, ctx, &mut Vec<Mask>, …) -> MaskListResult`, drawn in three
existing places:

| Owner | Where a person looks (AE / Premiere / Resolve) | Here |
|---|---|---|
| the event's picture | AE: the layer's Masks group; Premiere: Opacity → masks | the Geometry node's "Transform" form, a "Masks" group between Crop and Position (the node's order crop → mask → placement), and the preview's right-click menu → Mask → Rectangle / Ellipse / Pen |
| an effect | Premiere: each effect's mask buttons; Resolve: the node's window | the effects window, a "Masks" group under the selected slot's parameters |
| a track | Resolve: a window on the timeline node; AE: an adjustment layer's mask | the track window, a "Masks" section after the blend modes |

A row of the list: the name (double-click renames), the mode, invert, opacity, feather + falloff,
expansion, the stopwatch of the path, "▸" to edit it on the preview, × to delete; the tool buttons
"Rectangle", "Ellipse", "Pen" above the list arm the preview tool for this owner. Up/down arrows on
a row reorder (the order is the order of the operations, so it goes through history). The curves of
the selected mask are drawn below by `automation::draw` (rule: one widget for all carriers); on a
track the widget shows the refusal hint instead, as the track's chain does.

## Risks / Trade-offs

- [The rasterizer is on the CPU even when the frame is on the GPU] → it runs only for masked
  owners, the memo removes it for static masks, and a 1080p mask with a 50 px feather costs about a
  millisecond; the alternative (a GPU rasterizer) costs a second implementation held to a threshold
  tolerance.
- [Parallel `fx_masks` must follow every chain edit] → `SetFx` trims, `active_fx_masks()` filters
  with `active_fx()`, and the FX editor's reorder is covered by a test (t1262).
- [8-bit coverage bands on a very wide feather] → the frame path is 8-bit anyway (F-MSK-001); the
  coverage adds no second rounding beyond one LSB.
- [A slot mask after the node follows only the LEADING node] → a second Geometry node or a lens
  distortion later in the chain is not followed; said in the spec.
- [Format 2] → written only by content; a person who never draws a mask never meets it.

## Migration Plan

No migration: new fields default to empty. Format 1 files read unchanged; format 2 is written only
with masks. Rollback = the format-2 refusal of an older build (honest).

## Open Questions

None for the owner: the scope is his batch (S1) and the brief's list; the choices above follow the
industry and the notes, and every one of them is reversible without a format break.
