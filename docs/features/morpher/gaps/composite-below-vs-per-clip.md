---
id: F-MPH-007
title: What a layer sees on input — the composite below or each clip
status: researched
priority: B
cost: XL
category: morpher/gaps
editors: [after-effects-26, premiere-26, vegas-pro-2026, resolve-20, fcp-12]
we_have: no
crates: [kt-model, kt-engine, kt-gpu, kt-cache, kt-app]
invariants: [4, 7, 11]
ui_rules: [13, 17]
round: null
point: null
tests: []
depends_on: [F-MPH-001, F-MPH-002, F-MPH-003, F-MPH-004, F-MPH-005]
checked: 2026-09-07
---

# F-MPH-007 — What a layer sees on input — the composite below or each clip

## Essence

The editor puts a layer over the edit and attaches an effect to it. The question this note answers: does that effect receive as input ONE already composited picture of everything below it, or EACH lower clip separately? From the outside both look the same ("effect on everything below"), but on screen they diverge: a blur across a crossfade, a grade of two cameras with one layer, a chroma key, a vignette, a camera push onto the scene. Each gives a different picture. All five reference editors take the composite as input. We take each clip, and this is the deepest divergence of the morpher from other editors' adjustment layers: from it follow the absence of overlay transparency (F-MPH-001, F-MPH-002), the absence of masks by frame area, and the presence of source substitution, which nobody else has.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | Adjustment Layer | the input is the STITCHED frame of all layers below it in this composition; the effect is computed once per frame, and the layer's alpha is the application map |
| Adobe Premiere | 26.x | Adjustment Layer | the input is the mixed-down picture of the tracks below; the output is composited back over exactly the same frame |
| VEGAS Pro | 2026.0 | Adjustment Event / Adjustment Track | the input is the composite of the lower tracks ("roughly post-compositing Track FX"); per-clip Media FX and Event FX levels remain separate alongside |
| DaVinci Resolve | 20.3.2 | Adjustment Clip **versus** Timeline grade | two levels under one slogan "apply to everything": the adjustment clip runs ONCE on the joined picture, while the Timeline grade runs once PER CLIP before compositing |
| Final Cut Pro | 12.3 | Adjustment Clip | the input is the composite of the clips below it in the stack; there is no concept of tracks, the stack order defines it |
| Kinotrain | — | morpher layer | the input is EVERY covered clip: `MorphPatch::apply_to` appends the FX to its chain, and a partially covered target is cut into pieces; the layer does not see the composite at all |

## The trick behind it

**1. "Adjustment layer" is not a feature but the name of a PLACE in the pipeline, and the place differs between editors.** Evidence lies within one product: in Resolve both the adjustment clip and the Timeline grade are described by the same phrase "acts on everything", but they stand on opposite sides of the composite: the Timeline grade runs once per clip BEFORE joining, the adjustment clip once AFTER (F-MPH-003). So the question "do we have an adjustment layer" is meaningless until the input is named. The morpher's input is per-clip, so by its design it is Resolve's Timeline grade and Premiere's Source Effect, and not a like-named layer at all.

**2. The schemes can be told apart by one measurement, and it also predicts the picture: how many times the effect's kernel runs for ONE output frame.** They run exactly once. We run as many times as there are layers under the morpher, and on a crossfade both ends of the pair are computed separately (`PlanLayer::Dissolve` in `src/libs/kt-engine/src/video_plan.rs` mixes frames that are ALREADY processed). The schemes give the same result only under two conditions at once: the effect is a LINEAR operator (convolution commutes with a weighted sum) AND the composite at that spot is a pure weighted sum, that is, a crossfade. The first condition is broken by threshold and nonlinearity (`Glow` with `threshold`, `Edges`, `Vhs`, `Noise` from `burst_index`), by geometry (`Pan`, `Pan3d`, `EventGrid`), and by alpha production (`Greenscreen`). The second is broken by the composite itself: `alpha_over()` in `src/libs/kt-engine/src/video.rs` is not a weighted sum but an alpha over-blend with divisions and clamping, and it stands at EVERY layer junction. So "the same" happens only in the degenerate case: one track, one opaque layer, a linear kernel; everything else gives different bytes. That is, "where the layer sits in the pipeline" is not an implementation detail: it is a different set of capabilities and a different result with the same parameters.

**3. The place of input determines WHAT a layer can address in principle, and two gaps turn out to be one.** A layer on the composite addresses the AREA OF THE FRAME: a mask, a vignette, "blur this corner" are one-click operations, and a source for it does not exist, because in a mix the sources are indistinguishable. A per-clip layer addresses the SOURCE: substituting `media`, shifting `source_in`, grading each camera on its own, and it cannot address an area, because it has no "frame". Hence the overlay transparency too: a composite layer gets wet/dry as a GIFT (its output lands back on the very picture it consumed), while a per-clip layer is not given it at all, because there is no unprocessed frame beside it to mix with. The claim of item 245 ("the transparency of the overlay is adjustable with a slider, and we do not have it") and our unique source substitution are not two independent gaps, but two consequences of one choice of input.

## Pros

Here and below — about OUR scheme: what it pays and what it gives.

- The effect sees one untouched SOURCE, so the morpher can do what a composite layer cannot do in principle: substitute `media` and shift `source_in` for each target (`MorphPatch` in `src/libs/kt-model/src/project.rs`). In a mixed-down frame there is nothing to substitute: the sources are already indistinguishable in it.
- The scope is declarative and addresses TARGETS, not "everything below": `MorphTargets::Clips` (a frozen list), `Marks` (named ranges), `Areas` (named areas "time × tracks", `MarkedArea.lanes` — semantic track keys). The composite stage can do exactly one mode, "everything below"; that is exactly what people lack in Premiere and Resolve (quotes below).
- Bake is honest and complete: `Command::BakeMorpher` (`src/libs/kt-model/src/model.rs`) writes the patch into the real clips and MATERIALIZES the cut — "bake == what was drawn" is verified by t36 (`morpher_render`). The composite stage cannot be baked into clips at all: there is nowhere to write it.
- Chroma key on a layer makes sense. `greenscreen()` in `src/libs/kt-fx/src/video.rs` sets `px[3] = 0`, and `alpha_over()` in `src/libs/kt-engine/src/video.rs` lets such a pixel through — the layer BELOW is visible. On the composite a key cannot separate the object from the background at all: the background is already mixed into the same pixels (in Premiere a hole reveals the same picture — F-MPH-004).
- The engine knows nothing about the morpher: `Project::effective_clips()` is the single point that outputs clips for preview, export and rendering. No new stage in `FramePlan`, no second kind of cache key, no mirror in WGSL — the morpher does not touch invariants 4, 7 and 11 AT ALL.
- Memoization works per clip: `BurstKind::PostFxVideo { quality, fx_hash }` (`src/libs/kt-cache/src/lib.rs`) — editing one lower layer does not discard the post-FX frames of the others. The composite key would be invalidated entirely by any hiccup on any track.

## Cons

- The cost multiplies by layers: N tracks under the morpher means N kernel runs per frame. The composite scheme has one; on a six-layer edit that is a six-fold difference, and Adobe sells the adjustment layer outright as a saving.
- The seam is not sealed. An area effect (`Blur`, `Glow`, `Noise`) is computed on each layer separately, and the joining happens afterwards. On a pure crossfade a linear kernel survives this (only u8 rounding diverges), but as soon as there is a stack under the layer that is joined by `alpha_over` — a keyed object over the background, PiP, a title, any level below 1.0 — the boundary between layers is not softened by an area effect from the layer at all: there it is blurred, here it stays sharp.
- The frame area is inexpressible: a layer has no "frame" object, so there are no masks, no vignette over the whole image, no frame. A vignette "for the whole film" in our scheme is N vignettes, one per layer, and in picture-in-picture they double (exactly minus Resolve's Timeline grade).
- Wet/dry is impossible without a second buffer: mixing "before/after" has to happen INSIDE the layer, that is, keep the pre-FX frame beside the post-FX one on each layer and on both backends.
- Partial coverage costs the model a CUT: `effective_clips()` cuts the target along the inner edges of the windows, and around this a whole mechanism has grown — `slice_id`/`slice_origin`, `forbidden_zones` (transition windows and neighbours' push-ins), `MIN_CLIP_DURATION`, an exception for time warp, the engine fix "the end of the transition is searched by `same_clip`" (t237). The composite stage needs none of this: it is bounded by time and that is all.
- Geometry diverges across layers. `pan_frame()` in `src/libs/kt-fx/src/video.rs` computes the frame FROM THE SOURCE SIZE (`src_w`/`src_h`), so one morpher with one `Pan` over a 4K plan and an HD title gives two different framings; "push the camera onto the whole scene" with one object cannot be expressed.
- Separate effects in the per-clip scheme simply break the composite: `event_grid()` returns a full-screen frame with `has_alpha: false`, and `alpha_over()` with `has_alpha == false` copies the bytes whole — so a mosaic appended by the morpher into EACH layer covers everything that was under it, and only the top covered layer is visible.

## What users say

The main complaint about the composite input is exactly what we have for free: they have no way to address a source.

> My understanding is that since adjustment layers affect all video
> tracks beneath them in a sequence, I can't (cleanly) have both
> camera's adjustment layers in the same sequence.
> — Tripecac2, Adobe Community, 02.02.2025,
> https://community.adobe.com/t5/premiere-pro-discussions/multi-camera-editing-best-way-to-assign-separate-adjustment-layers-to-each-source/td-p/15127444

> It seems like it would be so much easier if we could control which
> track(s) an adjustment layer affects.
> — Tripecac2, there, 02.02.2025

The sanctioned answer of an expert is to move from the composite level to the per-clip level, that is, exactly our scheme:

> Simple statement: Apply the Lumetri as a Source Effect ... done.
> — R Neil Haugen, Adobe Community, 02.02.2025, there

And the reverse side: where an effect needs ONE source, the composite input gets in the way, and people advise splitting by layers, agreeing to pay in rendering. The author of ReelSmart Motion Blur on his own plugin on an adjustment layer:

> The more you can separate layers and apply RMSB to each layer the
> better.
> — Peter Litwinowicz (RE:Vision Effects), Creative COW, 17.04.2008,
> https://creativecow.net/forums/thread/applying-motion-blur-to-each-layer-or-to-a-top-adj/

There is no negative feedback about our side of the choice to be had: there is no product with a per-clip "layer" on the market, and the nearest analogues — Source Effects in Premiere and Resolve's Timeline grade — people call not a layer but a workaround.

## How it fits Kinotrain

### Where the difference is visible in the picture

A walk through the cases; everything is checked against the code, not by analogy.

1. **Colour correction of two different sources with one layer.** They have a shared grade over the mix, and it is impossible in principle to pull up the log curves of two cameras. We have `apply_to()` append the chain to EACH clip, and each source goes through its own pass — but the parameters of the pass are the same, so "something of its own for each camera" today is achieved not by the morpher, but by a second morpher with another `MorphTargets::Clips`. The mechanism for this exists; they do not have it at all.
2. **Blur/`Glow` across a join and across a crossfade.** For them the seam is blurred: the transition is computed BEFORE the effect. For us `PlanLayer::Dissolve` holds two `LayerSrc`, each with its own `fx` chain and its own `fx_hash`; the kernel runs twice, and `dissolve_into` mixes frames that are already processed. Honest assessment: for a PURE crossfade and a PURELY linear kernel (`Blur`) the difference is only u8 rounding, since convolution commutes with a weighted sum. The difference becomes visible in two places. First, `Glow` has a `threshold`, and `Edges` has a Sobel threshold: the thresholds are applied to each frame of the pair before mixing, so in the middle of the transition what glows or gets outlined is not what would glow on the mix. Second, and more important — the seam between LAYERS (not between pieces of the transition): `alpha_over` is not a weighted sum, and an area effect from a layer does not blur the boundary of a keyed object with the background beneath it.
3. **Geometry over stacked tracks.** `Pan`/`Pan3d`/`EventGrid` in our scheme move each layer in its own coordinate system: `pan_frame()` takes the dimensions of the SOURCE, and `LayerSrc.cam3d` is resolved PER LAYER. One Pan over a stack = N different framings; on the composite it would be one transformation of one frame of the project's size.
4. **Chroma key.** The only case where the per-clip scheme is strictly stronger: for us the key sees the pixels of one source and, having punched the alpha, reveals the lower track; on the composite the mixed background lies under the "green", and there is nothing to reveal (the Premiere defect, F-MPH-004). This gain must not be lost when a composite stage appears — it needs a regression test.
5. **Vignette/frame over the whole image.** For them — one pass over the finished frame. For us there is no such effect, and if it appeared it would give N vignettes, one per layer, and in picture-in-picture there would be two. This is a pure minus of our scheme, not compensated by anything.
6. **`EventGrid` and any effect that makes the frame opaque.** `has_alpha: false` on the output plus the copying branch of `alpha_over()` = the layer covers everything under it. On the composite the same effect would make one mosaic out of the finished picture.

### What already exists

- `MorphPatch::apply_to()` and `Project::effective_clips()` in `src/libs/kt-model/src/project.rs` — rewriting clips BEFORE the plan is built, accumulation bottom-up by `(Reverse(row), MorphId)`, cutting of partially covered targets.
- `VideoRenderer::frame_plan` / `build_plan` in `src/libs/kt-engine/src/video.rs`, `FramePlan`/`PlanLayer`/`LayerSrc` in `src/libs/kt-engine/src/video_plan.rs` — the plan describes LAYERS. There is no object in it "a finished frame to which a chain can be applied": `execute_plan_cpu` walks `plan.layers` and puts the result into `out`, after which the composition is finished.
- Both executors are already symmetrical: CPU `execute_plan_cpu` (`alpha_over`/`dissolve_into`/`mix_into`) and GPU from `src/libs/kt-gpu/src/engine.rs`.
- Per-layer memoization: `BurstKind::PostFxVideo { quality, fx_hash }` in `src/libs/kt-cache/src/lib.rs`; the key is computed by `fx_chain_hash()` (`src/libs/kt-engine/src/video.rs`) from the chain parameters, the automation curves and the clip geometry.
- **The FRAME-level key is already written by us — but only on the GPU.** `plan_key()` in `src/libs/kt-gpu/src/engine.rs` hashes the whole plan (t, size, for each layer — `source`, `cache_idx`, `fx_hash`, `u`, camera, `alpha_k`, transition-kind parameters) and feeds it into `ReadbackCache` (`src/libs/kt-gpu/src/pool.rs`). This is a ready answer to the question "under which key to cache the post-composite result": `plan_key` plus a hash of the post-composite chain.

### What is missing

- The stage "effect on the JOINED frame" — a node in `FramePlan` after `layers` and its execution in both backends.
- A place where such a stage lives in the model. Hanging it on a checkbox on `MorpherEvent` is impossible without a revision: the morpher has rich targeting (`Clips`/`Marks`/`Areas`), while the composite stage by its nature knows only "everything below" — UI rule 17 (paired ways behave alike) requires either matching behaviour or different objects.
- A post-composite cache key on the CPU. `BurstKey` requires `source: SourceId` (`src/libs/kt-cache/src/lib.rs`), and a composite has no source: one needs either a synthetic `SourceId` "frame", or a new kind of key not addressed by a source.

### What stands in the way

Not the volume of code, but four obstacles in a row.

1. **Invariant 7.** The stage must appear inside `VideoRenderer::frame_plan` and be executed by BOTH executors — the CPU reference and the WGSL mirror (`src/libs/kt-gpu/shaders/`); otherwise it is exactly a second composition path, the one the invariant is written against. Export, in this case, does not mix backends: a GPU error in the middle of an export remains an export error.
2. **Invariant 11.** The new stage goes through GOLDEN_VIDEO (an empty stage must be an identity, otherwise the reference shifts), through "preview == export byte-for-byte on one backend" (t67/t184), and through the cross-backend check within tolerances — only `kt_gpu::assert_frames_close`, ad-hoc epsilons are forbidden.
3. **Invariant 4 and the cache.** On the GPU the key exists (`plan_key` + `ReadbackCache`), on the CPU there is nothing to hang it on. At the same time the post-composite kind, like `PostFxVideo`, MUST NOT get into the coverage bars: invariant 4 remains about PRE-FX data (`src/libs/kt-cache/CLAUDE.md`), otherwise warmth would lie. Also `FramePlan.memoize_postfx` is deliberately disabled in export (R.31 item 4), and the post-composite stage must respect the same flag.
4. **Bake breaks conceptually.** `Command::BakeMorpher` (`src/libs/kt-model/src/model.rs`) computes `effective_clips()` and writes the result into the clips. A composite stage does not write into clips at all — so for an object in composite mode the bake button either does not exist, or it means something else. This is an independent argument for a SEPARATE object, not for a checkbox on `MorpherEvent`.

### Which invariants it touches

Root `CLAUDE.md`: **7** (one composition code — the stage is written in `frame_plan`, not in a backend or in export), **11** (determinism: GOLDEN_VIDEO untouched with an empty stage, CPU byte-for-byte, cross-backend via `assert_frames_close`), **4** (the cache is the only truth about coverage: the new kind of key must behave like `PostFxVideo` and not get into the bars). Invariant **5** is not touched: the stage generates no requests to the decoder. UI rules from `src/apps/kt-app/CLAUDE.md`: **13** (the mode must fit into the already occupied place of the morpher window, not into a new panel) and **17** (if one object ends up with two inputs, both must behave predictably — or they are different objects with different names).

## Estimate

| What | Answer |
|---|---|
| Size | XL — a new render stage on both backends, a new kind of cache key and a format version |
| Where the main work is | kt-engine (`video_plan.rs` — a node after `layers`; `video.rs` — `execute_plan_cpu`), kt-gpu (a second pass + `plan_key`), kt-cache (frame-level key), kt-model (the object and its targeting), kt-app (showing which mode a layer is in) |
| Project format | a new version of `metadata["kt"]`: currently `KT_META_VERSION = 4` (`src/libs/kt-otio/src/meta.rs`); the mode/object raises it to v5; old files are read as per-clip |
| Tests that will have to be written | golden "blur per-layer ≠ blur after composite" (both schemes, different bytes); GOLDEN_VIDEO without the stage does not move; CPU↔GPU conformance of the new stage on the pattern of t181/t182; preview == export (t67/t184); t36 (morpher == bake) stays green for the per-clip mode, and for the composite mode the decision on bake is fixed by a test; regression "the chroma key from the morpher still reveals the layer below" |
| Depends on | F-MPH-001 … F-MPH-005 (reconnaissance of the input in all five); the overlay-strength slider logically comes next — it needs the same object "untouched frame" |
| Breaks compatibility | no, if the mode is a new field with the default "as now"; changing the mode of an existing morpher changes the picture and must be explicit |

## Verdict

We take it as a SEPARATE object and not first. The per-clip input is not a defect and not an unfinished piece: from it grow source substitution, `Areas`/`Marks`, honest bake and a working chroma key — that is, everything with which the morpher outpaces other editors' adjustment layers. Giving this up for a composite stage is not acceptable, and combining the two under one event with a checkbox means getting an object with two different meanings, different costs, different targeting and a broken bake button (UI rule 17). Therefore the composite stage is a second object ("effect on the result"), its own round, a revision of invariants 7 and 4 and a new kind of cache key; the frame-level key for it is half written already (`plan_key` in `src/libs/kt-gpu/src/engine.rs`), while on the CPU there is nothing to hang it on. Until then the honest formulation of the divergence is this: we have no level whose input is the mixed-down picture, and therefore we have neither masks by frame area, nor a vignette over the whole image, nor wet/dry.

## Sources

- https://community.adobe.com/t5/premiere-pro-discussions/multi-camera-editing-best-way-to-assign-separate-adjustment-layers-to-each-source/td-p/15127444 — the layer does not address the source; the answer "use a Source Effect" (accessed 2026-09-07)
- https://creativecow.net/forums/thread/applying-motion-blur-to-each-layer-or-to-a-top-adj/ — the RSMB author on why his plugin loses tracking on the composite (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/color-correcting-with-adjustments-layers-with-cross-dissolves-on-clips/m-p/9748402 — a grade from a layer and a crossfade under it (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/luma-key-or-color-key-or-ultra-key-not-working-on-adjustment-layers-bug-or-limitation/td-p/14397400 — keying on a composite layer (2026-09-07)
- https://helpx.adobe.com/after-effects/using/creating-layers.html — the AE adjustment-layer input and the promise "once per frame" (per the documentation, 2026-09-07)
- https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part2983.htm — Clip vs. Timeline Grading: the Timeline grade runs on each clip (2026-09-07)
- https://davinciresolveclub.com/adjustment-clips-davinci-resolve/ — the adjustment clip sees the composite under it (2026-09-07)
- https://forum.borisfx.com/t/revisiting-an-old-bug-with-track-fx-vs-event-fx/25189 — the place of the VEGAS adjustment event in the pipeline (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/adjustmentevents.htm — Adjustment events, VEGAS Pro 2026 (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/add-adjustment-clips-verfda63436e/mac — Adjustment Clip in FCP (2026-09-07)

No editor was run live: the other side is assembled from official documentation and forum threads (some pages of `helpx.adobe.com` and the Blackmagic forum in this session returned 403 and were read through search-result snippets). OUR side is checked against the code — every statement about Kinotrain rests on a file read from `src/`.
