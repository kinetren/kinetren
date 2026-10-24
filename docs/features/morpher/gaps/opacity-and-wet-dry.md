---
id: F-MPH-006
title: Blend strength slider on a morpher layer
status: candidate
priority: B
cost: M
category: morpher/gaps
editors: [vegas-pro-2026, after-effects-26, premiere-26, resolve-20, fcp-12]
we_have: no
crates: [kt-model, kt-fx, kt-engine, kt-gpu, kt-otio, kt-app]
invariants: [2, 3, 4, 7, 11]
ui_rules: [8, 13, 17, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MPH-006 — Blend strength slider on a morpher layer

## Essence

The editor put a grade, a blur and grain on the morpher layer, looked at it and says: "a bit too much." He needs a handle "the same thing, but at 40%" — one for the whole layer, not a walk through ten parameters of ten effects. Today the morpher has exactly two gradations: `enabled` on/off and the bitwise switches of the slots. This is the only divergence from Vegas that the user named himself, and with which the whole reconnaissance began (item 245).

The note is not about another editor but about OUR gap, so "Who has it and how" shows not "has/hasn't" but BY WHAT METHOD each one provides this handle: there are three methods, their cost differs, and the picture differs too.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | event opacity, corner fades, track Level and the composite-level envelope | a COMPOSITE method: the help lists for the adjustment event "Add fades in and out. Change the opacity" — the same code as for an ordinary event; wet/dry comes by itself, because background = input |
| After Effects | 26.x | layer Opacity (plus masks) and Effect Opacity in Compositing Options | TWO floors: composite (layer alpha) and per-effect; Adobe writes outright that Effect Opacity "provides the same functionality as the Blend With Original control" of older versions |
| Adobe Premiere | 26.x | layer Opacity in Effect Controls | ONLY the COMPOSITE; effects have no wet/dry of their own, and the standard forum answer to "I want to weaken the effect" is "put it on an adjustment layer and turn the layer's Opacity" |
| DaVinci Resolve | 20.3.2 | clip Opacity · Blend of each ResolveFX · Key Output Gain of the node | ALL THREE floors at once: composite (broken on an empty adjustment clip — F-MPH-003), per-effect ("every single Resolve FX filter has a Blend parameter that mixes that filter's effect against the original image") and the level of the whole node |
| Final Cut Pro | 12.3 | Compositing → Opacity of the clip · Amount and Mix of the effect | composite plus per-effect, and Apple distinguishes them IN WORDS: Amount — "sets the intensity of the effect", Mix — "sets how much of the original image is blended with the filtered image" |
| Kinotrain | — | none | none of the three for video; across the whole product there is exactly one wet/dry — `SlowedReverb.mix` ("Share of the wet signal 0..1"), and that one is audio |

## The trick behind it

The user calls "a transparency slider" three DIFFERENT mechanisms, and the difference between them lies not in the quality of implementation but in the picture.

**1. Composite transparency works as wet/dry by coincidence, not by design.** In Vegas, Premiere and FCP the "strength of the effect" is literally the same code that makes the clip semi-transparent; it becomes a mix only because UNDER the adjustment layer lies exactly the frame that the layer consumed. Indirect proof — how people use it: to make the EFFECT fade while the picture stays, forums advise not to turn the effect's keys, but to move the effect onto an adjustment layer and turn the layer's opacity. We have no such background by construction: FX are computed per layer in `apply_video_chain_pooled` BEFORE the composite, and under the layer lies the neighbouring track or a black canvas (`out.resize(w * h * 4, 0)` in `execute_plan_cpu`). Our `ClipEvent.opacity` on a clip with an effect gives not "the effect weaker" but "the clip paler": it leads to the LOWER TRACK, not to the unprocessed frame.

**2. A cheap slider inside the chain and a true composite give different pictures, and they diverge by CLASS of effect.** One can mix the input and output of the chain directly in `apply_video_chain_pooled`, but:

- on a SINGLE opaque layer and a per-pixel effect both methods give byte-for-byte the same result — both the mix and `alpha_over` are linear and commute. Hence the false impression that "there is no difference";
- as soon as the effect touches the ALPHA, the composite stops being linear in the content of the layer: `alpha_over` is `a·rgb + (1−a)·bg`, a product of the MIXED alpha and the MIXED colour. `greenscreen` sets `px[3] = 0`; mixing RGBA before the composite means getting translucent green, mixing after means "half a frame of hole with the lower track plus half a frame of green". The divergence is quadratic in alpha and is greatest where `softness` gives a soft edge;
- the "dry" side of the mix is DIFFERENT pictures in the two methods. In the cheap one it is the layer's own frame, in the true one it is the whole finished frame. On an overlap (PiP, a translucent top layer) and especially on a transition, where the morpher covers BOTH halves of the join (`PlanLayer::Dissolve`), 50% leads to different places: the cheap one mixes `FX(A)` with `A` and `FX(B)` with `B`, and then dissolves; the true one would dissolve first and apply the effect once per frame.

And the place of the mix for us is not at the edge of the chain: `MorphPatch::apply_to` does `c.fx.extend(add)` — the layer's effects are APPENDED to the clip's chain. So the "strength of the layer" is a mix not of "input ↔ output of the chain" but of "the frame after its OWN clip chain ↔ the output of the whole", that is, the point of saving the intermediate frame stands in the MIDDLE of the list, and both executors must know about it.

**3. For effects that change geometry, the overlay slider is not "half of the movement" but a double exposure.** `Pan`, `Pan3d` and `EventGrid` move pixels; 50% gives a translucent shifted frame OVER the unshifted one, not half of a push-in. This is equally true for us, for Vegas and for AE — the trap of expectation is built into the mechanism itself, while the real "strength" of such an effect lives in its own parameters (`zoom`, `pos_z`). The other side: `Mirror` has no numeric parameters at all (`horizontal`/`vertical` are booleans), and `Edges` has a grey output and "less Sobel" is not expressed by any field — for them the overlay slider is the only possible gradation, even if as a ghost.

## Pros

- A mix inside the layer does not introduce a new composite stage: the frame plan stays the same, only the CONTENT of the layer changes. Invariant 7 is not revised — unlike a true composite wet/dry (F-MPH-005).
- The slider appears on all ten video effects at once, including those that have no gradation of their own at all (`Mirror` — two booleans, `Edges` — grey output): the strength is given by the overlay, not by a plugin parameter.
- Our morpher addresses CLIPS (`MorphTargets::Clips/Marks/Areas`), not "everything below", so the strength can differ between the targets of one layer. In Vegas the opacity is one for the whole event — a consequence of the fact that there the layer is one object, while for us the patch is per target.
- A property that the cheap method PRESERVES and that a composite would lose: keying from the morpher reveals the NEXT layer for us, not the same picture (F-MPH-004, `greenscreen` → `alpha_over`). A mix inside the layer does not touch this.
- Mixing two frames with an integer weight is already written by us and already mirrored on the GPU (`dissolve_into`/`scale_alpha_into` ↔ `composite.wgsl`), so the mixing maths is not invented but reused.
- The audio side gets the same thing with one parameter: today wet/dry exists for exactly one effect (`SlowedReverb.mix`), and "halfway" for `Gain` can only be done by recalculating decibels by hand.

## Cons

- The cheap slider and the composite give a DIFFERENT picture (section above), so it cannot be named "transparency of the overlay, as in Vegas": on overlaps and transitions it behaves differently. Either this is explained in a hint, or one day the behaviour of already finished projects will have to change.
- The mix requires a SECOND frame per layer: the intermediate frame must survive to the end of the chain. An `FxScratch` pool exists, but this is +W·H·4 bytes per layer on both backends, and on the GPU an extra target texture in the pool.
- The post-FX cache key is computed by `fx_chain_hash` from the Debug representation of the chain plus geometry; the layer's strength is not part of the chain, so without extending the key 40% and 100% would return ONE frame. The same is true on the GPU — `plan_hash` mixes in `s.fx_hash`. This is invariant 4, not a detail.
- The slider means different things by eye for different effects: for blur it is "weaker", for pan — a ghost. One widget, two meanings — the very thing we criticize other editors for.
- An audio chain with `SlowedReverb` cannot be mixed with its own input in principle: it warps time, and the input and output have different lengths (`chain_avg_rate`, `chain_source_window`). The morpher already knows this and does not cut such clips — so the audio side of the slider will have an exception, and it will have to be shown as an error (UI rule 16), not silently skipped.
- The default must be an identity BYTE-FOR-BYTE: `src/libs/kt-fx/CLAUDE.md` requires an explicit branch "default → verbatim the previous code, including AVX2". Otherwise GOLDEN_VIDEO, 26 goldens of t26 and t67 would be regenerated — and they are the definition of determinism (invariant 11).
- `Pan3d` is not computed on the CPU at all (catch-all `_ => frame`), so on the CPU fallback there is nothing to mix: the behaviour of the strength on the two backends will have to be described explicitly, not left "as it turns out".

## What users say

The primary feedback is the words of the project owner, with which R.36 began:

> The morpher in Vegas lets you control the transparency of the overlay
> of effects with a slider, and we don't.
> — `docs/user-feedback.md`, R.36, 2026-09-07

Another user formulates the same need without knowing the word "wet/dry":

> Having trouble with making a video effect fade out and not the video
> itself...I have the starlight effect on the first clip and I want the
> effect to fade itself after about 2 seconds leaving the remaining video
> as normal.
> — Bruce Reilly, Creative COW, 27.03.2014,
> https://creativecow.net/forums/thread/changing-opacity-of-a-video-effect-not-the-video-i/

The answer in the same thread is the whole of mechanism No. 1, given as a trick:

> apply the Starlight to an Adjustment Layer, then adjust the opacity of
> the Adj Layer.
> — Richard Herd, there, 27.03.2014

And here is the same mechanics seen from the other side — when the background is not the layer's own input but the lower clips:

> It's just the way Adjustment Layers work. If you're removing opacity
> from the AL, then you're left showing what's underneath.
> — excited_Genie16B8, Adobe Community, 13.03.2018,
> https://community.adobe.com/t5/premiere-pro-discussions/can-t-make-any-opacity-in-adjustment-layer/td-p/9798312

## How it fits Kinotrain

### What already exists

- Composite transparency (mechanism No. 1) — but on the CLIP, not on the layer: `ClipEvent.opacity` in `src/libs/kt-model/src/project.rs` (~line 1512, the "level" of the event as in Vegas, a line along the top of the clip, t176) → `opacity_k` in `src/libs/kt-engine/src/video.rs` (~line 201) → `alpha_k` of `PlanLayer::One`/`Dissolve` in `src/libs/kt-engine/src/video_plan.rs` → the mirror in `src/libs/kt-gpu/src/shaders/composite.wgsl`. A wet/dry cannot be obtained from it: the background under the layer is the lower track or a black canvas, not the layer's own unprocessed frame.
- Wet/dry inside an effect (mechanism No. 2) — exactly ONE across the whole product: `FxInstance::SlowedReverb { mix }` in `src/libs/kt-model/src/project.rs` (~line 1889), automatable (`mix_points` in `src/libs/kt-fx/src/lib.rs`). Video effects have none: a grep for `wet|dry|blend|mix` in `src/libs/kt-fx/src/video.rs` is empty.
- The strength of the whole chain (mechanism No. 3) — exists nowhere: `MorpherEvent` and `MorphPatch` (`src/libs/kt-model/src/project.rs`, ~lines 569 and 677) have no fields `opacity`/`amount`/`mix`/`strength`, only `enabled`, `src_off` and the bitwise `video_bypass`/`audio_bypass`.
- Execution of the chain: `apply_video_chain_pooled` in `src/libs/kt-fx/src/video.rs` (line 61) — per layer, before the composite; the GPU mirror is `chain_ops` in `src/libs/kt-gpu/src/engine.rs` (~line 122). The morpher's chain is appended to the clip's chain by `c.fx.extend(add)` (`MorphPatch::apply_to`, `src/libs/kt-model/src/project.rs`, ~line 858).
- Ready maths for mixing two frames: `dissolve_into` and `scale_alpha_into` in `src/libs/kt-engine/src/video.rs` (~lines 726 and 700) — integer, with a mirror in `composite.wgsl`.
- The morpher chain strip with the master switch — `src/apps/kt-app/src/morpher_editor.rs` (~line 375, `chain_strip`, `auto: None`); the curve widget — `src/apps/kt-app/src/automation.rs`.

### What is missing

A slider as a model field, as a multiplier in both executors, as part of the cache key, as a format field and as a widget. The overlay is binary: the only gradation today is switching the slot off.

### What stands in the way

- **The mix point is the middle of the chain.** Because of `c.fx.extend(add)`, the layer's strength must mix not the input with the output, but the frame AT THE BOUNDARY between its own part and the morpher part. So `apply_video_chain_pooled` receives both the strength and the index of the start of the morpher part, and holds a copy of the frame at that point; the same goes for `chain_ops` and for the WGSL pass.
- **The cache key.** `fx_chain_hash` (`src/libs/kt-engine/src/video.rs`, line 103) hashes the Debug of the chain and the geometry; the strength does not get into it. The same defect is in `plan_hash` in `src/libs/kt-gpu/src/engine.rs` and in `BurstKind::PostFxVideo { quality, fx_hash }` in `src/libs/kt-cache/src/lib.rs`.
- **Byte-for-byte default.** The rule of `src/libs/kt-fx/CLAUDE.md`: a new parameter must have an explicit branch "as it was", otherwise the goldens move.
- **Audio exception.** A time-warping chain (`FxInstance::warps_time`) changes length: there is no mix of input with output. The morpher already refuses to cut such clips (`src/libs/kt-model/src/project.rs`, ~line 954) — the refusal to mix goes in the same place.
- **Format.** A new field → `KT_META_VERSION` in `src/libs/kt-otio/src/meta.rs` (now 4) is raised to 5: an old reader would silently lose the strength.
- **Place in the UI.** The morph row is three times lower than the clip rows (`THIN_ROW_DIV` in `src/apps/kt-app/src/timeline.rs`, line 537); the level line and the corner handle physically cannot be placed there in the same form as on a clip.

### Which invariants it touches

- **No. 7 "one composition code" — the central one, and it is exactly what makes this feature cheap.** The invariant enumerates by name: layers, the Vegas order, the weights of transitions/fades, LEVELS and the mapping "tick → source frame" are computed by one code (`VideoRenderer::frame_plan`), and the backend is a parameter of execution, not a second path. The overlay strength is a weight, that is, exactly the entity No. 7 forbids computing "in the shader" or "in export": it must be born in the model and the plan and be executed both by the CPU reference and by the GPU, otherwise preview would diverge from export. Moreover, the cheap variant stays INSIDE the existing stage (it changes the content of the layer), while the composite would open a NEW stage over the finished frame — and that is exactly the boundary between "M" and "a separate round" (F-MPH-005).
- **No. 4** — the strength enters the post-FX cache key, otherwise the warmth bars and the frame itself would lie.
- **No. 11** — CPU byte-for-byte against GOLDEN_VIDEO at strength 1.0; GPU against CPU only through `kt_gpu::assert_frames_close`.
- **No. 3** — the slider is dragged with the mouse during live playback: this is `set_params` with smoothing and reuse of state, not a rebuild of the graph.
- **No. 2** — the audio side of the strength is computed only in the audio-render thread.
- UI rules of `src/apps/kt-app/CLAUDE.md`: **13** (the slider takes the already occupied place of the chain strip, not a new row), **17** (the strength from the window and the strength by gesture on the block behave alike), **8** (not "an egui slider in the middle of the branded strip"), **20** (for old files the strength reads 1.0, rather than being "filled in by meaning").

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages. The earlier "S" estimates in F-MPH-002/003 are understated: the mix point in the MIDDLE of the chain drags along the cache key, the WGSL mirror and the format version |
| Where the main work is | kt-fx (`video.rs`: mix at the chain boundary + identity branch), kt-gpu (`chain_ops` + a new pass), kt-engine (`fx_chain_hash`, plan), kt-model (field, undo, strength automation), kt-otio (v5), kt-app (widget in the chain strip and the gesture on the block) |
| Project format | v5 needed (`KT_META_VERSION` 4 → 5); the default 1.0 is not serialized — old files stay byte-stable |
| Tests that will have to be written | round-trip of the model and reading of files without the field; "strength 1.0 == previous bytes" on GOLDEN_VIDEO, t26 and t67; "strength 0.0 == no morpher chain"; CPU↔GPU conformance on the pattern of t181 via `kt_gpu::assert_frames_close`; distinguishing strengths in the post-FX cache key on the pattern of t66; regression "keying from the morpher reveals the lower layer" (F-MPH-004); refusal of the mix on a time-warping audio chain; t36 (morpher == bake) at strength < 1; a harness test of the widget in `layout_tests.rs` |
| Depends on | nothing — for the variant "mix inside the layer". The true composite wet/dry depends on the post-composite stage (F-MPH-005) |
| Breaks compatibility | no |

## Verdict

We take it — but we take mechanisms No. 2–3, not No. 1, and we name it honestly. The mix at the chain boundary inside the layer closes the user's request (item 245), costs M and does not open a new composite stage, that is, does not revise invariant 7; the nearest outside analogue is not Vegas opacity, but Resolve's Key Output Gain and After Effects' Effect Opacity.

The composite wet/dry (a mix with the FINISHED frame) remains a hole and lives in F-MPH-005: without it, on overlaps, on transitions and on effects that change alpha or geometry, the picture at 40% will be DIFFERENT from Vegas's. This has to be written down now, while the decision is being made, and not worked out a year from now from the complaint "theirs is smooth, ours is not".

## Sources

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/adjustmentevents.htm — VEGAS Pro 2026: "Add fades in and out. Change the opacity", up to 32 effects, restrictions (reverse/Velocity) (accessed 2026-09-07)
- http://vegaspro.helpmax.net/en/using-vegas-software/using-markers-regions-and-commands/adjusting-video-track-controls/ — the track Level and the composite-level envelope as the "strength" over everything the track covers (2026-09-07)
- https://blogs.adobe.com/creativecloud/compositing-options-effect-masks/ — After Effects: Effect Opacity in Compositing Options "provides the same functionality as the Blend With Original control" (2026-09-07)
- https://helpx.adobe.com/after-effects/using/mask-reference.html — Compositing Options and Mask Reference of each effect (per the documentation; 2026-09-07)
- https://ltbits.github.io/davinci-resolve-manuals/DR17/DR17-RM-12%E2%80%93ResolveFXOverview.pdf — Resolve FX Overview: "every single Resolve FX filter has a Blend parameter that mixes that filter's effect against the original image" (a mirror of the manual; the wording is preserved in 18.6 and 20.x; 2026-09-07)
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf — Key Output Gain as the "contribution strength of the node", grade levels (2026-09-07)
- https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part2995.htm — "Adjusting Layer Node Strength Using Key Output Gain" (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/adjust-effects-ver4e3296f/mac — FCP: "Amount: Sets the intensity of the effect", "Mix: Sets how much of the original image is blended with the filtered image" (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/change-a-clips-opacity-ver52cf3376/mac — FCP: Compositing → Opacity in the Video inspector (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/add-adjustment-clips-verfda63436e/mac — FCP: "Effects applied to an adjustment clip are applied to all clips below it in the timeline" (2026-09-07)
- https://creativecow.net/forums/thread/changing-opacity-of-a-video-effect-not-the-video-i/ — the fading of the EFFECT, not the video, and the standard answer "put it on an adjustment layer" (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/can-t-make-any-opacity-in-adjustment-layer/td-p/9798312 — the mechanism of composite layer transparency in users' own words (2026-09-07)
- https://lutify.me/docs/how-can-i-change-the-strength-opacity-of-the-3d-luts-in-adobe-premiere-pro/ — Premiere: the strength of a LUT = the Opacity of the adjustment layer or the Intensity of the effect itself (2026-09-07)
- `docs/user-feedback.md`, R.36 (the project owner's primary feedback) and
  item 245
- Code (read, not retold by analogy):
  `src/libs/kt-model/src/project.rs`, `src/libs/kt-fx/src/video.rs`,
  `src/libs/kt-fx/src/lib.rs`, `src/libs/kt-engine/src/video.rs`,
  `src/libs/kt-engine/src/video_plan.rs`, `src/libs/kt-gpu/src/engine.rs`,
  `src/libs/kt-gpu/src/shaders/composite.wgsl`,
  `src/libs/kt-cache/src/lib.rs`, `src/apps/kt-app/src/morpher_editor.rs`

Not one of the five editors was run live: the mechanics are assembled from official help (VEGAS 2026, Adobe, Apple, Blackmagic) and forum threads. The statements "we have / we don't have" are verified by grep over `src/`.
