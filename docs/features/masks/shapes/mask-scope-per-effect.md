---
id: F-MSK-002
title: Where a mask lives — on the layer, on the effect or as a separate object
status: candidate
priority: B
cost: L
category: masks/shapes
editors: [premiere-26, after-effects-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: no
crates: [kt-model, kt-fx, kt-engine, kt-gpu, kt-cache, kt-otio, kt-app]
invariants: [3, 4, 7, 11]
ui_rules: [1, 8, 13, 17, 20]
round: null
point: null
tests: []
depends_on: [F-MSK-001, F-CLR-006, F-MPH-006]
checked: 2026-09-07
---

# F-MSK-002 — Where a mask lives — on the layer, on the effect or as a separate object

## Essence

The editor's task can be stated in one line: on ONE clip, blur a face and raise the brightness in a corner of the frame. From there, everything is decided by whose property the contour is. If the mask belongs to the layer — it cuts the whole layer, and for two different regions two layers are needed. If the mask belongs to the effect — both actions live as slots of one chain, but a shared contour cannot be handed to two effects. If the mask is a separate signal with its own input, the contour is reused and inverted by the consumer, but the effect gets a second input, and the program gets a connections editor. This is not "where the button is": the fork determines the data scheme, the cache key, the copy-paste of a slot and the project format, and it can be replayed only by rewriting everything listed above.

The neighboring note F-MSK-001 answers the question "what a contour is" (we take scalar primitives, not arbitrary Bezier) and explicitly defers the question "whose it is" to here. This note closes it.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Adobe Premiere | 26.x | masks in Effect Controls, INSIDE the effect | the contour is a property of a SPECIFIC effect, "You can only apply one mask to one effect"; a mask drawn without a selected target goes into the Unassigned masks section and is dragged with the mouse onto the needed effect or onto Opacity (Opacity = the layer's mask); the mask has an Inverted checkbox, feathering, tracking |
| After Effects | 26.x | LAYER masks + Compositing Options → Mask Reference on the effect | a HYBRID: the contour lives on the layer and by default cuts the whole layer (its alpha), but the effect has its own Compositing Options group with Effect Opacity and a Mask Reference list — the effect REFERENCES the layer's mask without owning it; separately, Set Matte takes the channel of another layer (per users' descriptions — either before or after that layer's own effects) |
| DaVinci Resolve | 21 | Power Windows on the NODE (Color) + mask nodes in Fusion | "all of the windows within a particular node work together to limit that node’s grade"; inside a node up to five windows, each with a Mask checkbox = add or subtract; the inverse region is not a second mask but an Outside node ("Corrector nodes with the Key Palette’s Key Input Invert control enabled"); in Fusion a mask is a SEPARATE node into the blue Effect Mask input, plus separate pre-mask, Garbage and Solid Matte inputs |
| Final Cut Pro | 12.3 | seven mask-effects in the stack + "effect masking" inside effects | ALSO a hybrid, but a different one: Draw / Shape / Graduated / Image / Vignette / Magnetic / Scene Removal are ordinary clip effects ("Add them to any clip in the timeline just as you would add any other clip effect"), and such a mask cuts the WHOLE layer; one correction can be limited only by the built-in mask of the effect itself — "you can isolate and control which areas of the video image are affected by clip effects" — and the help says directly about the boundary of the region: "mask blend modes interact within a single effect—not across multiple effects" |
| VEGAS Pro | 2026 | Bézier Masking OFX in the plugin chain (plus Cookie Cutter, Mask Generator, a mask in Pan/Crop) | a POSITIONAL model: the mask plugin stands in the event's chain; by default it cuts a hole in the frame, and the Mask FX checkbox turns it into a limiter — "to mask the effects of all plug-in(s) in the effect chain that precede the Bézier mask plug-in"; up to five masks per instance, Invert and Blend (strength) are shared per instance |

A note on versions: the online VEGAS help is available up to and including the 23 line, so the VEGAS row is checked against the VEGAS Pro 23 help — the predecessor of the 2026 line (see `_editors.md`); no differences in the design of Bézier Masking are declared in the 2026 release notes, but live it is **[NOT VERIFIED]**.

## The trick behind it

**1. A mask on an effect works AFTER the effect, not before it — and therefore not everything can be masked.** The Fusion manual says directly: "While masks (or mattes) are connected via an input, they are actually applied “post effect,” which means the node first applies its effect to the entire image, and then the mask is used to limit the result by copying over unaffected image data from the input" (Resolve 21 Reference Manual, ch. 79 "Rotoscoping with Masks"). Two things follow from this that appear in no list of features. The first: there are effects to which such a mask does NOT apply, and the manual enumerates them by name — "notably Savers, Time nodes, and Resize, Scale, and Crop nodes", that is, exactly those whose input and output lie in different rasters, so there is nothing to "copy over the unaffected part of the input" with. The second: a glow and a flare in Fusion have a SECOND, pre-mask input — "a pre-mask restricts the effect to certain areas of the image but allows the result of that effect to extend beyond the limits of the mask", — and it exists precisely because the post-effect mask behaves incorrectly. The same physics is described in their own words by a Premiere user: "The problem with doing things like adding a glow to a masked clip is that the glow is masked as well, so it doesn’t “glow” beyond the bounds of the mask". A mask on an effect is not "the effect was computed only here", but "the effect was computed everywhere, and shown only here": the cost of the frame does not fall, while the glow and the blur are cut along the contour.

**2. For a positional mask (VEGAS, FCP) the region is a PREFIX of the chain, and the second mask cuts the first.** The VEGAS help marks this in a separate footnote: "Note that the Bézier Masking OFX masks all of the preceding effects. This applies also other instances of the Bézier Mask, in other words, if you have more than one mask, everything outside the latest mask is not affected at all". An analysis of their same example: "wrong color" → mask 1 → blur → mask 2 results in the inside of the second mask being both blurred AND colored, while OUTSIDE the second mask the coloring is cancelled too, including the coloring limited by the first. That is, two masks give not two independent regions but their intersection, and "to limit the second of five effects" cannot be formulated by position at all. FCP has the same trap from the other side: there the mask-effect cuts the layer, not the neighboring effect, and a user describes this literally — "Draw mask affects the whole layer, not the colour board". The model "the mask belongs to its owner" (Premiere: the effect; Resolve: the node; FCP — but only for the effect's built-in masks) is free of this: five effects — five independent regions, and the question "what exactly am I masking" has an answer by construction, not by the order of slots.

A positional model, however, can be made precise — and one can see at what cost. In Shotcut and Kdenlive (outside the five, but here they are illustrative) the region of a mask is bounded from TWO sides: a mask filter is placed, then the effects, then an EXPLICIT terminator `Mask: Apply`, which "combines every thing in between", and the documentation warns at once: "So, the order of the filters is very important". So the exact region is paid for by two service slots per mask and by the order as a load-bearing structure; in addition, in such a scheme an effect periodically surfaces that simply ignores the mask, and the user cannot tell whose defect it is: "some effects, such as the NTSC one from frei0r, applies the effect to the entire canvas rather than to the rotoscoped area I designated".
A separate showcase of the same fork — FCP 12.3: there ONE AND THE SAME tool lives in two homes at once, "click the new Auto Mask button that’s visible in every effect header" and "add the Auto Mask effect (in the Masks and Keying category) directly to a clip" — that is, even the author of the hybrid could not choose one model and shipped both.

**3. Inversion of the contour and inversion of the CONSUMER are different mechanisms, and this is visible only on the second region.** The checkbox lives on the mask: Premiere — Inverted ("if you want the mask to apply to the background (checked) or foreground (unchecked)"), FCP — "A checkbox that, when selected, reverses the mask—swapping its solid and transparent areas", VEGAS — Invert in General Options, that is, at once for all five masks of the instance, Resolve — the Mask checkbox of each window inside the node ("whether one window adds to another window, or subtracts from that window"). With such an inversion one cannot obtain a SECOND correction over the inverse region without creating a second contour: you copy the window — and from then on two contours live their own lives and diverge from the first edit. Resolve solves this by inverting not the mask but the INCOMING KEY of the neighbor: "Outside nodes are really just Corrector nodes with the Key Palette’s Key Input Invert control enabled" (ch. 146), and if the checkbox is unchecked, a second correction of THE SAME region results. In AE the same result is achieved by a reference rather than a wire: Mask Reference takes the layer's mask, so two effects can be given one contour and it can be edited in one place. Premiere has nothing of the kind — "You can only apply one mask to one effect", and a shared contour exists only as a copy; in Resolve on the Color page reuse formally exists, but it is "Copy Window / Paste Window" into another node, that is, again a copy.

## Pros

Here and below — consequences of the DESIGN of each of the models, not estimates.

- **Mask on an effect:** "blur the face" and "raise the brightness in the corner" are two slots of ONE clip. No clip duplicate, no adjustment layer, no second decode of the source; AE users describe exactly this transition — "Usually I’d just apply the effect on the entire layer, then duplicate that layer and mask it".
- **Mask on an effect:** a neighboring effect in the chain does not see the mask, so editing the contour cannot spoil someone else's setting — the region and its consumer are tied rigidly.
- **Mask on a layer:** the contour is the layer's alpha, and so one mechanism gives transparency, a soft edge and participation in the composite at once; a per-effect mask does not touch the layer's alpha and does not get into the composite.
- **Mask on a layer:** one place of editing for the whole clip — five effects are guaranteed to sit in one contour and cannot drift apart.
- **Mask as a separate object (node/reference):** the contour feeds as many consumers as you like, an edit propagates to all at once, and "inside/outside" is the state of one signal (Outside), not two objects that the editor has to keep in sync by hand.
- **Positional mask (a mask-effect in the chain):** zero new concepts in the data scheme. The mask is an ordinary slot, so it gets into the copy-paste of the chain, the bypass checkbox, the presets and the project format for free.

## Cons

- **Mask on an effect:** one contour cannot be handed to two effects. Premiere says this in plain text, Resolve offers copying the window into another node — and from the first edit the copies diverge; synchronization becomes the person's duty.
- **Mask on an effect:** it is not visible. It is hidden inside a collapsed slot, and the price of this invisibility is measured in years: the mechanism in AE has existed since 2014, and people were building workaround duplicate layers up to 2026 (quotes below).
- **Mask on an effect:** not applicable to all effects — those that change the raster (scale, crop, resample) fall out of the model, because there is nowhere to take the "unaffected" part from.
- **Mask on a layer:** to limit ONE effect, a second object is needed — an adjustment layer, a precomp or a clip duplicate. This is an extra element in the stack, a second render pass and an extra undo step; in FCP the same need is solved by duplicating the clip even for combining two masks (there is no Add mode for mask-effects: "Dragging multiple instances of the Draw Mask or the Shape Mask to the same clip creates the equivalent of the Intersect blend mode").
- **Positional mask:** the order of slots starts to mean two things at once — what is computed after what AND what is limited by what; the second mask intersects with the first cumulatively (the VEGAS footnote above), and the picture cannot be predicted from a screenshot of the chain.
- **Separate node-object mask:** the effect gets a second input, the program gets a canvas of connections. The price was analyzed in F-CLR-006, and there it was acknowledged as unaffordable for us.
- **Common to all models:** a mask is an animatable GEOMETRY, not a number. Keys on the points of a contour do not fit into any common curve editor, and each editor creates a separate mode for them: VEGAS has its own "Animate Curve 1" with an Edit Mode, Premiere has a separate Clip/Frame switch, and users consider this a regression: "now the controls and tools are scattered in to tree different places".

## What users say

The question itself in pure form — and the answer that was correct until 2014:

> Is there a way to apply a mask to a single effect on a video track? […] Can I
> do this without having to duplicate the layer?
> — Murry Lightning, Creative COW, 02.08.2010, https://creativecow.net/forums/thread/masking-out-single-effect-on-layer/

> Not quite- you could ad an adjustment layer and apply the effect to it and
> then a mask, but any other layer that would be underneath would be affected as
> well.
> — Tudor «ted» jelescu, there, 02.08.2010, https://creativecow.net/forums/thread/masking-out-single-effect-on-layer/

(In square brackets, the phrase "Like when you apply a mask to a filter or color node in shake" is omitted — a reference to Shake, not relevant here.)

About the invisibility of the per-effect mask (AE, the mechanism appeared in 2014):

> I have been using AE since 2003. I just learned about Compositing Options in
> the year of our lord 2026.
> — u/newaccount47, r/AfterEffects, 28.06.2026, https://www.reddit.com/r/AfterEffects/comments/1k6zqqf/did_you_know_this_pro_tip/oua43gb/

About what it replaces:

> Usually I'd just apply the effect on the entire layer, then duplicate that
> layer and mask it.
> — u/MX010, r/AfterEffects, 18.06.2026, https://www.reddit.com/r/AfterEffects/comments/1u8yfk4/after_20_years_i_finally_figured_out/ose8yp0/

About two floors of strength (the strength of the effect versus the transparency of the layer — a direct link with F-MPH-006):

> The “Opacity” controller on the Fill effect itself actually makes the layer
> itself transparent, which isn’t really that helpful. But if you use the
> Opacity controller in the Compositing options it changes the opacity of just
> the Fill.
> — u/Stinky_Fartface, r/AfterEffects, 18.06.2026, https://www.reddit.com/r/AfterEffects/comments/1u8yfk4/after_20_years_i_finally_figured_out/osczsom/

How the per-effect mask looks in work in Premiere 26 (the mask first belongs to no one, then it is ASSIGNED to its owner):

> assuming you're using a recentish version, use one of the mask tools, then
> drag that mask from "unassigned masks" down onto your blur effect
> — u/reginaltus, r/premiere, 24.07.2026, https://www.reddit.com/r/premiere/comments/1v5ecar/need_help_with_applying_gaussian_blur/ozj86ju/

And a negative opinion about the same thing — assigning the owner by a separate gesture irritates:

> Seems convoluted but maybe there's a deeper reason they did it that way. I
> still default to After Effects for real masking.
> — u/DaleFairdale, r/premiere, 18.08.2026, https://www.reddit.com/r/premiere/comments/1vrodzr/do_you_prefer_the_new_masking_method_in_premiere/p4fjb0p/

And about the seams around the mechanism — editing the contour is spread across three places:

> So basically when previously all the necessary controls for masks were
> underneath *Effect Control* window, now the controls and tools are scattered
> in to tree different places (*Effect Controls, Toolbar and Video
> Preview/Program window.*)
> — u/Bluefi1, r/premiere, 24.06.2026, https://www.reddit.com/r/premiere/comments/1tcbgus/the_object_mask_tool_is/otkn70p/

About "the mask cuts the whole layer, not my effect" — FCP, where the mask-effect stands in the stack next to a correction and still does not limit it:

> Draw mask affects the whole layer, not the colour board. You could make it
> work though by duplicating the layer, make the colour board on the lower layer
> and draw mask on the upper layer.
> — u/Silver_Mention_3958, r/finalcutpro, 09.04.2026, https://www.reddit.com/r/finalcutpro/comments/1sgzj4a/draw_mask/of8y3mc/

About post-effect application of a mask, as seen from the editor's side:

> The problem with doing things like adding a glow to a masked clip is that the
> glow is masked as well, so it doesn't "glow" beyond the bounds of the mask.
> — u/mcarterphoto, r/premiere, 15.08.2026, https://www.reddit.com/r/premiere/comments/1vpcel9/how_to_apply_effects_to_a_masked_object_without_a/p3x32jn/

About the inverse region done by hand — exactly what Resolve does with one checkbox on someone else's key, here is done by a copy of the contour with the Inverted checkbox unset:

> Copy the object mask of the person and remove inverted checkbox.
> — u/NLE_Ninja85, r/premiere, 19.04.2026, https://www.reddit.com/r/premiere/comments/1spzhis/how_can_i_mimic_this_effect_in_premiere/oh44na6/

And the most expensive kind of feedback — "stayed on the old version because of this": a person brought a contour from an external tracker (Mocha Pro) and inserted it into a clip in order to hang an effect on it, and in version 26 this stopped working:

> I have both 2025 and 2026 installed, but use 2025 for this reason.
> — u/pmarval, r/premiere, 16.04.2026, https://www.reddit.com/r/premiere/comments/1r89e87/premiere_pro_2026_hate_it_or_love_it/ogiox3f/

The third request from the list — "share one contour between two effects" — sounds the same for everyone and likewise has no built-in answer:

> Is it possible to copy / transfer a Pixel Chooser Key matte from one effect to
> another?
> — Petergot, Boris FX forum, 18.05.2022, https://forum.borisfx.com/t/14040

That a copy of the contour is NOT a shared contour (FCP, but the diagnosis is general):

> With Draw Mask, the animation is locked to that specific mask instance, so if
> you paste keyframes onto a different mask layer even if it looks identical it
> won’t behave the way you expect.
> — u/StupidRaisins, r/finalcutpro, 24.02.2026, https://www.reddit.com/r/finalcutpro/comments/1rd3n79/keyframe_copy_and_paste_not_working_on_mask/o76n9sw/

This should be read as follows. There is not a single complaint of the kind "a per-effect mask is a bad idea"; people complain about two things. The first — that it is NOT FOUND: in AE the mechanism has existed since 2014, in Premiere and Resolve for as long as anyone remembers, and the threads of 2026 are titled "after 20 years i finally figured out". The second — the seams around it: assigning the owner by a separate gesture, copies of the contour drifting apart, and editing modes spread across three panels. So what is expensive is not the design but its presentation.

## How it fits Kinotrain

### What already exists

- **The clip's effect chain** — `ClipEvent.fx: Vec<FxInstance>` (`src/libs/kt-model/src/project.rs:1522`) plus the parallel `fx_bypass`, `fx_auto` and the master switch `fx_off`; the engine receives it via `active_fx()` (same file, 1569). A morpher layer appends its slots to THE SAME chain, and `Project::effective_clips()` (`project.rs:3013`) is the single point of delivering clips to the engine, export and drawing.
- **Two byte-matching chain executors**: `apply_video_chain_pooled` (`src/libs/kt-fx/src/video.rs:61`) — a `for f in fx` loop over one live frame, and its mirror `chain_ops` (`src/libs/kt-gpu/src/engine.rs:124`), which builds a `Vec<FxOp>`.
- **Alpha as a channel already exists**: `greenscreen` (`src/libs/kt-fx/src/video.rs:348`) writes it into `px[3]`, after which it goes into the composite's `alpha_over` (`src/libs/kt-engine/src/video.rs`). The key physically exists, but it has neither a name nor an address (diagnosis of F-CLR-006).
- **A non-numeric reference field in an effect — there is a precedent**: `Pan3d.camera: Option<CameraId>` (`project.rs:2097`). Numeric parameters are drawn by a unified grid over `ParamSpec`, while the reference is drawn as a combo box in the non-numeric tail of `draw_video_fx_params` (`src/apps/kt-app/src/fx_editor.rs:557` and onward).
- **A reference by NAME with a surviving dead reference — also a precedent**: `MorphTargets::Areas` looks up `MarkedArea` (`project.rs:1163`) by name; a deleted area is not forgotten and is drawn in an error style (rule 20).
- **The parameter passport of a struct** — `params_for_struct!` (which describes `CameraParams`, `project.rs:2394`): a shape-contour with numeric fields will get both the common field grid and the common curve widget without a single new line in the UI.
- **Automation** — `ParamCurve` is attached only to `ParamKind::Smooth` (`project.rs:2150`, 2174), the key domain is the MATERIAL TICK, so trim/split/reverse do not touch the curves.
- **Cache** — `fx_chain_hash` (`src/libs/kt-engine/src/video.rs:153`) hashes the Debug form of the WHOLE chain plus the clip's geometry; the post-FX frame is memoized in `layer_frame_cpu` (same file, 721).
- **Copy-paste of a slot** — `ops::FxSlotBlock` (`src/libs/kt-model/src/ops.rs:851`) and `FxChainBlock`: JSON in the system clipboard, survives a restart and moves between windows (t205/t215/t217).
- **Format** — `FxInstance` travels as a serde envelope in the effect's `metadata.kt` (`src/libs/kt-otio/src/convert.rs:548`), and the envelope version is set BY CONTENT (`src/libs/kt-otio/src/meta.rs:17`, currently `KT_META_VERSION = 4`).

### What is missing

There are no masks at all: a grep for `mask|matte|rotoscop` over `src/` finds only `front_mask` inside `transitions.rs` and the timecode-input mask in `util.rs`. There is no contour geometry, no rasterizer, no notion of a "frame region" — and this should be emphasized: our `MarkedArea` is TIME × TRACKS, we do not yet have any spatial meaning for the word "area" at all, and the name is already taken (the namespace of areas and marks is shared). There is also no on-screen editing: today the preview supports exactly one gesture — a click for play/pause (`src/apps/kt-app/src/app.rs:3205`, t74); not a single gizmo over the frame exists.

### What gets in the way

1. **Different rasters live inside the chain.** `Pan`, `CamMotion`, `EventGrid` and `Pan3d` resample the frame to the project size (visible from `chain_ops`), while the other seven video FX preserve the size. A mask on a resampling slot is undefined: the "original" and the "result" lie in different coordinate systems — exactly the restriction that Fusion documents by name for Resize/Scale/Crop.
2. **The frame arrives at QUALITY resolution.** The decoder delivers a frame already at `Quality { width, height }`, and the same chain is computed on different rasters in the preview and in the export (`q_key` in `layer_frame_cpu`). So the coordinates of a contour MUST be normalized; "pixels" are not suitable here.
3. **`params_for!` knows only numbers.** `ParamSpec` is min/max/default/step/unit/kind, and a mask reference does not fit into it: it is drawn by hand in the tail of `draw_video_fx_params`, like `Pan3d.camera`. The numeric fields of the shape itself (center, size, rotation, feathering) fit into the macro perfectly — and automation comes along with them for free. The Bezier path does not fit: keys on points are not a `ParamCurve`, and it would become a SECOND exception after `CamMotion.keyframes`, which already has its own mini-editor in `fx_editor.rs`, in violation of the rule "curves are drawn by one widget".
4. **The post-FX cache key.** `fx_chain_hash` hashes the Debug form of the chain. It will catch an inline contour by itself; a REFERENCE appears in Debug, but the geometry does not, and editing a shared contour would not invalidate post-FX frames (invariant No. 4). This is cured by the morpher's technique: resolve the reference in `effective_clips()` BEFORE the plan is built (`VideoRenderer::new` already calls it first thing) — the engine will see the inline geometry, and the hash will be correct again without a single edit in the cache.
5. **Copy-paste between projects.** `FxSlotBlock` is a serializable block in the system clipboard; a reference by name in another project would be dangling. There is a precedent for the solution: `ClipBlock` carries `areas`/`markers` along, and `paste_block` counts `PasteSkipped` instead of "fixing up" the paste.
6. **Format.** A new field will travel into `metadata.kt` by itself, but an old build (v4) will silently ignore it and render the effect over the WHOLE FRAME — this is exactly the case of "silent corruption" for which the refusal `KtMetaTooNew` was introduced. So v5, by the same rule "version by content" by which v3 (key domain) and v4 (morpher layer) were raised.
7. **The rasterizer is written twice.** The mixer "original ↔ result by mask" and the shape itself are needed both in the scalar kernel with AVX2 and in WGSL, and SIMD must match the scalar code byte for byte. An analytic shape (ellipse/rectangle via signed distance with feathering) is suitable for this; a polygonal scanline fill with antialiasing is the main risk of CPU↔GPU divergence.

### Which invariants it touches

- **No. 7 (one composition code).** The mask must be part of the same frame plan, not a second path: the mixer lives in the chain and is repeated in both executors. It costs exactly twice as much as an ordinary effect, but in form it is the same `FxOp`, not a new interpreter (unlike the graph, F-CLR-006).
- **No. 11 (determinism) and the rule of the kt-fx crate.** The default "no mask" must give the previous bytes — then GOLDEN_VIDEO and the golden references of the video FX are not regenerated. The schema `#[serde(default, skip_serializing_if …)]` is already configured for this in our code.
- **No. 4 (the cache is the single source of truth about coverage).** The geometry must get into the post-FX key; on the other hand, a shared mask changed once invalidates the post-FX frames of all consumer clips — this is the correct behavior, but it is a black and must be waited out.
- **No. 3 (editing the model does not destroy FX state).** The mask field is meaningful only for video variants; its appearance on a slot must not count as a change of the chain for the audio state in the diff by clip id.
- **UI rules.** 13 — the mask does not get its own panel: the numbers go into the same grid of the effect card, the reference goes into its non-numeric tail, and the shape is edited by a gizmo over the ALREADY existing preview. 17 — "a mask-effect in the chain" and "a mask field on a slot" describe the same action, so EXACTLY ONE of the models goes into the product. 20 — a dead reference to a deleted contour does not delete itself (the precedent of morpher areas). 1 — the points of the contour must have a grab zone wider than the drawing, and the gizmo must not break the play/pause click over the preview (t74) and the double clicks (t206/t207). 8 — the gizmo is drawn in the frame, not as a "standard widget".

## Estimate

| What | Answer |
|---|---|
| Size | L — a whole round: a contour object in the model, the mixer and the rasterizer in TWO executors, a gizmo over the preview, a format version |
| Where the main work is | kt-model (mask object + reference on the slot), kt-fx (mixer + shape: scalar and AVX2), kt-gpu (WGSL mirror and a new `FxOp`), kt-app (gizmo in the preview, the tail of the effect card), kt-otio (v5), kt-engine (resolving the reference in `effective_clips`, cache key) |
| Project format | needs version v5: an old reader would silently render the effect over the whole frame — the standard refusal is more honest than silent corruption |
| Tests that will have to be written | round-trip of the model and copy-paste of a slot with a reference (in the spirit of t205/t215); "without a mask — the previous bytes" (golden_export t11/t67, video_fx); an edit of the contour invalidates the post-FX frame (in the spirit of t66/t66c); CPU↔GPU conformance of the rasterization ONLY via `kt_gpu::assert_frames_close` (t181-style); a dead reference resolves to nothing and is drawn as an error (in the spirit of t247/t248 of named areas); a layout test of the gizmo and "a click over the preview still plays" (t74/t76) |
| Depends on | F-CLR-006 (the graph is rejected — so a FIELD branches, not a wire), F-MPH-006 (overlay strength: the same mixer, one buffer) |
| Breaks compatibility | no: a file without masks is byte-for-byte the same; a file with masks will be honestly refused by an old build |

## Verdict

**We take model 2 by semantics and model 3 by storage: the scope of action belongs to a SLOT of the chain, and the geometry belongs to a separate named object of the project, which the slot references (plus an inversion checkbox on the reference).** This is the only combination that answers the original question "blur the face and raise the brightness in the corner on one clip" without creating a clip duplicate or a second morpher layer, and at the same time does not pay the main price of model 2 — contour copies that drift apart.

Why not the others. **Model 1 (mask on the layer)** for us would mean "the mask cuts the whole clip", and "limit one effect" would have to be done with a morpher layer or a duplicate of the event — that is, to reproduce exactly the complaint for which people leave for AE ("duplicate that layer and mask it"). **The positional model (a mask-effect in the chain, VEGAS/FCP)** is tempting for its cheapness — zero new concepts — but its region is a PREFIX of the chain, the second mask intersects the first, and "limit the second of five effects" cannot be expressed in it; in addition, the order of slots would mean two things at once. **Model 3 in pure form (a node graph with a second input and a canvas of connections)** was rejected in F-CLR-006, and this note does not propose reopening it: only the ADDRESS of the contour branches, while the picture remains a linear list.

Three supports on which the recommendation rests are already in place in OUR code: a reference as a non-numeric field of an effect — `Pan3d.camera`; a reference by name with a surviving dead reference — the morpher's areas; and resolving a reference into inline data before the plan is built — `effective_clips()`, thanks to which the engine, the post-FX cache key and both executors do not learn of the existence of references at all. Dividends: the mixer "original ↔ result by mask" is the SAME code that F-MPH-006 needs for overlay strength (a mask with a uniform level is exactly the wet/dry of a slot), while the parametric shape gets automation, copy-paste and hashing for free, because its fields are numbers in `params_for!`.

Coordination with F-MSK-001. It arrives at "a mask at the slot" from the other side — from geometry — and this is a confirmation, not a coincidence; we diverge in one place, and it should be recorded. There it is said "at the chain slot — as in FCP", but in live FCP everything is more complicated: its seven mask-effects cut the WHOLE layer (the quote about Draw Mask and Color Board above), and a per-effect region there exists only as built-in masks inside the effects themselves, and only they have Boolean modes. The reference for us is not FCP but Premiere for ownership and AE for addressing: the owner is the slot, the contour is a shared object which the slot references. The shape of the contour, the number of points and the modes of combining masks inside a slot are described by F-MSK-001, and a foreign alpha as the source of a key — by F-MSK-003; this note decides nothing about them.

Boundaries of the decision that should be fixed at once: a mask is allowed only on slots that do not change the raster (Fusion lists the same set of prohibitions); the first stage is parametric shapes (ellipse, rectangle, linear gradient), as F-MSK-001 established, because keys on the points of an arbitrary contour do not fit into our single curve widget. And an honest caveat: the user has not yet asked for masks (`docs/user-feedback.md` is silent about them) — this note closes the architectural fork in advance, so that the first mask does not arrive as "an effect in the chain", from which it would later be impossible to escape without a format change.

## Sources

- DaVinci Resolve 21 Reference Manual (July 2026, read locally): ch. 138
  "Secondary Windows" — "all of the windows within a particular node work
  together to limit that node’s grade", "If you don’t create a new node before
  creating a window, you’ll discover you’ve inappropriately changed a primary
  correction into a secondary correction", section "Combining Power Windows with
  the Mask Control" (add/subtract), section "Copying and Pasting Windows"
  (Copy Window / Paste Window into another node) and "Saving Window Presets"
  ("Loaded window presets overwrite whatever other windows were set up in that
  node"); ch. 146 "Combining Keys and Using Mattes" — Outside Nodes and Key Input
  Invert; ch. 79 "Rotoscoping with Masks" — Effect Mask input, "applied “post
  effect”", list of nodes without mask support, Pre-Masking Inputs, Garbage/Solid
  Matte. https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/8-design/fxplugins/beziermask.htm
  — VEGAS Bézier Masking: Mask FX ("mask the effects of all plug-in(s) in the
  effect chain that precede the Bézier mask plug-in"), a footnote about the cumulative
  intersection of masks, Invert, Blend as strength, up to five masks, tracking, point
  animation "new since VEGAS Pro 23" (2026-09-07). No public documentation for the
  2026 line exists — see the caveat in "Who has it and how".
- https://support.apple.com/guide/final-cut-pro/intro-to-masking-verce5464e1a/mac
  — FCP: seven mask effects, "Add them to any clip in the timeline just as you
  would add any other clip effect", separate "effect masking" for most effects,
  "Masking blend modes … are available for effect masking only" (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/use-multiple-masks-ver1b0945a01/mac
  — FCP: two masks on a clip = Intersect; the Add mode requires duplicating the clip
  as a linked clip above (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/mask-controls-verf8949b45e/mac
  — FCP: mask inversion ("reverses the mask—swapping its solid and transparent
  areas") (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/intro-to-effect-masking-verd9d69f195/12.3/mac/15.6
  — FCP: built-in effect masks, "you can isolate and control which areas of
  the video image are affected by clip effects" (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/combine-multiple-effect-masks-ver2531c0ab1/12.3/mac/15.6
  — FCP, the key boundary of scope: "mask blend modes interact within a single
  effect—not across multiple effects" (checked by a direct query, 2026-09-07)
- https://www.provideocoalition.com/great-ways-to-use-auto-mask-in-final-cut-pro/
  — FCP 12.3: the same Auto Mask both as a button in the header of EACH effect, and
  as a separate clip effect from the Masks and Keying category (2026-09-07)
- https://forum.shotcut.org/t/mask-apply-video-filter/12859/1 and
  https://discuss.kde.org/t/43705 — editors outside the core five, indicative of the
  positional model: an explicit terminator `Mask: Apply`, "combines every thing in
  between", "So, the order of the filters is very important", and a typical failure
  "some effects … applies the effect to the entire canvas rather than to the
  rotoscoped area I designated" (2026-09-07)
- https://forum.borisfx.com/t/14040 — a request to "share a contour between
  effects" in the words of a third-party plugin user; checked by a direct query
  (2026-09-07)
- https://creativecow.net/forums/thread/masking-out-single-effect-on-layer/ —
  AE, 2010: the question as posed and both workarounds (adjustment layer and layer
  duplicate) before Compositing Options appeared; checked by a direct query (2026-09-07)
- https://larryjordan.com/articles/adobe-updates-and-modifies-masking-in-premiere-26/
  — Premiere 26: the mask is "located inside the effect", "You can only apply one mask
  to one effect", an Unassigned mask is dragged onto the effect name, the Inverted
  checkbox (2026-09-07)
- Reddit was read through the Arctic Shift archive (direct reddit.com links do not open
  from this environment); quotes are given verbatim from the archived records, and the
  links point to the original permalinks: r/AfterEffects
  (`did_you_know_this_pro_tip`, `after_20_years_i_finally_figured_out`),
  r/premiere (`do_you_prefer_the_new_masking_method_in_premiere`,
  `need_help_with_applying_gaussian_blur`,
  `how_to_apply_effects_to_a_masked_object_without_a`,
  `how_can_i_mimic_this_effect_in_premiere`, `the_object_mask_tool_is`,
  `premiere_pro_2026_hate_it_or_love_it`), r/finalcutpro
  (`draw_mask`, `keyframe_copy_and_paste_not_working_on_mask`) — all
  2026-09-07.
- The After Effects line is taken from the neighboring note F-MPH-002 (Compositing
  Options, Effect Opacity, Mask Reference — verified there via
  `helpx.adobe.com`): in this pass `helpx.adobe.com` did not open at all
  (the proxy drops the connection), and the Adobe Community search returns 403. The mechanism
  is additionally confirmed by user descriptions "hit the “+” symbol.
  It’ll auto-add the first mask on the layer"
  (https://www.reddit.com/r/AfterEffects/comments/1twm374/one_comment_one_great_advice_the_holy_thread_of/opww2u8/).
- Not tested live in any editor: Resolve — per the official 21st-version manual,
  FCP — per the official 12.3 help, VEGAS — per the help for the 23rd line,
  Premiere — per a review of update 26 and forum descriptions.
