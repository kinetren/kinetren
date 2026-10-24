---
id: F-MPH-002
title: Adjustment Layer in After Effects
status: candidate
priority: B
cost: L
category: morpher/analogues
editors: [after-effects-26, vegas-pro-2026, premiere-26, resolve-20, fcp-12]
we_have: partial
crates: [kt-model, kt-engine, kt-gpu, kt-app]
invariants: [3, 4, 7, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MPH-002 — Adjustment Layer in After Effects

## Essence

A layer without its own picture. Everything that lies BELOW it in the stack is first
stitched into one frame, and only this frame is handed to the layer's effects. It acts
only within the in/out of the layer itself. The layer's opacity works as the blend strength:
40 % means 40 % of the processed result over 60 % of the original composite.
A mask on the layer limits the effect to a region of the frame, with feathering and animation
of the path. This is the canonical implementation of the idea from which the others dance.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x (2026) | Adjustment Layer | a switch on ANY layer; effects receive the composite of everything below; the layer's alpha (opacity + masks) is the application map |
| VEGAS Pro | 2026.0 | Adjustment Event | an event on a track; by default it acts on the video events directly under it, the area is narrowed by assigning compositing children tracks; the event has opacity |
| Adobe Premiere | 26.x (2026) | Adjustment Layer | a synthetic bin item, placed on a track; acts on all tracks below; the mask lives on the EFFECT, not on the layer |
| DaVinci Resolve | 20.3.2 | Adjustment Clip | a clip from the Effects Library; the Inspector gives opacity + Composite Mode; the same clip on the Color page gives a grade with a temporal boundary |
| Final Cut Pro | 12.3 | Adjustment Clip (native since 11.1), before that a generator from Motion | a clip "above the timeline" over a range; for years it was a third-party .moti file that people put into the system folder by hand |

## The trick behind it

**First.** Opacity and mask are NOT two functions but one: the alpha of the layer.
An adjustment layer is not "visible" or "invisible" — its alpha, per channel, sets where and
how strongly the processed frame is laid over the unprocessed one. Hence a consequence that is
not in any feature list: **blend-with-original
appears on any effect and on the whole chain AT ONCE, even if the effect itself has no such
parameter**. The opacity slider = "40 % of colour correction, blur and grain together";
a mask with feathering = "inside this oval — 100 %, toward the edge — smoothly to zero";
animating the mask path = the effect moves with the object. One mechanism, three handles.

**Second, one floor down.** EVERY effect in the timeline has its own
Compositing Options group: the Effect Opacity property (Adobe's help names it directly as
the same thing the old Blend With Original was) and an arbitrary number of Mask References.
The effect's masks do NOT change the alpha of the layer — they change WHERE the result of this effect
is composited. Therefore one adjustment layer can hold a blur by mask No. 1 at 30 % and
colour correction by mask No. 2 at 100 %, and both masks live on that same layer. Two independent
levels of "strength": the layer-wide one for the whole chain and the per-effect one inside.

**Third.** An adjustment layer is a 2D break in the stack. A group of 3D layers, cut in half
by it, collapses into a flat result, as if it had been precomposed: intersections
and shadows across the boundary stop being computed. The mirror side of the same mechanism:
an adjustment layer inside a precomp with Collapse Transformations "leaks" outward and
colours the layers that lie BELOW the precomp in the parent composition.

## Pros

- The effect is computed once per composite frame, not N times for N layers: the cost does not
  grow because there are not two clips under the layer but forty (Adobe's help states this
  directly in plain text as an argument for the adjustment layer).
- Area effects (blur, distortion, grain, glow) see the STITCHED frame, so the boundaries
  between the layers beneath them do not show through: the whole picture is blurred, not each
  piece separately with its own sharp edge.
- Editing one layer rewrites the whole stretch of film without walking through clips and without
  forty undo steps; deleting the layer returns the source, because the pixels under it were not touched.
- Opacity gives the dry/wet mix of the whole chain in one movement, and its keyframes give a
  soft in and out of the effect, which the effects themselves do not have.
- Adjustment is a switch on any layer, not a separate object type: a solid
  turns into an adjustment layer and back with a checkbox, without re-creating anything.
- A mask turns "an effect on everything below" into "an effect here", without creating
  a precomp or a duplicate layer.

## Cons

- The input is the whole composite frame, including places where there is NOTHING under the layer. Blur at the edge
  of the composition pulls in transparency and darkens (it is cured by Repeat Edge Pixels,
  a solid backing, Grow Bounds), and alpha effects such as Simple Choker eat the alpha
  of the accumulated buffer, not an "object", which the engine does not know about.
- The buffer equals the size of the composition: everything that went past the frame does not
  exist for the effect. An area effect at the border gets a cut-off rather than a continuation
  of the picture — and this is noticeable exactly when the layer beneath it is animated and sliding in.
- Area application is paid for with the 3D scene: the layer slices the stack, and intersections
  with shadows through it are not computed. It is cured by pre-rendering or by applying the effect
  one by one — that is, by giving up the very idea of the adjustment layer.
- "Stopped working in the middle of the composition" is a typical complaint, and almost always it is not a
  bug but the last opacity keyframe or the end of the layer: because of the hard in/out, the effect
  breaks off exactly on a frame, and the timeline marks the break nowhere.
- Precomposing breaks the layer: inside the new precomp there is nothing under it. Without Collapse
  Transformations it does not act outward; with it, it does act, but at the same time
  it drags blend modes from inside the precomp outward.
- A layer has no pixels, so it cannot be a track matte, and the AI roto from AE 26
  (Object Matte, 26.2, April 2026) does not target it: one-click selection
  works on the source frames, so the matte area is still built on
  the source layer and dragged from there into the mask.
- The order is by the stack, not by time. Two adjustment layers swapped with the mouse, and the result
  changes without a single parameter edit; in a chain of four, "why does it not look like this" is debugged by dragging.

## What users say

> When you adjust the opacity of an empty adjustment layer, you are effectively
> copying every layer below the adjustment layer, precomping it, and adjusting
> the opacity of the precomp.
> — Walter Soyka, Creative COW, 2015-02-19,
> https://creativecow.net/forums/thread/adjustment-layers-and-opacity/

The negative side is the same mechanics, seen from below. About precomps:

> If you pre-compose an adjustment layer, you must collapse transformations to
> get the adjustments to work in the main comp.
> — Rick Gerard (Community Expert), Adobe Community, 2023-03-04,
> https://community.adobe.com/t5/after-effects-discussions/all-effects-from-adjustment-layer-disappear-after-pre-compositing/td-p/12810680

And about what is paid for this in the other direction:

> Using collapse transformations/ continuously rasterize passes through blending
> operations from the pre-comp when not used with 3D layers.
> — Mylenium, Adobe Community, 2016-12-16,
> https://community.adobe.com/t5/after-effects-discussions/adjustment-layer-within-a-precomp-affecting-layers-outside-of-precomp/td-p/8691477

The thread "adjustment layers stop working mid way through comp" has lived since 2017 and
collects new replies in 2022: people cut a five-minute composition into pieces,
although the cause is the last keyframe at the three-minute mark
(https://community.adobe.com/questions-529/adjustment-layers-stop-working-mid-way-through-comp-45963).

## How it fits Kinotrain

### What already exists

- `MorpherEvent` and `MorphPatch` in `src/libs/kt-model/src/project.rs`, the target mode
  `MorphTargets::Below` — "everything below the morpher line", exactly the semantics of
  an adjustment layer. It is applied in `Project::effective_clips()` BEFORE the plan is built;
  the engine knows nothing about the morpher; cumulatively from bottom to top, a partially covered
  target is cut into pieces.
- The morpher window — `src/apps/kt-app/src/morpher_editor.rs`: one chain strip
  (`chain_strip`), a master switch `enabled`, a "by time" checkbox (`timed`),
  slot bypass switches (`video_bypass`).
- A level as such we have, but on a CLIP: `ClipEvent.opacity` in
  `src/libs/kt-model/src/project.rs` → `opacity_k` in
  `src/libs/kt-engine/src/video.rs`, multiplied by the layer's alpha in the same place as the
  fades.
- The closest thing to a frame area is the chroma key `Greenscreen` in
  `src/libs/kt-fx/src/video.rs` — but it is not a mask, it is a colour key.

### What is missing

- **Blend strength.** A grep for `MorphPatch` finds not a single field like
  `amount`/`opacity`/`mix`: there are `media`, `source_in_delta`,
  `absolute_source_in`, `src_off`, `video_fx`, `audio_fx`, `video_bypass`,
  `audio_bypass`, and that is all. The morpher is binary: `enabled` — on/off. This is exactly the
  discrepancy because of which the base was created (item 245).
- **Automation in the morpher chain**: `chain_strip` is called with `auto: None`,
  whereas for an event the FX editor shows curves (`ParamCurve`/`AutoKey`,
  `src/apps/kt-app/src/automation.rs`).
- **Masks nowhere in the project.** A grep over `src/` gives only `tick_to_mask`/
  `mask_digits` (timecode), `front_mask` in `src/libs/kt-engine/src/transitions.rs`
  (the transition curve) and the wgpu `write_mask`. There are no Bezier paths, no feathering, no track
  matte, and no masks on an effect.
- **Audio FX in the morpher picker**: `chain_strip(…, false, …)` — the `audio` flag
  is off, although `MorphPatch::audio_fx` exists in the model and `split_chains` fills it.

### What gets in the way

The main thing is **the point where effects are applied**. For us video FX are computed PER CLIP
and placed into the post-FX cache, and the composite goes AFTER them:
`src/libs/kt-engine/src/video_plan.rs` builds a `FramePlan` from
`PlanLayer::One`/`PlanLayer::Dissolve`, and `compose_plan_cpu` in
`src/libs/kt-engine/src/video.rs` only mixes the ready layers
(`alpha_over`, `dissolve_into`). In AE the input of an effect is the result of the composite.

The difference is NOT cosmetic, and it is visible by effect class:

- point effects (brightness/colour/`Mirror` on a single opaque layer) give
  the same result in both schemes;
- area effects — `Blur`, `Glow`, `Edges`, `Noise`, `VHS`, `Pan`/`Pan3d` — give a
  DIFFERENT result: for us each layer is blurred on its own and after the overlay its edge
  stays sharp; in AE the already stitched picture is blurred, and the edges of the layers
  are not visible in the result at all;
- the maximum difference is in the transition window: our `PlanLayer::Dissolve` mixes the pair
  AFTER the FX have run on each of the clips, whereas in AE the effect would land on
  the already mixed frame.

To feed the chain with the composite, a SECOND point of FX application is needed — after
`compose_plan_*` — and it must be introduced identically on CPU and on GPU, otherwise a
second composition path appears. Plus the cache: post-layer FX is cached per clip, while a
post-composite one has nothing to be cached by — the key becomes the whole set of layers
of the frame.

Masks are an independent entity (path, feathering, animation, the CPU reference and the WGSL
mirror); this is a note of the `masks/` category, not a field in the morpher.

### Which invariants it touches

- **7 (the composition code is ONE)** — the central one. The second point of FX application must
  live inside `VideoRenderer::frame_plan`/`compose_plan_*`, not separately in export;
  otherwise preview and export will diverge.
- **4 (the cache is the only truth about coverage)** — a new kind of key appears
  (post-composite), and the coverage bars must count it, not keep a second set of books.
- **11 (determinism)** — CPU byte-for-byte against the reference, CPU↔GPU only through
  `kt_gpu::assert_frames_close`.
- **3 (an edit of the model does not kill FX state)** — the strength slider is moved
  with the mouse in real time, this is `set_params` with smoothing, not a rebuild.
- UI: **13** ("do not waste space") — the strength slider asks to sit in the morpher window header
  next to the master switch; it is not worth a separate panel; **17**
  ("paired ways") — if the strength is in the window, it must also be a gesture on the block,
  like the fade corner on an event; **20** ("nothing grows by itself") — strength in old
  files is not "substituted by meaning" but is read as 1.0.

## Estimate

| What | Answer |
|---|---|
| Size | L — blend strength S, post-composite FX application point L; masks are not included here |
| Where the main work is | kt-engine (`video_plan.rs`/`video.rs` — the second FX point after the composite), kt-gpu (the same in the composite pipeline), kt-model (the strength field in `MorphPatch`), kt-app (the slider and the gesture) |
| Project format | a new field in `MorphPatch` → the version of `metadata["kt"]` is raised; files without the field are read with strength 1.0 |
| Tests that will have to be written | a model round-trip and reading of files without the new field (kt-otio); golden "blur over two layers: per-layer ≠ after composite"; CPU↔GPU conformance of the post-composite chain via `kt_gpu::assert_frames_close`; a harness test of the slider in the morpher window (`layout_tests.rs`) |
| Depends on | masks — only the third handle of "the trick behind it"; the blend strength and the application point are independent |
| Breaks compatibility | the new field — no; changing the application point — YES in the picture: an old project with a morpher over several layers will render differently, so the application point must be a switch on the event with the default "as it was" |

## Verdict

We take it in parts. The blend strength (0..1 in `MorphPatch`, a slider in the morpher window and a
gesture on the block) — a direct closing of the discrepancy from item 245; it is cheap and needs no revision
of invariants: it is a multiplier right where `opacity_k` is. The post-composite
FX application point is a separate piece of the round and a revision of invariant 7; it must be taken
deliberately and with a switch on the event, otherwise already finished projects will shift. Masks
on the morpher — not here: that is the note `masks/`; without them our morpher stays
"an effect on everything below entirely", whereas in AE it is "the effect here and with such a strength".

## Sources

- https://helpx.adobe.com/after-effects/using/creating-layers.html (adjustment layers, masks on them, the performance promise; access date 2026-09-07)
- https://helpx.adobe.com/after-effects/using/mask-reference.html (Compositing Options, Effect Opacity, Mask Reference; 2026-09-07)
- https://helpx.adobe.com/after-effects/how-to/apply-effects-masks.html (effect mask, feathering; 2026-09-07)
- https://helpx.adobe.com/after-effects/using/3d-layers.html (2D layers slice groups of 3D layers: intersections and shadows; 2026-09-07)
- https://helpx.adobe.com/after-effects/desktop/roto-brush-and-refine-matte/roto-brush/object-matte.html (Object Matte, AE 26.2; 2026-09-07)
- https://www.cgchannel.com/2026/04/adobe-releases-after-effects-26-2/ (the composition of the 26.2 release, April 2026; 2026-09-07)
- https://creativecow.net/forums/thread/adjustment-layers-and-opacity/ (2026-09-07)
- https://community.adobe.com/t5/after-effects-discussions/all-effects-from-adjustment-layer-disappear-after-pre-compositing/td-p/12810680 (2026-09-07)
- https://community.adobe.com/t5/after-effects-discussions/adjustment-layer-within-a-precomp-affecting-layers-outside-of-precomp/td-p/8691477 (2026-09-07)
- https://community.adobe.com/questions-529/adjustment-layers-stop-working-mid-way-through-comp-45963 (2026-09-07)
- https://community.adobe.com/t5/after-effects-discussions/simple-choker-in-after-effects-is-choking-the-adjustment-layer-rather-than-the-layers-underneath/m-p/9632680 (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/adjustmentevents.htm (Vegas: adjustment events, compositing children, transparency; 2026-09-07)
- https://helpx.adobe.com/premiere/desktop/add-video-effects/apply-video-effects/create-adjustment-layers.html (Premiere; 2026-09-07)
- https://davinciresolveclub.com/adjustment-clips-davinci-resolve/ (Resolve: adjustment clip, Inspector, Color page; 2026-09-07)
- https://larryjordan.com/articles/create-an-adjustment-layer-for-final-cut-pro-11-using-motion-5-9/ and https://support.apple.com/en-uz/102825 (FCP: native adjustment clip since 11.1, before that a generator from Motion; 2026-09-07)

A caveat per the rules of `_editors.md`: the `helpx.adobe.com` pages in this session were not served to the loader (timeouts) and were read through search-result snippets;
the Adobe and Creative COW forum threads and the VEGAS help were read in full. Nothing was checked live
in the editors.
