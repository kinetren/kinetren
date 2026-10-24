---
id: F-MSK-003
title: Track matte — one clip sets the transparency of another
status: candidate
priority: B
cost: L
category: masks/keying
editors: [after-effects-26, premiere-26, vegas-pro-2026, resolve-21, fcp-12]
we_have: no
crates: [kt-model, kt-engine, kt-gpu, kt-fx, kt-media, kt-cache, kt-otio, kt-app]
invariants: [4, 5, 7, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MSK-003 — Track matte — one clip sets the transparency of another

## Essence

The editor puts two clips on top of each other and says: don't show the upper one, use it as a stencil for the lower one. White (or opaque) in the stencil — the lower clip is visible, black (transparent) — a hole, gray — semi-transparent. Four kinds: **Alpha** (the stencil's alpha is taken), **Alpha inverted**, **Luma** (the stencil's BRIGHTNESS is taken), **Luma inverted**. Title "text-window" effects, light reveals, reveal transitions, face blurring over a moving spot all rest on this — anything where the transparency shape changes over time. In everyday use people almost always take luma: any gradient, any black-and-white video, any text on black becomes a mask without a single drawn outline.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | Track Matte — a property of the LAYER (plus a separate Set Matte effect) | the layer has a TrkMat column: the kind (Alpha / Alpha Inverted / Luma / Luma Inverted) + a REFERENCE to the matte layer. Before AE 23.0 (10.2022) there was no reference — the matte was the layer exactly above. From 23.0: "Any layer in your comp, no matter where it sits in the layer stack, can be used as a Track Matte source for any other layer. You can even have multiple layers all reference a single layer for their matte". AE hides the matte layer itself and does not switch it back on |
| Adobe Premiere | 26.x | Track Matte Key — an EFFECT | attached to the carrier clip; the `Matte` parameter is a drop-down list of TRACKS (`Video 2`, `Video 3`, …), `Composite Using` — `Matte Alpha` / `Matte Luma`, plus `Reverse`. The stencil track is hidden by the editor; the editor does not touch it. The stencil follows the carrier's Position INDEPENDENTLY of the effect order — in the words of an Adobe moderator, a "longstanding quirk" and "it has never behaved the same way as it does in After Effects". In version 26 Adobe moved MASKS (not mattes) from Effect Controls into Object Mask / Vector Masking on the toolbar and recommends them instead of Track Matte Key |
| VEGAS Pro | 2026.0 | there is no "track matte" mode: the track compositing mode `Multiply (Mask)` + Parent/Child + the Mask Generator plugin | the matte is a PARENT TRACK, and what is masked are its child tracks: "The parent track is the highest track in a group of tracks and the behavior of the child tracks is determined by the parent track". A non-gray image is brought to a matte by the Mask Generator plugin as a Track FX. Track kinship is part of the project structure, not a property of the clip (the same logic as Adjustment Events, F-MPH-001, and Video Bus Track, F-MPH-005) |
| DaVinci Resolve | 21 (also checked on 20.x) | there is no "track matte" entity on the Edit page | three paths: (1) the compositing modes `Alpha` / `Inverted Alpha` / `Foreground`, with which the upper track works as a matte for the lower one — they appeared in Resolve 17, release notes: "Support for compositing using track mattes or external alpha" [search-result snippet, the primary source did not open]; (2) External Matte on the Color page — a file is attached to a clip in the media pool ("Add as Matte") and becomes an input of the node; (3) Fusion — MatteControl/Merge, the alpha returns to Edit |
| Final Cut Pro | 12.3 | there is NO track matte, NO Set Matte effect | instead of a reference — the blend modes of the "alpha channel" group on the stencil clip itself: Stencil Alpha, Stencil Luma, Silhouette Alpha, Silhouette Luma. Apple: "Does the same thing as the Stencil Alpha blend mode, but uses the affected clip's luma value to define transparency. Stencil Luma is useful if the clip you want to use for cropping has no alpha channel of its own". The closest "matte from someone else's picture" is the Image Mask effect, but it takes the channel from a SEPARATE image, not from a clip on the neighboring track |

## The trick behind it

**1. The matte layer is switched out of the display — and the three editors solve this in three different ways, from which three different rakes grow.** AE turns off the eye of the matte layer itself and does NOT switch it back on when the matte is removed: the layer stays in the composition but is invisible, and a newcomer thinks they have lost it. This is not a bug but a consequence of the model: "Layers used as mattes are not normally on (visible)", and since AE 23.0 it cannot be otherwise — one matte serves several layers, and "turning the eye back on" after removing the matte from ONE of them would be an error. Premiere turns nothing off: the stencil track keeps being drawn on top of everything, and it is switched off by hand — forget, and instead of the cut-out you see the stencil itself. FCP does not turn it off and cannot: there the stencil is a clip with the Stencil blend mode, that is, it is itself the carrier of the effect.

**2. An explicit reference to a matte costs more than it seems, and Adobe had to roll back half of it.** In AE 23.0 the implicit link ("the layer exactly above") was replaced by choosing a layer from a list and the pick whip. The gain is obvious — one stencil for N layers. The price turned out to be threefold: (a) the frequent case became slower, and this was said in plain text — "The new feature solved a problem that sometimes happens, but completely ruined the workflow for 90% of the rest of the shots"; Adobe returned the "Matte with Layer Above" command with an assignable hotkey only in 23.4, half a year later; (b) the reference got into the project format, so 23.0 breaks compatibility — "Projects saved in the 23.0 beta will not be able to be opened in other versions of After Effects"; (c) the reference does not move when copying: "I really wish these track mattes would update their parent link when you copy and paste a group of them". Point (c) is already closed for us for morphers (item 242, tests 249/250: a reference to a COPIED name moves to the copy, to a non-copied one it stays on the original), and this is ready-made semantics for a matte.

**3. The matte lives in someone else's time and someone else's geometry — and this is still broken.** The stencil is a separate clip with its own length, trimming and keyframes; the carrier is another. Nobody stretches the stencil to the carrier: they take the stencil's frame at the SAME point of the timeline. Hence the Premiere 26.0.1 defect: "the Track Matte key plug-in does not recognise that the source clip has been trimmed and offsets the keys as if the clip is untrimmed" — exactly the question that we have closed with the decision "key domain = material tick" (F-AUT-002). As for geometry — the mirror-image trouble: the carrier's Position DRAGS the stencil along independently of the effect order, the standard cure is nesting, and an Adobe Community Expert in the same thread answers "Have not used Track Matte in a long time".

**4. A small thing that spoils the edges: premultiplied alpha and size.** AE's Set Matte effect carries the checkboxes "Premultiply Matte Layer" and "Stretch Matte to Fit": a stencil of a foreign size is RESAMPLED, not cropped, and a premultiplied one without the checkbox gives a dark fringe. Plus a historical gotcha: Set Matte took the layer's SOURCE, ignoring its masks and effects — it was fixed at the SDK level ("Effects that use layers as an input, such as Set Matte and Displacement Map, can now target the input layer's masks and effects, instead of only the source of the layer"). That is, "a matte from a layer" and "a matte from what this layer draws" are two DIFFERENT things, and the authors themselves confused them.

## Pros

- The stencil is an ordinary clip, so EVERYTHING that acts on a clip acts on it: the effect chain, automation, trimming, reverse. A soft edge comes from blurring the stencil, not from a `feather` parameter on each effect; the shape over time comes from ordinary keyframes.
- A luma matte requires no alpha from anywhere: a black-and-white gradient, a pulled shadow, a title on black — these are ready-made masks. Apple's wording is exactly about this: luma is needed "if the clip you want to use for cropping has no alpha channel of its own". Larry Jordan explains the choice of luma the same way: "while Final Cut text has alpha channels built into it, the generated shapes do not".
- An explicit reference (AE 23.0+) makes the stencil a reusable object: editing one clip rewrites the shape at once for N carriers — without N edits and N undo steps.
- For compositing this is ONE operation: "multiply the layer's alpha by a number from another frame". No second compositing code appears — the source of the alpha changes, not the way layers are stacked.

## Cons

- The stencil disappears from the picture, staying in the project, and is marked by nothing except a switched-off "eye": it can be accidentally moved, trimmed or deleted — and a place nobody touched will break.
- A reference by track number (Premiere) breaks on insertion, deletion and reordering of tracks — operations people do without thinking.
- A reference by layer (AE) costs speed in the frequent case "stencil exactly above" and gets into the project format, breaking backward compatibility. Copying the pair "carrier + stencil" does not rebind the reference.
- Two sources are read per frame instead of one, each with its own chain: decoding, cache volume and the chance of missing the deadline all double.
- The stencil and the carrier are linked neither in time nor in geometry: trim the carrier — the stencil stays; move the carrier with Position in Premiere — the stencil moves with it, although nobody asked for that.
- The premultiplied alpha of the stencil gives a fringe, and a stencil of a different resolution is scaled to the carrier, not cropped: "put 4K on HD" is not a crop, and the edges will shift.

## What users say

The layer really does "disappear", and people ask about it in plain text:

> This may be normal AE behavior, does a layers visability (eye ball)
> always stay off after selecting/removing a track mattee layer in the
> drop down?
> — 55878965, Adobe Support Community, 21.12.2023,
> https://community.adobe.com/questions-529/layer-visability-stays-off-after-removing-track-matte-is-that-normal-ae-behavior-54298

The answer of an Adobe Community Expert in the same thread is the whole design of the feature:

> In the latest versions of After Effects, you can use the same layer as a
> track matte for multiple layers. Turning the Matte layer back on after
> turning off the Track matte on one of the layers would be a mistake.
> — Rick Gerard, there, 21.12.2023

The switch to an explicit reference (AE 23.0) is the most valuable negative feedback in this note: the feature was asked for for years, and once received, people regretted it:

> The new feature solved a problem that sometimes happens, but completely
> ruined the workflow for 90% of the rest of the shots 😞
> — Filip23315833k75u, Adobe Community (After Effects Ideas), 11.11.2022,
> https://community.adobe.com/t5/after-effects-ideas/2023-new-track-matte-feature-needs-some-work/idi-p/13339012

The same change, seen a day after the release:

> It looks like bug to me because there shouldn't be two Parent & Link
> windows/dropdowns. Especially when one is labeled Track Matte but works
> like parent & link. It's a bug
> — anonymous, Adobe Community, 21.10.2022,
> https://community.adobe.com/t5/after-effects-discussions/track-matte-disapeared/m-p/13291308

The reference does not move on copying — exactly what we have closed with item 242:

> I really wish these track mattes would update their parent link when you
> copy and paste a group of them.
> — jackthegiantkiller, Adobe Community, 31.05.2023,
> https://community.adobe.com/t5/after-effects-beta-discussions/feature-focus-selectable-track-matte-layers-have-arrived/td-p/13110013

The model "an effect with a reference to a track" (Premiere) has earned a reputation for unreliability, up to abandoning the feature — and this from a Community Expert:

> I've found the Track Matte effect in Premiere Pro to be unreliable
> (sometimes it mattes, sometimes it doesn't, even in the same sequence
> with the same matte).
> — Chris Walsh, Creative COW, 10.09.2020,
> https://creativecow.net/forums/thread/track-matte-premiere-pro/

> Have not used Track Matte in a long time: switch over to the Properties
> Panel. Clip and matte can be moved around independently.
> — Ann Bens (Community Expert), Adobe Community, 07.04.2026,
> https://community.adobe.com/questions-729/track-matte-key-does-not-respect-effect-order-1556713

A fresh (March 2026) bug report — about decoupling the times of the stencil and the carrier:

> Now in 2026.0.1 the Track Matte key plug-in does not recognise that the
> source clip has been trimmed and offsets the keys as if the clip is
> untrimmed.
> — LastArkTom, Adobe Support Community, 02.03.2026,
> https://community.adobe.com/bug-reports-728/track-matte-key-not-working-with-keyframes-in-pp-2026-0-1-1552070

The rigid track order is a beginner's first mistake, and it is taken for a breakdown of the matte's KIND:

> The effect works, but only in the mode 'Matte Luma' and not in 'Matte
> Alpha'... when i use the setting Alpha Matte it dosen't work as a mask.
> — Alejandro Andersson, Creative COW, 20.04.2010,
> https://creativecow.net/forums/thread/frustrating-error-whit-track-matte-key/

> I think your matte clip has to go on the next track up, not underneath.
> — Eric Jurgenson, there, 20.04.2010

Why people use luma rather than alpha in everyday work:

> Why use Travel Matte – Luma? Because, while Final Cut text has alpha
> channels built into it, the generated shapes do not.
> — Larry Jordan, 15.05.2011,
> https://larryjordan.com/articles/travel-mattes-blurring-a-face-to-protect-the-innocent/

And the same rule in colloquial language in the Premiere thread:

> If you keep it white use the luma matte setting with the Track Matte
> Key. If you make it a color use the alpha matte setting.
> — Paul Neumann, Creative COW, 11.09.2014,
> https://creativecow.net/forums/thread/track-matte-problem-cant-choose-target-video-fro/

Finally — how it looks for a refugee from AE into Vegas (April 2026):

> I right click on the b/w clip and change alpha to 'straight (unmatted)
> as seen on a tutorial but nothing works? How do I use this to knock out
> the background correctly please?
> — johnnyswedish, forum.borisfx.com (VEGAS Pro), 07.04.2026,
> https://forum.borisfx.com/t/22023

> On the track header use 'Multiply (Mask)' From the video FX window -
> Utility - drag Mask Generator onto the B/W mask. Parent/Child the mask &
> logo clip to see the track below.
> — Gid Joiner, there, 07.04.2026

## How it fits Kinotrain

### What already exists

- **The composite already handles alpha, and handles it as STRAIGHT** (not premultiplied): `alpha_over` in `src/libs/kt-engine/src/video.rs` (line 908), the AVX2 twin `alpha_over_avx2` in `src/libs/kt-engine/src/video_simd.rs` (line 154), the WGSL mirror of `over()` in `src/libs/kt-gpu/src/shaders/composite.wgsl` (line 9). This is the key fact for the estimate: with straight alpha a matte is a multiplication of ONE channel, and RGB need not be touched. With premultiplied alpha one would have to multiply all four and regenerate the golden references.
- **Multiplying alpha by a weight is already written three times** — in scalar code, in AVX2 and in the shader: `scale_alpha_into` / `scale_alpha_inplace` (`src/libs/kt-engine/src/video.rs`, lines 818 and 808), `scale_alpha_avx2` (`video_simd.rs`, line 107), `scale_into` (`composite.wgsl`, line 22). A matte is exactly this operation, but the multiplier is taken from the pixel of ANOTHER frame, not from the scalar `alpha_k`.
- **An integer luma for a luma matte already exists and is already frozen**: `(77·R + 150·G + 29·B) >> 8` in `src/libs/kt-fx/src/video.rs` (line 419 in `edges`, line 685 in `glow`). There is no need to introduce a luma formula of our own — on the contrary, a second formula would be a defect.
- **The alpha flag of a frame**: `VideoFrame.has_alpha` (`src/libs/kt-media/src/lib.rs`, line 58) and `Frame.has_alpha` (`src/libs/kt-fx/src/video.rs`, line 25).
- **The only producer of alpha today is chroma key**: `greenscreen()` sets `f.has_alpha = true` (`src/libs/kt-fx/src/video.rs`, line 385), the mirror is `src/libs/kt-gpu/src/shaders/greenscreen.wgsl`.
- **Precedent "a layer takes a SECOND frame from the cache, and their shared plan code gets in the way"**: `LayerSrc.blend` / `LayerBlend` in `src/libs/kt-engine/src/video_plan.rs` (lines 62 and 67), filled in `layer_src` (`video.rs`, line 486). The comment there explains directly why the blending is done in the plan rather than in the executors: "invariant No. 7 — one composition code, the backend is a parameter of execution". A matte fits this precedent one-to-one.
- **Precedent of a reference by a semantic key, not by a row number** — and it was accepted deliberately: `MarkedArea.lanes`, `src/libs/kt-model/src/project.rs` (line 1163+): "SEMANTIC track keys, not row numbers: swapping rows does not move the area". Nearby is `MorphTargets::Clips { ids }` (line 613): a frozen list of `ClipId` that survives move/split/trim.
- **Precedent of rebinding references on copy-paste** — the very thing AE still does not have: item 242, tests 249/250. `ops::ClipBlock` carries morphers, areas and marks; "a morpher's reference to a COPIED name moves to the copy, to a non-copied one it stays on the original", and an element that has no targets left is SKIPPED and counted, not "fixed up" (rule 20).
- **z-order and the Vegas rule**: `ClipEvent.lane` (`project.rs`, line 1509, "z-order for video, smaller = higher") and the sort `clips.sort_by_key(|c| (Reverse(c.lane), c.start.0))` (`src/libs/kt-engine/src/video.rs`, line 331) — "on top is the one that started later".
- **Post-FX memoization with a key**: `fx_chain_hash` (`src/libs/kt-engine/src/video.rs`, line 153) and `BurstKind::PostFxVideo { quality, fx_hash }` (`src/libs/kt-cache/src/lib.rs`, line 31).
- **There are no masks, mattes or rotoscoping at all**: a grep for `matte|mask` over `src/libs` and `src/apps` yields only `front_mask` (the transition shape in `transitions.rs` and `composite.wgsl`) and `mask_digits` (the timecode mask in `src/apps/kt-app/src/util.rs`). Not a single model field, not a single FX.

### What is missing

- **A source of alpha from a FILE.** `FfmpegSource::read_video_frame` and `read_video_span` hard-set `has_alpha: false` (`src/libs/kt-media/src/ffmpeg.rs`, lines 133 and 160) — even though swscale converts the frame into `AV_PIX_FMT_RGBA` (`src/libs/kt-ffmpeg/src/source_impl.rs`, line 928), that is, the alpha bytes are physically in the buffer, but the engine is told to treat the frame as opaque. The same holds for `placeholder.rs` (line 254) and `synthetic.rs` (line 204). **Conclusion: today we have nowhere to take an Alpha matte from, except the output of chroma key; a Luma matte is the only kind that would work without changes in the decoder.** This is not a matter of taste but a consequence of one line.
- **Model fields "this clip is my stencil"** and of the matte kind: `ClipEvent` (`src/libs/kt-model/src/project.rs`, line 1502+) has nothing similar.
- **One more input for the composite.** The header of `composite.wgsl`: "T0 = acc_in, T1 = layer A, T2 = layer B (dissolve)"; the bind group provides exactly three textures (`src/libs/kt-gpu/src/engine.rs`, lines 1039–1041, bindings 1–3). The stencil is the fourth.
- **Access of the FX chain to a foreign frame.** `apply_video_chain_pooled(fx, frame, ctx, scratch)` (`src/libs/kt-fx/src/video.rs`, line 61) receives EXACTLY ONE frame and no access to the cache at all. So the Premiere model ("the matte is an effect") cannot be attached to us in principle: the matte must be born in the PLAN, where the frames are taken.

### What gets in the way

1. **The model "track strictly above" does not fit our scheme.** We have no container tracks with a fixed order: there is `lane` (z-order) and LEGAL overlaps on one lane with automatic crossfade. Putting a stencil "strictly above the carrier" means: (a) occupying a lane with it, where it takes part in the general sort; (b) handing the fate of the pair to the rule "on top is the one that started later" — shift the carrier to the right, and it itself becomes the upper one; (c) when the stencil overlaps a neighbor in the same lane, getting a crossfade between them, which has nothing to do with the matte. The Premiere model ("matte = track number") is even worse: the row order is changed through undo (`Project::set_lane_order`, `project.rs`, line 2989), and any reordering would rewrite all references — exactly what we already gave up in `MarkedArea.lanes`. **Only an explicit reference to a clip by `ClipId` is compatible with our model**, like `MorphTargets::Clips { ids }`: it survives move, trim, split and reordering of rows, and it also gives for free what AE broke its format for in 23.0 — one stencil for several carriers, with the copy-paste semantics already written (item 242).
2. **An explicit reference must have a fast gesture.** The experience of AE 23.0 ("ruined the workflow for 90% of the rest of the shots", the return of the "Matte with Layer Above" command half a year later) reads as a direct requirement of rule 17 of the UI checklist: the clip picker and the gesture "take the clip directly above as a stencil" must BOTH exist, give the same result and the same entry in the history.
3. **The stencil is a clip with its own chain, not a picture.** The plan builds a FULL `LayerSrc` for it: its own source, its own `cache_idx`, its own `Cow<[FxInstance]>` with already resolved automation, possibly its own `blend`. This is a recursion inside `build_plan` (`src/libs/kt-engine/src/video.rs`, line 538), and it must be cut off: a matte of a matte — either a prohibition in the model or a depth limit.
4. **Excluding the stencil from the display.** Technically this is a `continue` in `build_plan`. But rule 20 of the checklist in `src/apps/kt-app/CLAUDE.md` ("nothing grows or switches off by itself") directly forbids silently hiding what the user has placed on the timeline — and this is exactly where AE loses the layer. So the stencil clip remains VISIBLE on the timeline and explicitly marked, and "is not drawn in the frame" is read from its block, not discovered by elimination.
5. **The cache keys lie.** `fx_chain_hash` and `BurstKind::PostFxVideo { fx_hash }` do not know about the matte: two clips with the same chain and different stencils would collapse into one post-FX frame. The same defect is in the `plan_hash` of the GPU executor.
6. **GPU.** A fourth texture in the bind group, a new `mode` in `composite.wgsl`, and a byte-for-byte mirror of the integer formula. The rule of `src/libs/kt-fx/CLAUDE.md` — the scalar code, the AVX2 kernel and the WGSL are edited in ONE commit, SIMD byte-for-byte with the scalar code.
7. **Format.** A new field → `KT_META_VERSION` (`src/libs/kt-otio/src/meta.rs`, line 17) is raised from 4 to 5: an old reader would lose the reference silently, and the project would open "simply without the reveal". AE 23.0's breaking format change is a warning, not a model: we already have the envelope version and it knows how to complain (item 243).
8. **Starvation.** The stencil is a second source per frame, that is, a second reason for `complete = false` in `execute_plan_cpu` (`src/libs/kt-engine/src/video.rs`, line 624). The policy is chosen ONCE and identically for CPU and GPU: "no stencil → the carrier is visible in full" and "no stencil → the carrier is not visible" give different pictures on a cold cache, and if the executors diverged, the preview would stop matching the export.

### Which invariants it touches

- **No. 7 "ONE composition code" — the central one.** The invariant names explicitly what counts as the single code of `VideoRenderer::frame_plan`: the layers, the Vegas order, the weights of transitions/fades, LEVELS and the mapping "tick → source frame". A matte is a per-pixel level, that is, the same entity as `alpha_k`. So "who the stencil is, which frame it has, which kind and inversion" is resolved in the plan, and the executors receive a ready result — as is already done for `LayerBlend`. A separate "matte in the shader" cannot exist by definition of the invariant.
- **No. 4 "the cache is the single source of truth about coverage"** — the matte enters the post-FX cache key, and its pre-FX frame is an ordinary `BurstKind::Video` entry, which must get into the coverage bars on a par with the carrier.
- **No. 5 "one MediaWorker owns the decoder"** — the second source per frame goes through the same priority queue; prefetch must warm the stencil too, otherwise "no data by the deadline → last frame" will fire twice as often and unnoticed (the frame is there, just without the reveal).
- **No. 11 "determinism"** — CPU byte-for-byte against GOLDEN_VIDEO, and first of all in the ABSENCE of a matte (references are not regenerated); GPU against CPU — only via `kt_gpu::assert_frames_close`.
- **No. 8** — the thumbnail of the stencil clip is taken from the sidecar index like everyone else's; no "let's really draw the stencil on the timeline".
- UI rules of `src/apps/kt-app/CLAUDE.md`: **20** (the stencil does not switch off or hide itself — it is marked), **13** (the reference lives in the already existing clip inspector, not in a new panel), **17** (the picker and the gesture "clip above" behave identically and give one undo entry).

## Estimate

| What | Answer |
|---|---|
| Size | L — a whole round. Cheaper than it looks from outside: the composite is already straight-alpha, alpha multiplication is already written in scalar/AVX2/WGSL, the integer luma is already frozen, the semantics of rebinding references on copy-paste already exist (item 242). More expensive than "one more effect": the matte must be born in the plan, reach the GPU as a fourth texture, and get into the cache keys and into the format |
| Where the main work is | kt-engine (`video.rs`/`video_plan.rs`: the stencil's `LayerSrc`, exclusion from the stack, starvation policy, key), kt-gpu (fourth texture + mode in `composite.wgsl` + parity), kt-model (`ClipId` reference, kind, inversion; behavior on delete/split/replace of the source and on copy-paste), kt-cache (key), kt-otio (v5), kt-app (picker + gesture + label on the stencil clip). Separately and OPTIONALLY — kt-media: remove `has_alpha: false`, if an Alpha matte is needed |
| Project format | needs v5 (`KT_META_VERSION` 4 → 5); the field is not serialized when `None` — old files stay byte-stable |
| Tests that will have to be written | round-trip of the model and reading of files without the field; luma matte against the CPU reference; "no stencil → GOLDEN_VIDEO does not move"; CPU↔GPU conformance following the pattern of t181/t182 via `kt_gpu::assert_frames_close`; preview == export on one backend (pattern of t67/t184); distinguishing stencils in the post-FX cache key (pattern of t66); stencil deleted / split / replaced by a placeholder (`placeholder.rs`, pattern of t209/t211); copy-paste of the pair "carrier + stencil" rebinds the reference, copy-paste of the carrier alone does not (pattern of t205 and test 249); stencil under a morpher and bake (t35/t36); stencil on a reversed clip; a layout test of the label and the picker in `apps/kt-app/src/layout_tests.rs` |
| Depends on | nothing — for a luma matte from video. An Alpha matte depends on "alpha from a file" (change in `kt-media/src/ffmpeg.rs` + a conformance test on a file with alpha). A stencil from a DRAWN shape — on F-MSK-001. Titles as a stencil — on the `titles/` category |
| Breaks compatibility | no |

## Verdict

We take it — and we take the **luma matte by an EXPLICIT reference to a clip (`ClipId`)**, not the model "track strictly above". The justification is not a matter of taste: (1) our layer scheme is the lane plus legal overlaps with the rule "on top is the one that started later", and in it "strictly above" cannot be expressed without the pair "carrier + stencil" drifting apart on an ordinary move; (2) we have already rejected a reference by row number once, in `MarkedArea.lanes`, and there is no point in repeating Premiere's mistake ("Matte: Video 3"); (3) the explicit reference gives for free what AE broke its format for in 23.0 — one stencil for several carriers, and for it we already have the missing half that AE lacks: rebinding on copy-paste (item 242).

From AE's mistakes we take the lesson in full: the explicit reference must come TOGETHER with a fast gesture "the clip above serves as the stencil" (rule 17), otherwise we will repeat "ruined the workflow for 90% of the rest of the shots" and, like Adobe, will be returning the command half a year later.

We consciously postpone the Alpha matte, with the reason recorded: we have no source of alpha from a file (`has_alpha: false` in `src/libs/kt-media/src/ffmpeg.rs`), so an "Alpha matte" today would be a menu item working only on the output of chroma key. This is separate small work (decoder + conformance test), and it must be done BEFORE the word "Alpha" appears in the UI.

Our stencil does not switch off silently (rule 20): it is marked on the timeline and remains a visible block. This is a direct divergence from AE — and it is precisely this that removes the most widespread complaint about this feature.

## Sources

- https://community.adobe.com/t5/after-effects-beta-discussions/feature-focus-selectable-track-matte-layers-have-arrived/td-p/13110013 — **primary source on AE**: Catie Bass (Adobe), 02.08.2022 — announcement of Selectable Track Matte Layers for 23.0 beta ("Any layer in your comp, no matter where it sits in the layer stack, can be used as a Track Matte source for any other layer… You can even have multiple layers all reference a single layer for their matte"), the warning about a format-breaking change ("Projects saved in the 23.0 beta will not be able to be opened in other versions of After Effects") and about expressions counting by layer indices; the same thread — "this feature has shipped to After Effects 23.0" (18.10.2022), menu commands in 23.3 build 9 (26.01.2023), the return of the "Matte with Layer Above" command in 23.4 (14.06.2023), the complaint about references not rebinding on copy-paste (31.05.2023). Accessed 2026-09-07
- https://community.adobe.com/t5/after-effects-ideas/2023-new-track-matte-feature-needs-some-work/idi-p/13339012 — AE 23.0: "completely ruined the workflow for 90% of the rest of the shots" (11.11.2022; accessed 2026-09-07)
- https://community.adobe.com/t5/after-effects-discussions/track-matte-disapeared/m-p/13291308 — AE 23.0: "Where is Track Matte?" and "It's a bug" a day after the release (20–21.10.2022; accessed 2026-09-07)
- https://community.adobe.com/questions-529/layer-visability-stays-off-after-removing-track-matte-is-that-normal-ae-behavior-54298 — AE: switching off the stencil's visibility as standard behavior, in the words of an Adobe Community Expert (21.12.2023; accessed 2026-09-07)
- https://www.meganfriesth.com/aefundamentals/mattes — AE, summary of the mechanics: "In previous versions of After Effects, you could only use the layer directly above the affected layer as a matte", "After Effects will automatically turn off the visibility (eyeball icon) of the matte layer" (learning resource; consistent with the primary Adobe source above; 2026-09-07)
- https://creativecow.net/selectable-track-mattes-in-after-effects/ — Andy Ford, 24.10.2022: "you can either pickwhip to the layer or choose it from a dropdown" (2026-09-07)
- https://community.adobe.com/bug-reports-728/track-matte-key-not-working-with-keyframes-in-pp-2026-0-1-1552070 — Premiere 2026.0.1: Track Matte Key does not account for trimming of the stencil clip (02.03.2026; accessed 2026-09-07)
- https://community.adobe.com/questions-729/track-matte-key-does-not-respect-effect-order-1556713 — Premiere 26: the stencil follows the carrier's Position independently of the effect order; Adobe moderator Rach McIntire — "longstanding quirk… it has never behaved the same way as it does in After Effects", recommendation to switch to masks in the Properties panel; Ann Bens — "Have not used Track Matte in a long time" (07.04.2026; accessed 2026-09-07)
- https://community.adobe.com/questions-729/premiere-2026-masking-faqs-links-1555462 — Adobe (kes_akalaonu), 27.03.2026: in Premiere 26 masks moved from Effect Controls into Object Mask / Vector Masking, "this isn't a bug" (accessed 2026-09-07)
- https://creativecow.net/forums/thread/track-matte-problem-cant-choose-target-video-fro/ — Premiere: choosing the stencil by track NUMBER and the impossibility of choosing the needed number; there also the everyday rule "white → luma, color → alpha" (11.09.2014; accessed 2026-09-07)
- https://creativecow.net/forums/thread/frustrating-error-whit-track-matte-key/ — Premiere: "matte clip has to go on the next track up" and an alpha matte not working without alpha (20.04.2010; accessed 2026-09-07)
- https://creativecow.net/forums/thread/track-matte-premiere-pro/ — Premiere: the reputation of Track Matte as unreliable (10.09.2020; accessed 2026-09-07)
- https://charlescabrera.com/using-the-track-matte-key-effect-in-premiere-pro-2022-all-you-need-to-know/ — Premiere: the `Matte` parameter is a list of TRACKS ("under matte, I'm going to select video 2"), Matte Alpha versus Matte Luma (secondary source; 2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/8-design/compositingandmasks.htm — VEGAS: "The Compositing Mode button determines how the transparency in a video track is generated", "The parent track is the highest track in a group of tracks and the behavior of the child tracks is determined by the parent track", the `Multiply (Mask)` mode, Mask Generator (Vegas 22 help; the page of the same topic on the Boris FX CDN for 2026 returned 403, so "in 2026 the mechanics are the same" — [NOT VERIFIED] against the 2026 help, but confirmed by the live thread below; 2026-09-07)
- https://forum.borisfx.com/t/22023 — VEGAS Pro, 07.04.2026: a user who came from AE, and a working recipe "Multiply (Mask) + Mask Generator + Parent/Child" (accessed 2026-09-07)
- http://vegaspro.helpmax.net/en/using-vegas-software/applying-effects/compositing-video/compositing-and-masks/ — VEGAS: "To reproduce the 4.0 parent/child masking behavior, set the compositing mode of the parent track to Multiply, and then apply the Mask Generator" (mirror of the old help; 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/types-of-blend-modes-ver007271c06/mac — FCP: Stencil Luma "Does the same thing as the Stencil Alpha blend mode, but uses the affected clip's luma value to define transparency. Stencil Luma is useful if the clip you want to use for cropping has no alpha channel of its own" (the page is long, WebFetch returns it truncated — the definition was obtained from an indexed snippet of the same page; 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/intro-to-masking-verce5464e1a/mac — FCP: the masking toolset (Shape/Draw/Graduated/Image/Magnetic/Scene Removal/Vignette/Auto Mask); Image Mask takes the channel from a SEPARATE image, not from a clip on a neighboring track; there is neither a track matte nor Set Matte in FCP (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/use-alpha-channels-ver52cf382e/mac — FCP: the alpha channel as a way of compositing; the manual says nothing about straight/premultiplied (2026-09-07)
- https://larryjordan.com/articles/travel-mattes-blurring-a-face-to-protect-the-innocent/ — a travel matte requires three layers, and why luma is used (15.05.2011; accessed 2026-09-07)
- https://mixinglight.com/color-grading-tutorials/external-mattes/ and https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolveManual_files/part494.htm — Resolve, External Matte: mattes are attached to a clip in the media pool and become inputs of the node on the Color page (texts obtained from indexed snippets: a direct download of the mirror of the manual returned 404; 2026-09-07)
- https://ae-plugins.docsforadobe.dev/print_page/ — AE SDK: "Effects that use layers as an input, such as Set Matte and Displacement Map, can now target the input layer's masks and effects, instead of only the source of the layer" (the version in which this appeared is [NOT VERIFIED]; 2026-09-07)
- https://www.freevisuals.net/post/after-effects-set-matte — AE Set Matte: "Take Matte From Layer", choice of channel, "Invert Matte", "Premultiplied"; the difference from Track Matte — "without requiring that source to sit in any specific position relative to each individual layer it is controlling" (secondary source; 2026-09-07)
- item 242 (copy-paste carries morphers, areas and marks; tests 249/250) and item 243 (explicit errors of the `metadata.kt` envelope)
- Code (read, not retold by analogy): `src/libs/kt-engine/src/video.rs`,
  `src/libs/kt-engine/src/video_plan.rs`, `src/libs/kt-engine/src/video_simd.rs`,
  `src/libs/kt-gpu/src/shaders/composite.wgsl`, `src/libs/kt-gpu/src/engine.rs`,
  `src/libs/kt-fx/src/video.rs`, `src/libs/kt-model/src/project.rs`,
  `src/libs/kt-media/src/lib.rs`, `src/libs/kt-media/src/ffmpeg.rs`,
  `src/libs/kt-media/src/placeholder.rs`, `src/libs/kt-ffmpeg/src/source_impl.rs`,
  `src/libs/kt-cache/src/lib.rs`, `src/libs/kt-otio/src/meta.rs`

None of the five editors was launched live. `helpx.adobe.com`, `reddit.com` and `forum.blackmagicdesign.com` did not open from this environment (403/unavailable), so the AE and Premiere rows rely on the official Adobe community (including posts by Adobe employees — this is the primary source on the change in 23.0), Creative COW and secondary analyses; the Vegas row relies on help 22 plus a live thread on the official Boris FX forum; the Resolve row relies on mirrors of the manual and search-result snippets, and the presence of the compositing modes `Alpha`/`Inverted Alpha` on the Edit page of Resolve 17+ is marked `[NOT VERIFIED]`. The statements "we have / we do not have" were checked by grep over `src/`.
