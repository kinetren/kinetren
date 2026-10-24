---
id: F-MPH-003
title: Adjustment Clip: an effect over the composite
status: candidate
priority: B
cost: L
category: morpher/analogues
editors: [resolve-20, vegas-pro-2026, premiere-26, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-gpu, kt-app]
invariants: [4, 7, 11]
ui_rules: [13, 17]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MPH-003 — Adjustment Clip: an effect over the composite

## Essence

An empty clip from the effects library that is placed on the top track. Everything
that lies under it on the tracks below, within its window, is glued into one picture, and
its effects, grade and transform are applied to that picture. To the editor it looks like an
ordinary clip: it has a length and handles, opacity, a blend mode, a name, and
it can be dragged into the bin and taken out again. The boundaries of its action are its
own boundaries on the timeline: extend it with a handle — and it covers two more cuts.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| DaVinci Resolve | 20.3 | Adjustment Clip | a clip from Effects Library → Toolbox → Effects; ResolveFX/OFX lie on it, the Inspector (Composite, Transform, Cropping, Dynamic Zoom), the Fusion comp and the grade with the sizing of the Color page |
| VEGAS Pro | 2026 | Adjustment Event / Adjustment Track | an event or a whole track; the scope of action is by default "everything below", but is narrowed to the compositing children of the track; the track's Level slider controls the STRENGTH of the effect on everything it covers |
| Adobe Premiere | 26 | Adjustment Layer | a synthetic element from the project bin; effects act downward, but the fixed Motion does not: the scale and rotation of the composite are done by the Transform effect from Distort |
| Final Cut Pro | 12 | Adjustment Clip | native only since 11.1 (Edit → Add Adjustment Clip, Option-A); before that — a homemade title placeholder assembled in Motion |
| After Effects | 26 | Adjustment Layer | a checkbox on the layer; acts on the composite of the layers BELOW in this same composition, and the layer's own mask limits the AREA of the effect's action |

## The trick behind it

**1. "Apply to many" in Resolve is two families giving different results, and the adjustment clip is lonely in one of them.** Everything that lives on the
Color page (Group Pre-Clip → Clip with node stack layers → Group Post-Clip →
Timeline, plus shared nodes) is executed ONCE FOR EACH CLIP and BEFORE the composite: the
manual about the upper of these levels says literally "Node trees assembled
in this mode affect every single clip in the entire Timeline". The adjustment clip is
the only mechanism that is executed ONCE over the glued picture.
Hence the practical point: a vignette "for the whole film" via a Timeline grade on a
picture-in-picture gives TWO vignettes, while on an adjustment clip it gives one; and conversely,
driving the sources into the working space (CST) is correct only at the
per-clip level, because at the composite the sources are already indistinguishable. The order of the levels
is not entirely rigid at this point: the project checkbox "Post Group Node Stack Layer" moves
the last layer of the node stack BEHIND Group Post-Clip.

**2. The opacity of an adjustment clip is not "the strength of the effect" but the compositing of its output
over the same composite from below**, that is, a wet/dry mix — and it is broken:
an EMPTY adjustment clip at 50 % noticeably darkens the picture, and at 0 % the brightness
returns. The numbers agree with the model "alpha applied twice" (a·a + (1−a) = a²−a+1: minimum 0.75 exactly at 50 %). A real wet/dry in Resolve
lives one floor lower, at the node: Key Output Gain, which "reduces the strength of
that node's contribution" when mixing through Layer/Parallel Mixer.

**3. An adjustment clip is not a layer type in the pipeline, but a clip whose source = "the composite from below".** From this follows everything else, both good and bad: it
gets into the list of clips on the Color page and STEALS the selection at each transition
to it (it is cured by Alt-clicking the track number in the mini-timeline — the number turns red,
the clip disappears from the list, and the effect remains); several adjustment clips can be
gathered into a group and graded as a group, so all non-adjacent pieces of the timeline are graded at once; its
Inspector-Transform moves the whole composite as one (in Premiere exactly this
does not work); and it is cached not where the grades are, but in the Sequence Cache — a separate
cache of "timeline" effects (transitions, transparency, blend modes),
whereas grades and OFX sit in the Node Cache.

## Pros

- The effect is computed EXACTLY ONCE per output frame, not once per layer:
  glow, grain and vignette are not doubled either on a crossfade or on
  a picture-in-picture.
- The scope of action is the length of the object on the timeline: the handle dragged to the right covers
  two more cuts, and no clip is edited in the process (and no undo accumulates per clip).
- Since it is a clip, the whole clip machinery works on it for free: the grade,
  the Fusion comp, groups, styles from the gallery, Paste Attributes, the render cache, moving
  into the bin as media, and reuse.
- Inspector-Transform / Cropping / Dynamic Zoom act on the glued frame,
  so "zooming the camera onto the whole scene" is one object, not N identical
  transforms across layers.
- It is switched off one by one or as a whole track, without losing the settings.

## Cons

- The input is an already glued picture, and the sources in it are indistinguishable: normalising by
  camera, balancing and matching shots to each other is impossible at this level in principle; for these a
  different mechanism is needed (a shared node / a group).
- The scope is rigid — "everything below": on a transition it covers both sides at once, and
  "only the nearest track down" is not expressible at all;
  the only workaround is to wrap the lower layers into a compound clip.
- It is a clip — so it is in the clip list of the Color page: it steals the selection, takes part in
  the numbering, and requires a separate trick to remove it from there.
- The opacity slider is not equivalent to the strength of the effect, and on an empty clip it spoils
  the picture; the "correct" wet/dry is found at another level (Key Output Gain) and
  requires assembling a Layer/Parallel Mixer node.
- The adjustment clip itself exists in the free version too, but most of what is usually put on it — noise reduction, Magic Mask, film grain, optical blur and
  dozens of other ResolveFX — is only in Studio.

## What users say

> It's the transitions that give me trouble with Adjustment clips, because the
> "adjustment" affects everything underneath it, including the incoming clip.
> — kinvermark (Mark Wilson), 23.12.2019,
> https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=104651

> In other applicatons like Photoshop etc. there's a checkbox somewhere that
> sets the authority of the adjustment layer to all layers below it or just to
> the next layer underneath the adjustment layer.
> — Hardy Heinlin, 21.09.2024,
> https://forum.blackmagicdesign.com/viewtopic.php?t=208739

> From 100% to 50% the opacity/brightness of the clips underneath decreases
> gradually, from 50% and bellow it starts to increase.
> — panos_mts (Panagiotis Vellios), 22.12.2019,
> https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=104651

In the same thread, in the next message: "I can confirm this behaviour. It looks like a bug
to me." (Peter Cave, 23.12.2019). This is a 2019 thread; no fresh confirmations for 20.x
were found live — it cannot be treated as a current defect, but the behavioural model of the slider
("a fade between 'with the effect' and 'without'") is described in the thread correctly and matches
what Vegas does with its Level slider.

A separate story: the adjustment clip captures the selection on the Color page; the thread runs from 2019 to 2022, and the solution there was found not by the developers but by the users:
https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=94192

## How it fits Kinotrain

### What already exists

- The morpher layer: `MorpherEvent` in `src/libs/kt-model/src/project.rs`, targets
  `MorphTargets` = `Below` | `Clips` | `Marks` | `Areas`, the patch = replacement of `media`,
  a shift of `source_in`, the chains `video_fx`/`audio_fx`, bitwise `*_bypass`.
  It is applied in `Project::effective_clips()` (same place) — per the comment in the code
  "the ONLY point of issuing clips to the engine, export and drawing", accumulation
  bottom to top, the order is deterministic by `MorphId`.
- **From this comes the main conclusion of the survey: by construction our morpher is not an adjustment
  clip, but Resolve's shared node / Group Post-Clip.** It edits the CLIPS before the composite, so one morpher with an effect on three layers = three executions
  of the effect, each on its own layer. We have no composite level at all.
- The composite: `FramePlan` in `src/libs/kt-engine/src/video_plan.rs` — layers
  back-to-front, each has its own `fx`, its own `fx_hash` and `alpha_k`; there is not a single "after
  gluing" stage in the plan, `PlanLayer` knows only `One` and `Dissolve`.
- A clip has opacity (`ClipEvent::opacity`, `src/libs/kt-model/src/
  project.rs`, goes into the plan as `alpha_k` in `src/libs/kt-engine/src/video.rs`),
  but `MorpherEvent` has neither a strength slider nor a blend mode — only the
  binary `enabled` and binary slot bypasses. This is exactly the claim of item 245 ("control the blend transparency with a slider").
- Caches: `BurstKind::PostFxVideo { quality, fx_hash }` in
  `src/libs/kt-cache/src/lib.rs` — the key is PER LAYER; `ReadbackCache` in
  `src/libs/kt-gpu/src/pool.rs` — already by the hash of the whole plan, that is, "by the composite". The split of Node Cache / Sequence Cache in Resolve is about the same
  thing, and we already have half of such a split.
- The morpher UI — `src/apps/kt-app/src/morpher_editor.rs`, parameter automation —
  `src/apps/kt-app/src/automation.rs` (the key domain = the material tick, linear/smoothstep).
- There is no colour correction at all: a grep over `src/` for wheels, saturation, curves,
  LUT and scopes gives only the generated FFmpeg bindings in `target/`. Therefore the
  comparison with Resolve is unequal — half of what is put on
  the adjustment clip we simply have no means to do. But the mechanism "an effect on the
  result" does not depend on the set of effects: our Blur/Glow/Edges/VHS on
  the composite would behave exactly the same way.

### What is missing

1. A strength slider on the morpher (wet/dry): "apply the effect at 40 %". Today the only gradation is on/off.
2. A second level of application — a stage of "the effect on the GLUED frame". Right now
   "a vignette on the whole scene" = N copies of the effect on N layers, and on a crossfade two
   vignettes blend with each other; on an adjustment clip this does not happen.

### What gets in the way

- `FramePlan` is flat: a post-composite stage is a new node of the plan, a new
  input for BOTH executors (the CPU reference and the WGSL mirror), and a new kind of cache key
  computed not from the layer's `fx_hash` but from the hash of the stage itself.
- Wet/dry on a layer is not free either: the FX are computed per layer, the result goes into the
  composite, so the "before/after" mixing has to be done INSIDE the layer, which means keeping the pre-FX
  frame next to the post-FX one — plus a buffer per layer on both backends.
- The targeting will diverge. Our `targets` are richer than Resolve's
  (`Marks`/`Areas` — named ranges and "time × tracks" areas), while the composite stage by its
  construction can only do "everything below". If we hang it as a checkbox on the same `MorpherEvent`, one object
  would have two different meanings with different costs — rule 17 (paired ways behave the same)
  requires either identical behaviour or different objects with different names.

### Which invariants it touches

- **7** (the composition code is one): the post-composite stage must live in
  `frame_plan` and execute identically on CPU and GPU, otherwise preview will diverge from
  export.
- **4** (the cache is the only truth about coverage): the new stage requires its own kind
  of key, otherwise the coverage bars would lie.
- **11** (determinism): the CPU reference byte-for-byte plus cross-backend within tolerances
  via `kt_gpu::assert_frames_close`.
- UI rules **13** (the slider must sit in the already occupied place in the morpher chain strip,
  not start a new row) and **17**.

## Estimate

| What | Answer |
|---|---|
| Size | L — but it splits: the strength slider separately S, the post-composite stage L |
| Where the main work is | kt-engine (`video_plan`/`video`), kt-gpu (shader + pool), kt-model (fields), kt-app (the widget in the chain strip) |
| Project format | the slider — a new `MorpherEvent` field with `skip_serializing_if`, old files stay byte-stable; the composite stage — a new schema version |
| Tests that will have to be written | a model round-trip and operation parity; CPU↔GPU conformance of the new stage on the model of t181/t182; preview == export (t67/t184); a harness test of the slider in the morpher window |
| Depends on | nothing (there is no colour correction, but the mechanism does not depend on it) |
| Breaks compatibility | no |

## Verdict

We take the morpher's strength slider: this is a direct request from the user (item 245), Vegas has it, and Resolve's is broken at the clip level — so what must be copied is not Opacity but Key Output Gain, that is, an honest
wet/dry inside the layer.

The post-composite stage ("a morpher that edits the RESULT, not the clips") we do not take in this pass, but we record it as a separate gap: Resolve has two such levels, we have one, and ours is a shared node, not an adjustment clip. The stage touches
invariant 7 and both backends at once, so it needs its own round, not a checkbox
on the morpher.

## Sources

- DaVinci Resolve 20 Reference Manual (PDF), chapters 47 "Editing, Adding, and
  Copying Effects and Filters" (Adjustment Clips), 142 "Grade Management"
  (Using Adjustment Clips, Using Shared Nodes, Pre/Post-Clip groups), 143 "Node
  Editing Basics" (Clip vs. Timeline Grading, Node Stack Layers, Post Group
  Node Stack Layer), 145 (Key Output Gain), 8 (Node Cache and Sequence Cache):
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf
  (access date 2026-09-07)
- https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=104651 — "Adjustment
  clip opacity", the behaviour of the slider and transitions (2026-09-07)
- https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=94192 — "Adjustment
  clip problem in Color page", the theft of the selection and Alt-click on the track number
  (2026-09-07)
- https://forum.blackmagicdesign.com/viewtopic.php?t=208739 — "Want adjustment
  clip to affect next layer underneath only" (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/adjustmentevents.htm
  — Adjustment events in VEGAS, the restriction of the scope by compositing children
  (2026-09-07)
- https://larryjordan.com/articles/create-an-adjustment-layer-for-final-cut-pro-11-using-motion-5-9/
  — the native Adjustment Clip in FCP appeared in 11.1 (2026-09-07)
- https://community.adobe.com/t5/premiere-pro/adjustment-layers-can-scale-be-used/m-p/9164885
  — in Premiere, Motion on an adjustment layer does not act on the clips below
  (2026-09-07)
- https://helpx.adobe.com/hk_en/after-effects/using/mask-reference.html — the mask of an
  adjustment layer as a limiter of the effect's area in AE (2026-09-07)

The Resolve part of the note is compiled from the official 20.x manual and the Blackmagic forum;
it was not checked live in the program.
